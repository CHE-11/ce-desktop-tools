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
    #[serde(default)]
    pub hidden: bool,
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
    /// Older files encrypt every snippet. Public text is unavailable until unlock.
    Sealed,
    /// Current files keep ordinary snippets readable and seal only hidden replacement text.
    Split {
        snippets: Vec<Snippet>,
        seal: Option<serde_json::Value>,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HiddenSecret {
    pub id: String,
    pub text: String,
}

pub fn read_disk(path: &Path) -> Result<Option<(String, DiskVault)>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let bytes =
        fs::read(path).map_err(|error| format!("Couldn't read {}: {error}", path.display()))?;
    let raw =
        String::from_utf8(bytes).map_err(|_| format!("{} is not valid UTF-8.", path.display()))?;
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
    let encrypted = value
        .get("encrypted")
        .and_then(|item| item.as_bool())
        .unwrap_or(false);
    if encrypted && value.get("snippets").is_none() {
        return Ok(DiskVault::Sealed);
    }
    let version = value
        .get("version")
        .and_then(|item| item.as_u64())
        .unwrap_or(0);
    if version >= 2 || value.get("seal").is_some() {
        let snippets = value
            .get("snippets")
            .cloned()
            .unwrap_or_else(|| serde_json::Value::Array(Vec::new()));
        let snippets: Vec<Snippet> = serde_json::from_value(snippets).map_err(|error| {
            format!(
                "{} is not valid expansion data, so it was left unchanged: {error}",
                path.display()
            )
        })?;
        let seal = value.get("seal").filter(|item| !item.is_null()).cloned();
        return Ok(DiskVault::Split { snippets, seal });
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
    fs::write(&temporary, raw)
        .map_err(|error| format!("Couldn't save {}: {error}", path.display()))?;
    if fs::rename(&temporary, path).is_ok() {
        return Ok(());
    }
    let wrote = fs::write(path, raw);
    let _ = fs::remove_file(&temporary);
    wrote.map_err(|error| format!("Couldn't save {}: {error}", path.display()))
}

pub fn strip_hidden_text(snippets: &[Snippet]) -> Vec<Snippet> {
    snippets
        .iter()
        .map(|snippet| {
            let mut snippet = snippet.clone();
            if snippet.hidden {
                snippet.text.clear();
            }
            snippet
        })
        .collect()
}

pub fn hidden_secrets(snippets: &[Snippet]) -> Vec<HiddenSecret> {
    snippets
        .iter()
        .filter(|snippet| snippet.hidden)
        .map(|snippet| HiddenSecret {
            id: snippet.id.clone(),
            text: snippet.text.clone(),
        })
        .collect()
}

pub fn merge_secrets(mut snippets: Vec<Snippet>, secrets: &[HiddenSecret]) -> Vec<Snippet> {
    for snippet in &mut snippets {
        if !snippet.hidden {
            continue;
        }
        if let Some(secret) = secrets.iter().find(|secret| secret.id == snippet.id) {
            snippet.text = secret.text.clone();
        }
    }
    snippets
}

pub fn split_document(
    snippets: &[Snippet],
    seal: Option<&serde_json::Value>,
) -> Result<String, String> {
    let mut document = serde_json::Map::new();
    document.insert("version".to_string(), serde_json::json!(2));
    document.insert(
        "snippets".to_string(),
        serde_json::to_value(strip_hidden_text(snippets))
            .map_err(|error| format!("Couldn't save expansions: {error}"))?,
    );
    if let Some(seal) = seal {
        document.insert("seal".to_string(), seal.clone());
    }
    let mut raw = serde_json::to_vec_pretty(&document)
        .map_err(|error| format!("Couldn't save expansions: {error}"))?;
    raw.push(b'\n');
    String::from_utf8(raw).map_err(|_| "Couldn't save expansions.".to_string())
}

pub fn write_split(
    path: &Path,
    snippets: &[Snippet],
    seal: Option<&serde_json::Value>,
) -> Result<String, String> {
    let raw = split_document(snippets, seal)?;
    write_raw(path, &raw)?;
    Ok(raw)
}

pub fn write_legacy(path: &Path, snippets: &[Snippet]) -> Result<String, String> {
    let mut raw = serde_json::to_vec_pretty(&ExpansionFile {
        snippets: snippets.to_vec(),
    })
    .map_err(|error| format!("Couldn't save expansions: {error}"))?;
    raw.push(b'\n');
    let raw = String::from_utf8(raw).map_err(|_| "Couldn't save expansions.".to_string())?;
    write_raw(path, &raw)?;
    Ok(raw)
}

pub fn normalize_snippets(snippets: Vec<Snippet>) -> Result<Vec<Snippet>, String> {
    let mut seen = std::collections::BTreeSet::new();
    let mut next = Vec::with_capacity(snippets.len());
    for snippet in snippets {
        let trigger = snippet.trigger.trim().to_string();
        if trigger.is_empty() {
            return Err("A trigger is required.".into());
        }
        if trigger
            .chars()
            .any(|ch| ch.is_whitespace() || ch.is_control())
        {
            return Err(format!(
                "Trigger \"{trigger}\" cannot contain spaces or line breaks."
            ));
        }
        if trigger.chars().count() > 64 {
            return Err(format!(
                "Trigger \"{trigger}\" is longer than 64 characters."
            ));
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
            hidden: snippet.hidden,
        });
    }
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::{DiskVault, ExpansionFile, classify};

    #[test]
    fn an_older_file_without_enabled_still_loads() {
        let raw = r#"{"snippets":[{"id":"a","trigger":";sig","text":"Thanks"}],"extra":true}"#;
        let DiskVault::Legacy(file) =
            classify(raw, std::path::Path::new("expansion.json")).unwrap()
        else {
            panic!("legacy file");
        };
        let _ = file;
        let file: ExpansionFile = serde_json::from_str(raw).unwrap();
        assert_eq!(file.snippets.len(), 1);
        assert!(file.snippets[0].enabled);
        assert!(!file.snippets[0].hidden);
        assert_eq!(file.snippets[0].text, "Thanks");
    }

    #[test]
    fn an_empty_object_loads_as_no_snippets() {
        let DiskVault::Legacy(file) =
            classify("{}", std::path::Path::new("expansion.json")).unwrap()
        else {
            panic!("legacy file");
        };
        assert!(file.snippets.is_empty());
    }

    #[test]
    fn an_encrypted_file_is_not_treated_as_snippets() {
        let raw = r#"{"version":1,"encrypted":true,"kdf":"argon2id","salt":"aa","nonce":"bb","ciphertext":"cc"}"#;
        assert!(matches!(
            classify(raw, std::path::Path::new("expansion.json")).unwrap(),
            DiskVault::Sealed
        ));
    }

    #[test]
    fn invalid_json_is_an_error() {
        let error = classify("not json", std::path::Path::new("expansion.json")).unwrap_err();
        assert!(error.contains("left unchanged"));
    }

    #[test]
    fn a_split_file_keeps_hidden_text_out_of_the_public_json() {
        let snippets = vec![
            super::Snippet {
                id: "a".into(),
                trigger: ";sig".into(),
                text: "Thanks".into(),
                enabled: true,
                hidden: false,
            },
            super::Snippet {
                id: "b".into(),
                trigger: ";pin".into(),
                text: "secret-value".into(),
                enabled: true,
                hidden: true,
            },
        ];
        let seal = serde_json::json!({
            "encrypted": true,
            "ciphertext": "aa"
        });
        let raw = super::split_document(&snippets, Some(&seal)).unwrap();
        assert!(raw.contains("Thanks"));
        assert!(!raw.contains("secret-value"));
        assert!(raw.contains("\"hidden\": true"));
        let DiskVault::Split {
            snippets: loaded,
            seal: loaded_seal,
        } = classify(&raw, std::path::Path::new("expansion.json")).unwrap()
        else {
            panic!("split file");
        };
        assert_eq!(loaded[0].text, "Thanks");
        assert_eq!(loaded[1].text, "");
        assert!(loaded[1].hidden);
        assert!(loaded_seal.is_some());
        let merged = super::merge_secrets(
            loaded,
            &[super::HiddenSecret {
                id: "b".into(),
                text: "secret-value".into(),
            }],
        );
        assert_eq!(merged[1].text, "secret-value");
    }
}
