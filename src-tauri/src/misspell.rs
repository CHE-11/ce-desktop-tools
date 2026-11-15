//! Common misspell corrections. They live in misspells.json beside the settings file.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub const FILE_NAME: &str = "misspells.json";

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

/// Bump when `starters` gains a row. An older file picks up those rows once, then a save keeps later deletions.
const STARTER_REVISION: u32 = 2;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct MisspellFile {
    entries: Vec<Entry>,
    revision: u32,
}

impl Default for MisspellFile {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            revision: 0,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MisspellState {
    pub settings_folder: String,
    pub entries: Vec<Entry>,
    pub error: Option<String>,
}

pub fn starters() -> Vec<Entry> {
    // Real words stay out: its, were, well, ill, lets, id, wed, shell, hell, shed.
    [
        ("aint", "ain't"),
        ("adn", "and"),
        ("alot", "a lot"),
        ("arent", "aren't"),
        ("becasue", "because"),
        ("becuase", "because"),
        ("buisness", "business"),
        ("cant", "can't"),
        ("couldnt", "couldn't"),
        ("couldve", "could've"),
        ("definately", "definitely"),
        ("didnt", "didn't"),
        ("doesnt", "doesn't"),
        ("dont", "don't"),
        ("freind", "friend"),
        ("gaurd", "guard"),
        ("hadnt", "hadn't"),
        ("hasnt", "hasn't"),
        ("havent", "haven't"),
        ("heres", "here's"),
        ("howd", "how'd"),
        ("hows", "how's"),
        ("hte", "the"),
        ("im", "I'm"),
        ("isnt", "isn't"),
        ("itd", "it'd"),
        ("itll", "it'll"),
        ("ive", "I've"),
        ("jsut", "just"),
        ("mightve", "might've"),
        ("mustnt", "mustn't"),
        ("mustve", "must've"),
        ("neednt", "needn't"),
        ("occured", "occurred"),
        ("recieve", "receive"),
        ("seperate", "separate"),
        ("shouldnt", "shouldn't"),
        ("shouldve", "should've"),
        ("taht", "that"),
        ("teh", "the"),
        ("thatd", "that'd"),
        ("thatll", "that'll"),
        ("thats", "that's"),
        ("therell", "there'll"),
        ("theres", "there's"),
        ("theyd", "they'd"),
        ("theyll", "they'll"),
        ("theyre", "they're"),
        ("theyve", "they've"),
        ("thier", "their"),
        ("tommorow", "tomorrow"),
        ("tommorrow", "tomorrow"),
        ("tomorow", "tomorrow"),
        ("untill", "until"),
        ("wasnt", "wasn't"),
        ("werent", "weren't"),
        ("weve", "we've"),
        ("whatd", "what'd"),
        ("whatll", "what'll"),
        ("whats", "what's"),
        ("wheres", "where's"),
        ("wich", "which"),
        ("wierd", "weird"),
        ("whod", "who'd"),
        ("wholl", "who'll"),
        ("whos", "who's"),
        ("wont", "won't"),
        ("wouldnt", "wouldn't"),
        ("wouldve", "would've"),
        ("youd", "you'd"),
        ("youll", "you'll"),
        ("youre", "you're"),
        ("youve", "you've"),
        ("yuo", "you"),
    ]
    .into_iter()
    .flat_map(|(trigger, text)| both(trigger, text))
    .collect()
}

fn both(trigger: &str, text: &str) -> [Entry; 2] {
    [
        starter(trigger, text),
        starter(&capitalize(trigger), &capitalize(text)),
    ]
}

fn capitalize(value: &str) -> String {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut upper = first.to_uppercase().collect::<String>();
    upper.push_str(chars.as_str());
    upper
}

fn starter(trigger: &str, text: &str) -> Entry {
    Entry {
        id: format!("starter-{trigger}"),
        trigger: trigger.to_string(),
        text: text.to_string(),
        enabled: true,
    }
}

pub fn load(folder: &str) -> Result<Vec<Entry>, String> {
    if folder.is_empty() || !Path::new(folder).is_dir() {
        return Ok(starters());
    }
    let path = file_path(folder);
    if !path.is_file() {
        let entries = starters();
        write_file(&path, &entries)?;
        return Ok(entries);
    }
    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("Couldn't read {}: {error}", path.display()))?;
    let file: MisspellFile = serde_json::from_str(&raw).map_err(|error| {
        format!(
            "{} is not valid misspell data, so it was left unchanged: {error}",
            path.display()
        )
    })?;
    Ok(migrate(sanitize(file.entries), file.revision))
}

fn migrate(mut entries: Vec<Entry>, revision: u32) -> Vec<Entry> {
    if revision >= STARTER_REVISION {
        return entries;
    }
    let seen: std::collections::BTreeSet<String> =
        entries.iter().map(|entry| entry.trigger.clone()).collect();
    for starter in starters() {
        if !seen.contains(&starter.trigger) {
            entries.push(starter);
        }
    }
    entries
}

pub fn save(folder: &str, entries: Vec<Entry>) -> Result<Vec<Entry>, String> {
    if folder.is_empty() || !Path::new(folder).is_dir() {
        return Err(
            "Choose a settings folder. misspells.json is saved there with the settings file."
                .into(),
        );
    }
    let entries = normalize(entries)?;
    write_file(&file_path(folder), &entries)?;
    Ok(entries)
}

fn file_path(folder: &str) -> PathBuf {
    Path::new(folder).join(FILE_NAME)
}

fn sanitize(entries: Vec<Entry>) -> Vec<Entry> {
    let mut seen = std::collections::BTreeSet::new();
    let mut next = Vec::new();
    for entry in entries {
        let trigger = entry.trigger.trim().to_string();
        if !valid_trigger(&trigger) || !seen.insert(trigger.clone()) {
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
            return Err("A typed word is required.".into());
        }
        if !valid_trigger(&trigger) {
            return Err(format!(
                "\"{trigger}\" cannot contain spaces or line breaks, and must be 64 characters or less."
            ));
        }
        if !seen.insert(trigger.clone()) {
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
    format!("m{nanos:x}{tick:x}")
}

fn write_file(path: &Path, entries: &[Entry]) -> Result<(), String> {
    let mut payload = serde_json::to_vec_pretty(&MisspellFile {
        entries: entries.to_vec(),
        revision: STARTER_REVISION,
    })
    .map_err(|error| format!("Couldn't save misspells: {error}"))?;
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
    use super::{Entry, MisspellFile, load, save, starters};

    #[test]
    fn starters_keep_case_separate() {
        let entries = starters();
        let text = |trigger: &str| {
            entries
                .iter()
                .find(|entry| entry.trigger == trigger)
                .unwrap()
                .text
                .as_str()
        };
        assert_eq!(text("dont"), "don't");
        assert_eq!(text("Dont"), "Don't");
        assert_eq!(text("wont"), "won't");
        assert_eq!(text("Wont"), "Won't");
        assert_eq!(text("doesnt"), "doesn't");
        assert_eq!(text("im"), "I'm");
        assert_eq!(text("Im"), "I'm");
        assert_eq!(text("youre"), "you're");
        assert_eq!(text("alot"), "a lot");
        assert_eq!(text("Alot"), "A lot");
        assert_eq!(text("teh"), "the");
        assert!(entries.iter().all(|entry| entry.enabled));
        let mut seen = std::collections::BTreeSet::new();
        for entry in &entries {
            assert!(seen.insert(entry.trigger.clone()));
        }
        for trigger in ["its", "Its", "were", "well", "ill", "lets", "id"] {
            assert!(!seen.contains(trigger));
        }
    }

    #[test]
    fn an_older_file_without_enabled_still_loads() {
        let directory = std::env::temp_dir().join(format!(
            "ce-desktop-tools-misspell-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("misspells.json");
        std::fs::write(&path, br#"{"entries":[{"id":"a","trigger":"dont","text":"don't"},{"trigger":"  ","text":"skip"}]}"#).unwrap();
        let loaded = load(&directory.to_string_lossy()).unwrap();
        let kept = loaded.iter().find(|entry| entry.trigger == "dont").unwrap();
        assert!(kept.enabled);
        assert_eq!(kept.id, "a");
        assert_eq!(kept.text, "don't");
        assert_eq!(
            loaded
                .iter()
                .filter(|entry| entry.trigger == "dont")
                .count(),
            1
        );
        assert!(
            loaded
                .iter()
                .any(|entry| entry.trigger == "cant" && entry.text == "can't")
        );
        let raw = std::fs::read_to_string(&path).unwrap();
        let file: MisspellFile = serde_json::from_str(&raw).unwrap();
        assert_eq!(file.entries.len(), 2);
        assert_eq!(file.revision, 0);
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn saving_a_shorter_list_does_not_bring_starters_back() {
        let directory = std::env::temp_dir().join(format!(
            "ce-desktop-tools-misspell-save-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let folder = directory.to_string_lossy().to_string();
        let saved = save(
            &folder,
            vec![Entry {
                id: "a".into(),
                trigger: "dont".into(),
                text: "do not".into(),
                enabled: true,
            }],
        )
        .unwrap();
        assert_eq!(saved.len(), 1);
        let loaded = load(&folder).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].text, "do not");
        let _ = std::fs::remove_dir_all(&directory);
    }
}
