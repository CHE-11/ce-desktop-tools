//! Interactive shells on a pseudo console. Each session owns a ConPTY pair; a
//! reader thread streams output as `terminal-output` events and a waiter
//! thread reports `terminal-exit` and drops the session.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellInfo {
    pub id: String,
    pub label: String,
    pub path: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalSession {
    pub id: String,
    pub shell: String,
    pub cwd: String,
    pub title: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OutputEvent<'a> {
    id: &'a str,
    data: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExitEvent<'a> {
    id: &'a str,
    code: Option<u32>,
}

struct Session {
    info: TerminalSession,
    master: Mutex<Box<dyn MasterPty + Send>>,
    writer: Mutex<Box<dyn Write + Send>>,
    killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
}

#[derive(Clone, Default)]
pub struct Terminals {
    sessions: Arc<Mutex<HashMap<String, Arc<Session>>>>,
}

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

pub fn shells() -> Vec<ShellInfo> {
    let mut found = vec![ShellInfo {
        id: "cmd".into(),
        label: "Command Prompt".into(),
        path: system_exe("cmd.exe"),
    }];
    let powershell = windows_powershell();
    if Path::new(&powershell).is_file() || which("powershell.exe").is_some() {
        found.push(ShellInfo {
            id: "powershell".into(),
            label: "Windows PowerShell".into(),
            path: powershell,
        });
    }
    if let Some(pwsh) = pwsh_path() {
        found.push(ShellInfo {
            id: "pwsh".into(),
            label: "PowerShell 7".into(),
            path: pwsh.display().to_string(),
        });
    }
    found
}

impl Terminals {
    pub fn spawn(
        &self,
        app: AppHandle,
        shell: &str,
        cwd: Option<String>,
        cols: u16,
        rows: u16,
    ) -> Result<TerminalSession, String> {
        let info = shells()
            .into_iter()
            .find(|candidate| candidate.id == shell)
            .ok_or_else(|| format!("{shell} is not installed."))?;
        let cwd = cwd
            .map(|path| path.trim().to_string())
            .filter(|path| !path.is_empty() && Path::new(path).is_dir())
            .or_else(|| std::env::var("USERPROFILE").ok())
            .unwrap_or_else(|| "C:\\".into());

        let pair = native_pty_system()
            .openpty(size(cols, rows))
            .map_err(|error| format!("Couldn't open a console: {error}"))?;
        let mut command = CommandBuilder::new(&info.path);
        if info.id != "cmd" {
            command.arg("-NoLogo");
        }
        command.cwd(&cwd);
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        let mut child = pair
            .slave
            .spawn_command(command)
            .map_err(|error| format!("Couldn't start {}: {error}", info.label))?;
        drop(pair.slave);
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|error| format!("Couldn't read the console: {error}"))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|error| format!("Couldn't write to the console: {error}"))?;

        let id = format!("term-{}", NEXT_ID.fetch_add(1, Ordering::Relaxed));
        let session = TerminalSession {
            id: id.clone(),
            shell: info.id.clone(),
            cwd,
            title: info.label.clone(),
        };
        self.sessions.lock().unwrap_or_else(|p| p.into_inner()).insert(
            id.clone(),
            Arc::new(Session {
                info: session.clone(),
                master: Mutex::new(pair.master),
                writer: Mutex::new(writer),
                killer: Mutex::new(child.clone_killer()),
            }),
        );

        let reader_app = app.clone();
        let reader_id = id.clone();
        let _ = std::thread::Builder::new()
            .name(format!("{id}-read"))
            .spawn(move || {
                let mut buffer = [0u8; 8192];
                let mut pending: Vec<u8> = Vec::new();
                loop {
                    match reader.read(&mut buffer) {
                        Ok(0) | Err(_) => break,
                        Ok(count) => {
                            pending.extend_from_slice(&buffer[..count]);
                            let data = take_utf8(&mut pending);
                            if !data.is_empty() {
                                let _ = reader_app.emit(
                                    "terminal-output",
                                    OutputEvent { id: &reader_id, data },
                                );
                            }
                        }
                    }
                }
            });

        let terminals = self.clone();
        let _ = std::thread::Builder::new()
            .name(format!("{id}-wait"))
            .spawn(move || {
                let code = child.wait().ok().map(|status| status.exit_code());
                terminals.remove(&id);
                let _ = app.emit("terminal-exit", ExitEvent { id: &id, code });
            });

        Ok(session)
    }

    pub fn write(&self, id: &str, data: &str) -> Result<(), String> {
        let session = self.get(id)?;
        let mut writer = session.writer.lock().unwrap_or_else(|p| p.into_inner());
        writer
            .write_all(data.as_bytes())
            .and_then(|_| writer.flush())
            .map_err(|error| format!("Couldn't write to the terminal: {error}"))
    }

    pub fn resize(&self, id: &str, cols: u16, rows: u16) -> Result<(), String> {
        let session = self.get(id)?;
        let master = session.master.lock().unwrap_or_else(|p| p.into_inner());
        master
            .resize(size(cols, rows))
            .map_err(|error| format!("Couldn't resize the terminal: {error}"))
    }

    pub fn kill(&self, id: &str) {
        if let Ok(session) = self.get(id) {
            let _ = session.killer.lock().unwrap_or_else(|p| p.into_inner()).kill();
        }
        self.remove(id);
    }

    pub fn kill_all(&self) {
        let ids: Vec<String> = self
            .sessions
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .keys()
            .cloned()
            .collect();
        for id in ids {
            self.kill(&id);
        }
    }

    pub fn list(&self) -> Vec<TerminalSession> {
        let mut sessions: Vec<TerminalSession> = self
            .sessions
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .values()
            .map(|session| session.info.clone())
            .collect();
        sessions.sort_by_key(|session| {
            session.id.trim_start_matches("term-").parse::<u64>().unwrap_or(0)
        });
        sessions
    }

    fn get(&self, id: &str) -> Result<Arc<Session>, String> {
        self.sessions
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(id)
            .cloned()
            .ok_or_else(|| "That terminal has closed.".into())
    }

    fn remove(&self, id: &str) {
        self.sessions
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(id);
    }
}

fn size(cols: u16, rows: u16) -> PtySize {
    PtySize {
        rows: rows.max(2),
        cols: cols.max(10),
        pixel_width: 0,
        pixel_height: 0,
    }
}

/// Decodes as much of `pending` as is valid UTF-8 and keeps a split character for the next read.
fn take_utf8(pending: &mut Vec<u8>) -> String {
    match std::str::from_utf8(pending) {
        Ok(text) => {
            let text = text.to_string();
            pending.clear();
            text
        }
        Err(error) if error.error_len().is_none() => {
            let valid = error.valid_up_to();
            let text = String::from_utf8_lossy(&pending[..valid]).into_owned();
            pending.drain(..valid);
            text
        }
        Err(_) => {
            let text = String::from_utf8_lossy(pending).into_owned();
            pending.clear();
            text
        }
    }
}

fn system_root() -> PathBuf {
    PathBuf::from(std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into()))
}

fn system_exe(name: &str) -> String {
    system_root().join("System32").join(name).display().to_string()
}

fn windows_powershell() -> String {
    system_root()
        .join(r"System32\WindowsPowerShell\v1.0\powershell.exe")
        .display()
        .to_string()
}

fn pwsh_path() -> Option<PathBuf> {
    if let Some(found) = which("pwsh.exe") {
        return Some(found);
    }
    let program_files = std::env::var("ProgramFiles").unwrap_or_else(|_| "C:\\Program Files".into());
    let candidate = Path::new(&program_files).join(r"PowerShell\7\pwsh.exe");
    candidate.is_file().then_some(candidate)
}

fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

#[cfg(test)]
mod tests {
    use super::take_utf8;

    #[test]
    fn keeps_split_character() {
        let bytes = "é".as_bytes();
        let mut pending = vec![b'a', bytes[0]];
        assert_eq!(take_utf8(&mut pending), "a");
        pending.push(bytes[1]);
        assert_eq!(take_utf8(&mut pending), "é");
        assert!(pending.is_empty());
    }
}
