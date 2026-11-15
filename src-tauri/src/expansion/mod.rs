mod crypto;
mod hook;
mod matcher;
mod store;

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

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
    pending_legacy: Mutex<Option<Vec<Snippet>>>,
    locked: AtomicBool,
    needs_pin: AtomicBool,
    failures: AtomicU32,
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
                pending_legacy: Mutex::new(None),
                locked: AtomicBool::new(true),
                needs_pin: AtomicBool::new(false),
                failures: AtomicU32::new(0),
            }),
        }
    }

    pub fn load_from_settings(&self) -> ExpansionState {
        let folder = crate::app_settings::settings_folder(&self.inner.app).unwrap_or_default();
        self.apply_vault(folder)
    }

    pub fn state(&self) -> ExpansionState {
        self.view()
    }

    pub fn save(&self, snippets: Vec<Snippet>) -> Result<ExpansionState, String> {
        if self.inner.locked.load(Ordering::Relaxed) {
            return Err("Unlock the text expansion vault before saving snippets.".into());
        }
        let vault = self.vault();
        if vault.is_empty() {
            return Err("Choose a settings folder. expansion.json is saved there with the settings file.".into());
        }
        if !Path::new(&vault).is_dir() {
            return Err(format!("Folder does not exist: {vault}"));
        }
        let snippets = store::normalize_snippets(snippets)?;
        let key = self.inner.key.lock().ok().and_then(|slot| slot.clone()).ok_or_else(|| "Unlock the text expansion vault before saving snippets.".to_string())?;
        let raw = crypto::seal_with_key(&snippets, &key)?;
        store::write_raw(&store::file_path(&vault), &raw)?;
        self.remember(&vault, raw, snippets, None, false, false);
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
        let snippets = match self.inner.pending_legacy.lock() {
            Ok(mut slot) => slot.take().unwrap_or_default(),
            Err(_) => Vec::new(),
        };
        let snippets = store::normalize_snippets(snippets)?;
        let (raw, key) = crypto::seal(&snippets, &pin)?;
        store::write_raw(&store::file_path(&vault), &raw)?;
        if let Ok(mut slot) = self.inner.key.lock() {
            *slot = Some(key);
        }
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
            std::thread::sleep(Duration::from_millis((failures as u64).saturating_mul(300).min(3000)));
        }
        let vault = self.vault();
        let path = store::file_path(&vault);
        let raw = std::fs::read_to_string(&path).map_err(|error| format!("Couldn't read {}: {error}", path.display()))?;
        match crypto::open(&raw, &pin) {
            Ok((file, key)) => {
                self.inner.failures.store(0, Ordering::Relaxed);
                if let Ok(mut slot) = self.inner.key.lock() {
                    *slot = Some(key);
                }
                self.remember(&vault, raw, file.snippets, None, false, false);
                Ok(self.view())
            }
            Err(error) => {
                self.inner.failures.fetch_add(1, Ordering::Relaxed);
                Err(error)
            }
        }
    }

    pub fn lock(&self) -> ExpansionState {
        let triggers = self
            .inner
            .snippets
            .read()
            .map(|slot| trigger_only(&slot))
            .unwrap_or_default();
        if let Ok(mut slot) = self.inner.key.lock() {
            *slot = None;
        }
        if let Ok(mut slot) = self.inner.snippets.write() {
            slot.clear();
        }
        self.inner.hook.replace_snippets(triggers);
        self.inner.locked.store(true, Ordering::Relaxed);
        self.inner.hook.locked.store(true, Ordering::Relaxed);
        let _ = self.inner.app.emit("expansion-changed", self.view());
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
        if vault.is_empty() || !Path::new(&vault).is_dir() {
            let error = if vault.is_empty() {
                None
            } else {
                Some(format!("Folder does not exist: {vault}"))
            };
            self.arm_lock(&vault, String::new(), Vec::new(), error, false);
            return self.view();
        }
        match store::read_disk(&store::file_path(&vault)) {
            Ok(None) => {
                self.arm_lock(&vault, String::new(), Vec::new(), None, true);
            }
            Ok(Some((raw, store::DiskVault::Legacy(file)))) => {
                if let Ok(mut slot) = self.inner.pending_legacy.lock() {
                    *slot = Some(file.snippets.clone());
                }
                self.arm_lock(&vault, raw, trigger_only(&file.snippets), None, true);
            }
            Ok(Some((raw, store::DiskVault::Sealed))) => {
                if let Ok(mut slot) = self.inner.pending_legacy.lock() {
                    *slot = None;
                }
                self.arm_lock(&vault, raw, Vec::new(), None, false);
            }
            Err(error) => {
                self.arm_lock(&vault, String::new(), Vec::new(), Some(error), false);
            }
        }
        self.watch(&vault);
        self.view()
    }

    fn arm_lock(&self, vault: &str, raw: String, triggers: Vec<matcher::Snippet>, error: Option<String>, needs_pin: bool) {
        if let Ok(mut slot) = self.inner.key.lock() {
            *slot = None;
        }
        self.inner.locked.store(true, Ordering::Relaxed);
        self.inner.needs_pin.store(needs_pin, Ordering::Relaxed);
        self.inner.hook.locked.store(true, Ordering::Relaxed);
        self.remember(vault, raw, Vec::new(), error, true, needs_pin);
        self.inner.hook.replace_snippets(triggers);
    }

    fn remember(&self, vault: &str, raw: String, snippets: Vec<Snippet>, error: Option<String>, locked: bool, needs_pin: bool) {
        if let Ok(mut slot) = self.inner.vault.lock() {
            *slot = vault.to_string();
        }
        if let Ok(mut slot) = self.inner.last_body.lock() {
            *slot = raw;
        }
        if let Ok(mut slot) = self.inner.error.lock() {
            *slot = error.clone();
        }
        self.inner.locked.store(locked, Ordering::Relaxed);
        self.inner.needs_pin.store(needs_pin, Ordering::Relaxed);
        self.inner.hook.locked.store(locked, Ordering::Relaxed);
        let visible = if locked || error.is_some() { Vec::new() } else { snippets.clone() };
        if let Ok(mut slot) = self.inner.snippets.write() {
            *slot = visible;
        }
        if !locked {
            self.inner.hook.replace_snippets(hook_snippets(&snippets));
        }
        let _ = self.inner.app.emit("expansion-changed", self.view());
    }

    fn watch(&self, vault: &str) {
        let inner = Arc::clone(&self.inner);
        let mut watcher = match notify::recommended_watcher(move |result: Result<Event, notify::Error>| {
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
        ExpansionState {
            settings_folder: self.vault(),
            snippets: {
                let locked = self.inner.locked.load(Ordering::Relaxed);
                if locked {
                    Vec::new()
                } else {
                    self.inner.snippets.read().map(|slot| slot.clone()).unwrap_or_default()
                }
            },
            enabled: self.inner.hook.enabled.load(Ordering::Relaxed),
            locked: self.inner.locked.load(Ordering::Relaxed),
            needs_pin: self.inner.needs_pin.load(Ordering::Relaxed),
            error: self.inner.error.lock().ok().and_then(|slot| slot.clone()),
        }
    }

    fn vault(&self) -> String {
        self.inner.vault.lock().map(|slot| slot.clone()).unwrap_or_default()
    }

    fn sync_pause_label(&self) {
        let paused = !self.inner.hook.enabled.load(Ordering::Relaxed);
        let label = if paused { "Resume expansion" } else { "Pause expansion" };
        if let Ok(slot) = self.inner.pause_item.lock() {
            if let Some(item) = slot.as_ref() {
                let _ = item.set_text(label);
            }
        }
    }
}

impl Inner {
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

    fn reload_from_disk(&self) {
        let vault = self.vault.lock().map(|slot| slot.clone()).unwrap_or_default();
        if vault.is_empty() {
            return;
        }
        let path = store::file_path(&vault);
        if !path.is_file() {
            return;
        }
        match store::read_disk(&path) {
            Ok(Some((raw, disk))) => {
                if self.last_body.lock().map(|slot| *slot == raw).unwrap_or(false) {
                    return;
                }
                match disk {
                    store::DiskVault::Sealed => {
                        let key = self.key.lock().ok().and_then(|slot| slot.clone());
                        if let Some(key) = key {
                            match crypto::open_with_key(&raw, &key) {
                                Ok(file) => {
                                    if let Ok(mut slot) = self.last_body.lock() {
                                        *slot = raw;
                                    }
                                    if let Ok(mut slot) = self.snippets.write() {
                                        *slot = file.snippets.clone();
                                    }
                                    self.hook.locked.store(false, Ordering::Relaxed);
                                    self.locked.store(false, Ordering::Relaxed);
                                    self.hook.replace_snippets(hook_snippets(&file.snippets));
                                    let _ = self.app.emit("expansion-changed", self.snapshot(file.snippets, None));
                                }
                                Err(_) => {
                                    self.lock_after_reload(vault, "The vault file changed and could not be unlocked.".into());
                                }
                            }
                        } else if let Ok(mut slot) = self.last_body.lock() {
                            *slot = raw;
                        }
                    }
                    store::DiskVault::Legacy(file) => {
                        if let Ok(mut slot) = self.pending_legacy.lock() {
                            *slot = Some(file.snippets.clone());
                        }
                        if let Ok(mut slot) = self.last_body.lock() {
                            *slot = raw;
                        }
                        self.needs_pin.store(true, Ordering::Relaxed);
                        self.locked.store(true, Ordering::Relaxed);
                        self.hook.locked.store(true, Ordering::Relaxed);
                        if let Ok(mut slot) = self.snippets.write() {
                            slot.clear();
                        }
                        if let Ok(mut slot) = self.key.lock() {
                            *slot = None;
                        }
                        self.hook.replace_snippets(trigger_only(&file.snippets));
                        let _ = self.app.emit("expansion-changed", self.snapshot(Vec::new(), None));
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
        if let Ok(mut slot) = self.snippets.write() {
            slot.clear();
        }
        if let Ok(mut slot) = self.key.lock() {
            *slot = None;
        }
        self.locked.store(true, Ordering::Relaxed);
        self.hook.locked.store(true, Ordering::Relaxed);
        self.hook.replace_snippets(Vec::new());
        let _ = self.app.emit("expansion-changed", self.snapshot_for(vault, error));
    }

    fn snapshot(&self, snippets: Vec<Snippet>, error: Option<String>) -> ExpansionState {
        ExpansionState {
            settings_folder: self.vault.lock().map(|slot| slot.clone()).unwrap_or_default(),
            snippets: if self.locked.load(Ordering::Relaxed) { Vec::new() } else { snippets },
            enabled: self.hook.enabled.load(Ordering::Relaxed),
            locked: self.locked.load(Ordering::Relaxed),
            needs_pin: self.needs_pin.load(Ordering::Relaxed),
            error,
        }
    }

    fn snapshot_for(&self, vault: String, error: String) -> ExpansionState {
        ExpansionState {
            settings_folder: vault,
            snippets: Vec::new(),
            enabled: self.hook.enabled.load(Ordering::Relaxed),
            locked: true,
            needs_pin: self.needs_pin.load(Ordering::Relaxed),
            error: Some(error),
        }
    }
}

pub fn request_quit() {
    QUITTING.store(true, Ordering::SeqCst);
}

pub fn is_quitting() -> bool {
    QUITTING.load(Ordering::SeqCst)
}

fn hook_snippets(snippets: &[Snippet]) -> Vec<matcher::Snippet> {
    snippets
        .iter()
        .map(|snippet| matcher::Snippet {
            trigger: snippet.trigger.clone(),
            text: snippet.text.clone(),
            enabled: snippet.enabled,
        })
        .collect()
}

fn trigger_only(snippets: &[Snippet]) -> Vec<matcher::Snippet> {
    snippets
        .iter()
        .map(|snippet| matcher::Snippet {
            trigger: snippet.trigger.clone(),
            text: String::new(),
            enabled: snippet.enabled,
        })
        .collect()
}
