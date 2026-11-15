//! Current-user "start when Windows signs in" registration.
//!
//! The value lives in HKCU\Software\Microsoft\Windows\CurrentVersion\Run.
//! A background start keeps the window hidden so the tray app is ready.

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const RUN_VALUE: &str = "ce-desktop-tools";

pub fn started_in_background() -> bool {
    std::env::args().any(|arg| arg == "--background")
}

pub fn sync_run_at_startup(enabled: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        sync_windows(enabled)
    }
    #[cfg(not(windows))]
    {
        let _ = enabled;
        Ok(())
    }
}

#[cfg(windows)]
fn sync_windows(enabled: bool) -> Result<(), String> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu
        .create_subkey(RUN_KEY)
        .map_err(|error| format!("Couldn't update Windows startup: {error}"))?;
    if !enabled {
        return match key.delete_value(RUN_VALUE) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!("Couldn't update Windows startup: {error}")),
        };
    }
    let executable = std::env::current_exe().map_err(|error| format!("Couldn't find this application: {error}"))?;
    let command = format!("\"{}\" --background", executable.display());
    key.set_value(RUN_VALUE, &command)
        .map_err(|error| format!("Couldn't update Windows startup: {error}"))
}
