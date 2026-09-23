//! Batch 2: update, diagram, export, plugins, skill-imports, convert
//! Offline-feasible routes that don't require DEFLATE inflate or office conversion

use crate::error::{ApiError, ApiResult};
use crate::server::{Request, Response, ok_json};
use crate::App;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

// ============================================================================
// Helper functions
// ============================================================================

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn generate_uuid() -> String {
    // Simple UUID v4 simulation using rand if available, otherwise fallback
    // For offline mode, we'll use a simple counter-based approach
    static COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let now = now_millis() as u64;
    format!("{:08x}-{:04x}-{:04x}-{:04x}-{:012x}", 
            n ^ now,
            (n >> 32) as u16,
            (n >> 48) as u16,
            (now >> 16) as u16,
            (now & 0xFFFF_FFFF_FFFF)
    )
}

/// Where Python stages an update package.
///
/// `updater._safe_update_target` (`updater.py:74-86`) finishes with
/// `return os.path.join(tempfile.gettempdir(), 'ReadMDUpdates', name)`: the
/// staging folder is the **system temp dir**, not anything under the app's
/// `data_dir`, and it is a plain persistent folder (it is *not* a
/// `tempfile.TemporaryDirectory`, which would be private and auto-deleted — the
/// installer has to outlive the process that downloaded the package).
/// `clean_old_update_artifacts()` (`updater.py:204-224`) sweeps the same folder.
fn temp_update_dir() -> std::path::PathBuf {
    std::env::temp_dir().join("ReadMDUpdates")
}

// ============================================================================
// /api/update/* — port of `src/readmd_modules/updater.py` + `readmd.py:1540-1595`
//
// `ureq` (the crate's only HTTP client, as in `h_plugin_catalog` and
// `diagrams::fetch_plantuml_svg`) does the transport; nothing here spawns a
// process.
// ============================================================================

/// `updater.GITHUB_REPO` (`updater.py:30`).
const GITHUB_REPO: &str = "Natsummerance/readMD";

/// `updater.check_update(..., timeout=2.5)` — the same budget on every tier.
const UPDATE_FETCH_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(2500);

/// `updater.TRANSIENT_FETCH_ATTEMPTS` / `TRANSIENT_FETCH_BACKOFF`.
const UPDATE_FETCH_ATTEMPTS: usize = 2;
const UPDATE_FETCH_BACKOFF: std::time::Duration = std::time::Duration::from_millis(300);

/// `updater._download_state` (`updater.py:51-65`) — the thirteen keys
/// `get_download_status()` copies out for `/api/update/status`, declared in
/// Python's own order with Python's own defaults.  `status` is
/// `idle | downloading | verifying | ready | error | cancelled`.
///
/// Note what is *not* here: there is no `downloaded`, `available`, `version` or
/// `install_ready` key on this contract.  Completion is `status == "ready"`, and
/// Python only ever writes `"ready"` after `compute_file_sha256()` has matched
/// `expected_sha` (`updater.py:685-694`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DownloadState {
    pub running: bool,
    pub total_bytes: u64,
    pub downloaded_bytes: u64,
    pub speed_bps: u64,
    pub percent: u64,
    pub status: String,
    pub error: String,
    pub error_code: String,
    pub target_file: String,
    pub cancel_requested: bool,
    pub asset_name: String,
    pub expected_sha: String,
    pub verified_sha: String,
}

impl Default for DownloadState {
    fn default() -> Self {
        // `updater.py:51-65`, verbatim: every string starts empty and
        // `status` starts at `idle`.
        DownloadState {
            running: false,
            total_bytes: 0,
            downloaded_bytes: 0,
            speed_bps: 0,
            percent: 0,
            status: "idle".to_string(),
            error: String::new(),
            error_code: String::new(),
            target_file: String::new(),
            cancel_requested: false,
            asset_name: String::new(),
            expected_sha: String::new(),
            verified_sha: String::new(),
        }
    }
}

static DOWNLOAD_STATE: std::sync::Mutex<Option<DownloadState>> = std::sync::Mutex::new(None);

/// `updater._download_lock` + the dict, read as one snapshot.
fn download_state() -> DownloadState {
    DOWNLOAD_STATE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .unwrap_or_default()
}

/// `updater._download_state.update({...})` under the lock.
fn with_download_state<F: FnOnce(&mut DownloadState)>(f: F) {
    let mut guard = DOWNLOAD_STATE.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(DownloadState::default));
}

/// `readmd.py:1541` — `if _STARTUP_PROBE.get('enabled')`.  `--startup-probe`
/// makes the check answer `probe_mode` instead of touching the network, because
/// the probe must measure boot, not GitHub.  `READMD_STARTUP_PROBE=1` is the
/// host-side seam (same env-override style as `READMD_FORCE_WIN7`).
static UPDATE_PROBE_MODE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_startup_probe_mode(on: bool) {
    UPDATE_PROBE_MODE.store(on, std::sync::atomic::Ordering::SeqCst);
}

pub(crate) fn startup_probe_enabled() -> bool {
    UPDATE_PROBE_MODE.load(std::sync::atomic::Ordering::SeqCst)
        || std::env::var("READMD_STARTUP_PROBE")
            .map(|v| v == "1")
            .unwrap_or(false)
}

/// `UpdateCheckRequest.force` — read, then deliberately given no power.
///
/// Python's `/api/update/check` is dispatched from `do_GET` (`readmd.py:1297`)
/// into `_api_update_check` (`readmd.py:1540-1550`), which never reads a body or
/// a query, and calls `updater.check_update(VERSION)` (`updater.py:386`), whose
/// only arguments are `current_version` and `timeout`.  So there is **no force
/// semantics to port**, and the honest port is: a forced check answers exactly
/// what a plain check answers.  In particular `force` does not bypass —
///
/// * the startup-probe refusal (`readmd.py:1541`, mirrored by the UI's own
///   manual check at `assets/js/features/updater.js:11`), or
/// * the release-channel rule (`versioning.select_update_release`: "Formal
///   builds stay on the formal channel and therefore never receive a beta or
///   release candidate"), or
/// * the SHA-256 trust gate on the download lane
///   (`updater.validate_update_source`).
///
/// `bool(body.get('force', False))` is how Python would have read it, so the
/// truthiness table is Python's (`0`/`""`/`[]`/`null`/absent → false, anything
/// else — including the string `"false"` — → true).
#[derive(Debug, Deserialize, Default)]
pub struct UpdateCheckRequest {
    #[serde(default)]
    pub force: bool,
}

fn py_truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Object(o)) => !o.is_empty(),
    }
}

/// Read `force` without ever failing the route: Python reads no body here, so a
/// missing, short or malformed body simply means "not forced".
fn update_check_force(req: &Request) -> bool {
    let declared = match content_length(req) {
        Some(v) => v.max(0) as usize,
        None => 0,
    };
    if declared == 0 || req.body.len() < declared {
        return false;
    }
    serde_json::from_slice::<Value>(&req.body[..declared])
        .ok()
        .and_then(|body| body.get("force").cloned())
        .map(|v| py_truthy(Some(&v)))
        .unwrap_or(false)
}

pub(crate) fn h_update_check(_app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    // `readmd.py:1541-1543` — 200 with two keys, before the `try`.
    if startup_probe_enabled() {
        return ok_json(json!({ "ok": false, "error_code": "probe_mode" }));
    }
    // Parsed on purpose (see `update_check_force`), and it changes nothing.
    let _force = update_check_force(req);
    // `readmd.py:1546` — `updater.check_update(VERSION)`: the check is always
    // against *this app's* own version constant, so the kernel reads the same
    // `server::VERSION` that `/api/version` and the `X-ReadMD-Engine` banner
    // answer with instead of re-deriving it from the cargo environment here.
    let current: &str = crate::server::VERSION;
    let payload = check_update(current, &mut |url| fetch_text(url, UPDATE_FETCH_TIMEOUT));
    ok_json(payload)
}

/// `updater.check_update` (`updater.py:386-466`) minus its two socket calls,
/// which arrive as `manifest` (the `SHA256SUMS.txt` reader,
/// `updater.resolve_expected_sha`).
fn check_update(
    current_version: &str,
    manifest: &mut dyn FnMut(&str) -> Option<String>,
) -> Value {
    let mut last_error_code = String::new();
    let mut data: Option<Value> = None;
    // `updater._release_check_urls`: `/latest` hides prereleases, so a prerelease
    // build scans the release list instead.
    for url in release_check_urls(current_version) {
        match fetch_release_json(&url) {
            Ok(payload) => {
                let releases: Vec<Value> = match payload {
                    Value::Array(items) => items,
                    other => vec![other],
                };
                let picked = crate::validators::select_update_release(current_version, &releases);
                // Decided before the move: `tag_name` is only read as "did the
                // picker land on a real release".
                let has_tag = picked
                    .as_ref()
                    .and_then(|d| d.get("tag_name"))
                    .and_then(|v| v.as_str())
                    .map(|t| !t.is_empty())
                    .unwrap_or(false);
                data = picked;
                if has_tag {
                    break;
                }
            }
            // "Keep provider/network details in logs only. The UI receives a
            // stable code" (`updater.py:398-403`).
            Err(_) => {
                last_error_code = "update_network_error".to_string();
                continue;
            }
        }
    }

    // `updater.py:405-421` — tiers 2 and 3 (`/releases/latest` 302 sniffing plus
    // mirror sniffing, then manifest re-assembly) are NOT ported: they are the
    // check's only egress to `ghfast.top` / `ghproxy.net`, and this crate does
    // not add mirror hosts.  The failure is reported honestly as
    // `update_network_error`, which is what Python answers when the sniff also
    // comes back empty.
    release_check_payload(current_version, data.as_ref(), &last_error_code, manifest)
}

/// The pure tail of `updater.check_update` (`updater.py:423-466`), split out so
/// the wire contract is testable without a socket.
fn release_check_payload(
    current_version: &str,
    data: Option<&Value>,
    last_error_code: &str,
    manifest: &mut dyn FnMut(&str) -> Option<String>,
) -> Value {
    let data = match data {
        None => {
            return json!({
                "ok": false,
                "error_code": if last_error_code.is_empty() {
                    "update_network_error".to_string()
                } else {
                    last_error_code.to_string()
                },
                "html_url": format!("https://github.com/{}/releases", GITHUB_REPO),
            })
        }
        Some(d) => d,
    };
    // `updater.py:430-463`; anything that escapes this block is
    // `{'ok': False, 'error_code': 'update_response_invalid'}` (`updater.py:464-466`).
    //
    // `latest_tag = data.get('tag_name', '')` (`updater.py:431`) is a *raw* echo, and
    // Python distinguishes the three shapes a JSON document can carry: an absent key
    // is `''`, an explicit `null` is `None`, a number stays a number.  Collapsing
    // them to `""` (the old behaviour) invented an empty version out of a malformed
    // release and made `"latest_version": null` unreachable.
    let latest_tag: Value = data.get("tag_name").cloned().unwrap_or_else(|| json!(""));
    // `is_newer_version(latest_tag, current_version)` →
    // `compare_versions` → `parse_version(str(value or '').strip())`
    // (`versioning.py:12-25`, `updater.py:95-97`).  The coercion is Python's
    // `str(x or '')`, so `null` / `0` / `false` / `""` all become `""` and parse to
    // nothing, and an unparseable tag is never "newer" (`compare_versions` returns
    // `None`, `is_newer_version` asks for exactly `1`).
    let has_update =
        crate::validators::is_newer_version(&py_str_or_empty(&latest_tag), &current_version);
    let flavor = detect_app_flavor();
    let assets = match crate::updater::release_asset_list(data.get("assets")) {
        Some(assets) => assets,
        None => return json!({"ok": false, "error_code": "update_response_invalid"}),
    };
    let (best_asset, sha_asset) = match_release_asset(&assets, &flavor);
    let sha_url = sha_asset
        .as_ref()
        .and_then(|a| a.get("browser_download_url"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let asset_name = best_asset
        .as_ref()
        .and_then(|a| a.get("name"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // `updater.py:436-444`: `if best_asset and best_asset.get('expected_sha')` is a
    // truthiness test on the *raw* value (the manifest-assembly tiers are the only
    // producers of that key), otherwise `resolve_expected_sha` reads it out of
    // `SHA256SUMS.txt`.
    let mut expected_sha = best_asset
        .as_ref()
        .and_then(|a| a.get("expected_sha"))
        .filter(|v| py_truthy(Some(v)))
        .cloned();
    if expected_sha.is_none() {
        expected_sha = resolve_expected_sha(sha_url.as_deref(), asset_name.as_deref(), manifest);
    }

    json!({
        "ok": true,
        "has_update": has_update,
        "current_version": current_version,
        "latest_version": latest_tag.clone(),
        // `data.get('name') or latest_tag` (`updater.py:451`) — the fallback is the
        // raw tag value, so an unnamed release of a `null`-tagged document answers
        // `"release_name": null`.
        "release_name": match data.get("name") {
            Some(name) if py_truthy(Some(name)) => name.clone(),
            _ => latest_tag.clone(),
        },
        "published_at": data.get("published_at").cloned().unwrap_or_else(|| json!("")),
        "release_notes": data.get("body").cloned().unwrap_or_else(|| json!("")),
        "html_url": data.get("html_url").cloned().unwrap_or_else(|| json!("")),
        "flavor": flavor,
        "asset": best_asset.map(|a| json!({
            "name": a.get("name").cloned().unwrap_or(Value::Null),
            "size": a.get("size").cloned().unwrap_or_else(|| json!(0)),
            "download_url": a.get("browser_download_url").cloned().unwrap_or(Value::Null),
            "expected_sha": expected_sha.clone().unwrap_or(Value::Null),
        })),
        "sha_url": sha_url.map(Value::String).unwrap_or(Value::Null),
    })
}

/// `updater.resolve_expected_sha` (`updater.py:265-281`) with its one socket call
/// folded into `manifest`.
///
/// The first thing it does is `if not sha_url or not asset_name: return None`
/// (`updater.py:267-268`) — **before** any request.  Skipping that guard is not
/// merely wasteful: with an empty `sha_url` the caller would fetch `""` and the
/// answer would come from whatever the fetch helper invents for a meaningless URL,
/// whereas Python answers `None` and the download lane then refuses the package for
/// want of a digest (`updater.py:504-505`).
fn resolve_expected_sha(
    sha_url: Option<&str>,
    asset_name: Option<&str>,
    manifest: &mut dyn FnMut(&str) -> Option<String>,
) -> Option<Value> {
    match (sha_url, asset_name) {
        (Some(url), Some(name)) if !url.is_empty() && !name.is_empty() => {
            // `_fetch_text(sha_url, timeout=timeout) or ''`, and an unreachable
            // manifest is swallowed into `None` (`updater.py:269-273`).
            let text = manifest(url)?;
            // `os.path.basename(...).casefold()` per line, digest lowercased
            // (`updater.py:275-279`).
            crate::validators::sha256sums_lookup(&text, name).map(Value::String)
        }
        _ => None,
    }
}

/// Python's `str(value or '')` (`versioning.py:14`): falsy → `''`, otherwise the
/// value's own `str()`.
fn py_str_or_empty(value: &Value) -> String {
    if py_truthy(Some(value)) {
        crate::mdexport::py_str(value)
    } else {
        String::new()
    }
}

/// `updater._release_check_urls` (`updater.py:100-104`).
fn release_check_urls(current_version: &str) -> Vec<String> {
    let current = crate::validators::parse_version(current_version);
    let is_prerelease = current.map(|v| v.rank == 0).unwrap_or(false);
    vec![if is_prerelease {
        format!("https://api.github.com/repos/{}/releases?per_page=100", GITHUB_REPO)
    } else {
        format!("https://api.github.com/repos/{}/releases/latest", GITHUB_REPO)
    }]
}

/// `updater.detect_app_flavor` (`updater.py:107-121`).  The kernel binary is the
/// packaged app, i.e. Python's `getattr(sys, 'frozen', False)` branch:
/// `win_portable` when the executable is named `*portable*`, `win_installer`
/// otherwise.
fn detect_app_flavor() -> String {
    if cfg!(target_os = "macos") {
        return "macos".to_string();
    }
    if cfg!(windows) {
        let exe_name = std::env::current_exe()
            .ok()
            .and_then(|p| {
                p.file_name()
                    .map(|n| n.to_string_lossy().to_lowercase())
            })
            .unwrap_or_default();
        return if exe_name.contains("portable") {
            "win_portable".to_string()
        } else {
            "win_installer".to_string()
        };
    }
    "linux".to_string()
}

/// `platform.machine().lower()` (`updater.py:129`).  CPython reports `AMD64` on
/// Windows x86-64 where `consts::ARCH` reports `x86_64`; the tokens below only
/// matter on the `linux` flavor, so the platform default is kept.
fn platform_machine() -> String {
    std::env::consts::ARCH.to_lowercase()
}

fn asset_name(a: &Value) -> String {
    a.get("name").and_then(|v| v.as_str()).unwrap_or("").to_lowercase()
}

/// `updater.match_release_asset` (`updater.py:124-201`), Python's order and
/// Python's early-outs.  Returns `(selected, sha_asset)`.
fn match_release_asset(assets: &[Value], flavor: &str) -> (Option<Value>, Option<Value>) {
    let machine = platform_machine();
    let is_arm = machine.contains("arm") || machine.contains("aarch64");
    let mut selected: Option<Value> = None;
    let mut sha_asset: Option<Value> = None;

    for a in assets {
        let name = a.get("name").and_then(|v| v.as_str()).unwrap_or("").to_uppercase();
        if name == "SHA256SUMS.TXT" || name.contains("SHA256") {
            sha_asset = Some(a.clone());
        }
    }

    for a in assets {
        let name = asset_name(a);
        if flavor == "win_portable" {
            if name.contains("portable") && (name.ends_with(".exe") || name.ends_with(".zip")) {
                selected = Some(a.clone());
                break;
            }
        } else if flavor == "win_installer" || flavor == "source" {
            if (name.contains("setup") || name.contains("readmdsetup")) && name.ends_with(".exe") {
                selected = Some(a.clone());
                break;
            } else if selected.is_none() && name.ends_with(".exe") && !name.contains("portable") {
                selected = Some(a.clone());
            }
        } else if flavor == "macos" {
            if name.ends_with(".zip") || name.ends_with(".dmg") {
                if is_arm && name.contains("arm64") {
                    selected = Some(a.clone());
                    break;
                } else if !is_arm
                    && (name.contains("x64") || name.contains("x86_64") || name.contains("intel"))
                {
                    selected = Some(a.clone());
                    break;
                } else if selected.is_none() && name.contains("macos") {
                    selected = Some(a.clone());
                }
            }
        } else if flavor == "linux" {
            let (deb_token, appimage_token): (&str, &str) =
                if machine.contains("loongarch") {
                    ("loongarch64", "loongarch64")
                } else if machine.contains("mips") {
                    ("mips64el", "mips64el")
                } else if machine.contains("sw_64") || machine.contains("sw64") {
                    ("sw64", "sw64")
                } else if machine.contains("riscv") {
                    ("riscv64", "riscv64")
                } else if machine.starts_with("armv") || machine.contains("armhf") {
                    ("armhf", "armhf")
                } else if is_arm || machine == "aarch64" || machine == "arm64" {
                    ("arm64", "aarch64")
                } else {
                    ("amd64", "x86_64")
                };
            if name.ends_with(&format!("_{}.deb", deb_token)) {
                selected = Some(a.clone());
                break;
            } else if name.contains(&format!("-{}-", appimage_token)) && name.ends_with(".appimage")
            {
                selected = Some(a.clone());
                break;
            } else if selected.is_none() && name.ends_with(".deb") {
                selected = Some(a.clone());
            }
        }
    }

    // `updater.py:187-199` — "如果没匹配到精准架构，选取最接近的 exe 或 zip".
    // Python compares against `'.AppImage'` after having lower-cased the name, so
    // that alternative can never match; the parity behaviour is `.deb` only.
    if selected.is_none() && !assets.is_empty() {
        for a in assets {
            let name = asset_name(a);
            if cfg!(windows) && name.ends_with(".exe") {
                selected = Some(a.clone());
                break;
            } else if cfg!(target_os = "macos") && name.ends_with(".zip") {
                selected = Some(a.clone());
                break;
            } else if cfg!(target_os = "linux") && name.ends_with(".deb") {
                selected = Some(a.clone());
                break;
            }
        }
    }

    (selected, sha_asset)
}

/// `updater._open_with_retry` + `_fetch_release_json` (`updater.py:233-255`):
/// two attempts on transient transport failures (a 4xx/5xx is a definitive
/// answer and is never retried), 2.5s budget, `ReadMD-Updater` UA.
fn fetch_release_json(url: &str) -> Result<Value, String> {
    let mut last = String::from("update_network_error");
    for attempt in 0..UPDATE_FETCH_ATTEMPTS {
        match ureq::get(url)
            .set("User-Agent", "ReadMD-Updater")
            .set("Accept", "application/vnd.github.v3+json")
            .timeout(UPDATE_FETCH_TIMEOUT)
            .call()
        {
            Ok(resp) => {
                return resp
                    .into_json::<Value>()
                    .map_err(|e| format!("update_response_invalid: {e}"))
            }
            Err(ureq::Error::Status(code, _)) => {
                return Err(format!("http {code}"));
            }
            Err(err) => {
                last = format!("{err:?}");
                if attempt + 1 < UPDATE_FETCH_ATTEMPTS {
                    std::thread::sleep(UPDATE_FETCH_BACKOFF);
                }
            }
        }
    }
    Err(last)
}

/// `updater._fetch_text` (`updater.py:258-262`) — the plain-text twin, used for
/// `SHA256SUMS.txt`.  Only ever called with an official `github.com` URL.
fn fetch_text(url: &str, timeout: std::time::Duration) -> Option<String> {
    let resp = ureq::get(url)
        .set("User-Agent", "ReadMD-Updater")
        .timeout(timeout)
        .call()
        .ok()?;
    let mut data: Vec<u8> = Vec::new();
    use std::io::Read;
    resp.into_reader().read_to_end(&mut data).ok()?;
    Some(String::from_utf8_lossy(&data).into_owned())
}

// ---------------------------------------------------------------- download lane

/// One staged download, i.e. what `download_asset_thread` receives as its
/// closure over `updater.py:546-550`.
#[derive(Debug, Clone)]
struct DownloadJob {
    url: String,
    save_path: PathBuf,
    part_path: PathBuf,
    expected_sha: String,
}

/// `updater.validate_update_source` (`updater.py:500-532`) — the trust boundary,
/// in Python's order, with Python's messages.  Returns `(url, error)`; a non-empty
/// `error` is the refusal.
fn validate_update_source(
    download_url: Option<&Value>,
    target_filename: Option<&Value>,
    expected_sha: Option<&Value>,
    use_mirror: bool,
) -> (String, String) {
    let download_url = match download_url {
        Some(Value::String(s)) => s.clone(),
        None => String::new(),
        Some(_) => return (String::new(), "更新参数无效".to_string()),
    };
    let target_filename = match target_filename {
        Some(Value::String(s)) => s.clone(),
        None => String::new(),
        Some(_) => return (String::new(), "更新参数无效".to_string()),
    };
    let sha_text = match expected_sha {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(other) => crate::mdexport::py_str(&other),
    };
    if sha_text.is_empty()
        || sha_text.len() != 64
        || !sha_text.chars().all(|c| c.is_ascii_hexdigit())
    {
        return (String::new(), "缺少有效的 SHA256 校验值".to_string());
    }
    if !download_url.starts_with("https://") {
        return (String::new(), "仅允许官方 GitHub Release 下载地址".to_string());
    }
    let rest = &download_url["https://".len()..];
    let authority = rest.split('/').next().unwrap_or("");
    let path = &rest[authority.len()..];
    if authority != "github.com" {
        return (String::new(), "仅允许官方 GitHub Release 下载地址".to_string());
    }
    // `parsed.path != os.path.normpath(parsed.path).replace(os.sep, '/')`
    if path != normalize_url_path(path) {
        return (String::new(), "下载地址路径无效".to_string());
    }
    if !path.starts_with(&format!("/{}/releases/download/", GITHUB_REPO)) {
        return (String::new(), "下载地址不在官方发布目录内".to_string());
    }
    let url_name = percent_decode_tail(path.rsplit('/').next().unwrap_or(""));
    if url_name.is_empty()
        || url_name.contains('/')
        || url_name.contains("\\")
        || target_filename != url_name
        || target_filename.contains("..")
        || !update_filename_re_ok(&target_filename)
    {
        return (String::new(), "更新文件名无效".to_string());
    }
    if use_mirror {
        // `MIRROR_PREFIXES[0]` — the mirror is a prefix on the official URL and
        // nothing else, so this validates the *prefix*, not a new host.
        if !matches!(
            url_host_prefix_ok("https://ghfast.top/"),
            true
        ) {
            return (String::new(), "镜像地址无效".to_string());
        }
    }
    (download_url, String::new())
}

fn url_host_prefix_ok(_prefix: &str) -> bool {
    // `MIRROR_PREFIXES[0]` is a constant `https://ghfast.top/`, so Python's
    // scheme/host assertion always passes for it.  The mirror is nevertheless
    // never *tried* by this crate: no new egress hosts (see `check_update`).
    true
}

/// `os.path.normpath` on an already-slash URL path: collapse `//`, `.` and `..`.
fn normalize_url_path(path: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "." => {}
            ".." => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    let joined = out.join("/");
    if path.starts_with('/') && !joined.starts_with('/') {
        format!("/{}", joined)
    } else {
        joined
    }
}

/// `urllib.parse.unquote(os.path.basename(parsed.path))` (`updater.py:518`).
fn percent_decode_tail(name: &str) -> String {
    percent_encoding::percent_decode_str(name)
        .decode_utf8_lossy()
        .into_owned()
}

/// `updater._UPDATE_FILENAME_RE.fullmatch(target_filename)` (`updater.py:521`,
/// pattern at `updater.py:68-71`):
/// `^[A-Za-z0-9][A-Za-z0-9._()-]{0,180}\.(?:exe|zip|dmg|deb|appimage)$`,
/// `re.IGNORECASE`.
///
/// Ported here rather than borrowed from `validators::update_filename_re_ok`,
/// because that helper's body class is missing `-`: it answers *false* for
/// `ReadMD-portable-2.4.0.exe` and `ReadMDSetup-9.9.9.exe`, i.e. for exactly the
/// names `match_release_asset` (`updater.py:124-201`) selects, so the trust gate
/// refused every legitimate package with `'更新文件名无效'`.
///
/// `fullmatch` anchors both ends (`\A`/`\Z`), so a name whose only difference is a
/// trailing newline is rejected, and the `{0,180}` bound counts the characters
/// *between* the mandatory first alnum character and the extension dot.
fn update_filename_re_ok(name: &str) -> bool {
    const EXTENSIONS: [&str; 5] = ["exe", "zip", "dmg", "deb", "appimage"];
    // `re.IGNORECASE` only ever folds ASCII here: every character the pattern
    // accepts is ASCII, so a non-ASCII letter stays rejected by the class.
    let lowered = name.to_ascii_lowercase();
    let (stem, extension) = match lowered.rsplit_once('.') {
        Some(pair) => pair,
        None => return false,
    };
    if !EXTENSIONS.contains(&extension) {
        return false;
    }
    let stem: Vec<char> = stem.chars().collect();
    // `[A-Za-z0-9]` then `[A-Za-z0-9._()-]{0,180}`.
    if stem.is_empty() || !stem[0].is_ascii_alphanumeric() || stem.len() - 1 > 180 {
        return false;
    }
    stem[1..]
        .iter()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '(' | ')' | '-'))
}

/// `updater.start_download_update`'s two different failure shapes, which
/// `readmd.py:1552-1568` reports differently and the old handler conflated:
///
/// * `Refused(message)` is a `return False, msg` — validate_update_source's
///   trust refusal, `_safe_update_target`'s `ValueError` (`updater.py:724-725`)
///   or the "already running" guard — and becomes
///   `400 {'ok': False, 'message': msg}`.
/// * `Failed` is an *exception* escaping into
///   `except Exception: self._send_api_error(500, 'update_download_failed')`,
///   i.e. `500 {'ok': False, 'error_code': 'update_download_failed'}` with no
///   `message` key at all.  `os.makedirs(os.path.dirname(save_path))`
///   (`updater.py:726`) is the one call in this lane outside a `try`.
#[derive(Debug, PartialEq)]
enum StartError {
    Refused(String),
    Failed,
}

/// `updater.start_download_update` (`updater.py:717-756`) up to and including
/// the state update — the `_download_state` half that a test can assert without
/// letting a worker thread touch the socket.  `Err(Refused(message))` is the
/// `return False, error` answer; `Ok(job)` means `running` is now raised and the
/// caller owes the thread.
fn begin_download(
    download_url: Option<&Value>,
    target_filename: Option<&Value>,
    expected_sha: Option<&Value>,
    use_mirror: bool,
) -> Result<DownloadJob, StartError> {
    let (_url, error) =
        validate_update_source(download_url, target_filename, expected_sha, use_mirror);
    if !error.is_empty() {
        return Err(StartError::Refused(error));
    }
    let filename = match download_url {
        Some(Value::String(s)) => s.rsplit('/').next().unwrap_or("").to_string(),
        _ => String::new(),
    };
    let filename = match target_filename {
        Some(Value::String(s)) => s.clone(),
        _ => filename,
    };
    // `_safe_update_target` → `<temp>/ReadMDUpdates/<name>` (`updater.py:74-86`):
    // `temp_update_dir()` *is* that folder, so `validators::safe_update_target`
    // gets its parent and only contributes Python's four `ValueError` guards.
    // Nothing under `data_dir` is ever an update staging area — the previous
    // handler's `app.paths.data_dir.join("updates")` was an invented location.
    let staging = temp_update_dir();
    let temp_root = staging
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let save_path = match crate::validators::safe_update_target(&filename, &temp_root) {
        Ok(path) => path,
        Err(exc) => return Err(StartError::Refused(exc)),
    };
    // `os.makedirs(os.path.dirname(save_path), exist_ok=True)` — unguarded, so a
    // failure is the route's 500.
    if std::fs::create_dir_all(&staging).is_err() && !staging.is_dir() {
        return Err(StartError::Failed);
    }
    let expected = match expected_sha {
        Some(v) => crate::mdexport::py_str(v).to_lowercase(),
        None => String::new(),
    };
    let url = match download_url {
        Some(Value::String(s)) => s.clone(),
        _ => String::new(),
    };
    // `.{name}.{uuid4().hex}.part` (`updater.py:548-550`): the published name is
    // only ever produced by a verified rename.
    let part_path = staging.join(format!(
        ".{}.{}.part",
        save_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("update"),
        uuid::Uuid::new_v4().simple()
    ));

    let job = DownloadJob {
        url,
        save_path,
        part_path,
        expected_sha: expected.clone(),
    };
    let mut guard = DOWNLOAD_STATE.lock().unwrap_or_else(|e| e.into_inner());
    let state = guard.get_or_insert_with(DownloadState::default);
    if state.running {
        return Err(StartError::Refused("已有下载任务正在进行".to_string()));
    }
    // `updater.py:730-743`.
    state.running = true;
    state.total_bytes = 0;
    state.downloaded_bytes = 0;
    state.speed_bps = 0;
    state.percent = 0;
    state.status = "downloading".to_string();
    state.error = String::new();
    state.error_code = String::new();
    state.target_file = job.save_path.to_string_lossy().to_string();
    state.asset_name = job
        .save_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();
    state.expected_sha = expected;
    state.verified_sha = String::new();
    state.cancel_requested = false;
    drop(guard);
    Ok(job)
}

/// `updater.py:535-621` — the worker's fetch half: the official URL first, then
/// the mirror prefixes.  This crate keeps only the official candidate (no new
/// egress hosts), so a failed official attempt is the `update_download_failed`
/// answer Python reaches at the end of its candidate loop.
fn run_download(job: DownloadJob) {
    let _running = RunningDownloadGuard;
    let resp = ureq::get(&job.url)
        .set("User-Agent", "ReadMD-Updater")
        .timeout(std::time::Duration::from_secs(25))
        .call();
    let resp = match resp {
        Ok(resp) => resp,
        Err(_) => {
            fail_download("update_download_failed");
            cleanup_part(&job.part_path);
            return;
        }
    };
    let total = resp
        .header("Content-Length")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    with_download_state(|state| state.total_bytes = total);
    let mut reader = resp.into_reader();
    if let Err(detail) = land_download(&mut reader, &job, total) {
        // `updater.py:696-714` — the download's own `except Exception` branch.
        log::debug!("download update failed: {}", detail);
        fail_download("update_download_failed");
        cleanup_part(&job.part_path);
    }
}

/// `updater.py:623-694` — stream to the `.part` file, `fsync`, re-read its
/// digest and only then publish the final name.  `downloaded`/`ready` are
/// therefore written *after* the bytes exist and verify; an unverified package
/// is never reported as usable.
fn land_download<R: std::io::Read>(
    reader: &mut R,
    job: &DownloadJob,
    total: u64,
) -> Result<String, String> {
    use std::io::{BufWriter, Write};

    let file = std::fs::File::create(&job.part_path)
        .map_err(|e| format!("cannot open {}: {e}", job.part_path.display()))?;
    let mut out = BufWriter::new(file);
    let mut chunk = vec![0u8; 65536];
    let mut written: u64 = 0;
    let start = std::time::Instant::now();
    let mut last_tick = start;
    let mut last_bytes: u64 = 0;
    loop {
        // `with _download_lock: if _download_state['cancel_requested']: break`
        // (`updater.py:636-638`) — the flag lives in the one shared state, so the
        // cancel route and this thread cannot drift apart.
        if download_state().cancel_requested {
            drop(out);
            cleanup_part(&job.part_path);
            with_download_state(|state| {
                state.status = "cancelled".to_string();
                state.running = false;
            });
            return Ok(String::new());
        }
        let n = reader
            .read(&mut chunk)
            .map_err(|e| format!("read failed: {e}"))?;
        if n == 0 {
            break;
        }
        out.write_all(&chunk[..n])
            .map_err(|e| format!("write failed: {e}"))?;
        written += n as u64;
        if start.elapsed().as_millis() >= 300 && last_tick.elapsed().as_millis() >= 300 {
            let dt = last_tick.elapsed().as_secs_f64().max(0.001);
            let speed = ((written - last_bytes) as f64 / dt) as u64;
            let percent = if total > 0 {
                ((written * 100) / total).min(100)
            } else {
                0
            };
            with_download_state(|state| {
                state.downloaded_bytes = written;
                state.speed_bps = speed;
                state.percent = percent;
            });
            last_tick = std::time::Instant::now();
            last_bytes = written;
        }
    }
    out.flush().map_err(|e| format!("flush failed: {e}"))?;
    let file = out
        .into_inner()
        .map_err(|e| format!("flush failed: {}", e.error()))?;
    file.sync_all().map_err(|e| format!("fsync failed: {e}"))?;

    // `updater.py:680-694` — verifying, digest, compare, publish.  `downloaded_bytes`
    // is deliberately *not* refreshed here: Python only publishes it on the 0.3s
    // progress cadence, so a package that lands faster than that is `ready` with a
    // stale byte counter and `percent == 100`.  Parity keeps the quirk.
    with_download_state(|state| {
        state.status = "verifying".to_string();
        state.percent = 100;
    });
    let actual = crate::crypto::sha256_file(&job.part_path)
        .map_err(|e| format!("digest failed: {e}"))?;
    if actual != job.expected_sha {
        return Err(format!(
            "SHA256 校验失败：期望 {}，实际 {}",
            job.expected_sha, actual
        ));
    }
    std::fs::rename(&job.part_path, &job.save_path)
        .map_err(|e| format!("rename failed: {e}"))?;
    with_download_state(|state| {
        state.status = "ready".to_string();
        state.running = false;
        state.verified_sha = actual.clone();
    });
    Ok(actual)
}

/// `updater.py:606-621` / `696-714` — the shared failure answer.
fn fail_download(code: &str) {
    with_download_state(|state| {
        state.status = "error".to_string();
        state.error = String::new();
        state.error_code = code.to_string();
        state.running = false;
        state.target_file = String::new();
        state.expected_sha = String::new();
    });
}

fn cleanup_part(path: &Path) {
    if path.is_file() {
        let _ = std::fs::remove_file(path);
    }
}

/// `readmd.py:1555-1556` / `readmd.py:1589-1590` — the two update routes that
/// take a body read it the **raw** way, inside the route's own `try`:
/// `length = int(self.headers.get('Content-Length', 0) or 0)` then
/// `json.loads(self.rfile.read(length).decode('utf-8')) if length > 0 else {}`.
/// So — unlike `_read_json_body` — there is no size gate and no 400: a header
/// `int()` rejects, a short read, invalid UTF-8 or a malformed document is the
/// *exception* that becomes `500 {'ok': False, 'error_code': …}`.
/// `Err(())` is that escape.
fn update_request_body(req: &Request) -> Result<Value, ()> {
    let declared = content_length(req).ok_or(())?;
    if declared <= 0 {
        // `if length > 0 else {}`, and `int('-5') > 0` is False.
        return Ok(json!({}));
    }
    let want = declared as usize;
    if req.body.len() < want {
        return Err(());
    }
    let text = std::str::from_utf8(&req.body[..want]).map_err(|_| ())?;
    serde_json::from_str::<Value>(text).map_err(|_| ())
}

/// `readmd.py:1552-1568` — `_api_update_download`.  The four body keys are the
/// whole contract; the response is `{ok, message}` (or `{ok, error}` for the
/// missing-parameters branch) at 200/400, never a claim about bytes.
pub(crate) fn h_update_download(_app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    let body = match update_request_body(req) {
        Ok(value) => value,
        Err(()) => return Ok(api_error(500, "update_download_failed")),
    };
    if !body.is_object() {
        // `body.get(...)` on a list/str is an `AttributeError` → the route's 500.
        return Ok(api_error(500, "update_download_failed"));
    }
    let download_url = body.get("download_url");
    let target_filename = body.get("target_filename");
    let expected_sha = body.get("expected_sha");
    let use_mirror = py_truthy(body.get("use_mirror"));
    if !py_truthy(download_url) || !py_truthy(target_filename) {
        return Ok(Response::json_status(
            400,
            &json!({ "ok": false, "error": "缺少下载参数" }),
        ));
    }
    match begin_download(download_url, target_filename, expected_sha, use_mirror) {
        // Python starts `download_asset_thread` as a daemon thread and answers
        // `{'ok': True, 'message': '下载已启动'}` — a statement about the *task*,
        // not the file.  `downloaded`/`ready` only ever appear on
        // `/api/update/status`, and only after `land_download` has verified the
        // bytes on disk.
        Ok(job) => {
            std::thread::spawn(move || run_download(job));
            Ok(Response::json_status(
                200,
                &json!({ "ok": true, "message": "下载已启动" }),
            ))
        }
        Err(StartError::Refused(message)) => Ok(Response::json_status(
            400,
            &json!({ "ok": false, "message": message }),
        )),
        Err(StartError::Failed) => Ok(api_error(500, "update_download_failed")),
    }
}

/// `readmd.py:1570-1580` — `_api_update_status` answers
/// `updater.get_download_status()`, i.e. a flat copy of `_download_state`.
pub(crate) fn h_update_status(_app: &Arc<App>, _req: &Request) -> ApiResult<Response> {
    let state = download_state();
    match serde_json::to_value(&state) {
        Ok(value) => ok_json(value),
        // `readmd.py:1576` — `_send_api_error(500, 'update_status_failed',
        // status='error')`, so the failure envelope carries three keys, not two.
        Err(_) => Ok(Response::json_status(500, &json!({
            "ok": false,
            "error_code": "update_status_failed",
            "status": "error",
        }))),
    }
}

/// Owns the download thread's `running` flag.
///
/// `updater.py:535-714` — `start_download_update()` raises
/// `_download_state['running']` before `t.start()` (`updater.py:730`), and
/// `download_asset_thread` lowers it on *every* exit path: the validation
/// failure (`:542`), the exhausted candidate list (`:612`), both cancel branches
/// (`:667`, `:677`), the verified success (`:693`) and the `except Exception`
/// tail (`:707`).  A `Drop` guard is the Rust spelling of "whatever happens, the
/// thread clears the flag": an early `return` on `cancel_requested`, a panic
/// inside `ureq`, or a failed `File::create` all still leave `running == false`,
/// so a later `/api/update/cancel` cannot keep answering `true` and a later
/// `/api/update/download` cannot stay wedged on `'已有下载任务正在进行'`.
struct RunningDownloadGuard;

impl Drop for RunningDownloadGuard {
    fn drop(&mut self) {
        with_download_state(|state| state.running = false);
    }
}

/// `readmd.py:1578-1584` — `self._send_json(200, {'ok': updater.cancel_download()})`.
///
/// `updater.cancel_download` (`updater.py:490-497`) is a two-line function: under
/// the lock, *if* `_download_state['running']`, raise `cancel_requested`, write
/// `status = 'cancelled'` and answer `True`; otherwise answer `False`.  So
/// "nothing is downloading" is a **false** answer, and there is no `cancelled`
/// key on the wire — the old stub invented one next to `ok`.  Note that `running`
/// deliberately stays `True` here: the worker clears it when it observes the flag
/// (`land_download`), exactly as Python's thread does.
pub(crate) fn h_update_cancel(_app: &Arc<App>, _req: &Request) -> ApiResult<Response> {
    let cancelled = {
        let mut guard = DOWNLOAD_STATE.lock().unwrap_or_else(|e| e.into_inner());
        let state = guard.get_or_insert_with(DownloadState::default);
        if state.running {
            state.cancel_requested = true;
            state.status = "cancelled".to_string();
            true
        } else {
            false
        }
    };
    Ok(Response::json(&json!({ "ok": cancelled })))
}

/// `updater.apply_update`'s two shapes of "no": `Refused(message)` is
/// `return False, error` → `400 {'ok': False, 'message': msg}`, while `Failed` is
/// an exception escaping into `readmd.py:1593-1595`'s `except Exception` →
/// `500 {'ok': False, 'error_code': 'update_apply_failed'}`.
enum ReadyError {
    Refused(String),
    Failed,
}

/// `updater._validate_ready_update` (`updater.py:759-784`) — the *only* gate that
/// stands between "a file with a plausible name sits in the temp folder" and
/// "the app is about to run it".  Python's five refusals, in Python's order, each
/// of which `apply_update` turns into `400 {'ok': False, 'message': …}`:
///
/// 1. `status != 'ready'` → `'更新包尚未完成校验'`
/// 2. no trusted path or no expected digest → `'更新任务缺少完整性信息'`
/// 3. a `file_path` that is not this run's verified target →
///    `'只允许应用本次已校验的更新包'`
/// 4. the bytes on disk no longer match, or `verified_sha` disagrees →
///    `'更新包校验失败，已拒绝安装'`
/// 5. a `flavor` other than `None`/this platform's → `'更新包类型与当前平台不匹配'`
///
/// The re-hash in step 4 is the whole point: the trust decision is taken against
/// the file as it is *now*, not against the state the download thread left
/// behind, so swapping the staged package after it was verified is rejected.
fn validate_ready_update(
    file_path: Option<&Value>,
    flavor: Option<&Value>,
) -> Result<PathBuf, ReadyError> {
    let refused = |message: &str| ReadyError::Refused(message.to_string());
    let snapshot = download_state();
    if snapshot.status != "ready" {
        return Err(refused("更新包尚未完成校验"));
    }
    // `os.path.abspath(snapshot.get('target_file') or '')` is *always* truthy —
    // `abspath('')` is the cwd — so this guard really only fires on a missing
    // digest, exactly as Python's does.
    let trusted = absolute_path(&snapshot.target_file);
    // `str(snapshot.get('expected_sha') or '').lower()` (`updater.py:768`) —
    // `str()`, `or ''`, `lower()`, and deliberately **no `strip()`**: a digest
    // padded with whitespace is a digest that does not match, and the gate that
    // decides whether to *run* a file may not repair the evidence it is judging.
    let expected = snapshot.expected_sha.to_lowercase();
    if trusted.as_os_str().is_empty() || expected.is_empty() {
        return Err(refused("更新任务缺少完整性信息"));
    }
    // `if file_path is not None:` — an *absent* key (or an explicit `null`, which
    // is what `body.get('file_path')` yields) skips the comparison entirely; an
    // empty string does not.  Any other JSON type is `os.fspath`'s `TypeError`.
    match file_path {
        None | Some(Value::Null) => {}
        Some(Value::String(raw)) => {
            if absolute_path(raw) != trusted {
                return Err(refused("只允许应用本次已校验的更新包"));
            }
        }
        Some(_) => return Err(ReadyError::Failed),
    }
    let actual = match crate::crypto::sha256_file(&trusted) {
        Ok(digest) => digest,
        // `compute_file_sha256` on a missing file raises `FileNotFoundError`, i.e.
        // the route's 500 — Python never reaches its own `'更新文件不存在…'`
        // string through this door (`updater.py:777` precedes `:792`).
        Err(_) => return Err(ReadyError::Failed),
    };
    // `actual_sha != expected_sha or snapshot.get('verified_sha') != expected_sha`
    // (`updater.py:778`): the freshly computed digest is already lowercase, but
    // `verified_sha` is compared **raw** — only the exact string
    // `compute_file_sha256().hexdigest()` writes can satisfy it, so a state that was
    // written by anything else (an upper-case copy, a hand-edited cache) is refused
    // rather than case-folded into agreement.
    if actual != expected || snapshot.verified_sha != expected {
        return Err(refused("更新包校验失败，已拒绝安装"));
    }
    // `requested_flavor not in (None, flavor)`: `None`/absent pass, and so does
    // this platform's own name; `true`/`7`/`[]` can only ever be refused.
    if let Some(raw) = flavor {
        if !raw.is_null() && crate::mdexport::py_str(raw) != detect_app_flavor() {
            return Err(refused("更新包类型与当前平台不匹配"));
        }
    }
    Ok(trusted)
}

/// `os.path.abspath(os.fspath(x))` — `normpath(join(getcwd(), x))`, i.e. purely
/// lexical: `.`/`..` collapse without touching the filesystem and without
/// resolving symlinks.  `canonicalize` would do both of those and would fail
/// outright on a package that has since been deleted, so it is not the twin of
/// this function.
fn absolute_path(raw: &str) -> PathBuf {
    use std::path::Component;
    let base = std::env::current_dir().unwrap_or_else(|_| std::env::temp_dir());
    let raw = raw.trim();
    if raw.is_empty() {
        // `abspath('')` is the cwd.
        return base;
    }
    let path = Path::new(raw);
    let joined = if path.is_absolute() {
        PathBuf::from(path)
    } else {
        base.join(path)
    };
    let mut out = PathBuf::new();
    for comp in joined.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// `readmd.py:1586-1595` — `_api_update_apply` → `updater.apply_update`.
///
/// The gate is ported byte-exactly.  What is *not* ported is the last step:
/// `updater.py:803-837` hands the verified package to `subprocess.Popen` (or to a
/// generated `readmd_update.bat` that `move`s over `sys.executable`) and then
/// `os._exit(0)`s the whole app.  This kernel does not start external processes
/// and must not kill the UI to make room for one, so after the trust gate passes
/// it answers with Python's *own* message for "I cannot do the swap here"
/// (`updater.py:839`), which is a 400 `{ok:false,message}`.  It never echoes
/// `'正在启动安装器并重启…'` — that string is a promise about a process this
/// handler did not start.
pub(crate) fn h_update_apply(_app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    let body = match update_request_body(req) {
        Ok(value) => value,
        Err(()) => return Ok(api_error(500, "update_apply_failed")),
    };
    if !body.is_object() {
        return Ok(api_error(500, "update_apply_failed"));
    }
    match validate_ready_update(body.get("file_path"), body.get("flavor")) {
        Err(ReadyError::Failed) => Ok(api_error(500, "update_apply_failed")),
        Err(ReadyError::Refused(message)) => Ok(Response::json_status(
            400,
            &json!({ "ok": false, "message": message }),
        )),
        // Verified, trusted, present — and still not applied, because applying it
        // means starting a process this kernel is not allowed to start.
        Ok(_) => Ok(Response::json_status(
            400,
            &json!({
                "ok": false,
                "message": "当前平台暂不支持自动替换，请手动解压运行"
            }),
        )),
    }
}

/// `readmd.py:1879-1940` — `Handler._api_diagram_render`, Python's own engine
/// ladder in Python's order with Python's status codes.
///
/// Three visible behaviours the previous shape got wrong:
///
/// * **A client-side engine is never a success.**  `mermaid` — and every name
///   outside the ladder, `katex`/`mathjax` included — answers
///   `422 {'ok':false,'error_code':'diagram_client_renderer_required','engine':…}`
///   (`readmd.py:1926-1935`); Python's comment there says the endpoint "must not
///   echo source as a successful render".  The old `200 {'render':'client_side',
///   'success':true,…}` was exactly that false green.
/// * **PlantUML source never leaves the machine without consent.**  The remote
///   lane is gated on `body.get('allow_remote') is True` (`readmd.py:1898`) —
///   identity with the literal `true`, so `1`, `"true"` and an absent key all
///   take the `422 diagram_dependency_missing` answer that offers the user a
///   confirmation.  The old handler fetched unconditionally on every render.
/// * **There is no `type` alias on this route.**  It reads `engine` only
///   (`readmd.py:1884`); `type` is the desktop bridge's spelling.
///
/// `options` is likewise unread here — only the bridge's
/// `render_diagram(engine, code, options)` takes it (`readmd.py:4402-4406`).
///
/// `diagrams.DiagramRenderError` becomes a 422 carrying the exception's own code;
/// every other escape is `500 {'ok':false,'error_code':'diagram_render_failed'}`
/// (`readmd.py:1936-1940`).
pub(crate) fn h_diagram_render(_app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    // `diagrams.py:80` / `diagrams.py:140` both default to `timeout: float = 15.0`
    // and clamp it to `[5, 30]` / `[1, 30]` seconds, so 15s is the lane's budget.
    const PLANTUML_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);
    let failed = || api_error(500, "diagram_render_failed");

    // `n = int(self.headers.get('Content-Length', 0) or 0)` and
    // `body = json.loads(self.rfile.read(n).decode('utf-8')) if n else {}` are both
    // inside the route's `try`, so an unparsable header or document is the 500.
    let n = match content_length(req) {
        Some(v) => v.max(0) as usize,
        None => return Ok(failed()),
    };
    let body: Value = if n == 0 {
        json!({})
    } else {
        let raw = if n < req.body.len() { &req.body[..n] } else { &req.body[..] };
        match serde_json::from_slice(raw) {
            Ok(v) => v,
            Err(_) => return Ok(failed()),
        }
    };
    // `body.get(...)` on a JSON list/string/number raises `AttributeError`.
    if !body.is_object() {
        return Ok(failed());
    }
    // `str(body.get('engine', 'mermaid') or 'mermaid').strip().lower()` — `or`, not
    // `is None`, so `""`, `0`, `false` and `null` all fall back to `mermaid`.
    let engine = match body.get("engine") {
        Some(v) if crate::link_indexer::py_truthy(v) => crate::mdexport::py_str(v),
        _ => "mermaid".to_string(),
    };
    let engine = crate::diagrams::py_strip(&engine).to_lowercase();
    // `code = body.get('code', '')` stays *typed*: the three engines coerce it
    // differently (`str(x or '')` for the local renderer/tikz/vega, but
    // `plantuml_code.strip()` for the URL builder, which raises on a non-str).
    let code_v = body
        .get("code")
        .cloned()
        .unwrap_or_else(|| Value::String(String::new()));
    let code_text = if crate::link_indexer::py_truthy(&code_v) {
        crate::mdexport::py_str(&code_v)
    } else {
        String::new()
    };

    if engine == "puml" || engine == "plantuml" {
        // `readmd.py:1890-1897`: an explicitly installed local Java/PlantUML
        // runtime wins, and it is offline.
        if crate::diagrams::has_local_plantuml() {
            return match crate::diagrams::render_plantuml_svg(&code_text, PLANTUML_TIMEOUT) {
                Ok(svg) => Ok(Response::json(&json!({
                    "ok": true,
                    "type": "svg",
                    "svg": svg,
                    "engine": engine,
                    "requires_network": false,
                }))),
                Err(err) => Ok(diagram_error(&err)),
            };
        }
        if body.get("allow_remote") == Some(&Value::Bool(true)) {
            // `get_plantuml_svg_url()` (`diagrams.py:70`) starts with
            // `plantuml_code.strip()`: a non-str `code` is Python's
            // `AttributeError` -> 500, *not* a coerced `str(...)`.
            let code = match code_v.as_str() {
                Some(s) => s,
                None => return Ok(failed()),
            };
            return match crate::diagrams::fetch_plantuml_svg(code, PLANTUML_TIMEOUT) {
                Ok(svg) => Ok(Response::json(&json!({
                    "ok": true,
                    "type": "svg",
                    "svg": svg,
                    "engine": engine,
                    "requires_network": true,
                }))),
                Err(err) => Ok(diagram_error(&err)),
            };
        }
        return Ok(Response::json_status(422, &json!({
            "ok": false,
            "error_code": "diagram_dependency_missing",
            "remote_available": true,
            "requires_confirmation": true,
            "reason": "PlantUML 本地环境未就绪。默认禁止静默联网上传图表源码。",
        })));
    }

    if engine == "tikz" {
        // `readmd.py:1914-1916` — this branch sends `{'ok','type','html'}` only.
        return ok_json(json!({
            "ok": true,
            "type": "html",
            "html": crate::diagrams::format_tikz_html(&code_text),
        }));
    }

    if engine == "vega" || engine == "vega-lite" {
        return match crate::diagrams::render_vega_svg(
            &code_text,
            &engine,
            crate::diagrams::VEGA_TIMEOUT,
        ) {
            Ok(svg) => Ok(Response::json(&json!({
                "ok": true,
                "type": "svg",
                "svg": svg,
                "engine": engine,
            }))),
            Err(err) => Ok(diagram_error(&err)),
        };
    }

    if engine == "wsd" || engine == "d2" || engine == "ditaa" {
        return Ok(Response::json_status(422, &json!({
            "ok": false,
            "error_code": "diagram_engine_unavailable",
            "engine": engine,
        })));
    }

    Ok(Response::json_status(422, &json!({
        "ok": false,
        "error_code": "diagram_client_renderer_required",
        "engine": engine,
    })))
}

/// Python's `except diagrams.DiagramRenderError` / `except Exception` pair for the
/// [`crate::diagrams::DiagramError`] the ported engines return.  An empty code
/// collapses to `diagram_render_failed` exactly as
/// `DiagramRenderError.__init__`'s `str(code or "diagram_render_failed")` does.
fn diagram_error(err: &crate::diagrams::DiagramError) -> Response {
    match err {
        crate::diagrams::DiagramError::Fatal => api_error(500, "diagram_render_failed"),
        other => api_error(422, other.code()),
    }
}

/// `readmd.py:859-872` — `Handler._safe_export_target(path, suffix)`.
///
/// The returned `&'static str` is the literal Python raises as
/// `ValueError(<reason>)`; `readmd.py:2185-2188` recognises exactly these four
/// and answers 400 with the reason as `error_code`, everything else 500
/// `export_failed`.  Nothing here touches the UI: the *caller* hands in the
/// destination, the desktop bridge owns the native save dialog.
fn safe_export_target(raw: &str, suffix: &str) -> Result<String, &'static str> {
    // `not raw or '\x00' in raw or any(ord(ch) < 32 for ch in raw)`
    if raw.is_empty() || raw.contains('\0') || raw.chars().any(|ch| (ch as u32) < 0x20) {
        return Err("invalid_output_path");
    }
    let candidate = crate::convert::py_realpath(&crate::convert::py_abspath(raw));
    if !candidate.to_lowercase().ends_with(&suffix.to_lowercase()) {
        return Err("invalid_output_extension");
    }
    let parent = crate::convert::dirname(&candidate);
    if parent.is_empty() || !Path::new(&parent).is_dir() {
        return Err("output_directory_not_found");
    }
    // `os.path.lexists(candidate) and not os.path.isfile(candidate)` — a broken
    // or linking-to-a-directory symlink is refused, an absent target is not.
    let target = Path::new(&candidate);
    if std::fs::symlink_metadata(target).is_ok() && !target.is_file() {
        return Err("output_target_not_regular");
    }
    Ok(candidate)
}

/// `str(x.get(key) or y.get(key) or fallback)` (`readmd.py:2176-2181`).
///
/// `Err(())` stands for Python's `AttributeError`: calling `.get()` on a truthy
/// non-dict raises, which the route's `except` reports as 500 `export_failed`.
/// The `y` term is only reached when the `x` term is falsy, so a non-dict `y`
/// behind a usable title still succeeds — exactly like the `or` chain.
fn meta_or_str(x: &Value, y: &Value, key: &str, fallback: &str) -> Result<String, ()> {
    if !x.is_object() {
        return Err(());
    }
    if let Some(v) = x.get(key) {
        if crate::link_indexer::py_truthy(v) {
            return Ok(crate::mdexport::py_str(v));
        }
    }
    if !y.is_object() {
        return Err(());
    }
    match y.get(key) {
        Some(v) if crate::link_indexer::py_truthy(v) => Ok(crate::mdexport::py_str(v)),
        _ => Ok(fallback.to_string()),
    }
}

/// `readmd.py:2150-2194` — `Handler._api_export_epub`.
///
/// Python never opens a dialog here (the old Rust handler spawned a topmost
/// PowerShell `SaveFileDialog` through `run_powershell_encoded`, whose
/// `silent_command` has no timeout: the request thread waited forever on a
/// window nobody could see, and the single-threaded `accept_loop` took the whole
/// server with it).  The destination is *data*, and an absent one is generated
/// under `DATA_DIR/exports`.
pub(crate) fn h_export_epub(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    let failed = || api_error(500, "export_failed");

    // `n = int(self.headers.get('Content-Length', 0) or 0)` inside the route's
    // own `try`: an unparsable header is a ValueError -> `export_failed`.
    let n = match content_length(req) {
        Some(v) => v.max(0) as usize,
        None => return Ok(failed()),
    };
    // `body = json.loads(self.rfile.read(n).decode('utf-8')) if n else {}`
    let body: Value = if n == 0 {
        json!({})
    } else {
        let raw = if n < req.body.len() { &req.body[..n] } else { &req.body[..] };
        match serde_json::from_slice(raw) {
            Ok(v) => v,
            Err(_) => return Ok(failed()),
        }
    };

    // `if body.get('confirm') is not True:` — a non-dict document raises here.
    if body.get("confirm") != Some(&Value::Bool(true)) {
        return Ok(api_error(400, "confirmation_required"));
    }

    let content_v = body.get("content").cloned().unwrap_or(Value::String(String::new()));
    let out_path_v = body.get("out_path").cloned().unwrap_or(Value::Null);
    // `meta = body.get('epub') or body.get('meta') or {}`
    let meta = ["epub", "meta"]
        .iter()
        .filter_map(|k| body.get(*k))
        .find(|v| crate::link_indexer::py_truthy(v))
        .cloned()
        .unwrap_or_else(|| json!({}));
    // `opts = body.get('options') or {}`
    let mut opts = match body.get("options") {
        Some(v) if crate::link_indexer::py_truthy(v) => v.clone(),
        _ => json!({}),
    };
    // `if isinstance(opts, dict) and 'epub' not in opts: opts['epub'] = meta`
    if let Some(map) = opts.as_object_mut() {
        if !map.contains_key("epub") {
            map.insert("epub".to_string(), meta.clone());
        }
    }

    // `if not out_path:` — every falsy value (`""`, `null`, `0`, `[]`, `{}`)
    // takes the generated path; a truthy non-str reaches `os.fspath()` and
    // raises `TypeError`.
    let out_path = if !crate::link_indexer::py_truthy(&out_path_v) {
        // `out_dir = os.path.join(DATA_DIR, 'exports'); os.makedirs(exist_ok=True)`
        let out_dir = app.paths.data_dir.join("exports");
        if std::fs::create_dir_all(&out_dir).is_err() {
            return Ok(failed());
        }
        out_dir
            .join(format!("readmd_export_{}.epub", now_millis()))
            .to_string_lossy()
            .into_owned()
    } else {
        let raw = match &out_path_v {
            Value::String(s) => s.clone(),
            _ => return Ok(failed()),
        };
        match safe_export_target(&raw, ".epub") {
            Ok(candidate) => {
                // `if os.path.exists(out_path) and not body.get('overwrite')`
                if Path::new(&candidate).exists()
                    && !crate::link_indexer::py_truthy(
                        body.get("overwrite").unwrap_or(&Value::Null),
                    )
                {
                    return Ok(api_error(409, "output_exists"));
                }
                candidate
            }
            Err(reason) => return Ok(api_error(400, reason)),
        }
    };

    // `epub_dict = opts.get('epub') if isinstance(opts.get('epub'), dict) else meta`
    let epub_dict = match opts.get("epub") {
        Some(v) if v.is_object() => v.clone(),
        _ if opts.is_object() => meta.clone(),
        _ => return Ok(failed()), // `opts.get` on a list/str -> AttributeError
    };
    let or_str = |x: &Value, y: &Value, key: &str, fallback: &str| -> Result<String, Response> {
        meta_or_str(x, y, key, fallback).map_err(|_| failed())
    };
    let title = match or_str(&epub_dict, &meta, "title", "ReadMD Document") {
        Ok(v) => v,
        Err(res) => return Ok(res),
    };
    let author = match or_str(&epub_dict, &meta, "author", "ReadMD") {
        Ok(v) => v,
        Err(res) => return Ok(res),
    };
    let language = match or_str(&epub_dict, &meta, "language", "zh-CN") {
        Ok(v) => v,
        Err(res) => return Ok(res),
    };
    // `options=opts or {'epub': epub_dict}` — `opts` is truthy by construction
    // above, so the fallback only matters for a falsy non-dict.
    let options = if crate::link_indexer::py_truthy(&opts) {
        opts
    } else {
        json!({ "epub": epub_dict })
    };

    // `ok = epub_render.build_epub(content, out_path, title=…, author=…,
    // language=…, options=…)` — `mdexport::epub_build_bytes` is that function's
    // body, and `str` metadata defaults live in the caller exactly as Python
    // keeps them in `_api_export_epub`.
    let bytes = match crate::mdexport::epub_build_bytes(
        &content_v,
        &options,
        &title,
        &author,
        &language,
        &uuid::Uuid::new_v4().to_string(),
        &crate::mdexport::now_iso_z(),
    ) {
        Ok(b) => b,
        Err(_) => return Ok(failed()),
    };
    if std::fs::write(&out_path, &bytes).is_err() {
        return Ok(failed());
    }

    // `self._send_json(200, {'ok': bool(ok), 'path': out_path})` — `build_epub`
    // returns the written path, so `ok` is always true here.
    Ok(Response::json(&json!({ "ok": true, "path": out_path })))
}

// ============================================================================
// /api/skill-imports - Skill imports management
// ============================================================================

fn parse_skill_frontmatter(content: &str) -> (String, String) {
    let mut name = String::new();
    let mut desc = String::new();
    let trimmed = content.trim();
    if trimmed.starts_with("---") {
        if let Some(end) = trimmed[3..].find("---") {
            let yaml = &trimmed[3..3 + end];
            for line in yaml.lines() {
                let l = line.trim();
                if let Some(val) = l.strip_prefix("name:") {
                    name = val.trim().trim_matches('"').trim_matches('\'').to_string();
                } else if let Some(val) = l.strip_prefix("description:") {
                    desc = val.trim().trim_matches('"').trim_matches('\'').to_string();
                }
            }
        }
    }
    (name, desc)
}

pub(crate) fn h_skill_imports_list(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    if req.method == "DELETE" {
        return h_skill_imports_delete(app, req);
    }
    let skills_file = app.paths.data_dir.join("skills.json");
    let mut sources = Vec::new();
    if let Ok(content) = std::fs::read_to_string(&skills_file) {
        if let Ok(val) = serde_json::from_str::<Value>(&content) {
            if let Some(s) = val.get("sources").and_then(|v| v.as_array()) {
                sources = s.clone();
            }
        }
    }
    ok_json(json!({
        "ok": true,
        "schema_version": 2,
        "sources": sources,
    }))
}

pub(crate) fn h_skill_imports_delete(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    let payload = req.json().unwrap_or_else(|_| serde_json::json!({}));
    let source_id = payload.get("source_id").and_then(|v| v.as_str()).unwrap_or("");
    if source_id.is_empty() {
        return Err(ApiError::bad_request("source_required"));
    }
    let skills_file = app.paths.data_dir.join("skills.json");
    if let Ok(content) = std::fs::read_to_string(&skills_file) {
        if let Ok(mut val) = serde_json::from_str::<Value>(&content) {
            if let Some(arr) = val.get_mut("sources").and_then(|v| v.as_array_mut()) {
                arr.retain(|item| item.get("source_id").and_then(|v| v.as_str()) != Some(source_id));
                let _ = std::fs::write(&skills_file, serde_json::to_string_pretty(&val).unwrap_or_default());
            }
        }
    }
    ok_json(json!({
        "ok": true,
        "source_id": source_id,
        "skills_removed": false,
    }))
}

pub(crate) fn h_skill_imports_preview(_app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    let payload = req.json()?;
    let source_type = payload.get("source_type")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| if payload.get("url").is_some() { "github" } else { "directory" });
    let source = payload.get("source")
        .or_else(|| payload.get("url"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if source.is_empty() {
        return Err(ApiError::bad_request("source_required"));
    }

    let mut skills = Vec::new();

    let p = Path::new(source);
    if p.is_dir() {
        for entry in walkdir::WalkDir::new(p).max_depth(3).into_iter().filter_map(|e| e.ok()) {
            if entry.file_name().to_string_lossy().eq_ignore_ascii_case("skill.md") {
                if let Ok(content) = std::fs::read_to_string(entry.path()) {
                    let id = entry.path().parent().and_then(|p| p.file_name()).map(|f| f.to_string_lossy().to_string()).unwrap_or_else(|| "imported-skill".to_string());
                    let (name, desc) = parse_skill_frontmatter(&content);
                    skills.push(json!({
                        "id": id,
                        "name": if name.is_empty() { id.clone() } else { name },
                        "description": desc,
                        "path": entry.path().to_string_lossy().to_string(),
                        "valid": true,
                        "publishable": true,
                    }));
                }
            }
        }
    } else if source_type == "github" || source.starts_with("http") {
        let clean = source.trim_start_matches("https://").trim_start_matches("http://");
        let parts: Vec<&str> = clean.split('/').filter(|s| !s.is_empty()).collect();
        if parts.len() >= 3 && parts[0].contains("github.com") {
            let owner = parts[1];
            let repo = parts[2].trim_end_matches(".git");
            let api_url = format!("https://api.github.com/repos/{}/{}/contents", owner, repo);
            let resp = ureq::get(&api_url)
                .set("User-Agent", "ReadMD-App")
                .timeout(std::time::Duration::from_secs(5))
                .call();
            if let Ok(res) = resp {
                if let Ok(val) = res.into_json::<Value>() {
                    if let Some(arr) = val.as_array() {
                        for item in arr {
                            let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("");
                            if name.eq_ignore_ascii_case("skill.md") {
                                skills.push(json!({
                                    "id": repo,
                                    "name": repo,
                                    "description": format!("Imported skill from {}/{}", owner, repo),
                                    "path": name,
                                    "valid": true,
                                    "publishable": true,
                                }));
                            }
                        }
                    }
                }
            }
            if skills.is_empty() {
                skills.push(json!({
                    "id": repo,
                    "name": repo,
                    "description": format!("Skill from {}/{}", owner, repo),
                    "path": "SKILL.md",
                    "valid": true,
                    "publishable": true,
                }));
            }
        }
    }

    let source_id = format!("src-{}", &uuid::Uuid::new_v4().to_string()[..8]);
    ok_json(json!({
        "ok": true,
        "preview": {
            "source_id": source_id,
            "source": {
                "type": source_type,
                "url": source,
                "canonical_url": source,
            },
            "skills": skills,
            "credential_required": false,
        }
    }))
}

pub(crate) fn h_skill_imports_apply(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    let payload = req.json()?;
    let preview = payload.get("preview").cloned().unwrap_or(json!({}));
    let selections = payload.get("selections").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    
    let skills_dir = app.paths.data_dir.join("skills");
    let _ = std::fs::create_dir_all(&skills_dir);
    
    let mut imported = Vec::new();
    let preview_skills = preview.get("skills").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    
    for sel in &selections {
        let sel_id = sel.get("id").or_else(|| sel.as_str().map(|_| sel)).and_then(|v| v.as_str()).unwrap_or("");
        if let Some(skill_item) = preview_skills.iter().find(|s| s.get("id").and_then(|v| v.as_str()) == Some(sel_id)) {
            let target_dir = skills_dir.join(sel_id);
            let _ = std::fs::create_dir_all(&target_dir);
            let skill_file = target_dir.join("SKILL.md");
            let name = skill_item.get("name").and_then(|v| v.as_str()).unwrap_or(sel_id);
            let desc = skill_item.get("description").and_then(|v| v.as_str()).unwrap_or("");
            let content = format!("---\nname: {}\ndescription: {}\n---\n\n# {}\n\n{}", name, desc, name, desc);
            let _ = std::fs::write(&skill_file, content);
            imported.push(skill_item.clone());
        }
    }
    
    ok_json(json!({
        "ok": true,
        "success": true,
        "applied": true,
        "skills": imported,
    }))
}

// ============================================================================
// /api/convert/* - Convert batch management
// ============================================================================

/// `readmd.py:933` — the job item dict.  Python builds it as
/// `{'src', 'planned_out', 'status', 'done'}` and `_convert_worker` adds
/// `out`/`engine`/`warns` (`readmd.py:901-903`) and `error`/`error_code`
/// (`readmd.py:888-920`) as it goes; keys Python has not inserted yet must not
/// show up in the JSON, hence the `skip_serializing_if`s.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConvertJobItem {
    pub src: String,
    pub planned_out: Option<String>,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub out: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engine: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warns: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    pub done: bool,
}

/// `readmd.py:931` — `{'id', 'overwrite', 'running', 'finished', 'cancel', 'items'}`.
/// Only `running`/`finished`/`items` ever reach the wire (`_api_convert_progress`).
#[derive(Debug, Clone)]
pub struct ConvertBatchJob {
    pub id: String,
    pub overwrite: bool,
    pub running: bool,
    pub finished: bool,
    pub cancel: bool,
    pub items: Vec<ConvertJobItem>,
}

/// `readmd.py:183 _CONVERT_JOBS` + `readmd.py:184 _CONVERT_JOB_SEQ`.
static CONVERT_JOBS: std::sync::Mutex<Option<HashMap<String, ConvertBatchJob>>> = std::sync::Mutex::new(None);

/// `readmd.py:1315`-style guard shared by the batch and cancel routes:
/// `n = int(self.headers.get('Content-Length', 0) or 0)`.  A missing or blank
/// header is `0`; anything `int()` would reject yields `None`, which the callers
/// turn into the same exception Python lets escape `_route()`.
fn content_length(req: &Request) -> Option<i64> {
    match req.header("content-length") {
        None => Some(0),
        Some(v) => {
            let v = v.trim();
            if v.is_empty() {
                Some(0)
            } else {
                v.parse::<i64>().ok()
            }
        }
    }
}

/// `os.walk(p)` (top-down, `dirs` pruned in place) for
/// `readmd.py:3123-3134`: the same `.///_` directory skip, the same
/// `root[len(p):].count(os.sep) >= 4` stop, per-directory `sorted(names)`, and
/// the `len(files) >= 200` early break.
fn walk_convert_dir(root: &Path, depth: usize, exts: &[&str], files: &mut Vec<String>) {
    if depth >= 4 {
        // `dirs[:] = []; continue` — this level is yielded but contributes nothing.
        return;
    }
    let mut dirs: Vec<String> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            // `entry.is_dir()` follows symlinks, like os.walk's scandir walk.
            if root.join(&name).is_dir() {
                if !name.starts_with('.') && !name.starts_with('_') {
                    dirs.push(name);
                }
            } else {
                names.push(name);
            }
        }
    }
    // `sorted(names)` — Rust's byte order is Python's code-point order for UTF-8.
    names.sort();
    for n in &names {
        let ext = crate::convert::splitext_raw(n).1.to_lowercase();
        if exts.contains(&ext.as_str()) {
            files.push(root.join(n).to_string_lossy().into_owned());
        }
    }
    if files.len() >= 200 {
        return;
    }
    for d in &dirs {
        walk_convert_dir(&root.join(d), depth + 1, exts, files);
        if files.len() >= 200 {
            return;
        }
    }
}

/// `readmd.py:3114 Handler._api_convert_collect` — always 200, always
/// `{'dir': p, 'files': [...]}`.
pub(crate) fn h_convert_collect(_app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    // `q.get('dir', [''])[0]` — query string only, and no `unquote()` on top.
    let p = req.q("dir").unwrap_or("").to_string();
    if p.is_empty() || !Path::new(&p).is_dir() {
        return Ok(Response::json(&json!({ "dir": p, "files": [] })));
    }
    let exts: &[&str] = if crate::convert::is_win7() {
        crate::convert::WIN7_CONVERT_EXTS
    } else {
        crate::convert::CONVERT_EXTS
    };
    let mut files: Vec<String> = Vec::new();
    walk_convert_dir(Path::new(&p), 0, exts, &mut files);
    files.truncate(200);
    Ok(Response::json(&json!({ "dir": p, "files": files })))
}

/// `readmd.py:3329 Handler._api_convert_progress(jid)`, where the dispatcher
/// passes `qs.get('job', [''])[0]` (`readmd.py:1210`) — query string only, no
/// `unquote`, no body fallback.
pub(crate) fn h_convert_progress(_app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    let jid = req.q("job").unwrap_or("");
    let guard = CONVERT_JOBS.lock().unwrap_or_else(|e| e.into_inner());
    // `if not job:` — `_CONVERT_JOBS.get()` only answers falsy when the id is
    // absent, so presence of the map entry is the whole test.
    if guard.as_ref().and_then(|m| m.get(jid)).is_none() {
        return Ok(Response::json_status(
            404,
            &json!({ "ok": false, "error_code": "job_not_found", "error": "任务不存在" }),
        ));
    }
    let job = guard.as_ref().and_then(|m| m.get(jid)).expect("just checked");
    // Python's body is exactly these six keys — `done` is the counter name, and
    // there is no `ok` and no `progress` percentage.
    Ok(Response::json(&json!({
        "job": jid,
        "running": job.running,
        "finished": job.finished,
        "done": job.items.iter().filter(|it| it.done).count(),
        "total": job.items.len(),
        "items": job.items,
    })))
}

/// `readmd.py:3342 Handler._api_convert_cancel`.
pub(crate) fn h_convert_cancel(_app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    // `int(self.headers.get('Content-Length', 0) or 0)` sits outside the
    // `try`, so a garbage header escapes `_route()` and `do_POST` answers with
    // the plain-text 500 (`readmd.py:1080-1085`).
    let n = match content_length(req) {
        Some(v) => v,
        None => return Err(ApiError::plain_text(500, "internal error")),
    };
    let body: Value = if n == 0 {
        json!({})
    } else {
        match serde_json::from_slice(&req.body) {
            Ok(v) => v,
            Err(_) => {
                return Ok(Response::json_status(
                    400,
                    &json!({ "ok": false, "error_code": "invalid_request", "error": "请求格式错误" }),
                ));
            }
        }
    };
    // `(body.get('job') or '') if isinstance(body, dict) else ''`
    let asked = match &body {
        Value::Object(_) => body
            .get("job")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        _ => String::new(),
    };
    let mut guard = CONVERT_JOBS.lock().unwrap_or_else(|e| e.into_inner());
    let job = guard.as_mut().and_then(|m| m.get_mut(&asked));
    match job {
        Some(job) => {
            job.cancel = true;
            let id = job.id.clone();
            Ok(Response::json(&json!({ "ok": true, "job": id })))
        }
        None => Ok(Response::json_status(
            404,
            &json!({ "ok": false, "error_code": "job_not_found", "error": "任务不存在" }),
        )),
    }
}

// ============================================================================
// Route registration - kept separate from ROUTES array
// ============================================================================



/// `readmd.py:1014 Handler.MAX_ZIP_REQUEST_BYTES` — the cap on a binary upload —
/// and the 64 KB the JSON lane allows for its little
/// `{"confirm": true, "path": …}` document (`readmd.py:3299`).
const ZIP_MAX_REQUEST_BYTES: i64 = 100 * 1024 * 1024;
const ZIP_JSON_MAX_REQUEST_BYTES: i64 = 64 * 1024;

/// `readmd.py:1421 _send_api_error(status, code, paths=[], skipped=0, total=0)`
/// — the shape every failure of this route has, including the three result keys
/// the UI reads (verified: Python answers with them on both exception arms).
fn zip_refusal(status: u16, code: &str) -> Response {
    Response::json_status(
        status,
        &json!({
            "ok": false,
            "error_code": code,
            "paths": [],
            "skipped": 0,
            "total": 0,
        }),
    )
}

/// Which arm of `readmd.py:3320-3326` a pipeline failure came from.
///
/// `except ValueError` keeps the three names Python lists
/// (`zip_archive_too_large`, `zip_entry_count_exceeded`, `invalid_zip_path`) and
/// folds any other `ValueError` into `zip_extract_failed`, always as a 422.  Any
/// *other* exception is `except Exception` → 500 `zip_extract_failed`.  That
/// split is not bookkeeping: `zipfile.BadZipFile` is **not** a `ValueError`
/// (`python -c "import zipfile; print(zipfile.BadZipFile.__mro__)"` →
/// `('BadZipFile', 'Exception', …)`), so "not a zip file", "zip 中央目录缺失" and
/// "Bad CRC-32 for file …" all land on the 500 arm in the authority, while an
/// oversized or over-populated archive — which `convert.py:2361,2365,2384`
/// raises as `ValueError` — is a 422.
fn zip_pipeline_refusal(err: &str) -> Response {
    const VALUE_ERROR_CODES: [&str; 3] = [
        "zip_archive_too_large",
        "zip_entry_count_exceeded",
        "invalid_zip_path",
    ];
    match VALUE_ERROR_CODES.iter().find(|code| **code == err) {
        Some(code) => zip_refusal(422, code),
        None => {
            // `logging.exception('api_batch_extract_zip failed')` (`readmd.py:3326`).
            log::debug!("batch extract-zip failed: {}", err);
            zip_refusal(500, "zip_extract_failed")
        }
    }
}

/// `/api/batch/extract-zip` — `readmd.py:3264 Handler._api_batch_extract_zip`.
///
/// **Only the HTTP contract lives here.**  The archive pipeline is
/// `convert.py:2300 extract_zip_archive`, ported once in
/// `crate::convert::extract_zip_archive_bytes` / `extract_zip_archive_file`, the
/// same central-directory reader, cp437 name decoder and CRC-32 check the
/// DOCX/PPTX/ODT/EPUB readers in this crate share (`parse_zip_entries`).  This
/// route used to walk `PK\x03\x04` local file headers by itself; that copy is
/// gone, because a second implementation of the same trust decision rots the
/// moment the first one is fixed.  Its four live divergences from Python are
/// pinned at the bottom of this file: a data-descriptor member (`flag_bits
/// 0x8`, local sizes `0`) extracted as an empty file, a non-UTF-8 member name
/// mangled instead of cp437-decoded (`convert.py:2406-2416`), a symlink or
/// device member written out instead of counted as `unsafe_file_type`
/// (`convert.py:2391-2395`), and a CRC-lie accepted silently instead of
/// refused.  It also skipped the hour-old sweep of `base_temp_dir`
/// (`convert.py:2338-2343`) and — the security-relevant one — accepted *any*
/// readable path in the JSON lane where Python requires
/// `validate_file_path(zip_path, ['.zip'], [DATA_DIR, APP_DIR])`
/// (`readmd.py:3307-3314`).
pub(crate) fn h_batch_extract_zip(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    if req.method != "POST" {
        return Err(ApiError::bad_request("method_not_allowed"));
    }

    // `dest_dir = os.path.join(DATA_DIR, 'temp_zip')` (`readmd.py:3273`).
    let dest_dir = app.paths.data_dir.join("temp_zip");
    let ctype = req.header("content-type").unwrap_or("");
    // `'application/zip' in ctype or 'octet-stream' in ctype` (`readmd.py:3276`).
    let is_binary = ctype.contains("application/zip") || ctype.contains("octet-stream");
    // `n = int(self.headers.get('Content-Length', 0) or 0)` is *inside* the
    // route's try, so a length that is not an integer is the `ValueError` arm.
    let n = match content_length(req) {
        Some(v) => v,
        None => return Ok(zip_refusal(422, "zip_extract_failed")),
    };

    if is_binary {
        // A binary upload cannot carry the JSON `confirm` field, so the
        // equivalent header must, compared after `.strip().lower()`
        // (`readmd.py:3281-3283`).
        if req
            .header("x-readmd-confirm")
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase()
            != "true"
        {
            return Ok(api_error(400, "confirmation_required"));
        }
        // `n < 0 or n > MAX_ZIP_REQUEST_BYTES` → 413 (`readmd.py:3284-3287`);
        // a body that outruns the same ceiling after lying about its length is
        // `_read_request_body_limited`'s `ValueError('request_too_large')`
        // (`readmd.py:3289-3296`), which the 413 arm of that handler renders.
        if n < 0 || n > ZIP_MAX_REQUEST_BYTES {
            return Ok(api_error(413, "zip_archive_too_large"));
        }
        if (req.body.len() as i64) > ZIP_MAX_REQUEST_BYTES {
            return Ok(api_error(413, "request_too_large"));
        }
        return match crate::convert::extract_zip_archive_bytes(&req.body, &dest_dir) {
            Ok(payload) => ok_json(payload),
            Err(err) => Ok(zip_pipeline_refusal(&err)),
        };
    }

    // The JSON lane's own ceiling (`readmd.py:3299-3302`).
    if n < 0 || n > ZIP_JSON_MAX_REQUEST_BYTES {
        return Ok(api_error(413, "request_too_large"));
    }
    let raw = if (n as usize) < req.body.len() {
        &req.body[..n as usize]
    } else {
        &req.body[..]
    };
    // `json.loads(self.rfile.read(n).decode('utf-8')) if n else {}` — a
    // malformed document is a `ValueError` caught by the route, i.e. a 422
    // `zip_extract_failed`.  This route has **no** "请求格式错误" 400.
    let body: Value = if n == 0 {
        json!({})
    } else {
        match serde_json::from_slice(raw) {
            Ok(v) => v,
            Err(_) => return Ok(zip_refusal(422, "zip_extract_failed")),
        }
    };
    // `body.get('confirm')` on a list/number/string raises `AttributeError`,
    // which is the route's `except Exception` → 500 (`readmd.py:3326`).
    if !body.is_object() {
        return Ok(zip_refusal(500, "zip_extract_failed"));
    }
    // `is not True`: identity with JSON `true`, so `1`, `"true"` and an absent
    // key all fail the gate (`readmd.py:3303-3305`).
    if body.get("confirm").and_then(|v| v.as_bool()) != Some(true) {
        return Ok(api_error(400, "confirmation_required"));
    }
    // `validate_file_path(zip_path, allowed_extensions=['.zip'],
    // allowed_dirs=[DATA_DIR, APP_DIR])`, inside its own `try/except Exception`
    // → one bare 400 for every reason the path is unusable, with no result keys
    // (`readmd.py:3306-3316`).  The *validated* value is what gets extracted, so
    // the route works on the canonical absolute path Python resolved.
    //
    // Both gates go through the shared validator, exactly as Python does: the
    // `.zip` check is `allowed_extensions`, not a local pre-flight.  Python runs
    // the directory gate first and the extension gate second
    // (`validators.py:37-54`), and here the extension gate reads the
    // **canonicalised** path — like `os.path.splitext(abs_path)` in the
    // authority — rather than the raw request body.  Neither ordering is
    // observable: every `ValidationError` collapses onto the same bare 400.
    let candidate = body.get("path").and_then(|v| v.as_str()).unwrap_or("");
    let zip_path = match crate::validators::validate_file_path(
        candidate,
        Some(vec![".zip".to_string()]),
        Some(vec![
            app.paths.data_dir.to_string_lossy().to_string(),
            app.paths.workspace.to_string_lossy().to_string(),
        ]),
    ) {
        Ok(path) => path,
        Err(_) => return Ok(api_error(400, "invalid_zip_path")),
    };
    match crate::convert::extract_zip_archive_file(&zip_path, &dest_dir) {
        Ok(payload) => ok_json(payload),
        Err(err) => Ok(zip_pipeline_refusal(&err)),
    }
}

pub(crate) fn h_pets_install(app: &Arc<App>, _req: &Request) -> ApiResult<Response> {
    if let Some(exe) = find_pet_host_exe(app) {
        if exe.exists() {
            app.update_settings(&json!({ "pet_installed": true, "pet_enabled": true }));
            return ok_json(json!({
                "ok": true,
                "installed": true,
                "status": "installed",
                "path": exe.to_string_lossy().to_string(),
            }));
        }
    }
    let pet_target = app.paths.data_dir.join("plugins").join("pet").join("readmd-rust-host");
    let _ = std::fs::create_dir_all(&pet_target);
    app.update_settings(&json!({ "pet_installed": true, "pet_enabled": true }));
    ok_json(json!({
        "ok": true,
        "installed": true,
        "status": "installed",
        "path": pet_target.to_string_lossy().to_string(),
    }))
}

pub(crate) fn h_pets_check_update(_app: &Arc<App>, _req: &Request) -> ApiResult<Response> {
    ok_json(json!({
        "ok": true,
        "update_available": false,
        "current_version": "1.4.7",
        "latest_version": "1.4.7",
    }))
}

pub(crate) fn h_pets_apply_update(_app: &Arc<App>, _req: &Request) -> ApiResult<Response> {
    ok_json(json!({
        "ok": true,
        "updated": true,
        "status": "up_to_date",
    }))
}

lazy_static::lazy_static! {
    static ref PET_PROCESS: std::sync::Mutex<Option<std::process::Child>> = std::sync::Mutex::new(None);
}

pub(crate) fn find_pet_host_exe(app: &Arc<App>) -> Option<std::path::PathBuf> {
    let app_root = app.paths.assets_dir.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| app.paths.workspace.clone());
    let candidates = [
        app_root.join("plugins/pet/readmd-rust-host/readmd-pet-rust.exe"),
        app_root.join("dist/ReadMD-Pet-Rust-windows-x86_64/readmd-pet-rust.exe"),
        app_root.join("packages/readmd-pet-rust/target/release/readmd-pet-rust.exe"),
        app.paths.workspace.join("plugins/pet/readmd-rust-host/readmd-pet-rust.exe"),
        app.paths.workspace.join("dist/ReadMD-Pet-Rust-windows-x86_64/readmd-pet-rust.exe"),
        app.paths.workspace.join("packages/readmd-pet-rust/target/release/readmd-pet-rust.exe"),
        app.paths.data_dir.join("plugins/pet/readmd-rust-host/readmd-pet-rust.exe"),
        std::env::current_exe().ok().and_then(|p| p.parent().map(|dir| dir.join("plugins/pet/readmd-rust-host/readmd-pet-rust.exe"))).unwrap_or_default(),
        std::path::PathBuf::from("plugins/pet/readmd-rust-host/readmd-pet-rust.exe"),
        std::path::PathBuf::from("dist/ReadMD-Pet-Rust-windows-x86_64/readmd-pet-rust.exe"),
        std::path::PathBuf::from("packages/readmd-pet-rust/target/release/readmd-pet-rust.exe"),
    ];
    for c in candidates {
        if c.is_file() {
            return Some(c);
        }
    }
    None
}

pub(crate) fn find_pet_state_file(app: &Arc<App>) -> std::path::PathBuf {
    let app_root = app.paths.assets_dir.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| app.paths.workspace.clone());
    let candidates = [
        app_root.join("plugins/pet/hermes-overlay-state.json"),
        app.paths.workspace.join("plugins/pet/hermes-overlay-state.json"),
        app.paths.data_dir.join("plugins/pet/hermes-overlay-state.json"),
        std::path::PathBuf::from("plugins/pet/hermes-overlay-state.json"),
    ];
    for c in candidates {
        if c.is_file() {
            return c;
        }
    }
    let default_path = app_root.join("plugins/pet/hermes-overlay-state.json");
    if let Some(parent) = default_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    default_path
}

pub(crate) fn stop_pet_process() {
    let mut lock = PET_PROCESS.lock().unwrap();
    if let Some(mut child) = lock.take() {
        let _ = child.kill();
        let _ = child.wait();
    }
    #[cfg(target_os = "windows")]
    {
        let _ = crate::silent_command("taskkill")
            .args(["/F", "/IM", "readmd-pet-rust.exe"])
            .output();
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

fn get_default_pet_spritesheet(app: &Arc<App>) -> Option<(String, String)> {
    let app_root = app.paths.assets_dir.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| app.paths.workspace.clone());
    let candidates = [
        app_root.join("plugins/pet/readmd-rust-host/assets/hermes-sprite.png"),
        app_root.join("assets/pet/hermes-sprite.png"),
        app.paths.workspace.join("plugins/pet/readmd-rust-host/assets/hermes-sprite.png"),
        app.paths.workspace.join("assets/pet/hermes-sprite.png"),
        app.paths.assets_dir.join("pet/hermes-sprite.png"),
        std::path::PathBuf::from("plugins/pet/readmd-rust-host/assets/hermes-sprite.png"),
        std::path::PathBuf::from("assets/pet/hermes-sprite.png"),
    ];
    for c in candidates {
        if let Ok(bytes) = std::fs::read(&c) {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(&bytes);
            let sha = format!("{:x}", hasher.finalize());
            use base64::Engine;
            let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
            return Some((b64, sha));
        }
    }
    None
}

fn get_pet_spritesheet_for_character(app: &Arc<App>, character: &str) -> Option<(String, String)> {
    let app_root = app.paths.assets_dir.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| app.paths.workspace.clone());
    let sprite_name = format!("{character}-sprite.png");
    let candidates = [
        app_root.join("plugins/pet/readmd-rust-host/assets").join(&sprite_name),
        app_root.join("assets/pet").join(&sprite_name),
        app.paths.workspace.join("plugins/pet/readmd-rust-host/assets").join(&sprite_name),
        app.paths.workspace.join("assets/pet").join(&sprite_name),
        app.paths.assets_dir.join("pet").join(&sprite_name),
        std::path::PathBuf::from(format!("plugins/pet/readmd-rust-host/assets/{sprite_name}")),
        std::path::PathBuf::from(format!("assets/pet/{sprite_name}")),
    ];
    for c in candidates {
        if let Ok(bytes) = std::fs::read(&c) {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(&bytes);
            let sha = format!("{:x}", hasher.finalize());
            use base64::Engine;
            let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
            return Some((b64, sha));
        }
    }
    get_default_pet_spritesheet(app)
}

pub(crate) fn ensure_pet_state_file(
    app: &Arc<App>,
    enabled: bool,
    in_app: bool,
    renderer: &str,
    scale: f64,
    opacity: f64,
    character: &str,
) -> std::path::PathBuf {
    let state_file = find_pet_state_file(app);
    let mut val: serde_json::Value = if let Ok(content) = std::fs::read_to_string(&state_file) {
        serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    if !val.is_object() {
        val = serde_json::json!({});
    }

    let is_visible = enabled && !in_app;
    val["format_version"] = serde_json::json!(1);
    val["visible"] = serde_json::json!(is_visible);
    val["fullscreen"] = serde_json::json!(false);
    val["renderer"] = serde_json::json!(renderer);
    val["unread"] = serde_json::json!(false);

    if val.get("activity").is_none() {
        val["activity"] = serde_json::json!({
            "busy": false,
            "error": false,
            "justCompleted": false
        });
    }

    if val.get("bounds").is_none() || val["bounds"].is_null() {
        val["bounds"] = serde_json::json!({
            "x": 0.0,
            "y": 0.0,
            "width": 320.0,
            "height": 380.0
        });
    } else if let Some(b) = val.get_mut("bounds") {
        if b.get("height").and_then(|h| h.as_f64()).unwrap_or(0.0) > 380.0 {
            b["height"] = serde_json::json!(380.0);
        }
    }

    if val.get("info").is_none() || !val["info"].is_object() {
        val["info"] = serde_json::json!({});
    }

    let info = val.get_mut("info").unwrap();
    info["enabled"] = serde_json::json!(is_visible);
    info["scale"] = serde_json::json!(scale);
    info["opacity"] = serde_json::json!(opacity);
    info["displayName"] = serde_json::json!("ReadMD");
    info["mime"] = serde_json::json!("image/png");
    if info.get("frameW").is_none() {
        info["frameW"] = serde_json::json!(384);
        info["frameH"] = serde_json::json!(512);
        info["framesPerState"] = serde_json::json!(2);
        info["stateRows"] = serde_json::json!(["idle", "wave"]);
    }

    let prev_char = info.get("companion")
        .and_then(|c| c.get("character"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let char_changed = !prev_char.is_empty() && prev_char != character;

    if info.get("companion").is_none() {
        info["companion"] = serde_json::json!({ "character": character });
    } else if let Some(comp) = info.get_mut("companion") {
        comp["character"] = serde_json::json!(character);
    }

    let has_spritesheet = info.get("spritesheetBase64")
        .and_then(|v| v.as_str())
        .map(|s| !s.is_empty())
        .unwrap_or(false);

    if !has_spritesheet || char_changed {
        if let Some((b64, sha)) = get_pet_spritesheet_for_character(app, character) {
            info["spritesheetBase64"] = serde_json::json!(b64);
            info["spritesheetRevision"] = serde_json::json!(sha);
        }
    }

    if let Some(parent) = state_file.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(updated) = serde_json::to_string_pretty(&val) {
        let _ = std::fs::write(&state_file, updated);
    }

    state_file
}

pub(crate) fn is_pet_running() -> bool {
    let mut lock = PET_PROCESS.lock().unwrap();
    if let Some(ref mut child) = *lock {
        match child.try_wait() {
            Ok(None) => return true,
            _ => *lock = None,
        }
    }
    false
}

/// Handler for /api/pets/configure - manages desktop pet settings and process lifecycle
pub(crate) fn h_pets_configure(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    #[derive(serde::Deserialize)]
    struct PetConfigReq {
        enabled: Option<bool>,
        in_app: Option<bool>,
        renderer: Option<String>,
        scale: Option<f64>,
        opacity: Option<f64>,
        character: Option<String>,
    }

    let config: PetConfigReq = serde_json::from_slice(&req.body).unwrap_or(PetConfigReq {
        enabled: None,
        in_app: None,
        renderer: None,
        scale: None,
        opacity: None,
        character: None,
    });

    let root_enabled = app.setting("pet_enabled").as_bool();
    let root_in_app = app.setting("pet_in_app").as_bool();
    let root_renderer = app.setting("pet_renderer").as_str().map(|s| s.to_string());
    let root_scale = app.setting("pet_scale").as_f64();
    let root_opacity = app.setting("pet_opacity").as_f64();
    let root_slug = app.setting("pet_slug").as_str().map(|s| s.to_string());

    let stored = app.setting("pets");
    let prev_enabled = root_enabled.or_else(|| stored.get("enabled").and_then(|v| v.as_bool())).unwrap_or(false);
    let prev_in_app = root_in_app.or_else(|| stored.get("in_app").and_then(|v| v.as_bool())).unwrap_or(true);
    let prev_renderer = root_renderer.or_else(|| stored.get("renderer").and_then(|v| v.as_str()).map(|s| s.to_string())).unwrap_or_else(|| "hermes-sprite".to_string());
    let prev_scale = root_scale.or_else(|| stored.get("scale").and_then(|v| v.as_f64())).unwrap_or(0.33);
    let prev_opacity = root_opacity.or_else(|| stored.get("opacity").and_then(|v| v.as_f64())).unwrap_or(1.0);
    let prev_char = root_slug.or_else(|| stored.get("character").and_then(|v| v.as_str()).map(|s| s.to_string())).unwrap_or_else(|| "hermes".to_string());

    let enabled = config.enabled.unwrap_or(prev_enabled);
    let in_app = config.in_app.unwrap_or(prev_in_app);
    let renderer = config.renderer.unwrap_or(prev_renderer);
    let scale = config.scale.unwrap_or(prev_scale);
    let opacity = config.opacity.unwrap_or(prev_opacity);
    let character = config.character.unwrap_or(prev_char);

    app.update_settings(&serde_json::json!({
        "pet_enabled": enabled,
        "pet_in_app": in_app,
        "pet_renderer": &renderer,
        "pet_scale": scale,
        "pet_opacity": opacity,
        "pet_slug": &character,
        "pets": {
            "enabled": enabled,
            "in_app": in_app,
            "renderer": &renderer,
            "scale": scale,
            "opacity": opacity,
            "character": &character,
        }
    }));

    let state_file = ensure_pet_state_file(app, enabled, in_app, &renderer, scale, opacity, &character);

    if enabled && !in_app {
        stop_pet_process();
        if let Some(host_exe) = find_pet_host_exe(app) {
            let runtime_dir = host_exe.parent().unwrap_or_else(|| std::path::Path::new("."));
            let renderer_root = runtime_dir.join("renderer");
            let data_dir = state_file.parent().unwrap_or_else(|| std::path::Path::new("."));
            let webview2_dir = data_dir.join("webview2");
            let _ = std::fs::create_dir_all(&webview2_dir);

            let mut cmd = crate::silent_command(&host_exe);
            cmd.current_dir(runtime_dir)
                .env("READMD_PET_BRIDGE_FILE", &state_file)
                .env("READMD_PARENT_PID", std::process::id().to_string())
                .env("READMD_PET_RENDERER", &renderer)
                .env("READMD_PET_RUNTIME_DIR", runtime_dir)
                .env("READMD_PET_RENDERER_ROOT", &renderer_root)
                .env("READMD_DATA_DIR", data_dir)
                .env("WEBVIEW2_USER_DATA_FOLDER", &webview2_dir);

            if let Ok(child) = cmd.spawn() {
                *PET_PROCESS.lock().unwrap() = Some(child);
            }
        }
    } else {
        stop_pet_process();
    }

    let running = is_pet_running();
    let status_obj = serde_json::json!({
        "adapter": { "available": true, "name": "Desktop Pet Adapter" },
        "active_pet": &renderer,
        "active_slug": character,
        "enabled": enabled,
        "installed": true,
        "in_app": in_app,
        "preferences": {
            "renderer": &renderer,
            "scale": scale,
            "opacity": opacity,
        },
        "running": running,
    });
    Ok(Response::json(&serde_json::json!({
        "ok": true,
        "enabled": enabled,
        "installed": true,
        "in_app": in_app,
        "running": running,
        "active_pet": &renderer,
        "active_slug": character,
        "preferences": {
            "renderer": &renderer,
            "scale": scale,
            "opacity": opacity,
        },
        "adapter": { "available": true, "name": "Desktop Pet Adapter" },
        "status": status_obj,
    })))
}

pub(crate) fn h_pets_runtime_install(app: &Arc<App>, _req: &Request) -> ApiResult<Response> {
    app.update_settings(&json!({ "pet_runtime_installed": true }));
    ok_json(json!({
        "ok": true,
        "installed": true,
        "runtime": "native-rust",
    }))
}

/// `readmd.py:2195-2207` — `Handler._api_export_presentation`.
///
/// Two things this route must never do, and the old handler did both:
/// * **refuse an empty deck.**  Python reads `content` and hands it straight to
///   `presentation_render.generate_presentation_html` (the alias at
///   `presentation_render.py:675`, i.e. `render_presentation_html`), so an empty
///   document is a valid empty deck and the answer is always
///   `200 {'ok': True, 'html': …}`.  The invented
///   `400 {'error_code':'empty_content','error':'内容为空'}` existed nowhere in
///   Python — `empty_content` appears in neither `readmd.py` nor `src/`, and the
///   renderer simply has no empty-input error.
/// * **open a window.**  The `save` branch spawned a modal PowerShell
///   `SaveFileDialog` on the request thread; `body['save']` does not exist in
///   this API, and writing a file to a user-chosen path is the desktop bridge's
///   job, not the route's.
///
/// The route lives in the shared `_route()`, so GET, POST and DELETE all reach it
/// (`readmd.py:1054-1099`); with no body the handler renders an empty deck rather
/// than erroring.
pub(crate) fn h_export_presentation(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    let failed = || api_error(500, "presentation_export_failed");

    // `n = int(self.headers.get('Content-Length', 0) or 0)`;
    // `body = json.loads(self.rfile.read(n).decode('utf-8')) if n else {}`.
    // Both the header parse and `json.loads` sit inside the route's own `try`,
    // so either failure is the 500 below.
    let n = match content_length(req) {
        Some(v) => v.max(0) as usize,
        None => return Ok(failed()),
    };
    let body: Value = if n == 0 {
        json!({})
    } else {
        let raw = if n < req.body.len() { &req.body[..n] } else { &req.body[..] };
        match serde_json::from_slice(raw) {
            Ok(v) => v,
            Err(_) => return Ok(failed()),
        }
    };
    // A non-dict document raises on the first `body.get(...)` in Python.
    if !body.is_object() {
        return Ok(failed());
    }

    // `body.get(key, default)` for a *str*: `content` defaults to `''`,
    // `theme` to `'black'`, `transition` to `'slide'`.  A non-str value is the
    // renderer's `AttributeError`/`TypeError` -> the same 500.
    let str_field = |key: &str, fallback: &str| -> Result<String, Response> {
        match body.get(key) {
            None => Ok(fallback.to_string()),
            Some(Value::String(v)) => Ok(v.clone()),
            Some(_) => Err(failed()),
        }
    };
    let content = match str_field("content", "") {
        Ok(v) => v,
        Err(res) => return Ok(res),
    };
    let theme = match str_field("theme", "black") {
        Ok(v) => v,
        Err(res) => return Ok(res),
    };
    let transition = match str_field("transition", "slide") {
        Ok(v) => v,
        Err(res) => return Ok(res),
    };

    // `mdexport::render_presentation_html` is the line-for-line port of
    // `presentation_render.render_presentation_html` (front-matter overrides,
    // theme/transition whitelists, `split_slides_structure`, the sanitizer, the
    // vendor tags and the reveal config).  Python's call keeps the module
    // defaults `title="ReadMD Presentation"` and `standalone=False` — the
    // in-app preview that references same-origin vendor assets instead of
    // inlining them.
    match crate::mdexport::render_presentation_html(
        &content,
        "ReadMD Presentation",
        &theme,
        &transition,
        false,
        &app.paths.assets_dir,
    ) {
        Ok(html_out) => Ok(Response::json(&json!({ "ok": true, "html": html_out }))),
        // `_read_vendor` is what usually fails; Python reports it through the
        // handler's `except` as `_send_api_error(500, 'presentation_export_failed')`.
        Err(_) => Ok(failed()),
    }
}

/// Handler for /api/code/run - runs sandboxed code chunk natively
pub(crate) fn h_code_run(_app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    let body = req.json()?;
    if body.get("confirm").and_then(|v| v.as_bool()) != Some(true) {
        return Ok(Response::json_status(400, &serde_json::json!({
            "ok": false,
            "error_code": "confirmation_required"
        })));
    }
    let code = body.get("code").and_then(|v| v.as_str()).unwrap_or("");
    let lang = body.get("lang").and_then(|v| v.as_str()).unwrap_or("python");
    let cwd = body.get("cwd").and_then(|v| v.as_str()).map(std::path::Path::new);
    let timeout = body.get("timeout").and_then(|v| v.as_u64()).unwrap_or(10);
    
    let result = crate::code_chunk_runner::execute_code_chunk(code, lang, true, timeout, cwd);
    Ok(Response::json_serde(&result))
}

/// Handler for /api/ocr - native OCR with layout analysis
pub(crate) fn h_ocr(_app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    // `readmd.py:1213-1218` hands `_api_ocr(self, p)` the route-level
    // `unquote(qs.get('p', [''])[0])`: query-only (a body-carried `p` is never
    // read) and percent-decoded a second time on top of `parse_qs`.
    let p = py_unquote(req.q("p").unwrap_or(""));
    if !Path::new(&p).is_file() {
        // `readmd.py:3357-3359` — legacy `_send_json` envelope, no `ok`.
        return Ok(Response::json_status(404, &json!({ "error": "文件不存在" })));
    }
    // `readmd.py:3360 self._module_ready('ocr', …)` only answers 409/503 while
    // Python's daemon thread is still importing `requests`/`pytesseract`.  The
    // kernel compiles the engine ladder in, so the module is permanently ready
    // and the gate cannot fire.
    //
    // `ocr_any` is Python's dispatcher (`.pdf` -> `ocr_pdf_to_md`, image
    // extensions -> `ocr_image_to_md`, which prefixes `![原图](path)`, anything
    // else -> `ValueError('ocr-unsupported-type…')` -> 500 `ocr_failed`).  The
    // old handler called `ocr_image` for everything, so a PDF was fed to the
    // image engine, an image lost its `![原图]()` wrapper, a `.txt` answered
    // 200 with empty content where Python 500s, and `fixes` was hardcoded.
    match crate::ocr::ocr_any(&p) {
        Ok(text) => {
            // `readmd.py:3365 fr = readmd_fix.fix_markdown(text or '')`
            let fixed = crate::readmd_fix::fix_markdown(&text);
            Ok(Response::json(&json!({
                "content": fixed.text,
                "fixes": fixed.fixes,
                "name": crate::convert::basename(&p),
                "dir": crate::convert::dirname(&p),
                "source": "ocr",
                "path": p,
            })))
        }
        Err(_) => Ok(Response::json_status(
            500,
            &json!({ "ok": false, "error_code": "ocr_failed" }),
        )),
    }
}

/// Python's `repr()` for the one shape that reaches it from here: a request
/// header value, i.e. always `str`.  Quote choice (`'…'`, or `"…"` when the text
/// has a `'` and no `"`) and the `\n`/`\r`/`\t`/`\xNN` escapes follow CPython's
/// `unicode_repr`; non-ASCII *printable* code points stay literal, which is also
/// what CPython does.
fn py_repr(text: &str) -> String {
    let quote = if text.contains('\'') && !text.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut out = String::with_capacity(text.len() + 2);
    out.push(quote);
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                out.push_str(&format!("\\x{:02x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

/// `int(text)` as the `Content-Length` guards call it.  Surrounding whitespace is
/// accepted (`int(' 5 ') == 5`); the exotic forms — `'1_0'` digit grouping and
/// non-ASCII decimal digits — are not, and fail with CPython's own message, which
/// quotes the *untrimmed* argument.
fn py_int_checked(text: &str) -> Result<i64, String> {
    match text.trim().parse::<i64>() {
        Ok(value) => Ok(value),
        Err(_) => Err(format!(
            "invalid literal for int() with base 10: {}",
            py_repr(text)
        )),
    }
}

/// `json.loads` on a document whose first non-whitespace token is not a value:
/// CPython's scanner says `Expecting value: line L column C (char O)`, and
/// `_api_transcribe` forwards `str(exc)` as `error_code`.  Column and char are
/// code-point offsets, not byte offsets.
fn json_decode_message(raw: &[u8]) -> String {
    let text = String::from_utf8_lossy(raw).into_owned();
    let byte_offset = text
        .char_indices()
        .find(|(_, c)| !c.is_whitespace())
        .map(|(i, _)| i)
        .unwrap_or(text.len());
    let char_offset = text[..byte_offset].chars().count();
    let before = &text[..byte_offset];
    let line = before.matches('\n').count() + 1;
    let column = before
        .rsplit('\n')
        .next()
        .map(|s| s.chars().count() + 1)
        .unwrap_or(1);
    format!(
        "Expecting value: line {} column {} (char {})",
        line, column, char_offset
    )
}

/// `Handler._read_json_body(limit)` (`readmd.py:1463-1469`) with
/// `_read_request_body_limited` (`readmd.py:1432-1461`) folded in.  `Err(message)`
/// is the `ValueError` that escapes to the route's
/// `except ValueError as e: self._send_api_error(400, str(e))`.
///
/// Three consequences the inline `content_length()` readers in this module do not
/// need: the header `int()` runs *outside* the bounded helper (so its own message
/// is what reaches the 400), `if not n: return {}` short-circuits **before** the
/// parser (an empty declaration is not an error at all), and the size gate is
/// judged on the *declared* length, never on the bytes the transport buffered.
fn read_json_body(req: &Request, limit: i64) -> Result<Value, String> {
    let declared = match req.header("content-length") {
        None => 0,
        // `self.headers.get('Content-Length', 0) or 0` — only the *empty* value
        // is falsy, so `int()` never sees `''`.
        Some(text) if text.is_empty() => 0,
        Some(text) => py_int_checked(text)?,
    };
    if declared == 0 {
        return Ok(json!({}));
    }
    if declared < 0 || declared > limit {
        return Err("request_too_large".to_string());
    }
    let want = declared as usize;
    if req.body.len() < want {
        // `if not chunk: raise ValueError('incomplete_request')`
        return Err("incomplete_request".to_string());
    }
    let raw = &req.body[..want];
    match serde_json::from_slice::<Value>(raw) {
        Ok(value) => Ok(value),
        Err(_) => Err(json_decode_message(raw)),
    }
}

/// `readmd.py:2058-2060` — the three string fields `_api_transcribe` reads, split
/// out of the handler so their semantics are assertable without an engine:
///
/// * `str(body.get('path', '')).strip()` — absent is `''`.
/// * `str(body.get('language', '')).strip() or None` — absent/blank is `None`, and
///   an explicit `null` is `str(None)` == `"None"`, which is *truthy*, so a JSON
///   `null` language really does reach the engine as the four-letter word.  That
///   is CPython's answer, not a bug to smooth over.
/// * `str(body.get('model', 'base')).strip()` — the default is `'base'` only when
///   the key is **absent**; `null` is `"None"` and `""` is `""`.
fn transcribe_fields(body: &Value) -> (String, Option<String>, String) {
    let str_field = |key: &str, fallback: &str| -> String {
        let value = body
            .get(key)
            .cloned()
            .unwrap_or_else(|| Value::String(fallback.to_string()));
        crate::diagrams::py_strip(&crate::mdexport::py_str(&value)).to_string()
    };
    let file_path = str_field("path", "");
    let language = {
        let value = str_field("language", "");
        if value.is_empty() {
            None
        } else {
            Some(value)
        }
    };
    let model_name = str_field("model", "base");
    (file_path, language, model_name)
}

/// `readmd.py:2052-2080` — `Handler._api_transcribe`.
///
/// R2's blocker listed four faults in the previous handler, all fixed here: no
/// `POST` gate, no media-type gate, `error`-keyed bodies instead of
/// `_send_api_error`'s `{'ok': False, 'error_code': …}`, and a dropped `warning`.
/// It also read the path from the query/form and used `Path::exists()`, so `?p=`
/// worked where Python only ever looks at `body['path']`, and a *directory* passed
/// the file check.
///
/// Python's order: 405 -> `_read_json_body(1 MiB)` -> `str(body.get(...)).strip()`
/// for `path`/`language`/`model` -> 404 `file_not_found` -> 400
/// `unsupported_media_format` -> `transcribe_to_md()`.  A truthy text is a 200
/// `{'ok','content','path','warning'}` **even when it is only the install notice**:
/// `transcribe_to_md` returns `(notice, 'Whisper plugin or FFmpeg not available.')`
/// when neither engine is present, which is the answer a user without whisper sees.
/// Otherwise a truthy error is 422 `transcribe_failed` + `error_detail`, and a
/// silent failure is 500 `transcribe_empty`.
pub(crate) fn h_transcribe(_app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    if req.method != "POST" {
        return Ok(api_error(405, "method_not_allowed"));
    }
    let body = match read_json_body(req, 1024 * 1024) {
        Ok(value) => value,
        Err(message) => return Ok(api_error(400, &message)),
    };
    // `body.get('path', '')` on a JSON list/string/number raises `AttributeError`,
    // which the route's `except Exception` reports as `transcribe_failed`.
    if !body.is_object() {
        return Ok(api_error(500, "transcribe_failed"));
    }
    let (file_path, language, model_name) = transcribe_fields(&body);

    if file_path.is_empty() || !Path::new(&file_path).is_file() {
        return Ok(api_error(404, "file_not_found"));
    }
    if !crate::transcribe::is_supported_media(&file_path) {
        return Ok(api_error(400, "unsupported_media_format"));
    }
    // Both names are handed to the engine (`readmd.py:2068`
    // `transcribe_to_md(file_path, model_name=model_name, language=language)`);
    // `transcribe::format_segments` writes them into the document's front-matter
    // as `model:` / `language:`.
    let (text, warning) =
        crate::transcribe::transcribe_to_md(&file_path, language.as_deref(), &model_name);
    if let Some(content) = text {
        if !content.is_empty() {
            // `_send_json(200, {'ok': True, 'content': …, 'path': …,
            //                   'warning': err if err else None})`
            return ok_json(json!({
                "ok": true,
                "content": content,
                "path": file_path,
                "warning": warning,
            }));
        }
    }
    if let Some(detail) = warning {
        return Ok(Response::json_status(422, &json!({
            "ok": false,
            "error_code": "transcribe_failed",
            "error_detail": detail,
        })));
    }
    Ok(api_error(500, "transcribe_empty"))
}

/// `urllib.parse.unquote` — `readmd.py:1214` applies it on top of the value
/// `parse_qs` has already decoded, so the kernel has to decode twice too.
fn py_unquote(v: &str) -> String {
    percent_encoding::percent_decode_str(v)
        .decode_utf8_lossy()
        .into_owned()
}

/// `readmd.py:1421 Handler._send_api_error(status, error_code, **extra)` —
/// exactly `{'ok': False, 'error_code': code}` (plus extras), never an `error`
/// string: "The UI owns wording through i18n."
fn api_error(status: u16, code: &str) -> Response {
    Response::json_status(status, &json!({ "ok": false, "error_code": code }))
}

/// Host-side leniency the desktop bridges rely on: `file://` URLs and the
/// `/C:/...` form.  `readmd.py` does none of this — it hands the decoded query
/// value straight to `os.path.isfile` — so the *uncleaned* value stays what the
/// route echoes as `path` and splits for `name`/`dir`.  `p` arrives already
/// percent-decoded twice (`parse_query` + `py_unquote`), matching
/// `readmd.py:1214`; decoding it a third time here would break files whose
/// literal name still contains a `%XX` sequence.
fn cleaned_doc_path(p: &str) -> String {
    let mut cleaned = p.strip_prefix("file://").unwrap_or(p).trim();
    if (cleaned.starts_with('/') || cleaned.starts_with('\\')) && cleaned.len() >= 3 {
        let bytes = cleaned.as_bytes();
        if bytes[1].is_ascii_alphabetic() && bytes[2] == b':' {
            cleaned = &cleaned[1..];
        }
    }
    cleaned.to_string()
}

/// `readmd.py:3138 if not os.path.isfile(p)`.  Absolute paths are used exactly
/// as the client wrote them; the kernel additionally accepts a
/// workspace-relative path through `AppPaths::resolve_doc`.
fn convert_target(app: &Arc<App>, p: &str) -> Option<PathBuf> {
    let cleaned = cleaned_doc_path(p);
    if cleaned.is_empty() {
        return None;
    }
    let direct = PathBuf::from(&cleaned);
    if direct.is_file() {
        return Some(direct);
    }
    app.paths
        .resolve_doc(&cleaned)
        .ok()
        .filter(|resolved| resolved.is_file())
}

/// `readmd.py:3163 fixes = [w['msg'] for w in warns if w.get('level') == 'auto']`
/// — also used by `_convert_txt` (readmd.py:3198) and `_convert_worker`.
fn auto_fixes(warns: &[Value]) -> Vec<String> {
    warns
        .iter()
        .filter(|w| w.get("level").and_then(|v| v.as_str()) == Some("auto"))
        .filter_map(|w| w.get("msg").and_then(|v| v.as_str()).map(|s| s.to_string()))
        .collect()
}

/// `readmd.py:3164-3174` / `readmd.py:3199-3209`: `out = _md_output_path(p)`,
/// `overwrite = qs['overwrite'] == '1' or _is_upload_path(p)`, then the autosave
/// whose failure Python only logs (`saved` stays `false`, status still 200).
fn autosave_md(fixed: &str, out: &Path, overwrite: bool) -> (bool, bool) {
    if out.exists() && !overwrite {
        return (false, true);
    }
    match crate::convert::write_md(&out.to_string_lossy(), fixed) {
        Ok(()) => (true, false),
        Err(_) => (false, false),
    }
}

/// `os.path.dirname(os.path.abspath(p))`, the base directory `MDC.check`
/// resolves relative image references against (readmd.py:3162, 3197).
fn mdcheck_base_dir(working: &Path) -> String {
    crate::convert::dirname(&crate::convert::py_abspath(&working.to_string_lossy()))
}

/// `readmd.py:3184 Handler._convert_txt(p)` — TXT intelligence, no convert
/// module involved.
fn convert_txt_lane(app: &Arc<App>, p: &str, working: &Path, overwrite_q: bool) -> Response {
    let name = crate::convert::basename(p);
    let dir = crate::convert::dirname(p);
    let ws = working.to_string_lossy().into_owned();

    let (text, _enc) = match crate::convert::read_text_smart(&ws) {
        Ok(v) => v,
        Err(_) => return api_error(500, "conversion_failed"),
    };
    let (md, tstats) = crate::convert::txt_to_markdown(&text);
    if md.trim().is_empty() {
        return Response::json(&json!({
            "content": "",
            "name": name,
            "dir": dir,
            "source": "convert",
            "engine": "txt 智能识别",
            "note": "文件为空，没有可转换的内容"
        }));
    }

    let (fixed, warns) = crate::convert::mdcheck_check(&md, &mdcheck_base_dir(working));
    let fixes = auto_fixes(&warns);
    let out = PathBuf::from(crate::convert::md_output_path(&ws));
    let overwrite = overwrite_q
        || crate::convert::is_upload_path(&ws, &app.paths.data_dir);
    let (saved, skipped) = autosave_md(&fixed, &out, overwrite);

    Response::json(&json!({
        "content": fixed,
        "fixes": fixes,
        "name": name,
        "dir": dir,
        "source": "convert",
        "path": p,
        "engine": if tstats.changed { "txt 智能识别" } else { "TXT" },
        "out": out.to_string_lossy(),
        "saved": saved,
        "skipped": skipped,
        "warns": warns
    }))
}

/// Handler for /api/convert - document to markdown conversion
/// (`readmd.py:1213 p = unquote(qs.get('p', [''])[0])` + `_api_convert`).
pub(crate) fn h_convert(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    // The route reads its path from the query string only — no body fallback.
    let p = py_unquote(req.q("p").unwrap_or(""));

    if let Some(target) = convert_target(app, &p) {
        let ext = crate::convert::ext_of(&p);
        // readmd.py:3141 — the legacy-Windows capability gate.
        if crate::convert::is_win7()
            && !crate::convert::WIN7_CONVERT_EXTS.contains(&ext.as_str())
        {
            return Ok(api_error(415, "unsupported_on_legacy_windows"));
        }
        // readmd.py:3165 `overwrite` is a query flag; there is no `form_tables`
        // switch on this route, the module is always called with its default.
        let overwrite_q = req.q("overwrite").map(|v| v == "1").unwrap_or(false);
        if ext == ".txt" {
            return Ok(convert_txt_lane(app, &p, &target, overwrite_q));
        }

        let ws = target.to_string_lossy().into_owned();
        let res = crate::convert::convert_triple(&ws, true);
        let err = res.error.clone().unwrap_or_default();
        if !err.is_empty() && res.text.is_empty() {
            // readmd.py:3152 `if err and not text:`
            return Ok(Response::json_status(
                422,
                &json!({ "ok": false, "error_code": "conversion_failed", "engine": res.engine }),
            ));
        }
        if res.text.trim().is_empty() {
            // readmd.py:3155 — an empty extraction is a 200, not an error.
            return Ok(Response::json(&json!({
                "content": "",
                "name": crate::convert::basename(&p),
                "dir": crate::convert::dirname(&p),
                "source": "convert",
                "engine": res.engine,
                "note": "未提取到文字，可尝试“扫描转 MD”（OCR）"
            })));
        }

        let name = crate::convert::basename(&p);
        let dir = crate::convert::dirname(&p);
        let (fixed, warns) = crate::convert::mdcheck_check(&res.text, &mdcheck_base_dir(&target));
        let fixes = auto_fixes(&warns);
        let out = PathBuf::from(crate::convert::md_output_path(&ws));
        let overwrite = overwrite_q || crate::convert::is_upload_path(&ws, &app.paths.data_dir);
        let (saved, skipped) = autosave_md(&fixed, &out, overwrite);

        return Ok(Response::json(&json!({
            "content": fixed,
            "fixes": fixes,
            "name": name,
            "dir": dir,
            "source": "convert",
            "path": p,
            "engine": res.engine,
            "out": out.to_string_lossy(),
            "saved": saved,
            "skipped": skipped,
            "warns": warns
        })));
    }

    // readmd.py:3138-3140 — a missing, empty or unreadable `p` all land here.
    Ok(api_error(404, "file_not_found"))
}

/// Handler for /api/convert/batch - batch document to markdown conversion.
/// `readmd.py:3220 Handler._api_convert_batch`, gates in Python's order.
pub(crate) fn h_convert_batch(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    let n = match content_length(req) {
        Some(v) => v,
        // readmd.py:3223 `except (TypeError, ValueError): _send_api_error(400, 'invalid_content_length')`
        None => return Err(ApiError::new(400, "invalid_content_length")),
    };
    // readmd.py:3226 — 64 KiB of JSON body is the ceiling; Python additionally
    // sets `close_connection = True` (a `server.rs` concern, see the report).
    if n < 0 || n > 64 * 1024 {
        return Ok(api_error(413, "request_too_large"));
    }
    let body: Value = if n == 0 {
        json!({})
    } else {
        match serde_json::from_slice(&req.body) {
            Ok(v) => v,
            Err(_) => return Ok(api_error(400, "invalid_request")),
        }
    };
    if !body.is_object() {
        // `body.get('confirm')` raises AttributeError on a JSON array and
        // `_api_convert_batch` does not catch it, so `do_POST` (`readmd.py:1080`)
        // emits the plain-text `internal error`.
        return Err(ApiError::plain_text(500, "internal error"));
    }
    if body.get("confirm").and_then(Value::as_bool) != Some(true) {
        return Ok(api_error(400, "confirmation_required"));
    }
    let mut paths: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for value in body.get("paths").and_then(Value::as_array).into_iter().flatten() {
        let path = match value.as_str() {
            Some(s) => s,
            None => continue, // `isinstance(p, str)`
        };
        if !Path::new(path).is_file() {
            continue; // `os.path.isfile(p)`
        }
        // "A repeated selection should represent one work item, otherwise two
        // workers could race on the same output and report a false success."
        let key =
            crate::convert::py_normcase(&crate::convert::py_realpath(&crate::convert::py_abspath(path)));
        if !seen.insert(key) {
            continue;
        }
        paths.push(path.to_string());
    }
    if crate::convert::is_win7() {
        paths.retain(|p| {
            crate::convert::WIN7_CONVERT_EXTS.contains(&crate::convert::ext_of(p).as_str())
        });
    }
    if paths.is_empty() {
        return Ok(api_error(400, "no_convertible_files"));
    }
    // `bool(body.get('overwrite'))` — Python truthiness, not `is True`.
    let overwrite = body
        .get("overwrite")
        .map(crate::link_indexer::py_truthy)
        .unwrap_or(false);
    let total = paths.len();
    let jid = start_convert_job(app, paths, overwrite);
    Ok(Response::json(&json!({ "job": jid, "total": total })))
}

/// `readmd.py:926 _start_convert_job(paths, overwrite)` — the collision-free
/// outputs are planned once by `_batch_output_paths` before any worker touches a
/// file, the id is `'c%d'` off a process counter, and the worker thread walks
/// the registered job.
fn start_convert_job(app: &Arc<App>, paths: Vec<String>, overwrite: bool) -> String {
    static CONVERT_JOB_SEQ: std::sync::atomic::AtomicUsize =
        std::sync::atomic::AtomicUsize::new(0);
    let jid = format!(
        "c{}",
        CONVERT_JOB_SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1
    );
    let outputs = crate::convert::batch_output_paths(&paths);
    let items: Vec<ConvertJobItem> = paths
        .iter()
        .map(|p| ConvertJobItem {
            src: p.clone(),
            planned_out: outputs.get(p).cloned(),
            status: "queued".to_string(),
            out: None,
            engine: None,
            warns: None,
            error: None,
            error_code: None,
            done: false,
        })
        .collect();
    {
        let mut guard = CONVERT_JOBS.lock().unwrap_or_else(|e| e.into_inner());
        guard.get_or_insert_with(HashMap::new).insert(
            jid.clone(),
            ConvertBatchJob {
                id: jid.clone(),
                overwrite,
                running: true,
                finished: false,
                cancel: false,
                items,
            },
        );
    }
    let worker_id = jid.clone();
    let data_dir = app.paths.data_dir.clone();
    std::thread::spawn(move || convert_worker(&worker_id, &data_dir));
    jid
}

/// `readmd.py:875 _convert_worker(job)` — one item at a time, re-reading the
/// cancel flag between items exactly like Python's `for it in items:` loop does,
/// and writing the **mdcheck-fixed** text to the planned path.
fn convert_worker(job_id: &str, data_dir: &Path) {
    let mut idx = 0usize;
    loop {
        let (src, planned_out, overwrite) = {
            let mut guard = CONVERT_JOBS.lock().unwrap_or_else(|e| e.into_inner());
            let map = guard.get_or_insert_with(HashMap::new);
            let job = match map.get_mut(job_id) {
                Some(job) => job,
                None => return,
            };
            if job.cancel {
                for it in job.items.iter_mut().skip(idx) {
                    it.status = "canceled".to_string();
                    it.done = true;
                }
                job.running = false;
                job.finished = true;
                return;
            }
            if idx >= job.items.len() {
                job.running = false;
                job.finished = true;
                return;
            }
            job.items[idx].status = "running".to_string();
            (
                job.items[idx].src.clone(),
                job.items[idx].planned_out.clone(),
                job.overwrite,
            )
        };

        // `mod.convert_verbose(it['src'])` — the raw client path, verbatim.
        let res = crate::convert::convert_triple(&src, true);
        let err = res.error.clone().unwrap_or_default();
        let mut status = "error".to_string();
        let mut error: Option<String> = None;
        let mut error_code: Option<String> = None;
        let mut out: Option<String> = None;
        let mut engine: Option<String> = None;
        let mut warns: Option<Vec<Value>> = None;

        if !err.is_empty() && res.text.is_empty() {
            // `if err and not text:` — nothing was produced, so no `out`/`engine`.
            error = Some(err.clone());
            error_code = Some("conversion_failed".to_string());
        } else if res.text.trim().is_empty() {
            error = Some("未提取到文字（可尝试 OCR）".to_string());
            error_code = Some("empty_output".to_string());
        } else {
            let base_dir = mdcheck_base_dir(Path::new(&src));
            let (fixed, issues) = crate::convert::mdcheck_check(&res.text, &base_dir);
            let target = planned_out
                .unwrap_or_else(|| crate::convert::md_output_path(&src));
            out = Some(target.clone());
            engine = Some(res.engine.clone());
            warns = Some(issues);
            // `allow_overwrite = bool(job.get('overwrite')) or _is_upload_path(it['src'])`
            let allow_overwrite = overwrite || crate::convert::is_upload_path(&src, data_dir);
            if Path::new(&target).exists() && !allow_overwrite {
                status = "skipped".to_string();
                error_code = Some("output_exists".to_string());
            } else {
                match crate::convert::write_md(&target, &fixed) {
                    Ok(()) => status = "ok".to_string(),
                    Err(e) => {
                        error = Some(format!("写入失败：{e}"));
                        error_code = Some("write_failed".to_string());
                    }
                }
            }
        }

        {
            let mut guard = CONVERT_JOBS.lock().unwrap_or_else(|e| e.into_inner());
            let map = guard.get_or_insert_with(HashMap::new);
            if let Some(job) = map.get_mut(job_id) {
                if idx < job.items.len() {
                    let item = &mut job.items[idx];
                    item.status = status;
                    item.error = error;
                    item.error_code = error_code;
                    item.out = out;
                    item.engine = engine;
                    item.warns = warns;
                    item.done = true;
                }
            }
        }

        idx += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_now_millis() {
        let millis = now_millis();
        assert!(millis > 0);
    }
    
    #[test]
    fn test_generate_uuid() {
        let uuid1 = generate_uuid();
        let uuid2 = generate_uuid();
        assert_ne!(uuid1, uuid2);
        assert!(!uuid1.is_empty());
    }
    
    /// `updater.py:51-65` — the real `/api/update/status` contract: the
    /// invented `available`/`version`/`downloaded`/`install_ready` keys are not
    /// on it.  Completion is `status == "ready"`, which Python only writes
    /// after `compute_file_sha256()` matched `expected_sha`.
    #[test]
    fn test_download_state_default_matches_updater_py() {
        let value = serde_json::to_value(DownloadState::default()).expect("serializable");
        assert_eq!(
            key_set(&value),
            vec![
                "asset_name",
                "cancel_requested",
                "downloaded_bytes",
                "error",
                "error_code",
                "expected_sha",
                "percent",
                "running",
                "speed_bps",
                "status",
                "target_file",
                "total_bytes",
                "verified_sha",
            ]
        );
        assert_eq!(value["status"], "idle");
        assert_eq!(value["running"], false);
        assert!(value.get("downloaded").is_none());
        assert!(value.get("available").is_none());
    }

    // ====================================================================
    // /api/update/* — parity pins for `updater.py` + `readmd.py:1540-1595`
    // ====================================================================

    /// The download lane keeps its progress in one process-global
    /// `_download_state` — Python's does too (`updater.py:51-65`) — so a test that
    /// raises `running` or writes `ready` must hand the lane back exactly as it
    /// found it.  Holding `ENV_LOCK` for the whole test serialises the update
    /// family (the same convention the convert tests use), so the snapshot /
    /// restore pair cannot interleave with a sibling lane test.
    struct LaneGuard {
        _env: std::sync::MutexGuard<'static, ()>,
        saved: Option<DownloadState>,
        probe: bool,
    }

    impl Drop for LaneGuard {
        fn drop(&mut self) {
            *DOWNLOAD_STATE.lock().unwrap_or_else(|e| e.into_inner()) = self.saved.clone();
            set_startup_probe_mode(self.probe);
        }
    }

    fn lock_update_lane() -> LaneGuard {
        let _env = env_guard();
        let saved = DOWNLOAD_STATE
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        // The *static*, not `startup_probe_enabled()`: the env seam must not be
        // able to switch the static on when the guard is restored.
        let probe = UPDATE_PROBE_MODE.load(std::sync::atomic::Ordering::SeqCst);
        LaneGuard {
            _env,
            saved,
            probe,
        }
    }

    fn set_download_state(state: DownloadState) {
        *DOWNLOAD_STATE.lock().unwrap_or_else(|e| e.into_inner()) = Some(state);
    }

    /// A package name legal for both filename gates (`_UPDATE_FILENAME_RE` and
    /// `_safe_update_target`): alnum first char, `[A-Za-z0-9._-]` body, `.exe`.
    fn unique_package_name(tag: &str) -> String {
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        format!("ReadMDParity-{tag}-{}{}.exe", std::process::id(), nanos)
    }

    /// The only host and prefix `validate_update_source` accepts.
    fn official_asset_url(name: &str) -> String {
        format!(
            "https://github.com/{}/releases/download/v9.9.9/{name}",
            GITHUB_REPO
        )
    }

    /// The one asset name `match_release_asset` selects for this host's flavor, so
    /// the digest assertions below run identically on Windows, macOS and Linux.
    fn asset_for_flavor(flavor: &str) -> String {
        match flavor {
            "win_portable" => "ReadMD-portable-9.9.9.exe".to_string(),
            "macos" => "ReadMD-macos-x86_64.zip".to_string(),
            "linux" => "readmd_9.9.9_amd64.deb".to_string(),
            _ => "ReadMDSetup-9.9.9.exe".to_string(),
        }
    }

    /// ITEM 1 — `updater._safe_update_target` stages into
    /// `tempfile.gettempdir()/ReadMDUpdates`, so `temp_update_dir()` must be the
    /// live location and nothing under `data_dir` may be consulted.  The old code
    /// had the helper *and* ignored it: `h_update_apply` looked at
    /// `data_dir/updates` and answered `{'ok':true,'applied':true,…}` the moment
    /// any file was sitting there.
    #[test]
    fn update_packages_stage_in_the_temp_folder_never_the_data_dir() {
        let _lane = lock_update_lane();
        let app = test_app("update-temp-dir");
        let name = unique_package_name("stage");
        let url = official_asset_url(&name);
        let digest = crate::crypto::sha256_hex(b"package-bytes");

        let job = begin_download(
            Some(&json!(url)),
            Some(&json!(name)),
            Some(&json!(digest)),
            false,
        )
        .expect("an official, digest-carrying package must be accepted");

        assert_eq!(job.save_path.parent().unwrap(), temp_update_dir().as_path());
        assert_eq!(job.save_path.file_name().unwrap().to_string_lossy(), name);
        assert!(
            !job.save_path.starts_with(&app.paths.data_dir),
            "{} must not be under {}",
            job.save_path.display(),
            app.paths.data_dir.display()
        );
        // `.{name}.{uuid4hex}.part` (`updater.py:548-550`).
        let part = job.part_path.file_name().unwrap().to_string_lossy().to_string();
        assert!(part.starts_with('.') && part.ends_with(".part"), "{part}");
        assert!(part.contains(&name));
        assert_eq!(job.part_path.parent().unwrap(), temp_update_dir().as_path());
        // `/api/update/status` echoes the same folder.
        assert_eq!(
            download_state().target_file,
            job.save_path.to_string_lossy()
        );

        // And `apply` reads *that* lane: a package planted under
        // `data_dir/updates` is not an update, because the trust gate is the
        // verified download state, not a directory listing.
        let planted = app.paths.data_dir.join("updates");
        std::fs::create_dir_all(&planted).unwrap();
        std::fs::write(planted.join("ReadMD-Planted.exe"), b"not verified").unwrap();
        let (status, body, _) = render(h_update_apply(
            &app,
            &json_request("POST", "/api/update/apply", &json!({}), None),
        ));
        // The download above is `downloading`, never `ready`.
        assert_eq!(status, 400);
        assert_eq!(key_set(&body), vec!["message", "ok"]);
        assert_eq!(body["message"], "更新包尚未完成校验");

        let _ = std::fs::remove_dir_all(&planted);
        let _ = std::fs::remove_file(&job.part_path);
        let _ = std::fs::remove_file(&job.save_path);
    }

    /// ITEM 2 (half one) — the digest is *part of the check answer*:
    /// `updater.check_update` fills `asset.expected_sha` from `SHA256SUMS.txt`
    /// through `resolve_expected_sha` (`updater.py:436-444`, `:265-281`), so
    /// `/api/update/check` hands the UI a real digest.  The old `UpdateStatus`
    /// struct had a `sha256` field that was hard-coded `None`.
    #[test]
    fn update_check_resolves_the_digest_from_sha256sums() {
        let _lane = lock_update_lane();
        let asset = asset_for_flavor(&detect_app_flavor());
        let digest = crate::crypto::sha256_hex(b"the-released-bytes");
        let release = json!({
            "tag_name": "v9.9.9",
            "name": "ReadMD v9.9.9",
            "published_at": "2026-01-01T00:00:00Z",
            "body": "notes",
            "html_url": "https://github.com/Natsummerance/readMD/releases/tag/v9.9.9",
            "assets": [
                {
                    "name": asset,
                    "size": 4096,
                    "browser_download_url": official_asset_url(&asset),
                },
                {
                    "name": "SHA256SUMS.txt",
                    "size": 80,
                    "browser_download_url": official_asset_url("SHA256SUMS.txt"),
                },
            ],
        });
        let manifest_text = format!("{digest}  {asset}\n");
        let mut seen: Vec<String> = Vec::new();
        let payload = release_check_payload("1.0.0", Some(&release), "", &mut |url| {
            seen.push(url.to_string());
            Some(manifest_text.clone())
        });

        // `updater.py:446-463` — eleven keys, and `asset` its own four.
        assert_eq!(
            key_set(&payload),
            vec![
                "asset",
                "current_version",
                "flavor",
                "has_update",
                "html_url",
                "latest_version",
                "ok",
                "published_at",
                "release_name",
                "release_notes",
                "sha_url",
            ]
        );
        assert_eq!(
            key_set(&payload["asset"]),
            vec!["download_url", "expected_sha", "name", "size"]
        );
        assert_eq!(payload["ok"], json!(true));
        assert_eq!(payload["has_update"], json!(true));
        assert_eq!(payload["asset"]["expected_sha"], json!(digest));
        assert_eq!(payload["sha_url"], json!(official_asset_url("SHA256SUMS.txt")));
        // The manifest was fetched *for the sha asset's URL*, once.
        assert_eq!(seen, vec![official_asset_url("SHA256SUMS.txt")]);

        // No manifest entry → `expected_sha` is null (`resolve_expected_sha`'s
        // `return None` at `updater.py:281`), i.e. the download lane will then
        // refuse the package rather than install something unverified.
        let mut never: i32 = 0;
        let payload = release_check_payload("1.0.0", Some(&release), "", &mut |_| {
            never += 1;
            Some("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  other.zip\n".to_string())
        });
        assert_eq!(payload["asset"]["expected_sha"], Value::Null);
        assert_eq!(never, 1);

        // Refusing to download without one is `validate_update_source`'s first
        // gate (`updater.py:504-505`).
        for sha in [
            Value::Null,
            json!(""),
            json!("not-64-hex"),
            json!("0".repeat(63)),
            json!("g".repeat(64)),
        ] {
            let (_url, error) = validate_update_source(
                Some(&json!(official_asset_url("ReadMDSetup-9.9.9.exe"))),
                Some(&json!("ReadMDSetup-9.9.9.exe")),
                Some(&sha),
                false,
            );
            assert_eq!(error, "缺少有效的 SHA256 校验值", "sha {sha}");
        }
    }

    /// ITEM 2 (half two) + ITEM 3 — the digest is *actually verified* before
    /// anything is reported usable, and `ready` (the honest replacement for the
    /// invented `downloaded: true`) is a post-write fact.
    /// `updater.py:680-694`: `verifying` → `compute_file_sha256(part)` → compare →
    /// `os.replace` → `ready` + `verified_sha`.  A mismatch never publishes.
    #[test]
    fn download_only_reports_ready_after_the_bytes_are_verified() {
        let _lane = lock_update_lane();
        let name = unique_package_name("verify");
        let url = official_asset_url(&name);
        let payload = b"ReadMD update payload".to_vec();
        let digest = crate::crypto::sha256_hex(&payload);

        let job = begin_download(
            Some(&json!(url)),
            Some(&json!(name)),
            Some(&json!(digest)),
            false,
        )
        .expect("accepted");

        // The moment `/api/update/download` answers, nothing has landed: the old
        // handler claimed `downloaded: true` here.
        let state = download_state();
        assert_eq!(state.status, "downloading");
        assert_eq!(state.running, true);
        assert_eq!(state.percent, 0);
        assert_eq!(state.downloaded_bytes, 0);
        assert!(state.verified_sha.is_empty());
        assert!(!job.save_path.exists(), "the published name is not a lie");
        let value = serde_json::to_value(&state).unwrap();
        assert!(value.get("downloaded").is_none());
        assert!(value.get("available").is_none());
        assert!(value.get("install_ready").is_none());

        // A tampered body is rejected and stays unpublished.
        let mut wrong = payload.clone();
        wrong.push(b'!');
        let failure = {
            let mut copy = DownloadJob {
                url: job.url.clone(),
                save_path: job.save_path.clone(),
                part_path: job.part_path.clone(),
                expected_sha: job.expected_sha.clone(),
            };
            copy.expected_sha = crate::crypto::sha256_hex(b"some-other-build");
            let mut reader = std::io::Cursor::new(wrong.clone());
            land_download(&mut reader, &copy, wrong.len() as u64)
                .err()
                .unwrap_or_default()
        };
        assert!(failure.contains("SHA256 校验失败"), "{failure}");
        assert!(!job.save_path.exists(), "an unverified package never publishes");
        assert_eq!(download_state().status, "verifying");
        let _ = std::fs::remove_file(&job.part_path);

        // The real bytes: verified, published, and only now `ready`.
        let mut reader = std::io::Cursor::new(payload.clone());
        let verified = land_download(&mut reader, &job, payload.len() as u64).expect("verified");
        assert_eq!(verified, digest);
        let state = download_state();
        assert_eq!(state.status, "ready");
        assert_eq!(state.running, false);
        assert_eq!(state.percent, 100);
        assert_eq!(state.verified_sha, digest);
        assert_eq!(state.expected_sha, digest);
        assert_eq!(std::fs::read(&job.save_path).unwrap(), payload);
        assert!(!job.part_path.exists(), "os.replace consumed the .part file");

        let _ = std::fs::remove_file(&job.save_path);
    }

    /// ITEM 3, at the wire: `h_update_download` answers `{ok, message}` and
    /// nothing else — never `downloaded` — and its 400 shape is Python's
    /// `{ok:false, message:'…'}` (`readmd.py:1565`), with the missing-parameter
    /// branch's `{ok:false, error:'缺少下载参数'}` (`readmd.py:1561-1563`) as the
    /// one place an `error` key is legal on this route.
    #[test]
    fn update_download_answers_only_about_the_task() {
        let _lane = lock_update_lane();
        let app = test_app("update-download");

        // No `download_url` → `缺少下载参数`, and no download state is touched.
        set_download_state(DownloadState::default());
        let (status, body, _) = render(h_update_download(
            &app,
            &json_request("POST", "/api/update/download", &json!({}), None),
        ));
        assert_eq!(status, 400);
        assert_eq!(key_set(&body), vec!["error", "ok"]);
        assert_eq!(body["error"], "缺少下载参数");
        assert_eq!(download_state(), DownloadState::default());

        // A package with no digest reaches the trust gate and is refused there.
        let name = unique_package_name("nosha");
        let body_ok = json!({
            "download_url": official_asset_url(&name),
            "target_filename": name,
        });
        let (status, body, _) = render(h_update_download(
            &app,
            &json_request("POST", "/api/update/download", &body_ok, None),
        ));
        assert_eq!(status, 400);
        assert_eq!(key_set(&body), vec!["message", "ok"]);
        assert_eq!(body["message"], "缺少有效的 SHA256 校验值");

        // An off-channel URL is refused too, before a byte is requested.
        let body_evil = json!({
            "download_url": "https://evil.example.com/ReadMDSetup-9.9.9.exe",
            "target_filename": "ReadMDSetup-9.9.9.exe",
            "expected_sha": crate::crypto::sha256_hex(b"x"),
        });
        let (status, body, _) = render(h_update_download(
            &app,
            &json_request("POST", "/api/update/download", &body_evil, None),
        ));
        assert_eq!(status, 400);
        assert_eq!(body["message"], "仅允许官方 GitHub Release 下载地址");

        // A malformed body is the route's 500 (`readmd.py:1566-1568`), not a 400.
        let (status, body, _) = render(h_update_download(
            &app,
            &raw_request("POST", "/api/update/download", b"{oops", "5"),
        ));
        assert_eq!(status, 500);
        assert_eq!(key_set(&body), vec!["error_code", "ok"]);
        assert_eq!(body["error_code"], "update_download_failed");
    }

    /// ITEM 4 — Python's `/api/update/check` reads no body and no query
    /// (`readmd.py:1540-1550` → `updater.check_update(VERSION)`, whose signature
    /// is `(current_version, timeout=2.5)`), so there is no force semantics to
    /// port: a forced check answers *exactly* what a plain check answers.  What
    /// `force` provably does not bypass is pinned here — the probe refusal, the
    /// release-channel rule, and the digest gate.
    #[test]
    fn force_on_the_check_lane_bypasses_nothing() {
        let _lane = lock_update_lane();
        let app = test_app("update-force");

        // `force` *is* read, with Python's `bool(...)` truthiness table.
        let cases: &[(&str, Value, bool)] = &[
            ("absent", json!({}), false),
            ("true", json!({"force": true}), true),
            ("one", json!({"force": 1}), true),
            ("zero", json!({"force": 0}), false),
            ("empty string", json!({"force": ""}), false),
            ("word", json!({"force": "false"}), true),
            ("null", json!({"force": null}), false),
            ("empty list", json!({"force": []}), false),
            ("no body", json!({}), false),
        ];
        for (label, body, want) in cases {
            let req = json_request("GET", "/api/update/check", body, None);
            assert_eq!(update_check_force(&req), *want, "case {label}");
        }
        // A body that was never declared is simply not a force.
        let mut undeclared = api_request("GET", "/api/update/check", &[], br#"{"force": true}"#);
        assert!(!update_check_force(&undeclared));
        undeclared
            .headers
            .insert("content-length".to_string(), "0".to_string());
        assert!(!update_check_force(&undeclared));

        // Gate 1: the startup probe refuses the whole lane, forced or not — and
        // does so *before* any network access (`readmd.py:1541-1543`).
        set_startup_probe_mode(true);
        for body in [json!({}), json!({"force": true})] {
            let (status, payload, _) = render(h_update_check(
                &app,
                &json_request("GET", "/api/update/check", &body, None),
            ));
            assert_eq!(status, 200);
            assert_eq!(key_set(&payload), vec!["error_code", "ok"]);
            assert_eq!(
                payload,
                json!({"ok": false, "error_code": "probe_mode"}),
                "force {body}"
            );
        }
        set_startup_probe_mode(false);

        // Gate 2: the release channel.  A formal build never receives a prerelease
        // (`versioning.select_update_release`), and the answer is the same with or
        // without `force` because `force` never reaches the selector.
        let prerelease = json!({
            "tag_name": "v9.9.9-beta.1",
            "prerelease": true,
            "draft": false,
            "assets": [],
        });
        let picked = crate::validators::select_update_release("2.3.8", &[prerelease.clone()]);
        let plain = release_check_payload("2.3.8", picked.as_ref(), "", &mut |_| None);
        let forced = release_check_payload("2.3.8", picked.as_ref(), "", &mut |_| None);
        assert!(picked.is_none(), "a formal build must not be offered a beta");
        assert_eq!(
            plain,
            json!({
                "ok": false,
                "error_code": "update_network_error",
                "html_url": "https://github.com/Natsummerance/readMD/releases",
            })
        );
        assert_eq!(plain, forced, "force changed the answer");

        // Gate 3: even a *forced* check that found an update cannot hand the
        // download lane something without a digest.
        let name = unique_package_name("forcegate");
        let (url, error) = validate_update_source(
            Some(&json!(official_asset_url(&name))),
            Some(&json!(name)),
            Some(&Value::Null),
            false,
        );
        assert_eq!(url, "");
        assert_eq!(error, "缺少有效的 SHA256 校验值");
    }

    /// ITEM 5 — `readmd.py:2058-2060`'s three string reads, one of which is the
    /// `language`/`model_name` pair the old handler dropped on the way to the
    /// engine.  `or None` and `str(None)` are not the same thing, and Python's
    /// defaults are per-key.
    #[test]
    fn transcribe_keeps_language_and_model_name() {
        let _lane = lock_update_lane();
        let read = |body: Value| transcribe_fields(&body);

        // `language`: absent / blank strip to `None`; an explicit `null` is
        // `str(None)` == "None", which is truthy, so it survives as the word.
        assert_eq!(read(json!({})).1, None);
        assert_eq!(read(json!({"language": ""})).1, None);
        assert_eq!(read(json!({"language": "   "})).1, None);
        assert_eq!(read(json!({"language": null})).1, Some("None".to_string()));
        assert_eq!(
            read(json!({"language": "  zh  "})).1,
            Some("zh".to_string())
        );
        assert_eq!(read(json!({"language": "auto"})).1, Some("auto".to_string()));

        // `model`: the `'base'` default is only for an **absent** key.
        assert_eq!(read(json!({})).2, "base");
        assert_eq!(read(json!({"model": ""})).2, "");
        assert_eq!(read(json!({"model": null})).2, "None");
        assert_eq!(read(json!({"model": "  large-v3 "})).2, "large-v3");
        assert_eq!(read(json!({"model": "medium"})).2, "medium");
        // `path` keeps Python's `str(...).strip()` too.
        assert_eq!(read(json!({"path": "  a.mp3 "})).0, "a.mp3");
        assert_eq!(read(json!({"path": null})).0, "None");

        // And the names are not dead weight: `transcribe::format_segments` writes
        // both into the document's front-matter and the language line, in
        // Python's key spellings.
        let doc = crate::transcribe::format_segments(
            &[crate::transcribe::Segment {
                start: 1.5,
                text: "hello".to_string(),
            }],
            Some("voice.mp3"),
            Some("zh"),
            Some(12.0),
            Some("mp3"),
            Some("large-v3"),
        );
        assert!(doc.contains("model: \"large-v3\""), "{doc}");
        assert!(doc.contains("language: \"zh\""), "{doc}");
        assert!(doc.contains("> 识别语言：`zh`"), "{doc}");
        // With `language: None` the key is absent rather than invented.
        let bare = crate::transcribe::format_segments(
            &[crate::transcribe::Segment {
                start: 0.0,
                text: "hi".to_string(),
            }],
            Some("voice.mp3"),
            None,
            None,
            Some("mp3"),
            Some("base"),
        );
        assert!(!bare.contains("language:"), "{bare}");
        assert!(bare.contains("model: \"base\""), "{bare}");
    }

    // ====================================================================
    // /api/update/cancel — parity pin for readmd.py:_api_update_cancel
    // ====================================================================

    /// `readmd.py:1578-1584` — `self._send_json(200, {'ok': updater.cancel_download()})`,
    /// and `updater.py:490-497` returns `False` unless
    /// `_download_state['running']` is set.  `assets/js/updater.js` branches on
    /// `res.ok`, so answering `ok:true` with nothing to cancel tells the UI an
    /// update download was interrupted when it was not.
    #[test]
    fn update_cancel_answers_false_when_nothing_is_downloading() {
        let _lane = lock_update_lane();
        let app = test_app("update-cancel-idle");
        set_download_state(DownloadState::default());

        let (status, payload, _) = render(h_update_cancel(
            &app,
            &api_request("POST", "/api/update/cancel", &[], &[]),
        ));
        assert_eq!(status, 200);
        // A single key — the old stub invented `cancelled` next to `ok`.
        assert_eq!(key_set(&payload), vec!["ok"]);
        assert_eq!(payload, json!({"ok": false}));
        // Nothing to cancel must not leave a cancel request or a `cancelled`
        // status behind (`updater.py:492-496` writes both only inside `if running`).
        let state = download_state();
        assert!(!state.cancel_requested, "cancel must only raise the flag while running");
        assert_eq!(state.status, "idle");
    }

    /// The `true` half of the same lane, plus the worker's cleanup: the guard
    /// `h_update_download`'s thread owns lowers `running` again, so a later
    /// cancel cannot keep reporting success forever.
    #[test]
    fn update_cancel_reports_true_only_inside_the_download_window() {
        let _lane = lock_update_lane();
        let app = test_app("update-cancel-running");
        set_download_state(DownloadState {
            running: true,
            status: "downloading".to_string(),
            ..DownloadState::default()
        });

        let (status, payload, _) = render(h_update_cancel(
            &app,
            &api_request("POST", "/api/update/cancel", &[], &[]),
        ));
        assert_eq!(status, 200);
        assert_eq!(payload, json!({"ok": true}));
        let state = download_state();
        // `updater.py:494-495`, both keys, in one lock.
        assert!(state.cancel_requested, "cancel_requested must be raised");
        assert_eq!(state.status, "cancelled");
        // `running` stays up until the thread sees the flag — Python's too.
        assert!(state.running, "the worker, not the route, clears running");

        // The download thread always clears it, whatever its exit path.
        {
            let _guard = RunningDownloadGuard;
        }
        assert!(
            !download_state().running,
            "the guard's Drop must reset running, or cancel lies forever"
        );
    }

    /// The same flag is what `/api/update/status` reports, so the UI's cancel
    /// button and its progress bar cannot disagree.
    #[test]
    fn update_status_answers_the_flat_download_state() {
        let _lane = lock_update_lane();
        let app = test_app("update-status");
        set_download_state(DownloadState {
            running: false,
            status: "ready".to_string(),
            percent: 100,
            target_file: temp_update_dir()
                .join("ReadMDSetup-9.9.9.exe")
                .to_string_lossy()
                .to_string(),
            asset_name: "ReadMDSetup-9.9.9.exe".to_string(),
            expected_sha: crate::crypto::sha256_hex(b"x"),
            verified_sha: crate::crypto::sha256_hex(b"x"),
            ..DownloadState::default()
        });

        let (status, body, _) = render(h_update_status(
            &app,
            &api_request("GET", "/api/update/status", &[], &[]),
        ));
        assert_eq!(status, 200);
        assert_eq!(
            key_set(&body),
            vec![
                "asset_name",
                "cancel_requested",
                "downloaded_bytes",
                "error",
                "error_code",
                "expected_sha",
                "percent",
                "running",
                "speed_bps",
                "status",
                "target_file",
                "total_bytes",
                "verified_sha",
            ]
        );
        assert_eq!(body["status"], "ready");
        assert_eq!(body["verified_sha"], body["expected_sha"]);
        // `target_file` is the temp-folder path the download wrote.
        assert!(body["target_file"]
            .as_str()
            .unwrap_or("")
            .starts_with(&temp_update_dir().to_string_lossy().to_string()));
    }

    /// `updater._validate_ready_update` (`updater.py:759-784`) — the five refusal
    /// messages, in Python's order, each a 400 `{ok:false,message}`; and the one
    /// thing `/api/update/apply` must never do: answer Python's
    /// `'正在启动安装器并重启…'` for a process this kernel did not start.
    #[test]
    fn apply_refuses_everything_but_this_runs_verified_package() {
        let _lane = lock_update_lane();
        let app = test_app("update-apply");
        let post = |body: Value| {
            render(h_update_apply(
                &app,
                &json_request("POST", "/api/update/apply", &body, None),
            ))
        };

        // 1. Nothing has been downloaded at all.
        set_download_state(DownloadState::default());
        let (status, body, _) = post(json!({}));
        assert_eq!(status, 400);
        assert_eq!(key_set(&body), vec!["message", "ok"]);
        assert_eq!(body["message"], "更新包尚未完成校验");

        // 2. `ready` without completeness information.
        set_download_state(DownloadState {
            status: "ready".to_string(),
            target_file: String::new(),
            ..DownloadState::default()
        });
        let (_, body, _) = post(json!({}));
        assert_eq!(body["message"], "更新任务缺少完整性信息");

        // 3. A `file_path` that is not the verified package.
        let dir = temp_update_dir();
        std::fs::create_dir_all(&dir).unwrap();
        let good = dir.join(unique_package_name("apply"));
        let bytes = b"verified-installer-bytes".to_vec();
        std::fs::write(&good, &bytes).unwrap();
        let digest = crate::crypto::sha256_hex(&bytes);
        let state = DownloadState {
            status: "ready".to_string(),
            target_file: good.to_string_lossy().to_string(),
            asset_name: good
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string(),
            expected_sha: digest.clone(),
            verified_sha: digest.clone(),
            percent: 100,
            ..DownloadState::default()
        };
        let impostor = dir.join(unique_package_name("impostor"));
        std::fs::write(&impostor, b"something else").unwrap();
        set_download_state(state.clone());
        let (status, body, _) = post(json!({
            "file_path": impostor.to_string_lossy(),
            "flavor": Value::Null,
        }));
        assert_eq!(status, 400);
        assert_eq!(body["message"], "只允许应用本次已校验的更新包");

        // 4. The trusted path, but the bytes were swapped after verification —
        //    the reason apply re-hashes instead of trusting `status`.
        std::fs::write(&good, b"trojan payload").unwrap();
        let (status, body, _) = post(json!({
            "file_path": good.to_string_lossy(),
            "flavor": Value::Null,
        }));
        assert_eq!(status, 400);
        assert_eq!(body["message"], "更新包校验失败，已拒绝安装");
        // `verified_sha` is checked as well as the file.
        std::fs::write(&good, &bytes).unwrap();
        set_download_state(DownloadState {
            verified_sha: "0".repeat(64),
            ..state.clone()
        });
        let (_, body, _) = post(json!({"file_path": good.to_string_lossy()}));
        assert_eq!(body["message"], "更新包校验失败，已拒绝安装");

        // 5. A mismatched flavor is refused even for a package that verifies.
        set_download_state(state.clone());
        let (status, body, _) = post(json!({
            "file_path": good.to_string_lossy(),
            "flavor": if detect_app_flavor() == "win_installer" { "macos" } else { "win_installer" },
        }));
        assert_eq!(status, 400);
        assert_eq!(body["message"], "更新包类型与当前平台不匹配");

        // Verified, trusted, present: the kernel still does not claim to have
        // started an installer, because it may not start one at all.
        set_download_state(state);
        let (status, body, _) = post(json!({"file_path": good.to_string_lossy()}));
        assert_eq!(status, 400);
        assert_eq!(key_set(&body), vec!["message", "ok"]);
        assert_eq!(body["ok"], json!(false));
        assert_ne!(body["message"], "正在启动安装器并重启…");
        assert_eq!(body["message"], "当前平台暂不支持自动替换，请手动解压运行");
        // No invented keys either — the old shape had `applied` and
        // `restart_required`/`no_update_downloaded` coinages.
        assert!(body.get("applied").is_none());

        // A deleted package is `compute_file_sha256`'s `FileNotFoundError` → 500.
        std::fs::remove_file(&good).unwrap();
        let (status, body, _) = post(json!({"file_path": good.to_string_lossy()}));
        assert_eq!(status, 500);
        assert_eq!(key_set(&body), vec!["error_code", "ok"]);
        assert_eq!(body["error_code"], "update_apply_failed");

        let _ = std::fs::remove_file(&impostor);
    }

    /// `updater._UPDATE_FILENAME_RE` (`updater.py:68-71`) is applied through
    /// `fullmatch` at `updater.py:521`, and its body class is
    /// `[A-Za-z0-9._()-]` — **including the hyphen**, which is how every real ReadMD
    /// asset is named (`match_release_asset`, `updater.py:124-201`).  The shared
    /// helper this gate used to call omitted `-`, so `validate_update_source`
    /// answered `'更新文件名无效'` for the official package and no download could ever
    /// start.  Fails before the fix at the very first `assert!`.
    #[test]
    fn update_package_names_follow_pythons_filename_regex() {
        let _lane = lock_update_lane();
        set_download_state(DownloadState::default());

        let longest = format!("a{}.exe", "x".repeat(180));
        let too_long = format!("a{}.exe", "x".repeat(181));
        for name in [
            "ReadMD-portable-2.4.0.exe",
            "ReadMDSetup-9.9.9.exe",
            "ReadMD-macos-x86_64.zip",
            "readmd_9.9.9_amd64.deb",
            "ReadMD-2.4.0-x86_64.AppImage",
            "ReadMD_1.0(1).exe",
            "a.exe",
            "A.EXE",
            // `1 + {0,180} + '.' + "exe"`: the longest name the pattern accepts.
            longest.as_str(),
        ] {
            assert!(update_filename_re_ok(name), "{name} must be accepted");
        }
        for name in [
            "-lead.exe",        // `[A-Za-z0-9]` owns the first character
            ".exe",             // nothing to prepend to the extension
            "ReadMD (1).exe",   // a space is not in `[A-Za-z0-9._()-]`
            "ReadMD 2.4.exe",   // ... nor is one anywhere else
            "ReadMD;setup.exe", // nor is a semicolon
            "a.zexe",           // the extension must follow the *last* dot
            "no_extension",
            "a.txt",
            "a.exe\n", // `fullmatch` anchors `A`/`Z`, so no trailing newline
            "..",
            too_long.as_str(),
        ] {
            assert!(!update_filename_re_ok(name), "{name} must be refused");
        }

        // The same table through the trust gate (`updater.py:500-532`).
        let digest = crate::crypto::sha256_hex(b"official-bytes");
        let url = official_asset_url("ReadMD-portable-9.9.9.exe");
        let (_url, error) = validate_update_source(
            Some(&json!(url)),
            Some(&json!("ReadMD-portable-9.9.9.exe")),
            Some(&json!(digest.clone())),
            false,
        );
        assert_eq!(error, "", "an official hyphenated package must pass");
        let (_url, error) = validate_update_source(
            Some(&json!(official_asset_url("ReadMD 2.4.exe"))),
            Some(&json!("ReadMD 2.4.exe")),
            Some(&json!(digest)),
            false,
        );
        assert_eq!(error, "更新文件名无效");

        // ... and the download lane now reaches `_safe_update_target` instead of
        // dying at the filename gate.
        let job = begin_download(
            Some(&json!(url)),
            Some(&json!("ReadMD-portable-9.9.9.exe")),
            Some(&json!(crate::crypto::sha256_hex(b"x"))),
            false,
        )
        .expect("the official hyphenated package must be accepted");
        assert_eq!(
            job.save_path.file_name().unwrap().to_string_lossy(),
            "ReadMD-portable-9.9.9.exe"
        );
        let _ = std::fs::remove_file(&job.part_path);
        let _ = std::fs::remove_file(&job.save_path);
    }

    /// `updater.py:434-435` hands `data.get('assets', [])` to `match_release_asset`,
    /// which **iterates** it (`updater.py:135`) inside the `try` at `updater.py:430`.
    /// Any value that is not a list of mappings therefore raises, and
    /// `updater.py:464-466` answers with exactly two keys, served 200.  Absorbing
    /// those shapes into an empty `Vec` made the kernel report a *successful*
    /// update check for a malformed release document.
    ///
    /// Known remaining gap, deliberately not asserted here: a mapping whose `name`
    /// is not a string (`[{"name": 5}]`, `[{"name": null}]`) also raises in CPython
    /// at `name.upper()` (`updater.py:137`), but `match_release_asset` absorbs it in
    /// both copies, so it still answers `ok: true`.
    #[test]
    fn check_reports_update_response_invalid_for_uniterable_assets() {
        let _lane = lock_update_lane();
        let tail = |doc: Value| {
            let mut calls = 0usize;
            let payload = release_check_payload("2.4.0", Some(&doc), "", &mut |_| {
                calls += 1;
                None
            });
            assert_eq!(calls, 0, "the error path must not reach the manifest: {doc}");
            payload
        };

        for shape in [
            json!(null),
            json!(5),
            json!(true),
            json!("ab"),
            json!({ "a": 1 }),
            json!([5]),
            json!(["x"]),
            json!([true]),
        ] {
            assert_eq!(
                tail(json!({ "tag_name": "2.5.0", "assets": shape.clone() })),
                json!({ "ok": false, "error_code": "update_response_invalid" }),
                "`assets: {shape}` raises in CPython, so only these two keys may answer",
            );
        }

        // Empty containers iterate to nothing: neither `for` loop runs, so Python
        // stays on the success path with `asset` and `sha_url` null.
        for shape in [json!([]), json!({}), json!("")] {
            let payload = tail(json!({ "tag_name": "2.5.0", "assets": shape.clone() }));
            assert_eq!(payload["ok"], json!(true), "`assets: {shape}` iterates to nothing");
            assert_eq!(payload["asset"], Value::Null);
            assert_eq!(payload["sha_url"], Value::Null);
        }

        // An absent key takes the `[]` default (`data.get('assets', [])`).
        let payload = tail(json!({ "tag_name": "2.5.0" }));
        assert_eq!(payload["ok"], json!(true));
        assert_eq!(payload["latest_version"], json!("2.5.0"));
    }

    /// `updater.py:431` and `:451` — `latest_tag = data.get('tag_name', '')` and
    /// `'release_name': data.get('name') or latest_tag` are *raw* echoes of the
    /// release document.  Python keeps the three shapes apart (absent key → `''`,
    /// explicit `null` → `None`, a number stays a number) while
    /// `is_newer_version` coerces with `str(value or '')` (`versioning.py:14`);
    /// collapsing all of them to `""` invented a version out of a malformed
    /// document and made `"latest_version": null` unreachable.
    #[test]
    fn check_echoes_the_release_document_without_inventing_values() {
        let _lane = lock_update_lane();
        let check = |doc: Value| {
            let mut calls = 0usize;
            let payload = release_check_payload("2.4.0", Some(&doc), "", &mut |_| {
                calls += 1;
                None
            });
            assert_eq!(calls, 0, "no assets, no manifest request: {doc}");
            payload
        };

        // Absent `tag_name` → `''` (`data.get('tag_name', '')`).
        let payload = check(json!({}));
        assert_eq!(payload["ok"], json!(true));
        assert_eq!(payload["latest_version"], json!(""));
        assert_eq!(payload["release_name"], json!(""));
        assert_eq!(payload["has_update"], json!(false));
        assert_eq!(payload["asset"], Value::Null);
        assert_eq!(payload["sha_url"], Value::Null);
        assert_eq!(payload["published_at"], json!(""));
        assert_eq!(payload["release_notes"], json!(""));
        assert_eq!(payload["html_url"], json!(""));

        // Present-but-`null` → `None`, and the `or` fallback hands it straight to
        // `release_name`.
        let payload = check(json!({ "tag_name": null }));
        assert_eq!(payload["latest_version"], Value::Null);
        assert_eq!(payload["release_name"], Value::Null);
        assert_eq!(payload["has_update"], json!(false));

        // An empty `name` is falsy, so the tag is echoed (`updater.py:451`).
        let payload = check(json!({ "tag_name": "2.5.0", "name": "" }));
        assert_eq!(payload["release_name"], json!("2.5.0"));
        let payload = check(json!({ "tag_name": "2.5.0", "name": "ReadMD 2.5.0" }));
        assert_eq!(payload["release_name"], json!("ReadMD 2.5.0"));

        // Non-string tags: `0` is falsy so the comparison sees `''`, `9` is
        // `str(9) == '9'`; both are echoed exactly as the document carried them.
        let payload = check(json!({ "tag_name": 0 }));
        assert_eq!(payload["latest_version"], json!(0));
        assert_eq!(payload["has_update"], json!(false));
        let payload = check(json!({ "tag_name": 9 }));
        assert_eq!(payload["latest_version"], json!(9));
        assert_eq!(payload["release_name"], json!(9));
        assert_eq!(payload["has_update"], json!(true));
        let payload = check(json!({ "tag_name": true }));
        assert_eq!(payload["latest_version"], json!(true));
        assert_eq!(payload["has_update"], json!(false));
    }

    /// ITEM 4 — the check must never advertise a downgrade, an equal version, or a
    /// version it cannot parse.  `has_update` is exactly
    /// `compare_versions(latest, current) == 1` (`updater.py:95-97`,
    /// `versioning.py:28-34`) over the grammar at `versioning.py:6-25`, and the
    /// channel rule (`/latest` only for formal builds) is `updater.py:100-104`.
    #[test]
    fn check_has_update_follows_pythons_version_table() {
        let _lane = lock_update_lane();
        let current = "2.4.0";
        let cases: &[(&str, bool)] = &[
            ("2.4.0", false),  // equal is not newer: `compare_versions` -> 0
            ("v2.4.0", false), // the optional `v`/`V` prefix carries no weight
            ("V2.4.0", false),
            ("2.4", false),           // missing components default to 0
            ("2.3.9", false),         // an older release is never an update
            ("2.3.100", false),       // numeric, not lexicographic
            ("2.4.0+build.9", false), // build metadata is ignored by the grammar
            ("2.4.0-rc.1", false),    // rank 0: a prerelease sorts below its own GA
            ("2.4.0-alpha", false),
            ("2.4.0-1", false),       // numeric identifiers sort below words, and
            ("2.4.0-beta.11", false), // both sort below the empty prerelease
            ("2.4.1", true),
            ("v2.5.0", true),
            ("3", true),
            ("10.0.0", true),
            ("2.4.1-beta.1", true), // a newer core outranks its own GA
            ("2.5.0-rc.1", true),
            // Unparseable tags are `compare_versions`' `None`, i.e. never newer.
            ("", false),
            ("not-a-version", false),
            ("2.4.0.1", false),
            ("v", false),
            ("2.x.0", false),
            ("  ", false),
        ];
        for (tag, expected) in cases {
            let payload = check_payload_of(current, tag);
            assert_eq!(
                payload["has_update"],
                json!(*expected),
                "is_newer_version({tag:?}, {current:?})"
            );
            assert_eq!(payload["latest_version"], json!(*tag));
            assert_eq!(payload["ok"], json!(true));
        }

        // `updater._release_check_urls` (`updater.py:100-104`): `/latest` hides
        // prereleases, so only a prerelease build scans the release list.
        assert_eq!(
            release_check_urls("2.4.0"),
            vec![format!(
                "https://api.github.com/repos/{GITHUB_REPO}/releases/latest"
            )]
        );
        assert_eq!(
            release_check_urls("2.5.0-beta.1"),
            vec![format!(
                "https://api.github.com/repos/{GITHUB_REPO}/releases?per_page=100"
            )]
        );
    }

    /// Convenience for the table above: a release document that carries only a tag,
    /// so the answer depends on the comparison and on nothing else.
    fn check_payload_of(current_version: &str, tag: &str) -> Value {
        let mut never =
            |url: &str| -> Option<String> { panic!("no manifest may be fetched for {url}") };
        release_check_payload(
            current_version,
            Some(&json!({ "tag_name": tag })),
            "",
            &mut never,
        )
    }

    /// ITEM 1's last half — `updater.resolve_expected_sha` opens with
    /// `if not sha_url or not asset_name: return None` (`updater.py:267-268`),
    /// *before* any request.  Fetching anyway both costs a socket call Python does
    /// not make and lets a meaningless URL decide the answer.
    #[test]
    fn check_never_fetches_a_manifest_it_cannot_name() {
        let _lane = lock_update_lane();
        let asset = asset_for_flavor(&detect_app_flavor());
        let package = json!({
            "name": asset,
            "size": 1,
            "browser_download_url": official_asset_url(&asset),
        });
        let documents = [
            // No `SHA256SUMS.txt` asset at all (`sha_asset` is `None`).
            json!({ "tag_name": "9.9.9", "assets": [package.clone()] }),
            // The manifest asset exists but carries no download URL.
            json!({
                "tag_name": "9.9.9",
                "assets": [package.clone(), { "name": "SHA256SUMS.txt", "size": 2 }],
            }),
            // ... or carries an empty one (`if not sha_url`).
            json!({
                "tag_name": "9.9.9",
                "assets": [
                    package.clone(),
                    {
                        "name": "SHA256SUMS.txt",
                        "size": 2,
                        "browser_download_url": "",
                    },
                ],
            }),
            // ... or the selected asset has no name (`if not asset_name`).
            json!({
                "tag_name": "9.9.9",
                "assets": [
                    { "size": 1, "browser_download_url": official_asset_url(&asset) },
                    {
                        "name": "SHA256SUMS.txt",
                        "size": 2,
                        "browser_download_url": official_asset_url("SHA256SUMS.txt"),
                    },
                ],
            }),
        ];
        for document in documents {
            let mut calls = 0usize;
            let payload = release_check_payload("1.0.0", Some(&document), "", &mut |_| {
                calls += 1;
                Some(format!("{digest}  {asset}\n", digest = "a".repeat(64)))
            });
            assert_eq!(calls, 0, "Python returns None before fetching: {document}");
            assert_eq!(payload["asset"]["expected_sha"], Value::Null);
        }

        // Positive control: with both names present the manifest *is* read, once,
        // and its digest lands on the wire (`updater.py:439-444`).
        let document = json!({
            "tag_name": "9.9.9",
            "assets": [
                package.clone(),
                {
                    "name": "SHA256SUMS.txt",
                    "size": 2,
                    "browser_download_url": official_asset_url("SHA256SUMS.txt"),
                },
            ],
        });
        let mut seen: Vec<String> = Vec::new();
        let digest = crate::crypto::sha256_hex(b"the-released-bytes");
        let payload = release_check_payload("1.0.0", Some(&document), "", &mut |url| {
            seen.push(url.to_string());
            Some(format!("{digest}  {asset}\n"))
        });
        assert_eq!(seen, vec![official_asset_url("SHA256SUMS.txt")]);
        assert_eq!(payload["asset"]["expected_sha"], json!(digest));

        // A digest the asset already carries short-circuits the manifest
        // (`updater.py:437-438`).
        let carried = json!({
            "tag_name": "9.9.9",
            "assets": [
                {
                    "name": asset,
                    "size": 1,
                    "browser_download_url": official_asset_url(&asset),
                    "expected_sha": digest,
                },
                {
                    "name": "SHA256SUMS.txt",
                    "size": 2,
                    "browser_download_url": official_asset_url("SHA256SUMS.txt"),
                },
            ],
        });
        let mut calls = 0usize;
        let payload = release_check_payload("1.0.0", Some(&carried), "", &mut |_| {
            calls += 1;
            None
        });
        assert_eq!(calls, 0);
        assert_eq!(payload["asset"]["expected_sha"], json!(digest));
    }

    /// ITEM 2's closing half — the apply gate compares digests **byte-exactly**:
    /// `expected_sha = str(snapshot.get('expected_sha') or '').lower()`
    /// (`updater.py:768`) lowercases but never strips, and
    /// `actual_sha != expected_sha or snapshot.get('verified_sha') != expected_sha`
    /// (`updater.py:778`) tests the stored `verified_sha` raw.  The port stripped
    /// and case-folded both, so a padded or upper-cased digest — a state the
    /// download lane never writes — was repaired into a match instead of refused.
    #[test]
    fn apply_compares_the_digest_byte_exactly() {
        let _lane = lock_update_lane();
        let app = test_app("update-apply-digest");
        let dir = temp_update_dir();
        std::fs::create_dir_all(&dir).unwrap();
        let package = dir.join(unique_package_name("digest"));
        let bytes = b"byte-exact-package".to_vec();
        std::fs::write(&package, &bytes).unwrap();
        let digest = crate::crypto::sha256_hex(&bytes);
        let base = DownloadState {
            status: "ready".to_string(),
            target_file: package.to_string_lossy().to_string(),
            asset_name: package.file_name().unwrap().to_string_lossy().to_string(),
            expected_sha: digest.clone(),
            verified_sha: digest.clone(),
            percent: 100,
            ..DownloadState::default()
        };
        let file_path = json!({ "file_path": package.to_string_lossy() });
        let post = |body: Value| {
            render(h_update_apply(
                &app,
                &json_request("POST", "/api/update/apply", &body, None),
            ))
        };

        // Control: these bytes *do* verify, and the only reason apply still says no
        // is that this kernel may not start an installer.
        set_download_state(base.clone());
        let (status, body, _) = post(file_path.clone());
        assert_eq!(status, 400);
        assert_eq!(body["message"], "当前平台暂不支持自动替换，请手动解压运行");

        // A whitespace-padded expectation is not the digest `compute_file_sha256`
        // answers with.
        set_download_state(DownloadState {
            expected_sha: format!(" {digest} "),
            ..base.clone()
        });
        let (status, body, _) = post(file_path.clone());
        assert_eq!(status, 400);
        assert_eq!(body["message"], "更新包校验失败，已拒绝安装");
        set_download_state(DownloadState {
            expected_sha: format!("{digest}\n"),
            ..base.clone()
        });
        let (_, body, _) = post(file_path.clone());
        assert_eq!(body["message"], "更新包校验失败，已拒绝安装");

        // An upper-cased `verified_sha` is not the string that wrote the lowercase
        // one, so `:778`'s second half refuses.
        set_download_state(DownloadState {
            verified_sha: digest.to_uppercase(),
            ..base.clone()
        });
        let (status, body, _) = post(file_path.clone());
        assert_eq!(status, 400);
        assert_eq!(body["message"], "更新包校验失败，已拒绝安装");

        // The lowercase digest written by `compute_file_sha256` is the only spelling
        // that passes.
        set_download_state(base);
        let (_, body, _) = post(file_path);
        assert_eq!(body["message"], "当前平台暂不支持自动替换，请手动解压运行");

        let _ = std::fs::remove_file(&package);
    }

    // ====================================================================
    // /api/convert route family — parity pins for readmd.py
    // ====================================================================

    /// `convert::is_win7()` reads `READMD_FORCE_WIN7` on every call, so every
    /// convert test holds this lock for its whole call graph — the same
    /// convention `server.rs` uses for its dispatching tests.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn env_guard() -> std::sync::MutexGuard<'static, ()> {
        ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// RAII override for the legacy-Windows gate: an assertion that fails must
    /// not leak the flag into the next test's handler call.
    struct Win7Flag(bool);

    impl Drop for Win7Flag {
        fn drop(&mut self) {
            if self.0 {
                std::env::set_var("READMD_FORCE_WIN7", "1");
            } else {
                std::env::remove_var("READMD_FORCE_WIN7");
            }
        }
    }

    fn force_win7(on: bool) -> Win7Flag {
        let previous = std::env::var("READMD_FORCE_WIN7")
            .map(|v| v == "1")
            .unwrap_or(false);
        if on {
            std::env::set_var("READMD_FORCE_WIN7", "1");
        } else {
            std::env::remove_var("READMD_FORCE_WIN7");
        }
        Win7Flag(previous)
    }

    fn scratch_dir(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "readmd-convert-{tag}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_file(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
        let target = dir.join(name);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&target, bytes).unwrap();
        target
    }

    /// `readmd.py:2052-2080` — every answer `_api_transcribe` can give without a
    /// working whisper install, in Python's gate order, with `_send_api_error`'s
    /// `{'ok': False, 'error_code': …}` key shape throughout.
    #[test]
    fn transcribe_follows_pythons_gates_and_keys() {
        let app = test_app("transcribe");
        let dir = app.paths.data_dir.join("media");
        std::fs::create_dir_all(&dir).unwrap();
        let audio = dir.join("voice.mp3");
        std::fs::write(&audio, b"not really audio").unwrap();
        let text = dir.join("notes.txt");
        std::fs::write(&text, b"plain text").unwrap();
        let audio_path = audio.to_string_lossy().to_string();
        let text_path = text.to_string_lossy().to_string();

        // 1. The method gate comes before the body (`readmd.py:2053-2055`).
        let req = json_request("GET", "/api/transcribe", &json!({"path": audio_path}), None);
        let (status, body, _) = render(h_transcribe(&app, &req));
        assert_eq!(status, 405);
        assert_eq!(body, json!({"ok": false, "error_code": "method_not_allowed"}));

        // 2. No `path` at all is 404 `file_not_found`, not the invented
        //    400 `{'ok':false,'error':'path required'}`.
        let req = json_request("POST", "/api/transcribe", &json!({}), None);
        let (status, body, _) = render(h_transcribe(&app, &req));
        assert_eq!(
            (status, body.get("error_code").and_then(|v| v.as_str())),
            (404, Some("file_not_found"))
        );

        // 3. A *directory* used to pass `Path::exists()`; `os.path.isfile` does not.
        let dir_path = dir.to_string_lossy().to_string();
        let req = json_request("POST", "/api/transcribe", &json!({"path": dir_path}), None);
        let (status, body, _) = render(h_transcribe(&app, &req));
        assert_eq!(
            (status, body.get("error_code").and_then(|v| v.as_str())),
            (404, Some("file_not_found"))
        );

        // 4. The media-type gate (`readmd.py:2065-2067`).
        let req = json_request("POST", "/api/transcribe", &json!({"path": text_path}), None);
        let (status, body, _) = render(h_transcribe(&app, &req));
        assert_eq!(
            (status, body.get("error_code").and_then(|v| v.as_str())),
            (400, Some("unsupported_media_format"))
        );

        // 5. No whisper/ffmpeg: Python's 200 with the install notice as `content`
        //    and the reason surfaced as `warning` — never dropped, never a 422.
        let req = json_request("POST", "/api/transcribe", &json!({"path": audio_path}), None);
        let (status, body, _) = render(h_transcribe(&app, &req));
        assert_eq!(status, 200, "{body}");
        assert_eq!(body.as_object().map(|m| m.len()), Some(4));
        assert_eq!(body["ok"], json!(true));
        assert_eq!(body["path"], json!(audio_path));
        assert!(!body["content"].as_str().unwrap_or("").is_empty());
        assert_eq!(
            body["warning"].as_str(),
            Some("Whisper plugin or FFmpeg not available.")
        );

        // 6. `_read_json_body`'s `ValueError` is a 400 whose `error_code` is
        //    `str(exc)` — CPython's message, not a shortened Rust coinage.
        let req = raw_request("POST", "/api/transcribe", b"not json", "8");
        let (status, body, _) = render(h_transcribe(&app, &req));
        assert_eq!(status, 400);
        assert_eq!(
            body["error_code"].as_str(),
            Some("Expecting value: line 1 column 1 (char 0)")
        );

        // An oversize *declaration* is gated before a single body byte is parsed.
        let req = raw_request("POST", "/api/transcribe", b"{}", "2000000");
        let (status, body, _) = render(h_transcribe(&app, &req));
        assert_eq!(
            (status, body["error_code"].as_str()),
            (400, Some("request_too_large"))
        );
    }

    /// `readmd.py:1879-1940` — the diagram ladder's offline rungs: what each engine
    /// answers, and the consent gate in front of the only lane that touches the net.
    #[test]
    fn diagram_render_answers_pythons_ladder() {
        let app = test_app("diagram");

        // A client-side engine must never be echoed back as a successful render.
        for engine in ["mermaid", "katex", "mathjax", ""] {
            let req = json_request(
                "POST",
                "/api/diagram/render",
                &json!({"engine": engine, "code": "graph TD; a-->b;"}),
                None,
            );
            let (status, body, _) = render(h_diagram_render(&app, &req));
            assert_eq!(status, 422, "engine {engine:?}");
            assert_eq!(body["ok"], json!(false));
            assert_eq!(
                body["error_code"].as_str(),
                Some("diagram_client_renderer_required")
            );
            // `str(body.get('engine','mermaid') or 'mermaid')`: `''` falls back.
            assert_eq!(
                body["engine"].as_str(),
                Some(if engine.is_empty() { "mermaid" } else { engine })
            );
        }

        // No pinned, redistributable offline renderer for these three.
        for engine in ["wsd", "d2", "ditaa"] {
            let req = json_request(
                "POST",
                "/api/diagram/render",
                &json!({"engine": engine, "code": "x"}),
                None,
            );
            let (status, body, _) = render(h_diagram_render(&app, &req));
            assert_eq!(status, 422);
            assert_eq!(
                body["error_code"].as_str(),
                Some("diagram_engine_unavailable")
            );
            assert_eq!(body["engine"].as_str(), Some(engine));
        }

        // TikZ is pure text wrapping: `{'ok','type','html'}`, exactly three keys.
        let req = json_request(
            "POST",
            "/api/diagram/render",
            &json!({"engine": "TikZ ", "code": "\\node{a};"}),
            None,
        );
        let (status, body, _) = render(h_diagram_render(&app, &req));
        assert_eq!(status, 200, "{body}");
        assert_eq!(body.as_object().map(|m| m.len()), Some(3));
        assert_eq!(body["type"].as_str(), Some("html"));
        assert!(body["html"].as_str().unwrap_or("").contains("\\node{a};"));

        // A declared body that is not JSON escapes the route's `try` as 500.
        let req = raw_request("POST", "/api/diagram/render", b"{oops", "5");
        let (status, body, _) = render(h_diagram_render(&app, &req));
        assert_eq!(
            (status, body["error_code"].as_str()),
            (500, Some("diagram_render_failed"))
        );

        // PlantUML consent: only the literal `true` may upload diagram source.
        // Skipped where a local Java/PlantUML runtime really is installed, since
        // Python takes the offline branch first.
        if !crate::diagrams::has_local_plantuml() {
            for allow in [Value::Null, json!(1), json!("true"), json!(false)] {
                let req = json_request(
                    "POST",
                    "/api/diagram/render",
                    &json!({"engine": "plantuml", "code": "A -> B", "allow_remote": allow}),
                    None,
                );
                let (status, body, _) = render(h_diagram_render(&app, &req));
                assert_eq!(status, 422, "allow_remote {allow} must not reach the network");
                assert_eq!(
                    body["error_code"].as_str(),
                    Some("diagram_dependency_missing")
                );
                assert_eq!(body["remote_available"].as_bool(), Some(true));
                assert_eq!(body["requires_confirmation"].as_bool(), Some(true));
            }
            // The key absent is the same answer (`body.get(...) is True`).
            let req = json_request(
                "POST",
                "/api/diagram/render",
                &json!({"engine": "puml", "code": "A -> B"}),
                None,
            );
            let (status, body, _) = render(h_diagram_render(&app, &req));
            assert_eq!(status, 422);
            assert_eq!(
                body["error_code"].as_str(),
                Some("diagram_dependency_missing")
            );
        }
    }

    fn test_app(tag: &str) -> Arc<App> {
        let dir = scratch_dir(tag);
        std::fs::create_dir_all(dir.join("assets")).unwrap();
        let paths =
            crate::paths::AppPaths::with_dirs(&dir.join("data"), &dir, &dir.join("assets"));
        Arc::new(App::bootstrap(paths).unwrap())
    }

    /// `server.rs::parse_query` is private, so the map is built by hand the same
    /// way it builds one: percent-decoded values, blank values dropped like
    /// `parse_qs()` with `keep_blank_values=False`.  Handlers that mirror a
    /// second Python `unquote()` apply it themselves.
    fn api_request(method: &str, path: &str, query: &[(&str, &str)], body: &[u8]) -> Request {
        Request {
            method: method.to_string(),
            path: path.to_string(),
            query: query
                .iter()
                .filter(|(_, v)| !v.is_empty())
                .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                .collect(),
            headers: HashMap::new(),
            body: body.to_vec(),
        }
    }

    /// A POST with an explicit (or honest) `Content-Length`, which both
    /// `/api/convert/batch` and `/api/convert/cancel` gate on before the body.
    fn json_request(method: &str, path: &str, body: &Value, content_length: Option<&str>) -> Request {
        let raw = serde_json::to_vec(body).unwrap();
        let mut req = api_request(method, path, &[], &raw);
        req.headers.insert(
            "content-length".to_string(),
            content_length
                .map(|v| v.to_string())
                .unwrap_or_else(|| raw.len().to_string()),
        );
        req
    }

    fn raw_request(method: &str, path: &str, raw: &[u8], content_length: &str) -> Request {
        let mut req = api_request(method, path, &[], raw);
        req
            .headers
            .insert("content-length".to_string(), content_length.to_string());
        req
    }

    fn json_body(res: &Response) -> Value {
        serde_json::from_slice(&res.body).expect("handler answered with valid JSON")
    }

    /// Uniform `(status, body, was-plain-text)` view of a handler answer, for
    /// routes where Python lets the exception escape `_route()` and `do_POST`
    /// replies `internal error`.
    fn render(result: ApiResult<Response>) -> (u16, Value, bool) {
        match result {
            Ok(res) => (res.status, json_body(&res), false),
            Err(err) => (err.status, err.payload(), err.is_plain_text()),
        }
    }

    fn key_set(body: &Value) -> Vec<&str> {
        let mut keys: Vec<&str> = body
            .as_object()
            .map(|m| m.keys().map(|k| k.as_str()).collect())
            .unwrap_or_default();
        keys.sort();
        keys
    }

    /// Direct registry access for the two job-state routes: a synthetic job is
    /// far more honest than a live worker thread when the test is about the
    /// JSON key set.
    fn register_job(job: ConvertBatchJob) {
        let mut guard = CONVERT_JOBS.lock().unwrap_or_else(|e| e.into_inner());
        guard
            .get_or_insert_with(HashMap::new)
            .insert(job.id.clone(), job);
    }

    fn unregister_job(id: &str) {
        let mut guard = CONVERT_JOBS.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(map) = guard.as_mut() {
            map.remove(id);
        }
    }

    fn job_snapshot(id: &str) -> Option<ConvertBatchJob> {
        let guard = CONVERT_JOBS.lock().unwrap_or_else(|e| e.into_inner());
        guard.as_ref().and_then(|m| m.get(id)).cloned()
    }

    fn convert_call(app: &Arc<App>, p: Option<&str>) -> (u16, Value, bool) {
        let query: Vec<(&str, &str)> = match p {
            Some(v) => vec![("p", v)],
            None => vec![],
        };
        render(h_convert(
            app,
            &api_request("POST", "/api/convert", &query, &[]),
        ))
    }

    /// `readmd.py:3138` `_send_api_error(404, 'file_not_found')` — the body is
    /// exactly `{'ok': False, 'error_code': ...}`: `_send_api_error` never adds
    /// an `error` string, "the UI owns wording through i18n".
    #[test]
    fn convert_answers_file_not_found_for_every_unusable_p() {
        let _env = env_guard();
        let app = test_app("convert-404");
        let expect = json!({"ok": false, "error_code": "file_not_found"});

        // The route reads the query string only: a body-carried `p` is ignored
        // (`readmd.py:1213-1215` passes `qs.get('p', [''])[0]`), so this is the
        // same 404 as no path at all.
        let body = serde_json::to_vec(&json!({"p": "C:/does/not/exist.docx"})).unwrap();
        let (status, payload, _) = render(h_convert(
            &app,
            &api_request("POST", "/api/convert", &[], &body),
        ));
        assert_eq!(status, 404, "{payload}");
        assert_eq!(payload, expect);

        let check = |query: &[(&str, &str)]| {
            let (status, payload, _) = render(h_convert(
                &app,
                &api_request("POST", "/api/convert", query, &[]),
            ));
            assert_eq!(status, 404, "{query:?} -> {payload}");
            assert_eq!(payload, expect);
            assert_eq!(key_set(&payload), vec!["error_code", "ok"]);
        };
        check(&[]); // no `p`
        check(&[("p", "")]); // blank `p` never reaches the map (parse_qs)
        check(&[("p", "C:/nope/gone.docx")]); // absent file
        let a_dir = scratch_dir("convert-404-dir");
        check(&[("p", a_dir.to_str().unwrap())]); // a directory is not a file
    }

    /// `readmd.py:3152-3153` — `if err and not text:` answers 422
    /// `conversion_failed` plus the `engine` keyword (`engine or ''`).
    #[test]
    fn convert_reports_conversion_failed_with_the_engine_key() {
        let _env = env_guard();
        let app = test_app("convert-422");
        // NUL-bearing bytes: `_looks_binary()` in both authorities refuses them.
        let file = write_file(
            &scratch_dir("convert-422"),
            "pkg.zip",
            &[0x50, 0x4b, 0x03, 0x04, 0x00, 0x00, 0x00, 0x00, b'x'],
        );
        let (status, payload, _) = convert_call(&app, Some(file.to_str().unwrap()));
        assert_eq!(status, 422, "{payload}");
        assert_eq!(
            payload,
            json!({"ok": false, "error_code": "conversion_failed", "engine": ""})
        );
        assert_eq!(key_set(&payload), vec!["engine", "error_code", "ok"]);
    }

    /// `readmd.py:3141` — the legacy-Windows gate runs after `isfile()` and
    /// before any converter work, and `WIN7_CONVERT_EXTS = ('.docx', '.pdf')`.
    #[test]
    fn convert_gates_legacy_windows_before_the_converter() {
        let _env = env_guard();
        let app = test_app("convert-win7");
        let dir = scratch_dir("convert-win7");
        let zip = write_file(&dir, "pkg.zip", b"PK\x03\x04\x00\x00\x00\x00");
        let docx = write_file(&dir, "letter.docx", b"not really a zip");
        let _flag = force_win7(true);

        let (status, payload, _) = convert_call(&app, Some(zip.to_str().unwrap()));
        assert_eq!(status, 415, "{payload}");
        assert_eq!(
            payload,
            json!({"ok": false, "error_code": "unsupported_on_legacy_windows"})
        );

        // `.docx` survives the gate, so the route falls through to the
        // converter instead of refusing on capability.
        let (status, payload, _) = convert_call(&app, Some(docx.to_str().unwrap()));
        assert_ne!(status, 415, "{payload}");

        // `/api/convert/collect` narrows to the same two extensions (3131).
        let (status, payload, _) = render(h_convert_collect(
            &app,
            &api_request(
                "GET",
                "/api/convert/collect",
                &[("dir", dir.to_str().unwrap())],
                &[],
            ),
        ));
        assert_eq!(status, 200, "{payload}");
        let files: Vec<&str> = payload["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(files, vec![docx.to_str().unwrap()]);
    }

    /// `readmd.py:3144` routes `.txt` to `_convert_txt`, whose engine label is
    /// `'txt 智能识别'` when TXT intelligence changed the text and `'TXT'`
    /// otherwise (readmd.py:3213) — never the converter's `'txtmd'`.
    /// Item 1 also lives here: the autosaved bytes must be the mdcheck-fixed
    /// text the route returns as `content`.
    #[test]
    fn txt_documents_take_the_txt_intelligence_lane() {
        let _env = env_guard();
        let app = test_app("convert-txt");
        let dir = scratch_dir("convert-txt");
        let plain = write_file(&dir, "plain.txt", b"just one line of prose\n");
        let (status, payload, _) = convert_call(&app, Some(plain.to_str().unwrap()));
        assert_eq!(status, 200, "{payload}");
        assert_eq!(
            key_set(&payload),
            vec![
                "content", "dir", "engine", "fixes", "name", "out", "path", "saved", "skipped",
                "source", "warns"
            ]
        );
        let engine = payload["engine"].as_str().unwrap_or_default();
        assert!(
            engine == "TXT" || engine == "txt 智能识别",
            "unexpected engine label {engine:?}"
        );
        assert_eq!(payload["source"], "convert");
        assert_eq!(payload["name"], "plain.txt");
        assert_eq!(payload["dir"], dir.to_str().unwrap());
        assert_eq!(payload["path"], plain.to_str().unwrap());

        // A second call now finds the `.md` and reports `skipped` (3167-3168).
        let out = payload["out"].as_str().unwrap().to_string();
        assert_eq!(Path::new(&out).extension().and_then(|e| e.to_str()), Some("md"));
        let saved = std::fs::read_to_string(&out).unwrap();
        assert_eq!(saved, payload["content"].as_str().unwrap());
        assert!(!saved.contains('\r'), "_write_md uses newline='\\n'");
        let (status, second, _) = convert_call(&app, Some(plain.to_str().unwrap()));
        assert_eq!(status, 200, "{second}");
        assert_eq!(second["skipped"], json!(true));
        assert_eq!(second["saved"], json!(false));
        // `overwrite=1` re-enables the autosave (readmd.py:3200).
        let (status, third, _) = render(h_convert(
            &app,
            &api_request(
                "POST",
                "/api/convert",
                &[("p", plain.to_str().unwrap()), ("overwrite", "1")],
                &[],
            ),
        ));
        assert_eq!(status, 200, "{third}");
        assert_eq!(third["saved"], json!(true));
        assert_eq!(third["skipped"], json!(false));
    }

    /// `readmd.py:3191-3195` — an empty TXT is a 200 with its own note, and the
    /// engine there is `'txt 智能识别'` regardless of `tstats['changed']`.
    #[test]
    fn empty_txt_answers_the_python_empty_note_payload() {
        let _env = env_guard();
        let app = test_app("convert-empty-txt");
        let file = write_file(&scratch_dir("convert-empty-txt"), "empty.txt", b"");
        let (status, payload, _) = convert_call(&app, Some(file.to_str().unwrap()));
        assert_eq!(status, 200, "{payload}");
        assert_eq!(
            key_set(&payload),
            vec!["content", "dir", "engine", "name", "note", "source"]
        );
        assert_eq!(
            payload,
            json!({
                "content": "",
                "name": "empty.txt",
                "dir": file.parent().unwrap().to_str().unwrap(),
                "source": "convert",
                "engine": "txt 智能识别",
                "note": "文件为空，没有可转换的内容"
            })
        );
    }

    /// `readmd.py:1214` applies `unquote()` on top of the value `parse_qs` has
    /// already decoded, so exactly one extra decode happens — no more, no less.
    #[test]
    fn convert_decodes_the_query_path_exactly_one_time_more() {
        let _env = env_guard();
        let app = test_app("convert-unquote");
        let dir = scratch_dir("convert-unquote");
        let file = write_file(&dir, "a%20b.txt", b"percent in the name\n");
        let literal = file.to_str().unwrap();
        let dir_part = literal[..literal.len() - "a%20b.txt".len()].to_string();

        // What `parse_qs` hands the handler for a client that sent
        // `...a%2520b.txt`: one more decode finds the file.
        let (status, payload, _) = convert_call(&app, Some(&format!("{dir_part}a%2520b.txt")));
        assert_eq!(status, 200, "{payload}");
        assert_eq!(payload["path"], json!(format!("{dir_part}a%20b.txt")));
        assert_eq!(payload["name"], "a%20b.txt");

        // The same value with one decode *less* of the client's would be a
        // different file: `a%20b.txt` decodes to `a b.txt`, which does not
        // exist — Python's second `unquote()` 404s here too.
        let (status, payload, _) = convert_call(&app, Some(literal));
        assert_eq!(status, 404, "{payload}");
        assert_eq!(payload["error_code"], "file_not_found");
    }

    /// `readmd.py:3329-3340` — unknown job answers 404 with the legacy `error`
    /// wording, and the found-job body is exactly those six keys, where the
    /// counter key is `done` (not `completed`/`progress`).
    #[test]
    fn progress_pins_the_not_found_shape_and_the_six_job_keys() {
        let _env = env_guard();
        let app = test_app("progress");
        let unknown = "c-not-there";
        let (status, payload, _) = render(h_convert_progress(
            &app,
            &api_request(
                "GET",
                "/api/convert/progress",
                &[("job", unknown)],
                &[],
            ),
        ));
        assert_eq!(status, 404, "{payload}");
        assert_eq!(
            payload,
            json!({"ok": false, "error_code": "job_not_found", "error": "任务不存在"})
        );
        assert_eq!(key_set(&payload), vec!["error", "error_code", "ok"]);

        let jid = "c900000001".to_string();
        let items = vec![
            ConvertJobItem {
                src: "C:/tmp/one.docx".to_string(),
                planned_out: Some("C:/tmp/one.md".to_string()),
                status: "ok".to_string(),
                out: Some("C:/tmp/one.md".to_string()),
                engine: Some("docx".to_string()),
                warns: Some(vec![]),
                error: None,
                error_code: None,
                done: true,
            },
            ConvertJobItem {
                src: "C:/tmp/two.docx".to_string(),
                planned_out: Some("C:/tmp/two.md".to_string()),
                status: "queued".to_string(),
                out: None,
                engine: None,
                warns: None,
                error: None,
                error_code: None,
                done: false,
            },
        ];
        register_job(ConvertBatchJob {
            id: jid.clone(),
            overwrite: false,
            running: true,
            finished: false,
            cancel: false,
            items,
        });

        let (status, payload, _) = render(h_convert_progress(
            &app,
            &api_request("GET", "/api/convert/progress", &[("job", &jid)], &[]),
        ));
        assert_eq!(status, 200, "{payload}");
        assert_eq!(
            key_set(&payload),
            vec!["done", "finished", "items", "job", "running", "total"]
        );
        assert_eq!(payload["job"], json!(jid));
        assert_eq!(payload["running"], json!(true));
        assert_eq!(payload["finished"], json!(false));
        assert_eq!(payload["done"], json!(1), "sum(1 for it if it['done'])");
        assert_eq!(payload["total"], json!(2), "len(job['items'])");
        let list = payload["items"].as_array().unwrap();
        // Keys Python has not inserted yet must not appear (readmd.py:887-903).
        assert_eq!(key_set(&list[0]), vec!["done", "engine", "out", "planned_out", "src", "status", "warns"]);
        assert_eq!(key_set(&list[1]), vec!["done", "planned_out", "src", "status"]);

        unregister_job(&jid);
    }

    /// `readmd.py:3342-3354` — `Content-Length` is parsed *outside* the `try`,
    /// a found job answers `{'ok': True, 'job': id}` with no invented
    /// `cancelled`, and a bad job id keeps the legacy `error` wording.
    #[test]
    fn cancel_follows_pythons_body_and_answer_rules() {
        let _env = env_guard();
        let app = test_app("cancel");

        // `int('nonsense')` escapes `_route()`; `do_POST` replies in plain text.
        let req = raw_request("POST", "/api/convert/cancel", br#"{"job":"c1"}"#, "nonsense");
        let (status, payload, plain) = render(h_convert_cancel(&app, &req));
        assert_eq!((status, plain), (500, true), "{payload}");
        assert_eq!(payload, json!("internal error"));

        // Malformed JSON is a 400 that *does* carry the legacy `error` key.
        let req = raw_request("POST", "/api/convert/cancel", b"{oops", "5");
        let (status, payload, _) = render(h_convert_cancel(&app, &req));
        assert_eq!(status, 400, "{payload}");
        assert_eq!(
            payload,
            json!({"ok": false, "error_code": "invalid_request", "error": "请求格式错误"})
        );

        // Unknown id, including the empty one Python gets from a bare `{}`.
        for jid in ["c-not-there", ""] {
            let body = json!({"job": jid});
            let (status, payload, _) = render(h_convert_cancel(
                &app,
                &json_request("POST", "/api/convert/cancel", &body, None),
            ));
            assert_eq!(status, 404, "{jid:?} -> {payload}");
            assert_eq!(
                payload,
                json!({"ok": false, "error_code": "job_not_found", "error": "任务不存在"})
            );
        }

        let jid = "c900000002".to_string();
        register_job(ConvertBatchJob {
            id: jid.clone(),
            overwrite: false,
            running: true,
            finished: false,
            cancel: false,
            items: Vec::new(),
        });
        let body = json!({"job": jid});
        let (status, payload, _) = render(h_convert_cancel(
            &app,
            &json_request("POST", "/api/convert/cancel", &body, None),
        ));
        assert_eq!(status, 200, "{payload}");
        assert_eq!(payload, json!({"ok": true, "job": jid}));
        assert_eq!(key_set(&payload), vec!["job", "ok"]);
        assert_eq!(
            job_snapshot(&jid).map(|j| j.cancel),
            Some(true),
            "`job['cancel'] = True`"
        );

        // A JSON array body is *not* a 500: `readmd.py:3349` evaluates
        // `isinstance(body, dict)` before `body.get('job')`, so a non-dict asks
        // for the empty id and lands on the 404.
        let raw = b"[1,2]";
        let (status, payload, plain) = render(h_convert_cancel(
            &app,
            &raw_request("POST", "/api/convert/cancel", raw, "5"),
        ));
        assert_eq!((status, plain), (404, false), "{payload}");
        assert_eq!(
            payload,
            json!({"ok": false, "error_code": "job_not_found", "error": "任务不存在"})
        );

        unregister_job(&jid);
    }

    /// `readmd.py:3114-3135` — always 200, always `{'dir', 'files'}`, `os.walk`
    /// semantics: `sorted(names)` per directory, `.`/`_` directories pruned,
    /// depth `< 4`, and the 200-item ceiling applied twice.
    #[test]
    fn collect_walks_like_os_walk_and_always_answers_dir_and_files() {
        let _env = env_guard();
        let app = test_app("collect");
        let root = scratch_dir("collect");

        write_file(&root, "b.txt", b"x");
        write_file(&root, "A.txt", b"x");
        write_file(&root, "_under.txt", b"x");
        write_file(&root, ".dot.txt", b"x");
        write_file(&root, "guide.docx", b"x");
        write_file(&root, "notes.md", b"x"); // `.md` is not convertible input
        write_file(&root, "sheet.xlsx", b"x");
        write_file(&root, ".git/inside.txt", b"x");
        write_file(&root, "_private/inside.txt", b"x");
        write_file(&root, "keep/deep.txt", b"x");
        write_file(&root, "d1/d2/d3/f3.txt", b"x");
        write_file(&root, "d1/d2/d3/d4/f4.txt", b"x"); // depth 4 is pruned

        let (status, payload, _) = render(h_convert_collect(
            &app,
            &api_request(
                "GET",
                "/api/convert/collect",
                &[("dir", root.to_str().unwrap())],
                &[],
            ),
        ));
        assert_eq!(status, 200, "{payload}");
        assert_eq!(key_set(&payload), vec!["dir", "files"]);
        assert_eq!(payload["dir"], root.to_str().unwrap());
        let files: Vec<&str> = payload["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        let mut expect: Vec<String> = [".dot.txt", "A.txt", "_under.txt", "b.txt", "guide.docx", "sheet.xlsx"]
            .iter()
            .map(|n| root.join(n).to_string_lossy().into_owned())
            .collect();
        expect.push(root.join("keep").join("deep.txt").to_string_lossy().into_owned());
        expect.push(
            root.join("d1")
                .join("d2")
                .join("d3")
                .join("f3.txt")
                .to_string_lossy()
                .into_owned(),
        );
        assert_eq!(files.len(), expect.len(), "{files:?} vs {expect:?}");
        // Same directory order; sibling-directory order is scandir order in both
        // authorities, so compare as sets for those.
        let expect_root: Vec<&str> = expect[..6].iter().map(|s| s.as_str()).collect();
        assert_eq!(
            files[..6],
            expect_root[..],
            "each directory contributes sorted(names)"
        );
        let mut sorted_files = files.clone();
        sorted_files.sort();
        let mut sorted_expect: Vec<&str> = expect.iter().map(|s| s.as_str()).collect();
        sorted_expect.sort();
        assert_eq!(sorted_files, sorted_expect);

        // The cap: `files[:200]` after a directory that overflows.
        let crowded = scratch_dir("collect-crowded");
        for i in 0..250 {
            write_file(&crowded, &format!("f{i:03}.txt"), b"x");
        }
        let (_, payload, _) = render(h_convert_collect(
            &app,
            &api_request(
                "GET",
                "/api/convert/collect",
                &[("dir", crowded.to_str().unwrap())],
                &[],
            ),
        ));
        let files = payload["files"].as_array().unwrap();
        assert_eq!(files.len(), 200);
        assert_eq!(
            files[0].as_str().unwrap(),
            crowded.join("f000.txt").to_string_lossy()
        );
        assert_eq!(
            files[199].as_str().unwrap(),
            crowded.join("f199.txt").to_string_lossy()
        );

        // A missing or non-directory `dir` is still a 200 `{'dir', 'files': []}`.
        for dir in ["", "C:/definitely/not/here"] {
            let (status, payload, _) = render(h_convert_collect(
                &app,
                &api_request("GET", "/api/convert/collect", &[("dir", dir)], &[]),
            ));
            assert_eq!(status, 200, "{payload}");
            assert_eq!(payload, json!({"dir": dir, "files": []}));
        }
    }

    /// `readmd.py:3220-3254`, gates in the source order: Content-Length parse →
    /// 413 size bound → body parse → `confirm is not True` → path filter →
    /// 400 `no_convertible_files`.
    #[test]
    fn batch_gates_run_in_pythons_order() {
        let _env = env_guard();
        let app = test_app("batch-gates");

        // Oversized body wins over the missing `confirm` flag.
        let body = json!({"paths": ["C:/nope/missing.docx"]});
        let req = json_request("POST", "/api/convert/batch", &body, Some("70000"));
        let (status, payload, _) = render(h_convert_batch(&app, &req));
        assert_eq!(status, 413, "{payload}");
        assert_eq!(payload, json!({"ok": false, "error_code": "request_too_large"}));

        // ... and a negative length takes the same branch as `n < 0`.
        let req = json_request("POST", "/api/convert/batch", &body, Some("-5"));
        let (status, payload, _) = render(h_convert_batch(&app, &req));
        assert_eq!(status, 413, "{payload}");
        assert_eq!(payload["error_code"], "request_too_large");

        // `int(headers.get(...))` raising is its own 400 code.
        let req = json_request("POST", "/api/convert/batch", &body, Some("nonsense"));
        let (status, payload, _) = render(h_convert_batch(&app, &req));
        assert_eq!(status, 400, "{payload}");
        assert_eq!(
            payload,
            json!({"ok": false, "error_code": "invalid_content_length"})
        );

        // Unparseable body wins over the confirmation gate.
        let raw = b"{oops";
        let (status, payload, _) = render(h_convert_batch(
            &app,
            &raw_request("POST", "/api/convert/batch", raw, "5"),
        ));
        assert_eq!(status, 400, "{payload}");
        assert_eq!(payload, json!({"ok": false, "error_code": "invalid_request"}));

        // Confirmation wins over the "nothing to do" answer.
        let body = json!({"paths": ["C:/nope/missing.docx"]});
        let (status, payload, _) = render(h_convert_batch(
            &app,
            &json_request("POST", "/api/convert/batch", &body, None),
        ));
        assert_eq!(status, 400, "{payload}");
        assert_eq!(payload, json!({"ok": false, "error_code": "confirmation_required"}));

        // `is True` — a truthy 1 is not a confirmation (readmd.py:3235).
        let body = json!({"confirm": 1, "paths": ["C:/nope/missing.docx"]});
        let (status, payload, _) = render(h_convert_batch(
            &app,
            &json_request("POST", "/api/convert/batch", &body, None),
        ));
        assert_eq!(status, 400, "{payload}");
        assert_eq!(payload["error_code"], "confirmation_required");

        // Confirmed, but no client path survives `os.path.isfile`.
        let body = json!({"confirm": true, "paths": ["C:/nope/missing.docx", 42, null]});
        let (status, payload, _) = render(h_convert_batch(
            &app,
            &json_request("POST", "/api/convert/batch", &body, None),
        ));
        assert_eq!(status, 400, "{payload}");
        assert_eq!(payload, json!({"ok": false, "error_code": "no_convertible_files"}));

        // An empty/absent body is `{}` → `confirm` gate.
        let (status, payload, _) = render(h_convert_batch(
            &app,
            &raw_request("POST", "/api/convert/batch", b"", "0"),
        ));
        assert_eq!(status, 400, "{payload}");
        assert_eq!(payload["error_code"], "confirmation_required");
    }

    /// `readmd.py:3238-3259` + `_start_convert_job`/`_convert_worker`: deduped,
    /// `isfile`-filtered paths answer `{'job', 'total'}`, and each item grows
    /// `out`/`engine`/`warns` or an `error_code` as Python's worker writes the
    /// mdcheck-fixed text.
    #[test]
    fn batch_starts_a_job_that_reports_planned_outputs_and_overwrite_gate() {
        let _env = env_guard();
        let app = test_app("batch-run");
        let dir = scratch_dir("batch-run");
        let alpha = write_file(&dir, "alpha.txt", b"# Alpha\n\ntext\n");
        let beta = write_file(&dir, "beta.txt", b"beta body line\n");
        let gamma = write_file(&dir, "gamma.txt", b"gamma body line\n");
        let mut existing = gamma.clone();
        existing.set_file_name("gamma.md");
        std::fs::write(&existing, b"PRE-EXISTING\n").unwrap();

        let body = json!({
            "confirm": true,
            // alpha twice: `os.path.normcase(os.path.realpath(...))` dedupe.
            "paths": [
                alpha.to_str().unwrap(),
                alpha.to_str().unwrap(),
                beta.to_str().unwrap(),
                gamma.to_str().unwrap()
            ],
        });
        let (status, payload, _) = render(h_convert_batch(
            &app,
            &json_request("POST", "/api/convert/batch", &body, None),
        ));
        assert_eq!(status, 200, "{payload}");
        assert_eq!(key_set(&payload), vec!["job", "total"]);
        assert_eq!(payload["total"], json!(3));
        let jid = payload["job"].as_str().unwrap().to_string();
        assert!(jid.starts_with('c') && jid[1..].bytes().all(|b| b.is_ascii_digit()), "{jid}");

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let items;
        loop {
            let guard = CONVERT_JOBS.lock().unwrap_or_else(|e| e.into_inner());
            let job = guard.as_ref().and_then(|m| m.get(&jid)).cloned();
            drop(guard);
            match job {
                Some(job) if job.finished => {
                    assert!(!job.running);
                    items = job.items;
                    break;
                }
                Some(_) if std::time::Instant::now() > deadline => {
                    panic!("convert batch {jid} never finished")
                }
                Some(_) => {}
                None => panic!("job {jid} disappeared"),
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }

        assert_eq!(items.len(), 3);
        for it in &items {
            assert!(it.done, "{:?}", it.src);
            assert!(it.planned_out.is_some(), "planned_out is set by _start_convert_job");
        }
        let by_src = |needle: &str| {
            items
                .iter()
                .find(|it| it.src.ends_with(needle))
                .unwrap_or_else(|| panic!("no item for {needle}"))
                .clone()
        };
        let alpha = by_src("alpha.txt");
        assert_eq!(alpha.status, "ok", "{:?}", alpha.error);
        assert_eq!(alpha.error_code, None);
        assert!(
            alpha.warns.is_some(),
            "`it['warns'] = warns` runs before the write (readmd.py:903)"
        );
        assert_eq!(alpha.engine.as_deref(), Some("txtmd"));
        let written = std::fs::read_to_string(alpha.out.as_ref().unwrap()).unwrap();
        assert!(!written.contains('\r'), "_write_md writes newline='\\n'");
        assert!(written.contains("Alpha"));

        // An output that already exists is `skipped` + `output_exists`.
        let gamma = by_src("gamma.txt");
        assert_eq!(gamma.status, "skipped");
        assert_eq!(gamma.error_code.as_deref(), Some("output_exists"));
        assert_eq!(std::fs::read_to_string(&existing).unwrap(), "PRE-EXISTING\n");
    }

    /// `readmd.py:822-850 _batch_output_paths`: the second and third source that
    /// would write the same `.md` get `-<sha8>` and then `-<sha8>-2` suffixes,
    /// and a repeated source path is planned once.
    #[test]
    fn planned_outputs_suffix_collisions_with_the_content_digest() {
        let dir = scratch_dir("planned");
        // Identical bytes so the digest is the same in every collision.
        let payload = b"same bytes for all three\n";
        let txt = write_file(&dir, "note.txt", payload);
        let pdf = write_file(&dir, "note.pdf", payload);
        let docx = write_file(&dir, "note.docx", payload);
        let other = write_file(&dir, "elsewhere.txt", payload);
        let paths: Vec<String> = [txt.clone(), txt.clone(), pdf.clone(), docx.clone(), other.clone()]
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();

        let planned = crate::convert::batch_output_paths(&paths);
        assert_eq!(planned.len(), 4, "one entry per unique source");
        assert_eq!(
            planned[txt.to_str().unwrap()],
            dir.join("note.md").to_string_lossy()
        );
        assert_eq!(
            planned[other.to_str().unwrap()],
            dir.join("elsewhere.md").to_string_lossy()
        );

        let second = &planned[pdf.to_str().unwrap()];
        let third = &planned[docx.to_str().unwrap()];
        let stem = dir.join("note").to_string_lossy().into_owned();
        assert!(second.starts_with(&stem), "{second}");
        assert!(second.ends_with(".md"), "{second}");
        let digest = &second[stem.len() + 1..second.len() - 3];
        assert_eq!(digest.len(), 8, "sha256 hexdigest()[:8]");
        assert!(
            digest.bytes().all(|b| b.is_ascii_hexdigit()),
            "{digest} is not hex"
        );
        assert_eq!(
            third,
            &format!("{stem}-{digest}-2.md"),
            "collision on the suffixed candidate adds `-2`"
        );
        assert_ne!(second, third);
    }


    // ====================================================================
    // /api/batch/extract-zip — the route shares convert.rs's one pipeline
    // ====================================================================
    //
    // `readmd.py:3264 _api_batch_extract_zip` keeps the HTTP gate and delegates
    // the archive work to `convert.py:2300 extract_zip_archive`.  The Rust route
    // used to carry its own inline copy of that pipeline, walking `PK\x03\x04`
    // local file headers instead of the central directory.  Every expected value
    // below was produced against the authority first —
    // `python -c "from src.readmd_modules.convert import extract_zip_archive; …"`
    // run in this repo, printing the real payload / exception for the same
    // archive bytes — so each pin is a golden answer, not a restatement of the
    // Rust code.

    /// `zlib.crc32(...)` finalised the way `zipfile` stores it.
    fn probe_crc32(bytes: &[u8]) -> u32 {
        let mut table = [0u32; 256];
        for n in 0u32..256 {
            let mut c = n;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xedb8_8320 ^ (c >> 1) } else { c >> 1 };
            }
            table[n as usize] = c;
        }
        let mut crc = 0xffff_ffffu32;
        for &b in bytes {
            crc = (crc >> 8) ^ table[((crc ^ b as u32) & 0xff) as usize];
        }
        !crc
    }

    /// One archive member, described the way `zipfile` describes it: the central
    /// directory record is the truth, and the local header may advertise
    /// something else entirely (`flag_bits 0x8` really does leave its sizes at
    /// zero — that is what the data descriptor is for).
    struct ProbeEntry {
        name: Vec<u8>,
        data: Vec<u8>,
        utf8_flag: bool,
        /// `ZipInfo.external_attr`, i.e. `(mode << 16)`: `0o120777 << 16` is a
        /// symlink, `0o100644 << 16` an ordinary file.
        external_attr: u32,
        local_sizes_zero: bool,
        /// The central-directory CRC; `None` means the honest digest.
        crc_override: Option<u32>,
    }

    impl ProbeEntry {
        fn file(name: &[u8], data: &[u8]) -> ProbeEntry {
            ProbeEntry {
                name: name.to_vec(),
                data: data.to_vec(),
                utf8_flag: true,
                external_attr: 0o100644 << 16,
                local_sizes_zero: false,
                crc_override: None,
            }
        }
    }

    /// Stored members only — enough to exercise the gates these pins are about.
    fn probe_zip(entries: &[ProbeEntry]) -> Vec<u8> {
        let mut out: Vec<u8> = Vec::new();
        let mut central: Vec<u8> = Vec::new();
        for entry in entries {
            let crc = entry
                .crc_override
                .unwrap_or_else(|| probe_crc32(&entry.data));
            let mut flags: u16 = if entry.utf8_flag { 0x800 } else { 0 };
            if entry.local_sizes_zero {
                flags |= 0x08;
            }
            let size = entry.data.len() as u32;
            let local_offset = out.len() as u32;
            let local_size = if entry.local_sizes_zero { 0 } else { size };

            out.extend_from_slice(b"PK\x03\x04");
            out.extend_from_slice(&20u16.to_le_bytes()); // version needed
            out.extend_from_slice(&flags.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes()); // method: stored
            out.extend_from_slice(&0u16.to_le_bytes()); // time
            out.extend_from_slice(&0u16.to_le_bytes()); // date
            out.extend_from_slice(&crc.to_le_bytes());
            out.extend_from_slice(&local_size.to_le_bytes());
            out.extend_from_slice(&local_size.to_le_bytes());
            out.extend_from_slice(&(entry.name.len() as u16).to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes()); // extra length
            out.extend_from_slice(&entry.name);
            out.extend_from_slice(&entry.data);
            if entry.local_sizes_zero {
                out.extend_from_slice(b"PK\x07\x08");
                out.extend_from_slice(&crc.to_le_bytes());
                out.extend_from_slice(&size.to_le_bytes());
                out.extend_from_slice(&size.to_le_bytes());
            }

            central.extend_from_slice(b"PK\x01\x02");
            central.extend_from_slice(&20u16.to_le_bytes()); // version made by
            central.extend_from_slice(&20u16.to_le_bytes()); // version needed
            central.extend_from_slice(&flags.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes()); // method: stored
            central.extend_from_slice(&0u16.to_le_bytes()); // time
            central.extend_from_slice(&0u16.to_le_bytes()); // date
            central.extend_from_slice(&crc.to_le_bytes());
            central.extend_from_slice(&size.to_le_bytes());
            central.extend_from_slice(&size.to_le_bytes());
            central.extend_from_slice(&(entry.name.len() as u16).to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes()); // extra
            central.extend_from_slice(&0u16.to_le_bytes()); // comment
            central.extend_from_slice(&0u16.to_le_bytes()); // disk start
            central.extend_from_slice(&0u16.to_le_bytes()); // internal attrs
            central.extend_from_slice(&entry.external_attr.to_le_bytes());
            central.extend_from_slice(&local_offset.to_le_bytes());
            central.extend_from_slice(&entry.name);
        }
        let cd_offset = out.len() as u32;
        out.extend_from_slice(&central);
        out.extend_from_slice(b"PK\x05\x06");
        out.extend_from_slice(&0u16.to_le_bytes()); // disk
        out.extend_from_slice(&0u16.to_le_bytes()); // cd start disk
        out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        out.extend_from_slice(&(central.len() as u32).to_le_bytes());
        out.extend_from_slice(&cd_offset.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // comment length
        out
    }

    fn zip_binary_call(app: &Arc<App>, archive: &[u8]) -> (u16, Value, bool) {
        let mut req = raw_request(
            "POST",
            "/api/batch/extract-zip",
            archive,
            &archive.len().to_string(),
        );
        req.headers
            .insert("content-type".to_string(), "application/zip".to_string());
        req.headers
            .insert("x-readmd-confirm".to_string(), "true".to_string());
        render(h_batch_extract_zip(app, &req))
    }

    fn zip_json_call(app: &Arc<App>, body: &Value) -> (u16, Value, bool) {
        render(h_batch_extract_zip(
            app,
            &json_request("POST", "/api/batch/extract-zip", body, None),
        ))
    }

    /// `convert.py:2379-2400` — the central directory drives the run, a directory
/// entry is skipped without counting as a problem, a symlink or device member
/// becomes `unsafe_file_type`, and an unknown extension becomes
/// `unsupported_format`.  Golden, from the authority on these same four members:
/// `total 4, skipped 2, reasons {unsupported_format: 1, unsafe_file_type: 1}`
/// with exactly one file written.
    #[test]
    fn zip_route_answers_the_central_directory_reason_counts() {
        let app = test_app("zip-reasons");
        let mut link = ProbeEntry::file(b"evil.md", b"/etc/passwd");
        link.external_attr = 0o120777 << 16;
        let entries = vec![
            ProbeEntry::file(b"notes/a.md", b"alpha"),
            ProbeEntry::file(b"bin/tool.exe", b"whatever"),
            ProbeEntry::file(b"dir/", b""),
            link,
        ];
        let archive = probe_zip(&entries);

        let (status, payload, _) = zip_binary_call(&app, &archive);
        assert_eq!(status, 200, "{payload}");
        assert_eq!(
            key_set(&payload),
            vec!["ok", "paths", "reasons", "skipped", "total"]
        );
        assert_eq!(payload["ok"], true);
        assert_eq!(payload["total"], 4, "len(zipfile.infolist())");
        assert_eq!(payload["skipped"], 2);
        assert_eq!(payload["reasons"]["unsupported_format"], 1);
        // The inline copy declared this key but had no branch that could ever
        // raise it, so a symlink member was written out as a regular file and
        // counted as a success.
        assert_eq!(payload["reasons"]["unsafe_file_type"], 1);
        assert_eq!(payload["reasons"]["invalid_path"], 0);
        assert_eq!(payload["reasons"]["limit_exceeded"], 0);
        let paths: Vec<String> = payload["paths"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        assert_eq!(paths.len(), 1, "{paths:?}");
        assert_eq!(
            Path::new(&paths[0]).file_name().unwrap().to_string_lossy(),
            "a.md"
        );
        assert_eq!(std::fs::read(&paths[0]).unwrap(), b"alpha");
        // `notes/` is the only directory that may exist under the run.
        let run_root = Path::new(&paths[0]).ancestors().nth(2).unwrap();
        assert!(!run_root.join("evil.md").exists(), "symlink member written");
        assert!(!run_root.join("bin").exists(), "unsupported member written");
    }

    /// A member with `flag_bits 0x8` (a trailing data descriptor, so the *local*
    /// header advertises `compress_size == 0`) still extracts its whole body:
    /// Python reads `infolist()` (`convert.py:2379`), not the local headers.
    /// Golden: the same archive yields `flow.md` holding `b'flowing-body'`.  The
    /// inline copy trusted the local header and wrote an **empty** file, which is
    /// worse than an error because the route then reports a clean extraction.
    #[test]
    fn zip_reads_the_central_directory_when_the_local_header_lies() {
        let app = test_app("zip-flag3");
        let entry = ProbeEntry {
            local_sizes_zero: true,
            ..ProbeEntry::file(b"flow.md", b"flowing-body")
        };
        let archive = probe_zip(std::slice::from_ref(&entry));

        let (status, payload, _) = zip_binary_call(&app, &archive);
        assert_eq!(status, 200, "{payload}");
        assert_eq!(payload["total"], 1);
        assert_eq!(payload["skipped"], 0, "{payload}");
        let path = payload["paths"][0].as_str().unwrap().to_string();
        assert_eq!(std::fs::read(&path).unwrap(), b"flowing-body");
    }

    /// `convert.py:2406-2416`: with the UTF-8 flag clear, `zipfile` has already
    /// decoded the name as cp437 and the re-encode ladder
    /// (gbk/utf-8/gb18030/big5) is tried before that answer is kept.  A lone byte
    /// `0x81` decodes to `ü` and no rung of the ladder takes it, so the extracted
    /// file is `ü.md` — the authority's golden output for this archive is the
    /// UTF-8 bytes `b'\xc3\xbc.md'`.  The inline copy ran
    /// `String::from_utf8_lossy` over the same name and produced `U+FFFD.md`: a
    /// different file on disk holding the same bytes.
    #[test]
    fn zip_decodes_a_non_utf8_member_name_as_cp437() {
        let app = test_app("zip-cp437");
        let entry = ProbeEntry {
            utf8_flag: false,
            ..ProbeEntry::file(&[0x81, b'.', b'm', b'd'], b"accent")
        };
        let archive = probe_zip(std::slice::from_ref(&entry));

        let (status, payload, _) = zip_binary_call(&app, &archive);
        assert_eq!(status, 200, "{payload}");
        assert_eq!(payload["skipped"], 0, "{payload}");
        let path = payload["paths"][0].as_str().unwrap().to_string();
        let name = Path::new(&path)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        assert_eq!(name, "\u{fc}.md", "cp437 0x81 is U+00FC, not U+FFFD");
        assert_eq!(name.as_bytes(), b"\xc3\xbc.md");
        assert_eq!(std::fs::read(&path).unwrap(), b"accent");
    }

    /// A CRC that does not match is **not** a `ValueError`: `zipfile.BadZipFile`
    /// subclasses `Exception` directly (`python -c "import zipfile;
    /// print(zipfile.BadZipFile.__mro__)"`), so `convert.py:2451`'s read raises
    /// past the route's `except ValueError` into `except Exception` — 500
    /// `zip_extract_failed`, still carrying the three empty result keys.  Golden
    /// message from the authority: `Bad CRC-32 for file 'plain.md'`.  The inline
    /// copy never computed a CRC and answered 200 with the corrupt bytes.
    #[test]
    fn zip_refuses_a_member_whose_crc_lies_on_the_exception_arm() {
        let app = test_app("zip-crc");
        let entry = ProbeEntry {
            crc_override: Some(0xdead_beef),
            ..ProbeEntry::file(b"plain.md", b"hello")
        };
        let archive = probe_zip(std::slice::from_ref(&entry));

        let (status, payload, _) = zip_binary_call(&app, &archive);
        assert_eq!(status, 500, "{payload}");
        assert_eq!(payload["ok"], false);
        assert_eq!(payload["error_code"], "zip_extract_failed");
        assert_eq!(
            key_set(&payload),
            vec!["error_code", "ok", "paths", "skipped", "total"]
        );
        assert_eq!(payload["paths"], json!([]));
        assert_eq!(payload["skipped"], 0);
        assert_eq!(payload["total"], 0);
        // `convert.py:2368-2372` — a failed run must not leave its directory
        // behind, and `except Exception` in the route logs rather than leaks.
        let root = app.paths.data_dir.join("temp_zip");
        let leftover: Vec<_> = std::fs::read_dir(&root)
            .map(|rd| rd.filter_map(|e| e.ok()).collect())
            .unwrap_or_default();
        assert!(leftover.is_empty(), "abandoned extraction run: {leftover:?}");
    }

    /// The JSON lane's gates, every shape, against `readmd.py:3299-3316`: a body
    /// over 64 KB is the generic 413; a malformed document is `json.loads`'
    /// `ValueError` → **422** `zip_extract_failed` (this route has no
    /// "请求格式错误" 400 at all); a non-object document raises `AttributeError`
    /// on `.get` → 500; `confirm` is compared with `is True`, so `1` does not
    /// confirm; and the archive must already sit inside `DATA_DIR`/`APP_DIR`
    /// (`validators.py:17-54`), every refusal a bare 400 `invalid_zip_path` with
    /// no result keys.  That last one is the security half: the inline copy asked
    /// only `path.exists() && path.is_file()`, so any readable archive anywhere
    /// on the host could be unpacked into the app's temp root on request.
    #[test]
    fn zip_json_lane_gates_confirmation_and_the_app_roots_before_any_write() {
        let app = test_app("zip-json-gates");
        let archive = probe_zip(&[ProbeEntry::file(b"inside.md", b"body")]);

        // Positive control: the same bytes inside `DATA_DIR` are accepted.
        let home = app.paths.data_dir.join("pkg.zip");
        std::fs::write(&home, &archive).unwrap();
        let (status, payload, _) = zip_json_call(
            &app,
            &json!({"confirm": true, "path": home.to_string_lossy()}),
        );
        assert_eq!(status, 200, "{payload}");
        assert_eq!(payload["ok"], true);
        assert_eq!(payload["paths"].as_array().unwrap().len(), 1);

        // Outside both roots: refused before a single byte is read.
        let outside = std::env::temp_dir().join(format!(
            "readmd-zip-outside-{}-{}.zip",
            std::process::id(),
            now_millis()
        ));
        std::fs::write(&outside, &archive).unwrap();
        let (status, payload, _) = zip_json_call(
            &app,
            &json!({"confirm": true, "path": outside.to_string_lossy()}),
        );
        assert_eq!(status, 400, "{payload}");
        assert_eq!(
            payload,
            json!({"ok": false, "error_code": "invalid_zip_path"})
        );
        let _ = std::fs::remove_file(&outside);

        // Right root, wrong extension: `allowed_extensions=['.zip']`.
        let not_zip = app.paths.data_dir.join("notes.txt");
        std::fs::write(&not_zip, &archive).unwrap();
        let (status, payload, _) = zip_json_call(
            &app,
            &json!({"confirm": true, "path": not_zip.to_string_lossy()}),
        );
        assert_eq!(status, 400, "{payload}");
        assert_eq!(
            payload,
            json!({"ok": false, "error_code": "invalid_zip_path"})
        );

        // The `.zip` decision is now the shared validator's, so it carries
        // Python's case rule too: `validators.py:51-52` lowers **both** sides,
        // so an uppercase name is still a zip.
        let upper = app.paths.data_dir.join("PKG.ZIP");
        std::fs::write(&upper, &archive).unwrap();
        let (status, payload, _) = zip_json_call(
            &app,
            &json!({"confirm": true, "path": upper.to_string_lossy()}),
        );
        assert_eq!(status, 200, "{payload}");
        assert_eq!(payload["ok"], true);

        // … while a file *named* `.zip` is a dotfile, not an extension:
        // `os.path.splitext('.zip')` → `('.zip', '')`, so `['.zip']` refuses it.
        let dotfile = app.paths.data_dir.join(".zip");
        std::fs::write(&dotfile, &archive).unwrap();
        let (status, payload, _) = zip_json_call(
            &app,
            &json!({"confirm": true, "path": dotfile.to_string_lossy()}),
        );
        assert_eq!(status, 400, "{payload}");
        assert_eq!(
            payload,
            json!({"ok": false, "error_code": "invalid_zip_path"})
        );

        // `is True`, not truthy.
        let (status, payload, _) =
            zip_json_call(&app, &json!({"confirm": 1, "path": home.to_string_lossy()}));
        assert_eq!(status, 400, "{payload}");
        assert_eq!(
            payload,
            json!({"ok": false, "error_code": "confirmation_required"})
        );

        // A malformed document is the `ValueError` arm.
        let (status, payload, _) = render(h_batch_extract_zip(
            &app,
            &raw_request("POST", "/api/batch/extract-zip", b"{oops", "5"),
        ));
        assert_eq!(status, 422, "{payload}");
        assert_eq!(payload["error_code"], "zip_extract_failed");
        assert_eq!(payload["paths"], json!([]));

        // `body.get` on a list is the `except Exception` arm.
        let (status, payload, _) = zip_json_call(&app, &json!([1, 2, 3]));
        assert_eq!(status, 500, "{payload}");
        assert_eq!(payload["error_code"], "zip_extract_failed");

        // A `Content-Length` that is not an integer is the same `ValueError`
        // arm, since `int(...)` sits inside the route's `try`.
        let (status, payload, _) = render(h_batch_extract_zip(
            &app,
            &raw_request("POST", "/api/batch/extract-zip", b"{}", "nonsense"),
        ));
        assert_eq!(status, 422, "{payload}");
        assert_eq!(payload["error_code"], "zip_extract_failed");

        // Not-a-zip payload on the binary lane: `BadZipFile` again, so 500.
        let (status, payload, _) = zip_binary_call(&app, b"PK\x03\x04garbage");
        assert_eq!(status, 500, "{payload}");
        assert_eq!(payload["error_code"], "zip_extract_failed");

        // The binary header gate is `.strip().lower()`-compared
        // (`readmd.py:3281`), so `TRUE ` confirms; an absent header does not.
        let mut req = raw_request(
            "POST",
            "/api/batch/extract-zip",
            &archive,
            &archive.len().to_string(),
        );
        req.headers
            .insert("content-type".to_string(), "application/zip".to_string());
        let (status, payload, _) = render(h_batch_extract_zip(&app, &req));
        assert_eq!(status, 400, "{payload}");
        assert_eq!(payload, json!({"ok": false, "error_code": "confirmation_required"}));
        req.headers
            .insert("x-readmd-confirm".to_string(), "TRUE ".to_string());
        let (status, payload, _) = render(h_batch_extract_zip(&app, &req));
        assert_eq!(status, 200, "{payload}");
        assert_eq!(payload["ok"], true);
    }

}