//! Start menu apps, and a shortcut for one of them on the desktop.
//!
//! The list is the same `Get-StartApps` cache the launcher uses. Each row also
//! carries the publisher, version, install path, and icon. A shortcut points
//! at that app's shell item so Windows keeps its icon.

use std::path::{Path, PathBuf};

use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopApp {
    pub name: String,
    pub app_id: String,
    pub on_desktop: bool,
    pub publisher: Option<String>,
    pub version: Option<String>,
    pub kind: String,
    pub location: Option<String>,
    pub icon: Option<String>,
}

pub fn list() -> Result<Vec<DesktopApp>, String> {
    let desktop = desktop_dir()?;
    let apps = apps_or_refresh()?;
    let facts = crate::desktop_app_details::facts_for(&apps);
    Ok(apps
        .into_iter()
        .zip(facts)
        .map(|(app, facts)| {
            let on_desktop = shortcut_path(&desktop, &app.name).exists();
            DesktopApp {
                name: app.name,
                app_id: app.app_id,
                on_desktop,
                publisher: facts.publisher,
                version: facts.version,
                kind: facts.kind,
                location: facts.location,
                icon: facts.icon,
            }
        })
        .collect())
}

pub fn add(name: &str, app_id: &str) -> Result<String, String> {
    let name = name.trim();
    let app_id = app_id.trim();
    if name.is_empty() {
        return Err("That app has no name.".into());
    }
    if !valid_app_id(app_id) {
        return Err("That app has no launch id.".into());
    }
    // Use the name and AppID the UI already resolved. Do not require a second
    // Start menu hit — path-style AppIDs and stale cache rows still work.
    let path = shortcut_path(&desktop_dir()?, name);
    if !path.exists() {
        write_shortcut(&path, name, app_id)?;
    }
    Ok(path.display().to_string())
}

fn apps_or_refresh() -> Result<Vec<crate::launcher::apps::AppEntry>, String> {
    let cached = crate::launcher::apps::snapshot();
    if !cached.is_empty() {
        return Ok(cached);
    }
    crate::launcher::apps::refresh();
    let apps = crate::launcher::apps::snapshot();
    if apps.is_empty() {
        return Err("Couldn't list Start menu apps.".into());
    }
    Ok(apps)
}

fn shortcut_path(desktop: &Path, name: &str) -> PathBuf {
    desktop.join(format!("{}.lnk", shortcut_file_name(name)))
}

fn shortcut_file_name(name: &str) -> String {
    let mut cleaned = String::new();
    for ch in name.chars() {
        if ch.is_control() || matches!(ch, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
            continue;
        }
        cleaned.push(ch);
    }
    let trimmed = cleaned.trim().trim_end_matches(['.', ' ']);
    let mut name: String = trimmed.chars().take(200).collect();
    name = name.trim_end_matches(['.', ' ']).to_string();
    if name.is_empty() {
        return "App".into();
    }
    if reserved(&name) {
        return format!("{name} app");
    }
    name
}

fn reserved(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((upper.starts_with("COM") || upper.starts_with("LPT"))
            && upper.len() == 4
            && upper.as_bytes()[3].is_ascii_digit()
            && upper.as_bytes()[3] != b'0')
}

fn valid_app_id(app_id: &str) -> bool {
    // Desktop AppIDs often look like `{GUID}\Vendor\App\app.exe`.
    !app_id.is_empty() && app_id.chars().all(|ch| !ch.is_control() && ch != '"')
}

#[cfg(windows)]
fn desktop_dir() -> Result<PathBuf, String> {
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::{FOLDERID_Desktop, KF_FLAG_DEFAULT, SHGetKnownFolderPath};

    let wide = unsafe { SHGetKnownFolderPath(&FOLDERID_Desktop, KF_FLAG_DEFAULT, None) }
        .map_err(|error| format!("Couldn't find the desktop: {error}"))?;
    let text = unsafe { wide.to_string() };
    unsafe { CoTaskMemFree(Some(wide.as_ptr().cast())) };
    let text = text.map_err(|_| "Couldn't read the desktop path.".to_string())?;
    Ok(PathBuf::from(text))
}

#[cfg(not(windows))]
fn desktop_dir() -> Result<PathBuf, String> {
    Err("Add to Desktop runs on Windows.".into())
}

#[cfg(windows)]
fn write_shortcut(path: &Path, name: &str, app_id: &str) -> Result<(), String> {
    use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
    use windows::Win32::UI::Shell::{ILFree, SHParseDisplayName};
    use windows::core::HSTRING;

    let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    let fresh = hr.is_ok() && hr.0 == 0;
    if hr.is_err() {
        let changed = hr.0 == 0x8001_0106_u32 as i32 || hr.0 == 0x8004_01F1_u32 as i32;
        if !changed {
            return Err(format!("Couldn't start COM: {hr}"));
        }
    }

    let result = (|| {
        let parsing = format!("shell:AppsFolder\\{app_id}");
        let mut pidl = std::ptr::null_mut();
        let parsed =
            unsafe { SHParseDisplayName(&HSTRING::from(&parsing), None, &mut pidl, 0, None) };
        if parsed.is_ok() && !pidl.is_null() {
            let saved = save_link(path, name, |link| {
                unsafe { link.SetIDList(pidl) }
                    .map_err(|error| format!("Couldn't point the shortcut at that app: {error}"))
            });
            unsafe { ILFree(Some(pidl)) };
            if saved.is_ok() {
                return Ok(());
            }
        }
        let explorer = explorer_path();
        let arguments = format!("shell:AppsFolder\\{app_id}");
        save_link(path, name, |link| {
            unsafe { link.SetPath(&HSTRING::from(&explorer)) }
                .map_err(|error| format!("Couldn't point the shortcut at that app: {error}"))?;
            unsafe { link.SetArguments(&HSTRING::from(&arguments)) }
                .map_err(|error| format!("Couldn't point the shortcut at that app: {error}"))
        })
    })();

    if fresh {
        unsafe { CoUninitialize() };
    }
    result
}

#[cfg(windows)]
fn save_link(
    path: &Path,
    name: &str,
    fill: impl FnOnce(&windows::Win32::UI::Shell::IShellLinkW) -> Result<(), String>,
) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, IPersistFile};
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
    use windows::core::{HSTRING, Interface};

    let link: IShellLinkW = unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) }
        .map_err(|error| format!("Couldn't create a shortcut: {error}"))?;
    fill(&link)?;
    let _ = unsafe { link.SetDescription(&HSTRING::from(name)) };
    let file: IPersistFile = link
        .cast()
        .map_err(|error| format!("Couldn't save the shortcut: {error}"))?;
    let wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    unsafe { file.Save(&HSTRING::from_wide(&wide), true) }
        .map_err(|error| format!("Couldn't save the shortcut: {error}"))
}

#[cfg(windows)]
fn explorer_path() -> String {
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into());
    format!("{root}\\explorer.exe")
}

#[cfg(not(windows))]
fn write_shortcut(_path: &Path, _name: &str, _app_id: &str) -> Result<(), String> {
    Err("Add to Desktop runs on Windows.".into())
}

#[cfg(test)]
mod tests {
    use super::{shortcut_file_name, valid_app_id};

    #[test]
    fn cleans_a_file_name() {
        assert_eq!(shortcut_file_name("  Chrome: Beta? "), "Chrome Beta");
        assert_eq!(shortcut_file_name("..."), "App");
        assert_eq!(shortcut_file_name("CON"), "CON app");
        assert_eq!(shortcut_file_name("COM1"), "COM1 app");
        assert_eq!(shortcut_file_name("Notes"), "Notes");
    }

    #[test]
    fn accepts_path_style_app_ids() {
        assert!(valid_app_id(
            "{6D809377-6AF0-444B-8957-A3773F02200E}\\Google\\Chrome\\Application\\chrome.exe"
        ));
        assert!(valid_app_id("Microsoft.WindowsNotepad_8wekyb3d8bbwe!App"));
        assert!(valid_app_id("Chrome"));
        assert!(!valid_app_id(""));
        assert!(!valid_app_id("bad\"id"));
        assert!(!valid_app_id("bad\nid"));
    }

    #[cfg(windows)]
    #[test]
    fn writes_a_start_menu_shortcut() {
        use std::os::windows::ffi::OsStrExt;
        use windows::Win32::System::Com::{
            CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
            CoUninitialize, IPersistFile, STGM_READ,
        };
        use windows::Win32::UI::Shell::{ILFree, IShellLinkW, ShellLink};
        use windows::core::{HSTRING, Interface};

        let apps = super::apps_or_refresh().expect("start menu apps");
        let app = &apps[0];
        let dir =
            std::env::temp_dir().join(format!("ce-desktop-tools-shortcut-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let _cleanup = super::Cleanup(&dir);
        let path = dir.join("app.lnk");
        super::write_shortcut(&path, &app.name, &app.app_id).expect("shortcut");
        assert!(path.metadata().unwrap().len() > 0);

        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        let fresh = hr.is_ok() && hr.0 == 0;
        let link: IShellLinkW =
            unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) }.unwrap();
        let file: IPersistFile = link.cast().unwrap();
        let wide: Vec<u16> = path.as_os_str().encode_wide().collect();
        unsafe { file.Load(&HSTRING::from_wide(&wide), STGM_READ) }.unwrap();
        let mut args = vec![0u16; 1024];
        let _ = unsafe { link.GetArguments(&mut args) };
        let args = String::from_utf16_lossy(
            &args[..args
                .iter()
                .position(|unit| *unit == 0)
                .unwrap_or(args.len())],
        );
        let id_list = unsafe { link.GetIDList() };
        let has_id = id_list.as_ref().is_ok_and(|pidl| !pidl.is_null());
        if let Ok(pidl) = id_list {
            if !pidl.is_null() {
                unsafe { ILFree(Some(pidl)) };
            }
        }
        if fresh {
            unsafe { CoUninitialize() };
        }
        assert!(has_id || args.contains("shell:AppsFolder"), "{args}");
    }
}

#[cfg(all(test, windows))]
struct Cleanup<'a>(&'a std::path::Path);

#[cfg(all(test, windows))]
impl Drop for Cleanup<'_> {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(self.0);
    }
}
