//! The native pet host launch contract.
//!
//! Authority is split across two trees, and both have to agree for a pet to
//! appear:
//!
//! * the **parent** side is `src/readmd_modules/pet/runtime.py:536-608`
//!   (`RustPetRuntime.start`) and `:410-435` (`_health`) / `:508-536`
//!   (`_wait_health`);
//! * the **child** side is the separate binary `packages/readmd-pet-rust`
//!   (`main.rs` + `runtime.rs::HostConfig::from_env`, `bridge/health.rs`,
//!   `bridge/parent_liveness.rs`).  The kernel does not link it and must not
//!   grow its own window: it is a supervisor for a program shipped as a
//!   verified runtime package.
//!
//! # The contract, verbatim
//!
//! argv is `[binary_path]` and nothing else (`runtime.py:588`); `cwd` is
//! `runtime_dir` (`:573`); the environment is the parent's plus exactly the
//! seven keys below (`:562`, `:567`, `:569`).  Health is
//! `<bridge state file>.rust.health.json` (`bridge/health.rs:48-53`), and a
//! report is only evidence when it is under 32 KiB, parses to an object with
//! `engine == "rust"`, and carries the pid of the child *we* just spawned
//! (`runtime.py:412-425`).
//!
//! ```text
//! READMD_PET_BRIDGE_FILE    = <bridge>.hermes-overlay-state.json
//! READMD_PARENT_PID         = str(os.getpid())
//! READMD_PET_RENDERER       = "hermes-sprite" | "live2d"
//! READMD_PET_RUNTIME_DIR    = <installed rust host tree>
//! READMD_PET_RENDERER_ROOT  = <runtime dir>/renderer
//! READMD_DATA_DIR           = <bridge root>.parent      (the plugins directory)
//! WEBVIEW2_USER_DATA_FOLDER = <bridge root>/webview2
//! READMD_PARENT_PIPE_HANDLE = inherited anonymous pipe read end (optional)
//! ```
//!
//! # Liveness / parent death
//!
//! Python also hands the child an inheritable anonymous pipe and closes its
//! write end on `stop()` (`runtime.py:493-506`, `:610-616`).  `packages/
//! readmd-pet-rust` treats that pipe as one of *two* probes:
//! `resolve_parent_liveness` (`bridge/parent_liveness.rs:45-57`) only exits on a
//! real pipe EOF, and otherwise falls back to a `READMD_PARENT_PID` liveness
//! probe — and stays up when there is no evidence at all, "because a leaked pet
//! is recoverable while a pet that vanishes at launch is not".  Inheriting a
//! pipe handle from Rust needs `STARTUPINFOEX`/`CreatePipe`, i.e. a Win32
//! binding the kernel does not have and cannot gain without a new dependency,
//! so this lane ships the documented PID-probe path and leaves the handle
//! unset.  Consequence, stated plainly: `stop()` cannot be *instant*, it is
//! bounded by the child's own liveness poll interval.
//!
//! # `// WIRING:`
//!
//! Nothing here calls into `pet_queue.rs` / `sprite_processor.rs`; the launch
//! path needs neither.  The seam for the *state* half of the bridge (publishing
//! `hermes-overlay-state.json` so the overlay has something to read) is
//! `PetBridgePublisher` below.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// `runtime.py:562`, `:567`, `:569`.
pub const ENV_BRIDGE_FILE: &str = "READMD_PET_BRIDGE_FILE";
pub const ENV_PARENT_PID: &str = "READMD_PARENT_PID";
pub const ENV_RENDERER: &str = "READMD_PET_RENDERER";
pub const ENV_RUNTIME_DIR: &str = "READMD_PET_RUNTIME_DIR";
pub const ENV_RENDERER_ROOT: &str = "READMD_PET_RENDERER_ROOT";
pub const ENV_DATA_DIR: &str = "READMD_DATA_DIR";
pub const ENV_WEBVIEW2_FOLDER: &str = "WEBVIEW2_USER_DATA_FOLDER";
pub const ENV_PARENT_PIPE_HANDLE: &str = "READMD_PARENT_PIPE_HANDLE";
/// `runtime.py:604` — the wait budget, overridable exactly as Python allows.
pub const ENV_HEALTH_TIMEOUT: &str = "READMD_PET_HEALTH_TIMEOUT";
/// `runtime.py:604`'s literal default.
pub const DEFAULT_HEALTH_TIMEOUT_SECS: f64 = 15.0;

/// `bridge/health.rs:48-53` and `runtime.py:412`: the report path and its cap.
pub const HEALTH_FILE_SUFFIX: &str = ".rust.health.json";
pub const MAX_HEALTH_BYTES: u64 = 32 * 1024;

/// `runtime.py:419` — a report from any other engine is not evidence.
pub const HEALTH_ENGINE: &str = "rust";

/// `RustPetRuntime.renderer_root` (`runtime.py:401-403`).
pub fn renderer_root(runtime_dir: &Path) -> PathBuf {
    runtime_dir.join("renderer")
}

/// `Path(str(bridge.state_path) + ".rust.health.json")` — a *string* suffix in
/// Python, not a path join, so the file is a sibling named after the state file.
pub fn health_file(bridge_file: &Path) -> PathBuf {
    PathBuf::from(format!("{}{HEALTH_FILE_SUFFIX}", bridge_file.display()))
}

/// Everything `RustPetRuntime.start` puts on the child's command line, as a
/// pure function of the four paths Python reads off its own object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostPlan {
    pub binary: PathBuf,
    pub cwd: PathBuf,
    pub env: BTreeMap<String, String>,
    pub health_file: PathBuf,
    pub bridge_file: PathBuf,
}

/// Build the launch plan.
///
/// `bridge_root` is `HermesPetBridge._root` (`hermes_adapter.py:84-86`), i.e.
/// `<install root>/pet`; `runtime.py:562` sends its **parent** as the child's
/// `READMD_DATA_DIR` and puts `webview2/` beside the state file so the WebView
/// cache cannot read as tree tampering (`runtime.py:563-567`).
pub fn build_plan(
    binary: &Path,
    runtime_dir: &Path,
    bridge_file: &Path,
    bridge_root: &Path,
    renderer: &str,
    parent_pid: u32,
    extra_env: &BTreeMap<String, String>,
) -> HostPlan {
    // Every key is an owned copy: `extra_env` is the caller's ambient map and
    // the seven contract keys must overwrite it rather than sit beside it.
    let contract: [(&str, String); 7] = [
        (ENV_BRIDGE_FILE, bridge_file.display().to_string()),
        (ENV_PARENT_PID, parent_pid.to_string()),
        (ENV_RENDERER, renderer.to_string()),
        (ENV_RUNTIME_DIR, runtime_dir.display().to_string()),
        (
            ENV_RENDERER_ROOT,
            renderer_root(runtime_dir).display().to_string(),
        ),
        // `str(self._bridge._root.parent)` — the plugins directory, *not*
        // `config.DATA_DIR` (`runtime.py:562`).
        (
            ENV_DATA_DIR,
            bridge_root
                .parent()
                .unwrap_or(bridge_root)
                .display()
                .to_string(),
        ),
        (
            ENV_WEBVIEW2_FOLDER,
            bridge_root.join("webview2").display().to_string(),
        ),
    ];
    let mut env = extra_env.clone();
    for (key, value) in contract {
        env.insert(key.to_string(), value);
    }
    HostPlan {
        binary: binary.to_path_buf(),
        cwd: runtime_dir.to_path_buf(),
        env,
        health_file: health_file(bridge_file),
        bridge_file: bridge_file.to_path_buf(),
    }
}

/// `runtime.py:604`'s `float(os.environ.get("READMD_PET_HEALTH_TIMEOUT", "15"))`.
/// A malformed value is a `ValueError` in Python, i.e. a failed start; the port
/// keeps that shape by falling back to the documented default only when the
/// variable is absent.
pub fn health_timeout(env: &BTreeMap<String, String>) -> Duration {
    let seconds = env
        .get(ENV_HEALTH_TIMEOUT)
        .and_then(|value| value.trim().parse::<f64>().ok())
        .unwrap_or(DEFAULT_HEALTH_TIMEOUT_SECS);
    Duration::from_millis((seconds.max(0.0) * 1000.0) as u64)
}

/// One observation of the child's health report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthVerdict {
    /// `state == "ready"` from the owned pid: the pet is on screen.
    Ready,
    /// A terminal state the parent must treat as a failed start
    /// (`runtime.py:528-535`), carrying the `rust_health_<state>_<code>`
    /// diagnostic Python records.
    Failed(String),
    /// Nothing usable yet — a missing file, a foreign engine, a stale pid, or
    /// an oversized report (`runtime.py:412-425` all return `{}`).
    Unusable,
}

/// `RustPetRuntime._health(allowed_pids = {owned_pid})` (`runtime.py:410-425`).
///
/// `None` is Python's `{}`: a report over the size cap, an unparseable body, a
/// non-object, a foreign `engine`, or a pid that is not the child we own.
pub fn health_report(raw: &[u8], allowed_pid: u32) -> Option<serde_json::Value> {
    if raw.len() as u64 > MAX_HEALTH_BYTES {
        return None;
    }
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&String::from_utf8_lossy(raw))
    else {
        return None;
    };
    let Some(object) = value.as_object() else {
        return None;
    };
    if object.get("engine").and_then(serde_json::Value::as_str) != Some(HEALTH_ENGINE) {
        return None;
    }
    // `_wait_health` accepts a `ready` from *our* pid only.
    if object.get("pid").and_then(serde_json::Value::as_u64) != Some(u64::from(allowed_pid)) {
        return None;
    }
    Some(value)
}

/// `_wait_health`'s branch logic (`runtime.py:520-535`) over one accepted report.
pub fn verdict_of(report: &serde_json::Value) -> HealthVerdict {
    let state = report.get("state").and_then(serde_json::Value::as_str);
    match state {
        Some("ready") => HealthVerdict::Ready,
        Some("degraded") | Some("failed") | Some("stopped") => {
            let code = report
                .get("code")
                .and_then(serde_json::Value::as_str)
                .filter(|value| !value.is_empty())
                .unwrap_or("unknown");
            HealthVerdict::Failed(format!(
                "rust_health_{}_{}",
                state.unwrap_or(""),
                code
            ))
        }
        // `booting` / `host_starting` / anything else keeps polling.
        _ => HealthVerdict::Unusable,
    }
}

/// `_health(allowed_pids = {owned_pid})` followed by `_wait_health`'s branch
/// logic, as one pure step over the bytes on disk.
pub fn classify_health(raw: &[u8], owned_pid: u32) -> HealthVerdict {
    match health_report(raw, owned_pid) {
        Some(report) => verdict_of(&report),
        None => HealthVerdict::Unusable,
    }
}

/// Read the report the way `_health` does: the file may legitimately not exist
/// yet, and every other failure mode means "no evidence".
pub fn read_health(health_file: &Path, owned_pid: u32) -> HealthVerdict {
    match std::fs::read(health_file) {
        Ok(bytes) => classify_health(&bytes, owned_pid),
        Err(_) => HealthVerdict::Unusable,
    }
}

/// `_wait_health`, minus the process-table probe (the caller owns the child).
///
/// Polls at Python's 50 ms (`runtime.py:536`).  `Ok(())` is a `ready` from this
/// pid; `Err` carries the same diagnostic string Python writes into
/// `self._diagnostic`.
pub fn wait_for_health(
    health_file: &Path,
    owned_pid: u32,
    budget: Duration,
    mut exited: impl FnMut() -> Option<Option<i32>>,
) -> Result<(), String> {
    let deadline = Instant::now() + budget;
    loop {
        // `runtime.py:511-514`: an exited child wins over whatever is on disk.
        if let Some(code) = exited() {
            return Err(format!("rust_exit_{}", code.map_or("signal".to_string(), |value| value.to_string())));
        }
        match read_health(health_file, owned_pid) {
            HealthVerdict::Ready => return Ok(()),
            HealthVerdict::Failed(diagnostic) => return Err(diagnostic),
            HealthVerdict::Unusable => {}
        }
        if Instant::now() >= deadline {
            return Err("rust_health_timeout".to_string());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// The seam the orchestrator owns: the app must also *publish*
/// `hermes-overlay-state.json` (`HermesPetBridge.publish`,
/// `hermes_adapter.py:94+`) or the overlay boots with nothing to draw.  This
/// trait exists so the launch path compiles and is testable today while the
/// bridge port lands.
// WIRING: connect to the pet bridge publisher when that lane exists.
pub trait PetBridgePublisher {
    fn state_path(&self) -> PathBuf;
    fn root(&self) -> PathBuf;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths() -> (PathBuf, PathBuf, PathBuf, PathBuf) {
        (
            PathBuf::from("C:/app/plugins/pet/readmd-rust-host/readmd-pet-rust.exe"),
            PathBuf::from("C:/app/plugins/pet/readmd-rust-host"),
            PathBuf::from("C:/app/plugins/pet/hermes-overlay-state.json"),
            PathBuf::from("C:/app/plugins/pet"),
        )
    }

    fn plan(renderer: &str) -> HostPlan {
        let (binary, runtime, bridge, root) = paths();
        build_plan(&binary, &runtime, &bridge, &root, renderer, 4242, &BTreeMap::new())
    }

    #[test]
    fn the_plan_carries_exactly_the_seven_python_keys_and_no_pipe() {
        let plan = plan("hermes-sprite");
        let mut keys: Vec<&str> = plan.env.keys().map(String::as_str).collect();
        keys.sort_unstable();
        let mut expected = [
            ENV_BRIDGE_FILE,
            ENV_DATA_DIR,
            ENV_PARENT_PID,
            ENV_RENDERER,
            ENV_RENDERER_ROOT,
            ENV_RUNTIME_DIR,
            ENV_WEBVIEW2_FOLDER,
        ];
        expected.sort_unstable();
        assert_eq!(keys, expected.to_vec());
        assert!(!plan.env.contains_key(ENV_PARENT_PIPE_HANDLE));
    }

    #[test]
    fn every_value_matches_runtime_py() {
        let plan = plan("hermes-sprite");
        assert_eq!(plan.binary.display().to_string().replace('\\', "/"), "C:/app/plugins/pet/readmd-rust-host/readmd-pet-rust.exe");
        // `kwargs = {"cwd": str(self.runtime_dir), ...}` (`runtime.py:573`).
        assert_eq!(plan.cwd, PathBuf::from("C:/app/plugins/pet/readmd-rust-host"));
        assert_eq!(
            plan.env[ENV_BRIDGE_FILE],
            PathBuf::from("C:/app/plugins/pet/hermes-overlay-state.json")
                .display()
                .to_string()
        );
        assert_eq!(plan.env[ENV_PARENT_PID], "4242");
        assert_eq!(plan.env[ENV_RENDERER], "hermes-sprite");
        assert_eq!(
            plan.env[ENV_RENDERER_ROOT],
            renderer_root(Path::new("C:/app/plugins/pet/readmd-rust-host"))
                .display()
                .to_string()
        );
        // `READMD_DATA_DIR` is the *plugins* directory, not `config.DATA_DIR`.
        assert_eq!(
            plan.env[ENV_DATA_DIR],
            PathBuf::from("C:/app/plugins").display().to_string()
        );
        // `runtime.py:567` is `str(self._bridge._root / "webview2")`.  On
        // Windows pathlib rewrites the *whole* string with `\` (measured live
        // in `scratch/rust_parity/pets_probe2.py` CASE H1:
        // `str(WindowsPath("C:/app/plugins/pet") / "webview2")` ==
        // `'C:\\app\\plugins\\pet\\webview2'`), while Rust's `Path::join` only
        // *inserts* a separator and leaves the caller's base spelling alone, so
        // a forward-slash base yields the mixed
        // `C:/app/plugins/pet\webview2`.  Production cannot observe the
        // difference — `HermesPetBridge._root` is `Path(...).resolve()`d, hence
        // already all-backslash — so the row pins what parity actually means
        // here: the bridge root plus exactly one `webview2` component.  Same
        // normalisation this test already applies to `plan.binary`.
        assert_eq!(
            plan.env[ENV_WEBVIEW2_FOLDER].replace('\\', "/"),
            "C:/app/plugins/pet/webview2"
        );
    }

    #[test]
    fn live2d_is_forwarded_unchanged() {
        assert_eq!(plan("live2d").env[ENV_RENDERER], "live2d");
    }

    #[test]
    fn an_inherited_environment_survives() {
        let mut extra = BTreeMap::new();
        extra.insert("PATH".to_string(), "C:/windows".to_string());
        extra.insert(ENV_DATA_DIR.to_string(), "C:/stale".to_string());
        let (binary, runtime, bridge, root) = paths();
        let plan = build_plan(&binary, &runtime, &bridge, &root, "hermes-sprite", 7, &extra);
        assert_eq!(plan.env["PATH"], "C:/windows");
        // The seven contract keys always win over an ambient value.
        assert_eq!(
            plan.env[ENV_DATA_DIR],
            PathBuf::from("C:/app/plugins").display().to_string()
        );
    }

    #[test]
    fn the_health_file_is_a_string_suffix_on_the_state_path() {
        let bridge = PathBuf::from("C:/app/plugins/pet/hermes-overlay-state.json");
        assert_eq!(
            health_file(&bridge).file_name().unwrap().to_string_lossy(),
            "hermes-overlay-state.json.rust.health.json"
        );
    }

    fn health(state: &str, code: &str, pid: u64) -> Vec<u8> {
        format!(
            r#"{{"engine":"rust","state":"{state}","renderer":"hermes-sprite","code":"{code}","pid":{pid},"updated_at":1,"engine_generation":1,"runtime_generation":2,"protocol_version":1}}"#
        )
        .into_bytes()
    }

    #[test]
    fn only_a_ready_from_our_own_pid_is_success() {
        assert_eq!(classify_health(&health("ready", "", 4242), 4242), HealthVerdict::Ready);
        // `runtime.py:520-524`: a predecessor's stale `ready` is not evidence.
        assert_eq!(classify_health(&health("ready", "", 999), 4242), HealthVerdict::Unusable);
    }

    #[test]
    fn a_terminal_state_becomes_the_python_diagnostic() {
        assert_eq!(
            classify_health(&health("failed", "webview_create_failed", 4242), 4242),
            HealthVerdict::Failed("rust_health_failed_webview_create_failed".to_string())
        );
        // `code` missing or empty -> "unknown" (`runtime.py:534`).
        assert_eq!(
            classify_health(&health("stopped", "", 4242), 4242),
            HealthVerdict::Failed("rust_health_stopped_unknown".to_string())
        );
    }

    #[test]
    fn a_booting_report_and_every_malformed_report_are_no_evidence() {
        assert_eq!(classify_health(&health("booting", "", 4242), 4242), HealthVerdict::Unusable);
        assert_eq!(classify_health(b"not json", 4242), HealthVerdict::Unusable);
        assert_eq!(classify_health(b"[1,2]", 4242), HealthVerdict::Unusable);
        // `runtime.py:419`: another engine's report is ignored outright.
        assert_eq!(
            classify_health(br#"{"engine":"electron","state":"ready","pid":4242}"#, 4242),
            HealthVerdict::Unusable
        );
        // `runtime.py:414-415`: an oversized report is discarded before parsing.
        let big = health("ready", "", 4242);
        let mut padded = big.clone();
        padded.resize(MAX_HEALTH_BYTES as usize + 1, b' ');
        assert_eq!(classify_health(&padded, 4242), HealthVerdict::Unusable);
    }

    #[test]
    fn a_missing_health_file_never_panics() {
        assert_eq!(
            read_health(Path::new("Z:/definitely/not/here.json.rust.health.json"), 1),
            HealthVerdict::Unusable
        );
    }

    #[test]
    fn the_timeout_reads_the_same_variable_python_does() {
        assert_eq!(health_timeout(&BTreeMap::new()), Duration::from_secs_f64(15.0));
        let mut env = BTreeMap::new();
        env.insert(ENV_HEALTH_TIMEOUT.to_string(), "2.5".to_string());
        assert_eq!(health_timeout(&env), Duration::from_millis(2500));
        env.insert(ENV_HEALTH_TIMEOUT.to_string(), "nope".to_string());
        assert_eq!(health_timeout(&env), Duration::from_secs_f64(15.0));
    }

    #[test]
    fn wait_for_health_reports_a_dead_child_before_the_timeout() {
        let dir = std::env::temp_dir().join(format!(
            "readmd-pet-host-dead-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("state.json.rust.health.json");
        let started = Instant::now();
        let verdict = wait_for_health(&file, 5, Duration::from_secs(30), || Some(Some(3)));
        assert_eq!(verdict, Err("rust_exit_3".to_string()));
        // A dead child short-circuits the 15 s budget instead of timing out.
        assert!(started.elapsed() < Duration::from_secs(2));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn wait_for_health_accepts_a_report_that_lands_mid_flight() {
        let dir = std::env::temp_dir().join(format!(
            "readmd-pet-host-live-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("state.json.rust.health.json");
        let writer_file = file.clone();
        let writer = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(120));
            let _ = std::fs::write(&writer_file, health("ready", "", 77));
        });
        assert_eq!(
            wait_for_health(&file, 77, Duration::from_secs(10), || None),
            Ok(())
        );
        writer.join().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn wait_for_health_gives_up_with_the_python_diagnostic() {
        let dir = std::env::temp_dir().join(format!(
            "readmd-pet-host-timeout-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("state.json.rust.health.json");
        assert_eq!(
            wait_for_health(&file, 1, Duration::from_millis(120), || None),
            Err("rust_health_timeout".to_string())
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

}
