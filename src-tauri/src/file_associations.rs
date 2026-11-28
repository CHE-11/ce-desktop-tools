//! Per-user Windows file associations. Each association gets its own ProgID
//! under HKCU\Software\Classes with an open command built from a program path
//! and an argument template. The list lives in file-associations.json beside
//! the settings file, so a removed association can restore the old handler.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

pub const FILE_NAME: &str = "file-associations.json";
const PROG_ID_PREFIX: &str = "CeDesktopTools.";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FileAssociation {
    pub id: String,
    pub extension: String,
    pub program_path: String,
    pub arguments: String,
    pub label: String,
    pub icon_path: Option<String>,
    /// The `.ext` default ProgID that was in HKCU before this association replaced it.
    pub previous_prog_id: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct AssociationsFile {
    associations: Vec<FileAssociation>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileAssociationView {
    #[serde(flatten)]
    pub association: FileAssociation,
    /// `applied`, `userChoice`, or `notApplied`.
    pub status: String,
    pub command: String,
    pub user_choice_prog_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileAssociationsState {
    pub settings_folder: String,
    pub associations: Vec<FileAssociationView>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionHandler {
    pub extension: String,
    pub prog_id: Option<String>,
    /// `userChoice`, `classes`, or `none`.
    pub source: String,
    pub friendly_name: Option<String>,
    pub command: Option<String>,
}

pub fn state(app: &AppHandle) -> FileAssociationsState {
    let folder = crate::app_settings::settings_folder(app).unwrap_or_default();
    let (file, error) = if folder.is_empty() {
        (AssociationsFile::default(), None)
    } else {
        match load(&folder) {
            Ok(file) => (file, None),
            Err(error) => (AssociationsFile::default(), Some(error)),
        }
    };
    view_of(folder, file, error)
}

pub fn apply(
    app: &AppHandle,
    association: FileAssociation,
) -> Result<FileAssociationsState, String> {
    let folder = require_folder(app)?;
    let mut file = load(&folder)?;
    let mut next = prepare(association)?;
    if file
        .associations
        .iter()
        .any(|other| other.id != next.id && other.extension == next.extension)
    {
        return Err(format!(
            "{} already has an association in this list.",
            next.extension
        ));
    }
    let existing = file
        .associations
        .iter()
        .position(|other| other.id == next.id);
    if let Some(index) = existing {
        let old = &file.associations[index];
        next.previous_prog_id = old.previous_prog_id.clone();
        if old.extension != next.extension {
            registry::remove(old)?;
            next.previous_prog_id = None;
        }
    }
    next.previous_prog_id = registry::apply(&next)?.or(next.previous_prog_id);
    match existing {
        Some(index) => file.associations[index] = next,
        None => file.associations.push(next),
    }
    write_file(&file_path(&folder), &file)?;
    registry::notify();
    Ok(view_of(folder, file, None))
}

pub fn remove(app: &AppHandle, id: &str) -> Result<FileAssociationsState, String> {
    let folder = require_folder(app)?;
    let mut file = load(&folder)?;
    let Some(index) = file.associations.iter().position(|other| other.id == id) else {
        return Err("That association is no longer in the list.".into());
    };
    registry::remove(&file.associations[index])?;
    file.associations.remove(index);
    write_file(&file_path(&folder), &file)?;
    registry::notify();
    Ok(view_of(folder, file, None))
}

pub fn lookup(extension: &str) -> Result<ExtensionHandler, String> {
    let extension = normalize_extension(extension)?;
    Ok(registry::lookup(&extension))
}

pub fn preview(program_path: &str, arguments: &str, sample: &str) -> String {
    let command = command_line(program_path, arguments);
    let sample = if sample.trim().is_empty() {
        "C:\\example\\file.txt"
    } else {
        sample.trim()
    };
    command.replace("%1", sample).replace("%L", sample)
}

pub fn open_default_apps() -> Result<(), String> {
    std::process::Command::new("explorer.exe")
        .arg("ms-settings:defaultapps")
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Couldn't open Default Apps: {error}"))
}

/// Builds `"program" args`. A template without `%1` or `%L` gets `"%1"` at the end.
pub fn command_line(program_path: &str, arguments: &str) -> String {
    let program = program_path.trim().trim_matches('"');
    let arguments = arguments.trim();
    let mut command = format!("\"{program}\"");
    if !arguments.is_empty() {
        command.push(' ');
        command.push_str(arguments);
    }
    if !arguments.contains("%1") && !arguments.contains("%L") {
        command.push_str(" \"%1\"");
    }
    command
}

pub fn prog_id_for(extension: &str) -> String {
    format!("{PROG_ID_PREFIX}{}", extension.trim_start_matches('.'))
}

fn normalize_extension(value: &str) -> Result<String, String> {
    let trimmed = value
        .trim()
        .trim_start_matches('*')
        .trim_start_matches('.')
        .to_lowercase();
    if trimmed.is_empty() {
        return Err("Enter a file extension, such as .log.".into());
    }
    if !trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '+' | '.'))
    {
        return Err(format!(".{trimmed} is not a valid file extension."));
    }
    Ok(format!(".{trimmed}"))
}

fn prepare(mut association: FileAssociation) -> Result<FileAssociation, String> {
    association.extension = normalize_extension(&association.extension)?;
    association.program_path = association
        .program_path
        .trim()
        .trim_matches('"')
        .to_string();
    if association.program_path.is_empty() {
        return Err("Choose a program to open these files.".into());
    }
    if !Path::new(&association.program_path).is_file() {
        return Err(format!("Program not found: {}", association.program_path));
    }
    association.arguments = association.arguments.trim().to_string();
    association.label = association.label.trim().to_string();
    association.icon_path = association
        .icon_path
        .map(|path| path.trim().to_string())
        .filter(|path| !path.is_empty());
    if association.id.trim().is_empty() {
        association.id = format!("assoc-{}", association.extension.trim_start_matches('.'));
    }
    Ok(association)
}

fn view_of(folder: String, file: AssociationsFile, error: Option<String>) -> FileAssociationsState {
    let associations = file
        .associations
        .into_iter()
        .map(|association| {
            let (status, user_choice) = registry::status(&association);
            FileAssociationView {
                command: command_line(&association.program_path, &association.arguments),
                association,
                status: status.into(),
                user_choice_prog_id: user_choice,
            }
        })
        .collect();
    FileAssociationsState {
        settings_folder: folder,
        associations,
        error,
    }
}

fn require_folder(app: &AppHandle) -> Result<String, String> {
    let folder = crate::app_settings::settings_folder(app).unwrap_or_default();
    if folder.is_empty() || !Path::new(&folder).is_dir() {
        return Err(
            "Choose a settings folder. file-associations.json is saved there with the settings file."
                .into(),
        );
    }
    Ok(folder)
}

fn file_path(folder: &str) -> PathBuf {
    Path::new(folder).join(FILE_NAME)
}

fn load(folder: &str) -> Result<AssociationsFile, String> {
    let path = file_path(folder);
    if !path.is_file() {
        return Ok(AssociationsFile::default());
    }
    let bytes =
        fs::read(&path).map_err(|error| format!("Couldn't read {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|error| {
        format!(
            "{} is not a valid associations file: {error}",
            path.display()
        )
    })
}

fn write_file(path: &Path, file: &AssociationsFile) -> Result<(), String> {
    let mut payload = serde_json::to_vec_pretty(file)
        .map_err(|error| format!("Couldn't save associations: {error}"))?;
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

#[cfg(windows)]
mod registry {
    use super::{ExtensionHandler, FileAssociation, command_line, prog_id_for};
    use winreg::RegKey;
    use winreg::enums::{HKEY_CLASSES_ROOT, HKEY_CURRENT_USER, KEY_READ};

    const CLASSES: &str = r"Software\Classes";
    const FILE_EXTS: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts";

    fn hkcu() -> RegKey {
        RegKey::predef(HKEY_CURRENT_USER)
    }

    fn default_value(key: &RegKey, path: &str) -> Option<String> {
        key.open_subkey_with_flags(path, KEY_READ)
            .ok()?
            .get_value::<String, _>("")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    }

    fn user_choice(extension: &str) -> Option<String> {
        hkcu()
            .open_subkey_with_flags(format!(r"{FILE_EXTS}\{extension}\UserChoice"), KEY_READ)
            .ok()?
            .get_value::<String, _>("ProgId")
            .ok()
            .filter(|value| !value.trim().is_empty())
    }

    /// Writes the ProgID and points `.ext` at it. Returns the ProgID it replaced.
    pub fn apply(association: &FileAssociation) -> Result<Option<String>, String> {
        let prog_id = prog_id_for(&association.extension);
        let user = hkcu();
        let fail = |error: std::io::Error| format!("Couldn't write the association: {error}");
        let (prog_key, _) = user
            .create_subkey(format!(r"{CLASSES}\{prog_id}"))
            .map_err(fail)?;
        let label = if association.label.is_empty() {
            format!(
                "{} file",
                association.extension.trim_start_matches('.').to_uppercase()
            )
        } else {
            association.label.clone()
        };
        prog_key.set_value("", &label).map_err(fail)?;
        prog_key
            .set_value("FriendlyTypeName", &label)
            .map_err(fail)?;
        let icon = association
            .icon_path
            .clone()
            .unwrap_or_else(|| format!("\"{}\",0", association.program_path));
        let (icon_key, _) = prog_key.create_subkey("DefaultIcon").map_err(fail)?;
        icon_key.set_value("", &icon).map_err(fail)?;
        let (command_key, _) = prog_key
            .create_subkey(r"shell\open\command")
            .map_err(fail)?;
        command_key
            .set_value(
                "",
                &command_line(&association.program_path, &association.arguments),
            )
            .map_err(fail)?;

        let ext_path = format!(r"{CLASSES}\{}", association.extension);
        let previous = default_value(&user, &ext_path).filter(|value| value != &prog_id);
        let (ext_key, _) = user.create_subkey(&ext_path).map_err(fail)?;
        ext_key.set_value("", &prog_id).map_err(fail)?;
        let (open_with, _) = ext_key.create_subkey("OpenWithProgids").map_err(fail)?;
        open_with.set_value(&prog_id, &"").map_err(fail)?;
        Ok(previous)
    }

    pub fn remove(association: &FileAssociation) -> Result<(), String> {
        let prog_id = prog_id_for(&association.extension);
        let user = hkcu();
        let fail = |error: std::io::Error| format!("Couldn't remove the association: {error}");
        let ext_path = format!(r"{CLASSES}\{}", association.extension);
        if let Ok(ext_key) = user.open_subkey_with_flags(&ext_path, winreg::enums::KEY_ALL_ACCESS) {
            if let Ok(open_with) =
                ext_key.open_subkey_with_flags("OpenWithProgids", winreg::enums::KEY_ALL_ACCESS)
            {
                let _ = open_with.delete_value(&prog_id);
            }
            let current = ext_key.get_value::<String, _>("").unwrap_or_default();
            if current == prog_id {
                match &association.previous_prog_id {
                    Some(previous) => ext_key.set_value("", previous).map_err(fail)?,
                    None => {
                        let _ = ext_key.delete_value("");
                    }
                }
            }
        }
        match user.delete_subkey_all(format!(r"{CLASSES}\{prog_id}")) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(fail(error)),
        }
    }

    pub fn status(association: &FileAssociation) -> (&'static str, Option<String>) {
        let prog_id = prog_id_for(&association.extension);
        let user = hkcu();
        let applied = default_value(&user, &format!(r"{CLASSES}\{}", association.extension))
            .is_some_and(|value| value == prog_id)
            && default_value(&user, &format!(r"{CLASSES}\{prog_id}\shell\open\command")).is_some();
        if !applied {
            return ("notApplied", None);
        }
        match user_choice(&association.extension) {
            Some(choice) if !choice.eq_ignore_ascii_case(&prog_id) => ("userChoice", Some(choice)),
            _ => ("applied", None),
        }
    }

    pub fn lookup(extension: &str) -> ExtensionHandler {
        let classes = RegKey::predef(HKEY_CLASSES_ROOT);
        let (prog_id, source) = match user_choice(extension) {
            Some(choice) => (Some(choice), "userChoice"),
            None => match default_value(&classes, extension) {
                Some(value) => (Some(value), "classes"),
                None => (None, "none"),
            },
        };
        let (friendly_name, command) = match &prog_id {
            Some(id) => (
                default_value(&classes, id),
                default_value(&classes, &format!(r"{id}\shell\open\command")),
            ),
            None => (None, None),
        };
        ExtensionHandler {
            extension: extension.to_string(),
            prog_id,
            source: source.into(),
            friendly_name,
            command,
        }
    }

    pub fn notify() {
        use windows::Win32::UI::Shell::{SHCNE_ASSOCCHANGED, SHCNF_IDLIST, SHChangeNotify};
        unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None) };
    }
}

#[cfg(not(windows))]
mod registry {
    use super::{ExtensionHandler, FileAssociation};

    pub fn apply(_: &FileAssociation) -> Result<Option<String>, String> {
        Err("File associations are only available on Windows.".into())
    }

    pub fn remove(_: &FileAssociation) -> Result<(), String> {
        Ok(())
    }

    pub fn status(_: &FileAssociation) -> (&'static str, Option<String>) {
        ("notApplied", None)
    }

    pub fn lookup(extension: &str) -> ExtensionHandler {
        ExtensionHandler {
            extension: extension.to_string(),
            prog_id: None,
            source: "none".into(),
            friendly_name: None,
            command: None,
        }
    }

    pub fn notify() {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_line_adds_file_placeholder() {
        assert_eq!(
            command_line("C:\\a b\\x.exe", ""),
            "\"C:\\a b\\x.exe\" \"%1\""
        );
        assert_eq!(
            command_line("x.exe", "--reuse \"%1\""),
            "\"x.exe\" --reuse \"%1\""
        );
    }

    #[test]
    fn extension_is_normalized() {
        assert_eq!(normalize_extension("*.LOG").unwrap(), ".log");
        assert_eq!(normalize_extension("tar.gz").unwrap(), ".tar.gz");
        assert!(normalize_extension("a b").is_err());
        assert!(normalize_extension("").is_err());
    }
}
