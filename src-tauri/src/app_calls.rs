//! Processes that currently have an outbound TCP connection.
//!
//! Windows reports the process, address, and port. It does not report the
//! URL, method, or body of another app's HTTPS request.

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use crate::processes::ProcessInfo;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppCallsSnapshot {
    pub foreground_pid: u32,
    pub apps: Vec<AppCall>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppCall {
    pub placement: String,
    pub name: String,
    pub path: String,
    pub pid: u32,
    pub window_title: String,
    pub memory_bytes: u64,
    /// Share of the whole machine since the previous snapshot. Absent on the first sample.
    pub cpu_percent: Option<f32>,
    pub connections: Vec<AppCallConnection>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppCallConnection {
    pub protocol: String,
    pub local_address: String,
    pub remote_address: String,
    pub remote_port: u16,
    pub state: String,
    pub local: bool,
}

struct OwnedConnection {
    connection: AppCallConnection,
    pid: u32,
}

struct Group {
    pids: Vec<u32>,
    path_key: String,
    connections: Vec<AppCallConnection>,
}

pub fn snapshot() -> Result<AppCallsSnapshot, String> {
    #[cfg(windows)]
    {
        let processes = crate::processes::list()?;
        let windows = visible_windows()?;
        let foreground_pid = foreground_pid();
        let connections = outbound_connections()?;
        let pids: Vec<u32> = processes.iter().map(|process| process.pid).collect();
        let cpu = crate::processes::cpu_percents(&pids);
        return Ok(assemble(
            processes,
            windows,
            foreground_pid,
            connections,
            &cpu,
        ));
    }
    #[cfg(not(windows))]
    {
        Err("Network Explorer only works on Windows.".into())
    }
}

fn assemble(
    processes: Vec<ProcessInfo>,
    windows: HashMap<u32, String>,
    foreground_pid: u32,
    connections: Vec<OwnedConnection>,
    cpu: &HashMap<u32, f32>,
) -> AppCallsSnapshot {
    let by_pid: HashMap<u32, ProcessInfo> = processes
        .into_iter()
        .map(|process| (process.pid, process))
        .collect();
    let foreground_path = by_pid
        .get(&foreground_pid)
        .map(|process| process.path.clone())
        .unwrap_or_default();
    let foreground_key = path_key(&foreground_path);
    let mut foreground: Option<Group> = None;
    let mut background: HashMap<String, Group> = HashMap::new();

    for owned in connections {
        let pid = owned.pid;
        if in_foreground(pid, foreground_pid, &foreground_key, &by_pid) {
            let group = foreground.get_or_insert_with(|| Group {
                pids: Vec::new(),
                path_key: foreground_key.clone(),
                connections: Vec::new(),
            });
            push_member(group, pid, owned.connection);
            continue;
        }
        let key = background_key(pid, &by_pid);
        let path_key = by_pid
            .get(&pid)
            .map(|process| path_key(&process.path))
            .unwrap_or_default();
        let group = background.entry(key).or_insert_with(|| Group {
            pids: Vec::new(),
            path_key,
            connections: Vec::new(),
        });
        push_member(group, pid, owned.connection);
    }

    let mut apps = Vec::new();
    if let Some(group) = foreground {
        let title = foreground_title(foreground_pid, &group, &windows, &by_pid);
        apps.push(finish(
            "foreground",
            foreground_pid,
            &group,
            &title,
            &windows,
            &by_pid,
            cpu,
        ));
    }
    let mut background_apps: Vec<AppCall> = background
        .into_values()
        .map(|group| {
            let pid = background_pid(&group, &windows, &by_pid);
            let title = title_for(&group, &windows, &by_pid);
            finish("background", pid, &group, &title, &windows, &by_pid, cpu)
        })
        .collect();
    background_apps.sort_by(|left, right| {
        left.name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
            .then(left.pid.cmp(&right.pid))
    });
    apps.extend(background_apps);

    AppCallsSnapshot {
        foreground_pid,
        apps,
    }
}

fn push_member(group: &mut Group, pid: u32, connection: AppCallConnection) {
    if !group.pids.contains(&pid) {
        group.pids.push(pid);
    }
    group.connections.push(connection);
}

fn finish(
    placement: &str,
    preferred_pid: u32,
    group: &Group,
    title: &str,
    windows: &HashMap<u32, String>,
    by_pid: &HashMap<u32, ProcessInfo>,
    cpu: &HashMap<u32, f32>,
) -> AppCall {
    let pid = if preferred_pid != 0
        && (by_pid.contains_key(&preferred_pid) || windows.contains_key(&preferred_pid))
    {
        preferred_pid
    } else {
        group.pids.iter().copied().min().unwrap_or(preferred_pid)
    };
    let process = by_pid.get(&pid);
    let mut usage_pids = group.pids.clone();
    if pid != 0 && !usage_pids.contains(&pid) {
        usage_pids.push(pid);
    }
    let memory_bytes = usage_pids
        .iter()
        .filter_map(|each| by_pid.get(each))
        .map(|each| each.memory_bytes)
        .sum();
    let sampled: Vec<f32> = usage_pids
        .iter()
        .filter_map(|each| cpu.get(each).copied())
        .collect();
    let cpu_percent = if sampled.is_empty() {
        None
    } else {
        Some(sampled.iter().sum::<f32>().clamp(0.0, 100.0))
    };
    let mut connections = group.connections.clone();
    connections.sort_by(|left, right| {
        (
            &left.remote_address,
            left.remote_port,
            &left.state,
            &left.local_address,
        )
            .cmp(&(
                &right.remote_address,
                right.remote_port,
                &right.state,
                &right.local_address,
            ))
    });
    AppCall {
        placement: placement.into(),
        name: process
            .map(|item| item.name.clone())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| format!("PID {pid}")),
        path: process.map(|item| item.path.clone()).unwrap_or_default(),
        pid,
        window_title: title.into(),
        memory_bytes,
        cpu_percent,
        connections,
    }
}

fn in_foreground(
    pid: u32,
    foreground_pid: u32,
    foreground_key: &str,
    by_pid: &HashMap<u32, ProcessInfo>,
) -> bool {
    if foreground_pid != 0 && reaches(pid, foreground_pid, by_pid) {
        return true;
    }
    if foreground_key.is_empty() {
        return false;
    }
    by_pid
        .get(&pid)
        .is_some_and(|process| path_key(&process.path) == foreground_key)
}

fn background_key(pid: u32, by_pid: &HashMap<u32, ProcessInfo>) -> String {
    by_pid
        .get(&pid)
        .map(|process| path_key(&process.path))
        .filter(|key| !key.is_empty())
        .unwrap_or_else(|| format!("pid:{pid}"))
}

fn background_pid(
    group: &Group,
    windows: &HashMap<u32, String>,
    by_pid: &HashMap<u32, ProcessInfo>,
) -> u32 {
    let titled = |pid: &u32| windows.get(pid).is_some_and(|title| !title.is_empty());
    if let Some(pid) = group.pids.iter().copied().find(|pid| titled(pid)) {
        return pid;
    }
    if let Some(pid) = group
        .pids
        .iter()
        .copied()
        .find(|pid| windows.contains_key(pid))
    {
        return pid;
    }
    if !group.path_key.is_empty() {
        if let Some(pid) = by_pid.iter().find_map(|(pid, process)| {
            (path_key(&process.path) == group.path_key && titled(pid)).then_some(*pid)
        }) {
            return pid;
        }
    }
    group.pids.iter().copied().min().unwrap_or(0)
}

fn foreground_title(
    foreground_pid: u32,
    group: &Group,
    windows: &HashMap<u32, String>,
    by_pid: &HashMap<u32, ProcessInfo>,
) -> String {
    if let Some(title) = windows.get(&foreground_pid) {
        if !title.is_empty() {
            return title.clone();
        }
    }
    title_for(group, windows, by_pid)
}

fn title_for(
    group: &Group,
    windows: &HashMap<u32, String>,
    by_pid: &HashMap<u32, ProcessInfo>,
) -> String {
    for pid in &group.pids {
        if let Some(title) = windows.get(pid) {
            if !title.is_empty() {
                return title.clone();
            }
        }
    }
    if group.path_key.is_empty() {
        return String::new();
    }
    by_pid
        .iter()
        .find_map(|(pid, process)| {
            if path_key(&process.path) != group.path_key {
                return None;
            }
            windows.get(pid).filter(|title| !title.is_empty()).cloned()
        })
        .unwrap_or_default()
}

fn reaches(start: u32, target: u32, by_pid: &HashMap<u32, ProcessInfo>) -> bool {
    if target == 0 {
        return false;
    }
    let mut current = start;
    let mut seen = HashSet::new();
    for _ in 0..32 {
        if current == 0 || !seen.insert(current) {
            return false;
        }
        if current == target {
            return true;
        }
        let Some(process) = by_pid.get(&current) else {
            return false;
        };
        if process.parent_pid == current {
            return false;
        }
        current = process.parent_pid;
    }
    false
}

fn path_key(path: &str) -> String {
    path.trim().to_lowercase()
}

#[cfg(windows)]
fn foreground_pid() -> u32 {
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return 0;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        pid
    }
}

#[cfg(windows)]
fn visible_windows() -> Result<HashMap<u32, String>, String> {
    use windows::Win32::Foundation::LPARAM;
    use windows::Win32::UI::WindowsAndMessaging::EnumWindows;

    let mut titles = HashMap::new();
    unsafe {
        EnumWindows(
            Some(collect_window),
            LPARAM(&mut titles as *mut HashMap<u32, String> as isize),
        )
        .map_err(|error| format!("Couldn't list windows: {error}"))?;
    }
    Ok(titles)
}

#[cfg(windows)]
unsafe extern "system" fn collect_window(
    hwnd: windows::Win32::Foundation::HWND,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::core::BOOL {
    use windows::Win32::Foundation::TRUE;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
    };

    unsafe {
        if !IsWindowVisible(hwnd).as_bool() {
            return TRUE;
        }
        let titles = &mut *(lparam.0 as *mut HashMap<u32, String>);
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return TRUE;
        }
        let mut buffer = [0u16; 512];
        let len = GetWindowTextW(hwnd, &mut buffer);
        let title = if len > 0 {
            String::from_utf16_lossy(&buffer[..len as usize])
        } else {
            String::new()
        };
        match titles.get_mut(&pid) {
            Some(current) if current.is_empty() && !title.is_empty() => *current = title,
            Some(_) => {}
            None => {
                titles.insert(pid, title);
            }
        }
        TRUE
    }
}

#[cfg(windows)]
fn outbound_connections() -> Result<Vec<OwnedConnection>, String> {
    use std::net::{Ipv4Addr, Ipv6Addr};
    use windows::Win32::NetworkManagement::IpHelper::{
        GetExtendedTcpTable, MIB_TCP6TABLE_OWNER_PID, MIB_TCPTABLE_OWNER_PID,
        TCP_TABLE_OWNER_PID_ALL,
    };
    use windows::Win32::Networking::WinSock::{AF_INET, AF_INET6};

    const ESTABLISHED: u32 = 5;
    const SYN_SENT: u32 = 3;
    const CLOSE_WAIT: u32 = 8;

    let mut owned = Vec::new();
    unsafe {
        let buffer = crate::processes::read_table(|table, size| {
            GetExtendedTcpTable(
                table,
                size,
                false,
                AF_INET.0 as u32,
                TCP_TABLE_OWNER_PID_ALL,
                0,
            )
        })?;
        let table = &*(buffer.as_ptr() as *const MIB_TCPTABLE_OWNER_PID);
        let rows = std::slice::from_raw_parts(table.table.as_ptr(), table.dwNumEntries as usize);
        for row in rows {
            if !matches!(row.dwState, ESTABLISHED | SYN_SENT | CLOSE_WAIT) || row.dwOwningPid == 0 {
                continue;
            }
            let remote = Ipv4Addr::from(u32::from_be(row.dwRemoteAddr));
            let remote_port = crate::processes::port_of(row.dwRemotePort);
            if remote.is_unspecified() || remote_port == 0 {
                continue;
            }
            let local_port = crate::processes::port_of(row.dwLocalPort);
            let local = Ipv4Addr::from(u32::from_be(row.dwLocalAddr));
            owned.push(OwnedConnection {
                pid: row.dwOwningPid,
                connection: AppCallConnection {
                    protocol: "TCP".into(),
                    local_address: format!("{local}:{local_port}"),
                    remote_address: remote.to_string(),
                    remote_port,
                    state: crate::processes::tcp_state(row.dwState).into(),
                    local: remote.is_loopback(),
                },
            });
        }

        let buffer = crate::processes::read_table(|table, size| {
            GetExtendedTcpTable(
                table,
                size,
                false,
                AF_INET6.0 as u32,
                TCP_TABLE_OWNER_PID_ALL,
                0,
            )
        })?;
        let table = &*(buffer.as_ptr() as *const MIB_TCP6TABLE_OWNER_PID);
        let rows = std::slice::from_raw_parts(table.table.as_ptr(), table.dwNumEntries as usize);
        for row in rows {
            if !matches!(row.dwState, ESTABLISHED | SYN_SENT | CLOSE_WAIT) || row.dwOwningPid == 0 {
                continue;
            }
            let remote = Ipv6Addr::from(row.ucRemoteAddr);
            let remote_port = crate::processes::port_of(row.dwRemotePort);
            if remote.is_unspecified() || remote_port == 0 {
                continue;
            }
            let local_port = crate::processes::port_of(row.dwLocalPort);
            let local = Ipv6Addr::from(row.ucLocalAddr);
            let loopback = remote.is_loopback()
                || remote
                    .to_ipv4_mapped()
                    .is_some_and(|address| address.is_loopback());
            owned.push(OwnedConnection {
                pid: row.dwOwningPid,
                connection: AppCallConnection {
                    protocol: "TCPv6".into(),
                    local_address: format!("[{local}]:{local_port}"),
                    remote_address: remote.to_string(),
                    remote_port,
                    state: crate::processes::tcp_state(row.dwState).into(),
                    local: loopback,
                },
            });
        }
    }
    Ok(owned)
}

#[cfg(test)]
mod tests {
    use super::{AppCallConnection, OwnedConnection, assemble};
    use crate::processes::ProcessInfo;
    use std::collections::HashMap;

    fn listed(
        processes: Vec<ProcessInfo>,
        windows: HashMap<u32, String>,
        foreground_pid: u32,
        connections: Vec<OwnedConnection>,
    ) -> super::AppCallsSnapshot {
        assemble(
            processes,
            windows,
            foreground_pid,
            connections,
            &HashMap::new(),
        )
    }

    fn process(pid: u32, parent: u32, name: &str, path: &str) -> ProcessInfo {
        ProcessInfo {
            pid,
            parent_pid: parent,
            name: name.into(),
            path: path.into(),
            memory_bytes: 0,
        }
    }

    fn connection(pid: u32, remote: &str, port: u16, local: bool) -> OwnedConnection {
        OwnedConnection {
            pid,
            connection: AppCallConnection {
                protocol: "TCP".into(),
                local_address: format!("10.0.0.2:{port}"),
                remote_address: remote.into(),
                remote_port: port,
                state: "ESTABLISHED".into(),
                local,
            },
        }
    }

    #[test]
    fn foreground_helpers_stay_together_and_services_are_listed() {
        let processes = vec![
            process(10, 1, "chrome.exe", r"C:\App\chrome.exe"),
            process(11, 10, "chrome.exe", r"C:\App\chrome.exe"),
            process(12, 11, "chrome.exe", r"c:\app\chrome.exe"),
            process(20, 1, "notepad.exe", r"C:\Windows\notepad.exe"),
            process(4, 1, "svchost.exe", r"C:\Windows\System32\svchost.exe"),
        ];
        let mut windows = HashMap::new();
        windows.insert(10, "Inbox - Gmail".into());
        windows.insert(20, "Untitled - Notepad".into());
        let connections = vec![
            connection(12, "1.1.1.1", 443, false),
            connection(11, "9.9.9.9", 443, false),
            connection(20, "127.0.0.1", 80, true),
            connection(4, "8.8.8.8", 443, false),
        ];

        let snapshot = listed(processes, windows, 10, connections);

        assert_eq!(snapshot.foreground_pid, 10);
        assert_eq!(snapshot.apps.len(), 3);
        let foreground = &snapshot.apps[0];
        assert_eq!(foreground.placement, "foreground");
        assert_eq!(foreground.pid, 10);
        assert_eq!(foreground.name, "chrome.exe");
        assert_eq!(foreground.window_title, "Inbox - Gmail");
        assert_eq!(foreground.connections.len(), 2);
        assert_eq!(foreground.connections[0].remote_address, "1.1.1.1");
        assert_eq!(foreground.connections[1].remote_address, "9.9.9.9");
        assert_eq!(snapshot.apps[1].name, "notepad.exe");
        assert_eq!(snapshot.apps[1].placement, "background");
        assert!(snapshot.apps[1].connections[0].local);
        assert_eq!(snapshot.apps[2].name, "svchost.exe");
        assert_eq!(snapshot.apps[2].placement, "background");
        assert_eq!(snapshot.apps[2].connections[0].remote_address, "8.8.8.8");
    }

    #[test]
    fn a_child_with_another_exe_stays_with_the_foreground_app() {
        let processes = vec![
            process(10, 1, "app.exe", r"C:\App\app.exe"),
            process(11, 10, "helper.exe", r"C:\App\helper.exe"),
        ];
        let mut windows = HashMap::new();
        windows.insert(10, "App".into());
        let snapshot = listed(
            processes,
            windows,
            10,
            vec![connection(11, "1.2.3.4", 443, false)],
        );
        assert_eq!(snapshot.apps.len(), 1);
        assert_eq!(snapshot.apps[0].placement, "foreground");
        assert_eq!(snapshot.apps[0].pid, 10);
        assert_eq!(snapshot.apps[0].connections.len(), 1);
    }

    #[test]
    fn a_background_child_with_another_exe_is_its_own_process() {
        let processes = vec![
            process(30, 1, "keep.exe", r"C:\Keep\keep.exe"),
            process(31, 30, "helper.exe", r"C:\Keep\helper.exe"),
        ];
        let mut windows = HashMap::new();
        windows.insert(30, "Keep".into());
        let snapshot = listed(
            processes,
            windows,
            99,
            vec![connection(31, "1.2.3.4", 443, false)],
        );
        assert_eq!(snapshot.apps.len(), 1);
        assert_eq!(snapshot.apps[0].placement, "background");
        assert_eq!(snapshot.apps[0].name, "helper.exe");
        assert_eq!(snapshot.apps[0].pid, 31);
    }

    #[test]
    fn the_same_path_as_a_windowed_app_is_that_background_app() {
        let processes = vec![
            process(20, 1, "code.exe", r"C:\Code\code.exe"),
            process(21, 20, "code.exe", r"C:\Code\code.exe"),
        ];
        let mut windows = HashMap::new();
        windows.insert(20, "main.rs".into());
        let snapshot = listed(
            processes,
            windows,
            99,
            vec![connection(21, "1.2.3.4", 443, false)],
        );
        assert_eq!(snapshot.apps.len(), 1);
        assert_eq!(snapshot.apps[0].placement, "background");
        assert_eq!(snapshot.apps[0].pid, 20);
        assert_eq!(snapshot.apps[0].window_title, "main.rs");
        assert_eq!(snapshot.apps[0].connections.len(), 1);
    }

    #[test]
    fn an_idle_foreground_app_is_absent_and_its_pid_is_kept() {
        let processes = vec![
            process(10, 1, "idle.exe", r"C:\Idle\idle.exe"),
            process(20, 1, "busy.exe", r"C:\Busy\busy.exe"),
        ];
        let mut windows = HashMap::new();
        windows.insert(10, "Idle".into());
        windows.insert(20, "Busy".into());
        let snapshot = listed(
            processes,
            windows,
            10,
            vec![connection(20, "5.5.5.5", 8080, false)],
        );
        assert_eq!(snapshot.foreground_pid, 10);
        assert_eq!(snapshot.apps.len(), 1);
        assert_eq!(snapshot.apps[0].placement, "background");
        assert_eq!(snapshot.apps[0].name, "busy.exe");
        assert_eq!(snapshot.apps[0].cpu_percent, None);
    }

    #[test]
    fn a_row_adds_cpu_and_memory_for_every_process_it_contains() {
        let mut window = process(10, 1, "chrome.exe", r"C:\App\chrome.exe");
        window.memory_bytes = 100;
        let mut helper = process(11, 10, "chrome.exe", r"C:\App\chrome.exe");
        helper.memory_bytes = 250;
        let mut windows = HashMap::new();
        windows.insert(10, "Inbox".into());
        let mut cpu = HashMap::new();
        cpu.insert(10, 1.5);
        cpu.insert(11, 4.0);
        let snapshot = assemble(
            vec![window, helper],
            windows,
            10,
            vec![connection(11, "1.1.1.1", 443, false)],
            &cpu,
        );
        assert_eq!(snapshot.apps.len(), 1);
        assert_eq!(snapshot.apps[0].memory_bytes, 350);
        assert_eq!(snapshot.apps[0].cpu_percent, Some(5.5));
    }
}
