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
const MAX_RULES: usize = 40;
const MAX_PATTERN: usize = 2_000;
const CLEAN_SILENCE_MS: u64 = 400;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Clip {
    pub id: String,
    pub text: String,
    #[serde(default)]
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ClipRule {
    pub id: String,
    pub enabled: bool,
    pub find: String,
    pub replace: String,
    pub case_insensitive: bool,
}

impl Default for ClipRule {
    fn default() -> Self {
        Self {
            id: String::new(),
            enabled: true,
            find: String::new(),
            replace: String::new(),
            case_insensitive: false,
        }
    }
}

/// Find-and-replace rules applied to text as it lands on the Windows clipboard.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ClipCleanerFile {
    pub enabled: bool,
    pub rules: Vec<ClipRule>,
}

impl Default for ClipCleanerFile {
    fn default() -> Self {
        Self {
            enabled: false,
            rules: Vec::new(),
        }
    }
}

impl ClipCleanerFile {
    pub fn normalize(&mut self) {
        if self.rules.len() > MAX_RULES {
            self.rules.truncate(MAX_RULES);
        }
        let mut seen = std::collections::HashSet::new();
        for (index, rule) in self.rules.iter_mut().enumerate() {
            rule.find = truncate_chars(&rule.find, MAX_PATTERN);
            rule.replace = truncate_chars(&rule.replace, MAX_PATTERN);
            let trimmed = rule.id.trim().to_string();
            if !trimmed.is_empty() && seen.insert(trimmed.clone()) {
                rule.id = trimmed;
                continue;
            }
            let mut n = index;
            loop {
                let next = format!("rule-{n}");
                if seen.insert(next.clone()) {
                    rule.id = next;
                    break;
                }
                n += 1;
            }
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanerPreview {
    pub text: String,
    pub error: Option<String>,
}

struct CleanOutcome {
    text: String,
    error: Option<String>,
}

struct Inner {
    app: AppHandle,
    locked: AtomicBool,
    key: Mutex<Option<VaultKey>>,
    folder: Mutex<String>,
    clips: Mutex<Vec<Clip>>,
    cleaner: Mutex<ClipCleanerFile>,
    own_text: Mutex<Option<String>>,
    error: Mutex<Option<String>>,
    silence_until: AtomicU64,
}

pub struct Clipboard {
    inner: Arc<Inner>,
}

impl Clipboard {
    pub fn start(app: AppHandle) -> Self {
        let cleaner = crate::app_settings::load_clip_cleaner(&app).unwrap_or_default();
        let inner = Arc::new(Inner {
            app,
            locked: AtomicBool::new(true),
            key: Mutex::new(None),
            folder: Mutex::new(String::new()),
            clips: Mutex::new(Vec::new()),
            cleaner: Mutex::new(cleaner),
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

    pub fn delete(&self, id: String) -> Result<ClipboardState, String> {
        self.mutate(|clips| {
            clips.retain(|clip| clip.id != id);
        })
    }

    pub fn clear(&self) -> Result<ClipboardState, String> {
        self.mutate(|clips| {
            clips.clear();
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

    pub fn cleaner(&self) -> ClipCleanerFile {
        self.inner
            .cleaner
            .lock()
            .map(|slot| slot.clone())
            .unwrap_or_default()
    }

    pub fn save_cleaner(&self, next: ClipCleanerFile) -> Result<ClipCleanerFile, String> {
        let saved = crate::app_settings::save_clip_cleaner(&self.inner.app, next)?;
        if let Ok(mut slot) = self.inner.cleaner.lock() {
            *slot = saved.clone();
        }
        Ok(saved)
    }

    pub fn reload_cleaner(&self) {
        let file = crate::app_settings::load_clip_cleaner(&self.inner.app).unwrap_or_default();
        if let Ok(mut slot) = self.inner.cleaner.lock() {
            *slot = file;
        }
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
        let silenced = now_millis() < self.silence_until.load(Ordering::SeqCst);
        if self.take_own(&text) {
            return;
        }
        if silenced {
            if let Ok(mut own) = self.own_text.lock() {
                *own = None;
            }
            return;
        }
        let cleaned = self.cleaned(&text);
        let text = if cleaned == text {
            text
        } else if self.write_cleaned(&cleaned) {
            notify_cleaned(&text, &cleaned);
            cleaned
        } else {
            text
        };
        if self.locked.load(Ordering::Relaxed) {
            return;
        }
        let text = text.trim().to_string();
        if text.is_empty() || text.chars().count() > MAX_TEXT {
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

    fn take_own(&self, text: &str) -> bool {
        self.own_text
            .lock()
            .ok()
            .and_then(|mut own| {
                if own.as_deref() == Some(text) {
                    *own = None;
                    Some(())
                } else {
                    None
                }
            })
            .is_some()
    }

    fn cleaned(&self, text: &str) -> String {
        let cleaner = self
            .cleaner
            .lock()
            .map(|slot| slot.clone())
            .unwrap_or_default();
        apply_rules(&cleaner, text, MAX_TEXT).text
    }

    /// Writes cleaned text back and ignores the echo of that write.
    fn write_cleaned(&self, text: &str) -> bool {
        if let Ok(mut own) = self.own_text.lock() {
            *own = Some(text.to_string());
        }
        let until = now_millis().saturating_add(CLEAN_SILENCE_MS);
        let _ = self.silence_until.fetch_max(until, Ordering::SeqCst);
        if set_clipboard_text(text).is_ok() {
            return true;
        }
        if let Ok(mut own) = self.own_text.lock() {
            *own = None;
        }
        false
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
    if clips.len() > MAX_UNPINNED {
        clips.truncate(MAX_UNPINNED);
    }
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

pub fn preview_cleaner(mut cleaner: ClipCleanerFile, text: String) -> CleanerPreview {
    cleaner.normalize();
    cleaner.enabled = true;
    if text.chars().count() > MAX_TEXT {
        return CleanerPreview {
            text: String::new(),
            error: Some("That sample is too long to try.".into()),
        };
    }
    let outcome = apply_rules(&cleaner, &text, MAX_TEXT);
    CleanerPreview {
        text: outcome.text,
        error: outcome.error,
    }
}

fn apply_rules(cleaner: &ClipCleanerFile, text: &str, max_chars: usize) -> CleanOutcome {
    if !cleaner.enabled {
        return CleanOutcome {
            text: text.to_string(),
            error: None,
        };
    }
    let mut current = text.to_string();
    let mut errors = Vec::new();
    for (index, rule) in cleaner.rules.iter().enumerate() {
        if !rule.enabled || rule.find.is_empty() {
            continue;
        }
        let label = format!("Rule {}", index + 1);
        let mut builder = regex::RegexBuilder::new(&rule.find);
        builder.case_insensitive(rule.case_insensitive);
        builder.size_limit(1 << 20);
        let expression = match builder.build() {
            Ok(expression) => expression,
            Err(error) => {
                errors.push(format!(
                    "{label} is not a valid regular expression ({error})."
                ));
                continue;
            }
        };
        let next = expression.replace_all(&current, rule.replace.as_str());
        if next.chars().count() > max_chars {
            errors.push(format!(
                "{label} made the text too long, so it was skipped."
            ));
            continue;
        }
        current = next.into_owned();
    }
    CleanOutcome {
        text: current,
        error: if errors.is_empty() {
            None
        } else {
            Some(errors.join(" "))
        },
    }
}

fn notify_cleaned(before: &str, after: &str) {
    let body = clean_notice(before, after);
    #[cfg(windows)]
    {
        std::thread::Builder::new()
            .name("clip-cleaner-toast".into())
            .spawn(move || show_toast("Clipboard cleaned", &body))
            .ok();
    }
    #[cfg(not(windows))]
    {
        let _ = body;
    }
}

/// One line for the toast: what was copied, then what replaced it.
fn clean_notice(before: &str, after: &str) -> String {
    let after_line = one_line(after, 72);
    if after_line.is_empty() {
        return "The copy was cleared.".into();
    }
    let before_line = one_line(before, 72);
    if before_line == after_line {
        return "The copy was rewritten.".into();
    }
    format!("{before_line} → {after_line}")
}

fn one_line(text: &str, max: usize) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = flat.chars();
    let short: String = chars.by_ref().take(max).collect();
    if chars.next().is_some() {
        format!("{short}…")
    } else {
        short
    }
}

/// Installs the Start menu shortcut Windows requires before this process can toast.
pub fn register_notifications() {
    #[cfg(windows)]
    {
        use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        let fresh = hr.is_ok();
        ensure_toast_shortcut();
        if fresh {
            unsafe { CoUninitialize() };
        }
    }
}

#[cfg(windows)]
fn show_toast(title: &str, body: &str) {
    use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};
    // A single-threaded apartment with no message pump accepts the toast and then drops it.
    let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    let fresh = hr.is_ok();
    ensure_toast_shortcut();
    let shown = toast_with(Some("com.ce.desktop-tools"), title, body)
        .or_else(|_| toast_with(None, title, body))
        .or_else(|_| {
            toast_with(
                Some("{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\\WindowsPowerShell\\v1.0\\powershell.exe"),
                title,
                body,
            )
        });
    if let Err(error) = shown {
        eprintln!("ce-desktop-tools could not show a clipboard notice: {error}");
    }
    if fresh {
        unsafe { CoUninitialize() };
    }
}

#[cfg(windows)]
fn toast_with(app_id: Option<&str>, title: &str, body: &str) -> windows::core::Result<()> {
    use windows::Data::Xml::Dom::XmlDocument;
    use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
    use windows::core::HSTRING;

    let xml = XmlDocument::new()?;
    let markup = format!(
        "<toast duration=\"short\"><visual><binding template=\"ToastGeneric\"><text>{}</text><text>{}</text></binding></visual></toast>",
        xml_escape(title),
        xml_escape(body),
    );
    xml.LoadXml(&HSTRING::from(markup))?;
    let toast = ToastNotification::CreateToastNotification(&xml)?;
    let _ = toast.SetTag(&HSTRING::from("clip-cleaner"));
    let notifier = match app_id {
        Some(app_id) => {
            ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(app_id))?
        }
        None => ToastNotificationManager::CreateToastNotifier()?,
    };
    notifier.Show(&toast)?;
    // Keep the notifier alive until Windows has taken the toast.
    std::thread::sleep(std::time::Duration::from_millis(400));
    Ok(())
}

#[cfg(windows)]
fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Windows only shows a toast for an unpackaged app when a Start menu shortcut carries the same app id.
#[cfg(windows)]
fn ensure_toast_shortcut() {
    use std::sync::atomic::AtomicBool;
    static READY: AtomicBool = AtomicBool::new(false);
    if READY.load(Ordering::Acquire) {
        return;
    }
    match write_toast_shortcut() {
        Ok(()) => READY.store(true, Ordering::Release),
        Err(error) => {
            eprintln!("ce-desktop-tools could not register clipboard notices: {error}");
        }
    }
}

#[cfg(windows)]
fn write_toast_shortcut() -> Result<(), String> {
    use windows::Win32::Foundation::PROPERTYKEY;
    use windows::Win32::System::Com::StructuredStorage::PropVariantClear;
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, CoCreateInstance, IPersistFile, STGM_READWRITE,
    };
    use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
    use windows::core::{GUID, HSTRING, Interface};

    let appdata = std::env::var("APPDATA").map_err(|_| "APPDATA is not set.".to_string())?;
    let path = PathBuf::from(appdata)
        .join("Microsoft")
        .join("Windows")
        .join("Start Menu")
        .join("Programs")
        .join("ce-desktop-tools.lnk");
    let existed = path.is_file();
    if !existed {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("Couldn't create {}: {error}", parent.display()))?;
        }
    }
    let link: IShellLinkW = unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) }
        .map_err(|error| format!("Couldn't create a shortcut: {error}"))?;
    let file: IPersistFile = link
        .cast()
        .map_err(|error| format!("Couldn't save the shortcut: {error}"))?;
    let path_name = HSTRING::from(path.to_string_lossy().as_ref());
    if existed {
        let _ = unsafe { file.Load(&path_name, STGM_READWRITE) };
    }
    let exe = std::env::current_exe()
        .map_err(|error| format!("Couldn't find this program: {error}"))?;
    unsafe { link.SetPath(&HSTRING::from(exe.to_string_lossy().as_ref())) }
        .map_err(|error| format!("Couldn't point the shortcut at this program: {error}"))?;
    let _ = unsafe { link.SetDescription(&HSTRING::from("ce-desktop-tools")) };
    let store: IPropertyStore = link
        .cast()
        .map_err(|error| format!("Couldn't set the app id: {error}"))?;
    let key = PROPERTYKEY {
        fmtid: GUID::from_u128(0x9F4C2855_9F79_4B39_A8D0_E1D42DE1D5F3),
        pid: 5,
    };
    // The shell frees this string with CoTaskMemFree. A normal Rust allocation would corrupt the heap.
    let mut variant = app_id_variant("com.ce.desktop-tools")?;
    unsafe {
        let stored = store.SetValue(&key, &variant);
        let committed = stored.and_then(|()| store.Commit());
        let _ = PropVariantClear(&mut variant);
        committed.map_err(|error| format!("Couldn't set the app id: {error}"))?;
    }
    unsafe { file.Save(&path_name, true) }
        .map_err(|error| format!("Couldn't save {}: {error}", path.display()))?;
    unsafe {
        windows::Win32::UI::Shell::SHChangeNotify(
            windows::Win32::UI::Shell::SHCNE_ASSOCCHANGED,
            windows::Win32::UI::Shell::SHCNF_FLUSH,
            None,
            None,
        );
    }
    Ok(())
}

/// A `VT_LPWSTR` the shell is allowed to free. The text is allocated with `CoTaskMemAlloc`.
#[cfg(windows)]
fn app_id_variant(text: &str) -> Result<windows::Win32::System::Com::StructuredStorage::PROPVARIANT, String> {
    use windows::Win32::System::Com::CoTaskMemAlloc;
    use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
    use windows::Win32::System::Variant::VT_LPWSTR;
    use windows::core::PWSTR;

    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let bytes = wide.len() * std::mem::size_of::<u16>();
    let mem = unsafe { CoTaskMemAlloc(bytes) }.cast::<u16>();
    if mem.is_null() {
        return Err("Couldn't set the app id.".into());
    }
    unsafe { std::ptr::copy_nonoverlapping(wide.as_ptr(), mem, wide.len()) };
    let mut variant = PROPVARIANT::default();
    unsafe {
        let inner = &mut *variant.Anonymous.Anonymous;
        inner.vt = VT_LPWSTR;
        inner.Anonymous.pwszVal = PWSTR(mem);
    }
    Ok(variant)
}

fn truncate_chars(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        text.chars().take(max).collect()
    }
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

static CLIPBOARD_LOCK: Mutex<()> = Mutex::new(());

/// Hold this for as long as the Windows clipboard is open, everywhere in the app.
///
/// Windows lets a second thread of the same process open the clipboard while the
/// first still has it. The second thread's close then closes it for both, and the
/// first is left reading memory Windows has already freed. Do not take the gate
/// again while holding it.
pub(crate) fn clipboard_gate() -> std::sync::MutexGuard<'static, ()> {
    CLIPBOARD_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

#[cfg(windows)]
pub(crate) fn read_clipboard_text() -> Option<String> {
    let _gate = clipboard_gate();
    let text = clipboard_win::get_clipboard_string().ok()?;
    if text.is_empty() || text.encode_utf16().count() > MAX_TEXT {
        return None;
    }
    Some(text)
}

#[cfg(windows)]
pub(crate) fn set_clipboard_text(text: &str) -> Result<(), String> {
    let _gate = clipboard_gate();
    clipboard_win::set_clipboard_string(text).map_err(|_| "Couldn't copy that clip.".into())
}

#[cfg(windows)]
pub(crate) fn clear_clipboard() -> Result<(), String> {
    use windows::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard};
    let _gate = clipboard_gate();
    unsafe {
        if OpenClipboard(None).is_err() {
            return Err("Couldn't clear the Windows clipboard.".into());
        }
        let cleared = EmptyClipboard();
        let _ = CloseClipboard();
        cleared.map_err(|error| format!("Couldn't clear the Windows clipboard: {error}"))
    }
}

#[cfg(not(windows))]
pub(crate) fn read_clipboard_text() -> Option<String> {
    None
}

#[cfg(not(windows))]
pub(crate) fn set_clipboard_text(_text: &str) -> Result<(), String> {
    Err("Clipboard history is only available on Windows.".into())
}

#[cfg(not(windows))]
pub(crate) fn clear_clipboard() -> Result<(), String> {
    Err("Clipboard history is only available on Windows.".into())
}

#[cfg(test)]
mod tests {
    use super::{
        Clip, ClipCleanerFile, ClipRule, MAX_UNPINNED, apply_rules, clean_notice, push_clip,
    };

    fn rule(find: &str, replace: &str, case_insensitive: bool) -> ClipRule {
        ClipRule {
            id: find.into(),
            enabled: true,
            find: find.into(),
            replace: replace.into(),
            case_insensitive,
        }
    }

    fn clip(text: &str, pinned: bool) -> Clip {
        Clip {
            id: text.into(),
            text: text.into(),
            pinned,
            copied_at: 0,
        }
    }

    #[test]
    fn a_repeat_moves_to_the_front() {
        let mut clips = vec![clip("old", false), clip("keep", false)];
        push_clip(&mut clips, "keep".into());
        assert_eq!(clips[0].text, "keep");
        assert_eq!(clips.len(), 2);
    }

    #[test]
    fn history_keeps_the_newest_clips() {
        let mut clips = vec![clip("oldest", true)];
        for index in 0..=MAX_UNPINNED {
            push_clip(&mut clips, format!("item-{index}"));
        }
        assert!(!clips.iter().any(|clip| clip.text == "oldest"));
        assert_eq!(clips.len(), MAX_UNPINNED);
        assert!(!clips.iter().any(|clip| clip.text == "item-0"));
        assert_eq!(clips[0].text, format!("item-{MAX_UNPINNED}"));
    }

    #[test]
    fn rules_run_in_order_and_can_ignore_case() {
        let cleaner = ClipCleanerFile {
            enabled: true,
            rules: vec![rule("a", "b", false), rule("B+", "c", true)],
        };
        assert_eq!(apply_rules(&cleaner, "a Bbb", 100).text, "c c");
    }

    #[test]
    fn a_cleaner_that_is_off_leaves_the_text() {
        let cleaner = ClipCleanerFile {
            enabled: false,
            rules: vec![rule("a", "b", false)],
        };
        assert_eq!(apply_rules(&cleaner, "a", 100).text, "a");
        assert!(apply_rules(&cleaner, "a", 100).error.is_none());
    }

    #[test]
    fn an_empty_or_invalid_pattern_is_skipped() {
        let cleaner = ClipCleanerFile {
            enabled: true,
            rules: vec![
                rule("", "x", false),
                rule("(", "x", false),
                rule("a", "b", false),
            ],
        };
        let outcome = apply_rules(&cleaner, "a", 100);
        assert_eq!(outcome.text, "b");
        let error = outcome.error.unwrap();
        assert!(error.contains("Rule 2"));
        assert!(error.contains("not a valid regular expression"));
    }

    #[test]
    fn a_disabled_rule_does_not_run() {
        let mut off = rule("a", "b", false);
        off.enabled = false;
        let cleaner = ClipCleanerFile {
            enabled: true,
            rules: vec![off],
        };
        assert_eq!(apply_rules(&cleaner, "a", 100).text, "a");
    }

    #[cfg(windows)]
    #[test]
    fn a_toast_app_id_can_be_freed() {
        use windows::Win32::System::Com::StructuredStorage::PropVariantClear;
        let mut variant = super::app_id_variant("com.ce.desktop-tools").expect("app id");
        unsafe { PropVariantClear(&mut variant).expect("free") };
    }

    #[test]
    fn a_clean_notice_shows_the_change() {
        assert_eq!(clean_notice("a  b", "ab"), "a b → ab");
        assert_eq!(clean_notice("secret", ""), "The copy was cleared.");
        assert_eq!(clean_notice("same", "same"), "The copy was rewritten.");
    }

    #[test]
    fn groups_are_substituted() {
        let cleaner = ClipCleanerFile {
            enabled: true,
            rules: vec![rule(r"(\d+)", "n=$1", false)],
        };
        assert_eq!(apply_rules(&cleaner, "id 42", 100).text, "id n=42");
    }

    #[test]
    fn a_rule_that_grows_past_the_limit_is_skipped() {
        let cleaner = ClipCleanerFile {
            enabled: true,
            rules: vec![rule("a", "aaaa", false)],
        };
        let outcome = apply_rules(&cleaner, "a", 3);
        assert_eq!(outcome.text, "a");
        assert!(outcome.error.unwrap().contains("too long"));
    }

    #[test]
    fn normalize_fills_blank_ids_and_keeps_the_first_duplicate() {
        let mut cleaner = ClipCleanerFile {
            enabled: true,
            rules: vec![
                ClipRule {
                    id: " same ".into(),
                    ..rule("a", "b", false)
                },
                ClipRule {
                    id: "same".into(),
                    ..rule("c", "d", false)
                },
                ClipRule {
                    id: "  ".into(),
                    ..rule("e", "f", false)
                },
            ],
        };
        cleaner.normalize();
        assert_eq!(cleaner.rules[0].id, "same");
        assert_eq!(cleaner.rules[1].id, "rule-1");
        assert_eq!(cleaner.rules[2].id, "rule-2");
    }
}
