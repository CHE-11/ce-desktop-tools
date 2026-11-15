//! Preview a batch rename, then rename only the rows the user confirms.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenamePlan {
    pub paths: Vec<String>,
    pub kind: String,
    pub text: String,
    #[serde(default)]
    pub replacement: String,
    #[serde(default = "default_start")]
    pub start: u32,
    #[serde(default = "default_pad")]
    pub pad: u32,
    #[serde(default)]
    pub names: Vec<NameEdit>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NameEdit {
    pub from: String,
    pub name: String,
}

fn default_start() -> u32 {
    1
}

fn default_pad() -> u32 {
    2
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameRow {
    pub from: String,
    pub to: String,
    pub ok: bool,
    pub message: String,
}

pub fn preview(plan: RenamePlan) -> Vec<RenameRow> {
    let files = files_of(&plan.paths);
    let width = plan.pad.clamp(1, 6) as usize;
    let targets: Vec<PathBuf> = files
        .iter()
        .enumerate()
        .map(|(index, path)| {
            let number = plan.start.saturating_add(index as u32);
            let from = path.display().to_string();
            if let Some(edit) = plan.names.iter().find(|edit| edit.from == from) {
                return path
                    .parent()
                    .unwrap_or_else(|| Path::new(""))
                    .join(&edit.name);
            }
            planned_name(
                path,
                &plan.kind,
                &plan.text,
                &plan.replacement,
                number,
                width,
            )
        })
        .collect();
    files
        .iter()
        .zip(targets.iter())
        .map(|(path, target)| judge(path, target, &files, &targets))
        .collect()
}

pub fn apply(rows: Vec<RenameRow>) -> Result<Vec<RenameRow>, String> {
    let mut done = Vec::with_capacity(rows.len());
    for row in rows {
        if !row.ok || row.from == row.to {
            done.push(row);
            continue;
        }
        match std::fs::rename(&row.from, &row.to) {
            Ok(()) => done.push(RenameRow {
                ok: false,
                message: "Renamed".into(),
                ..row
            }),
            Err(error) => done.push(RenameRow {
                ok: false,
                message: format!("Couldn't rename: {error}"),
                ..row
            }),
        }
    }
    Ok(done)
}

fn files_of(paths: &[String]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for raw in paths {
        let path = PathBuf::from(raw.trim());
        if path.is_file() {
            files.push(path);
            continue;
        }
        if path.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&path) {
                let mut nested = Vec::new();
                for entry in entries.flatten() {
                    let child = entry.path();
                    if child.is_file() {
                        nested.push(child);
                    }
                }
                nested.sort();
                files.extend(nested);
            }
        }
    }
    files.sort();
    files.dedup();
    files
}

fn planned_name(
    path: &Path,
    kind: &str,
    text: &str,
    replacement: &str,
    number: u32,
    pad: usize,
) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("file");
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    let current_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(stem);
    let file_name = match kind {
        "prefix" => join_extension(&format!("{text}{stem}"), extension),
        "suffix" => join_extension(&format!("{stem}{text}"), extension),
        "number" => join_extension(&format!("{stem} {number:0pad$}"), extension),
        "replace" if !text.is_empty() => current_name.replace(text, replacement),
        _ => current_name.to_string(),
    };
    parent.join(file_name)
}

fn join_extension(stem: &str, extension: &str) -> String {
    if extension.is_empty() {
        stem.to_string()
    } else {
        format!("{stem}.{extension}")
    }
}

fn judge(path: &Path, target: &Path, files: &[PathBuf], targets: &[PathBuf]) -> RenameRow {
    let from = path.display().to_string();
    let to = target.display().to_string();
    if path == target {
        return RenameRow {
            from,
            to,
            ok: false,
            message: "Unchanged".into(),
        };
    }
    let name = target
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    if let Some(problem) = illegal_name(name) {
        return RenameRow {
            from,
            to,
            ok: false,
            message: problem,
        };
    }
    if targets.iter().filter(|item| *item == target).count() > 1 {
        return RenameRow {
            from,
            to,
            ok: false,
            message: "Another selected file would use this name.".into(),
        };
    }
    if target.exists() && !files.iter().any(|file| file == target) {
        return RenameRow {
            from,
            to,
            ok: false,
            message: "A file with this name already exists.".into(),
        };
    }
    if files.iter().any(|file| file == target) {
        return RenameRow {
            from,
            to,
            ok: false,
            message: "That name belongs to another selected file.".into(),
        };
    }
    RenameRow {
        from,
        to,
        ok: true,
        message: "Ready".into(),
    }
}

fn illegal_name(name: &str) -> Option<String> {
    if name.is_empty() || name.ends_with('.') || name.ends_with(' ') {
        return Some("That file name is not allowed.".into());
    }
    if name.chars().any(|ch| {
        matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || (ch as u32) < 32
    }) {
        return Some("The name contains a character Windows does not allow.".into());
    }
    let stem = name.split('.').next().unwrap_or(name);
    const RESERVED: &[&str] = &[
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if RESERVED
        .iter()
        .any(|reserved| stem.eq_ignore_ascii_case(reserved))
    {
        return Some("That name is reserved by Windows.".into());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{illegal_name, planned_name};
    use std::path::Path;

    #[test]
    fn prefix_suffix_and_number_keep_the_extension() {
        let path = Path::new(r"C:\Photos\Vacation.jpg");
        assert_eq!(
            planned_name(path, "prefix", "Trip ", "", 1, 2)
                .file_name()
                .unwrap(),
            "Trip Vacation.jpg"
        );
        assert_eq!(
            planned_name(path, "suffix", " final", "", 1, 2)
                .file_name()
                .unwrap(),
            "Vacation final.jpg"
        );
        assert_eq!(
            planned_name(path, "number", "", "", 1, 2)
                .file_name()
                .unwrap(),
            "Vacation 01.jpg"
        );
        assert_eq!(
            planned_name(path, "replace", "Vacation", "Holiday", 1, 2)
                .file_name()
                .unwrap(),
            "Holiday.jpg"
        );
        assert_eq!(
            planned_name(path, "replace", ".jpg", ".png", 1, 2)
                .file_name()
                .unwrap(),
            "Vacation.png"
        );
    }

    #[test]
    fn reserved_and_punctuation_names_are_rejected() {
        assert!(illegal_name("CON.txt").is_some());
        assert!(illegal_name("notes?.txt").is_some());
        assert!(illegal_name("notes.txt").is_none());
    }
}
