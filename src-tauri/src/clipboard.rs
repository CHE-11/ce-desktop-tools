//! Encrypted clipboard history. Recording stays off while the vault is locked.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::vault_crypto::{VaultKey, open_bytes_with_key, seal_bytes_with_key};

const FILE_NAME: &str = "clipboard.json";
const MAX_UNPINNED: usize = 200;
const MAX_TEXT: usize = 100_000;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Clip {
    pub id: String,
    pub text: String,
    pub pinned: bool,
    #[serde(default)]
    pub copied_at: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct ClipboardFile {
    clips: Vec<Clip>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardState {
    pub locked: bool,
    pub clips: Vec<Clip>,
    pub error: Option<String>,
}

struct Inner {
    app: AppHandle,
    locked: AtomicBool,
    key: Mutex<Option<VaultKey>>,
    folder: Mutex<String>,
    clips: Mutex<Vec<Clip>>,
    own_text: Mutex<Option<String>>,
    error: Mutex<Option<String>>,
    silence_until: AtomicU64,
}

pub struct Clipboard {
    inner: Arc<Inner>,
}

impl Clipboard {
    pub fn start(app: AppHandle) -> Self {
        let inner = Arc::new(Inner {
            app,
            locked: AtomicBool::new(true),
            key: Mutex::new(None),
            folder: Mutex::new(String::new()),
            clips: Mutex::new(Vec::new()),
            own_text: Mutex::new(None),
            error: Mutex::new(None),
            silence_until: AtomicU64::new(0),
        });
        install_listener(Arc::clone(&inner));
        Self { inner }
    }

    pub fn on_locked(&self) {
        self.inner.locked.store(true, Ordering::Relaxed);
        if let Ok(mut key) = self.inner.key.lock() {
            *key = None;
        }
        if let Ok(mut clips) = self.inner.clips.lock() {
            clips.clear();
        }
        self.inner.emit();
    }

    pub fn on_unlocked(&self, key: VaultKey, folder: String) {
        if let Ok(mut slot) = self.inner.key.lock() {
            *slot = Some(key);
        }
        if let Ok(mut slot) = self.inner.folder.lock() {
            *slot = folder;
        }
        self.inner.locked.store(false, Ordering::Relaxed);
        self.inner.reload();
        self.inner.emit();
    }

    pub fn state(&self) -> ClipboardState {
        self.inner.view()
    }

    pub fn set_pinned(&self, id: String, pinned: bool) -> Result<ClipboardState, String> {
        self.mutate(|clips| {
            if let Some(clip) = clips.iter_mut().find(|clip| clip.id == id) {
                clip.pinned = pinned;
            }
        })
    }

    pub fn delete(&self, id: String) -> Result<ClipboardState, String> {
        self.mutate(|clips| {
            clips.retain(|clip| clip.id != id);
        })
    }

    /// Rewrites clipboard.json with a new vault key. Returns the previous file bytes so the caller can restore them.
    pub fn reencrypt(
        &self,
        folder: &str,
        old: &VaultKey,
        new: &VaultKey,
    ) -> Result<Option<Vec<u8>>, String> {
        let folder = if folder.is_empty() {
            self.inner
                .folder
                .lock()
                .map(|slot| slot.clone())
                .unwrap_or_default()
        } else {
            folder.to_string()
        };
        if folder.is_empty() {
            return Ok(None);
        }
        let path = PathBuf::from(&folder).join(FILE_NAME);
        if !path.is_file() {
            return Ok(None);
        }
        let previous = std::fs::read(&path)
            .map_err(|error| format!("Couldn't read {}: {error}", path.display()))?;
        let plain = open_bytes_with_key(&String::from_utf8_lossy(&previous), old)?;
        let sealed = seal_bytes_with_key(&plain, new)?;
        write_vault_file(&path, &sealed)?;
        Ok(Some(previous))
    }

    pub fn restore_encrypted(&self, folder: &str, bytes: &[u8]) -> Result<(), String> {
        let folder = if folder.is_empty() {
            self.inner
                .folder
                .lock()
                .map(|slot| slot.clone())
                .unwrap_or_default()
        } else {
            folder.to_string()
        };
        if folder.is_empty() {
            return Ok(());
        }
        let path = PathBuf::from(folder).join(FILE_NAME);
        std::fs::write(&path, bytes)
            .map_err(|error| format!("Couldn't restore {}: {error}", path.display()))
    }

    pub fn suppress_next(&self, text: String) {
        if let Ok(mut own) = self.inner.own_text.lock() {
            *own = Some(text);
        }
    }

    /// Ignore clipboard changes until this many milliseconds from now. A probe copy uses this so it is not stored as history.
    pub fn silence_for(&self, millis: u64) {
        let until = now_millis().saturating_add(millis);
        let _ = self.inner.silence_until.fetch_max(until, Ordering::SeqCst);
    }

    pub fn copy(&self, id: String) -> Result<(), String> {
        let text = self
            .inner
            .clips
            .lock()
            .map_err(|_| "Clipboard history is unavailable.".to_string())?
            .iter()
            .find(|clip| clip.id == id)
            .map(|clip| clip.text.clone())
            .ok_or_else(|| "That clip is no longer in the history.".to_string())?;
        if let Ok(mut own) = self.inner.own_text.lock() {
            *own = Some(text.clone());
        }
        set_clipboard_text(&text)
    }

    fn mutate(&self, change: impl FnOnce(&mut Vec<Clip>)) -> Result<ClipboardState, String> {
        if self.inner.locked.load(Ordering::Relaxed) {
            return Err("Unlock the text expansion vault first.".into());
        }
        {
            let mut clips = self
                .inner
                .clips
                .lock()
                .map_err(|_| "Clipboard history is unavailable.".to_string())?;
            change(&mut clips);
        }
        self.inner.persist()?;
        self.inner.emit();
        Ok(self.inner.view())
    }
}

impl Inner {
    fn record(&self, text: String) {
        if now_millis() < self.silence_until.load(Ordering::SeqCst) {
            return;
        }
        if self.locked.load(Ordering::Relaxed) {
            return;
        }
        let text = text.trim().to_string();
        if text.is_empty() || text.chars().count() > MAX_TEXT {
            return;
        }
        if self
            .own_text
            .lock()
            .ok()
            .and_then(|mut own| {
                if own.as_deref() == Some(text.as_str()) {
                    *own = None;
                    Some(())
                } else {
                    None
                }
            })
            .is_some()
        {
            return;
        }
        {
            let mut clips = match self.clips.lock() {
                Ok(clips) => clips,
                Err(_) => return,
            };
            push_clip(&mut clips, text);
        }
        if self.persist().is_ok() {
            self.emit();
        }
    }

    fn reload(&self) {
        let folder = self
            .folder
            .lock()
            .map(|slot| slot.clone())
            .unwrap_or_default();
        let key = self.key.lock().ok().and_then(|slot| slot.clone());
        let Some(key) = key else {
            return;
        };
        let path = PathBuf::from(&folder).join(FILE_NAME);
        if !path.is_file() {
            if let Ok(mut clips) = self.clips.lock() {
                clips.clear();
            }
            if let Ok(mut error) = self.error.lock() {
                *error = None;
            }
            return;
        }
        match std::fs::read_to_string(&path).and_then(|raw| {
            open_bytes_with_key(&raw, &key)
                .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
        }) {
            Ok(bytes) => {
                let file: ClipboardFile = serde_json::from_slice(&bytes).unwrap_or_default();
                if let Ok(mut clips) = self.clips.lock() {
                    *clips = file.clips;
                }
                if let Ok(mut error) = self.error.lock() {
                    *error = None;
                }
            }
            Err(error) => {
                if let Ok(mut clips) = self.clips.lock() {
                    clips.clear();
                }
                if let Ok(mut slot) = self.error.lock() {
                    *slot = Some(format!("Couldn't read clipboard history: {error}"));
                }
            }
        }
    }

    fn persist(&self) -> Result<(), String> {
        let folder = self
            .folder
            .lock()
            .map(|slot| slot.clone())
            .unwrap_or_default();
        let key = self
            .key
            .lock()
            .ok()
            .and_then(|slot| slot.clone())
            .ok_or_else(|| "Unlock the text expansion vault first.".to_string())?;
        let clips = self
            .clips
            .lock()
            .map_err(|_| "Clipboard history is unavailable.".to_string())?
            .clone();
        let plain = serde_json::to_vec(&ClipboardFile { clips })
            .map_err(|error| format!("Couldn't save clipboard history: {error}"))?;
        let raw = seal_bytes_with_key(&plain, &key)?;
        let path = PathBuf::from(folder).join(FILE_NAME);
        let temporary = path.with_extension("json.tmp");
        std::fs::write(&temporary, &raw)
            .map_err(|error| format!("Couldn't save {}: {error}", path.display()))?;
        if std::fs::rename(&temporary, &path).is_err() {
            let wrote = std::fs::write(&path, &raw);
            let _ = std::fs::remove_file(&temporary);
            wrote.map_err(|error| format!("Couldn't save {}: {error}", path.display()))?;
        }
        Ok(())
    }

    fn view(&self) -> ClipboardState {
        let locked = self.locked.load(Ordering::Relaxed);
        ClipboardState {
            locked,
            clips: if locked {
                Vec::new()
            } else {
                self.clips
                    .lock()
                    .map(|slot| slot.clone())
                    .unwrap_or_default()
            },
            error: self.error.lock().ok().and_then(|slot| slot.clone()),
        }
    }

    fn emit(&self) {
        let _ = self.app.emit("clipboard-changed", self.view());
    }
}

fn write_vault_file(path: &std::path::Path, raw: &str) -> Result<(), String> {
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, raw)
        .map_err(|error| format!("Couldn't save {}: {error}", path.display()))?;
    if std::fs::rename(&temporary, path).is_err() {
        let wrote = std::fs::write(path, raw);
        let _ = std::fs::remove_file(&temporary);
        wrote.map_err(|error| format!("Couldn't save {}: {error}", path.display()))?;
    }
    Ok(())
}

fn push_clip(clips: &mut Vec<Clip>, text: String) {
    let pinned = clips
        .iter()
        .find(|clip| clip.text == text)
        .map(|clip| clip.pinned)
        .unwrap_or(false);
    let id = clips
        .iter()
        .find(|clip| clip.text == text)
        .map(|clip| clip.id.clone())
        .unwrap_or_else(new_id);
    clips.retain(|clip| clip.text != text);
    clips.insert(
        0,
        Clip {
            id,
            text,
            pinned,
            copied_at: now_millis(),
        },
    );
    let mut unpinned = 0;
    clips.retain(|clip| {
        if clip.pinned {
            return true;
        }
        unpinned += 1;
        unpinned <= MAX_UNPINNED
    });
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn new_id() -> String {
    format!("c{:x}", now_millis())
}

fn install_listener(inner: Arc<Inner>) {
    #[cfg(windows)]
    {
        LISTENER.set(Arc::clone(&inner)).ok();
        std::thread::Builder::new()
            .name("clipboard-listener".into())
            .spawn(|| unsafe { listen_loop(inner) })
            .ok();
    }
    #[cfg(not(windows))]
    {
        let _ = inner;
    }
}

#[cfg(windows)]
static LISTENER: OnceLock<Arc<Inner>> = OnceLock::new();

#[cfg(windows)]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn listen_loop(inner: Arc<Inner>) {
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::DataExchange::AddClipboardFormatListener;
    use windows::Win32::UI::WindowsAndMessaging::{
        CS_HREDRAW, CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, HWND_MESSAGE,
        MSG, RegisterClassW, TranslateMessage, WINDOW_EX_STYLE, WNDCLASSW, WS_OVERLAPPED,
    };
    use windows::core::w;

    unsafe extern "system" fn wndproc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if message == 0x031D {
            if let Some(listener) = LISTENER.get() {
                if let Some(text) = read_clipboard_text() {
                    listener.record(text);
                }
            }
            return LRESULT(0);
        }
        unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
    }

    let class = w!("ce-desktop-tools-clipboard");
    let window_class = WNDCLASSW {
        style: CS_HREDRAW,
        lpfnWndProc: Some(wndproc),
        hInstance: windows::Win32::System::LibraryLoader::GetModuleHandleW(None)
            .unwrap_or_default()
            .into(),
        lpszClassName: class,
        ..Default::default()
    };
    if RegisterClassW(&window_class) == 0 {
        eprintln!("ce-desktop-tools could not watch the clipboard");
        return;
    }
    let hwnd = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        class,
        w!("clipboard"),
        WS_OVERLAPPED,
        0,
        0,
        0,
        0,
        Some(HWND_MESSAGE),
        None,
        None,
        None,
    );
    let Ok(hwnd) = hwnd else {
        eprintln!("ce-desktop-tools could not watch the clipboard");
        return;
    };
    if AddClipboardFormatListener(hwnd).is_err() {
        eprintln!("ce-desktop-tools could not watch the clipboard");
        return;
    }
    let mut message = MSG::default();
    while GetMessageW(&mut message, None, 0, 0).0 != 0 {
        let _ = TranslateMessage(&message);
        DispatchMessageW(&message);
    }
    let _ = inner;
}

#[cfg(windows)]
static CLIPBOARD_LOCK: Mutex<()> = Mutex::new(());

#[cfg(windows)]
pub(crate) fn read_clipboard_text() -> Option<String> {
    let _lock = CLIPBOARD_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let text = clipboard_win::get_clipboard_string().ok()?;
    if text.is_empty() || text.encode_utf16().count() > MAX_TEXT {
        return None;
    }
    Some(text)
}

#[cfg(windows)]
pub(crate) fn set_clipboard_text(text: &str) -> Result<(), String> {
    let _lock = CLIPBOARD_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clipboard_win::set_clipboard_string(text).map_err(|_| "Couldn't copy that clip.".into())
}

#[cfg(not(windows))]
pub(crate) fn read_clipboard_text() -> Option<String> {
    None
}

#[cfg(not(windows))]
pub(crate) fn set_clipboard_text(_text: &str) -> Result<(), String> {
    Err("Clipboard history is only available on Windows.".into())
}

#[cfg(test)]
mod tests {
    use super::{Clip, MAX_UNPINNED, push_clip};

    fn clip(text: &str, pinned: bool) -> Clip {
        Clip {
            id: text.into(),
            text: text.into(),
            pinned,
            copied_at: 0,
        }
    }

    #[test]
    fn a_repeat_moves_to_the_front_and_keeps_the_pin() {
        let mut clips = vec![clip("old", false), clip("keep", true)];
        push_clip(&mut clips, "keep".into());
        assert_eq!(clips[0].text, "keep");
        assert!(clips[0].pinned);
        assert_eq!(clips.len(), 2);
    }

    #[test]
    fn pins_survive_when_unpinned_clips_are_trimmed() {
        let mut clips = vec![clip("pinned", true)];
        for index in 0..=MAX_UNPINNED {
            push_clip(&mut clips, format!("item-{index}"));
        }
        assert!(
            clips
                .iter()
                .any(|clip| clip.text == "pinned" && clip.pinned)
        );
        assert_eq!(
            clips.iter().filter(|clip| !clip.pinned).count(),
            MAX_UNPINNED
        );
        assert!(!clips.iter().any(|clip| clip.text == "item-0"));
    }
}
