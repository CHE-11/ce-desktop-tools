//! Selection toolbar.
//!
//! A mouse hook notices a drag or a double-click. A worker thread reads the
//! selection and shows a borderless window. Keystrokes are sent from that
//! worker, never from the hook callback. The enabled switch controls that
//! selection popup. A saved hotkey opens the bar even while the switch is off.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

const DEFAULT_INSTALLED: &[&str] = &["copy", "cut", "paste", "search", "open-link"];
const MAX_TEXT: usize = 100_000;
const MAX_CUSTOM: usize = 40;
const MAX_EXCLUDED: usize = 50;
const MAX_NAME: usize = 40;
const MAX_TEMPLATE: usize = 500;
const MAX_ARG: usize = 80;

const ICONS: &[&str] = &[
    "copy",
    "scissors",
    "clipboard-paste",
    "clipboard",
    "search",
    "link",
    "case-upper",
    "case-lower",
    "case-sensitive",
    "wrap-text",
    "space",
    "parentheses",
    "brackets",
    "braces",
    "quote",
    "hash",
    "binary",
    "globe",
    "book-open",
    "book-marked",
    "languages",
    "map-pin",
    "youtube",
    "sparkles",
    "type",
    "eraser",
    "pilcrow",
    "file-text",
    "mail",
    "calculator",
    "baseline",
    "text-quote",
];

const OPS: &[&str] = &[
    "upper",
    "lower",
    "title",
    "trim",
    "remove-line-breaks",
    "remove-whitespace",
    "wrap",
    "replace",
    "base64-encode",
    "base64-decode",
];

const GOOGLE: &str = "https://www.google.com/search?q={text}";
const DUCKDUCKGO: &str = "https://duckduckgo.com/?q={text}";
const WIKIPEDIA: &str = "https://en.wikipedia.org/wiki/Special:Search?search={text}";
const MAPS: &str = "https://www.google.com/maps/search/?api=1&query={text}";
const YOUTUBE: &str = "https://www.youtube.com/results?search_query={text}";
const DICTIONARY: &str = "https://www.dictionary.com/browse/{text}";
const THESAURUS: &str = "https://www.thesaurus.com/browse/{text}";
const TRANSLATE: &str = "https://translate.google.com/?sl=auto&tl=en&text={text}&op=translate";

struct Builtin {
    id: &'static str,
    name: &'static str,
    group: &'static str,
    icon: &'static str,
}

fn builtins() -> &'static [Builtin] {
    &[
        Builtin {
            id: "copy",
            name: "Copy",
            group: "clipboard",
            icon: "copy",
        },
        Builtin {
            id: "cut",
            name: "Cut",
            group: "clipboard",
            icon: "scissors",
        },
        Builtin {
            id: "paste",
            name: "Paste",
            group: "clipboard",
            icon: "clipboard-paste",
        },
        Builtin {
            id: "paste-plain",
            name: "Paste plain",
            group: "tools",
            icon: "clipboard",
        },
        Builtin {
            id: "upper",
            name: "Uppercase",
            group: "text",
            icon: "case-upper",
        },
        Builtin {
            id: "lower",
            name: "Lowercase",
            group: "text",
            icon: "case-lower",
        },
        Builtin {
            id: "title",
            name: "Title case",
            group: "text",
            icon: "case-sensitive",
        },
        Builtin {
            id: "remove-line-breaks",
            name: "Remove line breaks",
            group: "text",
            icon: "wrap-text",
        },
        Builtin {
            id: "remove-whitespace",
            name: "Remove whitespace",
            group: "text",
            icon: "space",
        },
        Builtin {
            id: "remove-spaces",
            name: "Remove spaces",
            group: "text",
            icon: "space",
        },
        Builtin {
            id: "delete-specials",
            name: "Delete specials",
            group: "text",
            icon: "eraser",
        },
        Builtin {
            id: "slash-forward",
            name: "Backslashes to slashes",
            group: "text",
            icon: "arrow-left-right",
        },
        Builtin {
            id: "slash-back",
            name: "Slashes to backslashes",
            group: "text",
            icon: "arrow-left-right",
        },
        Builtin {
            id: "parentheses",
            name: "Parentheses",
            group: "text",
            icon: "parentheses",
        },
        Builtin {
            id: "brackets",
            name: "Brackets",
            group: "text",
            icon: "brackets",
        },
        Builtin {
            id: "dual-brackets",
            name: "Double brackets",
            group: "text",
            icon: "brackets",
        },
        Builtin {
            id: "braces",
            name: "Braces",
            group: "text",
            icon: "braces",
        },
        Builtin {
            id: "dual-braces",
            name: "Double braces",
            group: "text",
            icon: "braces",
        },
        Builtin {
            id: "quotes",
            name: "Quotes",
            group: "text",
            icon: "quote",
        },
        Builtin {
            id: "single-quotes",
            name: "Single quotes",
            group: "text",
            icon: "text-quote",
        },
        Builtin {
            id: "wrap-link",
            name: "Wrap link",
            group: "text",
            icon: "file-text",
        },
        Builtin {
            id: "character-count",
            name: "Character count",
            group: "text",
            icon: "hash",
        },
        Builtin {
            id: "base64-encode",
            name: "Base64 encode",
            group: "text",
            icon: "binary",
        },
        Builtin {
            id: "base64-decode",
            name: "Base64 decode",
            group: "text",
            icon: "binary",
        },
        Builtin {
            id: "search",
            name: "Search",
            group: "search",
            icon: "search",
        },
        Builtin {
            id: "duckduckgo",
            name: "DuckDuckGo",
            group: "search",
            icon: "globe",
        },
        Builtin {
            id: "wikipedia",
            name: "Wikipedia",
            group: "search",
            icon: "book-marked",
        },
        Builtin {
            id: "maps",
            name: "Maps",
            group: "search",
            icon: "map-pin",
        },
        Builtin {
            id: "youtube",
            name: "YouTube",
            group: "search",
            icon: "youtube",
        },
        Builtin {
            id: "dictionary",
            name: "Dictionary",
            group: "tools",
            icon: "book-open",
        },
        Builtin {
            id: "thesaurus",
            name: "Thesaurus",
            group: "tools",
            icon: "library",
        },
        Builtin {
            id: "translate",
            name: "Translate",
            group: "tools",
            icon: "languages",
        },
        Builtin {
            id: "open-link",
            name: "Open link",
            group: "tools",
            icon: "link",
        },
    ]
}

fn builtin(id: &str) -> Option<&'static Builtin> {
    builtins().iter().find(|entry| entry.id == id)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CustomTransform {
    pub op: String,
    pub prefix: String,
    pub suffix: String,
    pub find: String,
    pub replace_with: String,
}

impl Default for CustomTransform {
    fn default() -> Self {
        Self {
            op: "upper".into(),
            prefix: String::new(),
            suffix: String::new(),
            find: String::new(),
            replace_with: String::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CustomAction {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub kind: String,
    pub url_template: String,
    pub transform: CustomTransform,
    pub output: String,
}

impl Default for CustomAction {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            icon: "sparkles".into(),
            kind: "transform".into(),
            url_template: "https://example.com/search?q={text}".into(),
            transform: CustomTransform::default(),
            output: "replace".into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PantherbarFile {
    pub enabled: bool,
    pub installed: Option<Vec<String>>,
    pub custom: Vec<CustomAction>,
    pub excluded_apps: Vec<String>,
    pub hotkey: String,
}

impl Default for PantherbarFile {
    fn default() -> Self {
        Self {
            enabled: false,
            installed: None,
            custom: Vec::new(),
            excluded_apps: Vec::new(),
            hotkey: String::new(),
        }
    }
}

impl PantherbarFile {
    pub fn normalize(&mut self) {
        self.custom = std::mem::take(&mut self.custom)
            .into_iter()
            .filter_map(|action| accept_custom(action).ok())
            .take(MAX_CUSTOM)
            .collect();
        dedupe_custom(&mut self.custom);
        if let Some(installed) = self.installed.take() {
            self.installed = Some(clean_ids(&installed, &self.custom));
        }
        self.excluded_apps = clean_excluded(&self.excluded_apps);
        self.hotkey = canonical_hotkey(&self.hotkey);
    }

    pub fn prepare(mut self) -> Result<Self, String> {
        let mut custom = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut fresh = Vec::new();
        for mut action in self.custom {
            let is_new = action.id.trim().is_empty();
            if is_new {
                action.id = fresh_id(&seen);
            }
            let action = accept_custom(action)?;
            if !seen.insert(action.id.clone()) {
                return Err("Two actions use the same id.".into());
            }
            if is_new {
                fresh.push(action.id.clone());
            }
            custom.push(action);
            if custom.len() > MAX_CUSTOM {
                return Err("Pantherbar can keep 40 custom actions.".into());
            }
        }
        self.custom = custom;
        let mut installed = clean_ids(&self.installed.take().unwrap_or_default(), &self.custom);
        for id in fresh {
            if !installed.iter().any(|existing| existing == &id) {
                installed.push(id);
            }
        }
        self.installed = Some(installed);
        self.excluded_apps = clean_excluded(&self.excluded_apps);
        self.hotkey = canonical_hotkey(&self.hotkey);
        Ok(self)
    }

    fn resolved_ids(&self) -> Vec<String> {
        match &self.installed {
            None => DEFAULT_INSTALLED
                .iter()
                .map(|id| (*id).to_string())
                .collect(),
            Some(ids) => ids.clone(),
        }
    }

    fn visible(&self, text: &str) -> Vec<BarAction> {
        self.resolved_ids()
            .into_iter()
            .filter_map(|id| {
                if id == "open-link" && link_target(text).is_none() {
                    return None;
                }
                let (name, icon) = action_label(self, &id)?;
                Some(BarAction { id, name, icon })
            })
            .collect()
    }

    fn view(&self, last_app: String) -> PantherbarState {
        PantherbarState {
            enabled: self.enabled,
            installed: self.resolved_ids(),
            using_defaults: self.installed.is_none(),
            custom: self.custom.clone(),
            excluded_apps: self.excluded_apps.clone(),
            catalog: builtins()
                .iter()
                .map(|entry| CatalogEntry {
                    id: entry.id.to_string(),
                    name: entry.name.to_string(),
                    group: entry.group.to_string(),
                    icon: entry.icon.to_string(),
                })
                .collect(),
            icons: ICONS.iter().map(|icon| (*icon).to_string()).collect(),
            defaults: DEFAULT_INSTALLED
                .iter()
                .map(|id| (*id).to_string())
                .collect(),
            last_app,
            hotkey: self.hotkey.clone(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BarAction {
    pub id: String,
    pub name: String,
    pub icon: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogEntry {
    pub id: String,
    pub name: String,
    pub group: String,
    pub icon: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PantherbarState {
    pub enabled: bool,
    pub installed: Vec<String>,
    pub using_defaults: bool,
    pub custom: Vec<CustomAction>,
    pub excluded_apps: Vec<String>,
    pub catalog: Vec<CatalogEntry>,
    pub icons: Vec<String>,
    pub defaults: Vec<String>,
    pub last_app: String,
    pub hotkey: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PantherbarSave {
    pub enabled: bool,
    pub installed: Vec<String>,
    pub custom: Vec<CustomAction>,
    #[serde(default)]
    pub excluded_apps: Vec<String>,
    #[serde(default)]
    pub hotkey: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ShowPayload {
    actions: Vec<BarAction>,
}

fn action_label(file: &PantherbarFile, id: &str) -> Option<(String, String)> {
    if let Some(entry) = builtin(id) {
        return Some((entry.name.to_string(), entry.icon.to_string()));
    }
    file.custom
        .iter()
        .find(|action| action.id == id)
        .map(|action| (action.name.clone(), action.icon.clone()))
}

fn accept_custom(mut action: CustomAction) -> Result<CustomAction, String> {
    action.name = action.name.trim().to_string();
    if action.name.is_empty() || action.name.chars().count() > MAX_NAME {
        return Err("An action needs a name of 40 characters or fewer.".into());
    }
    if !ICONS.contains(&action.icon.as_str()) {
        return Err("Choose an icon from the list.".into());
    }
    action.id = action.id.trim().to_string();
    if action.id.is_empty() || builtin(&action.id).is_some() || !action.id.starts_with("custom-") {
        return Err("That action id is not available.".into());
    }
    match action.kind.trim() {
        "url" => {
            action.kind = "url".into();
            action.url_template = action.url_template.trim().to_string();
            if action.url_template.chars().count() > MAX_TEMPLATE
                || !action.url_template.contains("{text}")
            {
                return Err("The address needs {text} where the selection goes.".into());
            }
            if !(action.url_template.starts_with("https://")
                || action.url_template.starts_with("http://"))
            {
                return Err("The address needs to start with https://.".into());
            }
            action.output = "copy".into();
        }
        "transform" => {
            action.kind = "transform".into();
            action.transform.op = action.transform.op.trim().to_string();
            if !OPS.contains(&action.transform.op.as_str()) {
                return Err("Choose a text change from the list.".into());
            }
            action.transform.prefix = bounded("The text before", &action.transform.prefix)?;
            action.transform.suffix = bounded("The text after", &action.transform.suffix)?;
            action.transform.find = bounded("The text to find", &action.transform.find)?;
            action.transform.replace_with =
                bounded("The replacement", &action.transform.replace_with)?;
            if action.output != "copy" {
                action.output = "replace".into();
            }
        }
        _ => return Err("Choose a link or a text change.".into()),
    }
    Ok(action)
}

fn bounded(label: &str, value: &str) -> Result<String, String> {
    let value = value.trim().to_string();
    if value.chars().count() > MAX_ARG {
        return Err(format!(
            "{label} needs to be {MAX_ARG} characters or fewer."
        ));
    }
    Ok(value)
}

fn fresh_id(seen: &std::collections::HashSet<String>) -> String {
    loop {
        let id = format!("custom-{:x}", rand::random::<u64>());
        if !seen.contains(&id) {
            return id;
        }
    }
}

fn dedupe_custom(custom: &mut Vec<CustomAction>) {
    let mut seen = std::collections::HashSet::new();
    custom.retain(|action| seen.insert(action.id.clone()));
}

fn clean_ids(ids: &[String], custom: &[CustomAction]) -> Vec<String> {
    let mut out = Vec::new();
    for id in ids {
        let id = id.trim();
        if id.is_empty() {
            continue;
        }
        let known = builtin(id).is_some() || custom.iter().any(|action| action.id == id);
        if !known || out.iter().any(|existing: &String| existing == id) {
            continue;
        }
        out.push(id.to_string());
    }
    out
}

fn clean_excluded(apps: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for app in apps {
        let base = app.trim().rsplit(['\\', '/']).next().unwrap_or("").trim();
        if base.is_empty() || base.chars().count() > 64 {
            continue;
        }
        let name = if base.to_ascii_lowercase().ends_with(".exe") {
            base.to_string()
        } else {
            format!("{base}.exe")
        };
        if out
            .iter()
            .any(|existing: &String| existing.eq_ignore_ascii_case(&name))
        {
            continue;
        }
        out.push(name);
        if out.len() == MAX_EXCLUDED {
            break;
        }
    }
    out
}

fn looks_like_url(text: &str) -> bool {
    link_target(text).is_some()
}

fn link_target(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() || text.len() > 2000 || text.chars().any(char::is_whitespace) {
        return None;
    }
    if let Some(rest) = text
        .strip_prefix("https://")
        .or_else(|| text.strip_prefix("http://"))
    {
        if rest.starts_with("localhost") || rest.contains('.') {
            return Some(text.to_string());
        }
        return None;
    }
    let host = text.strip_prefix("www.").unwrap_or(text);
    let host_only = host.split(['/', '?', '#']).next().unwrap_or("");
    let mut parts = host_only.split('.');
    let first = parts.next()?;
    let last = parts.last().unwrap_or("");
    if first.is_empty() || !last.chars().all(|ch| ch.is_ascii_alphabetic()) {
        return None;
    }
    if !COMMON_TLD.contains(&last.to_ascii_lowercase().as_str()) {
        return None;
    }
    if !host_only
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '.' || ch == '-')
    {
        return None;
    }
    Some(format!("https://{text}"))
}

const COMMON_TLD: &[&str] = &[
    "com", "org", "net", "io", "dev", "app", "co", "uk", "us", "me", "info", "edu", "gov", "ai",
    "gg", "tv", "ly", "sh", "xyz", "page", "site",
];

fn apply_template(template: &str, text: &str) -> String {
    template.replace("{text}", &url_encode(text))
}

fn url_encode(text: &str) -> String {
    let mut out = String::new();
    for byte in text.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn title_case(text: &str) -> String {
    let mut out = String::new();
    let mut cap = true;
    for ch in text.chars() {
        if ch.is_whitespace() {
            cap = true;
            out.push(ch);
        } else if cap {
            out.extend(ch.to_uppercase());
            cap = false;
        } else {
            out.extend(ch.to_lowercase());
        }
    }
    out
}

fn remove_line_breaks(text: &str) -> String {
    let mut out = String::new();
    let mut pending_space = false;
    for ch in text.chars() {
        if ch == '\n' || ch == '\r' {
            pending_space = true;
            continue;
        }
        if pending_space && !out.is_empty() && !out.ends_with(' ') {
            out.push(' ');
        }
        pending_space = false;
        out.push(ch);
    }
    out.trim().to_string()
}

fn transform_text(op: &str, transform: &CustomTransform, text: &str) -> Result<String, String> {
    match op {
        "upper" => Ok(text.to_uppercase()),
        "lower" => Ok(text.to_lowercase()),
        "title" => Ok(title_case(text)),
        "trim" => Ok(text.trim().to_string()),
        "remove-line-breaks" => Ok(remove_line_breaks(text)),
        "remove-whitespace" => Ok(text.chars().filter(|ch| !ch.is_whitespace()).collect()),
        "remove-spaces" => Ok(text.replace(' ', "")),
        "delete-specials" => Ok(text
            .chars()
            .filter(|ch| ch.is_ascii_alphanumeric() || *ch == ' ')
            .collect()),
        "slash-forward" => Ok(text.replace('\\', "/")),
        "slash-back" => Ok(text.replace('/', "\\")),
        "wrap" => Ok(format!("{}{text}{}", transform.prefix, transform.suffix)),
        "replace" => {
            if transform.find.is_empty() {
                return Err("Enter the text to replace.".into());
            }
            Ok(text.replace(&transform.find, &transform.replace_with))
        }
        "base64-encode" => Ok(STANDARD.encode(text.as_bytes())),
        "base64-decode" => decode_base64(text),
        _ => Err("That text change is not available.".into()),
    }
}

fn decode_base64(text: &str) -> Result<String, String> {
    let cleaned: String = text.chars().filter(|ch| !ch.is_whitespace()).collect();
    let bytes = STANDARD
        .decode(cleaned)
        .map_err(|_| "That text is not valid Base64.".to_string())?;
    String::from_utf8(bytes).map_err(|_| "That Base64 text is not written in UTF-8.".to_string())
}

#[derive(Clone, Copy)]
pub(crate) struct Chord {
    pub(crate) ctrl: bool,
    pub(crate) alt: bool,
    pub(crate) shift: bool,
    pub(crate) win: bool,
    pub(crate) vk: u16,
}

pub(crate) fn canonical_hotkey(value: &str) -> String {
    parse_hotkey(value)
        .map(|chord| chord.label())
        .unwrap_or_default()
}

pub(crate) fn parse_hotkey(value: &str) -> Option<Chord> {
    let mut ctrl = false;
    let mut alt = false;
    let mut shift = false;
    let mut win = false;
    let mut vk = None;
    for part in value.split('+') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => ctrl = true,
            "alt" => alt = true,
            "shift" => shift = true,
            "win" | "meta" => win = true,
            other => {
                if vk.is_some() {
                    return None;
                }
                vk = Some(vk_of(other)?);
            }
        }
    }
    Some(Chord {
        ctrl,
        alt,
        shift,
        win,
        vk: vk?,
    })
}

fn vk_of(name: &str) -> Option<u16> {
    let bytes = name.as_bytes();
    if bytes.len() == 1 {
        let byte = bytes[0].to_ascii_uppercase();
        if byte.is_ascii_alphabetic() {
            return Some(byte as u16);
        }
        if byte.is_ascii_digit() {
            return Some(byte as u16);
        }
        return match byte {
            b';' => Some(0xBA),
            b'\'' => Some(0xDE),
            b',' => Some(0xBC),
            b'.' => Some(0xBE),
            b'/' => Some(0xBF),
            b'\\' => Some(0xDC),
            b'[' => Some(0xDB),
            b']' => Some(0xDD),
            b'-' => Some(0xBD),
            b'=' => Some(0xBB),
            b'`' => Some(0xC0),
            _ => None,
        };
    }
    let lower = name.to_ascii_lowercase();
    if let Some(rest) = lower.strip_prefix('f') {
        if let Ok(number) = rest.parse::<u16>() {
            if (1..=24).contains(&number) {
                return Some(0x6F + number);
            }
        }
    }
    if let Some(rest) = lower.strip_prefix("numpad") {
        if let Ok(number) = rest.parse::<u16>() {
            if number <= 9 {
                return Some(0x60 + number);
            }
        }
    }
    Some(match lower.as_str() {
        "space" => 0x20,
        "enter" => 0x0D,
        "tab" => 0x09,
        "esc" | "escape" => 0x1B,
        "backspace" => 0x08,
        "delete" => 0x2E,
        "insert" => 0x2D,
        "home" => 0x24,
        "end" => 0x23,
        "pageup" => 0x21,
        "pagedown" => 0x22,
        "left" => 0x25,
        "up" => 0x26,
        "right" => 0x27,
        "down" => 0x28,
        _ => return None,
    })
}

impl Chord {
    pub(crate) fn label(self) -> String {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("Ctrl".to_string());
        }
        if self.alt {
            parts.push("Alt".to_string());
        }
        if self.shift {
            parts.push("Shift".to_string());
        }
        if self.win {
            parts.push("Win".to_string());
        }
        parts.push(key_name(self.vk));
        parts.join("+")
    }
}

fn key_name(vk: u16) -> String {
    match vk {
        0x41..=0x5A => char::from(vk as u8).to_string(),
        0x30..=0x39 => char::from(vk as u8).to_string(),
        0x70..=0x87 => format!("F{}", vk - 0x6F),
        0x60..=0x69 => format!("Numpad{}", vk - 0x60),
        0x20 => "Space".into(),
        0x0D => "Enter".into(),
        0x09 => "Tab".into(),
        0x1B => "Esc".into(),
        0x08 => "Backspace".into(),
        0x2E => "Delete".into(),
        0x2D => "Insert".into(),
        0x24 => "Home".into(),
        0x23 => "End".into(),
        0x21 => "PageUp".into(),
        0x22 => "PageDown".into(),
        0x25 => "Left".into(),
        0x26 => "Up".into(),
        0x27 => "Right".into(),
        0x28 => "Down".into(),
        0xBA => ";".into(),
        0xDE => "'".into(),
        0xBC => ",".into(),
        0xBE => ".".into(),
        0xBF => "/".into(),
        0xDC => "\\".into(),
        0xDB => "[".into(),
        0xDD => "]".into(),
        0xBD => "-".into(),
        0xBB => "=".into(),
        0xC0 => "`".into(),
        _ => String::new(),
    }
}

fn count_message(text: &str) -> String {
    let characters = text.chars().count();
    let words = text.split_whitespace().count();
    format!("{characters} characters, {words} words")
}

struct Inner {
    app: AppHandle,
    file: Mutex<PantherbarFile>,
    selection: Mutex<String>,
    anchor: Mutex<(i32, i32)>,
    target: Mutex<isize>,
    last_exe: Mutex<String>,
    shown: AtomicBool,
    hook: Mutex<Option<HookSession>>,
}

struct HookSession {
    hook: Option<JoinHandle<()>>,
    worker: Option<JoinHandle<()>>,
}

enum Job {
    Hide,
    Note,
    Trigger,
    Read {
        x: i32,
        y: i32,
    },
    Fit {
        width: f64,
        height: f64,
    },
    Run {
        id: String,
        reply: Sender<Result<Option<String>, String>>,
    },
}

struct ScreenRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

struct Gesture {
    tracking: bool,
    down_x: i32,
    down_y: i32,
    over_bar: bool,
    double_click: bool,
    last_down: Option<(i32, i32, Instant)>,
}

static JOBS: Mutex<Option<Sender<Job>>> = Mutex::new(None);
static BAR_RECT: Mutex<Option<ScreenRect>> = Mutex::new(None);
static EPOCH: AtomicU64 = AtomicU64::new(0);
static GESTURE: Mutex<Gesture> = Mutex::new(Gesture {
    tracking: false,
    down_x: 0,
    down_y: 0,
    over_bar: false,
    double_click: false,
    last_down: None,
});
static HOOK_THREAD: AtomicU64 = AtomicU64::new(0);
static CHORD: Mutex<Option<Chord>> = Mutex::new(None);
static HELD_KEY: AtomicU16 = AtomicU16::new(0);

pub struct Pantherbar {
    inner: std::sync::Arc<Inner>,
}

impl Pantherbar {
    pub fn start(app: AppHandle) -> Result<Self, String> {
        let mut file = crate::app_settings::load_pantherbar(&app).unwrap_or_default();
        file.normalize();
        let listen = hook_wanted(file.enabled, &file.hotkey);
        if listen {
            ensure_window(&app)?;
        }
        publish_hotkey(&file.hotkey);
        let inner = std::sync::Arc::new(Inner {
            app,
            file: Mutex::new(file),
            selection: Mutex::new(String::new()),
            anchor: Mutex::new((0, 0)),
            target: Mutex::new(0),
            last_exe: Mutex::new(String::new()),
            shown: AtomicBool::new(false),
            hook: Mutex::new(None),
        });
        let pantherbar = Self { inner };
        if listen {
            pantherbar.start_hook();
        }
        Ok(pantherbar)
    }

    pub fn state(&self) -> PantherbarState {
        let file = lock_file(&self.inner);
        let last_app = self
            .inner
            .last_exe
            .lock()
            .map(|slot| slot.clone())
            .unwrap_or_default();
        file.view(last_app)
    }

    pub fn save(&self, save: PantherbarSave) -> Result<PantherbarState, String> {
        let next = PantherbarFile {
            enabled: save.enabled,
            installed: Some(save.installed),
            custom: save.custom,
            excluded_apps: save.excluded_apps,
            hotkey: save.hotkey,
        };
        let saved = crate::app_settings::save_pantherbar(&self.inner.app, next)?;
        if let Ok(mut file) = self.inner.file.lock() {
            *file = saved;
        }
        let file = lock_file(&self.inner);
        publish_hotkey(&file.hotkey);
        if hook_wanted(file.enabled, &file.hotkey) {
            ensure_window(&self.inner.app)?;
            self.start_hook();
            if !file.enabled {
                bump_epoch();
                self.inner.hide();
            }
        } else {
            bump_epoch();
            self.inner.hide();
            self.stop_hook();
            self.inner.hide();
            destroy_window(&self.inner.app);
        }
        Ok(self.state())
    }

    pub fn run(&self, id: String) -> Result<Option<String>, String> {
        if id == "manage" {
            return self.inner.open_page();
        }
        let (tx, rx) = mpsc::channel();
        enqueue(Job::Run { id, reply: tx })?;
        rx.recv_timeout(Duration::from_secs(4))
            .map_err(|_| "That action took too long.".to_string())?
    }

    pub fn fit(&self, width: f64, height: f64) {
        if !width.is_finite() || !height.is_finite() {
            return;
        }
        let _ = enqueue(Job::Fit {
            width: width.clamp(32.0, 1200.0),
            height: height.clamp(24.0, 400.0),
        });
    }

    fn start_hook(&self) {
        let Ok(mut slot) = self.inner.hook.lock() else {
            return;
        };
        if slot.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        if let Ok(mut jobs) = JOBS.lock() {
            *jobs = Some(tx);
        }
        let inner = std::sync::Arc::clone(&self.inner);
        let worker = std::thread::Builder::new()
            .name("pantherbar-worker".into())
            .spawn(move || worker_loop(inner, rx))
            .ok();
        let hook = std::thread::Builder::new()
            .name("pantherbar-hook".into())
            .spawn(run_hook)
            .ok();
        *slot = Some(HookSession { hook, worker });
    }

    fn stop_hook(&self) {
        let session = self.inner.hook.lock().ok().and_then(|mut slot| slot.take());
        let Some(session) = session else {
            return;
        };
        if let Ok(mut jobs) = JOBS.lock() {
            *jobs = None;
        }
        let thread_id = HOOK_THREAD.load(Ordering::SeqCst);
        if thread_id != 0 {
            post_quit(thread_id as u32);
        }
        if let Some(handle) = session.hook {
            let _ = handle.join();
        }
        if let Some(handle) = session.worker {
            let _ = handle.join();
        }
    }
}

fn lock_file(inner: &Inner) -> PantherbarFile {
    inner
        .file
        .lock()
        .map(|slot| slot.clone())
        .unwrap_or_default()
}

fn publish_hotkey(value: &str) {
    let chord = parse_hotkey(value);
    if let Ok(mut slot) = CHORD.lock() {
        *slot = chord;
    }
}

fn hook_wanted(enabled: bool, hotkey: &str) -> bool {
    enabled || !hotkey.is_empty()
}

fn popup_allowed(file: &PantherbarFile, from_hotkey: bool) -> bool {
    if from_hotkey {
        !file.hotkey.is_empty()
    } else {
        file.enabled
    }
}

fn enqueue(job: Job) -> Result<(), String> {
    let jobs = JOBS
        .lock()
        .map_err(|_| "Pantherbar is unavailable.".to_string())?;
    let tx = jobs
        .as_ref()
        .ok_or_else(|| "Turn Pantherbar on first.".to_string())?;
    tx.send(job).map_err(|_| "Pantherbar is off.".to_string())
}

fn bump_epoch() {
    EPOCH.fetch_add(1, Ordering::SeqCst);
}

fn epoch() -> u64 {
    EPOCH.load(Ordering::SeqCst)
}

fn worker_loop(inner: std::sync::Arc<Inner>, rx: Receiver<Job>) {
    prepare_worker();
    while let Ok(job) = rx.recv() {
        match job {
            Job::Hide => inner.hide(),
            Job::Note => inner.remember_foreground(),
            Job::Trigger => {
                let (x, y) = cursor_point();
                inner.read_selection(x, y, true);
            }
            Job::Read { x, y } => inner.read_selection(x, y, false),
            Job::Fit { width, height } => {
                if inner.shown.load(Ordering::SeqCst) {
                    inner.place(width, height);
                }
            }
            Job::Run { id, reply } => {
                let _ = reply.send(inner.perform(&id));
            }
        }
    }
}

impl Inner {
    fn remember_foreground(&self) {
        if let Some((pid, exe)) = foreground_process() {
            if pid != std::process::id() && !exe.is_empty() {
                if let Ok(mut slot) = self.last_exe.lock() {
                    *slot = exe;
                }
            }
        }
    }

    fn read_selection(&self, x: i32, y: i32, from_hotkey: bool) {
        if !popup_allowed(&lock_file(self), from_hotkey) {
            return;
        }
        let seen = epoch();
        std::thread::sleep(Duration::from_millis(40));
        if seen != epoch() {
            return;
        }
        self.remember_foreground();
        if let Some((pid, exe)) = foreground_process() {
            if pid == std::process::id() {
                return;
            }
            if lock_file(self)
                .excluded_apps
                .iter()
                .any(|name| name.eq_ignore_ascii_case(&exe))
            {
                return;
            }
        }
        self.capture_target();
        let Some(text) = read_text(&self.app) else {
            return;
        };
        if seen != epoch() || text.is_empty() {
            return;
        }
        if let Ok(mut slot) = self.selection.lock() {
            *slot = text.clone();
        }
        if let Ok(mut slot) = self.anchor.lock() {
            *slot = (x, y);
        }
        let actions = lock_file(self).visible(&text);
        if actions.is_empty() {
            self.hide();
            return;
        }
        if seen != epoch() || !popup_allowed(&lock_file(self), from_hotkey) {
            self.hide();
            return;
        }
        let (width, height) = estimate(actions.len());
        let _ = self.app.emit("pantherbar-show", ShowPayload { actions });
        self.shown.store(true, Ordering::SeqCst);
        self.place(width, height);
    }

    fn perform(&self, id: &str) -> Result<Option<String>, String> {
        let text = self
            .selection
            .lock()
            .map(|slot| slot.clone())
            .unwrap_or_default();
        if text.is_empty() {
            return Err("Select some text first.".into());
        }
        self.focus_target();
        let file = lock_file(self);
        let outcome = run_action(&self.app, &file, id, &text)?;
        match outcome {
            Outcome::Message(message) => Ok(Some(message)),
            Outcome::Done => {
                self.hide();
                Ok(None)
            }
        }
    }

    fn open_page(&self) -> Result<Option<String>, String> {
        self.hide();
        if let Some(window) = self.app.get_webview_window("main") {
            let _ = window.show();
            let _ = window.unminimize();
            let _ = window.set_focus();
        }
        let _ = self.app.emit("pantherbar-open", ());
        Ok(None)
    }

    fn hide(&self) {
        self.shown.store(false, Ordering::SeqCst);
        if let Ok(mut rect) = BAR_RECT.lock() {
            *rect = None;
        }
        hide_window(&self.app);
        let _ = self.app.emit("pantherbar-hide", ());
    }

    fn capture_target(&self) {
        #[cfg(windows)]
        {
            let hwnd = unsafe { windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow() };
            if let Ok(mut slot) = self.target.lock() {
                *slot = hwnd.0 as isize;
            }
        }
    }

    fn focus_target(&self) {
        #[cfg(windows)]
        {
            let raw = self.target.lock().map(|slot| *slot).unwrap_or(0);
            if raw == 0 {
                return;
            }
            unsafe {
                let _ = windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow(
                    windows::Win32::Foundation::HWND(raw as *mut std::ffi::c_void),
                );
            }
        }
    }

    fn place(&self, width: f64, height: f64) {
        let (anchor_x, anchor_y) = self.anchor.lock().map(|slot| *slot).unwrap_or((0, 0));
        place_window(&self.app, anchor_x, anchor_y, width, height);
    }
}

enum Outcome {
    Done,
    Message(String),
}

fn run_action(
    app: &AppHandle,
    file: &PantherbarFile,
    id: &str,
    text: &str,
) -> Result<Outcome, String> {
    if let Some(action) = file.custom.iter().find(|action| action.id == id) {
        return run_custom(app, action, text);
    }
    match id {
        "copy" => {
            crate::clipboard::set_clipboard_text(text)?;
            Ok(Outcome::Done)
        }
        "cut" => {
            send_key(0x58);
            Ok(Outcome::Done)
        }
        "paste" => {
            send_key(0x56);
            Ok(Outcome::Done)
        }
        "paste-plain" => {
            crate::paste_plain::paste_plain(app);
            Ok(Outcome::Done)
        }
        "open-link" => {
            let url =
                link_target(text).ok_or_else(|| "That selection is not a link.".to_string())?;
            open_url(&url)?;
            Ok(Outcome::Done)
        }
        "character-count" => Ok(Outcome::Message(count_message(text))),
        "search" => open_template(GOOGLE, text),
        "duckduckgo" => open_template(DUCKDUCKGO, text),
        "wikipedia" => open_template(WIKIPEDIA, text),
        "maps" => open_template(MAPS, text),
        "youtube" => open_template(YOUTUBE, text),
        "dictionary" => open_template(DICTIONARY, text),
        "thesaurus" => open_template(THESAURUS, text),
        "translate" => open_template(TRANSLATE, text),
        "upper" | "lower" | "title" | "remove-line-breaks" | "remove-whitespace"
        | "remove-spaces" | "delete-specials" | "slash-forward" | "slash-back"
        | "base64-encode" | "base64-decode" => {
            let next = transform_text(id, &CustomTransform::default(), text)?;
            paste_text(app, &next)?;
            Ok(Outcome::Done)
        }
        "parentheses" => paste_wrapped(app, text, "(", ")"),
        "brackets" => paste_wrapped(app, text, "[", "]"),
        "dual-brackets" => paste_wrapped(app, text, "[[", "]]"),
        "braces" => paste_wrapped(app, text, "{", "}"),
        "dual-braces" => paste_wrapped(app, text, "{{", "}}"),
        "quotes" => paste_wrapped(app, text, "\"", "\""),
        "single-quotes" => paste_wrapped(app, text, "'", "'"),
        "wrap-link" => {
            let href = crate::clipboard::read_clipboard_text()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "Copy a link first, then select the words to wrap.".to_string())?;
            paste_text(app, &format!("[{text}]({href})"))?;
            Ok(Outcome::Done)
        }
        _ => Err("That action is not on the bar.".into()),
    }
}

fn run_custom(app: &AppHandle, action: &CustomAction, text: &str) -> Result<Outcome, String> {
    if action.kind == "url" {
        open_url(&apply_template(&action.url_template, text))?;
        return Ok(Outcome::Done);
    }
    let next = transform_text(&action.transform.op, &action.transform, text)?;
    if action.output == "copy" {
        crate::clipboard::set_clipboard_text(&next)?;
    } else {
        paste_text(app, &next)?;
    }
    Ok(Outcome::Done)
}

fn open_template(template: &str, text: &str) -> Result<Outcome, String> {
    open_url(&apply_template(template, text))?;
    Ok(Outcome::Done)
}

fn paste_wrapped(
    app: &AppHandle,
    text: &str,
    prefix: &str,
    suffix: &str,
) -> Result<Outcome, String> {
    paste_text(app, &format!("{prefix}{text}{suffix}"))?;
    Ok(Outcome::Done)
}

fn paste_text(app: &AppHandle, text: &str) -> Result<(), String> {
    if let Some(clipboard) = app.try_state::<crate::clipboard::Clipboard>() {
        clipboard.silence_for(1_600);
    }
    let result = crate::paste_plain::with_clipboard(|| {
        let snapshot = crate::paste_plain::capture_clipboard();
        if crate::clipboard::set_clipboard_text(text).is_err() {
            if let Ok(snapshot) = &snapshot {
                crate::paste_plain::restore_clipboard(snapshot);
            }
            return Err("Couldn't paste the changed text.".to_string());
        }
        send_key(0x56);
        std::thread::sleep(Duration::from_millis(1_000));
        if let Ok(snapshot) = &snapshot {
            crate::paste_plain::restore_clipboard(snapshot);
        }
        Ok(())
    });
    result.unwrap_or_else(|| Err("The clipboard is busy.".into()))
}

fn open_url(url: &str) -> Result<(), String> {
    std::process::Command::new("explorer")
        .arg(url)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Couldn't open that link: {error}"))
}

fn send_key(virtual_key: u16) {
    crate::paste_plain::send_ctrl(virtual_key);
}

fn read_text(app: &AppHandle) -> Option<String> {
    let automation = uia_text();
    if let Some(text) = automation.as_ref() {
        if text.chars().count() <= 20_000 {
            return Some(text.chars().take(MAX_TEXT).collect());
        }
    }
    if let Some(text) = copy_probe(app) {
        return Some(text.chars().take(MAX_TEXT).collect());
    }
    automation.map(|text| text.chars().take(MAX_TEXT).collect())
}

fn copy_probe(app: &AppHandle) -> Option<String> {
    if let Some(clipboard) = app.try_state::<crate::clipboard::Clipboard>() {
        clipboard.silence_for(1_200);
    }
    crate::paste_plain::with_clipboard(|| {
        let snapshot = crate::paste_plain::capture_clipboard().ok()?;
        let before = clipboard_sequence();
        send_key(0x43);
        let mut found = None;
        for _ in 0..12 {
            std::thread::sleep(Duration::from_millis(20));
            if clipboard_sequence() != before {
                found = crate::clipboard::read_clipboard_text();
                break;
            }
        }
        crate::paste_plain::restore_clipboard(&snapshot);
        found
            .filter(|text| !text.trim().is_empty())
            .map(|text| text.trim().to_string())
    })
    .flatten()
}

fn estimate(count: usize) -> (f64, f64) {
    let buttons = count as f64 + 1.0;
    (8.0 + buttons * 34.0, 40.0)
}

fn ensure_window(app: &AppHandle) -> Result<(), String> {
    if app.get_webview_window("pantherbar").is_some() {
        return Ok(());
    }
    let window = tauri::WebviewWindowBuilder::new(
        app,
        "pantherbar",
        tauri::WebviewUrl::App("index.html".into()),
    )
    .title("Pantherbar")
    .inner_size(320.0, 40.0)
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(false)
    .focused(false)
    .resizable(false)
    .shadow(false)
    .initialization_script("window.__PANTHERBAR__ = true")
    .build()
    .map_err(|error| format!("Couldn't open the Pantherbar window: {error}"))?;
    let _ = window.set_focusable(false);
    disarm_activation(&window);
    Ok(())
}

fn prepare_worker() {
    #[cfg(windows)]
    unsafe {
        let _ = windows::Win32::System::Com::CoInitializeEx(
            None,
            windows::Win32::System::Com::COINIT_MULTITHREADED,
        );
    }
}

fn post_quit(thread_id: u32) {
    #[cfg(windows)]
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::PostThreadMessageW(
            thread_id,
            windows::Win32::UI::WindowsAndMessaging::WM_QUIT,
            windows::Win32::Foundation::WPARAM(0),
            windows::Win32::Foundation::LPARAM(0),
        );
    }
    #[cfg(not(windows))]
    {
        let _ = thread_id;
    }
}

fn run_hook() {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{
            DispatchMessageW, GetMessageW, MSG, SetWindowsHookExW, TranslateMessage,
            UnhookWindowsHookEx, WH_KEYBOARD_LL, WH_MOUSE_LL,
        };
        let Ok(hook) = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), None, 0) else {
            eprintln!("ce-desktop-tools pantherbar did not start");
            return;
        };
        let keyboard = SetWindowsHookExW(WH_KEYBOARD_LL, Some(key_proc), None, 0).ok();
        if keyboard.is_none() {
            eprintln!("ce-desktop-tools pantherbar hotkey did not start");
        }
        HOOK_THREAD.store(
            windows::Win32::System::Threading::GetCurrentThreadId() as u64,
            Ordering::SeqCst,
        );
        let mut message = MSG::default();
        loop {
            let status = GetMessageW(&mut message, None, 0, 0);
            if status.0 == 0 || status.0 == -1 {
                break;
            }
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        if let Some(keyboard) = keyboard {
            let _ = UnhookWindowsHookEx(keyboard);
        }
        let _ = UnhookWindowsHookEx(hook);
        HOOK_THREAD.store(0, Ordering::SeqCst);
    }
}

#[cfg(windows)]
unsafe extern "system" fn key_proc(
    code: i32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::Foundation::LRESULT;
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, KBDLLHOOKSTRUCT, LLKHF_INJECTED, LLKHF_UP, WM_KEYDOWN, WM_KEYUP,
        WM_SYSKEYDOWN, WM_SYSKEYUP,
    };

    if code < 0 {
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    }
    let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
    let message = wparam.0 as u32;
    let vk = info.vkCode as u16;
    if message == WM_KEYUP || message == WM_SYSKEYUP {
        if HELD_KEY.load(Ordering::SeqCst) == vk {
            HELD_KEY.store(0, Ordering::SeqCst);
        }
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    }
    if (message == WM_KEYDOWN || message == WM_SYSKEYDOWN)
        && !info.flags.contains(LLKHF_INJECTED)
        && !info.flags.contains(LLKHF_UP)
        && hotkey_pressed(vk)
    {
        return LRESULT(1);
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

#[cfg(windows)]
fn hotkey_pressed(vk: u16) -> bool {
    if foreground_pid() == std::process::id() {
        return false;
    }
    let chord = CHORD.lock().ok().and_then(|slot| *slot);
    let Some(chord) = chord else {
        return false;
    };
    if !chord_matches(chord, vk) {
        return false;
    }
    if HELD_KEY.swap(vk, Ordering::SeqCst) == vk {
        return true;
    }
    let _ = enqueue(Job::Trigger);
    true
}

#[cfg(windows)]
fn chord_matches(chord: Chord, vk: u16) -> bool {
    vk == chord.vk
        && modifier_down(&[0x11, 0xA2, 0xA3]) == chord.ctrl
        && modifier_down(&[0x12, 0xA4, 0xA5]) == chord.alt
        && modifier_down(&[0x10, 0xA0, 0xA1]) == chord.shift
        && modifier_down(&[0x5B, 0x5C]) == chord.win
}

#[cfg(windows)]
fn modifier_down(keys: &[u16]) -> bool {
    keys.iter().any(|vk| unsafe {
        windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(*vk as i32) as u16 & 0x8000
            != 0
    })
}

#[cfg(windows)]
fn foreground_pid() -> u32 {
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return 0;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        pid
    }
}

#[cfg(windows)]
fn cursor_point() -> (i32, i32) {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
    let mut point = POINT::default();
    if unsafe { GetCursorPos(&mut point) }.is_ok() {
        (point.x, point.y)
    } else {
        (0, 0)
    }
}

#[cfg(not(windows))]
fn cursor_point() -> (i32, i32) {
    (0, 0)
}

#[cfg(windows)]
unsafe extern "system" fn mouse_proc(
    code: i32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, MSLLHOOKSTRUCT, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN,
        WM_RBUTTONDOWN,
    };

    if code < 0 {
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    }
    let info = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
    let x = info.pt.x;
    let y = info.pt.y;
    match wparam.0 as u32 {
        value if value == WM_LBUTTONDOWN => on_mouse_down(x, y),
        value if value == WM_LBUTTONUP => on_mouse_up(x, y),
        value if value == WM_RBUTTONDOWN || value == WM_MBUTTONDOWN => {
            bump_epoch();
            let _ = enqueue(Job::Hide);
        }
        _ => {}
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

#[cfg(windows)]
fn on_mouse_down(x: i32, y: i32) {
    let over_bar = point_over_bar(x, y);
    if !over_bar {
        bump_epoch();
        let _ = enqueue(Job::Hide);
    }
    let mut gesture = GESTURE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let double_click = gesture.last_down.is_some_and(|(last_x, last_y, at)| {
        at.elapsed().as_millis() <= double_click_millis()
            && (x - last_x).abs() <= double_click_distance()
            && (y - last_y).abs() <= double_click_distance()
    });
    gesture.tracking = true;
    gesture.down_x = x;
    gesture.down_y = y;
    gesture.over_bar = over_bar;
    gesture.double_click = double_click;
    gesture.last_down = Some((x, y, Instant::now()));
}

#[cfg(windows)]
fn on_mouse_up(x: i32, y: i32) {
    let mut gesture = GESTURE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if !gesture.tracking {
        return;
    }
    gesture.tracking = false;
    if gesture.over_bar {
        return;
    }
    let dragged = (x - gesture.down_x).abs() > 8 || (y - gesture.down_y).abs() > 8;
    let selection = dragged || gesture.double_click;
    drop(gesture);
    let _ = enqueue(Job::Note);
    if selection {
        let _ = enqueue(Job::Read { x, y });
    }
}

#[cfg(windows)]
fn double_click_millis() -> u128 {
    500
}

#[cfg(windows)]
fn double_click_distance() -> i32 {
    unsafe {
        windows::Win32::UI::WindowsAndMessaging::GetSystemMetrics(
            windows::Win32::UI::WindowsAndMessaging::SM_CXDOUBLECLK,
        )
    }
}

fn point_over_bar(x: i32, y: i32) -> bool {
    let Ok(rect) = BAR_RECT.lock() else {
        return false;
    };
    let Some(rect) = rect.as_ref() else {
        return false;
    };
    x >= rect.left && x < rect.right && y >= rect.top && y < rect.bottom
}

#[cfg(windows)]
fn foreground_process() -> Option<(u32, String)> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
        QueryFullProcessImageNameW,
    };
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
    use windows::core::PWSTR;

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return None;
        }
        let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return Some((pid, String::new()));
        };
        let mut buffer = [0u16; 1024];
        let mut length = buffer.len() as u32;
        let queried = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut length,
        );
        let _ = CloseHandle(process);
        if queried.is_err() {
            return Some((pid, String::new()));
        }
        let path = String::from_utf16_lossy(&buffer[..length as usize]);
        let exe = path.rsplit(['\\', '/']).next().unwrap_or(&path).to_string();
        Some((pid, exe))
    }
}

#[cfg(not(windows))]
fn foreground_process() -> Option<(u32, String)> {
    None
}

#[cfg(windows)]
fn clipboard_sequence() -> u32 {
    unsafe { windows::Win32::System::DataExchange::GetClipboardSequenceNumber() }
}

#[cfg(not(windows))]
fn clipboard_sequence() -> u32 {
    0
}

fn uia_text() -> Option<String> {
    #[cfg(windows)]
    {
        uia_text_windows()
    }
    #[cfg(not(windows))]
    {
        None
    }
}

#[cfg(windows)]
fn uia_text_windows() -> Option<String> {
    use std::cell::RefCell;
    use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
    use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation};

    thread_local! {
        static AUTO: RefCell<Option<IUIAutomation>> = RefCell::new(None);
    }

    AUTO.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            *slot = unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok() };
        }
        let automation = slot.clone()?;
        let focused = unsafe { automation.GetFocusedElement() }.ok()?;
        if let Some(text) = text_of_element(&focused) {
            return Some(text);
        }
        let walker = unsafe { automation.ControlViewWalker() }.ok()?;
        let mut current = focused;
        for _ in 0..8 {
            current = unsafe { walker.GetParentElement(&current) }.ok()?;
            if let Some(text) = text_of_element(&current) {
                return Some(text);
            }
        }
        None
    })
}

#[cfg(windows)]
fn text_of_element(
    element: &windows::Win32::UI::Accessibility::IUIAutomationElement,
) -> Option<String> {
    use windows::Win32::UI::Accessibility::{IUIAutomationTextPattern, UIA_TextPatternId};
    let pattern: IUIAutomationTextPattern =
        unsafe { element.GetCurrentPatternAs(UIA_TextPatternId) }.ok()?;
    let ranges = unsafe { pattern.GetSelection() }.ok()?;
    let length = unsafe { ranges.Length() }.ok()?;
    if length <= 0 {
        return None;
    }
    let range = unsafe { ranges.GetElement(0) }.ok()?;
    let text = unsafe { range.GetText(-1) }.ok()?.to_string();
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn hide_window(app: &AppHandle) {
    #[cfg(windows)]
    if let Some(window) = app.get_webview_window("pantherbar") {
        let _ = window.hide();
        if let Some(hwnd) = native_hwnd(&window) {
            unsafe {
                let _ = windows::Win32::UI::WindowsAndMessaging::ShowWindow(
                    hwnd,
                    windows::Win32::UI::WindowsAndMessaging::SW_HIDE,
                );
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = app;
    }
}

fn destroy_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("pantherbar") {
        let _ = window.destroy();
    }
}

fn place_window(app: &AppHandle, anchor_x: i32, anchor_y: i32, width_css: f64, height_css: f64) {
    #[cfg(windows)]
    {
        let Some(window) = app.get_webview_window("pantherbar") else {
            return;
        };
        let Some(hwnd) = native_hwnd(&window) else {
            return;
        };
        let scale = window.scale_factor().unwrap_or(1.0).max(1.0);
        let width = (width_css * scale).ceil().clamp(1.0, 4000.0) as i32;
        let height = (height_css * scale).ceil().clamp(1.0, 2000.0) as i32;
        let (x, y) = clamp_to_work_area(anchor_x, anchor_y, width, height);
        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::{
                HWND_TOPMOST, SWP_NOACTIVATE, SWP_SHOWWINDOW, SetWindowPos,
            };
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                x,
                y,
                width,
                height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        }
        if let Ok(mut rect) = BAR_RECT.lock() {
            *rect = Some(ScreenRect {
                left: x,
                top: y,
                right: x + width,
                bottom: y + height,
            });
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (app, anchor_x, anchor_y, width_css, height_css);
    }
}

#[cfg(windows)]
fn clamp_to_work_area(anchor_x: i32, anchor_y: i32, width: i32, height: i32) -> (i32, i32) {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint,
    };

    let mut left = anchor_x.saturating_sub(width / 2);
    let mut top = anchor_y.saturating_add(18);
    let monitor = unsafe {
        MonitorFromPoint(
            POINT {
                x: anchor_x,
                y: anchor_y,
            },
            MONITOR_DEFAULTTONEAREST,
        )
    };
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        rcMonitor: windows::Win32::Foundation::RECT::default(),
        rcWork: windows::Win32::Foundation::RECT::default(),
        dwFlags: 0,
    };
    if unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        let work = info.rcWork;
        if top + height > work.bottom {
            top = anchor_y.saturating_sub(height + 12);
        }
        if left + width > work.right {
            left = work.right - width;
        }
        if top + height > work.bottom {
            top = work.bottom - height;
        }
        if left < work.left {
            left = work.left;
        }
        if top < work.top {
            top = work.top;
        }
    }
    (left, top)
}

#[cfg(windows)]
fn disarm_activation(window: &tauri::WebviewWindow) {
    use windows::Win32::UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GetWindowLongPtrW, SetWindowLongPtrW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    };
    let Some(hwnd) = native_hwnd(window) else {
        return;
    };
    unsafe {
        let current = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let next = current | WS_EX_NOACTIVATE.0 as isize | WS_EX_TOOLWINDOW.0 as isize;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, next);
    }
}

#[cfg(windows)]
fn native_hwnd(window: &tauri::WebviewWindow) -> Option<windows::Win32::Foundation::HWND> {
    let hwnd = window.hwnd().ok()?;
    Some(windows::Win32::Foundation::HWND(hwnd.0))
}

#[cfg(test)]
mod tests {
    use super::{
        CustomTransform, DEFAULT_INSTALLED, PantherbarFile, apply_template, canonical_hotkey,
        count_message, decode_base64, link_target, looks_like_url, remove_line_breaks, title_case,
        transform_text,
    };

    #[test]
    fn a_missing_pantherbar_value_stays_off_and_uses_the_default_bar() {
        let file: PantherbarFile = serde_json::from_str("{}").unwrap();
        assert!(!file.enabled);
        assert!(file.installed.is_none());
        assert!(file.custom.is_empty());
        assert!(file.hotkey.is_empty());
        let view = file.view(String::new());
        assert!(!view.enabled);
        assert!(view.using_defaults);
        assert_eq!(view.installed, DEFAULT_INSTALLED);
        let plain = file.visible("hello");
        assert!(plain.iter().any(|action| action.id == "copy"));
        assert!(plain.iter().all(|action| action.id != "open-link"));
        let link = file.visible("https://example.com/a");
        assert!(link.iter().any(|action| action.id == "open-link"));
    }

    #[test]
    fn unknown_actions_and_icons_drop_out_on_load() {
        let mut file: PantherbarFile = serde_json::from_str(
            r#"{"enabled":true,"installed":["copy","missing","search"],"custom":[{"id":"custom-1","name":"Look","icon":"nope","kind":"url","urlTemplate":"https://example.com/?q={text}"}],"excludedApps":["notepad","C:\\Windows\\explorer.exe","notepad.exe"]}"#,
        )
        .unwrap();
        file.normalize();
        assert!(file.enabled);
        assert_eq!(
            file.installed.as_deref(),
            Some(&["copy".to_string(), "search".to_string()][..])
        );
        assert!(file.custom.is_empty());
        assert_eq!(
            file.excluded_apps,
            vec!["notepad.exe".to_string(), "explorer.exe".to_string()]
        );
    }

    #[test]
    fn a_new_custom_action_is_put_on_the_bar() {
        let file = PantherbarFile {
            enabled: true,
            installed: Some(vec!["copy".into()]),
            custom: vec![super::CustomAction {
                id: String::new(),
                name: "Search example".into(),
                icon: "globe".into(),
                kind: "url".into(),
                url_template: "https://example.com/?q={text}".into(),
                ..super::CustomAction::default()
            }],
            excluded_apps: Vec::new(),
            hotkey: String::new(),
        };
        let saved = file.prepare().unwrap();
        let id = saved.custom[0].id.clone();
        assert!(id.starts_with("custom-"));
        assert!(saved.installed.unwrap().iter().any(|entry| entry == &id));
    }

    #[test]
    fn a_custom_action_needs_a_name() {
        let file = PantherbarFile {
            enabled: true,
            installed: Some(vec!["copy".into()]),
            custom: vec![super::CustomAction {
                name: "  ".into(),
                ..super::CustomAction::default()
            }],
            excluded_apps: Vec::new(),
            hotkey: String::new(),
        };
        assert!(file.prepare().is_err());
    }

    #[test]
    fn templates_encode_the_selection() {
        assert_eq!(
            apply_template("https://example.com/search?q={text}", "a b&c"),
            "https://example.com/search?q=a%20b%26c"
        );
    }

    #[test]
    fn links_are_detected_and_plain_words_are_not() {
        assert!(looks_like_url("https://example.com"));
        assert_eq!(
            link_target("example.com/docs"),
            Some("https://example.com/docs".into())
        );
        assert_eq!(
            link_target("http://localhost:1420"),
            Some("http://localhost:1420".into())
        );
        assert!(!looks_like_url("hello"));
        assert!(!looks_like_url("not a url"));
        assert!(!looks_like_url("file.txt"));
    }

    #[test]
    fn transforms_change_the_selection() {
        let wrap = CustomTransform {
            op: "wrap".into(),
            prefix: "(".into(),
            suffix: ")".into(),
            ..CustomTransform::default()
        };
        assert_eq!(
            transform_text("upper", &CustomTransform::default(), "Ab").unwrap(),
            "AB"
        );
        assert_eq!(title_case("hello THERE"), "Hello There");
        assert_eq!(remove_line_breaks("a\r\nb\nc"), "a b c");
        assert_eq!(
            transform_text("remove-spaces", &CustomTransform::default(), "a b c").unwrap(),
            "abc"
        );
        assert_eq!(
            transform_text("delete-specials", &CustomTransform::default(), "a-b_c!").unwrap(),
            "abc"
        );
        assert_eq!(
            transform_text("slash-forward", &CustomTransform::default(), r"a\b\c").unwrap(),
            "a/b/c"
        );
        assert_eq!(
            transform_text("slash-back", &CustomTransform::default(), "a/b/c").unwrap(),
            r"a\b\c"
        );
        assert_eq!(transform_text("wrap", &wrap, "hi").unwrap(), "(hi)");
        assert_eq!(
            transform_text(
                "replace",
                &CustomTransform {
                    op: "replace".into(),
                    find: "a".into(),
                    replace_with: "b".into(),
                    ..CustomTransform::default()
                },
                "aba"
            )
            .unwrap(),
            "bbb"
        );
        assert_eq!(
            transform_text("base64-encode", &CustomTransform::default(), "hi").unwrap(),
            "aGk="
        );
        assert_eq!(decode_base64("aGk=").unwrap(), "hi");
        assert!(decode_base64("!!!").is_err());
        assert_eq!(count_message("one two"), "7 characters, 2 words");
    }

    #[test]
    fn a_hotkey_keeps_a_stable_name_and_a_blank_one_stays_off() {
        assert_eq!(canonical_hotkey(""), "");
        assert_eq!(canonical_hotkey("nope"), "");
        assert_eq!(canonical_hotkey("ctrl+shift+p"), "Ctrl+Shift+P");
        assert_eq!(canonical_hotkey("alt+space"), "Alt+Space");
        assert_eq!(canonical_hotkey("F2"), "F2");
        let mut file = PantherbarFile::default();
        file.hotkey = "ctrl+p".into();
        file.normalize();
        assert_eq!(file.hotkey, "Ctrl+P");
    }

    #[test]
    fn the_switch_gates_selection_and_a_hotkey_stays_armed() {
        use super::{hook_wanted, popup_allowed};
        assert!(!hook_wanted(false, ""));
        assert!(hook_wanted(true, ""));
        assert!(hook_wanted(false, "Ctrl+P"));
        let mut file = PantherbarFile::default();
        assert!(!popup_allowed(&file, false));
        assert!(!popup_allowed(&file, true));
        file.hotkey = "Ctrl+P".into();
        assert!(!popup_allowed(&file, false));
        assert!(popup_allowed(&file, true));
        file.enabled = true;
        file.hotkey.clear();
        assert!(popup_allowed(&file, false));
        assert!(!popup_allowed(&file, true));
    }
}
