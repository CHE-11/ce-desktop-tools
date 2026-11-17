mod crypto;
mod hook;
mod matcher;
mod store;

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, SystemTime};

/// An unlock lasts this long. Hiding the window to the tray does not lock the vault or pause this.
const UNLOCK_LIMIT: Duration = Duration::from_secs(12 * 60 * 60);

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use hook::HookState;
pub use store::Snippet;

static QUITTING: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpansionState {
    pub settings_folder: String,
    pub snippets: Vec<Snippet>,
    pub enabled: bool,
    pub locked: bool,
    pub needs_pin: bool,
    pub error: Option<String>,
    /// Seconds until the vault locks itself. Absent while the vault is locked.
    pub lock_seconds: Option<u64>,
}

struct Inner {
    app: AppHandle,
    hook: Arc<HookState>,
    vault: Mutex<String>,
    snippets: RwLock<Vec<Snippet>>,
    last_body: Mutex<String>,
    error: Mutex<Option<String>>,
    reload_ticket: AtomicU64,
    watcher: Mutex<Option<RecommendedWatcher>>,
    pause_item: Mutex<Option<tauri::menu::MenuItem<tauri::Wry>>>,
    key: Mutex<Option<crypto::VaultKey>>,
    seal: Mutex<Option<serde_json::Value>>,
    locked: AtomicBool,
    needs_pin: AtomicBool,
    unlock_epoch: AtomicU64,
    unlock_deadline: Mutex<Option<SystemTime>>,
    failures: AtomicU32,
    misspells: RwLock<Vec<crate::misspell::Entry>>,
    misspell_error: Mutex<Option<String>>,
    dates: RwLock<Vec<crate::date_expand::Entry>>,
    date_prefix: Mutex<String>,
    date_error: Mutex<Option<String>>,
    ascii: RwLock<Vec<crate::ascii::Entry>>,
    ascii_prefix: Mutex<String>,
    ascii_error: Mutex<Option<String>>,
}

pub struct Expansion {
    inner: Arc<Inner>,
}

impl Clone for Expansion {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl Expansion {
    pub fn new(app: AppHandle) -> Self {
        let hook = Arc::new(HookState::new());
        hook.set_app(app.clone());
        hook::spawn(Arc::clone(&hook));
        Self {
            inner: Arc::new(Inner {
                app,
                hook,
                vault: Mutex::new(String::new()),
                snippets: RwLock::new(Vec::new()),
                last_body: Mutex::new(String::new()),
                error: Mutex::new(None),
                reload_ticket: AtomicU64::new(0),
                watcher: Mutex::new(None),
                pause_item: Mutex::new(None),
                key: Mutex::new(None),
                seal: Mutex::new(None),
                locked: AtomicBool::new(true),
                needs_pin: AtomicBool::new(false),
                unlock_epoch: AtomicU64::new(0),
                unlock_deadline: Mutex::new(None),
                failures: AtomicU32::new(0),
                misspells: RwLock::new(Vec::new()),
                misspell_error: Mutex::new(None),
                dates: RwLock::new(Vec::new()),
                date_prefix: Mutex::new(String::new()),
                date_error: Mutex::new(None),
                ascii: RwLock::new(Vec::new()),
                ascii_prefix: Mutex::new(":".into()),
                ascii_error: Mutex::new(None),
            }),
        }
    }

    pub fn load_from_settings(&self) -> ExpansionState {
        let folder = crate::app_settings::settings_folder(&self.inner.app).unwrap_or_default();
        let state = self.apply_vault(folder);
        self.reload_misspells();
        self.reload_dates();
        self.reload_ascii();
        state
    }

    pub fn misspell_state(&self) -> crate::misspell::MisspellState {
        self.inner.misspell_view()
    }

    pub fn save_misspells(
        &self,
        entries: Vec<crate::misspell::Entry>,
    ) -> Result<crate::misspell::MisspellState, String> {
        let folder = self.vault();
        let entries = crate::misspell::save(&folder, entries)?;
        if let Ok(mut slot) = self.inner.misspells.write() {
            *slot = entries;
        }
        if let Ok(mut slot) = self.inner.misspell_error.lock() {
            *slot = None;
        }
        self.inner.reapply_hook();
        let state = self.inner.misspell_view();
        let _ = self.inner.app.emit("misspell-changed", &state);
        Ok(state)
    }

    fn reload_misspells(&self) {
        let folder = self.vault();
        match crate::misspell::load(&folder) {
            Ok(entries) => {
                if let Ok(mut slot) = self.inner.misspells.write() {
                    *slot = entries;
                }
                if let Ok(mut slot) = self.inner.misspell_error.lock() {
                    *slot = None;
                }
            }
            Err(error) => {
                if let Ok(mut slot) = self.inner.misspell_error.lock() {
                    *slot = Some(error);
                }
            }
        }
        self.inner.reapply_hook();
        let _ = self
            .inner
            .app
            .emit("misspell-changed", self.inner.misspell_view());
    }

    pub fn date_state(&self) -> crate::date_expand::DateState {
        self.inner.date_view()
    }

    pub fn save_dates(
        &self,
        entries: Vec<crate::date_expand::Entry>,
        prefix: String,
    ) -> Result<crate::date_expand::DateState, String> {
        let folder = self.vault();
        let (entries, prefix) = crate::date_expand::save(&folder, entries, &prefix)?;
        if let Ok(mut slot) = self.inner.dates.write() {
            *slot = entries;
        }
        if let Ok(mut slot) = self.inner.date_prefix.lock() {
            *slot = prefix;
        }
        if let Ok(mut slot) = self.inner.date_error.lock() {
            *slot = None;
        }
        self.inner.reapply_hook();
        let state = self.inner.date_view();
        let _ = self.inner.app.emit("date-changed", &state);
        Ok(state)
    }

    fn reload_dates(&self) {
        let folder = self.vault();
        match crate::date_expand::load(&folder) {
            Ok((entries, prefix)) => {
                if let Ok(mut slot) = self.inner.dates.write() {
                    *slot = entries;
                }
                if let Ok(mut slot) = self.inner.date_prefix.lock() {
                    *slot = prefix;
                }
                if let Ok(mut slot) = self.inner.date_error.lock() {
                    *slot = None;
                }
            }
            Err(error) => {
                if let Ok(mut slot) = self.inner.date_error.lock() {
                    *slot = Some(error);
                }
            }
        }
        self.inner.reapply_hook();
        let _ = self.inner.app.emit("date-changed", self.inner.date_view());
    }

    pub fn ascii_state(&self) -> crate::ascii::AsciiState {
        self.inner.ascii_view()
    }

    pub fn save_ascii(
        &self,
        entries: Vec<crate::ascii::Entry>,
        prefix: String,
    ) -> Result<crate::ascii::AsciiState, String> {
        let folder = self.vault();
        let (entries, prefix) = crate::ascii::save(&folder, entries, &prefix)?;
        if let Ok(mut slot) = self.inner.ascii.write() {
            *slot = entries;
        }
        if let Ok(mut slot) = self.inner.ascii_prefix.lock() {
            *slot = prefix;
        }
        if let Ok(mut slot) = self.inner.ascii_error.lock() {
            *slot = None;
        }
        self.inner.reapply_hook();
        let state = self.inner.ascii_view();
        let _ = self.inner.app.emit("ascii-changed", &state);
        Ok(state)
    }

    fn reload_ascii(&self) {
        let folder = self.vault();
        match crate::ascii::load(&folder) {
            Ok((entries, prefix)) => {
                if let Ok(mut slot) = self.inner.ascii.write() {
                    *slot = entries;
                }
                if let Ok(mut slot) = self.inner.ascii_prefix.lock() {
                    *slot = prefix;
                }
                if let Ok(mut slot) = self.inner.ascii_error.lock() {
                    *slot = None;
                }
            }
            Err(error) => {
                if let Ok(mut slot) = self.inner.ascii_error.lock() {
                    *slot = Some(error);
                }
            }
        }
        self.inner.reapply_hook();
        let _ = self
            .inner
            .app
            .emit("ascii-changed", self.inner.ascii_view());
    }

    pub fn state(&self) -> ExpansionState {
        self.lock_if_expired();
        self.view()
    }

    pub fn save(&self, snippets: Vec<Snippet>) -> Result<ExpansionState, String> {
        let vault = self.vault();
        if vault.is_empty() {
            return Err(
                "Choose a settings folder. expansion.json is saved there with the settings file."
                    .into(),
            );
        }
        if !Path::new(&vault).is_dir() {
            return Err(format!("Folder does not exist: {vault}"));
        }
        let locked = self.inner.locked.load(Ordering::Relaxed);
        let needs_pin = self.inner.needs_pin.load(Ordering::Relaxed);
        let existing = self
            .inner
            .snippets
            .read()
            .map(|slot| slot.clone())
            .unwrap_or_default();
        let snippets = store::normalize_snippets(keep_hidden_text(&existing, snippets))?;
        if locked {
            self.save_locked(&vault, snippets, &existing, needs_pin)
        } else {
            self.save_unlocked(&vault, snippets)
        }
    }

    fn save_locked(
        &self,
        vault: &str,
        snippets: Vec<Snippet>,
        existing: &[Snippet],
        needs_pin: bool,
    ) -> Result<ExpansionState, String> {
        for snippet in &snippets {
            if !snippet.hidden {
                continue;
            }
            let known = existing
                .iter()
                .any(|item| item.id == snippet.id && item.hidden);
            if !known {
                return Err(if needs_pin {
                    "Create a PIN before hiding a snippet.".into()
                } else {
                    "Unlock the vault before hiding a snippet.".into()
                });
            }
        }
        let path = store::file_path(vault);
        if needs_pin {
            let raw = store::write_legacy(&path, &snippets)?;
            self.set_seal(None);
            self.remember(vault, raw, snippets, None, true, true);
            return Ok(self.view());
        }
        let seal = self.seal();
        if seal.is_none() {
            return Err("Unlock the vault before saving snippets.".into());
        }
        let public = store::strip_hidden_text(&snippets);
        let raw = store::write_split(&path, &public, seal.as_ref())?;
        self.remember(vault, raw, public, None, true, false);
        Ok(self.view())
    }

    fn save_unlocked(&self, vault: &str, snippets: Vec<Snippet>) -> Result<ExpansionState, String> {
        let key = self
            .inner
            .key
            .lock()
            .ok()
            .and_then(|slot| slot.clone())
            .ok_or_else(|| "Unlock the text expansion vault before saving snippets.".to_string())?;
        let sealed = crypto::seal_secrets_with_key(&store::hidden_secrets(&snippets), &key)?;
        let seal = seal_value(&sealed)?;
        let raw = store::write_split(&store::file_path(vault), &snippets, Some(&seal))?;
        self.set_seal(Some(seal));
        self.remember(vault, raw, snippets, None, false, false);
        Ok(self.view())
    }

    pub fn create_pin(&self, pin: String) -> Result<ExpansionState, String> {
        crypto::check_pin(&pin)?;
        if !self.inner.needs_pin.load(Ordering::Relaxed) {
            return Err("This vault already has a PIN. Unlock it instead.".into());
        }
        let vault = self.vault();
        if vault.is_empty() || !Path::new(&vault).is_dir() {
            return Err("Choose a settings folder before creating a PIN.".into());
        }
        let snippets = self
            .inner
            .snippets
            .read()
            .map(|slot| slot.clone())
            .unwrap_or_default();
        let snippets = store::normalize_snippets(snippets)?;
        let (sealed, key) = crypto::seal_secrets(&store::hidden_secrets(&snippets), &pin)?;
        let seal = seal_value(&sealed)?;
        let raw = store::write_split(&store::file_path(&vault), &snippets, Some(&seal))?;
        self.set_key(Some(key));
        self.set_seal(Some(seal));
        self.inner.failures.store(0, Ordering::Relaxed);
        self.remember(&vault, raw, snippets, None, false, false);
        Ok(self.view())
    }

    pub fn unlock(&self, pin: String) -> Result<ExpansionState, String> {
        crypto::check_pin(&pin)?;
        if self.inner.needs_pin.load(Ordering::Relaxed) {
            return Err("Create a PIN first.".into());
        }
        let failures = self.inner.failures.load(Ordering::Relaxed);
        if failures > 0 {
            std::thread::sleep(Duration::from_millis(
                (failures as u64).saturating_mul(300).min(3000),
            ));
        }
        let vault = self.vault();
        let path = store::file_path(&vault);
        let raw = std::fs::read_to_string(&path)
            .map_err(|error| format!("Couldn't read {}: {error}", path.display()))?;
        let disk = match store::classify(&raw, &path) {
            Ok(disk) => disk,
            Err(error) => {
                self.inner.failures.fetch_add(1, Ordering::Relaxed);
                return Err(error);
            }
        };
        let opened = match disk {
            store::DiskVault::Sealed => {
                crypto::open(&raw, &pin).map(|(file, key)| (file.snippets, key, true))
            }
            store::DiskVault::Split {
                snippets,
                seal: Some(seal),
            } => crypto::open_secrets(&seal.to_string(), &pin)
                .map(|(secrets, key)| (store::merge_secrets(snippets, &secrets), key, false)),
            _ => Err("Create a PIN first.".into()),
        };
        match opened {
            Ok((snippets, key, migrate)) => {
                self.inner.failures.store(0, Ordering::Relaxed);
                if migrate {
                    let sealed =
                        crypto::seal_secrets_with_key(&store::hidden_secrets(&snippets), &key)?;
                    let seal = seal_value(&sealed)?;
                    let body = store::write_split(&path, &snippets, Some(&seal))?;
                    self.set_key(Some(key));
                    self.set_seal(Some(seal));
                    self.remember(&vault, body, snippets, None, false, false);
                } else {
                    self.set_key(Some(key));
                    self.remember(&vault, raw, snippets, None, false, false);
                }
                Ok(self.view())
            }
            Err(error) => {
                self.inner.failures.fetch_add(1, Ordering::Relaxed);
                Err(error)
            }
        }
    }

    pub fn change_pin(&self, current: String, next: String) -> Result<ExpansionState, String> {
        crypto::check_pin(&current)?;
        crypto::check_pin(&next)?;
        if current == next {
            return Err("Choose a different PIN.".into());
        }
        if self.inner.needs_pin.load(Ordering::Relaxed) {
            return Err("Create a PIN in Text Expansion first.".into());
        }
        let failures = self.inner.failures.load(Ordering::Relaxed);
        if failures > 0 {
            std::thread::sleep(Duration::from_millis(
                (failures as u64).saturating_mul(300).min(3000),
            ));
        }
        let vault = self.vault();
        if vault.is_empty() || !Path::new(&vault).is_dir() {
            return Err("Choose a settings folder before changing the PIN.".into());
        }
        let locked = self.inner.locked.load(Ordering::Relaxed);
        let path = store::file_path(&vault);
        let raw = std::fs::read_to_string(&path)
            .map_err(|error| format!("Couldn't read {}: {error}", path.display()))?;
        let (mut snippets, old_key) = match open_with_pin(&raw, &path, &current) {
            Ok(opened) => opened,
            Err(error) => {
                self.inner.failures.fetch_add(1, Ordering::Relaxed);
                return Err(error);
            }
        };
        if !locked {
            if let Ok(slot) = self.inner.snippets.read() {
                if !slot.is_empty() {
                    snippets = slot.clone();
                }
            }
        }
        let (sealed, new_key) = crypto::seal_secrets(&store::hidden_secrets(&snippets), &next)?;
        let seal = seal_value(&sealed)?;
        let backup =
            if let Some(clipboard) = self.inner.app.try_state::<crate::clipboard::Clipboard>() {
                clipboard.reencrypt(&vault, &old_key, &new_key)?
            } else {
                None
            };
        let body = match store::write_split(&path, &snippets, Some(&seal)) {
            Ok(body) => body,
            Err(error) => {
                if let Some(bytes) = backup {
                    if let Some(clipboard) =
                        self.inner.app.try_state::<crate::clipboard::Clipboard>()
                    {
                        let _ = clipboard.restore_encrypted(&vault, &bytes);
                    }
                }
                return Err(error);
            }
        };
        self.inner.failures.store(0, Ordering::Relaxed);
        self.set_seal(Some(seal));
        if locked {
            self.clear_key();
            let public = store::strip_hidden_text(&snippets);
            self.remember(&vault, body, public, None, true, false);
        } else {
            self.set_key(Some(new_key));
            self.remember(&vault, body, snippets, None, false, false);
        }
        Ok(self.view())
    }

    pub fn lock(&self) -> ExpansionState {
        let needs_pin = self.inner.needs_pin.load(Ordering::Relaxed);
        let public = self
            .inner
            .snippets
            .read()
            .map(|slot| store::strip_hidden_text(&slot))
            .unwrap_or_default();
        self.clear_key();
        let raw = self
            .inner
            .last_body
            .lock()
            .map(|slot| slot.clone())
            .unwrap_or_default();
        let vault = self.vault();
        self.remember(&vault, raw, public, None, true, needs_pin);
        self.view()
    }

    pub fn set_enabled(&self, enabled: bool) -> bool {
        self.inner.hook.enabled.store(enabled, Ordering::Relaxed);
        if !enabled {
            if let Ok(mut matcher) = self.inner.hook.matcher.lock() {
                matcher.clear();
            }
        }
        self.sync_pause_label();
        let _ = self.inner.app.emit("expansion-enabled", enabled);
        enabled
    }

    pub fn toggle_enabled(&self) -> bool {
        let next = !self.inner.hook.enabled.load(Ordering::Relaxed);
        self.set_enabled(next)
    }

    pub fn shutdown(&self) {
        let _ = self.lock();
        hook::stop(&self.inner.hook);
    }

    pub fn set_pause_item(&self, item: tauri::menu::MenuItem<tauri::Wry>) {
        if let Ok(mut slot) = self.inner.pause_item.lock() {
            *slot = Some(item);
        }
        self.sync_pause_label();
    }

    fn apply_vault(&self, vault: String) -> ExpansionState {
        self.unwatch();
        self.clear_key();
        if vault.is_empty() || !Path::new(&vault).is_dir() {
            let error = if vault.is_empty() {
                None
            } else {
                Some(format!("Folder does not exist: {vault}"))
            };
            self.set_seal(None);
            self.remember(&vault, String::new(), Vec::new(), error, true, false);
            return self.view();
        }
        match store::read_disk(&store::file_path(&vault)) {
            Ok(None) => {
                self.set_seal(None);
                self.remember(&vault, String::new(), Vec::new(), None, true, true);
            }
            Ok(Some((raw, store::DiskVault::Legacy(file)))) => {
                self.set_seal(None);
                self.remember(&vault, raw, file.snippets, None, true, true);
            }
            Ok(Some((raw, store::DiskVault::Sealed))) => {
                self.set_seal(None);
                self.remember(&vault, raw, Vec::new(), None, true, false);
            }
            Ok(Some((raw, store::DiskVault::Split { snippets, seal }))) => {
                let needs_pin = seal.is_none();
                self.set_seal(seal);
                self.remember(&vault, raw, snippets, None, true, needs_pin);
            }
            Err(error) => {
                self.set_seal(None);
                self.remember(&vault, String::new(), Vec::new(), Some(error), true, false);
            }
        }
        self.watch(&vault);
        self.view()
    }

    fn remember(
        &self,
        vault: &str,
        raw: String,
        snippets: Vec<Snippet>,
        error: Option<String>,
        locked: bool,
        needs_pin: bool,
    ) {
        if let Ok(mut slot) = self.inner.vault.lock() {
            *slot = vault.to_string();
        }
        if let Ok(mut slot) = self.inner.last_body.lock() {
            *slot = raw;
        }
        if let Ok(mut slot) = self.inner.error.lock() {
            *slot = error.clone();
        }
        let was_locked = self.inner.locked.load(Ordering::Relaxed);
        self.inner.locked.store(locked, Ordering::Relaxed);
        self.inner.needs_pin.store(needs_pin, Ordering::Relaxed);
        self.inner.hook.locked.store(locked, Ordering::Relaxed);
        if locked {
            self.inner.disarm_unlock_timer();
        } else if was_locked {
            self.arm_unlock_timer();
        }
        self.inner.apply_hook(hook_snippets(&snippets, locked));
        if let Ok(mut slot) = self.inner.snippets.write() {
            *slot = snippets;
        }
        self.sync_clipboard();
        let _ = self.inner.app.emit("expansion-changed", self.view());
    }

    fn set_key(&self, key: Option<crypto::VaultKey>) {
        if let Ok(mut slot) = self.inner.key.lock() {
            *slot = key;
        }
    }

    fn clear_key(&self) {
        self.set_key(None);
    }

    fn set_seal(&self, seal: Option<serde_json::Value>) {
        if let Ok(mut slot) = self.inner.seal.lock() {
            *slot = seal;
        }
    }

    fn seal(&self) -> Option<serde_json::Value> {
        self.inner.seal.lock().ok().and_then(|slot| slot.clone())
    }

    fn sync_clipboard(&self) {
        let Some(clipboard) = self.inner.app.try_state::<crate::clipboard::Clipboard>() else {
            return;
        };
        if self.inner.locked.load(Ordering::Relaxed) {
            clipboard.on_locked();
            return;
        }
        let key = self.inner.key.lock().ok().and_then(|slot| slot.clone());
        let folder = self.vault();
        if let Some(key) = key {
            clipboard.on_unlocked(key, folder);
        }
    }

    fn watch(&self, vault: &str) {
        let inner = Arc::clone(&self.inner);
        let mut watcher =
            match notify::recommended_watcher(move |result: Result<Event, notify::Error>| {
                if result.is_ok() {
                    inner.schedule_reload();
                }
            }) {
                Ok(watcher) => watcher,
                Err(error) => {
                    eprintln!("ce-desktop-tools could not watch the vault: {error}");
                    return;
                }
            };
        if let Err(error) = watcher.watch(Path::new(vault), RecursiveMode::NonRecursive) {
            eprintln!("ce-desktop-tools could not watch the vault: {error}");
            return;
        }
        if let Ok(mut slot) = self.inner.watcher.lock() {
            *slot = Some(watcher);
        }
    }

    fn unwatch(&self) {
        if let Ok(mut slot) = self.inner.watcher.lock() {
            *slot = None;
        }
    }

    fn view(&self) -> ExpansionState {
        self.ensure_locked_public();
        ExpansionState {
            settings_folder: self.vault(),
            snippets: self
                .inner
                .snippets
                .read()
                .map(|slot| redact_snippets(slot.clone()))
                .unwrap_or_default(),
            enabled: self.inner.hook.enabled.load(Ordering::Relaxed),
            locked: self.inner.locked.load(Ordering::Relaxed),
            needs_pin: self.inner.needs_pin.load(Ordering::Relaxed),
            error: self.inner.error.lock().ok().and_then(|slot| slot.clone()),
            lock_seconds: self.inner.lock_seconds(),
        }
    }

    fn lock_if_expired(&self) {
        if self.inner.lock_expired() {
            let _ = self.lock();
        }
    }

    fn arm_unlock_timer(&self) {
        let epoch = self.inner.unlock_epoch.fetch_add(1, Ordering::AcqRel) + 1;
        let deadline = SystemTime::now() + UNLOCK_LIMIT;
        if let Ok(mut slot) = self.inner.unlock_deadline.lock() {
            *slot = Some(deadline);
        }
        let expansion = self.clone();
        std::thread::spawn(move || {
            loop {
                if expansion.inner.unlock_epoch.load(Ordering::Acquire) != epoch {
                    return;
                }
                let Some(deadline) = expansion
                    .inner
                    .unlock_deadline
                    .lock()
                    .ok()
                    .and_then(|slot| *slot)
                else {
                    return;
                };
                let now = SystemTime::now();
                if now >= deadline {
                    if expansion.inner.unlock_epoch.load(Ordering::Acquire) == epoch {
                        let _ = expansion.lock();
                    }
                    return;
                }
                let remaining = deadline.duration_since(now).unwrap_or(Duration::ZERO);
                std::thread::sleep(remaining.min(Duration::from_secs(30)));
            }
        });
    }

    fn vault(&self) -> String {
        self.inner
            .vault
            .lock()
            .map(|slot| slot.clone())
            .unwrap_or_default()
    }

    /// A locked vault still keeps ordinary snippets in expansion.json. If memory was cleared, load those back so the list and the hook stay usable.
    fn ensure_locked_public(&self) {
        if !self.inner.locked.load(Ordering::Relaxed) {
            return;
        }
        let empty = self
            .inner
            .snippets
            .read()
            .map(|slot| slot.is_empty())
            .unwrap_or(true);
        if !empty {
            return;
        }
        let public = public_snippets_on_disk(&self.vault());
        if public.is_empty() {
            return;
        }
        self.inner.apply_hook(hook_snippets(&public, true));
        if let Ok(mut slot) = self.inner.snippets.write() {
            *slot = public;
        }
    }

    fn sync_pause_label(&self) {
        let paused = !self.inner.hook.enabled.load(Ordering::Relaxed);
        let label = if paused {
            "Resume expansion"
        } else {
            "Pause expansion"
        };
        if let Ok(slot) = self.inner.pause_item.lock() {
            if let Some(item) = slot.as_ref() {
                let _ = item.set_text(label);
            }
        }
    }
}

impl Inner {
    fn misspell_view(&self) -> crate::misspell::MisspellState {
        crate::misspell::MisspellState {
            settings_folder: self
                .vault
                .lock()
                .map(|slot| slot.clone())
                .unwrap_or_default(),
            entries: self
                .misspells
                .read()
                .map(|slot| slot.clone())
                .unwrap_or_default(),
            error: self
                .misspell_error
                .lock()
                .ok()
                .and_then(|slot| slot.clone()),
        }
    }

    fn date_view(&self) -> crate::date_expand::DateState {
        let entries = self
            .dates
            .read()
            .map(|slot| slot.clone())
            .unwrap_or_default();
        crate::date_expand::DateState {
            settings_folder: self
                .vault
                .lock()
                .map(|slot| slot.clone())
                .unwrap_or_default(),
            prefix: self
                .date_prefix
                .lock()
                .map(|slot| slot.clone())
                .unwrap_or_default(),
            entries: crate::date_expand::present(&entries),
            error: self.date_error.lock().ok().and_then(|slot| slot.clone()),
        }
    }

    fn ascii_view(&self) -> crate::ascii::AsciiState {
        crate::ascii::AsciiState {
            settings_folder: self
                .vault
                .lock()
                .map(|slot| slot.clone())
                .unwrap_or_default(),
            prefix: self
                .ascii_prefix
                .lock()
                .map(|slot| slot.clone())
                .unwrap_or_else(|_| ":".into()),
            entries: self
                .ascii
                .read()
                .map(|slot| slot.clone())
                .unwrap_or_default(),
            error: self.ascii_error.lock().ok().and_then(|slot| slot.clone()),
        }
    }

    fn reapply_hook(&self) {
        let current = self
            .hook
            .snippets
            .read()
            .map(|slot| slot.clone())
            .unwrap_or_default();
        let vault = current
            .into_iter()
            .filter(|snippet| !snippet.ending && !snippet.timed && !snippet.symbol)
            .collect();
        self.apply_hook(vault);
    }

    fn apply_hook(&self, mut snippets: Vec<matcher::Snippet>) {
        snippets.retain(|snippet| !snippet.ending && !snippet.timed && !snippet.symbol);
        if let Ok(entries) = self.misspells.read() {
            snippets.extend(entries.iter().map(|entry| matcher::Snippet {
                trigger: entry.trigger.clone(),
                text: entry.text.clone(),
                enabled: entry.enabled,
                ending: true,
                timed: false,
                ignore_case: false,
                symbol: false,
                secret: false,
            }));
        }
        if let Ok(entries) = self.dates.read() {
            let prefix = self
                .date_prefix
                .lock()
                .map(|slot| slot.clone())
                .unwrap_or_default();
            snippets.extend(entries.iter().map(|entry| matcher::Snippet {
                trigger: format!("{prefix}{}", entry.trigger),
                text: entry.template.clone(),
                enabled: entry.enabled,
                ending: false,
                timed: true,
                ignore_case: true,
                symbol: false,
                secret: false,
            }));
        }
        if let Ok(entries) = self.ascii.read() {
            let prefix = self
                .ascii_prefix
                .lock()
                .map(|slot| slot.clone())
                .unwrap_or_default();
            snippets.extend(entries.iter().map(|entry| matcher::Snippet {
                trigger: format!("{prefix}{}", entry.trigger),
                text: entry.text.clone(),
                enabled: entry.enabled,
                ending: false,
                timed: false,
                ignore_case: true,
                symbol: true,
                secret: false,
            }));
        }
        self.hook.replace_snippets(snippets);
    }

    fn schedule_reload(self: &Arc<Self>) {
        let ticket = self.reload_ticket.fetch_add(1, Ordering::Relaxed) + 1;
        let inner = Arc::clone(self);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(200));
            if inner.reload_ticket.load(Ordering::Relaxed) != ticket {
                return;
            }
            inner.reload_from_disk();
        });
    }

    fn migrate_sealed(
        &self,
        path: &Path,
        snippets: &[Snippet],
        key: &crypto::VaultKey,
    ) -> Result<(), String> {
        let sealed = crypto::seal_secrets_with_key(&store::hidden_secrets(snippets), key)?;
        let seal = seal_value(&sealed)?;
        let body = store::write_split(path, snippets, Some(&seal))?;
        if let Ok(mut slot) = self.seal.lock() {
            *slot = Some(seal);
        }
        if let Ok(mut slot) = self.last_body.lock() {
            *slot = body;
        }
        Ok(())
    }

    fn publish_snippets(&self, snippets: Vec<Snippet>, locked: bool, needs_pin: bool) {
        if let Ok(mut slot) = self.snippets.write() {
            *slot = snippets.clone();
        }
        if locked {
            self.disarm_unlock_timer();
        }
        self.locked.store(locked, Ordering::Relaxed);
        self.needs_pin.store(needs_pin, Ordering::Relaxed);
        self.hook.locked.store(locked, Ordering::Relaxed);
        self.apply_hook(hook_snippets(&snippets, locked));
        let _ = self
            .app
            .emit("expansion-changed", self.snapshot(snippets, None));
    }

    fn publish_locked_file(
        &self,
        raw: String,
        snippets: Vec<Snippet>,
        seal: Option<serde_json::Value>,
        needs_pin: bool,
    ) {
        if let Ok(mut slot) = self.seal.lock() {
            *slot = seal;
        }
        if let Ok(mut slot) = self.last_body.lock() {
            *slot = raw;
        }
        if let Ok(mut slot) = self.key.lock() {
            *slot = None;
        }
        self.publish_snippets(snippets, true, needs_pin);
    }

    fn reload_from_disk(&self) {
        let vault = self
            .vault
            .lock()
            .map(|slot| slot.clone())
            .unwrap_or_default();
        if vault.is_empty() {
            return;
        }
        let path = store::file_path(&vault);
        if !path.is_file() {
            return;
        }
        match store::read_disk(&path) {
            Ok(Some((raw, disk))) => {
                if self
                    .last_body
                    .lock()
                    .map(|slot| *slot == raw)
                    .unwrap_or(false)
                {
                    return;
                }
                match disk {
                    store::DiskVault::Sealed => {
                        let key = self.key.lock().ok().and_then(|slot| slot.clone());
                        if let Some(key) = key {
                            match crypto::open_with_key(&raw, &key) {
                                Ok(file) => {
                                    if let Err(error) =
                                        self.migrate_sealed(&path, &file.snippets, &key)
                                    {
                                        self.lock_after_reload(vault, error);
                                        return;
                                    }
                                    self.publish_snippets(file.snippets, false, false);
                                }
                                Err(_) => {
                                    self.lock_after_reload(
                                        vault,
                                        "The vault file changed and could not be unlocked.".into(),
                                    );
                                }
                            }
                        } else {
                            self.publish_locked_file(raw, Vec::new(), None, false);
                        }
                    }
                    store::DiskVault::Legacy(file) => {
                        self.publish_locked_file(raw, file.snippets, None, true);
                    }
                    store::DiskVault::Split { snippets, seal } => {
                        let key = self.key.lock().ok().and_then(|slot| slot.clone());
                        if let Some(key) = key {
                            let Some(seal_value) = seal else {
                                self.publish_locked_file(raw, snippets, None, true);
                                return;
                            };
                            match crypto::open_secrets_with_key(&seal_value.to_string(), &key) {
                                Ok(secrets) => {
                                    if let Ok(mut slot) = self.seal.lock() {
                                        *slot = Some(seal_value);
                                    }
                                    if let Ok(mut slot) = self.last_body.lock() {
                                        *slot = raw;
                                    }
                                    let snippets = store::merge_secrets(snippets, &secrets);
                                    self.publish_snippets(snippets, false, false);
                                }
                                Err(_) => {
                                    self.lock_after_reload(
                                        vault,
                                        "The vault file changed and could not be unlocked.".into(),
                                    );
                                }
                            }
                        } else {
                            let needs_pin = seal.is_none();
                            self.publish_locked_file(raw, snippets, seal, needs_pin);
                        }
                    }
                }
            }
            Ok(None) => {}
            Err(error) => self.lock_after_reload(vault, error),
        }
    }

    fn lock_after_reload(&self, vault: String, error: String) {
        if let Ok(mut slot) = self.error.lock() {
            *slot = Some(error.clone());
        }
        let public = public_snippets_on_disk(&vault);
        if let Ok(mut slot) = self.snippets.write() {
            *slot = public.clone();
        }
        if let Ok(mut slot) = self.key.lock() {
            *slot = None;
        }
        if let Ok(mut slot) = self.seal.lock() {
            *slot = None;
        }
        self.disarm_unlock_timer();
        self.locked.store(true, Ordering::Relaxed);
        self.hook.locked.store(true, Ordering::Relaxed);
        self.apply_hook(hook_snippets(&public, true));
        let _ = self.app.emit(
            "expansion-changed",
            ExpansionState {
                settings_folder: vault,
                snippets: redact_snippets(public),
                enabled: self.hook.enabled.load(Ordering::Relaxed),
                locked: true,
                needs_pin: self.needs_pin.load(Ordering::Relaxed),
                error: Some(error),
                lock_seconds: None,
            },
        );
    }

    fn disarm_unlock_timer(&self) {
        self.unlock_epoch.fetch_add(1, Ordering::AcqRel);
        if let Ok(mut slot) = self.unlock_deadline.lock() {
            *slot = None;
        }
    }

    fn lock_expired(&self) -> bool {
        if self.locked.load(Ordering::Relaxed) {
            return false;
        }
        self.unlock_deadline
            .lock()
            .ok()
            .and_then(|slot| *slot)
            .is_some_and(|deadline| SystemTime::now() >= deadline)
    }

    fn lock_seconds(&self) -> Option<u64> {
        remaining_lock_seconds(
            self.locked.load(Ordering::Relaxed),
            self.unlock_deadline.lock().ok().and_then(|slot| *slot),
            SystemTime::now(),
        )
    }

    fn snapshot(&self, snippets: Vec<Snippet>, error: Option<String>) -> ExpansionState {
        ExpansionState {
            settings_folder: self
                .vault
                .lock()
                .map(|slot| slot.clone())
                .unwrap_or_default(),
            snippets: redact_snippets(snippets),
            enabled: self.hook.enabled.load(Ordering::Relaxed),
            locked: self.locked.load(Ordering::Relaxed),
            needs_pin: self.needs_pin.load(Ordering::Relaxed),
            error,
            lock_seconds: self.lock_seconds(),
        }
    }
}

fn remaining_lock_seconds(locked: bool, deadline: Option<SystemTime>, now: SystemTime) -> Option<u64> {
    if locked {
        return None;
    }
    let deadline = deadline?;
    match deadline.duration_since(now) {
        Ok(remaining) => Some(remaining.as_secs()),
        Err(_) => Some(0),
    }
}

pub fn request_quit() {
    QUITTING.store(true, Ordering::SeqCst);
}

pub fn is_quitting() -> bool {
    QUITTING.load(Ordering::SeqCst)
}

fn keep_hidden_text(existing: &[Snippet], incoming: Vec<Snippet>) -> Vec<Snippet> {
    incoming
        .into_iter()
        .map(|mut snippet| {
            if let Some(previous) = existing
                .iter()
                .find(|item| !item.id.is_empty() && item.id == snippet.id)
            {
                if previous.hidden {
                    snippet.text = previous.text.clone();
                    snippet.hidden = true;
                }
            }
            snippet
        })
        .collect()
}

fn redact_snippets(snippets: Vec<Snippet>) -> Vec<Snippet> {
    snippets
        .into_iter()
        .map(|mut snippet| {
            if snippet.hidden {
                snippet.text.clear();
            }
            snippet
        })
        .collect()
}

fn hook_snippets(snippets: &[Snippet], locked: bool) -> Vec<matcher::Snippet> {
    snippets
        .iter()
        .map(|snippet| {
            let secret = locked && snippet.hidden;
            matcher::Snippet {
                trigger: snippet.trigger.clone(),
                text: if secret {
                    String::new()
                } else {
                    snippet.text.clone()
                },
                enabled: snippet.enabled,
                ending: false,
                timed: false,
                ignore_case: false,
                symbol: false,
                secret,
            }
        })
        .collect()
}

fn public_snippets_on_disk(vault: &str) -> Vec<Snippet> {
    if vault.is_empty() {
        return Vec::new();
    }
    match store::read_disk(&store::file_path(vault)) {
        Ok(Some((_, store::DiskVault::Split { snippets, .. }))) => snippets,
        Ok(Some((_, store::DiskVault::Legacy(file)))) => store::strip_hidden_text(&file.snippets),
        _ => Vec::new(),
    }
}

fn seal_value(raw: &str) -> Result<serde_json::Value, String> {
    serde_json::from_str(raw).map_err(|_| "Couldn't encrypt hidden snippets.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unlock_lasts_twelve_hours() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        let deadline = now + UNLOCK_LIMIT;
        assert_eq!(
            remaining_lock_seconds(false, Some(deadline), now),
            Some(12 * 60 * 60)
        );
    }

    #[test]
    fn locked_vault_has_no_countdown() {
        let now = SystemTime::UNIX_EPOCH;
        assert_eq!(remaining_lock_seconds(true, Some(now), now), None);
        assert_eq!(remaining_lock_seconds(false, None, now), None);
    }

    #[test]
    fn expired_unlock_reports_zero() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(50);
        let deadline = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
        assert_eq!(remaining_lock_seconds(false, Some(deadline), now), Some(0));
    }
}

fn open_with_pin(
    raw: &str,
    path: &Path,
    pin: &str,
) -> Result<(Vec<Snippet>, crypto::VaultKey), String> {
    match store::classify(raw, path)? {
        store::DiskVault::Sealed => {
            let (file, key) = crypto::open(raw, pin)?;
            Ok((file.snippets, key))
        }
        store::DiskVault::Split {
            snippets,
            seal: Some(seal),
        } => {
            let (secrets, key) = crypto::open_secrets(&seal.to_string(), pin)?;
            Ok((store::merge_secrets(snippets, &secrets), key))
        }
        _ => Err("Create a PIN first.".into()),
    }
}
