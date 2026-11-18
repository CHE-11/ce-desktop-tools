//! Starts and stops the MegaClip AutoHotkey script. A running copy is an
//! AutoHotkey process whose command line names this script.

use std::path::Path;
use std::time::{Duration, Instant};

const SCRIPT: &str = r"C:\Users\Connor\Documents\MEGA\Scripts_and_Portable Software\MegaClip.ahk";
const APPEAR_WAIT: Duration = Duration::from_secs(2);

pub fn running() -> bool {
    !script_pids().is_empty()
}

pub fn start() -> Result<(), String> {
    if running() {
        return Ok(());
    }
    let path = Path::new(SCRIPT);
    if !path.is_file() {
        return Err(format!("Couldn't find {SCRIPT}."));
    }
    open_document(path)?;
    let started = Instant::now();
    while started.elapsed() < APPEAR_WAIT {
        if running() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Ok(())
}

pub fn stop() -> Result<(), String> {
    let pids = script_pids();
    if pids.is_empty() {
        return Ok(());
    }
    terminate(&pids)
}

fn script_pids() -> Vec<u32> {
    #[cfg(windows)]
    {
        win::script_pids(SCRIPT)
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

fn open_document(path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        win::open_document(path)
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Err("MegaClip only starts on Windows.".into())
    }
}

fn terminate(pids: &[u32]) -> Result<(), String> {
    #[cfg(windows)]
    {
        win::terminate(pids)
    }
    #[cfg(not(windows))]
    {
        let _ = pids;
        Err("MegaClip only stops on Windows.".into())
    }
}

/// True when `command` is an AutoHotkey launch of `script`.
fn command_runs_script(command: &str, script: &str) -> bool {
    let command = command.to_ascii_lowercase().replace('/', "\\");
    let script = script.to_ascii_lowercase();
    command.contains(&script)
}

fn is_autohotkey_host(name: &str) -> bool {
    let file = name.rsplit(['\\', '/']).next().unwrap_or(name);
    let stem = file
        .strip_suffix(".exe")
        .or_else(|| file.strip_suffix(".EXE"))
        .unwrap_or(file);
    stem.to_ascii_lowercase().starts_with("autohotkey")
}

#[cfg(windows)]
mod win {
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::Diagnostics::Debug::ReadProcessMemory;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    };
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
        PROCESS_VM_READ, TerminateProcess, WaitForSingleObject,
    };
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    use windows::core::{PCWSTR, w};

    use super::{command_runs_script, is_autohotkey_host};

    #[repr(C)]
    struct ProcessBasicInformation {
        exit_status: isize,
        peb: *mut u8,
        affinity: usize,
        base_priority: isize,
        pid: usize,
        parent_pid: usize,
    }

    #[repr(C)]
    struct UnicodeString {
        length: u16,
        maximum_length: u16,
        _pad: u32,
        buffer: *mut u16,
    }

    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn NtQueryInformationProcess(
            process: HANDLE,
            class: u32,
            info: *mut std::ffi::c_void,
            length: u32,
            returned: *mut u32,
        ) -> i32;
    }

    pub(super) fn script_pids(script: &str) -> Vec<u32> {
        let Ok(snapshot) = (unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }) else {
            return Vec::new();
        };
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..PROCESSENTRY32W::default()
        };
        let mut pids = Vec::new();
        unsafe {
            if Process32FirstW(snapshot, &mut entry).is_ok() {
                loop {
                    let name = exe_name(&entry.szExeFile);
                    if entry.th32ProcessID != 0
                        && is_autohotkey_host(&name)
                        && command_line(entry.th32ProcessID)
                            .is_some_and(|line| command_runs_script(&line, script))
                    {
                        pids.push(entry.th32ProcessID);
                    }
                    entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
                    if Process32NextW(snapshot, &mut entry).is_err() {
                        break;
                    }
                }
            }
            let _ = CloseHandle(snapshot);
        }
        pids
    }

    fn exe_name(wide: &[u16]) -> String {
        let end = wide.iter().position(|unit| *unit == 0).unwrap_or(wide.len());
        String::from_utf16_lossy(&wide[..end])
    }

    /// The command line of `pid`, read from its process block. `None` when Windows will not share it.
    fn command_line(pid: u32) -> Option<String> {
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid).ok()?;
            let line = read_command_line(handle);
            let _ = CloseHandle(handle);
            line
        }
    }

    unsafe fn read_command_line(handle: HANDLE) -> Option<String> {
        unsafe {
            let mut info = std::mem::zeroed::<ProcessBasicInformation>();
            let status = NtQueryInformationProcess(
                handle,
                0,
                &mut info as *mut ProcessBasicInformation as *mut std::ffi::c_void,
                std::mem::size_of::<ProcessBasicInformation>() as u32,
                std::ptr::null_mut(),
            );
            if status < 0 || info.peb.is_null() {
                return None;
            }
            // ProcessParameters sits 0x20 bytes into the 64-bit process environment block.
            // CommandLine sits 0x70 bytes into those parameters. Both layouts are fixed on 64-bit Windows.
            let mut params: *mut u8 = std::ptr::null_mut();
            ReadProcessMemory(
                handle,
                info.peb.add(0x20) as *const _,
                &mut params as *mut *mut u8 as *mut _,
                std::mem::size_of::<*mut u8>(),
                None,
            )
            .ok()?;
            if params.is_null() {
                return None;
            }
            let mut text = std::mem::zeroed::<UnicodeString>();
            ReadProcessMemory(
                handle,
                params.add(0x70) as *const _,
                &mut text as *mut UnicodeString as *mut _,
                std::mem::size_of::<UnicodeString>(),
                None,
            )
            .ok()?;
            let bytes = text.length as usize;
            if bytes < 2 || !text.buffer.is_aligned() || bytes > 32_768 {
                return None;
            }
            let mut wide = vec![0u16; bytes / 2];
            ReadProcessMemory(
                handle,
                text.buffer as *const _,
                wide.as_mut_ptr() as *mut _,
                bytes,
                None,
            )
            .ok()?;
            Some(String::from_utf16_lossy(&wide))
        }
    }

    pub(super) fn open_document(path: &Path) -> Result<(), String> {
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let code = unsafe {
            ShellExecuteW(
                None,
                w!("open"),
                PCWSTR(wide.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            )
        };
        let code = code.0 as isize;
        if code > 32 {
            return Ok(());
        }
        Err(match code {
            2 => format!("Couldn't find {}.", path.display()),
            5 => "Windows refused to start MegaClip.".into(),
            31 => "AutoHotkey is not set up to open .ahk files.".into(),
            other => format!("Couldn't start MegaClip (Windows error {other})."),
        })
    }

    pub(super) fn terminate(pids: &[u32]) -> Result<(), String> {
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
            return Err("Couldn't stop MegaClip.".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{command_runs_script, is_autohotkey_host};

    const SCRIPT: &str = super::SCRIPT;

    #[test]
    fn only_an_autohotkey_host_counts() {
        assert!(is_autohotkey_host("AutoHotkey64.exe"));
        assert!(is_autohotkey_host("AutoHotkey.exe"));
        assert!(is_autohotkey_host(r"C:\Program Files\AutoHotkey\AutoHotkeyU64.exe"));
        assert!(!is_autohotkey_host("notepad.exe"));
        assert!(!is_autohotkey_host("NotAutoHotkey.exe"));
    }

    #[test]
    fn the_command_line_must_name_this_script() {
        let launched = format!(r#""C:\Program Files\AutoHotkey\AutoHotkey64.exe" "{SCRIPT}""#);
        assert!(command_runs_script(&launched, SCRIPT));
        assert!(command_runs_script(&launched.replace('\\', "/"), SCRIPT));
        assert!(!command_runs_script(
            r#""C:\Program Files\AutoHotkey\AutoHotkey64.exe" "C:\Other\MegaClip.ahk""#,
            SCRIPT
        ));
    }
}
