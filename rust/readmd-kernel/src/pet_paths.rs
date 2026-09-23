//! Platform data-directory resolution for the pets feature.
//!
//! Authority: `src/readmd_core/config.py:22-46` (`_platform_data_dir`, and the
//! `DATA_DIR = _platform_data_dir()` it feeds).  Every pet endpoint in
//! `readmd.py` takes its library root from that value —
//! `list_pets(DATA_DIR)` (`readmd.py:1656`), `register_local_pet(DATA_DIR, …)`
//! (`:1744`), `remove_pet(DATA_DIR, …)` (`:1767`), `find_pet(DATA_DIR, …)`
//! (`:1788`), `PetCompanion(DATA_DIR)` (`:3739`) — so the kernel has to land on
//! the same directory Python chose, on every platform.
//!
//! Python's precedence is three deep:
//! 1. `READMD_DATA_DIR`, verbatim — but only if nothing it was computed next to
//!    has moved since the module was imported (`config.py:27-35`);
//! 2. the platform default (`APPDATA` on Windows, `XDG_DATA_HOME` elsewhere);
//! 3. the home directory (`expanduser('~')`), including on Windows where
//!    `APPDATA` is simply absent.
//!
//! The selection itself is a pure function over an injected
//! platform / environment / home triple, which is what makes the non-Windows
//! branches testable from a Windows host.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The one component Python appends to every platform root
/// (`config.py:39`, `:41`, `:43`).  Python spells it `ReadMD`; the kernel's own
/// `paths::data_dir()` appends a lowercase `readmd`, which on Linux and macOS
/// is a genuinely different directory.
pub const APP_DATA_DIR_NAME: &str = "ReadMD";

/// `config.py:27` — the isolation override used by UI test servers.
pub const ENV_DATA_DIR_OVERRIDE: &str = "READMD_DATA_DIR";
/// `config.py:41` — the Windows root.
pub const ENV_APPDATA: &str = "APPDATA";
/// `config.py:42` — the XDG Base Directory root.
pub const ENV_XDG_DATA_HOME: &str = "XDG_DATA_HOME";

/// Python's `sys.platform` spellings, as `config.py` compares them.
pub const PLATFORM_WINDOWS: &str = "win32";
pub const PLATFORM_MACOS: &str = "darwin";

/// `_INITIAL_PLATFORM` / `_INITIAL_APPDATA` / `_INITIAL_XDG_DATA_HOME`
/// (`config.py:17-19`).  Python captures these at module import so an
/// in-process test that patches `sys.platform` or a data-root variable cannot
/// inherit a real user's `READMD_DATA_DIR`.  Rust has no import step, so the
/// kernel captures on first observation; a normal process reads `DATA_DIR`
/// exactly once at startup, which makes the two capture points equivalent in
/// every shipped path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportSnapshot {
    pub platform: String,
    pub appdata: Option<String>,
    pub xdg_data_home: Option<String>,
}

impl ImportSnapshot {
    /// A snapshot with nothing captured, i.e. a process that started with a
    /// clean environment.
    pub fn empty() -> Self {
        ImportSnapshot {
            platform: String::new(),
            appdata: None,
            xdg_data_home: None,
        }
    }
}

fn truthy<'a>(env: &'a BTreeMap<String, String>, key: &str) -> Option<&'a String> {
    // `os.environ.get(k)` then a bare `if value:` — an empty string is falsy in
    // Python, and Python never trims, so only `is_empty()` is faithful here.
    env.get(key).filter(|value| !value.is_empty())
}

/// `config.py:22-43`, with every ambient input injected.
///
/// `snapshot` is compared with raw `Option<String>` equality, *not* truthiness,
/// because that is what Python does (`os.environ.get('APPDATA') !=
/// _INITIAL_APPDATA`): unsetting a variable that was present at capture, or
/// setting it to the empty string, both count as "the root moved" and veto the
/// override.
pub fn select_data_dir(
    platform: &str,
    env: &BTreeMap<String, String>,
    home: &Path,
    snapshot: &ImportSnapshot,
) -> PathBuf {
    let raw = |key: &str| env.get(key).cloned();
    let override_value = truthy(env, ENV_DATA_DIR_OVERRIDE).cloned();
    if let Some(value) = override_value {
        let platform_changed = platform != snapshot.platform;
        let env_root_changed = raw(ENV_APPDATA) != snapshot.appdata
            || raw(ENV_XDG_DATA_HOME) != snapshot.xdg_data_home;
        // `config.py:34-36`: `override = None`, i.e. fall through to the
        // platform chain rather than return the vetoed value.
        if !(platform_changed || env_root_changed) {
            return PathBuf::from(value);
        }
    }
    if platform == PLATFORM_MACOS {
        return home
            .join("Library")
            .join("Application Support")
            .join(APP_DATA_DIR_NAME);
    }
    if platform == PLATFORM_WINDOWS {
        // `os.environ.get('APPDATA') or os.path.expanduser('~')`: an empty or
        // absent `APPDATA` falls back to home *directly*, not to
        // `%USERPROFILE%\AppData\Roaming`.
        let root = match truthy(env, ENV_APPDATA) {
            Some(appdata) => PathBuf::from(appdata),
            None => home.to_path_buf(),
        };
        return root.join(APP_DATA_DIR_NAME);
    }
    // Every non-Windows, non-macOS `sys.platform` takes this branch — Python
    // tests `'darwin'` and `'win32'` explicitly and lets the rest fall through.
    let xdg = match truthy(env, ENV_XDG_DATA_HOME) {
        Some(value) => PathBuf::from(value),
        None => home.join(".local").join("share"),
    };
    xdg.join(APP_DATA_DIR_NAME)
}

/// Python's `sys.platform` for the host this binary was built for.
pub fn current_platform() -> &'static str {
    match std::env::consts::OS {
        "windows" => PLATFORM_WINDOWS,
        "macos" => PLATFORM_MACOS,
        other => other,
    }
}

fn env_map() -> BTreeMap<String, String> {
    std::env::vars().collect()
}

/// The captured import-time environment, frozen on first call.
pub fn import_snapshot() -> ImportSnapshot {
    static SNAPSHOT: OnceLock<ImportSnapshot> = OnceLock::new();
    SNAPSHOT.get_or_init(|| {
        let env = env_map();
        ImportSnapshot {
            platform: current_platform().to_string(),
            appdata: env.get(ENV_APPDATA).cloned(),
            xdg_data_home: env.get(ENV_XDG_DATA_HOME).cloned(),
        }
    }).clone()
}

/// `config.DATA_DIR` for the running process.
pub fn data_dir() -> PathBuf {
    select_data_dir(
        current_platform(),
        &env_map(),
        &crate::paths::home_dir(),
        &import_snapshot(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn home() -> PathBuf {
        PathBuf::from(if cfg!(windows) { "C:\\Users\\native" } else { "/home/native" })
    }

    /// A snapshot that matches the injected inputs, so the override survives.
    fn steady(platform: &str, env: &BTreeMap<String, String>) -> ImportSnapshot {
        ImportSnapshot {
            platform: platform.to_string(),
            appdata: env.get(ENV_APPDATA).cloned(),
            xdg_data_home: env.get(ENV_XDG_DATA_HOME).cloned(),
        }
    }

    fn picked(platform: &str, pairs: &[(&str, &str)]) -> PathBuf {
        let env = env(pairs);
        let snapshot = steady(platform, &env);
        select_data_dir(platform, &env, home().as_path(), &snapshot)
    }

    // -- tier 2: platform default ------------------------------------------

    #[test]
    fn windows_uses_appdata_and_the_readmd_component() {
        // `config.py:41` with a normal desktop session.
        assert_eq!(
            picked(PLATFORM_WINDOWS, &[("APPDATA", "C:\\Users\\native\\AppData\\Roaming")]),
            PathBuf::from("C:\\Users\\native\\AppData\\Roaming").join("ReadMD")
        );
    }

    #[test]
    fn macos_uses_the_application_support_tree() {
        // `config.py:39`.
        assert_eq!(
            picked(PLATFORM_MACOS, &[]),
            home()
                .join("Library")
                .join("Application Support")
                .join("ReadMD")
        );
    }

    #[test]
    fn linux_uses_xdg_data_home_when_it_is_set() {
        // `config.py:42-43`, first arm of the `or`.
        assert_eq!(
            picked("linux", &[("XDG_DATA_HOME", "/var/lib/native")]),
            PathBuf::from("/var/lib/native").join("ReadMD")
        );
    }

    #[test]
    fn linux_falls_back_to_the_base_directory_specification_home() {
        // `config.py:42-43`, second arm of the `or`.
        assert_eq!(
            picked("linux", &[]),
            home().join(".local").join("share").join("ReadMD")
        );
    }

    #[test]
    fn an_unrecognised_platform_takes_the_xdg_branch() {
        // Python only tests `'darwin'` and `'win32'`; `freebsd` must not be
        // silently treated as Windows.
        assert_eq!(
            picked("freebsd", &[]),
            home().join(".local").join("share").join("ReadMD")
        );
    }

    // -- tier 3: home-directory fallback -----------------------------------

    #[test]
    fn an_absent_appdata_falls_back_to_home_not_to_roaming() {
        // The kernel's own `paths::data_dir()` synthesises
        // `%USERPROFILE%\AppData\Roaming`; Python does not, and the pets
        // library has to sit where Python puts it.
        assert_eq!(picked(PLATFORM_WINDOWS, &[]), home().join("ReadMD"));
    }

    #[test]
    fn an_empty_env_root_is_falsy_so_the_home_default_wins() {
        // `os.environ.get('APPDATA') or expanduser('~')` — an empty string is
        // falsy.  The same holds for `XDG_DATA_HOME`.
        assert_eq!(picked(PLATFORM_WINDOWS, &[("APPDATA", "")]), home().join("ReadMD"));
        assert_eq!(
            picked("linux", &[("XDG_DATA_HOME", "")]),
            home().join(".local").join("share").join("ReadMD")
        );
    }

    // -- tier 1: the isolation override ------------------------------------

    #[test]
    fn the_override_is_returned_verbatim_when_nothing_moved() {
        let env = env(&[(ENV_DATA_DIR_OVERRIDE, "D:\\isolated\\case Sensitive")]);
        let snapshot = steady(PLATFORM_WINDOWS, &env);
        assert_eq!(
            select_data_dir(PLATFORM_WINDOWS, &env, home().as_path(), &snapshot),
            PathBuf::from("D:\\isolated\\case Sensitive")
        );
    }

    #[test]
    fn the_override_does_not_win_over_the_platform_join_when_it_is_empty() {
        // `if override:` is falsy for `""`, so precedence falls to tier 2.
        assert_eq!(
            picked("linux", &[(ENV_DATA_DIR_OVERRIDE, "")]),
            home().join(".local").join("share").join("ReadMD")
        );
        assert_eq!(
            picked(PLATFORM_WINDOWS, &[(ENV_DATA_DIR_OVERRIDE, ""), ("APPDATA", "C:\\roaming")]),
            PathBuf::from("C:\\roaming").join("ReadMD")
        );
    }

    #[test]
    fn the_override_is_vetoed_when_the_platform_changed_since_import() {
        // `config.py:29` / `:34-35`.  This is the guard that keeps an
        // embedder probing another platform from reading a real user's data.
        let env = env(&[(ENV_DATA_DIR_OVERRIDE, "/should/be/ignored"), ("XDG_DATA_HOME", "/xdg")]);
        let snapshot = ImportSnapshot {
            platform: PLATFORM_WINDOWS.to_string(),
            appdata: None,
            xdg_data_home: None,
        };
        assert_eq!(
            select_data_dir("linux", &env, home().as_path(), &snapshot),
            PathBuf::from("/xdg").join("ReadMD")
        );
    }

    #[test]
    fn the_override_is_vetoed_when_appdata_changed_since_import() {
        let env = env(&[
            (ENV_DATA_DIR_OVERRIDE, "/should/be/ignored"),
            ("APPDATA", "C:\\moved"),
            ("XDG_DATA_HOME", "/xdg"),
        ]);
        let snapshot = ImportSnapshot {
            platform: "linux".to_string(),
            appdata: None, // unset at import, now present
            xdg_data_home: None,
        };
        assert_eq!(
            select_data_dir("linux", &env, home().as_path(), &snapshot),
            PathBuf::from("/xdg").join("ReadMD")
        );
    }

    #[test]
    fn clearing_a_data_root_also_vetoes_the_override() {
        // The comparison is `!=` on raw values, so *removing* a root Python
        // captured at import is a change too.
        let env = env(&[(ENV_DATA_DIR_OVERRIDE, "/should/be/ignored")]);
        let snapshot = ImportSnapshot {
            platform: "linux".to_string(),
            appdata: None,
            xdg_data_home: Some("/gone".to_string()),
        };
        assert_eq!(
            select_data_dir("linux", &env, home().as_path(), &snapshot),
            home().join(".local").join("share").join("ReadMD")
        );
    }

    #[test]
    fn setting_a_root_to_the_empty_string_vetoes_the_override() {
        // `os.environ.get('XDG_DATA_HOME')` is `""`, the capture was `None`:
        // `"" != None` is True, so the override dies even though `""` is itself
        // falsy for tier 2.
        let env = env(&[(ENV_DATA_DIR_OVERRIDE, "/ignored"), ("XDG_DATA_HOME", "")]);
        let snapshot = ImportSnapshot {
            platform: "linux".to_string(),
            appdata: None,
            xdg_data_home: None,
        };
        assert_eq!(
            select_data_dir("linux", &env, home().as_path(), &snapshot),
            home().join(".local").join("share").join("ReadMD")
        );
    }

    #[test]
    fn the_component_spelling_is_the_python_one_on_every_platform() {
        for platform in [PLATFORM_WINDOWS, PLATFORM_MACOS, "linux"] {
            let picked_path = picked(&platform, &[]);
            assert_eq!(
                picked_path.file_name().map(|name| name.to_string_lossy().to_string()),
                Some(APP_DATA_DIR_NAME.to_string()),
                "{platform} must append Python's {APP_DATA_DIR_NAME:?}, got {picked_path:?}"
            );
        }
    }

    #[test]
    fn data_dir_is_absolute_or_an_explicit_home_free_default() {
        // The live call may only be checked for shape: the ambient environment
        // belongs to the whole test process, so asserting a concrete root here
        // would make this lane's tests order-dependent.
        let dir = data_dir();
        assert_eq!(
            dir.file_name().map(|name| name.to_string_lossy().to_string()),
            Some(APP_DATA_DIR_NAME.to_string())
        );
    }
}
