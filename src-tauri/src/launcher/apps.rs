//! Start menu apps for the launcher.
//!
//! `Get-StartApps` lists desktop shortcuts and Store apps with one AppID each.
//! The list is cached and refreshed in the background, because PowerShell
//! takes about a second to answer.

use std::sync::RwLock;
use std::time::Duration;

const REFRESH: Duration = Duration::from_secs(10 * 60);
const SCRIPT: &str = "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; \
Get-StartApps | Select-Object Name, AppID | ConvertTo-Json -Compress";

#[derive(Clone, Debug, PartialEq)]
pub struct AppEntry {
    pub name: String,
    pub app_id: String,
}

static APPS: RwLock<Vec<AppEntry>> = RwLock::new(Vec::new());

pub fn start_indexing() {
    let _ = std::thread::Builder::new()
        .name("launcher-apps".into())
        .spawn(|| {
            loop {
                refresh();
                std::thread::sleep(REFRESH);
            }
        });
}

pub fn refresh() {
    match read_start_apps() {
        Ok(apps) if !apps.is_empty() => {
            if let Ok(mut slot) = APPS.write() {
                *slot = apps;
            }
        }
        Ok(_) => {}
        Err(error) => eprintln!("ce-desktop-tools could not list Start menu apps: {error}"),
    }
}

pub fn snapshot() -> Vec<AppEntry> {
    APPS.read().map(|apps| apps.clone()).unwrap_or_default()
}

pub fn launch(app_id: &str) -> Result<(), String> {
    if app_id.trim().is_empty() {
        return Err("That app has no launch id.".into());
    }
    std::process::Command::new("explorer.exe")
        .arg(format!("shell:AppsFolder\\{app_id}"))
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Couldn't start that app: {error}"))
}

fn read_start_apps() -> Result<Vec<AppEntry>, String> {
    let mut command = std::process::Command::new("powershell.exe");
    command.args([
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        SCRIPT,
    ]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let output = command.output().map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(parse(&String::from_utf8_lossy(&output.stdout)))
}

fn parse(json: &str) -> Vec<AppEntry> {
    let json = json.trim().trim_start_matches('\u{feff}');
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    let items = match value {
        serde_json::Value::Array(items) => items,
        object @ serde_json::Value::Object(_) => vec![object],
        _ => return Vec::new(),
    };
    let mut seen = std::collections::HashSet::new();
    let mut apps: Vec<AppEntry> = items
        .into_iter()
        .filter_map(|item| {
            let name = item.get("Name")?.as_str()?.trim().to_string();
            let app_id = item.get("AppID")?.as_str()?.trim().to_string();
            (!name.is_empty() && !app_id.is_empty()).then_some(AppEntry { name, app_id })
        })
        .filter(|entry| seen.insert(entry.app_id.to_lowercase()))
        .collect();
    apps.sort_by_key(|entry| entry.name.to_lowercase());
    apps
}

#[cfg(test)]
mod tests {
    use super::{AppEntry, parse};

    #[test]
    fn reads_a_list_or_a_single_app() {
        let list = parse(
            r#"[{"Name":"Notepad","AppID":"Microsoft.WindowsNotepad_8wekyb3d8bbwe!App"},{"Name":"Chrome","AppID":"Chrome"},{"Name":"Chrome copy","AppID":"chrome"}]"#,
        );
        assert_eq!(
            list,
            vec![
                AppEntry {
                    name: "Chrome".into(),
                    app_id: "Chrome".into()
                },
                AppEntry {
                    name: "Notepad".into(),
                    app_id: "Microsoft.WindowsNotepad_8wekyb3d8bbwe!App".into()
                },
            ]
        );
        let single = parse("\u{feff}{\"Name\":\"Paint\",\"AppID\":\"mspaint\"}");
        assert_eq!(single.len(), 1);
        assert!(parse("not json").is_empty());
        assert!(parse(r#"[{"Name":"","AppID":"x"}]"#).is_empty());
    }
}
