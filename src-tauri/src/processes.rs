//! Running processes, the process behind a network port, and the processes
//! that hold a file open.

use serde::Serialize;

/// Closing one of these logs the user out or crashes Windows.
const PROTECTED: &[&str] = &[
    "system",
    "idle",
    "registry",
    "smss.exe",
    "csrss.exe",
    "wininit.exe",
    "winlogon.exe",
    "lsass.exe",
    "services.exe",
    "memory compression",
];

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessInfo {
    pub pid: u32,
    pub parent_pid: u32,
    pub name: String,
    pub path: String,
    pub memory_bytes: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortOwner {
    pub protocol: String,
    pub local_port: u16,
    pub local_address: String,
    pub remote_address: String,
    pub state: String,
    pub pid: u32,
    pub name: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileLocker {
    pub pid: u32,
    pub name: String,
    pub app_name: String,
    pub path: String,
}

/// A port, or a port with `*` standing for one digit each, so `30**` is 3000 to 3099.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PortPattern {
    digits: Vec<Option<u8>>,
}

impl PortPattern {
    pub fn parse(text: &str) -> Result<Self, String> {
        let text = text.trim();
        if text.is_empty() {
            return Err("Enter a port, such as 3000, or a pattern, such as 30**.".into());
        }
        if text.len() > 5 {
            return Err("A port has at most 5 digits.".into());
        }
        let mut digits = Vec::new();
        for ch in text.chars() {
            match ch {
                '0'..='9' => digits.push(Some(ch as u8 - b'0')),
                '*' | '?' | 'x' | 'X' => digits.push(None),
                _ => {
                    return Err(format!(
                        "\"{text}\" is not a port. Use digits, with * for any one digit."
                    ));
                }
            }
        }
        if digits.first() == Some(&Some(0)) && digits.len() > 1 {
            return Err("A port does not start with 0.".into());
        }
        let pattern = Self { digits };
        let (low, high) = pattern.range();
        if low > 65535 || high == 0 {
            return Err(format!("{text} is outside the port range 1 to 65535."));
        }
        Ok(pattern)
    }

    fn bound(&self, wildcard: u8) -> u32 {
        self.digits.iter().enumerate().fold(0u32, |value, (index, digit)| {
            // A leading wildcard cannot be 0, or the port would have fewer digits.
            let fill = if index == 0 && wildcard == 0 { 1 } else { wildcard };
            value * 10 + u32::from(digit.unwrap_or(fill))
        })
    }

    /// Lowest and highest port the pattern can match, clamped to real ports.
    pub fn range(&self) -> (u32, u32) {
        let low = self.bound(0).max(1);
        let high = self.bound(9).min(65535);
        (low, high)
    }

    pub fn is_exact(&self) -> bool {
        self.digits.iter().all(Option::is_some)
    }

    pub fn matches(&self, port: u16) -> bool {
        let text = port.to_string();
        text.len() == self.digits.len()
            && text
                .bytes()
                .zip(&self.digits)
                .all(|(byte, digit)| digit.is_none_or(|digit| byte - b'0' == digit))
    }

    /// `3000` or `3000 to 3099`, for messages.
    pub fn describe(&self) -> String {
        let (low, high) = self.range();
        if self.is_exact() {
            format!("port {low}")
        } else {
            format!("ports {low} to {high}")
        }
    }
}

pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn is_protected(pid: u32, name: &str) -> bool {
    pid <= 4 || pid == std::process::id() || PROTECTED.contains(&name.to_lowercase().as_str())
}

/// Children listed before their parents, so a tree closes from the leaves up.
fn tree_order(root: u32, processes: &[ProcessInfo]) -> Vec<u32> {
    let mut order = Vec::new();
    let mut seen = std::collections::HashSet::new();
    fn visit(
        pid: u32,
        processes: &[ProcessInfo],
        seen: &mut std::collections::HashSet<u32>,
        order: &mut Vec<u32>,
    ) {
        if !seen.insert(pid) {
            return;
        }
        for child in processes
            .iter()
            .filter(|process| process.parent_pid == pid && process.pid != pid)
        {
            visit(child.pid, processes, seen, order);
        }
        order.push(pid);
    }
    visit(root, processes, &mut seen, &mut order);
    order
}

/// Closes a process, and with `tree` its children too. Returns a short
/// description of what closed.
pub fn kill(pid: u32, tree: bool) -> Result<String, String> {
    let processes = list()?;
    let Some(target) = processes.iter().find(|process| process.pid == pid) else {
        return Err(format!("Process {pid} is no longer running."));
    };
    if is_protected(pid, &target.name) {
        return Err(format!(
            "{} is part of Windows or this app, so it was left running.",
            target.name
        ));
    }
    let name = target.name.clone();
    let pids = if tree {
        tree_order(pid, &processes)
    } else {
        vec![pid]
    };
    let mut closed = 0usize;
    let mut last_error = None;
    for each in &pids {
        let each_name = processes
            .iter()
            .find(|process| process.pid == *each)
            .map(|process| process.name.as_str())
            .unwrap_or_default();
        if is_protected(*each, each_name) {
            continue;
        }
        match terminate(*each) {
            Ok(()) => closed += 1,
            Err(error) => last_error = Some(error),
        }
    }
    if closed == 0 {
        return Err(last_error.unwrap_or_else(|| format!("Couldn't close {name}.")));
    }
    Ok(if closed == 1 {
        format!("{name} (PID {pid})")
    } else {
        format!("{name} and {} child processes", closed - 1)
    })
}

#[cfg(windows)]
fn wide_text(wide: &[u16]) -> String {
    let end = wide
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(wide.len());
    String::from_utf16_lossy(&wide[..end])
}

#[cfg(windows)]
pub fn list() -> Result<Vec<ProcessInfo>, String> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    };

    let mut processes = Vec::new();
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
            .map_err(|error| format!("Couldn't list processes: {error}"))?;
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..PROCESSENTRY32W::default()
        };
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let pid = entry.th32ProcessID;
                let (path, memory_bytes) = details(pid);
                processes.push(ProcessInfo {
                    pid,
                    parent_pid: entry.th32ParentProcessID,
                    name: wide_text(&entry.szExeFile),
                    path,
                    memory_bytes,
                });
                entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);
    }
    Ok(processes)
}

#[cfg(windows)]
fn details(pid: u32) -> (String, u64) {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
        QueryFullProcessImageNameW,
    };
    use windows::core::PWSTR;

    if pid == 0 {
        return (String::new(), 0);
    }
    unsafe {
        let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return (String::new(), 0);
        };
        let mut buffer = [0u16; 1024];
        let mut size = buffer.len() as u32;
        let path = if QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut size,
        )
        .is_ok()
        {
            String::from_utf16_lossy(&buffer[..size as usize])
        } else {
            String::new()
        };
        let mut counters = PROCESS_MEMORY_COUNTERS {
            cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
            ..Default::default()
        };
        let memory = if K32GetProcessMemoryInfo(handle, &mut counters, counters.cb).as_bool() {
            counters.WorkingSetSize as u64
        } else {
            0
        };
        let _ = CloseHandle(handle);
        (path, memory)
    }
}

#[cfg(windows)]
fn terminate(pid: u32) -> Result<(), String> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE, TerminateProcess, WaitForSingleObject,
    };
    unsafe {
        let handle = OpenProcess(PROCESS_TERMINATE | PROCESS_SYNCHRONIZE, false, pid).map_err(|_| {
            format!("Windows refused to close process {pid}. It may need administrator rights.")
        })?;
        let result = TerminateProcess(handle, 1);
        if result.is_ok() {
            let _ = WaitForSingleObject(handle, 2000);
        }
        let _ = CloseHandle(handle);
        result.map_err(|error| format!("Couldn't close process {pid}: {error}"))
    }
}

#[cfg(windows)]
fn tcp_state(state: u32) -> &'static str {
    match state {
        1 => "CLOSED",
        2 => "LISTENING",
        3 => "SYN_SENT",
        4 => "SYN_RECEIVED",
        5 => "ESTABLISHED",
        6 => "FIN_WAIT_1",
        7 => "FIN_WAIT_2",
        8 => "CLOSE_WAIT",
        9 => "CLOSING",
        10 => "LAST_ACK",
        11 => "TIME_WAIT",
        12 => "DELETE_TCB",
        _ => "UNKNOWN",
    }
}

#[cfg(windows)]
fn port_of(raw: u32) -> u16 {
    u16::from_be((raw & 0xFFFF) as u16)
}

/// Calls an IP Helper table function twice: once for the size, once for the
/// rows. The buffer is made of `u64`s so the table header is aligned.
#[cfg(windows)]
fn read_table(fill: impl Fn(Option<*mut std::ffi::c_void>, &mut u32) -> u32) -> Result<Vec<u64>, String> {
    const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
    let mut size = 0u32;
    let mut buffer: Vec<u64> = Vec::new();
    for _ in 0..4 {
        let pointer = if buffer.is_empty() {
            None
        } else {
            Some(buffer.as_mut_ptr().cast())
        };
        match fill(pointer, &mut size) {
            0 if !buffer.is_empty() => return Ok(buffer),
            0 | ERROR_INSUFFICIENT_BUFFER => {
                buffer = vec![0u64; (size as usize).div_ceil(8) + 1];
            }
            code => return Err(format!("Couldn't read the network table (error {code}).")),
        }
    }
    Err("The network table kept changing size.".into())
}

#[cfg(windows)]
pub fn port_owners(pattern: &PortPattern) -> Result<Vec<PortOwner>, String> {
    use std::net::{Ipv4Addr, Ipv6Addr};
    use windows::Win32::NetworkManagement::IpHelper::{
        GetExtendedTcpTable, GetExtendedUdpTable, MIB_TCP6TABLE_OWNER_PID,
        MIB_TCPTABLE_OWNER_PID, MIB_UDP6TABLE_OWNER_PID, MIB_UDPTABLE_OWNER_PID,
        TCP_TABLE_OWNER_PID_ALL, UDP_TABLE_OWNER_PID,
    };
    use windows::Win32::Networking::WinSock::{AF_INET, AF_INET6};

    let names: std::collections::HashMap<u32, String> = list()
        .unwrap_or_default()
        .into_iter()
        .map(|process| (process.pid, process.name))
        .collect();
    let name_of = |pid: u32| {
        names
            .get(&pid)
            .cloned()
            .unwrap_or_else(|| format!("PID {pid}"))
    };
    let mut owners = Vec::new();

    unsafe {
        let buffer = read_table(|table, size| {
            GetExtendedTcpTable(table, size, false, AF_INET.0 as u32, TCP_TABLE_OWNER_PID_ALL, 0)
        })?;
        let table = &*(buffer.as_ptr() as *const MIB_TCPTABLE_OWNER_PID);
        let rows = std::slice::from_raw_parts(table.table.as_ptr(), table.dwNumEntries as usize);
        for row in rows {
            let local = port_of(row.dwLocalPort);
            let remote = port_of(row.dwRemotePort);
            if !pattern.matches(local) && !pattern.matches(remote) {
                continue;
            }
            owners.push(PortOwner {
                protocol: "TCP".into(),
                local_port: local,
                local_address: format!("{}:{local}", Ipv4Addr::from(u32::from_be(row.dwLocalAddr))),
                remote_address: format!("{}:{remote}", Ipv4Addr::from(u32::from_be(row.dwRemoteAddr))),
                state: tcp_state(row.dwState).into(),
                pid: row.dwOwningPid,
                name: name_of(row.dwOwningPid),
            });
        }

        let buffer = read_table(|table, size| {
            GetExtendedTcpTable(table, size, false, AF_INET6.0 as u32, TCP_TABLE_OWNER_PID_ALL, 0)
        })?;
        let table = &*(buffer.as_ptr() as *const MIB_TCP6TABLE_OWNER_PID);
        let rows = std::slice::from_raw_parts(table.table.as_ptr(), table.dwNumEntries as usize);
        for row in rows {
            let local = port_of(row.dwLocalPort);
            let remote = port_of(row.dwRemotePort);
            if !pattern.matches(local) && !pattern.matches(remote) {
                continue;
            }
            owners.push(PortOwner {
                protocol: "TCPv6".into(),
                local_port: local,
                local_address: format!("[{}]:{local}", Ipv6Addr::from(row.ucLocalAddr)),
                remote_address: format!("[{}]:{remote}", Ipv6Addr::from(row.ucRemoteAddr)),
                state: tcp_state(row.dwState).into(),
                pid: row.dwOwningPid,
                name: name_of(row.dwOwningPid),
            });
        }

        let buffer = read_table(|table, size| {
            GetExtendedUdpTable(table, size, false, AF_INET.0 as u32, UDP_TABLE_OWNER_PID, 0)
        })?;
        let table = &*(buffer.as_ptr() as *const MIB_UDPTABLE_OWNER_PID);
        let rows = std::slice::from_raw_parts(table.table.as_ptr(), table.dwNumEntries as usize);
        for row in rows {
            let local = port_of(row.dwLocalPort);
            if !pattern.matches(local) {
                continue;
            }
            owners.push(PortOwner {
                protocol: "UDP".into(),
                local_port: local,
                local_address: format!("{}:{local}", Ipv4Addr::from(u32::from_be(row.dwLocalAddr))),
                remote_address: String::new(),
                state: String::new(),
                pid: row.dwOwningPid,
                name: name_of(row.dwOwningPid),
            });
        }

        let buffer = read_table(|table, size| {
            GetExtendedUdpTable(table, size, false, AF_INET6.0 as u32, UDP_TABLE_OWNER_PID, 0)
        })?;
        let table = &*(buffer.as_ptr() as *const MIB_UDP6TABLE_OWNER_PID);
        let rows = std::slice::from_raw_parts(table.table.as_ptr(), table.dwNumEntries as usize);
        for row in rows {
            let local = port_of(row.dwLocalPort);
            if !pattern.matches(local) {
                continue;
            }
            owners.push(PortOwner {
                protocol: "UDPv6".into(),
                local_port: local,
                local_address: format!("[{}]:{local}", Ipv6Addr::from(row.ucLocalAddr)),
                remote_address: String::new(),
                state: String::new(),
                pid: row.dwOwningPid,
                name: name_of(row.dwOwningPid),
            });
        }
    }
    owners.retain(|owner| owner.pid != 0);
    owners.sort_by(|left, right| {
        (left.state != "LISTENING", left.local_port, left.pid).cmp(&(
            right.state != "LISTENING",
            right.local_port,
            right.pid,
        ))
    });
    Ok(owners)
}

#[cfg(windows)]
pub fn file_lockers(path: &str) -> Result<Vec<FileLocker>, String> {
    use windows::Win32::Foundation::{ERROR_MORE_DATA, ERROR_SUCCESS};
    use windows::Win32::System::RestartManager::{
        CCH_RM_SESSION_KEY, RM_PROCESS_INFO, RmEndSession, RmGetList, RmRegisterResources,
        RmStartSession,
    };
    use windows::core::{PCWSTR, PWSTR};

    let path = path.trim();
    if path.is_empty() {
        return Err("Choose a file first.".into());
    }
    if !std::path::Path::new(path).exists() {
        return Err(format!("{path} does not exist."));
    }
    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    let paths: std::collections::HashMap<u32, (String, String)> = list()
        .unwrap_or_default()
        .into_iter()
        .map(|process| (process.pid, (process.name, process.path)))
        .collect();

    unsafe {
        let mut session = 0u32;
        let mut key = [0u16; CCH_RM_SESSION_KEY as usize + 1];
        let started = RmStartSession(&mut session, None, PWSTR(key.as_mut_ptr()));
        if started != ERROR_SUCCESS {
            return Err(format!("Couldn't ask Windows who is using the file (error {}).", started.0));
        }
        let outcome = (|| {
            let files = [PCWSTR(wide.as_ptr())];
            let registered = RmRegisterResources(session, Some(&files), None, None);
            if registered != ERROR_SUCCESS {
                return Err(format!("Couldn't check that file (error {}).", registered.0));
            }
            let mut needed = 0u32;
            let mut reasons = 0u32;
            let mut infos: Vec<RM_PROCESS_INFO> = Vec::new();
            for _ in 0..4 {
                let mut count = infos.len() as u32;
                let pointer = if infos.is_empty() {
                    None
                } else {
                    Some(infos.as_mut_ptr())
                };
                let result = RmGetList(session, &mut needed, &mut count, pointer, &mut reasons);
                if result == ERROR_SUCCESS {
                    infos.truncate(count as usize);
                    return Ok(infos);
                }
                if result != ERROR_MORE_DATA {
                    return Err(format!("Couldn't list what holds that file (error {}).", result.0));
                }
                infos = vec![RM_PROCESS_INFO::default(); needed as usize + 2];
            }
            Err("The list of programs using that file kept changing.".into())
        })();
        let _ = RmEndSession(session);
        let infos = outcome?;
        Ok(infos
            .iter()
            .map(|info| {
                let pid = info.Process.dwProcessId;
                let (name, path) = paths.get(&pid).cloned().unwrap_or_default();
                FileLocker {
                    pid,
                    name: if name.is_empty() { format!("PID {pid}") } else { name },
                    app_name: wide_text(&info.strAppName),
                    path,
                }
            })
            .collect())
    }
}

#[cfg(not(windows))]
pub fn list() -> Result<Vec<ProcessInfo>, String> {
    Err("Process tools only work on Windows.".into())
}

#[cfg(not(windows))]
fn terminate(_pid: u32) -> Result<(), String> {
    Err("Process tools only work on Windows.".into())
}

#[cfg(not(windows))]
pub fn port_owners(_pattern: &PortPattern) -> Result<Vec<PortOwner>, String> {
    Err("Process tools only work on Windows.".into())
}

#[cfg(not(windows))]
pub fn file_lockers(_path: &str) -> Result<Vec<FileLocker>, String> {
    Err("Process tools only work on Windows.".into())
}

#[cfg(test)]
mod tests {
    use super::{PortPattern, ProcessInfo, format_bytes, is_protected, tree_order};

    #[test]
    fn a_star_stands_for_one_digit() {
        let pattern = PortPattern::parse("30**").unwrap();
        assert_eq!(pattern.range(), (3000, 3099));
        assert!(pattern.matches(3000));
        assert!(pattern.matches(3042));
        assert!(pattern.matches(3099));
        assert!(!pattern.matches(3100));
        assert!(!pattern.matches(300));
        assert!(!pattern.matches(30000));
        assert_eq!(pattern.describe(), "ports 3000 to 3099");

        let middle = PortPattern::parse("8*80").unwrap();
        assert!(middle.matches(8080));
        assert!(!middle.matches(8081));

        let leading = PortPattern::parse("*443").unwrap();
        assert_eq!(leading.range(), (1443, 9443));
        assert!(leading.matches(8443));

        let wide = PortPattern::parse("6****").unwrap();
        assert_eq!(wide.range(), (60000, 65535));
    }

    #[test]
    fn an_exact_port_still_works_and_bad_text_is_refused() {
        let exact = PortPattern::parse(" 3000 ").unwrap();
        assert!(exact.is_exact());
        assert!(exact.matches(3000));
        assert!(!exact.matches(3001));
        assert_eq!(exact.describe(), "port 3000");

        for bad in ["", "0", "0300", "70000", "7****", "123456", "30-40", "abc"] {
            assert!(PortPattern::parse(bad).is_err(), "{bad} should be refused");
        }
    }

    fn process(pid: u32, parent_pid: u32) -> ProcessInfo {
        ProcessInfo {
            pid,
            parent_pid,
            name: format!("p{pid}.exe"),
            path: String::new(),
            memory_bytes: 0,
        }
    }

    #[test]
    fn a_tree_closes_children_before_parents() {
        let processes = vec![
            process(10, 1),
            process(11, 10),
            process(12, 11),
            process(13, 10),
            process(20, 1),
        ];
        let order = tree_order(10, &processes);
        assert_eq!(order.last(), Some(&10));
        let position = |pid| order.iter().position(|each| *each == pid).unwrap();
        assert!(position(12) < position(11));
        assert!(!order.contains(&20));
        assert_eq!(order.len(), 4);
    }

    #[test]
    fn windows_and_this_app_are_protected() {
        assert!(is_protected(4, "System"));
        assert!(is_protected(500, "CSRSS.EXE"));
        assert!(is_protected(std::process::id(), "ce-desktop-tools.exe"));
        assert!(!is_protected(1234, "notepad.exe"));
    }

    #[test]
    fn sizes_read_naturally() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(200 * 1024 * 1024), "200.0 MB");
    }
}
