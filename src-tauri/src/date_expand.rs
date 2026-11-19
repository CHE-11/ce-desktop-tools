//! Date triggers. Typing one inserts the current date. They live in dates.json.

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub const FILE_NAME: &str = "dates.json";

fn enabled_default() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Entry {
    pub id: String,
    pub trigger: String,
    pub template: String,
    #[serde(default = "enabled_default")]
    pub enabled: bool,
}

impl Default for Entry {
    fn default() -> Self {
        Self {
            id: String::new(),
            trigger: String::new(),
            template: String::new(),
            enabled: true,
        }
    }
}

fn default_prefix() -> String {
    String::new()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct DateFile {
    entries: Vec<Entry>,
    #[serde(default = "default_prefix")]
    prefix: String,
}

impl Default for DateFile {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            prefix: default_prefix(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DateEntry {
    pub id: String,
    pub trigger: String,
    pub template: String,
    pub enabled: bool,
    pub preview: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DateState {
    pub settings_folder: String,
    pub prefix: String,
    pub entries: Vec<DateEntry>,
    pub error: Option<String>,
}

pub fn normalize_prefix(value: &str) -> Result<String, String> {
    let prefix = value.trim().to_string();
    if prefix.chars().count() > 4
        || prefix
            .chars()
            .any(|ch| ch.is_whitespace() || ch.is_control())
    {
        return Err("The prefix can be up to 4 characters, with no spaces.".into());
    }
    Ok(prefix)
}

#[derive(Clone, Debug)]
pub struct DateParts {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub weekday: u8,
    pub long_date: Option<String>,
}

pub fn starters() -> Vec<Entry> {
    vec![
        starter("longdate", "longdate"),
        starter("datetime", "yy/MM/dd - HH:mm"),
        starter(
            "odate",
            "---\rdate: {yy.MM.dd}\rlongdate: {longdate}\r---\r",
        ),
        starter("cdate", "yy.MM.dd"),
        starter("dt", "yy.MM.dd - HH.mmtt"),
        starter("/date", "MM/dd/yy"),
        starter("-date", "MM-dd-yy"),
        starter("date", "MM/dd/yy"),
        starter("time", "HH:mm"),
    ]
}

fn starter(trigger: &str, template: &str) -> Entry {
    Entry {
        id: format!("starter-{trigger}"),
        trigger: trigger.to_string(),
        template: template.to_string(),
        enabled: true,
    }
}

pub fn load(folder: &str) -> Result<(Vec<Entry>, String), String> {
    if folder.is_empty() || !Path::new(folder).is_dir() {
        return Ok((starters(), default_prefix()));
    }
    let path = Path::new(folder).join(FILE_NAME);
    if !path.is_file() {
        let entries = starters();
        let prefix = default_prefix();
        write_file(&path, &entries, &prefix)?;
        return Ok((entries, prefix));
    }
    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("Couldn't read {}: {error}", path.display()))?;
    let file: DateFile = serde_json::from_str(&raw).map_err(|error| {
        format!(
            "{} is not valid date data, so it was left unchanged: {error}",
            path.display()
        )
    })?;
    let prefix = normalize_prefix(&file.prefix).unwrap_or_else(|_| default_prefix());
    Ok((sanitize(file.entries), prefix))
}

pub fn save(
    folder: &str,
    entries: Vec<Entry>,
    prefix: &str,
) -> Result<(Vec<Entry>, String), String> {
    if folder.is_empty() || !Path::new(folder).is_dir() {
        return Err(
            "Choose a settings folder. dates.json is saved there with the settings file.".into(),
        );
    }
    let entries = normalize(entries)?;
    let prefix = normalize_prefix(prefix)?;
    write_file(&Path::new(folder).join(FILE_NAME), &entries, &prefix)?;
    Ok((entries, prefix))
}

pub fn present(entries: &[Entry]) -> Vec<DateEntry> {
    entries
        .iter()
        .map(|entry| DateEntry {
            id: entry.id.clone(),
            trigger: entry.trigger.clone(),
            template: entry.template.clone(),
            enabled: entry.enabled,
            preview: render_now(&entry.template),
        })
        .collect()
}

pub fn render_now(template: &str) -> String {
    render_at(template, &local_parts())
}

/// Private-use mark left where `{cursor}` was. The injector moves the caret here and does not type it.
pub const CURSOR: char = '\u{E000}';

struct VariableContext {
    clipboard: String,
    user: String,
    computer: String,
}

/// Replaces tokens in snippet text. Anything that is not a known token stays as written.
pub fn expand_variables(text: &str) -> String {
    expand_in(text, &local_parts(), &VariableContext::current())
}

#[cfg(test)]
fn expand_variables_at(text: &str, when: &DateParts) -> String {
    expand_in(text, when, &VariableContext::empty())
}

fn expand_in(text: &str, when: &DateParts, vars: &VariableContext) -> String {
    if !text.contains('{') {
        return text.to_string();
    }
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find('}') else {
            out.push('{');
            out.push_str(after);
            return out;
        };
        let name = after[..end].trim();
        if let Some(value) = brace_value(name, when, vars) {
            out.push_str(&value);
        } else {
            out.push('{');
            out.push_str(&after[..end]);
            out.push('}');
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

impl VariableContext {
    fn current() -> Self {
        Self {
            clipboard: crate::clipboard::read_clipboard_text()
                .unwrap_or_default()
                .replace(CURSOR, ""),
            user: env_first(&["USERNAME", "USER"]),
            computer: env_first(&["COMPUTERNAME", "HOSTNAME"]),
        }
    }

    #[cfg(test)]
    fn empty() -> Self {
        Self {
            clipboard: String::new(),
            user: String::new(),
            computer: String::new(),
        }
    }
}

fn env_first(names: &[&str]) -> String {
    names
        .iter()
        .find_map(|name| std::env::var(name).ok())
        .filter(|value| !value.is_empty())
        .unwrap_or_default()
}

fn brace_value(name: &str, when: &DateParts, vars: &VariableContext) -> Option<String> {
    if let Some(value) = named_value(name, vars) {
        return Some(value);
    }
    if name.is_empty() || !name.chars().any(|ch| ch.is_ascii_alphabetic()) {
        return None;
    }
    if name.eq_ignore_ascii_case("longdate") {
        return Some(format_picture(name, when));
    }
    let chars: Vec<char> = name.chars().collect();
    let mut index = 0;
    while index < chars.len() {
        if chars[index].is_ascii_alphabetic() {
            let rest: String = chars[index..].iter().collect();
            let (len, _) = take_token(&alias_prefix(&rest), when)?;
            index += len;
        } else {
            index += 1;
        }
    }
    Some(format_picture(name, when))
}

fn named_value(name: &str, vars: &VariableContext) -> Option<String> {
    if let Some(var) = env_var_name(name) {
        return std::env::var(var)
            .ok()
            .map(|value| value.replace(CURSOR, ""));
    }
    let value = match name.to_ascii_lowercase().as_str() {
        "clipboard" | "clip" => vars.clipboard.clone(),
        "user" | "username" => vars.user.clone(),
        "computer" | "hostname" | "host" => vars.computer.clone(),
        "uuid" | "guid" => new_uuid(),
        "enter" | "return" | "newline" | "nl" => "\n".to_string(),
        "tab" => "\t".to_string(),
        "cursor" | "caret" => CURSOR.to_string(),
        _ => return None,
    };
    Some(value)
}

fn env_var_name(name: &str) -> Option<&str> {
    let (prefix, rest) = name.split_at_checked(4)?;
    if !prefix.eq_ignore_ascii_case("env:") {
        return None;
    }
    let rest = rest.trim();
    let mut chars = rest.chars();
    let first = chars.next()?;
    if !(first.is_ascii_alphabetic() || first == '_')
        || chars.any(|ch| !(ch.is_ascii_alphanumeric() || ch == '_'))
    {
        return None;
    }
    Some(rest)
}

fn new_uuid() -> String {
    let mut bytes = rand::random::<[u8; 16]>();
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15]
    )
}

pub fn render_at(template: &str, when: &DateParts) -> String {
    if template.contains('{') {
        return expand_braces(template, when);
    }
    format_picture(template, when)
}

fn expand_braces(template: &str, when: &DateParts) -> String {
    let mut out = String::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find('}') else {
            out.push('{');
            out.push_str(after);
            return out;
        };
        out.push_str(&format_picture(&after[..end], when));
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

fn format_picture(picture: &str, when: &DateParts) -> String {
    if picture.eq_ignore_ascii_case("longdate") {
        return when.long_date.clone().unwrap_or_else(|| english_long(when));
    }
    let chars: Vec<char> = picture.chars().collect();
    let mut index = 0;
    let mut out = String::new();
    while index < chars.len() {
        let rest: String = chars[index..].iter().collect();
        if let Some((len, value)) = take_token(&alias_prefix(&rest), when) {
            out.push_str(&value);
            index += len;
        } else {
            out.push(chars[index]);
            index += 1;
        }
    }
    out
}

/// Uppercase year and day spellings people write next to `{MM}`. Length stays the same so the scan can advance.
fn alias_prefix(picture: &str) -> String {
    const ALIASES: &[(&str, &str)] = &[
        ("YYYY", "yyyy"),
        ("YY", "yy"),
        ("DDDD", "dddd"),
        ("DDD", "ddd"),
        ("DD", "dd"),
        ("D", "d"),
    ];
    for (from, to) in ALIASES {
        if let Some(rest) = picture.strip_prefix(from) {
            return format!("{to}{rest}");
        }
    }
    picture.to_string()
}

fn take_token(picture: &str, when: &DateParts) -> Option<(usize, String)> {
    let tokens: &[(&str, fn(&DateParts) -> String)] = &[
        ("yyyy", |when| when.year.to_string()),
        ("yy", |when| format!("{:02}", when.year % 100)),
        ("MMMM", |when| month_name(when.month).to_string()),
        ("MMM", |when| {
            month_name(when.month).get(..3).unwrap_or("").to_string()
        }),
        ("MM", |when| format!("{:02}", when.month)),
        ("M", |when| when.month.to_string()),
        ("dddd", |when| weekday_name(when.weekday).to_string()),
        ("ddd", |when| {
            weekday_name(when.weekday)
                .get(..3)
                .unwrap_or("")
                .to_string()
        }),
        ("dd", |when| format!("{:02}", when.day)),
        ("d", |when| when.day.to_string()),
        ("HH", |when| format!("{:02}", when.hour)),
        ("H", |when| when.hour.to_string()),
        ("hh", |when| format!("{:02}", hour12(when.hour))),
        ("h", |when| hour12(when.hour).to_string()),
        ("mm", |when| format!("{:02}", when.minute)),
        ("m", |when| when.minute.to_string()),
        ("ss", |when| format!("{:02}", when.second)),
        ("s", |when| when.second.to_string()),
        ("tt", |when| {
            if when.hour < 12 {
                "AM".into()
            } else {
                "PM".into()
            }
        }),
        ("t", |when| {
            if when.hour < 12 {
                "A".into()
            } else {
                "P".into()
            }
        }),
    ];
    for (token, render) in tokens {
        if picture.starts_with(token) {
            return Some((token.len(), render(when)));
        }
    }
    None
}

fn hour12(hour: u8) -> u8 {
    match hour % 12 {
        0 => 12,
        value => value,
    }
}

fn english_long(when: &DateParts) -> String {
    format!(
        "{}, {} {}, {}",
        weekday_name(when.weekday),
        month_name(when.month),
        when.day,
        when.year
    )
}

fn weekday_name(weekday: u8) -> &'static str {
    [
        "Sunday",
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
    ]
    .get(weekday as usize)
    .copied()
    .unwrap_or("Sunday")
}

fn month_name(month: u8) -> &'static str {
    [
        "",
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ]
    .get(month as usize)
    .copied()
    .unwrap_or("")
}

fn local_parts() -> DateParts {
    #[cfg(windows)]
    {
        let time = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
        let mut parts = DateParts {
            year: time.wYear,
            month: time.wMonth as u8,
            day: time.wDay as u8,
            hour: time.wHour as u8,
            minute: time.wMinute as u8,
            second: time.wSecond as u8,
            weekday: time.wDayOfWeek as u8,
            long_date: None,
        };
        parts.long_date = locale_long_date(&parts);
        return parts;
    }
    #[cfg(not(windows))]
    {
        DateParts {
            year: 2026,
            month: 11,
            day: 14,
            hour: 20,
            minute: 34,
            second: 0,
            weekday: 6,
            long_date: None,
        }
    }
}

#[cfg(windows)]
fn locale_long_date(when: &DateParts) -> Option<String> {
    use windows::Win32::Foundation::SYSTEMTIME;
    use windows::Win32::Globalization::{DATE_LONGDATE, GetDateFormatW, LOCALE_USER_DEFAULT};
    let time = SYSTEMTIME {
        wYear: when.year,
        wMonth: when.month as u16,
        wDayOfWeek: when.weekday as u16,
        wDay: when.day as u16,
        wHour: when.hour as u16,
        wMinute: when.minute as u16,
        wSecond: when.second as u16,
        wMilliseconds: 0,
    };
    let mut buffer = [0u16; 128];
    let written = unsafe {
        GetDateFormatW(
            LOCALE_USER_DEFAULT,
            DATE_LONGDATE.0,
            Some(&time),
            windows::core::PCWSTR::null(),
            Some(&mut buffer),
        )
    };
    if written <= 1 {
        return None;
    }
    Some(String::from_utf16_lossy(&buffer[..written as usize - 1]))
}

fn sanitize(entries: Vec<Entry>) -> Vec<Entry> {
    let mut seen = std::collections::BTreeSet::new();
    let mut next = Vec::new();
    for entry in entries {
        let trigger = entry.trigger.trim().to_string();
        if !valid_trigger(&trigger) || !seen.insert(trigger.to_ascii_lowercase()) {
            continue;
        }
        let id = {
            let trimmed = entry.id.trim();
            if trimmed.is_empty() {
                new_id()
            } else {
                trimmed.to_string()
            }
        };
        next.push(Entry {
            id,
            trigger,
            template: entry.template.replace("\r\n", "\n").replace('\r', "\n"),
            enabled: entry.enabled,
        });
    }
    next
}

fn normalize(entries: Vec<Entry>) -> Result<Vec<Entry>, String> {
    let mut seen = std::collections::BTreeSet::new();
    let mut next = Vec::new();
    for entry in entries {
        let trigger = entry.trigger.trim().to_string();
        if trigger.is_empty() {
            return Err("A trigger is required.".into());
        }
        if !valid_trigger(&trigger) {
            return Err(format!(
                "\"{trigger}\" cannot contain spaces or line breaks, and must be 64 characters or less."
            ));
        }
        if !seen.insert(trigger.to_ascii_lowercase()) {
            return Err(format!("\"{trigger}\" is already in the list."));
        }
        let id = {
            let trimmed = entry.id.trim();
            if trimmed.is_empty() {
                new_id()
            } else {
                trimmed.to_string()
            }
        };
        next.push(Entry {
            id,
            trigger,
            template: entry.template.replace("\r\n", "\n").replace('\r', "\n"),
            enabled: entry.enabled,
        });
    }
    Ok(next)
}

fn valid_trigger(trigger: &str) -> bool {
    !trigger.is_empty()
        && trigger.chars().count() <= 64
        && !trigger
            .chars()
            .any(|ch| ch.is_whitespace() || ch.is_control())
}

fn new_id() -> String {
    static IDS: AtomicU64 = AtomicU64::new(1);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let tick = IDS.fetch_add(1, Ordering::Relaxed);
    format!("d{nanos:x}{tick:x}")
}

fn write_file(path: &Path, entries: &[Entry], prefix: &str) -> Result<(), String> {
    let mut payload = serde_json::to_vec_pretty(&DateFile {
        entries: entries.to_vec(),
        prefix: prefix.to_string(),
    })
    .map_err(|error| format!("Couldn't save dates: {error}"))?;
    payload.push(b'\n');
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, &payload)
        .map_err(|error| format!("Couldn't save {}: {error}", path.display()))?;
    if fs::rename(&temporary, path).is_ok() {
        return Ok(());
    }
    let wrote = fs::write(path, &payload);
    let _ = fs::remove_file(&temporary);
    wrote.map_err(|error| format!("Couldn't save {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::{DateParts, FILE_NAME, expand_variables_at, load, render_at, save, starters};

    fn sample() -> DateParts {
        DateParts {
            year: 2026,
            month: 11,
            day: 14,
            hour: 20,
            minute: 34,
            second: 5,
            weekday: 6,
            long_date: None,
        }
    }

    #[test]
    fn the_starter_formats_match_the_script() {
        let when = sample();
        let by_trigger = |name: &str| {
            starters()
                .into_iter()
                .find(|entry| entry.trigger == name)
                .unwrap()
                .template
        };
        assert_eq!(
            render_at(&by_trigger("datetime"), &when),
            "26/11/14 - 20:34"
        );
        assert_eq!(render_at(&by_trigger("cdate"), &when), "26.11.14");
        assert_eq!(render_at(&by_trigger("dt"), &when), "26.11.14 - 20.34PM");
        assert_eq!(render_at(&by_trigger("/date"), &when), "11/14/26");
        assert_eq!(render_at(&by_trigger("-date"), &when), "11-14-26");
        assert_eq!(render_at(&by_trigger("date"), &when), "11/14/26");
        assert_eq!(render_at(&by_trigger("time"), &when), "20:34");
        assert_eq!(
            render_at(&by_trigger("longdate"), &when),
            "Saturday, November 14, 2026"
        );
        let report = render_at(&by_trigger("odate"), &when);
        assert!(report.starts_with("---\rdate: 26.11.14\r"));
        assert!(report.contains("longdate: Saturday, November 14, 2026\r---\r"));
    }

    #[test]
    fn snippet_variables_fill_in_and_leave_other_braces_alone() {
        let when = sample();
        assert_eq!(
            expand_variables_at("Due {MM}/{DD}/{YYYY}", &when),
            "Due 11/14/2026"
        );
        assert_eq!(expand_variables_at("{dd} {YY}", &when), "14 26");
        assert_eq!(expand_variables_at("{MMMM} {ddd}", &when), "November Sat");
        assert_eq!(
            expand_variables_at("{HH}:{mm}:{ss} {tt}", &when),
            "20:34:05 PM"
        );
        assert_eq!(expand_variables_at("{h:mm tt}", &when), "8:34 PM");
        assert_eq!(
            expand_variables_at("{longdate}", &when),
            "Saturday, November 14, 2026"
        );
        assert_eq!(
            expand_variables_at("Call {hello} at 5", &when),
            "Call {hello} at 5"
        );
        assert_eq!(expand_variables_at("Still {MM", &when), "Still {MM");
        assert_eq!(expand_variables_at("No tokens", &when), "No tokens");
    }

    #[test]
    fn named_tokens_fill_in_without_rereading_their_values() {
        let when = sample();
        let vars = super::VariableContext {
            clipboard: "{MM}".into(),
            user: "Ada".into(),
            computer: "WORK".into(),
        };
        assert_eq!(
            super::expand_in("Hi {user} on {hostname}", &when, &vars),
            "Hi Ada on WORK"
        );
        assert_eq!(super::expand_in("[{clipboard}]", &when, &vars), "[{MM}]");
        assert_eq!(
            super::expand_in("a{cursor}b{caret}c", &when, &vars),
            format!("a{}b{}c", super::CURSOR, super::CURSOR)
        );
        assert_eq!(super::expand_in("a{enter}b", &when, &vars), "a\nb");
        assert_eq!(super::expand_in("a{tab}b", &when, &vars), "a\tb");
        assert_eq!(super::expand_in("{MM}", &when, &vars), "11");
        let both = super::expand_in("{uuid} {guid}", &when, &vars);
        let mut parts = both.split(' ');
        let first = parts.next().unwrap();
        let second = parts.next().unwrap();
        assert_eq!(first.len(), 36);
        assert_ne!(first, second);
        if let Ok(user) = std::env::var("USERNAME") {
            assert_eq!(super::expand_in("{env:USERNAME}", &when, &vars), user);
        }
        assert_eq!(
            super::expand_in("{env:CE_EXPANSION_MISSING}", &when, &vars),
            "{env:CE_EXPANSION_MISSING}"
        );
    }

    #[test]
    fn midnight_and_noon_use_a_twelve_hour_clock() {
        let mut when = sample();
        when.hour = 0;
        when.minute = 5;
        assert_eq!(render_at("h:mm tt", &when), "12:05 AM");
        when.hour = 12;
        assert_eq!(render_at("h:mm tt", &when), "12:05 PM");
    }

    #[test]
    fn an_older_file_without_a_prefix_still_loads() {
        let dir = std::env::temp_dir().join(format!("ce-dates-old-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(FILE_NAME),
            "{\"entries\":[{\"id\":\"a\",\"trigger\":\"date\",\"template\":\"MM/dd/yy\",\"enabled\":true}]}\n",
        )
        .unwrap();
        let (entries, prefix) = load(dir.to_str().unwrap()).unwrap();
        assert_eq!(prefix, "");
        assert_eq!(entries[0].trigger, "date");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_prefix_with_spaces_falls_back_on_load_and_is_kept_on_save() {
        let dir = std::env::temp_dir().join(format!("ce-dates-prefix-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(FILE_NAME), "{\"prefix\":\"a b\",\"entries\":[]}\n").unwrap();
        let (_, prefix) = load(dir.to_str().unwrap()).unwrap();
        assert_eq!(prefix, "");
        let (_, saved) = save(dir.to_str().unwrap(), starters(), ":").unwrap();
        assert_eq!(saved, ":");
        let (_, loaded) = load(dir.to_str().unwrap()).unwrap();
        assert_eq!(loaded, ":");
        let rejected = save(dir.to_str().unwrap(), starters(), "12345");
        assert!(rejected.is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
