//! Pure Rust implementation of the ReadMD plugin & sandbox manager.
//!
//! Line-level port of `src/readmd_modules/plugin_manager.py`: the curated
//! catalog (`PLUGIN_SPECS` + `plugin_catalog.extend_catalog`), the
//! `plugins.json` manifest, the exclusive-capability activation rules and the
//! sandbox artefact removal used by `/api/plugins/*`.
//!
//! The Python module installs packages with `pip` into
//! `DATA_DIR/plugins/site-packages`, always inside its own process for a frozen
//! build (`_run_pip` → `runpy.run_module('pip')`, `plugin_manager.py:661-737`).
//! The Rust kernel owns the *contract* (manifest shape, enable/disable
//! arbitration, honest uninstall results) but has no bundled `pip` to run and
//! spawns nothing, so [`install_plugin_async`] publishes the exact task state
//! Python reserves for that case (`pip_unavailable`,
//! `plugin_manager.py:873-883`) instead of inventing one or pretending.

use crate::App;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

// -------------------------------------------------------------- catalog

/// One `PLUGIN_SPECS` entry after `plugin_catalog.extend_catalog()` ran.
pub struct PluginSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub name_key: &'static str,
    pub desc_key: &'static str,
    pub package: &'static str,
    pub pip_args: &'static [&'static str],
    pub import_name: &'static str,
    pub category: &'static str,
    pub weight: &'static str,
    pub approx_size: &'static str,
    pub cache_type: &'static str,
    pub cache_paths: &'static [&'static str],
    pub capability: &'static str,
    pub requires_model: bool,
    pub runtime_connected: bool,
}

/// `f'plugin.{pid}.name'` / `f'plugin.{pid}.desc'` — built at compile time from
/// the literal id so the two keys always agree with the plugin id, exactly like
/// `plugin_catalog.extend_catalog()` does in Python.
macro_rules! spec {
    (
        $id:literal,
        $name:literal,
        $package:literal,
        $pip_args:expr,
        $import_name:expr,
        $category:expr,
        $weight:expr,
        $approx_size:expr,
        $cache_type:expr,
        $cache_paths:expr,
        $capability:expr,
        $requires_model:expr,
        $runtime_connected:expr,
    ) => {
        PluginSpec {
            id: $id,
            name: $name,
            name_key: concat!("plugin.", $id, ".name"),
            desc_key: concat!("plugin.", $id, ".desc"),
            package: $package,
            pip_args: $pip_args,
            import_name: $import_name,
            category: $category,
            weight: $weight,
            approx_size: $approx_size,
            cache_type: $cache_type,
            cache_paths: $cache_paths,
            capability: $capability,
            requires_model: $requires_model,
            runtime_connected: $runtime_connected,
        }
    };
}

/// `PLUGIN_SPECS` in the legacy declaration order: the eight base specs in their
/// dict-literal order (`easyocr` first, `plugin_manager.py:48-153`), then the six
/// `plugin_catalog.extend_catalog` additions in theirs (`pymupdf4llm`, `docling`,
/// `faster_whisper`, `markdownify`, `trafilatura`, `charset_normalizer`;
/// `plugin_catalog.py:17-24`).  The order is what the UI cards show.
pub const PLUGIN_SPECS: &[PluginSpec] = &[
    spec!(
        "easyocr",
        "Easyocr",
        "easyocr",
        &["easyocr"],
        "easyocr",
        "ocr",
        "heavy",
        "~150MB",
        "dir_has_files",
        &["~/.EasyOCR/model"],
        "ocr",
        true,
        true,
    ),
    spec!(
        "pylatexenc",
        "Pylatexenc",
        "pylatexenc",
        &["pylatexenc"],
        "pylatexenc",
        "latex",
        "light",
        "~1MB",
        "installed",
        &[],
        "latex",
        false,
        false,
    ),
    spec!(
        "rapidocr",
        "Rapidocr",
        "rapidocr_onnxruntime",
        &["rapidocr_onnxruntime"],
        "rapidocr_onnxruntime",
        "ocr",
        "light",
        "~17MB",
        "installed",
        &[],
        "ocr",
        false,
        true,
    ),
    spec!(
        "rapid_table",
        "Rapid Table",
        "rapid_table",
        &["rapid_table>=0.1.3,<=0.3.0"],
        "rapid_table",
        "document",
        "light",
        "~15MB",
        "installed",
        &[],
        "table",
        false,
        true,
    ),
    spec!(
        "whisper",
        "Whisper",
        "openai-whisper",
        &["openai-whisper"],
        "whisper",
        "audio",
        "heavy",
        "~150MB",
        "dir_has_files",
        &["~/.cache/whisper"],
        "audio",
        true,
        true,
    ),
    spec!(
        "jieba",
        "Jieba",
        "jieba",
        &["jieba"],
        "jieba",
        "text",
        "light",
        "~18MB",
        "installed",
        &[],
        "keywords",
        false,
        false,
    ),
    spec!(
        "pygments",
        "Pygments",
        "pygments",
        &["pygments"],
        "pygments",
        "code",
        "light",
        "~12MB",
        "installed",
        &[],
        "highlight",
        false,
        false,
    ),
    spec!(
        "pandoc_bridge",
        "Pandoc Bridge",
        // plugin_catalog overrides pypandoc -> pypandoc-binary and ~5MB -> ~35MB
        // because the bridge is unusable without the bundled executable.
        "pypandoc-binary",
        &["pypandoc-binary"],
        "pypandoc",
        "tools",
        "light",
        "~35MB",
        "installed",
        &[],
        "document",
        false,
        false,
    ),
    spec!(
        "pymupdf4llm",
        "PyMuPDF4LLM",
        "pymupdf4llm",
        &["pymupdf4llm"],
        "pymupdf4llm",
        "document",
        "light",
        "~30MB",
        "installed",
        &[],
        "pdf",
        false,
        // `CONNECTED_PLUGINS` (`plugin_manager.py:158-162`) lists pymupdf4llm, so
        // `'runtime_connected': pid in CONNECTED_PLUGINS` (`:236`) is True: the
        // default PDF provider really is wired into the pipeline.
        true,
    ),
    spec!(
        "docling",
        "Docling",
        "docling",
        &["docling"],
        "docling",
        "document",
        "heavy",
        "~500MB+",
        "installed",
        &[],
        "pdf",
        true,
        true,
    ),
    spec!(
        "faster_whisper",
        "Faster Whisper",
        "faster-whisper",
        &["faster-whisper"],
        "faster_whisper",
        "audio",
        "heavy",
        "~150MB+",
        "installed",
        &[],
        "audio",
        true,
        true,
    ),
    spec!(
        "markdownify",
        "Markdownify",
        "markdownify",
        &["markdownify"],
        "markdownify",
        "document",
        "light",
        "~2MB",
        "installed",
        &[],
        "web",
        false,
        true,
    ),
    spec!(
        "trafilatura",
        "Trafilatura",
        "trafilatura",
        &["trafilatura"],
        "trafilatura",
        "document",
        "light",
        "~10MB",
        "installed",
        &[],
        "web",
        false,
        true,
    ),
    spec!(
        "charset_normalizer",
        "Charset Normalizer",
        "charset-normalizer",
        &["charset-normalizer"],
        "charset_normalizer",
        "text",
        "light",
        "~1MB",
        "installed",
        &[],
        "encoding",
        false,
        true,
    ),
];

/// `CAPABILITIES`: the unit of exclusive activation, in declaration order.
pub const CAPABILITIES: &[(&str, &[&str])] = &[
    ("ocr", &["rapidocr", "easyocr"]),
    ("table", &["rapid_table"]),
    ("pdf", &["pymupdf4llm", "docling"]),
    ("audio", &["whisper", "faster_whisper"]),
    ("web", &["markdownify", "trafilatura"]),
    ("document", &["pandoc_bridge"]),
    ("latex", &["pylatexenc"]),
    ("keywords", &["jieba"]),
    ("highlight", &["pygments"]),
    ("encoding", &["charset_normalizer"]),
];

/// `DEFAULT_ENABLED`: pipeline-connected heavyweight providers ship switched
/// on; everything else is the user's decision.
pub const DEFAULT_ENABLED: &[&str] = &["easyocr", "rapidocr", "rapid_table", "whisper"];

pub fn plugin_spec(plugin_id: &str) -> Option<&'static PluginSpec> {
    PLUGIN_SPECS.iter().find(|spec| spec.id == plugin_id)
}

pub fn capability_providers(capability: &str) -> &'static [&'static str] {
    CAPABILITIES
        .iter()
        .find(|(name, _)| *name == capability)
        .map(|(_, providers)| *providers)
        .unwrap_or(&[])
}

/// `spec['alternatives']` — every sibling provider of the same capability.
pub fn alternatives(plugin_id: &str) -> Vec<&'static str> {
    match plugin_spec(plugin_id) {
        Some(spec) => capability_providers(spec.capability)
            .iter()
            .copied()
            .filter(|other| *other != plugin_id)
            .collect(),
        None => Vec::new(),
    }
}

// -------------------------------------------------------------- sandbox paths

pub struct Sandbox {
    pub plugins_root: PathBuf,
    pub site_packages: PathBuf,
    pub bin_dir: PathBuf,
    pub manifest: PathBuf,
    pub home_dir: PathBuf,
}

impl Sandbox {
    pub fn for_app(app: &App) -> Sandbox {
        Sandbox::new(&app.paths.data_dir)
    }

    pub fn new(data_dir: &Path) -> Sandbox {
        let plugins_root = data_dir.join("plugins");
        Sandbox {
            manifest: plugins_root.join("plugins.json"),
            site_packages: plugins_root.join("site-packages"),
            bin_dir: plugins_root.join("bin"),
            home_dir: crate::paths::home_dir(),
            plugins_root,
        }
    }

    /// `_ensure_dirs()` — never fatal, the manager degrades to an empty view.
    pub fn ensure_dirs(&self) {
        let _ = fs::create_dir_all(&self.site_packages);
        let _ = fs::create_dir_all(&self.bin_dir);
    }
}

// -------------------------------------------------------------- install tasks

/// `_install_tasks[plugin_id]`, the transient progress/error state.
#[derive(Debug, Clone)]
pub struct InstallTask {
    pub status: String,
    pub error: String,
    pub error_code: String,
    pub error_detail: String,
    pub last_log: String,
    pub progress: i64,
}

impl Default for InstallTask {
    fn default() -> InstallTask {
        InstallTask {
            status: String::new(),
            error: String::new(),
            error_code: String::new(),
            error_detail: String::new(),
            last_log: String::new(),
            progress: 0,
        }
    }
}

fn tasks() -> &'static Mutex<BTreeMap<String, InstallTask>> {
    static TASKS: OnceLock<Mutex<BTreeMap<String, InstallTask>>> = OnceLock::new();
    TASKS.get_or_init(Default::default)
}

/// `_runtime_status` / `_active_uses`: the live worker reports both.
fn runtime_status() -> &'static Mutex<BTreeMap<String, Value>> {
    static RUNTIME: OnceLock<Mutex<BTreeMap<String, Value>>> = OnceLock::new();
    RUNTIME.get_or_init(Default::default)
}

fn active_uses() -> &'static Mutex<BTreeMap<String, i64>> {
    static USES: OnceLock<Mutex<BTreeMap<String, i64>>> = OnceLock::new();
    USES.get_or_init(Default::default)
}

/// `_set_task(...)`.
pub fn set_task(
    plugin_id: &str,
    status: &str,
    progress: i64,
    last_log: &str,
    error_code: &str,
    error_detail: &[String],
) {
    let detail = error_detail.join("\n");
    let detail = tail_chars(&detail, 4000);
    if let Ok(mut map) = tasks().lock() {
        map.insert(
            plugin_id.to_string(),
            InstallTask {
                status: status.to_string(),
                error: error_code.to_string(),
                error_code: error_code.to_string(),
                error_detail: detail,
                last_log: last_log.to_string(),
                progress,
            },
        );
    }
}

fn tail_chars(text: &str, max_chars: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max_chars {
        return text.to_string();
    }
    chars[chars.len() - max_chars..].iter().collect()
}

/// Lets the (future) native worker publish the same shape `run_plugin` reads.
pub fn report_runtime_status(plugin_id: &str, value: Value) {
    if let Ok(mut map) = runtime_status().lock() {
        map.insert(plugin_id.to_string(), value);
    }
}

pub fn clear_runtime_status(plugin_id: &str) {
    if let Ok(mut map) = runtime_status().lock() {
        map.remove(plugin_id);
    }
}

pub fn task_of(plugin_id: &str) -> Option<InstallTask> {
    tasks().lock().ok().and_then(|map| map.get(plugin_id).cloned())
}

pub fn installing(plugin_id: &str) -> bool {
    task_of(plugin_id).map(|task| task.status == "installing").unwrap_or(false)
}

// -------------------------------------------------------------- manifest

/// `_read_manifest_data()` — non-object entries are dropped, failures are empty.
pub fn read_manifest_data(sandbox: &Sandbox) -> Map<String, Value> {
    match fs::read_to_string(&sandbox.manifest)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
    {
        Some(Value::Object(map)) => map
            .into_iter()
            .filter(|(_, value)| value.is_object())
            .collect::<Map<String, Value>>(),
        _ => Map::new(),
    }
}

/// `save_manifest()` — only the three persisted keys survive.
pub fn save_manifest(sandbox: &Sandbox, manifest: &Map<String, Value>) {
    sandbox.ensure_dirs();
    let mut to_save = Map::new();
    for (pid, info) in manifest {
        let enabled = info.get("enabled").and_then(Value::as_bool).unwrap_or(false);
        let uninstalled = info.get("uninstalled").and_then(Value::as_bool).unwrap_or(false);
        let version = info
            .get("version")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        to_save.insert(
            pid.clone(),
            json!({ "enabled": enabled, "uninstalled": uninstalled, "version": version }),
        );
    }
    let text = serde_json::to_string_pretty(&Value::Object(to_save)).unwrap_or_else(|_| "{}".into());
    let tmp = sandbox.manifest.with_extension("json.tmp");
    if fs::write(&tmp, text.as_bytes()).is_ok() {
        if fs::rename(&tmp, &sandbox.manifest).is_err() {
            if fs::copy(&tmp, &sandbox.manifest).is_ok() {
                let _ = fs::remove_file(&tmp);
            }
        }
    }
}

// -------------------------------------------------------------- detection

/// `_normalize_dist_name`.
pub fn normalize_dist_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut previous_sep = false;
    for ch in name.chars() {
        if ch == '-' || ch == '_' || ch == '.' {
            if !previous_sep {
                out.push('-');
                previous_sep = true;
            }
        } else {
            out.push(ch.to_ascii_lowercase());
            previous_sep = false;
        }
    }
    out
}

/// `_dist_info_prefixes`.
pub fn dist_info_prefixes(package: &str) -> (String, String) {
    let normalized = normalize_dist_name(package);
    (
        format!("{normalized}-"),
        format!("{}-", normalized.replace('-', "_")),
    )
}

/// `_sandbox_metadata_exists`: did a pip run ever finish in the sandbox?
pub fn sandbox_metadata_exists(sandbox: &Sandbox, package: &str) -> bool {
    let (first, second) = dist_info_prefixes(package);
    match fs::read_dir(&sandbox.site_packages) {
        Ok(entries) => entries.flatten().any(|entry| {
            let lowered = entry.file_name().to_string_lossy().to_ascii_lowercase();
            (lowered.ends_with(".dist-info") || lowered.ends_with(".egg-info"))
                && (lowered.starts_with(&first) || lowered.starts_with(&second))
        }),
        Err(_) => false,
    }
}

/// `is_plugin_installed` step 1: the sandbox artefacts only.
///
/// Python additionally probes the host interpreter with
/// `importlib.util.find_spec`; a Rust-native kernel has no host Python and the
/// zero-Python-dependency gate forbids adding that probe, so an
/// environment-provided package is reported as not installed.
pub fn sandbox_plugin_installed(sandbox: &Sandbox, spec: &PluginSpec) -> bool {
    let pkg_file = sandbox.site_packages.join(format!("{}.py", spec.import_name));
    if pkg_file.is_file() {
        return true;
    }
    let pkg_dir = sandbox.site_packages.join(spec.import_name);
    pkg_dir.is_dir() && sandbox_metadata_exists(sandbox, spec.package)
}

pub fn is_plugin_installed(sandbox: &Sandbox, plugin_id: &str) -> bool {
    let spec = match plugin_spec(plugin_id) {
        Some(spec) => spec,
        None => return false,
    };
    sandbox.ensure_dirs();
    if read_manifest_data(sandbox)
        .get(plugin_id)
        .and_then(|entry| entry.get("uninstalled"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return false;
    }
    sandbox_plugin_installed(sandbox, spec)
}

fn entry_bool(entry: Option<&Value>, key: &str) -> Option<bool> {
    entry.and_then(|value| value.get(key)).and_then(Value::as_bool)
}

fn entry_str<'a>(entry: Option<&'a Value>, key: &str) -> Option<&'a str> {
    entry.and_then(|value| value.get(key)).and_then(Value::as_str)
}

/// `_check_model_cached`.
pub fn check_model_cached(sandbox: &Sandbox, plugin_id: &str) -> bool {
    let spec = match plugin_spec(plugin_id) {
        Some(spec) => spec,
        None => return false,
    };
    if spec.cache_type == "installed" {
        return is_plugin_installed(sandbox, plugin_id);
    }
    if spec.cache_type == "dirs_any" {
        return spec
            .cache_paths
            .iter()
            .any(|path| expand_user(sandbox, path).is_dir());
    }
    if spec.cache_type == "dir_has_files" {
        for path in spec.cache_paths {
            let dir = expand_user(sandbox, path);
            if dir.is_dir() {
                if let Ok(mut entries) = fs::read_dir(&dir) {
                    if entries.next().is_some() {
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn expand_user(sandbox: &Sandbox, path: &str) -> PathBuf {
    match path.strip_prefix("~/") {
        Some(rest) => sandbox.home_dir.join(rest),
        None => PathBuf::from(path),
    }
}

/// `load_manifest()` — the per-plugin view the UI renders.
/// What the Rust kernel itself provides for a plugin's capability.  The
/// plugin center shows these as built in instead of offering a pip install
/// this build can never run (`pip_unavailable`).
pub fn native_support(spec: &PluginSpec) -> Value {
    let engine = match spec.capability {
        "ocr" if crate::ocr::pick_engine().is_some() => Some("system-ocr"),
        "latex" => Some("texmd"),
        "highlight" => Some("code-highlight"),
        "document" => Some("native-converters"),
        "pdf" => Some("pdf-text"),
        "web" => Some("readability"),
        "encoding" => Some("encoding-detect"),
        _ => None,
    };
    json!({ "builtin": engine.is_some(), "engine": engine })
}

pub fn plugin_manifest(sandbox: &Sandbox) -> Map<String, Value> {
    sandbox.ensure_dirs();
    let data = read_manifest_data(sandbox);
    let tasks = tasks().lock().ok().map(|guard| guard.clone()).unwrap_or_default();
    let runtime = runtime_status().lock().ok().map(|guard| guard.clone()).unwrap_or_default();
    let uses = active_uses().lock().ok().map(|guard| guard.clone()).unwrap_or_default();

    let mut result: Map<String, Value> = Map::new();

    struct Row {
        pid: &'static str,
        installed: bool,
        enabled: bool,
        value: Value,
    }
    let mut rows: Vec<Row> = Vec::new();

    for spec in PLUGIN_SPECS {
        let entry = data.get(spec.id);
        let task = tasks.get(spec.id);
        let installed = is_plugin_installed(sandbox, spec.id);
        let cached = check_model_cached(sandbox, spec.id);
        let saved_enabled = entry.and_then(|value| value.get("enabled")).and_then(Value::as_bool);
        let enabled = if installed {
            saved_enabled.unwrap_or_else(|| DEFAULT_ENABLED.contains(&spec.id))
        } else {
            false
        };
        let runtime_value = runtime
            .get(spec.id)
            .cloned()
            .unwrap_or_else(|| Value::Object(Map::new()));
        let busy = uses.get(spec.id).copied().unwrap_or(0) != 0;
        let progress = match task {
            Some(record) => record.progress,
            None => {
                if installed {
                    100
                } else {
                    0
                }
            }
        };
        let value = json!({
            "id": spec.id,
            "runtime_connected": spec.runtime_connected,
            "name": spec.name,
            "capability": spec.capability,
            "alternatives": alternatives(spec.id),
            "requires_model": spec.requires_model,
            "runtime": runtime_value,
            "busy": busy,
            "name_key": spec.name_key,
            "desc_key": spec.desc_key,
            "category": spec.category,
            "weight": spec.weight,
            "approx_size": spec.approx_size,
            "installed": installed,
            "cached": cached,
            "enabled": enabled,
            "uninstalled": entry_bool(entry, "uninstalled").unwrap_or(false),
            "version": entry_str(entry, "version").unwrap_or("").to_string(),
            "installing": task.map(|record| record.status == "installing").unwrap_or(false),
            "progress": progress,
            "install_error": task.map(|record| record.error.clone()).unwrap_or_default(),
            "install_error_code": task.map(|record| record.error_code.clone()).unwrap_or_default(),
            "install_error_detail": task.map(|record| record.error_detail.clone()).unwrap_or_default(),
            "last_log": task.map(|record| record.last_log.clone()).unwrap_or_default(),
            "native": native_support(spec),
        });
        rows.push(Row {
            pid: spec.id,
            installed,
            enabled,
            value,
        });
    }

    // "Legacy manifests and environment-provided packages may enable both."
    // One pass per capability: explicit choices sort ahead of detected ones.
    for (capability, providers) in CAPABILITIES {
        let mut active: Vec<usize> = Vec::new();
        for provider in providers.iter() {
            if let Some(index) = rows.iter().position(|row| row.pid == *provider && row.enabled) {
                active.push(index);
            }
        }
        let explicit = |index: usize| -> bool {
            data.get(rows[index].pid)
                .and_then(|entry| entry.get("enabled"))
                .and_then(Value::as_bool)
                == Some(true)
        };
        // `active.sort(key=lambda pid: data.get(pid, {}).get('enabled') is not True)`
        // is a stable partition: explicit first, detected after, original order kept.
        active.sort_by_key(|index| if explicit(*index) { 0 } else { 1 });
        for index in active.iter().skip(1) {
            rows[*index].enabled = false;
            if let Some(object) = rows[*index].value.as_object_mut() {
                object.insert("enabled".to_string(), json!(false));
            }
        }
        let _ = capability;
    }

    for row in rows {
        let _ = (row.installed, row.enabled);
        result.insert(row.pid.to_string(), row.value);
    }
    result
}

/// `/api/plugins/list` payload: `{'ok', 'plugins', 'ffmpeg', 'sandbox_dir'}`.
pub fn plugins_list_payload(app: &App) -> Value {
    plugins_list_payload_for(&Sandbox::for_app(app))
}

pub fn plugins_list_payload_for(sandbox: &Sandbox) -> Value {
    json!({
        "ok": true,
        "plugins": Value::Object(plugin_manifest(sandbox)),
        "ffmpeg": get_ffmpeg_path(sandbox).is_some(),
        "sandbox_dir": sandbox.plugins_root.to_string_lossy().into_owned(),
    })
}

/// Compatibility shim for the superseded `server.rs` route.  Despite the name it
/// does **not** return the bare plugin map Python's `pm.load_manifest()` returns
/// (that port is [`plugin_manifest`], which `readmd.py:1974-1979` wraps in
/// `{ok, plugins, ffmpeg, sandbox_dir}`): it returns the *whole*
/// `/api/plugins/list` body from [`plugins_list_payload`], `ok` included.  No
/// outer wrapping is added — `ok_json` (`server.rs:1282`) forwards to
/// `Response::json`, which only serialises what it is handed — so the `"ok":
/// true` literal comes from [`plugins_list_payload_for`].  Prefer pointing the
/// route at [`h_plugins_list`], which carries the Python method guard.
pub fn load_manifest(app: &App) -> Value {
    plugins_list_payload(app)
}

// -------------------------------------------------------------- activation

/// `_disable_competing_plugins`.
fn disable_competing_plugins(manifest: &mut Map<String, Value>, plugin_id: &str) {
    let capability = match plugin_spec(plugin_id) {
        Some(spec) => spec.capability,
        None => return,
    };
    for other in capability_providers(capability) {
        if *other == plugin_id {
            continue;
        }
        let entry = manifest
            .entry((*other).to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        if let Some(object) = entry.as_object_mut() {
            object.insert("enabled".to_string(), json!(false));
        }
    }
}

/// `set_plugin_enabled`.
pub fn set_plugin_enabled(sandbox: &Sandbox, plugin_id: &str, enabled: bool) -> bool {
    if plugin_spec(plugin_id).is_none() || !is_plugin_installed(sandbox, plugin_id) {
        return false;
    }
    let mut manifest = read_manifest_data(sandbox);
    let entry = manifest
        .entry(plugin_id.to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if let Some(object) = entry.as_object_mut() {
        object.insert("enabled".to_string(), json!(enabled));
    }
    if enabled {
        disable_competing_plugins(&mut manifest, plugin_id);
    }
    save_manifest(sandbox, &manifest);
    let saved = read_manifest_data(sandbox);
    manifest.iter().all(|(pid, info)| {
        let expected = info.get("enabled").and_then(Value::as_bool).unwrap_or(false);
        saved.get(pid).and_then(|entry| entry.get("enabled")).and_then(Value::as_bool) == Some(expected)
    })
}

/// `is_plugin_enabled`.
pub fn is_plugin_enabled(sandbox: &Sandbox, plugin_id: &str) -> bool {
    if !is_plugin_installed(sandbox, plugin_id) {
        return false;
    }
    plugin_manifest(sandbox)
        .get(plugin_id)
        .and_then(|info| info.get("enabled"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// `active_provider`.
pub fn active_provider(sandbox: &Sandbox, capability: &str) -> Option<String> {
    let manifest = plugin_manifest(sandbox);
    capability_providers(capability)
        .iter()
        .copied()
        .find(|pid| {
            manifest
                .get(*pid)
                .and_then(|info| info.get("enabled"))
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .map(|pid| pid.to_string())
}

// -------------------------------------------------------------- (un)install

/// `_sandbox_artifacts`.
pub fn sandbox_artifacts(sandbox: &Sandbox, import_name: &str, package: &str) -> Vec<PathBuf> {
    let mut artifacts = Vec::new();
    for candidate in [
        sandbox.site_packages.join(import_name),
        sandbox.site_packages.join(format!("{import_name}.py")),
        sandbox.site_packages.join(format!("{import_name}.egg-info")),
    ] {
        if candidate.exists() {
            artifacts.push(candidate);
        }
    }
    let (first, second) = dist_info_prefixes(package);
    let mut entries: Vec<String> = match fs::read_dir(&sandbox.site_packages) {
        Ok(read) => read
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect(),
        Err(_) => return artifacts,
    };
    entries.sort();
    for entry in entries {
        let lowered = entry.to_ascii_lowercase();
        if !(lowered.ends_with(".dist-info") || lowered.ends_with(".egg-info")) {
            continue;
        }
        if lowered.starts_with(&first) || lowered.starts_with(&second) {
            artifacts.push(sandbox.site_packages.join(entry));
        }
    }
    artifacts
}

/// `_requested_requirement`.
pub fn requested_requirement(spec: &PluginSpec) -> String {
    spec.pip_args
        .iter()
        .find(|arg| !arg.starts_with('-'))
        .map(|arg| arg.to_string())
        .unwrap_or_default()
}

/// `uninstall_plugin`: delete the sandbox artefacts and only then mark the
/// plugin uninstalled.  Any leftover file means `false` plus an
/// `uninstall_locked` task, never a optimistic "已卸载".
pub fn uninstall_plugin(sandbox: &Sandbox, plugin_id: &str) -> bool {
    let spec = match plugin_spec(plugin_id) {
        Some(spec) => spec,
        None => return false,
    };
    if installing(plugin_id) {
        return false;
    }
    if active_uses()
        .lock()
        .ok()
        .and_then(|map| map.get(plugin_id).copied())
        .unwrap_or(0)
        != 0
    {
        return false;
    }
    let mut failures: Vec<String> = Vec::new();
    for artifact in sandbox_artifacts(sandbox, spec.import_name, spec.package) {
        let removed = if artifact.is_dir() && !artifact.symlink_metadata().map(|m| m.file_type().is_symlink()).unwrap_or(false) {
            fs::remove_dir_all(&artifact)
        } else {
            fs::remove_file(&artifact)
        };
        if let Err(error) = removed {
            failures.push(format!(
                "{}: {}",
                artifact.to_string_lossy(),
                error
            ));
        }
    }
    if !failures.is_empty() {
        set_task(plugin_id, "error", 0, &failures[0].clone(), "uninstall_locked", &failures);
        return false;
    }
    let mut manifest = read_manifest_data(sandbox);
    let entry = manifest
        .entry(plugin_id.to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if let Some(object) = entry.as_object_mut() {
        object.insert("uninstalled".to_string(), json!(true));
        object.insert("enabled".to_string(), json!(false));
    }
    save_manifest(sandbox, &manifest);
    if let Ok(mut map) = tasks().lock() {
        map.remove(plugin_id);
    }
    true
}

/// `install_plugin_async` (`plugin_manager.py:788-812`): `true` means "a worker
/// was scheduled for a known plugin id", nothing more.  The final state is only
/// ever reported through `/api/plugins/list`, exactly like the Python thread.
///
/// Python never shells out to a package installer it happens to find on `PATH`:
/// `_run_pip` (`plugin_manager.py:733-737`) routes a frozen build through
/// `runpy.run_module('pip', run_name='__main__', alter_sys=True)`
/// (`plugin_manager.py:661-730`) inside its own process, and only a *source
/// checkout* may fall back to a `sys.executable -m pip` child
/// (`plugin_manager.py:646-658`).  The Rust kernel is neither case — there is no
/// bundled `pip` module to run in-process and spawning one is exactly what the
/// native-independence rule forbids — so the faithful state is the one the
/// authority already defines for "this build has no pip": the `except
/// ImportError` arm at `plugin_manager.py:873-883`, copied verbatim
/// (`pip_unavailable` / `'pip is unavailable in this build.'` /
/// `['pip module not found in this build']`).
///
/// That code — not an invented one — is what the UI translates:
/// `PLUGIN_ERROR_TEXT` (`assets/js/features/convert.js:270-278`) is a closed set
/// of seven keys and `pip_unavailable` is among them, resolving to
/// `plugin.error.pip_unavailable` = "The bundled package installer is unavailable
/// in this build." (`assets/i18n/en.json:817`, mirrored in every locale).
pub fn install_plugin_async(sandbox: Sandbox, plugin_id: &str) -> bool {
    if plugin_spec(plugin_id).is_none() {
        return false;
    }

    // `plugin_manager.py:794-797`: a second request while a worker owns the id is
    // acknowledged without restarting the install.
    if installing(plugin_id) {
        return true;
    }

    // `plugin_manager.py:799-804`.
    set_task(plugin_id, "installing", 0, "Starting installation...", "", &[]);

    // `plugin_manager.py:806-810`: installing again clears the uninstall marker.
    let was_uninstalled = manifest_flag(&sandbox, plugin_id, "uninstalled");
    if was_uninstalled {
        let mut manifest = read_manifest_data(&sandbox);
        if let Some(entry) = manifest.get_mut(plugin_id).and_then(Value::as_object_mut) {
            entry.insert("uninstalled".to_string(), json!(false));
        }
        save_manifest(&sandbox, &manifest);
    }

    let id = plugin_id.to_string();
    let worker = std::thread::Builder::new()
        .name(format!("readmd-plugin-install-{id}"))
        .spawn(move || {
            // `_worker()` opens with `_ensure_dirs()` (`plugin_manager.py:813`).
            sandbox.ensure_dirs();
            report_pip_unavailable(&id);
        });
    if worker.is_err() {
        // Python has no such branch; keep the task out of a permanent
        // "installing" state with the same authoritative triple.
        report_pip_unavailable(plugin_id);
    }
    true
}

/// The one degraded outcome, shared by the worker thread and by a failed spawn.
/// Field-for-field `plugin_manager.py:874-882`.
fn report_pip_unavailable(plugin_id: &str) {
    set_task(
        plugin_id,
        "error",
        0,
        "pip is unavailable in this build.",
        "pip_unavailable",
        &["pip module not found in this build".to_string()],
    );
    // `logging.error('Plugin %s install failed: pip is unavailable', plugin_id)`
    log::error!("Plugin {plugin_id} install failed: pip is unavailable");
}

/// `manifest_data.get(plugin_id, {}).get(name)` for a boolean manifest key.
fn manifest_flag(sandbox: &Sandbox, plugin_id: &str, name: &str) -> bool {
    read_manifest_data(sandbox)
        .get(plugin_id)
        .and_then(|entry| entry.get(name))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// `get_ffmpeg_path`: the sandbox bundle first, then `PATH`.
pub fn get_ffmpeg_path(sandbox: &Sandbox) -> Option<PathBuf> {
    let bin_name = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
    let candidate = sandbox.bin_dir.join(bin_name);
    if candidate.is_file() && executable(&candidate) {
        return Some(candidate);
    }
    which("ffmpeg")
}

#[cfg(unix)]
fn executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata().map(|meta| meta.permissions().mode() & 0o111 != 0).unwrap_or(false)
}

#[cfg(not(unix))]
fn executable(_path: &Path) -> bool {
    true
}

/// `shutil.which('ffmpeg')`.
pub fn which(program: &str) -> Option<PathBuf> {
    let exts: Vec<&str> = if cfg!(windows) {
        vec![".exe", ".cmd", ".bat", ""]
    } else {
        vec![""]
    };
    let path = std::env::var("PATH").unwrap_or_default();
    for entry in path.split(if cfg!(windows) { ';' } else { ':' }) {
        if entry.trim().is_empty() {
            continue;
        }
        for extension in &exts {
            let candidate = Path::new(entry).join(format!("{program}{extension}"));
            if candidate.is_file() && executable(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

// ------------------------------------------------------------------ handlers

use crate::error::ApiResult;
use crate::server::{Request, Response};
use std::sync::Arc;

fn json_response(status: u16, value: Value) -> ApiResult<Response> {
    Ok(Response::json_status(status, &value))
}

fn error_code(status: u16, code: &str) -> ApiResult<Response> {
    json_response(status, json!({ "ok": false, "error_code": code }))
}

/// `Handler._api_plugins_list` (`readmd.py:1968`).
///
/// `readmd.py:1327-1334` runs one dispatcher chain for GET *and* POST, so this
/// own-command guard (`readmd.py:1969-1971`) is the only method enforcement:
/// every verb other than `GET` answers
/// `405 {'ok': False, 'error_code': 'method_not_allowed'}` before any payload
/// is built.
pub fn h_plugins_list(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    if req.method != "GET" {
        return error_code(405, "method_not_allowed");
    }
    json_response(200, plugins_list_payload(app))
}

/// `Handler._api_plugins_toggle` (`readmd.py:2011`).
pub fn h_plugins_toggle(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    if req.method != "POST" {
        return error_code(405, "method_not_allowed");
    }
    let sandbox = Sandbox::for_app(app);
    let body = req.json()?;
    // `readmd.py:2017` `plugin_id = str(body.get('plugin_id', '')).strip()`: the
    // same `py_str_field` coercion install and uninstall use, so a null or
    // numeric id becomes `"None"`/`"123"` rather than `""`.  Toggle has *no*
    // `plugin_id_required` branch (unlike `readmd.py:1991`), so such an id falls
    // straight through to the membership check at `readmd.py:2020` and answers
    // `plugin_not_integrated`, exactly like Python.
    let plugin_id = py_str_field(&body, "plugin_id").trim().to_string();
    let enabled = body.get("enabled").map(py_truthy).unwrap_or(false);
    if plugin_spec(&plugin_id).is_none() {
        return error_code(400, "plugin_not_integrated");
    }
    if !is_plugin_installed(&sandbox, &plugin_id) {
        return error_code(400, "plugin_not_installed");
    }
    if !set_plugin_enabled(&sandbox, &plugin_id, enabled) {
        return error_code(500, "plugin_toggle_failed");
    }
    json_response(200, json!({ "ok": true, "plugin_id": plugin_id, "enabled": enabled }))
}

/// `bool(value)` on a parsed JSON value: only `null`, `false`, numeric zero,
/// empty strings and empty containers are falsy — notably `"false"` is truthy.
fn py_truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

/// `str(body.get(key, ''))` — Python's `str()` on a parsed JSON value.
/// A wrong-typed value still yields a non-empty string, so it fails
/// downstream (e.g. as `plugin_not_integrated`) rather than as missing.
fn py_str_field(body: &Value, key: &str) -> String {
    match body.get(key) {
        None => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Null) => "None".to_string(),
        Some(Value::Bool(true)) => "True".to_string(),
        Some(Value::Bool(false)) => "False".to_string(),
        Some(other) => other.to_string(),
    }
}

/// `Handler._api_plugins_install` (`readmd.py:1984`).
pub fn h_plugins_install(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    if req.method != "POST" {
        return error_code(405, "method_not_allowed");
    }
    let sandbox = Sandbox::for_app(app);
    let body = req.json()?;
    let plugin_id = py_str_field(&body, "plugin_id").trim().to_string();
    if plugin_id.is_empty() {
        return error_code(400, "plugin_id_required");
    }
    if plugin_spec(&plugin_id).is_none() {
        return error_code(400, "plugin_not_integrated");
    }
    if !install_plugin_async(sandbox, &plugin_id) {
        return error_code(400, "invalid_plugin_id");
    }
    json_response(200, json!({ "ok": true, "plugin_id": plugin_id }))
}

/// `Handler._api_plugins_uninstall` (`readmd.py:2036`).
pub fn h_plugins_uninstall(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    if req.method != "POST" {
        return error_code(405, "method_not_allowed");
    }
    let sandbox = Sandbox::for_app(app);
    let body = req.json()?;
    let plugin_id = py_str_field(&body, "plugin_id").trim().to_string();
    let removed = uninstall_plugin(&sandbox, &plugin_id);
    json_response(200, json!({ "ok": removed, "plugin_id": plugin_id }))
}

// -------------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;

    /// 沙盒夹具。两点必须与 Python 对齐：
    ///
    /// * `_ensure_dirs()`（`plugin_manager.py:165`）在 `is_plugin_installed` /
    ///   `save_manifest` / `load_manifest` 里都会先跑，所以 `site-packages`
    ///   在使用前已经存在——夹具也一样先建目录，测试才能直接写产物。
    /// * `_check_model_cached` 用 `os.path.expanduser` 解析 `~/.EasyOCR/model`，
    ///   宿主 `%USERPROFILE%` 真装了 EasyOCR 时那里是有权重文件的。把
    ///   `home_dir` 指进沙盒，缓存判定才是夹具的属性而不是机器的属性。
    fn sandbox(tag: &str) -> Sandbox {
        let dir = std::env::temp_dir().join(format!(
            "readmd-plugin-{}-{}",
            tag,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let mut sandbox = Sandbox::new(&dir);
        sandbox.home_dir = dir.join("home");
        fs::create_dir_all(&sandbox.home_dir).unwrap();
        sandbox.ensure_dirs();
        sandbox
    }

    #[test]
    fn catalog_matches_the_python_declaration_order() {
        let ids: Vec<&str> = PLUGIN_SPECS.iter().map(|spec| spec.id).collect();
        assert_eq!(
            ids,
            vec![
                "easyocr",
                "pylatexenc",
                "rapidocr",
                "rapid_table",
                "whisper",
                "jieba",
                "pygments",
                "pandoc_bridge",
                "pymupdf4llm",
                "docling",
                "faster_whisper",
                "markdownify",
                "trafilatura",
                "charset_normalizer",
            ]
        );
        // plugin_catalog overrides: binary package and size for the bridge.
        let bridge = plugin_spec("pandoc_bridge").unwrap();
        assert_eq!(bridge.package, "pypandoc-binary");
        assert_eq!(bridge.pip_args, &["pypandoc-binary"]);
        assert_eq!(bridge.approx_size, "~35MB");
        assert_eq!(bridge.import_name, "pypandoc");
        assert_eq!(bridge.capability, "document");
        // `pid.replace('_', ' ').title()` for the eight base specs.
        assert_eq!(plugin_spec("easyocr").unwrap().name, "Easyocr");
        assert_eq!(plugin_spec("whisper").unwrap().name, "Whisper");
        assert_eq!(plugin_spec("faster_whisper").unwrap().name, "Faster Whisper");
        assert_eq!(plugin_spec("charset_normalizer").unwrap().name, "Charset Normalizer");
        // Only the four pipeline heavyweights default to enabled.
        assert_eq!(DEFAULT_ENABLED.len(), 4);
        assert!(!DEFAULT_ENABLED.contains(&"pylatexenc"));
        assert!(!DEFAULT_ENABLED.contains(&"charset_normalizer"));
    }

    #[test]
    fn capabilities_and_alternatives_are_exclusive() {
        assert_eq!(alternatives("rapidocr"), vec!["easyocr"]);
        assert_eq!(alternatives("whisper"), vec!["faster_whisper"]);
        assert_eq!(alternatives("pandoc_bridge"), Vec::<&str>::new());
        assert_eq!(alternatives("pygments"), Vec::<&str>::new());
        assert_eq!(capability_providers("ocr"), &["rapidocr", "easyocr"]);
        assert!(capability_providers("pdf").contains(&"docling"));
        assert!(plugin_spec("docling").unwrap().requires_model);
        assert!(!plugin_spec("pymupdf4llm").unwrap().requires_model);
    }

    #[test]
    fn manifest_shape_and_default_enablement() {
        let box_ = sandbox("manifest");
        let manifest = plugin_manifest(&box_);
        assert_eq!(manifest.len(), PLUGIN_SPECS.len());
        let easyocr = &manifest["easyocr"];
        let keys: Vec<&str> = easyocr
            .as_object()
            .unwrap()
            .keys()
            .map(|key| key.as_str())
            .collect();
        for expected in [
            "alternatives",
            "approx_size",
            "busy",
            "cached",
            "capability",
            "category",
            "desc_key",
            "enabled",
            "id",
            "install_error",
            "install_error_code",
            "install_error_detail",
            "installed",
            "installing",
            "last_log",
            "name",
            "name_key",
            "native",
            "progress",
            "requires_model",
            "runtime",
            "runtime_connected",
            "uninstalled",
            "version",
            "weight",
        ] {
            assert!(keys.contains(&expected), "missing {expected}");
        }
        assert_eq!(keys.len(), 25);
        assert_eq!(manifest["pymupdf4llm"]["native"], json!({ "builtin": true, "engine": "pdf-text" }));
        assert_eq!(manifest["whisper"]["native"], json!({ "builtin": false, "engine": null }));
        // Nothing is installed in a fresh sandbox, so nothing is enabled.
        assert_eq!(easyocr["installed"], json!(false));
        assert_eq!(easyocr["enabled"], json!(false));
        assert_eq!(easyocr["progress"], json!(0));
        assert_eq!(easyocr["runtime"], json!({}));
        assert_eq!(easyocr["alternatives"], json!(["rapidocr"]));
    }

    #[test]
    fn installed_default_enabled_plugin_can_be_disabled_and_competitors_arbitrated() {
        let box_ = sandbox("enable");
        // Fake a finished pip install for both OCR providers.
        for (import_name, package) in [
            ("easyocr", "easyocr"),
            ("rapidocr_onnxruntime", "rapidocr_onnxruntime"),
        ] {
            fs::create_dir_all(box_.site_packages.join(import_name)).unwrap();
            fs::create_dir_all(
                box_
                    .site_packages
                    .join(format!("{}-1.0.0.dist-info", package)),
            )
            .unwrap();
        }
        assert!(is_plugin_installed(&box_, "easyocr"));
        assert!(is_plugin_installed(&box_, "rapidocr"));
        let manifest = plugin_manifest(&box_);
        // Both default to enabled (easyocr is a DEFAULT_ENABLED provider and
        // rapidocr is detected as installed); CAPABILITIES['ocr'] lists
        // rapidocr first, so the stable sort keeps it and easyocr loses.
        assert_eq!(manifest["rapidocr"]["enabled"], json!(true));
        assert_eq!(manifest["easyocr"]["enabled"], json!(false));
        assert_eq!(active_provider(&box_, "ocr"), Some("rapidocr".into()));

        assert!(set_plugin_enabled(&box_, "easyocr", true));
        // Explicit easyocr wins; rapidocr is disabled by arbitration.
        assert!(!is_plugin_enabled(&box_, "rapidocr"));
        assert!(is_plugin_enabled(&box_, "easyocr"));
        assert_eq!(
            read_manifest_data(&box_)["rapidocr"]["enabled"],
            json!(false)
        );
        assert_eq!(active_provider(&box_, "ocr"), Some("easyocr".into()));

        assert!(set_plugin_enabled(&box_, "rapidocr", true));
        assert!(!is_plugin_enabled(&box_, "easyocr"));
        // Unknown or not-installed ids refuse the toggle.
        assert!(!set_plugin_enabled(&box_, "nope", true));
        assert!(!set_plugin_enabled(&box_, "pygments", true));
    }

    #[test]
    fn interrupted_install_is_not_installed_and_uninstall_reports_lock() {
        let box_ = sandbox("residue");
        fs::create_dir_all(box_.site_packages.join("pygments")).unwrap();
        // Package directory without dist-info == aborted install leftover.
        assert!(!is_plugin_installed(&box_, "pygments"));
        fs::create_dir_all(box_.site_packages.join("Pygments-2.0.0.dist-info")).unwrap();
        assert!(is_plugin_installed(&box_, "pygments"));
        assert!(uninstall_plugin(&box_, "pygments"));
        assert!(!is_plugin_installed(&box_, "pygments"));
        assert_eq!(read_manifest_data(&box_)["pygments"]["uninstalled"], json!(true));
        assert_eq!(read_manifest_data(&box_)["pygments"]["enabled"], json!(false));
        assert!(!uninstall_plugin(&box_, "unknown_plugin"));
    }

    #[test]
    fn uninstalled_flag_shadows_the_artefacts() {
        // `plugin_manager.py:300-303`：`is_plugin_installed` 先 `_ensure_dirs()`，
        // 再看 manifest 的 uninstalled 标记，标记为真就直接 False，哪怕
        // site-packages 里产物还在（:312 的产物检查根本不会跑到）。
        let box_ = sandbox("shadow");
        fs::write(box_.site_packages.join("jieba.py"), b"x").unwrap();
        assert!(is_plugin_installed(&box_, "jieba"));
        let mut manifest = Map::new();
        manifest.insert("jieba".into(), json!({ "uninstalled": true }));
        save_manifest(&box_, &manifest);
        assert!(!is_plugin_installed(&box_, "jieba"));
        // 产物没被 `save_manifest` 动过，被清掉的是“已安装”这个判定本身。
        assert!(box_.site_packages.join("jieba.py").is_file());
        assert_eq!(read_manifest_data(&box_)["jieba"]["uninstalled"], json!(true));
    }

    #[test]
    fn cached_probe_follows_cache_type() {
        // easyocr 是 dir_has_files：只看 ~/.EasyOCR/model，沙盒 home 里没有它。
        let box_ = sandbox("cached");
        assert!(!check_model_cached(&box_, "easyocr"));
        // pylatexenc 是 cache_type installed：走 `is_plugin_installed` 那条腿。
        assert!(!check_model_cached(&box_, "pylatexenc"));
        fs::write(box_.site_packages.join("pylatexenc.py"), b"x").unwrap();
        assert!(check_model_cached(&box_, "pylatexenc"));
        // 反向：装上 easyocr 的包文件不会喂到 dir_has_files 探针。
        fs::write(box_.site_packages.join("easyocr.py"), b"x").unwrap();
        assert!(is_plugin_installed(&box_, "easyocr"));
        assert!(!check_model_cached(&box_, "easyocr"));
        // 只有缓存目录真有内容时才算缓存（Python: `bool(os.listdir(p))`）。
        let model = box_.home_dir.join(".EasyOCR").join("model");
        fs::create_dir_all(&model).unwrap();
        assert!(!check_model_cached(&box_, "easyocr"));
        fs::write(model.join("craft_mlt_25k.pth"), b"x").unwrap();
        assert!(check_model_cached(&box_, "easyocr"));
    }

    #[test]
    fn dist_name_normalization_matches_python() {
        assert_eq!(normalize_dist_name("Rapid-Table"), "rapid-table");
        assert_eq!(normalize_dist_name("charset.normalizer"), "charset-normalizer");
        assert_eq!(normalize_dist_name("faster_whisper"), "faster-whisper");
        let (a, b) = dist_info_prefixes("openai-whisper");
        assert_eq!(a, "openai-whisper-");
        assert_eq!(b, "openai_whisper-");
    }

    #[test]
    fn ffmpeg_probe_prefers_the_sandbox_bundle() {
        let box_ = sandbox("ffmpeg");
        box_.ensure_dirs();
        let bin_name = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
        let bundled = box_.bin_dir.join(bin_name);
        fs::write(&bundled, b"#!/bin/sh\n").unwrap();
        assert_eq!(get_ffmpeg_path(&box_).as_deref(), Some(bundled.as_path()));
        let _ = fs::remove_file(&bundled);
        // Falls back to PATH; either answer is legitimate for the sandbox user.
        let _ = get_ffmpeg_path(&box_);
    }

    // ------------------------------------------------- HTTP handler parity
    //
    // Every status, key and code asserted here is the literal Python source, read
    // with the Read tool from `readmd.py`:
    //
    // * `_api_plugins_list` `readmd.py:1968` — `if self.command != 'GET'` →
    //   `self._send_api_error(405, 'method_not_allowed')`, else
    //   `self._send_json(200, {'ok': True, 'plugins': pm.load_manifest(),
    //   'ffmpeg': ..., 'sandbox_dir': ...})` (`:1974-1979`).
    // * `_api_plugins_install` `readmd.py:1984` — 405 (`:1985`), then 400
    //   `plugin_id_required` (`:1991-1992`), 400 `plugin_not_integrated`
    //   (`:1997-1998`), 400 `invalid_plugin_id` (`:2001-2002`), 200
    //   `{'ok': True, 'plugin_id': plugin_id}` (`:2004`).
    // * `_api_plugins_toggle` `readmd.py:2011` — 405 (`:2012`), then 400
    //   `plugin_not_integrated` (`:2020-2022`) *before* 400
    //   `plugin_not_installed` (`:2023-2024`), 500 `plugin_toggle_failed`
    //   (`:2026-2027`), 200 `{'ok': True, 'plugin_id': plugin_id,
    //   'enabled': enabled}` (`:2029`).  Note there is no `plugin_id_required`
    //   branch at all.
    // * `_api_plugins_uninstall` `readmd.py:2036` — 405 (`:2037`), then *always*
    //   200 `{'ok': ok, 'plugin_id': plugin_id}` (`:2044-2045`): Python has no
    //   error code on this route whatsoever.
    //
    // `_send_api_error` (`readmd.py:1421-1430`) is
    // `payload = {'ok': False, 'error_code': str(error_code or 'internal_error')}`
    // plus `**extra`, and none of these calls pass an extra, so every failure
    // asserted below is exactly two keys.
    //
    // `_install_tasks` is one process-wide map, so the ids are partitioned by who
    // writes a task: an `installing` task is only ever created here for
    // `markdownify`, `trafilatura` and `whisper`.  `uninstall_plugin` — which
    // refuses while a task reports the plugin installing (`plugin_manager.py:961`)
    // — only ever runs here against `docling`, `pandoc_bridge` and non-members.
    // The pre-existing tests above stay on
    // `pygments`/`jieba`/`easyocr`/`rapidocr`/`pylatexenc`, so no thread's transient
    // task can flip another thread's assertion.

    fn hrequest(method: &str, target: &str, body: &str) -> Request {
        Request {
            method: method.to_string(),
            path: target.to_string(),
            query: Default::default(),
            headers: Default::default(),
            body: body.as_bytes().to_vec(),
        }
    }

    fn json_of(res: &Response) -> Value {
        serde_json::from_slice(&res.body).expect("handler body must be JSON")
    }

    /// A real `App` over a throw-away data dir, so `Sandbox::for_app` — which is
    /// what every handler builds — resolves to fixture state rather than the
    /// developer's `%LOCALAPPDATA%\ReadMD`.
    fn fixture(tag: &str) -> (Sandbox, Arc<App>) {
        let dir = std::env::temp_dir().join(format!(
            "readmd-pmhttp-{}-{}-{}",
            tag,
            std::process::id(),
            crate::store::now_millis()
        ));
        let _ = fs::remove_dir_all(&dir);
        let data = dir.join("data");
        let ws = dir.join("ws");
        let assets = dir.join("assets");
        for created in [&data, &ws, &assets] {
            fs::create_dir_all(created).unwrap();
        }
        let app = Arc::new(
            App::bootstrap(crate::paths::AppPaths::with_dirs(&data, &ws, &assets))
                .expect("App bootstraps over a temp data dir"),
        );
        let mut sandbox = Sandbox::new(&data);
        sandbox.home_dir = dir.join("home");
        fs::create_dir_all(&sandbox.home_dir).unwrap();
        sandbox.ensure_dirs();
        (sandbox, app)
    }

    /// A finished pip run: importable package directory plus its dist-info.
    fn fake_installed(sandbox: &Sandbox, import_name: &str, package: &str) {
        fs::create_dir_all(sandbox.site_packages.join(import_name)).unwrap();
        fs::create_dir_all(
            sandbox
                .site_packages
                .join(format!("{}-1.0.0.dist-info", package)),
        )
        .unwrap();
    }

    fn method_not_allowed() -> Value {
        json!({ "ok": false, "error_code": "method_not_allowed" })
    }

    #[test]
    fn list_route_guards_its_method_before_touching_the_manifest() {
        let (_box_, app) = fixture("list");
        // `readmd.py:1969-1971`: the single dispatcher serves GET and POST alike,
        // so anything but GET is a 405 with the two-key error body.  The body is
        // deliberately a *valid* install request: the guard must win anyway.
        for verb in ["POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"] {
            let res = h_plugins_list(&app, &hrequest(verb, "/api/plugins/list", "{\"plugin_id\":\"docling\"}"))
                .expect("405 is a response, not an error");
            assert_eq!(res.status, 405, "{verb} must not reach the manifest");
            assert_eq!(json_of(&res), method_not_allowed());
        }
        let res = h_plugins_list(&app, &hrequest("GET", "/api/plugins/list", "")).unwrap();
        assert_eq!(res.status, 200);
        let body = json_of(&res);
        let keys: Vec<String> = body
            .as_object()
            .unwrap()
            .keys()
            .map(|key| key.to_string())
            .collect();
        // `readmd.py:1974-1979` — exactly these four, and `ok` comes from the
        // handler body itself (no wrapper injects it).
        assert_eq!(keys.len(), 4);
        for expected in ["ok", "plugins", "ffmpeg", "sandbox_dir"] {
            assert!(keys.contains(&expected.to_string()), "missing {expected}");
        }
        assert_eq!(body["ok"], json!(true));
        assert_eq!(body["plugins"].as_object().unwrap().len(), PLUGIN_SPECS.len());
        assert_eq!(body["sandbox_dir"], json!(Sandbox::for_app(&app).plugins_root.to_string_lossy()));
    }

    #[test]
    fn install_route_replays_the_four_python_responses_in_order() {
        let (_box_, app) = fixture("install");
        let post = |body: &str| {
            h_plugins_install(&app, &hrequest("POST", "/api/plugins/install", body)).unwrap()
        };
        // 1) `readmd.py:1991-1992` — the empty-id check runs *first*, so a missing
        //    or whitespace-only id is `plugin_id_required`, never
        //    `plugin_not_integrated`.
        for body in ["{}", "{\"plugin_id\":\"\"}", "{\"plugin_id\":\"   \"}", "{\"plugin_id\":\"\\t\\n\"}"] {
            let res = post(body);
            assert_eq!(res.status, 400, "{body}");
            assert_eq!(
                json_of(&res),
                json!({ "ok": false, "error_code": "plugin_id_required" }),
                "{body}"
            );
        }
        // 2) `readmd.py:1997-1998` — a non-empty id that is not a member.  Python
        //    reaches here for `null`/numbers/booleans too, because
        //    `str(body.get('plugin_id',''))` stringifies them to a *non-empty*
        //    value (`"None"`, `"123"`, `"True"`) — pinning the `py_str_field`
        //    emulation that fix 3 unified.
        for (body, id) in [
            ("{\"plugin_id\":\"no_such_plugin\"}", "no_such_plugin"),
            ("{\"plugin_id\":null}", "None"),
            ("{\"plugin_id\":123}", "123"),
            ("{\"plugin_id\":true}", "True"),
        ] {
            let res = post(body);
            assert_eq!(res.status, 400, "{body}");
            assert_eq!(
                json_of(&res),
                json!({ "ok": false, "error_code": "plugin_not_integrated" }),
                "{body} must be `plugin_not_integrated` for `{id}`"
            );
        }
        // 3) `readmd.py:2000-2002` — `invalid_plugin_id` is only reachable when
        //    `install_plugin_async` refuses an id the membership check already
        //    accepted, i.e. it is dead code behind the route.  Pin the predicate
        //    directly so the branch is not silently wrong.
        assert!(!install_plugin_async(Sandbox::for_app(&app), "no_such_plugin"));
        // 4) `readmd.py:2004` — a member id schedules a worker and answers 200 with
        //    exactly {'ok', 'plugin_id'}; the trailing whitespace is stripped before
        //    it is echoed back.
        for (body, id) in [
            ("{\"plugin_id\":\"markdownify\"}", "markdownify"),
            ("{\"plugin_id\":\"  trafilatura  \"}", "trafilatura"),
        ] {
            let res = post(body);
            assert_eq!(res.status, 200, "{body}");
            let payload = json_of(&res);
            assert_eq!(payload, json!({ "ok": true, "plugin_id": id }), "{body}");
            assert_eq!(payload.as_object().unwrap().len(), 2, "{body}");
        }
        // 405 (`readmd.py:1985`) wins before the body is parsed at all.
        let res = h_plugins_install(&app, &hrequest("GET", "/api/plugins/install", "not json")).unwrap();
        assert_eq!(res.status, 405);
        assert_eq!(json_of(&res), method_not_allowed());
    }

    /// The detached install worker settles in Python's own words and code, not
    /// invented prose: `plugin_manager.py:874-882` gives status `error`, progress
    /// `0`, `last_log` `'pip is unavailable in this build.'`, `error_code`
    /// `pip_unavailable` and the single detail line
    /// `'pip module not found in this build'`.  `pip_unavailable` is one of the
    /// seven keys of `PLUGIN_ERROR_TEXT` (`assets/js/features/convert.js:270-278`)
    /// and resolves to `plugin.error.pip_unavailable` in every locale; a code
    /// outside that set would render as the UI's generic fallback.
    #[test]
    fn install_reports_pythons_pip_unavailable_task_state() {
        let (_box_, app) = fixture("pip-unavailable");
        let plugin_id = "rapidocr";
        assert!(install_plugin_async(Sandbox::for_app(&app), plugin_id));

        // `plugin_manager.py:806-810` on top of the guard below, checked with ids
        // no other test puts into the process-global task map.
        let busy = "pymupdf4llm";
        set_task(busy, "installing", 42, "Collecting pymupdf4llm", "", &[]);
        assert!(install_plugin_async(Sandbox::for_app(&app), busy));
        let kept = task_of(busy).unwrap();
        assert_eq!(kept.progress, 42, "794-797 must not restart a running install");
        assert_eq!(kept.last_log, "Collecting pymupdf4llm");

        let cleared = "charset_normalizer";
        let mut manifest = read_manifest_data(&Sandbox::for_app(&app));
        manifest.insert(
            cleared.to_string(),
            json!({ "enabled": false, "uninstalled": true, "version": "" }),
        );
        save_manifest(&Sandbox::for_app(&app), &manifest);
        assert!(manifest_flag(&Sandbox::for_app(&app), cleared, "uninstalled"));
        assert!(install_plugin_async(Sandbox::for_app(&app), cleared));
        assert!(
            !manifest_flag(&Sandbox::for_app(&app), cleared, "uninstalled"),
            "scheduling an install clears the uninstalled marker"
        );

        let task = wait_for_settled(plugin_id);
        assert_eq!(task.status, "error");
        assert_eq!(task.progress, 0);
        assert_eq!(task.last_log, "pip is unavailable in this build.");
        assert_eq!(task.error_code, "pip_unavailable");
        assert_eq!(task.error, "pip_unavailable", "`error` mirrors the code in _set_task");
        assert_eq!(task.error_detail, "pip module not found in this build");

        wait_for_settled(cleared);
        // `busy` never got a worker — that is the guard working.
        assert_eq!(task_of(busy).unwrap().progress, 42);
    }

    /// `install_plugin_async` returns before the worker is done — the same
    /// contract the polling `/api/plugins/list` client relies on — so a test has
    /// to wait for it to leave `installing`.
    fn wait_for_settled(plugin_id: &str) -> InstallTask {
        for _ in 0..400 {
            if let Some(task) = task_of(plugin_id) {
                if task.status != "installing" {
                    return task;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("install worker never settled for {plugin_id}");
    }

    #[test]
    fn toggle_route_checks_membership_before_installation() {
        let (box_, app) = fixture("toggle");
        let post = |body: &str| {
            h_plugins_toggle(&app, &hrequest("POST", "/api/plugins/toggle", body)).unwrap()
        };
        // `readmd.py:2017` + `:2020`: no `plugin_id_required` branch exists, so a
        // missing id is just `''` and therefore `plugin_not_integrated`…
        for body in ["{}", "{\"plugin_id\":\"\"}", "{\"plugin_id\":\"   \"}"] {
            let res = post(body);
            assert_eq!(res.status, 400, "{body}");
            assert_eq!(
                json_of(&res),
                json!({ "ok": false, "error_code": "plugin_not_integrated" }),
                "{body}"
            );
        }
        // …and a wrong-typed id (`"None"` / `"123"`) lands in the same branch.
        for body in ["{\"plugin_id\":null}", "{\"plugin_id\":7}"] {
            let res = post(body);
            assert_eq!(res.status, 400, "{body}");
            assert_eq!(
                json_of(&res),
                json!({ "ok": false, "error_code": "plugin_not_integrated" }),
                "{body}"
            );
        }
        // Membership *before* installation (`:2020` then `:2023`): an unknown id in
        // a sandbox where nothing is installed still reports not-integrated, while
        // a member that is absent reports not-installed.
        let res = post("{\"plugin_id\":\"no_such_plugin\",\"enabled\":true}");
        assert_eq!(
            json_of(&res),
            json!({ "ok": false, "error_code": "plugin_not_integrated" })
        );
        assert!(!is_plugin_installed(&Sandbox::for_app(&app), "faster_whisper"));
        let res = post("{\"plugin_id\":\"faster_whisper\",\"enabled\":true}");
        assert_eq!(res.status, 400);
        assert_eq!(
            json_of(&res),
            json!({ "ok": false, "error_code": "plugin_not_installed" })
        );
        // Happy path, `readmd.py:2029`: exactly {'ok','plugin_id','enabled'}.
        fake_installed(&box_, "docling", "docling");
        let res = post("{\"plugin_id\":\"docling\",\"enabled\":true}");
        assert_eq!(res.status, 200);
        let body = json_of(&res);
        assert_eq!(
            body,
            json!({ "ok": true, "plugin_id": "docling", "enabled": true })
        );
        assert_eq!(body.as_object().unwrap().len(), 3);
        // The manifest really moved, and `pdf` arbitration disabled the sibling.
        let saved = read_manifest_data(&box_);
        assert_eq!(saved["docling"]["enabled"], json!(true));
        assert_eq!(saved["pymupdf4llm"]["enabled"], json!(false));
        assert!(is_plugin_enabled(&Sandbox::for_app(&app), "docling"));
        // Turning it back off round-trips the false.
        let res = post("{\"plugin_id\":\"docling\",\"enabled\":false}");
        assert_eq!(json_of(&res)["enabled"], json!(false));
        // 405 before the body read (`readmd.py:2012`).
        let res = h_plugins_toggle(&app, &hrequest("GET", "/api/plugins/toggle", "{}")).unwrap();
        assert_eq!(res.status, 405);
        assert_eq!(json_of(&res), method_not_allowed());
    }

    #[test]
    fn toggle_emulates_python_bool_truthiness() {
        let (box_, app) = fixture("toggle-bool");
        fake_installed(&box_, "charset_normalizer", "charset-normalizer");
        let toggle = |payload: &str| {
            let res = h_plugins_toggle(&app, &hrequest("POST", "/api/plugins/toggle", payload))
                .unwrap();
            assert_eq!(res.status, 200, "{payload}");
            json_of(&res)["enabled"].as_bool().unwrap()
        };
        // `readmd.py:2018` `enabled = bool(body.get('enabled'))`, no default: a
        // missing key is `bool(None)` → False.
        assert_eq!(toggle("{\"plugin_id\":\"charset_normalizer\"}"), false);
        assert_eq!(
            toggle("{\"plugin_id\":\"charset_normalizer\",\"enabled\":null}"),
            false
        );
        // Strings are judged by length, not content — the R3-verified case where
        // Python's `bool('false')` is True, so `"false"`, `"0"` and `"no"` ENABLE.
        for text in ["false", "0", "no", "off", " "] {
            assert_eq!(
                toggle(&format!("{{\"plugin_id\":\"charset_normalizer\",\"enabled\":\"{text}\"}}")),
                true,
                "bool('{text}') is True in Python"
            );
        }
        assert_eq!(
            toggle("{\"plugin_id\":\"charset_normalizer\",\"enabled\":\"\"}"),
            false
        );
        // Numbers, containers.
        assert_eq!(
            toggle("{\"plugin_id\":\"charset_normalizer\",\"enabled\":0}"),
            false
        );
        assert_eq!(
            toggle("{\"plugin_id\":\"charset_normalizer\",\"enabled\":2}"),
            true
        );
        assert_eq!(
            toggle("{\"plugin_id\":\"charset_normalizer\",\"enabled\":[]}"),
            false
        );
        assert_eq!(
            toggle("{\"plugin_id\":\"charset_normalizer\",\"enabled\":[0]}"),
            true
        );
        assert_eq!(
            toggle("{\"plugin_id\":\"charset_normalizer\",\"enabled\":{}}"),
            false
        );
        assert_eq!(
            toggle("{\"plugin_id\":\"charset_normalizer\",\"enabled\":{\"a\":1}}"),
            true
        );
        // Whatever the response says, the persisted state agrees.
        assert_eq!(
            read_manifest_data(&box_)["charset_normalizer"]["enabled"],
            json!(true)
        );
        assert!(is_plugin_enabled(&Sandbox::for_app(&app), "charset_normalizer"));
    }

    #[test]
    fn uninstall_route_always_answers_200() {
        let (box_, app) = fixture("uninstall");
        let post = |body: &str| {
            h_plugins_uninstall(&app, &hrequest("POST", "/api/plugins/uninstall", body)).unwrap()
        };
        // `readmd.py:2044-2045`: `ok` mirrors `uninstall_plugin`'s bool and there is
        // no error path, so even a nonsense id is a 200.
        for (body, id, removed) in [
            ("{}", "", false),
            ("{\"plugin_id\":\"\"}", "", false),
            ("{\"plugin_id\":null}", "None", false),
            ("{\"plugin_id\":\"no_such_plugin\"}", "no_such_plugin", false),
        ] {
            let res = post(body);
            assert_eq!(res.status, 200, "{body}");
            let payload = json_of(&res);
            assert_eq!(
                payload,
                json!({ "ok": removed, "plugin_id": id }),
                "{body}"
            );
            assert_eq!(payload.as_object().unwrap().len(), 2, "{body}");
            assert!(payload.get("error_code").is_none(), "{body}");
        }
        // A member with no artefacts is still "uninstalled successfully": Python
        // deletes nothing and writes the flag (`plugin_manager.py:992-997`).
        let res = post("{\"plugin_id\":\"docling\"}");
        assert_eq!(
            json_of(&res),
            json!({ "ok": true, "plugin_id": "docling" })
        );
        assert_eq!(read_manifest_data(&box_)["docling"]["uninstalled"], json!(true));
        // A member that is really installed has its artefacts removed first.
        fake_installed(&box_, "pypandoc", "pypandoc-binary");
        assert!(is_plugin_installed(&Sandbox::for_app(&app), "pandoc_bridge"));
        let res = post("{\"plugin_id\":\"  pandoc_bridge  \"}");
        assert_eq!(res.status, 200);
        assert_eq!(
            json_of(&res),
            json!({ "ok": true, "plugin_id": "pandoc_bridge" })
        );
        assert!(!is_plugin_installed(&Sandbox::for_app(&app), "pandoc_bridge"));
        // 405 (`readmd.py:2037`).
        let res = h_plugins_uninstall(&app, &hrequest("GET", "/api/plugins/uninstall", "{}")).unwrap();
        assert_eq!(res.status, 405);
        assert_eq!(json_of(&res), method_not_allowed());
    }

    #[test]
    fn runtime_connected_mirrors_the_python_connected_plugins_set() {
        // `plugin_manager.py:158-162`, consumed verbatim at `:236` as
        // `'runtime_connected': pid in CONNECTED_PLUGINS`.
        const CONNECTED: &[&str] = &[
            "easyocr",
            "rapidocr",
            "rapid_table",
            "whisper",
            "faster_whisper",
            "docling",
            "pymupdf4llm",
            "markdownify",
            "trafilatura",
            "charset_normalizer",
        ];
        assert_eq!(CONNECTED.len(), 10);
        assert_eq!(PLUGIN_SPECS.len(), 14);
        for spec in PLUGIN_SPECS {
            assert_eq!(
                spec.runtime_connected,
                CONNECTED.contains(&spec.id),
                "runtime_connected drifted for {}",
                spec.id
            );
        }
        // The defect itself: the *default* PDF provider is wired into the
        // pipeline, so `docling` and `pymupdf4llm` must both read as connected.
        assert!(plugin_spec("pymupdf4llm").unwrap().runtime_connected);
        assert!(plugin_spec("docling").unwrap().runtime_connected);
        // The four lightweight tool kits are the only unconnected ones.
        for pid in ["pylatexenc", "jieba", "pygments", "pandoc_bridge"] {
            assert!(!plugin_spec(pid).unwrap().runtime_connected, "{pid}");
        }
        // And `/api/plugins/list` surfaces the flag unchanged.
        let box_ = sandbox("connected");
        let manifest = plugin_manifest(&box_);
        assert_eq!(manifest["pymupdf4llm"]["runtime_connected"], json!(true));
        assert_eq!(manifest["jieba"]["runtime_connected"], json!(false));
        let payload = plugins_list_payload_for(&box_);
        assert_eq!(
            payload["plugins"]["pymupdf4llm"]["runtime_connected"],
            json!(true)
        );
    }

    #[test]
    fn plugin_id_uses_one_python_str_coercion_in_all_three_body_handlers() {
        // Fix 3: the three body handlers must agree on `str(body.get('plugin_id',
        // ''))` (`readmd.py:1990`, `:2017`, `:2042`), and they now all read it
        // through `py_str_field`.  A member id padded with whitespace proves the
        // `.strip()` half identically: every handler echoes the *trimmed* id in
        // its 200 body.  Order matters — install schedules a worker whose
        // `installing` task makes `uninstall_plugin` refuse that same id, so the
        // uninstall runs while the artefacts are still there and untouched.
        // `whisper` is used here (not `pandoc_bridge`, which the uninstall test
        // removes) because `_install_tasks` is one process-wide map.
        let (box_, app) = fixture("py-str");
        fake_installed(&box_, "whisper", "openai-whisper");
        let padded = "{\"plugin_id\":\"  whisper  \",\"enabled\":true}";
        let toggle =
            h_plugins_toggle(&app, &hrequest("POST", "/api/plugins/toggle", padded)).unwrap();
        let uninstall =
            h_plugins_uninstall(&app, &hrequest("POST", "/api/plugins/uninstall", padded))
                .unwrap();
        let install =
            h_plugins_install(&app, &hrequest("POST", "/api/plugins/install", padded)).unwrap();
        assert_eq!(toggle.status, 200);
        assert_eq!(uninstall.status, 200);
        assert_eq!(install.status, 200);
        for res in [&install, &toggle, &uninstall] {
            assert_eq!(json_of(res)["plugin_id"], json!("whisper"));
        }
        // The container probe: Python's `str({'a': 1})` is the dict repr, i.e. a
        // non-empty string that cannot match a member id, so install and toggle
        // both answer `plugin_not_integrated` and uninstall still answers 200
        // `ok: false`.  `py_str_field` spells that repr as JSON rather than
        // Python — a pre-existing gap shared by all three handlers, which this
        // fix keeps uniform rather than widening.
        let weird = "{\"plugin_id\":{\"a\":1}}";
        assert_eq!(
            json_of(&h_plugins_install(&app, &hrequest("POST", "/api/plugins/install", weird)).unwrap()),
            json!({ "ok": false, "error_code": "plugin_not_integrated" })
        );
        assert_eq!(
            json_of(&h_plugins_toggle(&app, &hrequest("POST", "/api/plugins/toggle", weird)).unwrap()),
            json!({ "ok": false, "error_code": "plugin_not_integrated" })
        );
        let out = json_of(&h_plugins_uninstall(&app, &hrequest("POST", "/api/plugins/uninstall", weird)).unwrap());
        assert_eq!(out["ok"], json!(false));
        assert_eq!(
            out["plugin_id"],
            json!(py_str_field(&json!({ "plugin_id": { "a": 1 } }), "plugin_id"))
        );
        // Every JSON type coerces to a *non-empty* string, so none of them can be
        // mistaken for the missing id that trips install's `plugin_id_required`
        // check (`readmd.py:1991`) — only `""`/whitespace does.
        for (value, expected) in [
            (json!(null), "None"),
            (json!(true), "True"),
            (json!(false), "False"),
            (json!(123), "123"),
            (json!("whisper"), "whisper"),
        ] {
            let body = json!({ "plugin_id": value });
            assert_eq!(py_str_field(&body, "plugin_id"), expected);
            let res = h_plugins_install(&app, &hrequest("POST", "/api/plugins/install", &body.to_string())).unwrap();
            assert_ne!(
                json_of(&res)["error_code"],
                json!("plugin_id_required"),
                "`str({value})` == `{expected}` is non-empty in Python too"
            );
        }
    }
}
