//! Symbol triggers. Typing one inserts a character. They live in ascii.json.

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub const FILE_NAME: &str = "ascii.json";

fn enabled_default() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Entry {
    pub id: String,
    pub trigger: String,
    pub text: String,
    #[serde(default = "enabled_default")]
    pub enabled: bool,
}

impl Default for Entry {
    fn default() -> Self {
        Self {
            id: String::new(),
            trigger: String::new(),
            text: String::new(),
            enabled: true,
        }
    }
}

fn default_prefix() -> String {
    ":".into()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct AsciiFile {
    entries: Vec<Entry>,
    #[serde(default = "default_prefix")]
    prefix: String,
}

impl Default for AsciiFile {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            prefix: default_prefix(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AsciiState {
    pub settings_folder: String,
    pub prefix: String,
    pub entries: Vec<Entry>,
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

pub fn starters() -> Vec<Entry> {
    vec![
        starter("1/4", "¼"),
        starter("1/2", "½"),
        starter("3/4", "¾"),
        starter("/", "×"),
        starter("go", "🔗"),
        starter("ce", "₠"),
        starter("bul", "•"),
        starter("check", "✓ "),
        starter("->", "➜"),
        starter("<-", "←"),
        starter("uarrow", "↑"),
        starter("darrow", "↓"),
        starter("rarrow", "→"),
        starter("...", "…"),
        starter("emdash", "—"),
        starter("endash", "–"),
        starter("deg", "°"),
        starter("+/-", "±"),
        starter("(c)", "©"),
        starter("(r)", "®"),
        starter("(tm)", "™"),
        starter("euro", "€"),
        starter("micro", "µ"),
        starter("inf", "∞"),
        starter("sqrt", "√"),
        starter("neq", "≠"),
        starter("lte", "≤"),
        starter("gte", "≥"),
        starter("approx", "≈"),
        starter("div", "÷"),
        starter("sect", "§"),
        starter("para", "¶"),
        starter("middot", "·"),
        starter("star", "★"),
        starter("xmark", "✗"),
    ]
}

fn starter(trigger: &str, text: &str) -> Entry {
    Entry {
        id: format!("starter-{trigger}"),
        trigger: trigger.to_string(),
        text: text.to_string(),
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
    let file: AsciiFile = serde_json::from_str(&raw).map_err(|error| {
        format!(
            "{} is not valid symbol data, so it was left unchanged: {error}",
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
            "Choose a settings folder. ascii.json is saved there with the settings file.".into(),
        );
    }
    let entries = normalize(entries)?;
    let prefix = normalize_prefix(prefix)?;
    write_file(&Path::new(folder).join(FILE_NAME), &entries, &prefix)?;
    Ok((entries, prefix))
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
            text: entry.text.replace("\r\n", "\n").replace('\r', "\n"),
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
        if entry.text.is_empty() {
            return Err(format!("\"{trigger}\" needs a character to insert."));
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
            text: entry.text.replace("\r\n", "\n").replace('\r', "\n"),
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
    format!("a{nanos:x}{tick:x}")
}

fn write_file(path: &Path, entries: &[Entry], prefix: &str) -> Result<(), String> {
    let mut payload = serde_json::to_vec_pretty(&AsciiFile {
        entries: entries.to_vec(),
        prefix: prefix.to_string(),
    })
    .map_err(|error| format!("Couldn't save symbols: {error}"))?;
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
    use super::starters;

    #[test]
    fn the_script_symbols_are_the_first_starters() {
        let entries = starters();
        let text = |trigger: &str| {
            entries
                .iter()
                .find(|entry| entry.trigger == trigger)
                .unwrap()
                .text
                .clone()
        };
        assert_eq!(text("1/4"), "¼");
        assert_eq!(text("1/2"), "½");
        assert_eq!(text("3/4"), "¾");
        assert_eq!(text("/"), "×");
        assert_eq!(text("go"), "🔗");
        assert_eq!(text("ce"), "₠");
        assert_eq!(text("bul"), "•");
        assert_eq!(text("check"), "✓ ");
        assert_eq!(text("->"), "➜");
        assert_eq!(text("<-"), "←");
        assert_eq!(text("uarrow"), "↑");
        assert_eq!(text("darrow"), "↓");
        assert!(entries.len() > 12);
    }
}
