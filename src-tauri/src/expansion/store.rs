use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub const FILE_NAME: &str = "expansion.json";

fn enabled_default() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snippet {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub trigger: String,
    #[serde(default)]
    pub text: String,
    #[serde(default = "enabled_default")]
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ExpansionFile {
    pub snippets: Vec<Snippet>,
}

impl Default for ExpansionFile {
    fn default() -> Self {
        Self {
            snippets: Vec::new(),
        }
    }
}

static IDS: AtomicU64 = AtomicU64::new(1);

pub fn new_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let tick = IDS.fetch_add(1, Ordering::Relaxed);
    format!("s{nanos:x}{tick:x}")
}

pub fn file_path(folder: &str) -> PathBuf {
    Path::new(folder).join(FILE_NAME)
}

#[derive(Debug)]
pub enum DiskVault {
    Legacy(ExpansionFile),
    Sealed,
}

pub fn read_disk(path: &Path) -> Result<Option<(String, DiskVault)>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(path).map_err(|error| format!("Couldn't read {}: {error}", path.display()))?;
    let raw = String::from_utf8(bytes).map_err(|_| format!("{} is not valid UTF-8.", path.display()))?;
    let vault = classify(&raw, path)?;
    Ok(Some((raw, vault)))
}

pub fn classify(raw: &str, path: &Path) -> Result<DiskVault, String> {
    let value: serde_json::Value = serde_json::from_str(raw).map_err(|error| {
        format!(
            "{} is not valid expansion data, so it was left unchanged: {error}",
            path.display()
        )
    })?;
    let encrypted = value.get("encrypted").and_then(|item| item.as_bool()).unwrap_or(false);
    if encrypted {
        return Ok(DiskVault::Sealed);
    }
    let file: ExpansionFile = serde_json::from_value(value).map_err(|error| {
        format!(
            "{} is not valid expansion data, so it was left unchanged: {error}",
            path.display()
        )
    })?;
    Ok(DiskVault::Legacy(file))
}

pub fn write_raw(path: &Path, raw: &str) -> Result<(), String> {
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, raw).map_err(|error| format!("Couldn't save {}: {error}", path.display()))?;
    if fs::rename(&temporary, path).is_ok() {
        return Ok(());
    }
    let wrote = fs::write(path, raw);
    let _ = fs::remove_file(&temporary);
    wrote.map_err(|error| format!("Couldn't save {}: {error}", path.display()))
}

pub fn normalize_snippets(snippets: Vec<Snippet>) -> Result<Vec<Snippet>, String> {
    let mut seen = std::collections::BTreeSet::new();
    let mut next = Vec::with_capacity(snippets.len());
    for snippet in snippets {
        let trigger = snippet.trigger.trim().to_string();
        if trigger.is_empty() {
            return Err("A trigger is required.".into());
        }
        if trigger.chars().any(|ch| ch.is_whitespace() || ch.is_control()) {
            return Err(format!(
                "Trigger \"{trigger}\" cannot contain spaces or line breaks."
            ));
        }
        if trigger.chars().count() > 64 {
            return Err(format!("Trigger \"{trigger}\" is longer than 64 characters."));
        }
        if !seen.insert(trigger.clone()) {
            return Err(format!("Trigger \"{trigger}\" is already used."));
        }
        let id = {
            let trimmed = snippet.id.trim();
            if trimmed.is_empty() {
                new_id()
            } else {
                trimmed.to_string()
            }
        };
        next.push(Snippet {
            id,
            trigger,
            text: snippet.text.replace("\r\n", "\n").replace('\r', "\n"),
            enabled: snippet.enabled,
        });
    }
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::{classify, DiskVault, ExpansionFile};

    #[test]
    fn an_older_file_without_enabled_still_loads() {
        let raw = r#"{"snippets":[{"id":"a","trigger":";sig","text":"Thanks"}],"extra":true}"#;
        let DiskVault::Legacy(file) = classify(raw, std::path::Path::new("expansion.json")).unwrap() else {
            panic!("legacy file");
        };
        let _ = file;
        let file: ExpansionFile = serde_json::from_str(raw).unwrap();
        assert_eq!(file.snippets.len(), 1);
        assert!(file.snippets[0].enabled);
        assert_eq!(file.snippets[0].text, "Thanks");
    }

    #[test]
    fn an_empty_object_loads_as_no_snippets() {
        let DiskVault::Legacy(file) = classify("{}", std::path::Path::new("expansion.json")).unwrap() else {
            panic!("legacy file");
        };
        assert!(file.snippets.is_empty());
    }

    #[test]
    fn an_encrypted_file_is_not_treated_as_snippets() {
        let raw = r#"{"version":1,"encrypted":true,"kdf":"argon2id","salt":"aa","nonce":"bb","ciphertext":"cc"}"#;
        assert!(matches!(classify(raw, std::path::Path::new("expansion.json")).unwrap(), DiskVault::Sealed));
    }

    #[test]
    fn invalid_json_is_an_error() {
        let error = classify("not json", std::path::Path::new("expansion.json")).unwrap_err();
        assert!(error.contains("left unchanged"));
    }
}
