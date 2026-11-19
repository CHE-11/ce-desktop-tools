//! Folder rules.
//!
//! Each watched folder has an ordered list of rules. When a file lands in the
//! folder, the first enabled rule that matches runs its steps in order: move,
//! copy, rename, or recycle. Rules live in folder-rules.json beside the settings file.
//! The activity log stays in memory and is never written down.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

pub const FILE_NAME: &str = "folder-rules.json";
const REVISION: u32 = 1;
const SETTLE: Duration = Duration::from_secs(2);
const GIVE_UP: Duration = Duration::from_secs(5 * 60);
const LOG_LIMIT: usize = 200;
const MAX_FOLDERS: usize = 20;
const MAX_RULES: usize = 50;
const MAX_STEPS: usize = 8;
const ACTIONS: &[&str] = &["move", "copy", "recycle", "rename"];
const CONFLICTS: &[&str] = &["number", "skip", "overwrite"];
const PARTIAL: &[&str] = &["crdownload", "part", "partial", "tmp", "download", "opdownload"];

fn enabled_default() -> bool {
    true
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RuleMatch {
    pub extensions: Vec<String>,
    pub name_contains: String,
    pub older_than_days: u32,
    pub min_size_kb: u64,
}

impl RuleMatch {
    fn is_empty(&self) -> bool {
        self.extensions.is_empty()
            && self.name_contains.trim().is_empty()
            && self.older_than_days == 0
            && self.min_size_kb == 0
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RuleStep {
    pub id: String,
    pub action: String,
    pub target: String,
    pub rename_pattern: String,
    pub on_conflict: String,
}

impl Default for RuleStep {
    fn default() -> Self {
        Self {
            id: String::new(),
            action: "move".into(),
            target: String::new(),
            rename_pattern: String::new(),
            on_conflict: "number".into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Rule {
    pub id: String,
    pub name: String,
    #[serde(default = "enabled_default")]
    pub enabled: bool,
    #[serde(rename = "match")]
    pub matcher: RuleMatch,
    pub steps: Vec<RuleStep>,
    /// Older files stored one action on the rule. Read on load, never written.
    #[serde(default, skip_serializing)]
    pub action: String,
    #[serde(default, skip_serializing)]
    pub target: String,
    #[serde(default, skip_serializing)]
    pub rename_pattern: String,
    #[serde(default, skip_serializing)]
    pub on_conflict: String,
}

impl Default for Rule {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            enabled: true,
            matcher: RuleMatch::default(),
            steps: Vec::new(),
            action: String::new(),
            target: String::new(),
            rename_pattern: String::new(),
            on_conflict: String::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WatchedFolder {
    pub id: String,
    pub path: String,
    #[serde(default = "enabled_default")]
    pub enabled: bool,
    pub rules: Vec<Rule>,
}

impl Default for WatchedFolder {
    fn default() -> Self {
        Self {
            id: String::new(),
            path: String::new(),
            enabled: true,
            rules: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct RulesFile {
    revision: u32,
    #[serde(default = "enabled_default")]
    enabled: bool,
    folders: Vec<WatchedFolder>,
}

impl Default for RulesFile {
    fn default() -> Self {
        Self {
            revision: REVISION,
            enabled: true,
            folders: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderRulesState {
    pub settings_folder: String,
    pub enabled: bool,
    pub folders: Vec<WatchedFolder>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityEntry {
    pub at: u64,
    pub folder: String,
    pub file: String,
    pub rule: String,
    pub action: String,
    /// `done`, `planned`, `skipped`, or `error`.
    pub outcome: String,
    pub to: String,
    pub detail: String,
}

struct Inner {
    app: AppHandle,
    file: Mutex<RulesFile>,
    folder: Mutex<String>,
    error: Mutex<Option<String>>,
    log: Mutex<VecDeque<ActivityEntry>>,
    /// Files a rule produced this session. They are not handled again, so a
    /// rename pattern does not apply to its own output.
    produced: Mutex<HashSet<PathBuf>>,
    watcher: Mutex<Option<RecommendedWatcher>>,
    events: Mutex<Option<Sender<PathBuf>>>,
}

#[derive(Clone)]
pub struct FolderRules {
    inner: Arc<Inner>,
}

impl FolderRules {
    pub fn start(app: AppHandle) -> Self {
        let (tx, rx) = mpsc::channel();
        let rules = Self {
            inner: Arc::new(Inner {
                app,
                file: Mutex::new(RulesFile::default()),
                folder: Mutex::new(String::new()),
                error: Mutex::new(None),
                log: Mutex::new(VecDeque::new()),
                produced: Mutex::new(HashSet::new()),
                watcher: Mutex::new(None),
                events: Mutex::new(Some(tx)),
            }),
        };
        let worker = rules.clone();
        let _ = std::thread::Builder::new()
            .name("folder-rules".into())
            .spawn(move || worker.settle_loop(rx));
        rules.reload();
        rules
    }

    /// Reads folder-rules.json from the current settings folder and restarts the watchers.
    pub fn reload(&self) {
        let folder = crate::app_settings::settings_folder(&self.inner.app).unwrap_or_default();
        let (file, error) = if folder.is_empty() {
            (RulesFile::default(), None)
        } else {
            match load(&folder) {
                Ok(file) => (file, None),
                Err(error) => (RulesFile::default(), Some(error)),
            }
        };
        set(&self.inner.folder, folder);
        set(&self.inner.error, error);
        set(&self.inner.file, file);
        self.rewatch();
    }

    pub fn state(&self) -> FolderRulesState {
        let file = get(&self.inner.file);
        FolderRulesState {
            settings_folder: get(&self.inner.folder),
            enabled: file.enabled,
            folders: file.folders,
            error: get(&self.inner.error),
        }
    }

    pub fn save(&self, enabled: bool, folders: Vec<WatchedFolder>) -> Result<FolderRulesState, String> {
        let folder = crate::app_settings::settings_folder(&self.inner.app).unwrap_or_default();
        if folder.is_empty() || !Path::new(&folder).is_dir() {
            return Err(
                "Choose a settings folder. folder-rules.json is saved there with the settings file."
                    .into(),
            );
        }
        let file = prepare(RulesFile {
            revision: REVISION,
            enabled,
            folders,
        })?;
        write_file(&file_path(&folder), &file)?;
        set(&self.inner.folder, folder);
        set(&self.inner.error, None);
        set(&self.inner.file, file);
        self.rewatch();
        Ok(self.state())
    }

    pub fn activity(&self) -> Vec<ActivityEntry> {
        self.inner
            .log
            .lock()
            .map(|log| log.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Applies rules to the files already in one folder, or in every enabled
    /// folder when `folder_id` is `None`. A dry run only reports the plan.
    pub fn run(&self, folder_id: Option<String>, dry_run: bool) -> Result<Vec<ActivityEntry>, String> {
        let file = get(&self.inner.file);
        let folders: Vec<WatchedFolder> = match &folder_id {
            Some(id) => {
                let folder = file
                    .folders
                    .iter()
                    .find(|folder| &folder.id == id)
                    .cloned()
                    .ok_or_else(|| "That folder is no longer in the list.".to_string())?;
                vec![folder]
            }
            None => file
                .folders
                .iter()
                .filter(|folder| folder.enabled)
                .cloned()
                .collect(),
        };
        let mut entries = Vec::new();
        for folder in folders {
            let Ok(children) = fs::read_dir(&folder.path) else {
                entries.push(entry(&folder.path, "", "", "", "error", "", "Couldn't read this folder."));
                continue;
            };
            let mut files: Vec<PathBuf> = children
                .flatten()
                .map(|child| child.path())
                .filter(|path| path.is_file())
                .collect();
            files.sort();
            for path in files {
                if let Some(result) = self.handle(&folder, &path, dry_run) {
                    entries.push(result);
                }
            }
        }
        Ok(entries)
    }

    fn rewatch(&self) {
        let file = get(&self.inner.file);
        let Ok(mut slot) = self.inner.watcher.lock() else {
            return;
        };
        *slot = None;
        if !file.enabled {
            return;
        }
        let paths: Vec<String> = file
            .folders
            .iter()
            .filter(|folder| folder.enabled && Path::new(&folder.path).is_dir())
            .map(|folder| folder.path.clone())
            .collect();
        if paths.is_empty() {
            return;
        }
        let Some(sender) = self.inner.events.lock().ok().and_then(|slot| slot.clone()) else {
            return;
        };
        let watcher = notify::recommended_watcher(move |result: Result<Event, notify::Error>| {
            let Ok(event) = result else {
                return;
            };
            if matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_)) {
                for path in event.paths {
                    let _ = sender.send(path);
                }
            }
        });
        let mut watcher = match watcher {
            Ok(watcher) => watcher,
            Err(error) => {
                set(&self.inner.error, Some(format!("Couldn't watch folders: {error}")));
                return;
            }
        };
        for path in paths {
            if let Err(error) = watcher.watch(Path::new(&path), RecursiveMode::NonRecursive) {
                set(&self.inner.error, Some(format!("Couldn't watch {path}: {error}")));
            }
        }
        *slot = Some(watcher);
    }

    /// Waits until a file has been quiet for a moment and can be opened alone,
    /// so a download or copy in progress is left alone.
    fn settle_loop(&self, events: Receiver<PathBuf>) {
        let mut pending: HashMap<PathBuf, (Instant, Instant)> = HashMap::new();
        loop {
            match events.recv_timeout(Duration::from_millis(500)) {
                Ok(path) => {
                    let now = Instant::now();
                    pending
                        .entry(path)
                        .and_modify(|(last, _)| *last = now)
                        .or_insert((now, now));
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
            let now = Instant::now();
            let ready: Vec<PathBuf> = pending
                .iter()
                .filter(|(_, (last, _))| now.duration_since(*last) >= SETTLE)
                .map(|(path, _)| path.clone())
                .collect();
            for path in ready {
                let Some((_, first)) = pending.get(&path).copied() else {
                    continue;
                };
                if !path.is_file() {
                    pending.remove(&path);
                    continue;
                }
                if !is_free(&path) {
                    if now.duration_since(first) > GIVE_UP {
                        pending.remove(&path);
                    } else if let Some(times) = pending.get_mut(&path) {
                        times.0 = now;
                    }
                    continue;
                }
                pending.remove(&path);
                self.on_new_file(&path);
            }
        }
    }

    fn on_new_file(&self, path: &Path) {
        let file = get(&self.inner.file);
        if !file.enabled {
            return;
        }
        let Some(parent) = path.parent() else {
            return;
        };
        let folder = file
            .folders
            .iter()
            .find(|folder| folder.enabled && same_path(Path::new(&folder.path), parent));
        if let Some(folder) = folder {
            self.handle(folder, path, false);
        }
    }

    fn handle(&self, folder: &WatchedFolder, path: &Path, dry_run: bool) -> Option<ActivityEntry> {
        if skip_file(path) || self.was_produced(path) {
            return None;
        }
        let metadata = fs::metadata(path).ok()?;
        let rule = folder
            .rules
            .iter()
            .find(|rule| rule.enabled && matches(rule, path, &metadata, SystemTime::now()))?;
        let file_name = display_name(path);
        let chain = apply_chain(rule, path, dry_run);
        if !chain.produced.is_empty()
            && let Ok(mut produced) = self.inner.produced.lock()
        {
            for path in chain.produced {
                produced.insert(path);
            }
        }
        let record = entry(
            &folder.path,
            &file_name,
            &rule.name,
            &chain.action,
            &chain.outcome,
            &chain.to,
            &chain.detail,
        );
        if !dry_run {
            self.log(record.clone());
        }
        Some(record)
    }

    fn was_produced(&self, path: &Path) -> bool {
        self.inner
            .produced
            .lock()
            .map(|produced| produced.contains(path))
            .unwrap_or(false)
    }

    fn log(&self, record: ActivityEntry) {
        if let Ok(mut log) = self.inner.log.lock() {
            log.push_front(record.clone());
            log.truncate(LOG_LIMIT);
        }
        let _ = self.inner.app.emit("folder-rules-activity", record);
    }
}

fn get<T: Clone + Default>(slot: &Mutex<T>) -> T {
    slot.lock().map(|value| value.clone()).unwrap_or_default()
}

fn set<T>(slot: &Mutex<T>, value: T) {
    if let Ok(mut guard) = slot.lock() {
        *guard = value;
    }
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or(0)
}

fn entry(
    folder: &str,
    file: &str,
    rule: &str,
    action: &str,
    outcome: &str,
    to: &str,
    detail: &str,
) -> ActivityEntry {
    ActivityEntry {
        at: now_millis(),
        folder: folder.into(),
        file: file.into(),
        rule: rule.into(),
        action: action.into(),
        outcome: outcome.into(),
        to: to.into(),
        detail: detail.into(),
    }
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default()
}

fn same_path(left: &Path, right: &Path) -> bool {
    let clean = |path: &Path| {
        path.to_string_lossy()
            .trim_end_matches(['\\', '/'])
            .to_lowercase()
    };
    clean(left) == clean(right)
}

fn skip_file(path: &Path) -> bool {
    let name = display_name(path).to_lowercase();
    if name.is_empty() || name.starts_with("~$") || name.starts_with('.') || name == "desktop.ini" || name == "thumbs.db" {
        return true;
    }
    let extension = extension_of(path);
    PARTIAL.contains(&extension.as_str())
}

fn extension_of(path: &Path) -> String {
    path.extension()
        .map(|extension| extension.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

/// A file another program is still writing cannot be opened without sharing.
fn is_free(path: &Path) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(path)
            .is_ok()
    }
    #[cfg(not(windows))]
    {
        fs::File::open(path).is_ok()
    }
}

fn matches(rule: &Rule, path: &Path, metadata: &fs::Metadata, now: SystemTime) -> bool {
    let condition = &rule.matcher;
    if condition.is_empty() {
        return false;
    }
    if !condition.extensions.is_empty() && !condition.extensions.contains(&extension_of(path)) {
        return false;
    }
    let pattern = condition.name_contains.trim().to_lowercase();
    if !pattern.is_empty() {
        let name = display_name(path).to_lowercase();
        let hit = if pattern.contains(['*', '?']) {
            wildcard(&pattern, &name)
        } else {
            name.contains(&pattern)
        };
        if !hit {
            return false;
        }
    }
    if condition.older_than_days > 0 {
        let age = metadata
            .modified()
            .ok()
            .and_then(|modified| now.duration_since(modified).ok())
            .unwrap_or_default();
        if age < Duration::from_secs(u64::from(condition.older_than_days) * 86_400) {
            return false;
        }
    }
    if condition.min_size_kb > 0 && metadata.len() < condition.min_size_kb.saturating_mul(1024) {
        return false;
    }
    true
}

fn wildcard(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let text: Vec<char> = text.chars().collect();
    let (mut p, mut t) = (0usize, 0usize);
    let (mut star, mut mark) = (None, 0usize);
    while t < text.len() {
        if p < pattern.len() && (pattern[p] == '?' || pattern[p] == text[t]) {
            p += 1;
            t += 1;
        } else if p < pattern.len() && pattern[p] == '*' {
            star = Some(p);
            mark = t;
            p += 1;
        } else if let Some(position) = star {
            p = position + 1;
            mark += 1;
            t = mark;
        } else {
            return false;
        }
    }
    while p < pattern.len() && pattern[p] == '*' {
        p += 1;
    }
    p == pattern.len()
}

struct ChainOutcome {
    produced: Vec<PathBuf>,
    outcome: String,
    to: String,
    detail: String,
    action: String,
}

/// Runs every step on one file. Move and rename hand the next step the new
/// path. Copy leaves the original in place. Recycle ends the chain.
fn apply_chain(rule: &Rule, start: &Path, dry_run: bool) -> ChainOutcome {
    let action = rule
        .steps
        .iter()
        .map(|step| step.action.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let fail = |produced: Vec<PathBuf>, to: String, detail: String| ChainOutcome {
        produced,
        outcome: "error".into(),
        to,
        detail,
        action: action.clone(),
    };
    if rule.steps.is_empty() {
        return fail(Vec::new(), String::new(), "This rule has no steps.".into());
    }
    let mut current = start.to_path_buf();
    let mut produced = Vec::new();
    let mut details = Vec::new();
    let mut final_to = String::new();
    let mut file_moved = false;
    let mut saw_done = false;

    for (index, step) in rule.steps.iter().enumerate() {
        if step.action == "recycle" && index + 1 < rule.steps.len() {
            return fail(
                produced,
                final_to,
                "Send to Recycle Bin has to be the last step.".into(),
            );
        }
        let planned = match plan_target(step, &current) {
            Ok(target) => target,
            Err(error) => return fail(produced, final_to, error),
        };
        if dry_run {
            details.push(step_detail(&step.action));
            remember_destination(step, planned, &mut current, &mut final_to, &mut file_moved);
            continue;
        }
        match apply(step, &current, planned.as_deref()) {
            Ok((step_outcome, step_to, detail)) => {
                details.push(detail);
                if step_outcome == "done" {
                    saw_done = true;
                    if step.action != "recycle"
                        && let Some(target) = step_to.clone()
                    {
                        produced.push(target);
                    }
                }
                remember_destination(step, step_to, &mut current, &mut final_to, &mut file_moved);
            }
            Err(error) => return fail(produced, final_to, error),
        }
    }

    let outcome = if dry_run {
        "planned"
    } else if saw_done {
        "done"
    } else {
        "skipped"
    };
    ChainOutcome {
        produced: if dry_run { Vec::new() } else { produced },
        outcome: outcome.into(),
        to: final_to,
        detail: details.join(", "),
        action,
    }
}

/// `to` is where the file itself ended up. A copy updates that only while the
/// original has not moved. Recycle clears it.
fn remember_destination(
    step: &RuleStep,
    target: Option<PathBuf>,
    current: &mut PathBuf,
    final_to: &mut String,
    file_moved: &mut bool,
) {
    match step.action.as_str() {
        "copy" => {
            if !*file_moved && let Some(target) = target {
                *final_to = target.display().to_string();
            }
        }
        "recycle" => final_to.clear(),
        _ => {
            if let Some(target) = target {
                *file_moved = true;
                *final_to = target.display().to_string();
                *current = target;
            }
        }
    }
}

fn step_detail(action: &str) -> String {
    match action {
        "copy" => "Copied",
        "rename" => "Renamed",
        "recycle" => "Sent to the Recycle Bin",
        _ => "Moved",
    }
    .into()
}

/// Where a file will go. `None` means it stays put (recycle).
fn plan_target(step: &RuleStep, path: &Path) -> Result<Option<PathBuf>, String> {
    let name = display_name(path);
    let desired = match step.action.as_str() {
        "move" | "copy" => Path::new(&expand_dates(step.target.trim())).join(&name),
        "rename" => {
            let next = rename_to(&step.rename_pattern, path);
            if next.is_empty() || next == name {
                return Err("The rename pattern gives the same name.".into());
            }
            path.with_file_name(next)
        }
        _ => return Ok(None),
    };
    if same_path(&desired, path) {
        return Err("The file is already where this rule puts it.".into());
    }
    if !desired.exists() {
        return Ok(Some(desired));
    }
    match step.on_conflict.as_str() {
        "overwrite" => Ok(Some(desired)),
        "skip" => Err(format!("{} already exists.", display_name(&desired))),
        _ => Ok(Some(numbered(&desired))),
    }
}

fn apply(
    step: &RuleStep,
    path: &Path,
    target: Option<&Path>,
) -> Result<(String, Option<PathBuf>, String), String> {
    match (step.action.as_str(), target) {
        ("recycle", _) => {
            recycle(path)?;
            Ok(("done".into(), None, "Sent to the Recycle Bin".into()))
        }
        ("copy", Some(target)) => {
            if already_copied(path, target) {
                return Ok((
                    "skipped".into(),
                    Some(target.to_path_buf()),
                    "Already copied".into(),
                ));
            }
            ensure_parent(target)?;
            fs::copy(path, target).map_err(|error| format!("Couldn't copy: {error}"))?;
            Ok(("done".into(), Some(target.to_path_buf()), "Copied".into()))
        }
        ("move" | "rename", Some(target)) => {
            ensure_parent(target)?;
            move_file(path, target)?;
            let detail = step_detail(&step.action);
            Ok(("done".into(), Some(target.to_path_buf()), detail))
        }
        _ => Err("This rule has nowhere to put the file.".into()),
    }
}

fn already_copied(source: &Path, target: &Path) -> bool {
    let (Ok(source), Ok(target)) = (fs::metadata(source), fs::metadata(target)) else {
        return false;
    };
    source.len() == target.len()
        && match (source.modified(), target.modified()) {
            (Ok(from), Ok(to)) => to >= from,
            _ => false,
        }
}

fn ensure_parent(target: &Path) -> Result<(), String> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Couldn't create {}: {error}", parent.display()))?;
    }
    Ok(())
}

/// `rename` fails across drives, so fall back to copy and delete.
fn move_file(from: &Path, to: &Path) -> Result<(), String> {
    if to.exists() {
        fs::remove_file(to).map_err(|error| format!("Couldn't replace {}: {error}", to.display()))?;
    }
    if fs::rename(from, to).is_ok() {
        return Ok(());
    }
    fs::copy(from, to).map_err(|error| format!("Couldn't move: {error}"))?;
    fs::remove_file(from).map_err(|error| {
        let _ = fs::remove_file(to);
        format!("Couldn't remove the original after copying: {error}")
    })
}

fn numbered(path: &Path) -> PathBuf {
    let stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_default();
    let extension = path
        .extension()
        .map(|extension| format!(".{}", extension.to_string_lossy()))
        .unwrap_or_default();
    for number in 1..10_000 {
        let candidate = path.with_file_name(format!("{stem} ({number}){extension}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    path.with_file_name(format!("{stem} ({}){extension}", now_millis()))
}

const NAME_MARK: char = '\u{E001}';
const EXT_MARK: char = '\u{E002}';

/// `{name}` is the name without its extension, `{ext}` is the extension
/// without the dot, and any other `{...}` is a date picture such as `{yyyy-MM-dd}`.
fn rename_to(pattern: &str, path: &Path) -> String {
    let stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_default();
    let extension = path
        .extension()
        .map(|extension| extension.to_string_lossy().to_string())
        .unwrap_or_default();
    let marked = replace_token(&replace_token(pattern.trim(), "name", NAME_MARK), "ext", EXT_MARK);
    let dated = expand_dates(&marked);
    let mut name = dated
        .replace(NAME_MARK, &stem)
        .replace(EXT_MARK, &extension);
    if !pattern.to_lowercase().contains("{ext}") && !extension.is_empty() {
        name.push('.');
        name.push_str(&extension);
    }
    name.chars()
        .filter(|ch| !matches!(ch, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        .collect::<String>()
        .trim()
        .to_string()
}

fn replace_token(text: &str, token: &str, mark: char) -> String {
    let needle = format!("{{{token}}}");
    let mut out = String::new();
    let mut rest = text;
    while let Some(position) = rest.to_ascii_lowercase().find(&needle) {
        out.push_str(&rest[..position]);
        out.push(mark);
        rest = &rest[position + needle.len()..];
    }
    out.push_str(rest);
    out
}

fn expand_dates(text: &str) -> String {
    if text.contains('{') {
        crate::date_expand::render_now(text)
    } else {
        text.to_string()
    }
}

#[cfg(windows)]
fn recycle(path: &Path) -> Result<(), String> {
    use windows::Win32::UI::Shell::{
        FO_DELETE, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT,
        SHFILEOPSTRUCTW, SHFileOperationW,
    };
    use windows::core::PCWSTR;

    let wide: Vec<u16> = path
        .as_os_str()
        .to_string_lossy()
        .encode_utf16()
        .chain([0, 0])
        .collect();
    let mut operation = SHFILEOPSTRUCTW {
        wFunc: FO_DELETE,
        pFrom: PCWSTR(wide.as_ptr()),
        fFlags: (FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT | FOF_NOERRORUI).0 as u16,
        ..Default::default()
    };
    let code = unsafe { SHFileOperationW(&mut operation) };
    if code != 0 || operation.fAnyOperationsAborted.as_bool() {
        return Err(format!("Couldn't send it to the Recycle Bin (error {code})."));
    }
    Ok(())
}

#[cfg(not(windows))]
fn recycle(_path: &Path) -> Result<(), String> {
    Err("The Recycle Bin is only available on Windows.".into())
}

fn file_path(folder: &str) -> PathBuf {
    Path::new(folder).join(FILE_NAME)
}

fn load(folder: &str) -> Result<RulesFile, String> {
    let path = file_path(folder);
    if !path.is_file() {
        return Ok(RulesFile::default());
    }
    let bytes = fs::read(&path).map_err(|error| format!("Couldn't read {}: {error}", path.display()))?;
    let file: RulesFile = serde_json::from_slice(&bytes)
        .map_err(|error| format!("{} is not a valid rules file: {error}", path.display()))?;
    Ok(normalize(file))
}

fn write_file(path: &Path, file: &RulesFile) -> Result<(), String> {
    let mut payload =
        serde_json::to_vec_pretty(file).map_err(|error| format!("Couldn't save rules: {error}"))?;
    payload.push(b'\n');
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, &payload)
        .map_err(|error| format!("Couldn't save {}: {error}", path.display()))?;
    if fs::rename(&temporary, path).is_ok() {
        return Ok(());
    }
    let wrote = fs::write(path, &payload);
    let _ = fs::remove_file(&temporary);
    wrote.map_err(|error| format!("Couldn't save {}: {error}", path.display()))
}

fn fresh_id(prefix: &str) -> String {
    format!("{prefix}-{:x}", rand::random::<u64>())
}

/// Cleans a loaded file without rejecting it. Unknown values fall back.
fn normalize(mut file: RulesFile) -> RulesFile {
    file.revision = REVISION;
    file.folders.truncate(MAX_FOLDERS);
    let mut folder_ids = HashSet::new();
    for folder in &mut file.folders {
        folder.path = folder.path.trim().to_string();
        if folder.id.trim().is_empty() || !folder_ids.insert(folder.id.clone()) {
            folder.id = fresh_id("folder");
            folder_ids.insert(folder.id.clone());
        }
        folder.rules.truncate(MAX_RULES);
        let mut rule_ids = HashSet::new();
        for rule in &mut folder.rules {
            if rule.id.trim().is_empty() || !rule_ids.insert(rule.id.clone()) {
                rule.id = fresh_id("rule");
                rule_ids.insert(rule.id.clone());
            }
            rule.name = rule.name.trim().to_string();
            if rule.steps.is_empty() {
                rule.steps.push(RuleStep {
                    action: rule.action.clone(),
                    target: rule.target.clone(),
                    rename_pattern: rule.rename_pattern.clone(),
                    on_conflict: rule.on_conflict.clone(),
                    ..RuleStep::default()
                });
            }
            rule.steps.truncate(MAX_STEPS);
            let mut step_ids = HashSet::new();
            for step in &mut rule.steps {
                if step.id.trim().is_empty() || !step_ids.insert(step.id.clone()) {
                    step.id = fresh_id("step");
                    step_ids.insert(step.id.clone());
                }
                if !ACTIONS.contains(&step.action.as_str()) {
                    step.action = "move".into();
                }
                if !CONFLICTS.contains(&step.on_conflict.as_str()) {
                    step.on_conflict = "number".into();
                }
            }
            rule.action.clear();
            rule.target.clear();
            rule.rename_pattern.clear();
            rule.on_conflict.clear();
            let mut extensions = Vec::new();
            for extension in &rule.matcher.extensions {
                for part in extension.split([',', ' ', ';']) {
                    let clean = part.trim().trim_start_matches("*.").trim_start_matches('.').to_lowercase();
                    if !clean.is_empty() && !extensions.contains(&clean) {
                        extensions.push(clean);
                    }
                }
            }
            rule.matcher.extensions = extensions;
            rule.matcher.name_contains = rule.matcher.name_contains.trim().to_string();
        }
    }
    file
}

/// Checks a file the user is saving. Problems are reported, not dropped.
fn prepare(file: RulesFile) -> Result<RulesFile, String> {
    if file.folders.len() > MAX_FOLDERS {
        return Err(format!("Folder rules can watch {MAX_FOLDERS} folders."));
    }
    for folder in &file.folders {
        for rule in &folder.rules {
            let label = if rule.name.is_empty() {
                "A rule"
            } else {
                rule.name.as_str()
            };
            if rule.steps.is_empty() && rule.action.trim().is_empty() {
                return Err(format!("{label} needs a then step."));
            }
            if rule.steps.len() > MAX_STEPS {
                return Err(format!("{label} can have {MAX_STEPS} then steps."));
            }
        }
    }
    let file = normalize(file);
    for folder in &file.folders {
        if folder.path.is_empty() {
            return Err("Each watched folder needs a path.".into());
        }
        if !Path::new(&folder.path).is_dir() {
            return Err(format!("Folder does not exist: {}", folder.path));
        }
        for rule in &folder.rules {
            let label = if rule.name.is_empty() {
                "A rule"
            } else {
                rule.name.as_str()
            };
            if rule.matcher.is_empty() {
                return Err(format!(
                    "{label} needs at least one condition, so it does not touch every file."
                ));
            }
            if rule.steps.is_empty() {
                return Err(format!("{label} needs a then step."));
            }
            let mut recycled = false;
            for step in &rule.steps {
                if recycled {
                    return Err(format!("{label} has a step after Send to Recycle Bin."));
                }
                match step.action.as_str() {
                    "move" | "copy" if step.target.trim().is_empty() => {
                        return Err(format!("{label} needs a destination folder."));
                    }
                    "move" | "copy"
                        if same_path(Path::new(step.target.trim()), Path::new(&folder.path)) =>
                    {
                        return Err(format!("{label} points back at the folder it watches."));
                    }
                    "rename" if step.rename_pattern.trim().is_empty() => {
                        return Err(format!("{label} needs a rename pattern."));
                    }
                    "recycle" => recycled = true,
                    _ => {}
                }
            }
        }
    }
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::{
        Rule, RuleMatch, RuleStep, RulesFile, WatchedFolder, apply_chain, matches, normalize,
        numbered, prepare, rename_to, wildcard,
    };
    use std::path::Path;
    use std::time::{Duration, SystemTime};

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "ce-desktop-tools-rules-{name}-{}",
            SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        directory
    }

    fn rule(matcher: RuleMatch) -> Rule {
        Rule {
            name: "test".into(),
            matcher,
            steps: vec![RuleStep {
                target: "C:\\Sorted".into(),
                ..RuleStep::default()
            }],
            ..Rule::default()
        }
    }

    #[test]
    fn conditions_must_all_match() {
        let directory = temp_dir("match");
        let path = directory.join("Invoice-2024.PDF");
        std::fs::write(&path, vec![0u8; 4096]).unwrap();
        let metadata = std::fs::metadata(&path).unwrap();
        let now = SystemTime::now();

        let by_extension = rule(RuleMatch {
            extensions: vec!["pdf".into()],
            ..RuleMatch::default()
        });
        assert!(matches(&by_extension, &path, &metadata, now));

        let by_name = rule(RuleMatch {
            extensions: vec!["pdf".into()],
            name_contains: "invoice".into(),
            ..RuleMatch::default()
        });
        assert!(matches(&by_name, &path, &metadata, now));

        let glob = rule(RuleMatch {
            name_contains: "inv*-20??.pdf".into(),
            ..RuleMatch::default()
        });
        assert!(matches(&glob, &path, &metadata, now));

        let too_big = rule(RuleMatch {
            min_size_kb: 5,
            ..RuleMatch::default()
        });
        assert!(!matches(&too_big, &path, &metadata, now));

        let old = rule(RuleMatch {
            older_than_days: 3,
            ..RuleMatch::default()
        });
        assert!(!matches(&old, &path, &metadata, now));
        assert!(matches(&old, &path, &metadata, now + Duration::from_secs(4 * 86_400)));

        let nothing = rule(RuleMatch::default());
        assert!(!matches(&nothing, &path, &metadata, now));

        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn wildcards_cover_star_and_question_mark() {
        assert!(wildcard("*.pdf", "a.pdf"));
        assert!(wildcard("img_????.jpg", "img_0001.jpg"));
        assert!(!wildcard("img_????.jpg", "img_01.jpg"));
        assert!(wildcard("*report*", "q3 report final.docx"));
    }

    #[test]
    fn a_taken_name_gets_the_next_number() {
        let directory = temp_dir("numbered");
        let path = directory.join("photo.jpg");
        std::fs::write(&path, b"x").unwrap();
        assert_eq!(numbered(&path), directory.join("photo (1).jpg"));
        std::fs::write(directory.join("photo (1).jpg"), b"x").unwrap();
        assert_eq!(numbered(&path), directory.join("photo (2).jpg"));
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn rename_patterns_keep_the_extension() {
        let path = Path::new("C:\\Downloads\\scan.PDF");
        assert_eq!(rename_to("{name}-signed", path), "scan-signed.PDF");
        assert_eq!(rename_to("{NAME}.{ext}", path), "scan.PDF");
        assert_eq!(rename_to("a/b:{name}", path), "abscan.PDF");
    }

    #[test]
    fn an_old_or_odd_file_still_loads() {
        let file: RulesFile = serde_json::from_str(
            r#"{"folders":[{"path":" C:\\Downloads ","rules":[{"name":"PDFs","match":{"extensions":[".PDF, *.docx"]},"action":"shred","onConflict":"maybe"}]}]}"#,
        )
        .unwrap();
        let file = normalize(file);
        assert!(file.enabled);
        let folder = &file.folders[0];
        assert!(folder.enabled);
        assert_eq!(folder.path, "C:\\Downloads");
        assert!(!folder.id.is_empty());
        let rule = &folder.rules[0];
        assert!(rule.enabled);
        assert_eq!(rule.steps.len(), 1);
        assert_eq!(rule.steps[0].action, "move");
        assert_eq!(rule.steps[0].on_conflict, "number");
        assert!(rule.action.is_empty());
        assert_eq!(rule.matcher.extensions, vec!["pdf", "docx"]);
        let saved = serde_json::to_value(rule).unwrap();
        assert!(saved.get("action").is_none());
        assert!(saved.get("steps").is_some());
    }

    #[test]
    fn steps_already_on_a_rule_are_left_alone() {
        let file: RulesFile = serde_json::from_str(
            r#"{"folders":[{"path":"C:\\Downloads","rules":[{"name":"PDFs","action":"recycle","steps":[{"id":"step-1","action":"copy","target":"D:\\A","onConflict":"skip"}]}]}]}"#,
        )
        .unwrap();
        let file = normalize(file);
        let step = &file.folders[0].rules[0].steps[0];
        assert_eq!(file.folders[0].rules[0].steps.len(), 1);
        assert_eq!(step.id, "step-1");
        assert_eq!(step.action, "copy");
        assert_eq!(step.target, "D:\\A");
        assert_eq!(step.on_conflict, "skip");
    }

    #[test]
    fn steps_run_rename_then_move_and_copy_keeps_the_original() {
        let source = temp_dir("chain-src");
        let archive = temp_dir("chain-archive");
        let dest = temp_dir("chain-dest");
        let file = source.join("scan.PDF");
        std::fs::write(&file, b"hello").unwrap();
        let copy_then_move = Rule {
            name: "chain".into(),
            steps: vec![
                RuleStep {
                    action: "copy".into(),
                    target: archive.display().to_string(),
                    ..RuleStep::default()
                },
                RuleStep {
                    action: "rename".into(),
                    rename_pattern: "{name}-signed".into(),
                    ..RuleStep::default()
                },
                RuleStep {
                    action: "move".into(),
                    target: dest.display().to_string(),
                    ..RuleStep::default()
                },
            ],
            ..Rule::default()
        };
        let preview = apply_chain(&copy_then_move, &file, true);
        assert_eq!(preview.outcome, "planned");
        assert_eq!(preview.detail, "Copied, Renamed, Moved");
        assert!(preview.produced.is_empty());
        assert!(file.is_file());

        let chain = apply_chain(&copy_then_move, &file, false);
        assert_eq!(chain.outcome, "done");
        assert_eq!(chain.detail, "Copied, Renamed, Moved");
        assert_eq!(chain.action, "copy, rename, move");
        let copied = archive.join("scan.PDF");
        let landed = dest.join("scan-signed.PDF");
        assert!(copied.is_file());
        assert!(landed.is_file());
        assert!(!file.exists());
        assert!(!source.join("scan-signed.PDF").exists());
        assert_eq!(chain.to, landed.display().to_string());
        let _ = std::fs::remove_dir_all(&source);
        let _ = std::fs::remove_dir_all(&archive);
        let _ = std::fs::remove_dir_all(&dest);
    }

    #[test]
    fn recycle_has_to_be_the_last_step() {
        let directory = temp_dir("prepare-recycle");
        let file = RulesFile {
            folders: vec![WatchedFolder {
                path: directory.display().to_string(),
                rules: vec![Rule {
                    name: "Bad".into(),
                    matcher: RuleMatch {
                        extensions: vec!["pdf".into()],
                        ..RuleMatch::default()
                    },
                    steps: vec![
                        RuleStep {
                            action: "recycle".into(),
                            ..RuleStep::default()
                        },
                        RuleStep {
                            action: "move".into(),
                            target: directory.display().to_string(),
                            ..RuleStep::default()
                        },
                    ],
                    ..Rule::default()
                }],
                ..WatchedFolder::default()
            }],
            ..RulesFile::default()
        };
        let error = prepare(file).unwrap_err();
        assert!(error.contains("Recycle Bin"), "{error}");
        let _ = std::fs::remove_dir_all(&directory);
    }
}
