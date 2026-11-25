//! Publisher, version, install path, and icon for a Start menu app.
//!
//! Store apps come from the per-user package registry and `AppxManifest.xml`.
//! Desktop apps are a known-folder path, or a Start menu shortcut whose
//! AppUserModelID matches. Icons are the package logo, the program icon, or
//! the AppsFolder shell image. The lookup is cached for the process so opening
//! the tool again does not walk the Start menu a second time.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::launcher::apps::AppEntry;

const INDEX_TTL: Duration = Duration::from_secs(10 * 60);
const LOGO_BYTES: u64 = 80_000;
const LOGO_SUFFIXES: &[&str] = &[
    ".targetsize-32.png",
    ".targetsize-48.png",
    ".targetsize-24.png",
    ".targetsize-36.png",
    ".scale-200.png",
    ".scale-100.png",
    ".png",
];

#[derive(Debug)]
pub struct AppFacts {
    pub publisher: Option<String>,
    pub version: Option<String>,
    pub kind: String,
    pub location: Option<String>,
    pub icon: Option<String>,
}

pub fn facts_for(apps: &[AppEntry]) -> Vec<AppFacts> {
    #[cfg(windows)]
    {
        windows_facts(apps)
    }
    #[cfg(not(windows))]
    {
        apps.iter().map(|_| empty_facts("desktop")).collect()
    }
}

#[cfg(not(windows))]
fn empty_facts(kind: &str) -> AppFacts {
    AppFacts {
        publisher: None,
        version: None,
        kind: kind.to_string(),
        location: None,
        icon: None,
    }
}

fn store_parts(app_id: &str) -> Option<(&str, &str)> {
    let (family, application) = app_id.split_once('!')?;
    if application.is_empty()
        || family.is_empty()
        || family.contains('\\')
        || family.contains('/')
        || !family.contains('_')
    {
        return None;
    }
    Some((family, application))
}

fn family_and_version(full: &str) -> Option<(String, String)> {
    if let Some((body, publisher)) = full.split_once("__") {
        return family_from_body(body, publisher);
    }
    for arch in ["x64", "x86", "arm64", "arm", "neutral"] {
        let marker = format!("_{arch}_");
        let Some(index) = full.find(&marker) else {
            continue;
        };
        let Some((name, version)) = full[..index].rsplit_once('_') else {
            continue;
        };
        if name.is_empty() || !is_version(version) {
            continue;
        }
        let rest = &full[index + marker.len()..];
        let publisher = rest
            .rsplit_once('_')
            .map(|(_, publisher)| publisher)
            .unwrap_or(rest);
        if publisher.is_empty() {
            continue;
        }
        return Some((format!("{name}_{publisher}"), version.to_string()));
    }
    None
}

fn family_from_body(body: &str, publisher: &str) -> Option<(String, String)> {
    if publisher.is_empty() {
        return None;
    }
    let (name_and_version, arch) = body.rsplit_once('_')?;
    if !matches!(arch, "x64" | "x86" | "arm64" | "arm" | "neutral") {
        return None;
    }
    let (name, version) = name_and_version.rsplit_once('_')?;
    if name.is_empty() || !is_version(version) {
        return None;
    }
    Some((format!("{name}_{publisher}"), version.to_string()))
}

fn is_version(value: &str) -> bool {
    let mut parts = value.split('.');
    (0..4).all(|_| {
        parts
            .next()
            .is_some_and(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit()))
    }) && parts.next().is_none()
}

fn version_parts(version: &str) -> Vec<u32> {
    version
        .split('.')
        .map(|part| part.parse().unwrap_or(0))
        .collect()
}

#[derive(Clone, Debug)]
struct ParsedApp {
    id: String,
    executable: Option<String>,
    logo: Option<String>,
}

#[derive(Clone, Debug)]
struct ParsedManifest {
    publisher: Option<String>,
    version: Option<String>,
    apps: Vec<ParsedApp>,
}

fn parse_manifest(xml: &str) -> ParsedManifest {
    ParsedManifest {
        publisher: element_text(xml, "PublisherDisplayName").and_then(clean_publisher),
        version: element_tag(xml, "Identity")
            .and_then(|tag| xml_attr(&tag, "Version").map(decode_xml)),
        apps: application_entries(xml),
    }
}

fn application_entries(xml: &str) -> Vec<ParsedApp> {
    let mut apps = Vec::new();
    let mut rest = xml;
    while let Some(index) = rest.find("<Application") {
        rest = &rest[index..];
        let Some(end) = rest.find('>') else {
            break;
        };
        let open = &rest[..=end];
        let id = xml_attr(open, "Id").unwrap_or("");
        let executable = xml_attr(open, "Executable").map(decode_xml);
        let body = if open.ends_with("/>") {
            ""
        } else {
            let after = &rest[end + 1..];
            let close = after.find("</Application>").unwrap_or(after.len());
            &after[..close]
        };
        let logo = xml_attr(body, "Square44x44Logo")
            .or_else(|| xml_attr(body, "Square30x30Logo"))
            .or_else(|| xml_attr(open, "Square44x44Logo"))
            .map(decode_xml);
        if !id.is_empty() {
            apps.push(ParsedApp {
                id: decode_xml(id),
                executable: executable.filter(|path| !path.is_empty()),
                logo: logo.filter(|path| !path.is_empty()),
            });
        }
        rest = if rest.len() > end {
            &rest[end + 1..]
        } else {
            ""
        };
    }
    apps
}

fn element_text(xml: &str, name: &str) -> Option<String> {
    let tag = element_tag(xml, name)?;
    if tag.ends_with("/>") {
        return None;
    }
    let start = xml.find(&tag)? + tag.len();
    let rest = &xml[start..];
    let end = rest.find('<')?;
    let text = decode_xml(rest[..end].trim());
    (!text.is_empty()).then_some(text)
}

fn element_tag<'a>(xml: &'a str, name: &str) -> Option<&'a str> {
    let mut search_from = 0;
    while let Some(index) = xml[search_from..].find(name) {
        let at = search_from + index;
        let before = xml[..at].chars().next_back();
        let after = xml[at + name.len()..].chars().next();
        if matches!(before, Some('<') | Some(':'))
            && matches!(after, Some(' ' | '>' | '/' | '\n' | '\r' | '\t'))
        {
            let end = xml[at + name.len()..].find('>')?;
            let tag_start = xml[..at].rfind('<')?;
            return Some(&xml[tag_start..=at + name.len() + end]);
        }
        search_from = at + name.len();
    }
    None
}

fn xml_attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let key = format!("{name}=\"");
    let mut rest = tag;
    while let Some(index) = rest.find(&key) {
        let before = rest[..index].chars().next_back();
        if matches!(before, Some(' ' | '\n' | '\r' | '\t' | '<')) {
            let value = &rest[index + key.len()..];
            let end = value.find('"')?;
            return Some(&value[..end]);
        }
        rest = &rest[index + key.len()..];
    }
    None
}

fn decode_xml(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

fn clean_publisher(value: String) -> Option<String> {
    let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if value.is_empty() || value.starts_with("ms-resource:") {
        None
    } else {
        Some(value)
    }
}

fn short_version(value: Option<String>) -> Option<String> {
    let mut value = value?;
    if let Some(index) = value.find(" (") {
        value.truncate(index);
    }
    let value = value.trim().to_string();
    (!value.is_empty()).then_some(value)
}

fn best_logo_variant(dir: &Path, stem: &str) -> Option<PathBuf> {
    LOGO_SUFFIXES.iter().find_map(|suffix| {
        let candidate = dir.join(format!("{stem}{suffix}"));
        usable_png(&candidate).then_some(candidate)
    })
}

fn usable_png(path: &Path) -> bool {
    std::fs::metadata(path)
        .ok()
        .is_some_and(|meta| meta.is_file() && meta.len() > 24 && meta.len() <= LOGO_BYTES)
}

fn package_logo(root: &Path, logo: &str) -> Option<PathBuf> {
    if logo.contains("..") {
        return None;
    }
    let full = root.join(logo.replace('/', "\\"));
    if usable_png(&full) {
        return Some(full);
    }
    let stem = full.file_stem()?.to_string_lossy();
    best_logo_variant(full.parent()?, &stem)
}

fn matching_app<'a>(parsed: &'a ParsedManifest, application: &str) -> Option<&'a ParsedApp> {
    if let Some(app) = parsed
        .apps
        .iter()
        .find(|app| app.id.eq_ignore_ascii_case(application))
    {
        return Some(app);
    }
    if parsed.apps.len() == 1 {
        parsed.apps.first()
    } else {
        None
    }
}

#[cfg(windows)]
fn windows_facts(apps: &[AppEntry]) -> Vec<AppFacts> {
    let _com = ComInit::new();
    let mut guard = CACHE.lock().unwrap_or_else(|error| error.into_inner());
    let cache = guard.get_or_insert_with(Cache::default);
    ensure_indexes(cache);
    apps.iter()
        .map(|app| describe(cache, &app.name, &app.app_id))
        .collect()
}

#[cfg(windows)]
fn describe(cache: &mut Cache, name: &str, app_id: &str) -> AppFacts {
    if let Some((family, application)) = store_parts(app_id) {
        return describe_store(cache, family, application, app_id);
    }
    describe_desktop(cache, name, app_id)
}

#[cfg(windows)]
fn describe_store(cache: &mut Cache, family: &str, application: &str, app_id: &str) -> AppFacts {
    let key = family.to_ascii_lowercase();
    let package = cache.packages.get(&key).cloned();
    let parsed = package.as_ref().and_then(|_| cache.parsed_manifest(&key));
    let app = parsed
        .as_ref()
        .and_then(|parsed| matching_app(parsed, application));
    let root = package.as_ref().map(|package| package.root.clone());
    let exe = root.as_ref().and_then(|root| {
        app.and_then(|app| app.executable.as_deref())
            .and_then(|executable| package_exe(root, executable))
    });
    let logo = root.as_ref().and_then(|root| {
        app.and_then(|app| app.logo.as_deref())
            .and_then(|logo| package_logo(root, logo))
    });
    let location = exe.clone().or(root.filter(|path| path.is_dir()));
    let publisher = parsed.as_ref().and_then(|parsed| parsed.publisher.clone());
    let version = package
        .as_ref()
        .map(|package| package.version.clone())
        .or_else(|| parsed.as_ref().and_then(|parsed| parsed.version.clone()));
    let mut icon = logo.as_ref().and_then(|path| logo_icon(cache, path));
    if icon.is_none() {
        if let Some(path) = exe.as_ref().filter(|path| path.is_file()) {
            icon = file_icon(cache, path);
        }
    }
    if icon.is_none() {
        icon = shell_icon(cache, app_id);
    }
    AppFacts {
        publisher,
        version,
        kind: "store".to_string(),
        location: location.map(|path| path.display().to_string()),
        icon,
    }
}

#[cfg(windows)]
fn describe_desktop(cache: &mut Cache, name: &str, app_id: &str) -> AppFacts {
    let mut path = exe_from_id(cache, app_id);
    if path.is_none() {
        path = cache.links.by_id.get(&app_id.to_ascii_lowercase()).cloned();
    }
    if path.is_none() {
        path = cache.links.by_name.get(&name.to_lowercase()).cloned();
    }
    let (publisher, version) = match &path {
        Some(path) => cache.file_version(path),
        None => (None, None),
    };
    let mut icon = None;
    if let Some(path) = path.as_ref().filter(|path| path.is_file()) {
        icon = file_icon(cache, path);
    }
    if icon.is_none() {
        icon = shell_icon(cache, app_id);
    }
    AppFacts {
        publisher,
        version,
        kind: "desktop".to_string(),
        location: path.map(|path| path.display().to_string()),
        icon,
    }
}

#[cfg(windows)]
fn package_exe(root: &Path, executable: &str) -> Option<PathBuf> {
    if executable.is_empty() || executable.contains("..") {
        return None;
    }
    let path = root.join(executable);
    path.is_file().then_some(path)
}

#[cfg(windows)]
struct ComInit {
    fresh: bool,
}

#[cfg(windows)]
impl ComInit {
    fn new() -> Self {
        let hr = unsafe {
            windows::Win32::System::Com::CoInitializeEx(
                None,
                windows::Win32::System::Com::COINIT_APARTMENTTHREADED,
            )
        };
        Self {
            fresh: hr.is_ok() && hr.0 == 0,
        }
    }
}

#[cfg(windows)]
impl Drop for ComInit {
    fn drop(&mut self) {
        if self.fresh {
            unsafe { windows::Win32::System::Com::CoUninitialize() };
        }
    }
}

#[cfg(windows)]
#[derive(Clone)]
struct Package {
    version: String,
    root: PathBuf,
}

#[cfg(windows)]
#[derive(Default)]
struct Links {
    by_id: HashMap<String, PathBuf>,
    by_name: HashMap<String, PathBuf>,
}

#[cfg(windows)]
struct Cache {
    ready: bool,
    indexes_at: Instant,
    links: Links,
    packages: HashMap<String, Package>,
    manifests: HashMap<String, ParsedManifest>,
    folders: HashMap<String, PathBuf>,
    versions: HashMap<String, (Option<String>, Option<String>)>,
    icons: HashMap<String, Option<String>>,
}

#[cfg(windows)]
impl Default for Cache {
    fn default() -> Self {
        Self {
            ready: false,
            indexes_at: Instant::now(),
            links: Links::default(),
            packages: HashMap::new(),
            manifests: HashMap::new(),
            folders: HashMap::new(),
            versions: HashMap::new(),
            icons: HashMap::new(),
        }
    }
}

#[cfg(windows)]
fn ensure_indexes(cache: &mut Cache) {
    if cache.ready && cache.indexes_at.elapsed() < INDEX_TTL {
        return;
    }
    cache.links = read_links();
    cache.packages = read_packages();
    cache.manifests.clear();
    cache.indexes_at = Instant::now();
    cache.ready = true;
}

#[cfg(windows)]
static CACHE: std::sync::Mutex<Option<Cache>> = std::sync::Mutex::new(None);

#[cfg(windows)]
fn read_packages() -> HashMap<String, Package> {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;

    let mut packages = HashMap::new();
    let Ok(key) = RegKey::predef(HKEY_CURRENT_USER).open_subkey(
        "Software\\Classes\\Local Settings\\Software\\Microsoft\\Windows\\CurrentVersion\\AppModel\\Repository\\Packages",
    ) else {
        return packages;
    };
    for name in key.enum_keys().filter_map(Result::ok) {
        let Ok(sub) = key.open_subkey(&name) else {
            continue;
        };
        let Ok(root) = sub.get_value::<String, _>("PackageRootFolder") else {
            continue;
        };
        let Some((family, version)) = family_and_version(&name) else {
            continue;
        };
        let family = family.to_ascii_lowercase();
        let replace = packages.get(&family).is_none_or(|current: &Package| {
            version_parts(&version) > version_parts(&current.version)
        });
        if replace {
            packages.insert(
                family,
                Package {
                    version,
                    root: PathBuf::from(root),
                },
            );
        }
    }
    packages
}

#[cfg(windows)]
impl Cache {
    fn parsed_manifest(&mut self, family: &str) -> Option<ParsedManifest> {
        if let Some(parsed) = self.manifests.get(family) {
            return Some(parsed.clone());
        }
        let root = self.packages.get(family)?.root.clone();
        let bytes = std::fs::read(root.join("AppxManifest.xml")).ok()?;
        let parsed = parse_manifest(&String::from_utf8_lossy(&bytes));
        self.manifests.insert(family.to_string(), parsed.clone());
        Some(parsed)
    }

    fn file_version(&mut self, path: &Path) -> (Option<String>, Option<String>) {
        let key = path_key(path);
        if let Some(found) = self.versions.get(&key) {
            return found.clone();
        }
        let found = read_file_version(path);
        self.versions.insert(key, found.clone());
        found
    }
}

#[cfg(windows)]
fn path_key(path: &Path) -> String {
    path.to_string_lossy().to_ascii_lowercase()
}

#[cfg(windows)]
fn exe_from_id(cache: &mut Cache, app_id: &str) -> Option<PathBuf> {
    let direct = PathBuf::from(app_id);
    if direct.is_absolute() {
        return Some(direct);
    }
    let rest = app_id.strip_prefix('{')?;
    let (guid, relative) = rest.split_once("}\\")?;
    if guid.len() != 36 || relative.is_empty() {
        return None;
    }
    Some(known_folder_cached(cache, guid)?.join(relative))
}

#[cfg(windows)]
fn known_folder_cached(cache: &mut Cache, guid_text: &str) -> Option<PathBuf> {
    if let Some(path) = cache.folders.get(guid_text) {
        return Some(path.clone());
    }
    let guid = windows::core::GUID::try_from(guid_text).ok()?;
    let path = known_folder(&guid)?;
    cache.folders.insert(guid_text.to_string(), path.clone());
    Some(path)
}

#[cfg(windows)]
fn known_folder(guid: &windows::core::GUID) -> Option<PathBuf> {
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::{KF_FLAG_DEFAULT, SHGetKnownFolderPath};

    let wide = unsafe { SHGetKnownFolderPath(guid, KF_FLAG_DEFAULT, None) }.ok()?;
    let text = unsafe { wide.to_string() };
    unsafe { CoTaskMemFree(Some(wide.as_ptr().cast())) };
    let text = text.ok()?;
    (!text.is_empty()).then(|| PathBuf::from(text))
}

#[cfg(windows)]
fn read_links() -> Links {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, CoCreateInstance, IPersistFile, STGM_READ,
    };
    use windows::Win32::UI::Shell::{
        FOLDERID_CommonPrograms, FOLDERID_Programs, IShellLinkW, ShellLink,
    };
    use windows::core::{HSTRING, Interface};

    let mut links = Links::default();
    let _com = ComInit::new();
    let Ok(link) =
        (unsafe { CoCreateInstance::<_, IShellLinkW>(&ShellLink, None, CLSCTX_INPROC_SERVER) })
    else {
        return links;
    };
    let Ok(file) = link.cast::<IPersistFile>() else {
        return links;
    };
    let mut paths = Vec::new();
    for guid in [&FOLDERID_Programs, &FOLDERID_CommonPrograms] {
        if let Some(dir) = known_folder(guid) {
            collect_lnks(&dir, &mut paths, 0);
        }
    }
    for path in paths {
        let wide: Vec<u16> = path.as_os_str().encode_wide().collect();
        if unsafe { file.Load(&HSTRING::from_wide(&wide), STGM_READ) }.is_err() {
            continue;
        }
        let Some(target) = link_target(&link) else {
            continue;
        };
        if let Some(app_id) = link_aumid(&link) {
            links
                .by_id
                .entry(app_id.to_ascii_lowercase())
                .or_insert_with(|| target.clone());
        }
        let stem = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_lowercase());
        if let Some(stem) = stem {
            links.by_name.entry(stem).or_insert(target);
        }
    }
    links
}

#[cfg(windows)]
fn collect_lnks(dir: &Path, out: &mut Vec<PathBuf>, depth: u8) {
    if depth > 6 || out.len() > 4000 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_lnks(&path, out, depth + 1);
        } else if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("lnk"))
        {
            out.push(path);
        }
    }
}

#[cfg(windows)]
fn link_target(link: &windows::Win32::UI::Shell::IShellLinkW) -> Option<PathBuf> {
    use windows::Win32::UI::Shell::{GPFIDL_DEFAULT, ILFree, SHGetPathFromIDListEx};

    let pidl = unsafe { link.GetIDList() }.ok()?;
    if pidl.is_null() {
        return None;
    }
    let mut buffer = vec![0u16; 1024];
    let ok = unsafe { SHGetPathFromIDListEx(pidl, &mut buffer, GPFIDL_DEFAULT) };
    unsafe { ILFree(Some(pidl)) };
    if !ok.as_bool() {
        return None;
    }
    let text = wide_text(&buffer);
    (!text.is_empty()).then(|| PathBuf::from(text))
}

#[cfg(windows)]
fn link_aumid(link: &windows::Win32::UI::Shell::IShellLinkW) -> Option<String> {
    use windows::Win32::Foundation::PROPERTYKEY;
    use windows::Win32::System::Com::StructuredStorage::PropVariantClear;
    use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
    use windows::core::Interface;

    const PKEY_APP_USER_MODEL_ID: PROPERTYKEY = PROPERTYKEY {
        fmtid: windows::core::GUID::from_u128(0x9f4c2855_9f79_4b39_a8d0_e1d42de1d5f3),
        pid: 5,
    };

    let store: IPropertyStore = link.cast().ok()?;
    let mut value = unsafe { store.GetValue(&PKEY_APP_USER_MODEL_ID) }.ok()?;
    let text = prop_string(&value);
    unsafe {
        let _ = PropVariantClear(&mut value);
    }
    text
}

#[cfg(windows)]
fn prop_string(
    value: &windows::Win32::System::Com::StructuredStorage::PROPVARIANT,
) -> Option<String> {
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::System::Com::StructuredStorage::PropVariantToStringAlloc;

    let wide = unsafe { PropVariantToStringAlloc(value) }.ok()?;
    let text = unsafe { wide.to_string() };
    unsafe { CoTaskMemFree(Some(wide.as_ptr().cast())) };
    let text = text.ok()?.trim().to_string();
    (!text.is_empty()).then_some(text)
}

#[cfg(windows)]
fn wide_text(buffer: &[u16]) -> String {
    let end = buffer
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(buffer.len());
    String::from_utf16_lossy(&buffer[..end]).trim().to_string()
}

#[cfg(windows)]
fn read_file_version(path: &Path) -> (Option<String>, Option<String>) {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;

    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let name = PCWSTR(wide.as_ptr());
    let size = unsafe { GetFileVersionInfoSizeW(name, std::ptr::null_mut()) };
    if size == 0 {
        return (None, None);
    }
    let mut block = vec![0u8; size as usize];
    if unsafe { GetFileVersionInfoW(name, 0, size, block.as_mut_ptr().cast()) } == 0 {
        return (None, None);
    }
    let (lang, page) = translation(&block);
    let company = query_version(
        &block,
        &format!("\\StringFileInfo\\{lang:04x}{page:04x}\\CompanyName"),
    )
    .or_else(|| query_version(&block, "\\StringFileInfo\\040904b0\\CompanyName"))
    .and_then(clean_publisher);
    let version = short_version(
        query_version(
            &block,
            &format!("\\StringFileInfo\\{lang:04x}{page:04x}\\FileVersion"),
        )
        .or_else(|| query_version(&block, "\\StringFileInfo\\040904b0\\FileVersion"))
        .or_else(|| {
            query_version(
                &block,
                &format!("\\StringFileInfo\\{lang:04x}{page:04x}\\ProductVersion"),
            )
        }),
    );
    (company, version)
}

#[cfg(windows)]
fn translation(block: &[u8]) -> (u16, u16) {
    use windows::core::PCWSTR;

    let key: Vec<u16> = "\\VarFileInfo\\Translation"
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let mut ptr = std::ptr::null_mut();
    let mut len = 0u32;
    let ok = unsafe {
        VerQueryValueW(
            block.as_ptr().cast(),
            PCWSTR(key.as_ptr()),
            &mut ptr,
            &mut len,
        )
    };
    if ok == 0 || ptr.is_null() || len < 4 {
        return (0x0409, 0x04b0);
    }
    unsafe {
        let lang = (ptr as *const u16).read_unaligned();
        let page = (ptr as *const u16).add(1).read_unaligned();
        (lang, page)
    }
}

#[cfg(windows)]
fn query_version(block: &[u8], path: &str) -> Option<String> {
    use windows::core::PCWSTR;

    let key: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
    let mut ptr = std::ptr::null_mut();
    let mut len = 0u32;
    let ok = unsafe {
        VerQueryValueW(
            block.as_ptr().cast(),
            PCWSTR(key.as_ptr()),
            &mut ptr,
            &mut len,
        )
    };
    if ok == 0 || ptr.is_null() {
        return None;
    }
    let mut units = Vec::new();
    unsafe {
        let mut cursor = ptr as *const u16;
        for _ in 0..512 {
            let unit = cursor.read();
            if unit == 0 {
                break;
            }
            units.push(unit);
            cursor = cursor.add(1);
        }
    }
    let text = String::from_utf16_lossy(&units).trim().to_string();
    (!text.is_empty()).then_some(text)
}

#[cfg(windows)]
#[link(name = "version")]
unsafe extern "system" {
    fn GetFileVersionInfoSizeW(filename: windows::core::PCWSTR, handle: *mut u32) -> u32;
    fn GetFileVersionInfoW(
        filename: windows::core::PCWSTR,
        handle: u32,
        len: u32,
        data: *mut core::ffi::c_void,
    ) -> i32;
    fn VerQueryValueW(
        block: *const core::ffi::c_void,
        sub: windows::core::PCWSTR,
        buffer: *mut *mut core::ffi::c_void,
        len: *mut u32,
    ) -> i32;
}

#[cfg(windows)]
fn logo_icon(cache: &mut Cache, path: &Path) -> Option<String> {
    cached_icon(cache, format!("logo:{}", path_key(path)), || png_file(path))
}

#[cfg(windows)]
fn file_icon(cache: &mut Cache, path: &Path) -> Option<String> {
    cached_icon(cache, format!("file:{}", path_key(path)), || file_png(path))
}

#[cfg(windows)]
fn shell_icon(cache: &mut Cache, app_id: &str) -> Option<String> {
    cached_icon(
        cache,
        format!("shell:{}", app_id.to_ascii_lowercase()),
        || shell_png(app_id),
    )
}

#[cfg(windows)]
fn cached_icon(
    cache: &mut Cache,
    key: String,
    load: impl FnOnce() -> Option<Vec<u8>>,
) -> Option<String> {
    if let Some(found) = cache.icons.get(&key) {
        return found.clone();
    }
    let icon = load().map(|bytes| data_url(&bytes));
    cache.icons.insert(key, icon.clone());
    icon
}

#[cfg(windows)]
fn png_file(path: &Path) -> Option<Vec<u8>> {
    let bytes = std::fs::read(path).ok()?;
    (bytes.len() > 8
        && bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        && bytes.len() <= LOGO_BYTES as usize)
        .then_some(bytes)
}

#[cfg(windows)]
fn data_url(bytes: &[u8]) -> String {
    use base64::Engine;
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

#[cfg(windows)]
fn file_png(path: &Path) -> Option<Vec<u8>> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::UI::Shell::SHFILEINFOW;
    use windows::Win32::UI::WindowsAndMessaging::DestroyIcon;
    use windows::core::PCWSTR;

    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut info = SHFILEINFOW::default();
    let got = unsafe {
        SHGetFileInfoW(
            PCWSTR(wide.as_ptr()),
            0,
            &mut info,
            std::mem::size_of::<SHFILEINFOW>() as u32,
            0x100,
        )
    };
    if got == 0 || info.hIcon.is_invalid() {
        return None;
    }
    let png = hicon_to_png(info.hIcon);
    unsafe {
        let _ = DestroyIcon(info.hIcon);
    }
    png
}

#[cfg(windows)]
#[link(name = "shell32")]
unsafe extern "system" {
    fn SHGetFileInfoW(
        path: windows::core::PCWSTR,
        attributes: u32,
        info: *mut windows::Win32::UI::Shell::SHFILEINFOW,
        size: u32,
        flags: u32,
    ) -> usize;
}

#[cfg(windows)]
fn shell_png(app_id: &str) -> Option<Vec<u8>> {
    use windows::Win32::Foundation::SIZE;
    use windows::Win32::Graphics::Gdi::{DeleteObject, HGDIOBJ};
    use windows::Win32::UI::Shell::{
        IShellItem, IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_BIGGERSIZEOK,
        SIIGBF_ICONONLY,
    };
    use windows::core::{HSTRING, Interface};

    let parsing = format!("shell:AppsFolder\\{app_id}");
    let item: IShellItem = unsafe {
        SHCreateItemFromParsingName(
            &HSTRING::from(parsing),
            None::<&windows::Win32::System::Com::IBindCtx>,
        )
    }
    .ok()?;
    let factory: IShellItemImageFactory = item.cast().ok()?;
    let bitmap = unsafe {
        factory.GetImage(
            SIZE { cx: 32, cy: 32 },
            windows::Win32::UI::Shell::SIIGBF(SIIGBF_ICONONLY.0 | SIIGBF_BIGGERSIZEOK.0),
        )
    }
    .ok()?;
    let png = hbitmap_to_png(bitmap, true);
    unsafe {
        let _ = DeleteObject(HGDIOBJ::from(bitmap));
    }
    png
}

#[cfg(windows)]
fn hicon_to_png(icon: windows::Win32::UI::WindowsAndMessaging::HICON) -> Option<Vec<u8>> {
    use windows::Win32::Graphics::Gdi::{DeleteObject, HGDIOBJ};
    use windows::Win32::UI::WindowsAndMessaging::{GetIconInfo, ICONINFO};

    let mut info = ICONINFO::default();
    unsafe { GetIconInfo(icon, &mut info).ok()? };
    let png = if info.hbmColor.is_invalid() {
        None
    } else {
        hbitmap_to_png(info.hbmColor, false)
    };
    if !info.hbmColor.is_invalid() {
        unsafe {
            let _ = DeleteObject(HGDIOBJ::from(info.hbmColor));
        }
    }
    if !info.hbmMask.is_invalid() {
        unsafe {
            let _ = DeleteObject(HGDIOBJ::from(info.hbmMask));
        }
    }
    png
}

#[cfg(windows)]
fn hbitmap_to_png(
    bitmap: windows::Win32::Graphics::Gdi::HBITMAP,
    unpremultiply: bool,
) -> Option<Vec<u8>> {
    use windows::Win32::Graphics::Gdi::{
        BITMAP, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, GetDC, GetDIBits, GetObjectW,
        HGDIOBJ, ReleaseDC,
    };

    let mut description = BITMAP::default();
    let written = unsafe {
        GetObjectW(
            HGDIOBJ::from(bitmap),
            std::mem::size_of::<BITMAP>() as i32,
            Some((&mut description as *mut BITMAP).cast()),
        )
    };
    if written == 0 {
        return None;
    }
    let width = description.bmWidth;
    let height = description.bmHeight.unsigned_abs();
    if width <= 0 || height == 0 || width > 256 || height > 256 {
        return None;
    }
    let mut info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -(height as i32),
            biPlanes: 1,
            biBitCount: 32,
            ..BITMAPINFOHEADER::default()
        },
        ..BITMAPINFO::default()
    };
    let mut pixels = vec![0u8; width as usize * height as usize * 4];
    let hdc = unsafe { GetDC(None) };
    if hdc.is_invalid() {
        return None;
    }
    let lines = unsafe {
        GetDIBits(
            hdc,
            bitmap,
            0,
            height,
            Some(pixels.as_mut_ptr().cast()),
            &mut info,
            DIB_RGB_COLORS,
        )
    };
    unsafe {
        ReleaseDC(None, hdc);
    }
    if lines <= 0 {
        return None;
    }
    for chunk in pixels.chunks_exact_mut(4) {
        chunk.swap(0, 2);
    }
    if pixels.chunks_exact(4).all(|chunk| chunk[3] == 0) {
        for chunk in pixels.chunks_exact_mut(4) {
            chunk[3] = 255;
        }
    } else if unpremultiply {
        for chunk in pixels.chunks_exact_mut(4) {
            let alpha = chunk[3] as u32;
            if alpha == 0 || alpha == 255 {
                continue;
            }
            for channel in &mut chunk[..3] {
                *channel = ((*channel as u32) * 255 / alpha).min(255) as u8;
            }
        }
    }
    encode_rgba(width as u32, height, &pixels)
}

#[cfg(windows)]
fn encode_rgba(width: u32, height: u32, pixels: &[u8]) -> Option<Vec<u8>> {
    use image::ImageEncoder;

    let mut output = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut output);
    encoder
        .write_image(pixels, width, height, image::ExtendedColorType::Rgba8)
        .ok()?;
    Some(output)
}

#[cfg(test)]
mod tests {
    use super::{best_logo_variant, family_and_version, parse_manifest};

    #[test]
    fn reads_a_package_full_name() {
        assert_eq!(
            family_and_version("Microsoft.WindowsCalculator_11.2607.0.0_x64__8wekyb3d8bbwe"),
            Some((
                "Microsoft.WindowsCalculator_8wekyb3d8bbwe".into(),
                "11.2607.0.0".into()
            ))
        );
        assert_eq!(
            family_and_version("Name_1.2.3.4_neutral_en-us_publisherid"),
            Some(("Name_publisherid".into(), "1.2.3.4".into()))
        );
        assert!(family_and_version("Chrome").is_none());
    }

    #[test]
    fn reads_publisher_and_logo_from_a_manifest() {
        let parsed = parse_manifest(
            r#"<Package>
              <Identity Name="Calc" Version="11.2607.0.0" Publisher="CN=Microsoft" />
              <Properties><PublisherDisplayName>AT&amp;T</PublisherDisplayName></Properties>
              <Applications>
                <Application Id="App" Executable="CalculatorApp.exe">
                  <uap:VisualElements Square44x44Logo="Assets\CalculatorAppList.png" />
                </Application>
                <Application Id="Other" Executable="Other.exe" />
              </Applications>
            </Package>"#,
        );
        assert_eq!(parsed.publisher.as_deref(), Some("AT&T"));
        assert_eq!(parsed.version.as_deref(), Some("11.2607.0.0"));
        assert_eq!(parsed.apps.len(), 2);
        assert_eq!(parsed.apps[0].id, "App");
        assert_eq!(
            parsed.apps[0].executable.as_deref(),
            Some("CalculatorApp.exe")
        );
        assert_eq!(
            parsed.apps[0].logo.as_deref(),
            Some("Assets\\CalculatorAppList.png")
        );
    }

    #[test]
    fn picks_the_small_square_logo() {
        let dir = std::env::temp_dir().join(format!("ce-desktop-logo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("AppList.scale-200.png"),
            b"not-a-real-png-but-long-enough!!",
        )
        .unwrap();
        std::fs::write(
            dir.join("AppList.targetsize-32.png"),
            b"not-a-real-png-but-long-enough!!",
        )
        .unwrap();
        std::fs::write(
            dir.join("AppList.targetsize-32_contrast-black.png"),
            b"not-a-real-png-but-long-enough!!",
        )
        .unwrap();
        let found = best_logo_variant(&dir, "AppList").unwrap();
        assert_eq!(found.file_name().unwrap(), "AppList.targetsize-32.png");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(windows)]
    #[test]
    fn installed_apps_include_an_icon_and_a_publisher() {
        let mut apps = crate::launcher::apps::snapshot();
        if apps.is_empty() {
            crate::launcher::apps::refresh();
            apps = crate::launcher::apps::snapshot();
        }
        if apps.is_empty() {
            return;
        }
        let facts = super::facts_for(&apps);
        assert_eq!(facts.len(), apps.len());
        assert!(
            facts.iter().any(|facts| facts.icon.is_some()),
            "expected an icon"
        );
        assert!(
            facts.iter().any(|facts| facts.publisher.is_some()),
            "expected a publisher"
        );
        assert!(
            facts
                .iter()
                .all(|facts| facts.kind == "store" || facts.kind == "desktop")
        );
        if let Some(facts) = named(&apps, &facts, "7-Zip File Manager") {
            assert!(facts.icon.is_some(), "7-Zip icon");
            let publisher = facts.publisher.as_deref().unwrap_or("");
            let location = facts.location.as_deref().unwrap_or("");
            assert!(
                publisher.to_lowercase().contains("pavlov") || location.contains("7zFM"),
                "publisher={publisher} location={location}"
            );
        }
        if let Some(facts) = named(&apps, &facts, "Calculator") {
            assert_eq!(facts.kind, "store");
            assert!(facts.icon.is_some(), "Calculator icon");
            let publisher = facts.publisher.as_deref().unwrap_or("");
            assert!(publisher.contains("Microsoft"), "publisher={publisher}");
        }
    }

    #[cfg(windows)]
    fn named<'a>(
        apps: &'a [crate::launcher::apps::AppEntry],
        facts: &'a [super::AppFacts],
        name: &str,
    ) -> Option<&'a super::AppFacts> {
        apps.iter()
            .zip(facts)
            .find(|(app, _)| app.name == name)
            .map(|(_, facts)| facts)
    }
}
