//! Restarts Windows Explorer by closing explorer.exe.
//! Windows usually opens it again. If it does not, this starts it.

use std::time::Duration;

pub fn restart() -> Result<(), String> {
    #[cfg(windows)]
    {
        let pids = explorer_pids()?;
        if !pids.is_empty() {
            terminate(&pids)?;
        }
        if !wait_for_explorer(Duration::from_secs(2))? {
            start_explorer()?;
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        Err("Windows Explorer can only be restarted on Windows.".into())
    }
}

pub(crate) fn is_explorer_exe(name: &str) -> bool {
    name.eq_ignore_ascii_case("explorer.exe")
}

#[cfg(windows)]
fn explorer_pids() -> Result<Vec<u32>, String> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    };

    unsafe {
        let snapshot =
            CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).map_err(|error| error.to_string())?;
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..PROCESSENTRY32W::default()
        };
        let mut pids = Vec::new();
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                if is_explorer_exe(&exe_name(&entry.szExeFile)) && entry.th32ProcessID != 0 {
                    pids.push(entry.th32ProcessID);
                }
                entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);
        Ok(pids)
    }
}

#[cfg(windows)]
fn exe_name(wide: &[u16]) -> String {
    let end = wide
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(wide.len());
    String::from_utf16_lossy(&wide[..end])
}

#[cfg(windows)]
fn terminate(pids: &[u32]) -> Result<(), String> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE, TerminateProcess, WaitForSingleObject,
    };

    let mut closed = 0usize;
    unsafe {
        for &pid in pids {
            let Ok(handle) = OpenProcess(PROCESS_TERMINATE | PROCESS_SYNCHRONIZE, false, pid)
            else {
                continue;
            };
            if TerminateProcess(handle, 0).is_ok() {
                let _ = WaitForSingleObject(handle, 2000);
                closed += 1;
            }
            let _ = CloseHandle(handle);
        }
    }
    if closed == 0 {
        return Err("Couldn't close Windows Explorer.".into());
    }
    Ok(())
}

#[cfg(windows)]
fn wait_for_explorer(limit: Duration) -> Result<bool, String> {
    let started = std::time::Instant::now();
    while started.elapsed() < limit {
        if !explorer_pids()?.is_empty() {
            return Ok(true);
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    Ok(!explorer_pids()?.is_empty())
}

#[cfg(windows)]
fn start_explorer() -> Result<(), String> {
    let windows = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".to_string());
    let path = std::path::Path::new(&windows).join("explorer.exe");
    std::process::Command::new(path)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Couldn't start Windows Explorer: {error}"))
}

#[cfg(test)]
mod tests {
    use super::is_explorer_exe;

    #[test]
    fn only_the_explorer_process_matches() {
        assert!(is_explorer_exe("explorer.exe"));
        assert!(is_explorer_exe("EXPLORER.EXE"));
        assert!(!is_explorer_exe("explorer.exe.bak"));
        assert!(!is_explorer_exe("ce-desktop-tools.exe"));
    }
}
