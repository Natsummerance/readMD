// `src/readmd_modules/system_native.py` + `windows_native.py` + `linux_native.py`
// + `macos_native.py` — the platform-information layer.
//
// # Parity contract
//
// The authority is the Python source on `main`; nothing here calls into it.
// Every branch Python decides from a *string* (an environment value, an
// `/etc/os-release` body, a `/proc/cpuinfo` body, a `platform.version()`
// rendering) is reproduced by a pure function taking that string as an
// argument, so the tables captured from Python under `scratch/rust_parity/
// native_system_s4/` can be asserted directly on any host.  Only the thin
// readers around them are `#[cfg]`-gated.
//
// # Why `std::env::consts::ARCH` is not a substitute
//
// `env::consts::ARCH` is a *compile-time constant of the Rust target triple*.
// `windows_native.architecture()` reports the *native* CPU identity of the host
// as seen from the running process — a different quantity:
//
//   * ARM64 Windows: Rust `ARCH` is `"aarch64"`, Python reports `"arm64"`.
//   * x64-emulated-on-ARM64 / WOW64: `ARCH` reports what the binary was
//     compiled for (`"x86_64"`/`"x86"`), Python reports the *host* read from
//     `PROCESSOR_ARCHITEW6432` (`"arm64"`/`"x86_64"`).
//   * `system_native`'s no-flavour path reports
//     `(platform.machine() or 'unknown').lower()`, i.e. `"amd64"` on this host;
//     `ARCH` can never produce that string, nor `"unknown"`.
//   * Linux `architecture()` folds `machine` into ReadMD's own vocabulary
//     (`armv7l` -> `arm`, `mips64` -> `mips64el`, `sw_64` -> `sw64`); `ARCH`
//     reports none of those (and spells RISC-V `riscv64gc`).
//
// Values are therefore computed from `PROCESSOR_ARCHITEW6432`,
// `PROCESSOR_ARCHITECTURE` and `platform.machine()` exactly as Python reads
// them, never from a compile-time constant.
//
// # FFI budget
//
// `advapi32` (registry probes), `shell32` (`ShellExecuteW` = `os.startfile`)
// and — only on the non-Windows `#[cfg]` branches — `libc::uname`.  Same
// precedent as the hand-written `extern "system"` blocks in `crypto.rs`.
// `windows_native.show_error()` needs `user32!MessageBoxW`, which is outside
// the budget and is therefore NOT ported (see `windows_show_error`).

#![allow(dead_code)]

use std::process::{Command, Stdio};

// ---------------------------------------------------------------------------
// Python-level constants
// ---------------------------------------------------------------------------

/// `windows_native.IS_WIN` / `linux_native.IS_LINUX` / `macos_native.IS_MAC`.
pub const IS_WIN: bool = cfg!(windows);
pub const IS_LINUX: bool = cfg!(target_os = "linux");
pub const IS_MAC: bool = cfg!(target_os = "macos");

/// `sys.platform`: `'win32'`, `'darwin'`, `'linux'`, or the raw value (e.g.
/// `'freebsd13'`), which matches no branch of `get_current_platform_flavor()`.
pub fn sys_platform() -> &'static str {
    if cfg!(windows) {
        "win32"
    } else if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "other"
    }
}

/// `"=" * 64` / `"-" * 64`, the rule used by every report.  // WIRING: rule
fn rule(ch: char) -> String {
    ch.to_string().repeat(64)
}

// ---------------------------------------------------------------------------
// Shared private helpers (Python stdlib stand-ins)
// ---------------------------------------------------------------------------

/// `os.environ.get(name)` — keeps *absent* distinct from *present but empty*.
/// These modules use both `environ.get(k, default)` and `environ.get(k) or …`,
/// which disagree on an empty value.  // WIRING: env_get
pub fn env_get(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

/// This process' environment as the injectable helpers want it.
/// // WIRING: env snapshots for the pure halves of the Windows probes
pub fn current_env() -> EnvMap {
    std::env::vars().collect::<EnvMap>()
}

/// Read a captured snapshot the way `os.environ.get` reads the live one: on
/// Windows key case is normalised away (`ntpath.normcase`), on POSIX it is not.
/// `std::env::vars()` hands back the environment block's own casing, which is
/// not the casing Python's `os.environ` stores, so an exact `BTreeMap` get would
/// miss.  // WIRING: env_get_in
pub fn env_get_in(env: &EnvMap, name: &str) -> Option<String> {
    if let Some(value) = env.get(name) {
        return Some(value.clone());
    }
    if cfg!(windows) {
        let want = normcase(name);
        for (key, value) in env {
            if normcase(key) == want {
                return Some(value.clone());
            }
        }
    }
    None
}

/// `os.environ.get(name, default)` against a captured snapshot.
pub fn env_lookup(env: &EnvMap, name: &str, default: &str) -> String {
    env_get_in(env, name).unwrap_or_else(|| default.to_string())
}

/// `os.environ.get(name, default)` — a present-but-empty value wins over the
/// default, exactly like Python.  // WIRING: env_get_default
pub fn env_get_default(name: &str, default: &str) -> String {
    env_get(name).unwrap_or_else(|| default.to_string())
}

/// `str.upper()`.  // WIRING: py_upper
pub fn py_upper(text: &str) -> String {
    text.to_uppercase()
}

/// `str.lower()`.  // WIRING: py_lower
pub fn py_lower(text: &str) -> String {
    text.to_lowercase()
}

/// `str.strip()` with no argument.  CPython's `Py_UNICODE_ISSPACE` is the
/// Unicode `White_Space` property **plus** `U+001C..U+001F` (file/group/record
/// unit separators), which `char::is_whitespace` leaves out; the captured set in
/// `scratch/rust_parity/native_system_s4/py_extra_truth.json`
/// (`strip_codepoints`) is `[9,13] [28,32] [133] [160] [5760] [8192,8202]
/// [8232,8233] [8239] [8287] [12288]`, i.e. exactly
/// `White_Space ∪ {U+1C..U+1F}`.  // WIRING: py_strip
pub fn py_strip(text: &str) -> &str {
    text.trim_matches(|c: char| c.is_whitespace() || matches!(c, '\u{1c}'..='\u{1f}'))
}

/// Every code-point run of Unicode category `Nd` (decimal digit), which is what
/// Python's `re` `\d` and `unicodedata.decimal()` accept and what
/// `char::to_digit(10)` — ASCII only — does not.
///
/// Measured over the whole BMP-plus-astral range in CPython 3.11.15
/// (`py_extra_truth.json` → `nd_blocks`, `nd_blocks_all_ten`): there are 66
/// runs, each exactly ten code points wide and each starting at digit value
/// `0`.  Membership and value therefore reduce to a binary search over the 66
/// starts.  // WIRING: unicode_decimal_digit
const ND_BLOCK_STARTS: [u32; 66] = [
    48, 1632, 1776, 1984, 2406, 2534, 2662, 2790,
    2918, 3046, 3174, 3302, 3430, 3558, 3664, 3792,
    3872, 4160, 4240, 6112, 6160, 6470, 6608, 6784,
    6800, 6992, 7088, 7232, 7248, 42528, 43216, 43264,
    43472, 43504, 43600, 44016, 65296, 66720, 68912, 69734,
    69872, 69942, 70096, 70384, 70736, 70864, 71248, 71360,
    71472, 71904, 72016, 72784, 73040, 73120, 92768, 92864,
    93008, 120782, 120792, 120802, 120812, 120822, 123200, 123632,
    125264, 130032,
];

/// `unicodedata.decimal(ch)` — the numeric value Python's `\d`/`int()` see for a
/// decimal digit, or `None`.  ASCII `'0'..='9'` is just the first block.
pub fn unicode_decimal_digit(ch: char) -> Option<u32> {
    let cp = ch as u32;
    let index = ND_BLOCK_STARTS.partition_point(|start| *start <= cp);
    if index == 0 {
        return None;
    }
    let start = ND_BLOCK_STARTS[index - 1];
    if cp - start < 10 {
        Some(cp - start)
    } else {
        None
    }
}

/// Code points where `str.isdigit()` is `True` but the category is **not** `Nd`:
/// `Numeric_Type=Digit` members of `No` (superscripts, circled and squared
/// numbers, …).  Together with [`ND_BLOCK_STARTS`] this is exactly the captured
/// `isdigit_codepoints` set (`nd | supplement == isdigit` was verified over all
/// 81 runs), and it is *strictly smaller* than `char::is_numeric()`, which also
/// admits `Nl` (`U+2160` ROMAN NUMERAL ONE) that Python's `isdigit()` rejects.
/// // WIRING: py_isdigit
const DIGIT_SUPPLEMENT_RUNS: [(u32, u32); 20] = [
    (178, 179),
    (185, 185),
    (4969, 4977),
    (6618, 6618),
    (8304, 8304),
    (8308, 8313),
    (8320, 8329),
    (9312, 9320),
    (9332, 9340),
    (9352, 9360),
    (9450, 9450),
    (9461, 9469),
    (9471, 9471),
    (10102, 10110),
    (10112, 10120),
    (10122, 10130),
    (68160, 68163),
    (69216, 69224),
    (69714, 69722),
    (127232, 127242),
];

/// `ch.isdigit()`.
fn char_isdigit(ch: char) -> bool {
    if unicode_decimal_digit(ch).is_some() {
        return true;
    }
    let cp = ch as u32;
    DIGIT_SUPPLEMENT_RUNS
        .iter()
        .any(|(first, last)| cp >= *first && cp <= *last)
}

/// `re.search(r'\d', text)` — the Unicode-`Nd` digit test every regex in these
/// modules uses.  Call sites previously wrote `is_ascii_digit()` or
/// `to_digit(10)`, which silently disagree with Python on e.g. `'٣'`
/// (ARABIC-INDIC DIGIT THREE, whose `to_digit(10)` is `None`).
fn py_re_digit(ch: char) -> bool {
    unicode_decimal_digit(ch).is_some()
}

/// `ntpath.join(first, second)` as CPython 3.11's `ntpath.join` body behaves:
/// a rooted second part restarts the path but keeps the first part's drive when
/// the second has none; a second part carrying a *different* root (compared
/// case-insensitively) discards the first part entirely, while the *same* root
/// is a relative append; an empty second part still appends a separator; and a
/// drive-relative root (`'C:'`) joins without one.  // WIRING: ntpath_join
pub fn ntpath_join(first: &str, second: &str) -> String {
    let (mut drive, mut path) = nt_split_drive(first);
    let (p_drive, p_path) = nt_split_drive(second);
    let head = p_path.chars().next();
    let mut restart = false;
    if head == Some('\\') || head == Some('/') {
        // `second` is rooted: it wins, and only `first`'s drive survives if it has none.
        if !p_drive.is_empty() || drive.is_empty() {
            drive = p_drive;
        }
        restart = true;
    } else if !p_drive.is_empty() && p_drive != drive {
        if p_drive.to_lowercase() != drive.to_lowercase() {
            // Different roots => ignore the first path entirely.
            drive = p_drive;
            restart = true;
        } else {
            // Same root in a different case.
            drive = p_drive;
        }
    }
    if restart {
        path = p_path;
    } else {
        // `second` is relative to `first`.
        if matches!(path.chars().last(), Some(last) if last != '\\' && last != '/') {
            path.push('\\');
        }
        path.push_str(&p_path);
    }
    // Add a separator between a UNC root and a non-absolute path.
    let rooted = matches!(path.chars().next(), Some('\\') | Some('/'));
    if !path.is_empty() && !rooted && !drive.is_empty() && drive.chars().last() != Some(':') {
        return format!("{}\\{}", drive, path);
    }
    format!("{}{}", drive, path)
}

/// `posixpath.join(first, second)`: only a leading `/` restarts, an empty part
/// still gets its separator.  // WIRING: posix_join
pub fn posixpath_join(first: &str, second: &str) -> String {
    if second.starts_with('/') {
        return second.to_string();
    }
    if second.is_empty() {
        if first.is_empty() || first.ends_with('/') {
            return first.to_string();
        }
        return format!("{}/", first);
    }
    if first.is_empty() || first.ends_with('/') {
        return format!("{}{}", first, second);
    }
    format!("{}/{}", first, second)
}

/// `os.path.join` for the platform being built.  Both spellings are kept
/// separate (and testable) because `shutil.which`, `tempfile` and
/// `probe_webview2_installed` all go through the *platform's* `os.path.join`.
/// // WIRING: path_join
pub fn path_join(first: &str, second: &str) -> String {
    if cfg!(windows) {
        ntpath_join(first, second)
    } else {
        posixpath_join(first, second)
    }
}

/// `os.path.exists(path)`.  // WIRING: path_exists
pub fn path_exists(path: &str) -> bool {
    !path.is_empty() && std::path::Path::new(path).exists()
}

/// `os.path.isfile(path)`.  // WIRING: path_is_file
pub fn path_is_file(path: &str) -> bool {
    !path.is_empty() && std::path::Path::new(path).is_file()
}

/// `os.path.isdir(path)`.  // WIRING: path_is_dir
pub fn path_is_dir(path: &str) -> bool {
    !path.is_empty() && std::path::Path::new(path).is_dir()
}

/// `os.access(path, os.X_OK)`.  Windows has no execute bit, so CPython's
/// `_access` reduces to "exists and is not a directory".  // WIRING: path_is_executable
pub fn path_is_executable(path: &str) -> bool {
    if path.is_empty() {
        return false;
    }
    let item = std::path::Path::new(path);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if cfg!(unix) {
            return match item.metadata() {
                Ok(meta) => meta.is_file() && meta.permissions().mode() & 0o111 != 0,
                Err(_) => false,
            };
        }
    }
    item.is_file()
}

/// `os.listdir(dir)` in the order Windows' directory index returns it.  NTFS
/// enumerates case-insensitively by filename while `read_dir` gives raw OS
/// order, so the case-folded sort reproduces what Python sees — which is what
/// the `EdgeWebView` "first `N.N.N.N` child" probe depends on.  // WIRING: list_dir
pub fn list_dir(path: &str) -> Vec<String> {
    let mut names: Vec<String> = match std::fs::read_dir(path) {
        Ok(entries) => entries
            .flatten()
            .filter_map(|entry| entry.file_name().into_string().ok())
            .collect(),
        Err(_) => return Vec::new(),
    };
    names.sort_by(|a, b| py_lower(a).cmp(&py_lower(b)));
    names
}

/// `os.path.normcase` — on Windows, `/` becomes `\` and the path case-folds;
/// elsewhere it is the identity.  Public because `convert.rs::py_normcase` was a
/// second copy of exactly this function and now delegates here.  // WIRING: normcase
pub fn normcase(path: &str) -> String {
    if cfg!(windows) {
        py_lower(&path.replace('/', "\\"))
    } else {
        path.to_string()
    }
}

/// The `os.pathsep` used to split `PATH` and `PATHEXT`.  // WIRING: path_sep
fn path_sep() -> char {
    if cfg!(windows) {
        ';'
    } else {
        ':'
    }
}

/// `shutil._WIN_DEFAULT_PATHEXT` (measured), used when `PATHEXT` is unset *or*
/// empty.
pub const WIN_DEFAULT_PATHEXT: &str = ".COM;.EXE;.BAT;.CMD;.VBS;.JS;.WS;.MSC";

/// `shutil.which(cmd)` as a pure function of `PATH`, `PATHEXT` and an
/// `_access_check` stand-in.  CPython 3.11's rules, each one pinned by
/// `py_branch_truth4.json` (`which_fed`):
///
/// * a `cmd` with a directory part is looked up *verbatim*: the answer is that
///   `cmd` when the access check passes and `None` when it does not — PATH is
///   never consulted either way;
/// * an empty `PATH` means "no match", an *absent* one means `os.defpath`;
/// * on Windows `os.curdir` is inserted at the head of the search path when it
///   is not already there;
/// * `PATHEXT` is `getenv('PATHEXT') or _WIN_DEFAULT_PATHEXT`, so a present but
///   empty value also falls back;
/// * candidates are `cmd + ext` **with the extension's own case**, and the
///   verbatim short-circuit is the only case-insensitive test;
/// * `PATH` entries are `os.path.normcase`-de-duplicated, in order.
/// // WIRING: shutil_which
pub fn which_from<F>(
    name: &str,
    search_path: Option<&str>,
    pathext: Option<&str>,
    is_windows: bool,
    can_exec: F,
) -> Option<String>
where
    F: Fn(&str) -> bool,
{
    let has_directory = if is_windows {
        name.contains('/') || name.contains('\\')
    } else {
        name.contains('/')
    };
    if has_directory {
        return if can_exec(name) {
            Some(name.to_string())
        } else {
            None
        };
    }
    let raw_path = search_path.unwrap_or(if is_windows { ".;C:\\bin" } else { "/bin:/usr/bin" });
    if raw_path.is_empty() {
        return None;
    }
    let separator = if is_windows { ';' } else { ':' };
    let mut entries: Vec<String> = raw_path.split(separator).map(String::from).collect();
    let candidates: Vec<String> = if is_windows {
        if !entries.iter().any(|entry| entry == ".") {
            entries.insert(0, ".".to_string());
        }
        let source = match pathext {
            Some(value) if !value.is_empty() => value.to_string(),
            _ => WIN_DEFAULT_PATHEXT.to_string(),
        };
        let extensions: Vec<String> = source
            .split(separator)
            .filter(|item| !item.is_empty())
            .map(|item| item.to_string())
            .collect();
        let folded = py_lower(name);
        if extensions
            .iter()
            .any(|ext| folded.ends_with(&py_lower(ext)))
        {
            vec![name.to_string()]
        } else {
            extensions
                .iter()
                .map(|ext| format!("{}{}", name, ext))
                .collect()
        }
    } else {
        vec![name.to_string()]
    };
    let mut seen: Vec<String> = Vec::new();
    for dir in entries {
        let normalized = if is_windows {
            py_lower(&dir.replace('/', "\\"))
        } else {
            dir.clone()
        };
        if seen.contains(&normalized) {
            continue;
        }
        seen.push(normalized);
        for item in &candidates {
            let probe = if is_windows {
                ntpath_join(&dir, item)
            } else {
                posixpath_join(&dir, item)
            };
            if can_exec(&probe) {
                return Some(probe);
            }
        }
    }
    None
}

/// `shutil.which(name)` against this process' environment.  `_access_check` is
/// `os.path.exists(p) and os.access(p, X_OK) and not os.path.isdir(p)`.
pub fn shutil_which(name: &str) -> Option<String> {
    which_from(
        name,
        env_get("PATH").as_deref(),
        env_get("PATHEXT").as_deref(),
        cfg!(windows),
        |probe| path_is_file(probe) && path_is_executable(probe),
    )
}

/// `ntpath.expandvars`: `%NAME%` is substituted case-insensitively and an
/// unknown name is left verbatim, which is why a `%SYSTEMROOT%\Temp` candidate
/// survives as a literal when `SYSTEMROOT` is unset.
pub fn nt_expandvars<F>(text: &str, getenv: F) -> String
where
    F: Fn(&str) -> Option<String>,
{
    let chars: Vec<char> = text.chars().collect();
    let mut result = String::new();
    let mut index = 0usize;
    while index < chars.len() {
        if chars[index] == '%' {
            if let Some(end) = (index + 1..chars.len()).find(|&item| chars[item] == '%') {
                let name: String = chars[index + 1..end].iter().collect();
                if let Some(value) = getenv(&name) {
                    result.push_str(&value);
                    index = end + 1;
                    continue;
                }
            }
        }
        result.push(chars[index]);
        index += 1;
    }
    result
}

/// `os.path.expanduser(r'~\AppData\Local\Temp')` as CPython's `ntpath` does it:
/// `USERPROFILE` wins even when empty, otherwise `HOMEDRIVE` + `HOMEPATH`,
/// otherwise the `~` path is returned **unchanged** (and `HOME` is ignored).
pub fn nt_expanduser_temp_dir<F>(getenv: F) -> String
where
    F: Fn(&str) -> Option<String>,
{
    const TAIL: &str = r"\AppData\Local\Temp";
    let home = match getenv("USERPROFILE") {
        Some(value) => value,
        None => match getenv("HOMEPATH") {
            Some(path) => ntpath_join(&getenv("HOMEDRIVE").unwrap_or_default(), &path),
            None => return format!("~{}", TAIL),
        },
    };
    format!("{}{}", home, TAIL)
}

/// `tempfile._candidate_tempdir_list()` on Windows: the three environment
/// variables (skipped when empty, in `TMPDIR`, `TEMP`, `TMP` order), then the
/// user temp, then `%SYSTEMROOT%\Temp`, then the four well-known directories,
/// then `os.getcwd()` last.  // WIRING: gettempdir
pub fn tempdir_candidates_nt<F>(getenv: F, cwd: &str) -> Vec<String>
where
    F: Fn(&str) -> Option<String>,
{
    let mut dirs: Vec<String> = Vec::new();
    for name in ["TMPDIR", "TEMP", "TMP"] {
        if let Some(value) = getenv(name) {
            if !value.is_empty() {
                dirs.push(value);
            }
        }
    }
    dirs.push(nt_expanduser_temp_dir(&getenv));
    dirs.push(nt_expandvars(r"%SYSTEMROOT%\Temp", &getenv));
    dirs.extend([r"c:\temp", r"c:\tmp", r"\temp", r"\tmp"].map(String::from));
    dirs.push(cwd.to_string());
    dirs
}

/// `tempfile._candidate_tempdir_list()` off Windows: same three variables, then
/// `/tmp`, `/var/tmp`, `/usr/tmp`, then the cwd.
pub fn tempdir_candidates_posix<F>(getenv: F, cwd: &str) -> Vec<String>
where
    F: Fn(&str) -> Option<String>,
{
    let mut dirs: Vec<String> = Vec::new();
    for name in ["TMPDIR", "TEMP", "TMP"] {
        if let Some(value) = getenv(name) {
            if !value.is_empty() {
                dirs.push(value);
            }
        }
    }
    dirs.extend(["/tmp", "/var/tmp", "/usr/tmp"].map(String::from));
    dirs.push(cwd.to_string());
    dirs
}

/// `tempfile._get_default_tempdir()`: the first candidate that survives an
/// actual file creation, *after* `os.path.abspath` has normalised it.
/// `std::env::temp_dir()` is unusable here: Windows' `GetTempPathW` checks `TMP`
/// before `TEMP`, the opposite of Python.
pub fn gettempdir() -> String {
    let cwd = std::env::current_dir()
        .map(|dir| dir.to_string_lossy().into_owned())
        .unwrap_or_else(|_| ".".to_string());
    let lookup = |name: &str| env_get(name);
    let candidates = if cfg!(windows) {
        tempdir_candidates_nt(lookup, &cwd)
    } else {
        tempdir_candidates_posix(lookup, &cwd)
    };
    for candidate in candidates {
        let base = if candidate == "." {
            candidate.clone()
        } else if cfg!(windows) {
            ntpath_abspath(&cwd, &candidate)
        } else {
            posixpath_abspath(&cwd, &candidate)
        };
        let probe = if cfg!(windows) {
            ntpath_join(&base, "readmd-tmp-probe")
        } else {
            posixpath_join(&base, "readmd-tmp-probe")
        };
        if std::fs::File::create(&probe).is_ok() {
            let _ = std::fs::remove_file(&probe);
            return base;
        }
    }
    cwd
}

/// `ntpath.normpath` / `posixpath.normpath` (platform-selected).  Both collapse
/// repeated separators, drop `.` segments and resolve `..` without touching the
/// filesystem.  On Windows the root is immutable — `C:`, `\\server\share`,
/// `\\?\`, `\\.\` — except for a `\\?\UNC\` path, where only the eight-code-point
/// marker is, so `\\?\C:\a\..\b` → `\\?\C:\b` while
/// `\\?\UNC\server\share\..\..` → `\\?\UNC\`.  // WIRING: normpath
pub fn normpath(path: &str) -> String {
    if cfg!(windows) {
        ntpath_normpath(path)
    } else {
        posixpath_normpath(path)
    }
}

/// The eight code points of a `\\?\UNC\` (extended-length UNC) marker, tested on
/// `p.replace(altsep, sep)` case-insensitively: Python uses `.upper()`, and
/// scanning every code point of U+0000..U+10FFFF against `eq_ignore_ascii_case`
/// for the only letters in the marker (`U`, `N`, `C`) found no divergence.
fn is_unc_extended(folded: &[char]) -> bool {
    const MARKER: [char; 8] = ['\\', '\\', '?', '\\', 'U', 'N', 'C', '\\'];
    folded.len() >= MARKER.len()
        && (0..MARKER.len()).all(|i| folded[i].eq_ignore_ascii_case(&MARKER[i]))
}

/// `ntpath.splitdrive` (CPython 3.11), transcribed.  Handles `C:`,
/// `\\server\share`, `\\?\C:`, `\\?\UNC\server\share` and `\\.\Device`; when a
/// `\\`-rooted path has no second separator component the whole input *is* the
/// root and the tail is empty.  Indices are code points (Python indexes code
/// points, not UTF-8 bytes) and both halves are sliced from the original `path`,
/// so the root keeps whatever slash characters it was written with and the tail
/// keeps its leading separator.
fn nt_split_drive(path: &str) -> (String, String) {
    const SEP: char = '\\';
    const ALTSEP: char = '/';
    let chars: Vec<char> = path.chars().collect();
    // `p.replace(altsep, sep)`: only used to *find* separators, never to slice.
    let normp: Vec<char> =
        chars.iter().map(|c| if *c == ALTSEP { SEP } else { *c }).collect();
    let find_sep = |from: usize| (from..normp.len()).find(|&i| normp[i] == SEP);
    if normp.len() >= 2 && normp[0] == SEP && normp[1] == SEP {
        // UNC `\\server\share`, extended `\\?\path` and device `\\.\path` roots.
        let start = if is_unc_extended(&normp) { 8 } else { 2 };
        let index = match find_sep(start) {
            Some(index) => index,
            None => return (path.to_string(), String::new()),
        };
        let index2 = match find_sep(index + 1) {
            Some(index2) => index2,
            None => return (path.to_string(), String::new()),
        };
        return (
            chars[..index2].iter().collect(),
            chars[index2..].iter().collect(),
        );
    }
    if normp.len() >= 2 && normp[1] == ':' {
        return (chars[..2].iter().collect(), chars[2..].iter().collect());
    }
    (String::new(), path.to_string())
}

/// Segment fold shared by both normpath implementations: drop empty and `.`
/// segments, resolve `..` against what survived, and drop a leading `..` only
/// when `absolute` (the root cannot be popped).  `keep_leading_curdir` seeds the
/// stack with a drive-relative path's leading `.`, which measured behaviour shows
/// to be a real segment: `C:.\e` → `C:.\e` but `C:.\..` → `C:` and
/// `C:.\..\a` → `C:a`.
fn fold_segments<'a>(
    segments: &[&'a str],
    absolute: bool,
    keep_leading_curdir: bool,
) -> Vec<&'a str> {
    let mut parts: Vec<&'a str> = Vec::new();
    let mut segments = segments;
    if keep_leading_curdir {
        parts.push(".");
        segments = &segments[1..];
    }
    for segment in segments {
        if segment.is_empty() || *segment == "." {
            continue;
        }
        if *segment == ".." {
            match parts.last() {
                Some(last) if *last != ".." => {
                    parts.pop();
                }
                _ => {
                    if !absolute {
                        parts.push("..");
                    }
                }
            }
            continue;
        }
        parts.push(segment);
    }
    parts
}

/// The root `ntpath.normpath` protects on Windows.  Unlike `nt_split_drive` it is
/// *written* with separators folded (`//a//b` → `\\a\\b`), except for a `X:`
/// drive, which is copied verbatim from the input (`/:` → `/:`, `c:/a/../b` →
/// `c:\b`).  A `\\?\UNC\` path protects only those eight code points, so
/// `server` and `share` fold like ordinary segments — which a plain
/// `\\server\share` root does not.
fn nt_normpath_root(path: &str) -> NtRoot {
    const SEP: char = '\\';
    let chars: Vec<char> = path.chars().collect();
    let normp: Vec<char> = chars.iter().map(|c| if *c == '/' { SEP } else { *c }).collect();
    let find_sep = |from: usize| (from..normp.len()).find(|&i| normp[i] == SEP);
    let whole = || NtRoot { prefix: normp.iter().collect(), tail: chars.len(), extended_unc: false };
    if normp.len() >= 2 && normp[0] == SEP && normp[1] == SEP {
        if is_unc_extended(&normp) {
            return NtRoot { prefix: normp[..8].iter().collect(), tail: 8, extended_unc: true };
        }
        let index = match find_sep(2) {
            Some(index) => index,
            None => return whole(),
        };
        let index2 = match find_sep(index + 1) {
            Some(index2) => index2,
            None => return whole(),
        };
        return NtRoot { prefix: normp[..index2].iter().collect(), tail: index2, extended_unc: false };
    }
    if normp.len() >= 2 && normp[1] == ':' {
        return NtRoot { prefix: chars[..2].iter().collect(), tail: 2, extended_unc: false };
    }
    NtRoot::default()
}

/// The root part `nt_normpath_root` protects, as code points.
#[derive(Default)]
struct NtRoot {
    prefix: String,
    /// Code-point offset of the tail within `path.replace(altsep, sep)`.
    tail: usize,
    /// `true` when the root is a `\\?\UNC\` marker rather than a `\\server\share`
    /// style root, so it already ends in a separator and must not gain another.
    extended_unc: bool,
}

fn ntpath_normpath(path: &str) -> String {
    // ntpath.normpath: cut the immutable root off, collapse the tail's leading
    // separators into it, then fold only the tail.  `..` can therefore never
    // escape into `C:`/`\\server\share`/`\\?\`/`\\.\`, and a UNC root keeps its
    // trailing separator.
    let root = nt_normpath_root(path);
    let normp: Vec<char> = path.chars().map(|c| if c == '/' { '\\' } else { c }).collect();
    let mut rest: String = normp[root.tail..].iter().collect();
    let mut prefix = root.prefix;
    if rest.starts_with('\\') {
        if !root.extended_unc {
            prefix.push('\\');
        }
        rest = rest.trim_start_matches('\\').to_string();
    }
    let absolute = root.extended_unc || prefix.ends_with('\\');
    let segments = rest.split('\\').collect::<Vec<&str>>();
    let keep_leading_curdir = !prefix.is_empty() && !absolute && segments.first() == Some(&".");
    let tail = fold_segments(&segments, absolute, keep_leading_curdir).join("\\");
    if prefix.is_empty() {
        // `'.'` is what an entirely folded-away relative path means.
        return if tail.is_empty() { ".".to_string() } else { tail };
    }
    format!("{}{}", prefix, tail)
}

/// `posixpath.normpath`.  // WIRING: posix_normpath
fn posixpath_normpath(path: &str) -> String {
    if path.is_empty() {
        return ".".to_string();
    }
    let absolute = path.starts_with('/');
    let double = path.starts_with("//") && !path.starts_with("///");
    let joined =
        fold_segments(&path.split('/').collect::<Vec<&str>>(), absolute, false).join("/");
    if !absolute {
        if joined.is_empty() {
            ".".to_string()
        } else {
            joined
        }
    } else if double {
        // POSIX leaves `//` implementation-defined; posixpath keeps both slashes.
        format!("//{}", joined)
    } else {
        format!("/{}", joined)
    }
}

/// `ntpath.isabs` (CPython 3.11): only the first three characters are looked at,
/// after `/` → `\`, and *either* a leading separator or a `:\` at index 1 counts
/// — so `/x` is absolute (a documented legacy bug) while `C:x` is not.  It is
/// deliberately not "`splitdrive` left a tail": a `\\server\share` or `\\?\C:`
/// root consumes the whole input yet is absolute.
/// // WIRING: nt_isabs
fn nt_isabs(path: &str) -> bool {
    let head: Vec<char> = path.chars().take(3).collect::<String>().replace('/', "\\").chars().collect();
    head.first() == Some(&'\\') || (head.get(1) == Some(&':') && head.get(2) == Some(&'\\'))
}

/// `ntpath.abspath` given the cwd: `normpath(join(cwd, path))` unless
/// `isabs(path)`.
/// // WIRING: abspath
pub fn ntpath_abspath(cwd: &str, path: &str) -> String {
    if nt_isabs(path) {
        return ntpath_normpath(path);
    }
    ntpath_normpath(&ntpath_join(cwd, path))
}

/// `posixpath.abspath` given the cwd.
pub fn posixpath_abspath(cwd: &str, path: &str) -> String {
    if path.starts_with('/') {
        return posixpath_normpath(path);
    }
    posixpath_normpath(&posixpath_join(cwd, path))
}

/// `os.path.abspath`.  `macos_native` uses it for `open`/`open -R`, `tempfile`
/// for every candidate directory.
pub fn abspath(path: &str) -> String {
    let cwd = std::env::current_dir()
        .map(|dir| dir.to_string_lossy().into_owned())
        .unwrap_or_else(|_| ".".to_string());
    if cfg!(windows) {
        ntpath_abspath(&cwd, path)
    } else {
        posixpath_abspath(&cwd, path)
    }
}

/// `os.path.basename` over POSIX separators.  // WIRING: basename
pub fn basename_posix(path: &str) -> &str {
    match path.rfind('/') {
        Some(index) => &path[index + 1..],
        None => path,
    }
}

/// `os.path.dirname` over POSIX separators.  // WIRING: dirname
pub fn dirname_posix(path: &str) -> String {
    match path.rfind('/') {
        Some(0) => "/".to_string(),
        Some(index) => path[..index].to_string(),
        None => String::new(),
    }
}

/// `os.makedirs(dir, exist_ok=True)`.  // WIRING: ensure_dir
fn ensure_dir(path: &str) {
    if !path.is_empty() && !path_is_dir(path) {
        let _ = std::fs::create_dir_all(path);
    }
}

/// `logging.info/debug/warning(...)`.  The wiring pass can re-point these at
/// `log::` without touching a call site.  // WIRING: native_log
fn native_log(_level: &str, _message: &str) {}

/// `subprocess.Popen(argv, stdout=DEVNULL, stderr=DEVNULL)`.  `new_session`
/// mirrors `start_new_session=True`; returns the pid, Python's `Popen` stand-in.
/// // WIRING: spawn_detached
fn spawn_detached(command: &[String], new_session: bool) -> Option<u32> {
    if command.is_empty() {
        return None;
    }
    let mut child = Command::new(&command[0]);
    child.args(&command[1..]);
    if new_session {
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            child.process_group(0);
        }
    }
    child
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()
        .map(|proc| proc.id())
}

/// `subprocess.check_output(argv, stderr=DEVNULL, timeout=1)`; `None` stands for
/// the `CalledProcessError`/`TimeoutExpired` Python swallows.  // WIRING: run_captured
fn run_captured(command: Vec<String>) -> Option<String> {
    let output = Command::new(&command[0])
        .args(&command[1..])
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// `open(path, encoding='utf-8', errors='ignore')` behind `os.path.exists`.
/// // WIRING: read_text_lossy
fn read_text_lossy(path: &str) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    Some(decode_ignore(&bytes))
}

/// `bytes.decode('utf-8', errors='ignore')` — invalid sequences disappear rather
/// than becoming U+FFFD, which matters for `/proc` content.  // WIRING: decode_ignore
fn decode_ignore(bytes: &[u8]) -> String {
    let mut kept: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut index = 0usize;
    while index < bytes.len() {
        match std::str::from_utf8(&bytes[index..]) {
            Ok(_) => {
                kept.extend_from_slice(&bytes[index..]);
                break;
            }
            Err(error) => {
                let valid = error.valid_up_to();
                if valid > 0 {
                    kept.extend_from_slice(&bytes[index..index + valid]);
                }
                index += valid;
                match error.error_len() {
                    Some(length) => index += length,
                    None => break,
                }
            }
        }
    }
    String::from_utf8(kept).unwrap_or_default()
}

/// `open(path, 'rb').read(n)`.  // WIRING: read_first_bytes
fn read_first_bytes(path: &str, limit: usize) -> Option<Vec<u8>> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).ok()?;
    let mut buffer = vec![0u8; limit];
    let read = file.read(&mut buffer).ok()?;
    buffer.truncate(read);
    Some(buffer)
}

// ---------------------------------------------------------------------------
// `platform.machine()`
// ---------------------------------------------------------------------------

/// `platform.machine()`.
///
/// On Windows CPython never calls `uname` (it does not exist): `uname()` falls
/// back to `_get_machine_win32()`, which is
/// `environ.get('PROCESSOR_ARCHITEW6432','') or environ.get('PROCESSOR_ARCHITECTURE','')`.
/// Note the `or`: a *present but empty* `PROCESSOR_ARCHITEW6432` falls through
/// here, while `windows_native.architecture()` uses `.get(k, default)` and does
/// **not** fall through.  Both spellings are reproduced.
/// On Linux/macOS it is `os.uname().machine`.
pub fn platform_machine() -> String {
    #[cfg(windows)]
    {
        machine_win32_from(
            env_get("PROCESSOR_ARCHITEW6432").as_deref(),
            env_get("PROCESSOR_ARCHITECTURE").as_deref(),
        )
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        uname_machine()
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        String::new()
    }
}

/// `_get_machine_win32()` as a pure function of the two environment values:
/// `environ.get('PROCESSOR_ARCHITEW6432','') or environ.get('PROCESSOR_ARCHITECTURE','')`.
/// The `or` is what makes a *present but empty* `PROCESSOR_ARCHITEW6432` fall
/// through to `PROCESSOR_ARCHITECTURE` — the opposite of
/// `windows_architecture_from()`, which uses `environ.get(k, default)`.
/// // WIRING: machine_win32_from
pub fn machine_win32_from(arch6432: Option<&str>, arch: Option<&str>) -> String {
    let first = arch6432.unwrap_or("");
    if first.is_empty() {
        arch.unwrap_or("").to_string()
    } else {
        first.to_string()
    }
}

/// `os.uname().machine` via the `uname(2)` syscall.  // WIRING: uname_machine
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn uname_machine() -> String {
    #[repr(C)]
    struct Utsname {
        fields: [u8; 6 * 256],
    }
    extern "C" {
        fn uname(buf: *mut Utsname) -> i32;
    }
    // SAFETY: `Utsname` is six fixed 256-byte arrays, exactly the layout
    // uname(2) writes, and it is owned by this frame for the whole call.
    unsafe {
        let mut buf = Utsname {
            fields: [0u8; 6 * 256],
        };
        if uname(&mut buf) != 0 {
            return String::new();
        }
        let start = 4 * 256; // machine is the 5th field on both platforms
        let slice = &buf.fields[start..start + 256];
        let end = slice.iter().position(|byte| *byte == 0).unwrap_or(slice.len());
        String::from_utf8_lossy(&slice[..end]).into_owned()
    }
}

// ---------------------------------------------------------------------------
// `src/readmd_modules/windows_native.py`
// ---------------------------------------------------------------------------

/// `windows_native.is_windows()`.
pub fn windows_is_windows() -> bool {
    IS_WIN
}

/// The dict behind `windows_native.get_windows_version_info()`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowsVersionInfo {
    /// Python keeps arbitrary-precision ints here, and the captured oracle
    /// contains the build `99999999999999999999`; `u32` truncated that to
    /// `None` and silently reported `Windows 10.0.99999999999999999999` as an
    /// unparsable string.  `u128` covers every value a registry can hold.
    pub major: u128,
    pub minor: u128,
    pub build: u128,
    pub name: String,
    pub is_win7: bool,
    pub is_win11: bool,
}

impl Default for WindowsVersionInfo {
    /// `{'major': 0, 'minor': 0, 'build': 0, 'name': 'Unknown Windows',
    /// 'is_win7': False, 'is_win11': False}`
    fn default() -> Self {
        WindowsVersionInfo {
            major: 0,
            minor: 0,
            build: 0,
            name: "Unknown Windows".to_string(),
            is_win7: false,
            is_win11: false,
        }
    }
}

fn read_number(chars: &[char], pos: &mut usize) -> Option<u128> {
    // `saturating_*` because Python's `int()` is arbitrary-precision and never
    // fails: a build wider than `u128::MAX` must still *match* (and still count
    // as `>= 22000`), it just clamps.  `None` here means "no digits at all",
    // which is the only way `re.match` can fail.
    let mut value = 0u128;
    let mut seen = 0usize;
    while *pos < chars.len() {
        match unicode_decimal_digit(chars[*pos]) {
            Some(digit) => {
                value = value.saturating_mul(10).saturating_add(u128::from(digit));
                *pos += 1;
                seen += 1;
            }
            None => break,
        }
    }
    if seen == 0 {
        None
    } else {
        Some(value)
    }
}

/// `re.match(r'(\d+)\.(\d+)\.(\d+)', ver_str)`.  Python's `\d` is the Unicode Nd
/// category, hence `to_digit(10)` instead of `is_ascii_digit`.  `None` when the
/// anchored pattern does not match, which is what leaves the three zeros in the
/// Python dict.  // WIRING: nt_version_match
pub fn parse_windows_version(ver_str: &str) -> Option<(u128, u128, u128)> {
    let chars: Vec<char> = ver_str.chars().collect();
    let mut pos = 0usize;
    let major = read_number(&chars, &mut pos)?;
    if chars.get(pos) != Some(&'.') {
        return None;
    }
    pos += 1;
    let minor = read_number(&chars, &mut pos)?;
    if chars.get(pos) != Some(&'.') {
        return None;
    }
    pos += 1;
    let build = read_number(&chars, &mut pos)?;
    Some((major, minor, build))
}

/// The name/flag block of `get_windows_version_info()`, given what
/// `platform.version()` rendered.  // WIRING: windows_product_name
pub fn windows_version_info_from_version(ver_str: &str) -> WindowsVersionInfo {
    let mut info = WindowsVersionInfo {
        name: String::new(),
        ..WindowsVersionInfo::default()
    };
    if let Some((major, minor, build)) = parse_windows_version(ver_str) {
        info.major = major;
        info.minor = minor;
        info.build = build;
    }
    if info.major == 10 && info.build >= 22000 {
        info.name = format!("Windows 11 (Build {})", info.build);
        info.is_win11 = true;
    } else if info.major == 10 {
        info.name = format!("Windows 10 (Build {})", info.build);
    } else if info.major == 6 && info.minor == 3 {
        info.name = "Windows 8.1".to_string();
    } else if info.major == 6 && info.minor == 2 {
        info.name = "Windows 8".to_string();
    } else if info.major == 6 && info.minor == 1 {
        info.name = "Windows 7 SP1".to_string();
        info.is_win7 = true;
    } else {
        // `'Windows %s' % ver_str` — the raw string survives, so 'nope' becomes
        // 'Windows nope' and '' becomes 'Windows '.
        info.name = format!("Windows {}", ver_str);
    }
    info
}

/// `windows_native.get_windows_version_info()`.
pub fn get_windows_version_info() -> WindowsVersionInfo {
    if !IS_WIN {
        return WindowsVersionInfo::default();
    }
    windows_version_info_from_version(&platform_version())
}

/// `windows_native.is_win7()`.
pub fn windows_is_win7() -> bool {
    get_windows_version_info().is_win7
}

/// `windows_native.is_win11()`.
pub fn windows_is_win11() -> bool {
    get_windows_version_info().is_win11
}

/// `'%d.%d.%d' % sys.getwindowsversion()[:3]`, i.e. what `platform.version()`
/// returns on Windows.  Read from
/// `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion` because a manifest-less
/// `GetVersionExW` lies (it reports 6.2.9200) and `ntdll!RtlGetVersion` is
/// outside the FFI budget; the three registry values are the same numbers
/// CPython surfaces.  `CurrentMajorVersionNumber` only exists on 8.1+, hence the
/// legacy `CurrentVersion` fallback.  // WIRING: platform_version
pub fn platform_version() -> String {
    if !IS_WIN {
        return String::new();
    }
    const ROOT: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
    let build = registry_string(ROOT, "CurrentBuildNumber")
        .and_then(|text| text.trim().parse::<u32>().ok())
        .unwrap_or(0);
    let (major, minor) = match registry_u32(ROOT, "CurrentMajorVersionNumber") {
        Some(major) => (major, registry_u32(ROOT, "CurrentMinorVersionNumber").unwrap_or(0)),
        None => {
            let legacy = registry_string(ROOT, "CurrentVersion").unwrap_or_default();
            let mut halves = legacy.splitn(2, '.');
            (
                halves
                    .next()
                    .and_then(|item| item.trim().parse::<u32>().ok())
                    .unwrap_or(0),
                halves
                    .next()
                    .and_then(|item| item.trim().parse::<u32>().ok())
                    .unwrap_or(0),
            )
        }
    };
    format!("{}.{}.{}", major, minor, build)
}

/// `windows_native.architecture()`'s decision table driven by the three strings
/// Python reads.  `arch6432`/`arch` are `Option` because the expression is
/// `environ.get('PROCESSOR_ARCHITEW6432', environ.get('PROCESSOR_ARCHITECTURE', ''))`
/// — a *present* empty value shadows the fallback instead of falling through.
/// // WIRING: windows_architecture_from
pub fn windows_architecture_from(
    arch6432: Option<&str>,
    arch: Option<&str>,
    machine: &str,
) -> String {
    let chosen = match arch6432 {
        Some(value) => value.to_string(),
        None => arch.unwrap_or("").to_string(),
    };
    let up = py_upper(&chosen);
    if up.contains("ARM64") {
        return "arm64".to_string();
    }
    if up.contains("64") || up.contains("AMD64") {
        return "x86_64".to_string();
    }
    if up.contains("86") {
        return "x86".to_string();
    }
    let lowered = py_lower(machine);
    if lowered.contains("arm") || lowered.contains("aarch64") {
        return "arm64".to_string();
    }
    "x86_64".to_string()
}

/// `windows_native.architecture()`.  Off Windows Python reports
/// `platform.machine().lower()` — a different vocabulary ('amd64', not
/// 'x86_64'), which is why this is not shared with the Linux function.
pub fn windows_architecture() -> String {
    if !IS_WIN {
        return py_lower(&platform_machine());
    }
    windows_architecture_from(
        env_get("PROCESSOR_ARCHITEW6432").as_deref(),
        env_get("PROCESSOR_ARCHITECTURE").as_deref(),
        &platform_machine(),
    )
}

/// `windows_native.is_arm64()`.
pub fn windows_is_arm64() -> bool {
    windows_architecture() == "arm64"
}

/// The `{'installed', 'version', 'path'}` dict of
/// `windows_native.probe_webview2_installed()`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Webview2Probe {
    pub installed: bool,
    pub version: String,
    pub path: String,
}

impl Webview2Probe {
    fn missing() -> Self {
        Webview2Probe {
            installed: false,
            version: String::new(),
            path: String::new(),
        }
    }
}

/// The three `EdgeWebView\Application` roots Python walks when the registry
/// misses.  // WIRING: webview2_dir_candidates
pub fn webview2_dir_candidates_in(env: &EnvMap) -> Vec<String> {
    const TAIL: &str = r"Microsoft\EdgeWebView\Application";
    // `os.environ.get('LOCALAPPDATA', '')` has no default, so an absent value
    // still joins onto the tail and yields a rooted path.
    vec![
        ntpath_join(
            &env_lookup(env, "ProgramFiles(x86)", r"C:\Program Files (x86)"),
            TAIL,
        ),
        ntpath_join(&env_lookup(env, "ProgramFiles", r"C:\Program Files"), TAIL),
        ntpath_join(&env_lookup(env, "LocalAppData", ""), TAIL),
    ]
}

pub fn webview2_dir_candidates() -> Vec<String> {
    webview2_dir_candidates_in(&current_env())
}

/// `re.match(r'^\d+\.\d+\.\d+\.\d+$', child)` — the whole name must be four
/// dot-separated runs of digits.  // WIRING: is_four_part_version
pub fn is_four_part_version(text: &str) -> bool {
    let mut groups = 0usize;
    let mut digits = 0usize;
    for ch in text.chars() {
        if ch == '.' {
            if digits == 0 {
                return false;
            }
            groups += 1;
            digits = 0;
        } else if py_re_digit(ch) {
            digits += 1;
        } else {
            return false;
        }
    }
    groups == 3 && digits > 0
}

/// The directory fallback of `probe_webview2_installed()`: for each candidate
/// root, `if os.path.isdir(cand)` then the *first* `os.listdir(cand)` child that
/// matches `^\d+\.\d+\.\d+\.\d+$` wins, with `path` = `os.path.join(cand, child)`.
/// Roots that are not directories are skipped without ever being listed, and a
/// directory whose only entries are e.g. `SetupMetrics` yields nothing — both
/// pinned by `py_branch_truth4.json` (`webview2_dir_fallback`).
/// // WIRING: webview2_dir_fallback
pub fn webview2_dir_fallback<G, H>(
    candidates: &[String],
    is_dir: G,
    list_children: H,
) -> Option<Webview2Probe>
where
    G: Fn(&str) -> bool,
    H: Fn(&str) -> Vec<String>,
{
    for candidate in candidates {
        if !is_dir(candidate) {
            continue;
        }
        for child in list_children(candidate) {
            if is_four_part_version(&child) {
                return Some(Webview2Probe {
                    installed: true,
                    version: child.clone(),
                    path: path_join(candidate, &child),
                });
            }
        }
    }
    None
}

/// `probe_webview2_installed()` over injected probes.  The registry triples are
/// in a fixed order and the *first* key whose `pv` is truthy and not
/// `'0.0.0.0'` wins, with `location` — **not** `InstallationFolder` — as the
/// path and `'System Evergreen'` when it is missing or empty.
/// // WIRING: webview2_probe
pub fn probe_webview2_with<F, G, H>(
    guid: &str,
    read_pv: F,
    dir_candidates: &[String],
    is_dir: G,
    list_versions: H,
) -> Webview2Probe
where
    F: Fn(usize, &str) -> (Option<String>, Option<String>),
    G: Fn(&str) -> bool,
    H: Fn(&str) -> Vec<String>,
{
    const PREFIXES: [&str; 3] = [
        r"SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\",
        r"SOFTWARE\Microsoft\EdgeUpdate\Clients\",
        r"Software\Microsoft\EdgeUpdate\Clients\",
    ];
    for (index, prefix) in PREFIXES.iter().enumerate() {
        let subkey = format!("{}{}", prefix, guid);
        let (pv, location) = read_pv(index, &subkey);
        let version = match pv {
            Some(value) => value,
            None => continue,
        };
        // `if ver and ver != '0.0.0.0'`
        if version.is_empty() || version == "0.0.0.0" {
            continue;
        }
        return Webview2Probe {
            installed: true,
            version,
            path: match location {
                Some(text) if !text.is_empty() => text,
                _ => "System Evergreen".to_string(),
            },
        };
    }
    match webview2_dir_fallback(dir_candidates, is_dir, list_versions) {
        Some(probe) => probe,
        None => Webview2Probe::missing(),
    }
}

/// `windows_native.probe_webview2_installed()`.
pub fn probe_webview2_installed() -> Webview2Probe {
    if !IS_WIN {
        return Webview2Probe::missing();
    }
    const GUID: &str = "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
    probe_webview2_with(
        GUID,
        |index, subkey| {
            let root = if index == 2 { HKCU } else { HKLM };
            let pv = match registry_pv_at(root, subkey, "pv") {
                Some(value) => value,
                None => return (None, None),
            };
            (
                Some(pv),
                registry_string_at(root, subkey, "location"),
            )
        },
        &webview2_dir_candidates(),
        path_is_dir,
        list_dir,
    )
}

/// The `find_app_browser()` candidate list with the environment resolved.  The
/// three `PATH` hits are `insert(0, …)` in `msedge, chrome, brave` order, so if
/// all three exist Brave ends up first — that reversal is reproduced.
/// // WIRING: windows_browser_candidates
pub fn windows_browser_candidates_in(
    env: &EnvMap,
    found_in_path: impl Fn(&str) -> Option<String>,
) -> Vec<String> {
    let pf_x86 = env_lookup(env, "ProgramFiles(x86)", r"C:\Program Files (x86)");
    let pf = env_lookup(env, "ProgramFiles", r"C:\Program Files");
    let local = env_lookup(env, "LocalAppData", "");
    let mut candidates = vec![
        path_join(&pf_x86, r"Microsoft\Edge\Application\msedge.exe"),
        path_join(&pf, r"Microsoft\Edge\Application\msedge.exe"),
        path_join(&local, r"Microsoft\Edge\Application\msedge.exe"),
        path_join(&pf, r"Google\Chrome\Application\chrome.exe"),
        path_join(&pf_x86, r"Google\Chrome\Application\chrome.exe"),
        path_join(&local, r"Google\Chrome\Application\chrome.exe"),
        path_join(&pf, r"BraveSoftware\Brave-Browser\Application\brave.exe"),
    ];
    for name in ["msedge.exe", "chrome.exe", "brave.exe"] {
        if let Some(found) = found_in_path(name) {
            candidates.insert(0, found);
        }
    }
    candidates
}

pub fn windows_browser_candidates() -> Vec<String> {
    windows_browser_candidates_in(&current_env(), shutil_which)
}

/// `windows_native.find_app_browser()`.
pub fn windows_find_app_browser() -> Option<String> {
    if !IS_WIN {
        return None;
    }
    windows_browser_candidates()
        .into_iter()
        .find(|item| !item.is_empty() && path_is_file(item) && path_is_executable(item))
}

/// The exact argv Python spawns for the App-mode window.  // WIRING: windows_browser_app_command
pub fn windows_browser_app_command(
    browser: &str,
    url: &str,
    user_data_dir: &str,
    width: u32,
    height: u32,
) -> Vec<String> {
    vec![
        browser.to_string(),
        format!("--app={}", url),
        format!("--user-data-dir={}", user_data_dir),
        format!("--window-size={},{}", width, height),
        "--no-first-run".to_string(),
        "--no-default-browser-check".to_string(),
        "--disable-sync".to_string(),
        "--disable-background-networking".to_string(),
        "--disable-features=Translate".to_string(),
    ]
}

/// `windows_native.launch_browser_app()`.  Python returns the `Popen` object;
/// the pid is the closest Rust equivalent (`None` = "fell back / failed").
/// `CREATE_NO_WINDOW` (`0x08000000`) is preserved.
pub fn windows_launch_browser_app(url: &str, width: u32, height: u32) -> Option<u32> {
    let browser = match windows_find_app_browser() {
        Some(found) => found,
        None => {
            native_log("info", "no app-mode browser; would fall back to webbrowser.open");
            return None;
        }
    };
    let user_data_dir = path_join(&gettempdir(), "readmd_win_app_profile");
    ensure_dir(&user_data_dir);
    let command = windows_browser_app_command(&browser, url, &user_data_dir, width, height);
    native_log("info", &command.join(" "));
    let mut child = Command::new(&command[0]);
    child.args(&command[1..]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        child.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    match child.stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
        Ok(proc) => Some(proc.id()),
        Err(error) => {
            native_log("warning", &format!("Failed to launch Windows browser app: {}", error));
            None
        }
    }
}

/// `windows_native.detect_system_dark_mode()` —
/// `HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize` /
/// `AppsUseLightTheme`, `int(val) == 0`.
///
/// The pure half of that, over `(was an integer value, str(val))`: a *missing*
/// key or value is `False` (Python's `except: return False`), `0` is `True` in
/// either type, `'1'` is `False`, and a `REG_SZ` that `int()` cannot parse —
/// `'garbage'` or `''` — is `False`.  All eight rows of
/// `py_branch_truth4.json` (`dark_mode_branches`) are pinned by
/// `windows_dark_mode_value_semantics`.  // WIRING: windows_dark_mode_from_value
pub fn windows_dark_mode_from_value(is_integer: bool, raw: Option<&str>) -> bool {
    match raw {
        None => false,
        Some(text) => {
            if is_integer {
                text == "0"
            } else {
                py_int_is_zero(text)
            }
        }
    }
}

/// `int(text) == 0`, `False` for anything Python would raise `ValueError` on.
/// CPython's `int(str)` accepts leading/trailing `Py_UNICODE_ISSPACE` (i.e.
/// `py_strip`'s set), one optional sign, Unicode `Nd` digits and single
/// underscores *between* digits; base 10 only, so `'0x0'` raises.
pub fn py_int_is_zero(text: &str) -> bool {
    let body = py_strip(text);
    let digits: Vec<char> = body
        .trim_start_matches(['+', '-'])
        .chars()
        .filter(|ch| *ch != '_')
        .collect();
    if digits.is_empty() || !digits.iter().all(|ch| py_re_digit(*ch)) {
        return false;
    }
    // `'1__0'`, `'_1'` and `'1_'` all raise; the underscore rules only matter
    // once the digits themselves are valid.
    let raw: Vec<char> = body.trim_start_matches(['+', '-']).chars().collect();
    if raw.first() == Some(&'_') || raw.last() == Some(&'_') || raw.windows(2).any(|w| w[0] == '_' && w[1] == '_') {
        return false;
    }
    digits.iter().all(|ch| unicode_decimal_digit(*ch) == Some(0))
}

pub fn windows_detect_system_dark_mode() -> bool {
    if !IS_WIN {
        return false;
    }
    let (is_integer, raw) = registry_value_at(
        HKCU,
        r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
        "AppsUseLightTheme",
    );
    windows_dark_mode_from_value(is_integer, raw.as_deref())
}

/// `windows_native.show_error()`.
///
/// NOT PORTED on Windows: the implementation is
/// `ctypes.windll.user32.MessageBoxW(0, message, title, MB_ICONERROR)`, and
/// `user32` is outside this crate's FFI budget (`kernel32`/`advapi32`/`shell32`
/// only — the rule `crypto.rs` follows).  Off-Windows Python returns `False`,
/// which is what is reproduced, so this never *falsely* claims a dialog was
/// shown.  // WIRING: user32!MessageBoxW(0, message, title, 0x10)
pub fn windows_show_error(title: &str, message: &str) -> bool {
    if !IS_WIN {
        return false;
    }
    let _ = (title, message);
    false
}

/// `windows_native.reveal_path()` — `explorer.exe /select, <normpath>`.
pub fn windows_reveal_path(path: &str) -> bool {
    if !IS_WIN || !path_exists(path) {
        return false;
    }
    Command::new("explorer.exe")
        .arg("/select,")
        .arg(normpath(path))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .is_ok()
}

/// `windows_native.open_path()` — `os.startfile(normpath(path))`, which *is*
/// `shell32!ShellExecuteW(NULL, "open", …)`.
pub fn windows_open_path(path: &str) -> bool {
    if !IS_WIN || !path_exists(path) {
        return false;
    }
    shell_open(&normpath(path))
}

/// Everything `windows_native.diagnose_system()` returns.  `app_browser` keeps
/// Python's `None`-vs-path tri-state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowsDiagnosis {
    pub is_windows: bool,
    pub version_info: WindowsVersionInfo,
    pub architecture: String,
    pub is_arm64: bool,
    pub webview2: Webview2Probe,
    pub app_browser: Option<String>,
    pub system_dark_mode: bool,
    pub preferred_backend: String,
    pub status: String,
}

/// `windows_native.diagnose_system()`.
pub fn windows_diagnose_system() -> WindowsDiagnosis {
    let webview2 = probe_webview2_installed();
    let app_browser = windows_find_app_browser();
    let architecture = windows_architecture();
    let preferred_backend = if webview2.installed {
        "webview2"
    } else if app_browser.is_some() {
        "browser-app"
    } else {
        "default-browser"
    };
    let status = if webview2.installed || app_browser.is_some() {
        "ready"
    } else {
        "degraded"
    };
    WindowsDiagnosis {
        is_windows: IS_WIN,
        version_info: get_windows_version_info(),
        is_arm64: architecture == "arm64",
        architecture,
        system_dark_mode: windows_detect_system_dark_mode(),
        app_browser,
        preferred_backend: preferred_backend.to_string(),
        status: status.to_string(),
        webview2,
    }
}

/// `windows_native.format_diagnosis_report()`.
pub fn windows_format_diagnosis_report(diag: &WindowsDiagnosis) -> String {
    let ver = &diag.version_info;
    let wv2 = &diag.webview2;
    let arm_text = if diag.is_arm64 {
        "是 (ARM64 原生优化)"
    } else {
        "否 (x86_64)"
    };
    let webview_text = if wv2.installed {
        format!("已就绪 (版本: {}, 路径: {})", wv2.version, wv2.path)
    } else {
        "未安装 (将自动平滑降级至 Browser App 模式)".to_string()
    };
    let browser_text = match &diag.app_browser {
        // Python tests truthiness, so an empty path means "not found".
        Some(found) if !found.is_empty() => format!("已就绪 ({})", found),
        _ => "未找到兼容浏览器".to_string(),
    };
    let readiness = if diag.status == "ready" {
        "[OK] 原生全生态开箱即用"
    } else {
        "[WARNING] 建议安装 Edge WebView2 运行时以获得最佳体验"
    };
    [
        rule('='),
        " ReadMD Windows 操作系统原生适配与图形引擎诊断报告".to_string(),
        rule('='),
        format!(
            "[*] 操作系统版本: {} (Major: {}, Minor: {}, Build: {})",
            ver.name, ver.major, ver.minor, ver.build
        ),
        format!(
            "[*] 处理器指令集架构: {} (Windows on ARM: {})",
            diag.architecture, arm_text
        ),
        format!(
            "[*] 系统深色模式: {}",
            if diag.system_dark_mode { "已开启 (Dark Theme)" } else { "浅色/默认" }
        ),
        rule('-'),
        "[*] 渲染引擎探测 (三重自愈双轨矩阵):".to_string(),
        format!("  - Microsoft Edge WebView2 运行时: {}", webview_text),
        format!("  - 独立 Browser App 模式 (msedge/chrome): {}", browser_text),
        format!("  - 自动首选启动链路: {}", diag.preferred_backend),
        rule('-'),
        format!("[*] 综合就绪状态: {}", readiness),
        rule('='),
    ]
    .join("\n")
}

// ---------------------------------------------------------------------------
// Registry / shell FFI (advapi32 + shell32 only)
// ---------------------------------------------------------------------------

/// `winreg.HKEY_LOCAL_MACHINE` / `winreg.HKEY_CURRENT_USER`.
pub const HKLM: usize = 0x8000_0002;
pub const HKCU: usize = 0x8000_0001;

#[cfg(windows)]
mod win {
    use std::ffi::c_void;

    type HKey = *mut c_void;

    const KEY_READ: u32 = 0x2001_9;
    const ERROR_SUCCESS: i32 = 0;

    #[link(name = "advapi32")]
    extern "system" {
        fn RegOpenKeyExW(
            key: HKey,
            sub_key: *const u16,
            options: u32,
            sam_desired: u32,
            result: *mut HKey,
        ) -> i32;
        fn RegCreateKeyExW(
            key: HKey,
            sub_key: *const u16,
            reserved: u32,
            class: *mut u16,
            options: u32,
            sam_desired: u32,
            security_attributes: *mut c_void,
            result: *mut HKey,
            disposition: *mut u32,
        ) -> i32;
        fn RegSetValueExW(
            key: HKey,
            value_name: *const u16,
            reserved: u32,
            kind: u32,
            data: *const u8,
            data_len: u32,
        ) -> i32;
        fn RegDeleteValueW(
            key: HKey,
            value_name: *const u16,
        ) -> i32;
        fn RegQueryValueExW(
            key: HKey,
            value_name: *const u16,
            reserved: *mut u32,
            kind: *mut u32,
            data: *mut u8,
            data_len: *mut u32,
        ) -> i32;
        fn RegCloseKey(key: HKey) -> i32;
    }

    #[link(name = "shell32")]
    extern "system" {
        fn ShellExecuteW(
            hwnd: *mut c_void,
            operation: *const u16,
            file: *const u16,
            parameters: *const u16,
            directory: *const u16,
            show_cmd: i32,
        ) -> *mut c_void;
    }

    pub const REG_SZ: u32 = 1;
    pub const REG_EXPAND_SZ: u32 = 2;
    pub const REG_DWORD: u32 = 4;
    pub const REG_QWORD: u32 = 11;

    /// Same `to_wide` shape as `crypto.rs`.
    pub fn to_wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    // SAFETY (whole module): every pointer passed below borrows a `Vec`/array
    // that outlives the call, and every opened `HKEY` is closed on all paths.
    pub unsafe fn open(root: usize, sub_key: &str) -> Option<HKey> {
        let wide = to_wide(sub_key);
        let mut handle: HKey = std::ptr::null_mut();
        if RegOpenKeyExW(root as HKey, wide.as_ptr(), 0, KEY_READ, &mut handle) != ERROR_SUCCESS {
            return None;
        }
        Some(handle)
    }

    pub unsafe fn close(handle: HKey) {
        RegCloseKey(handle);
    }

    /// `RegQueryValueExW` twice (size probe, then read).  `None` is the
    /// `FileNotFoundError` Python catches with a bare `except Exception`.
    pub unsafe fn query(handle: HKey, name: &[u16]) -> Option<(u32, Vec<u8>)> {
        let mut kind = 0u32;
        let mut length = 0u32;
        if RegQueryValueExW(
            handle,
            name.as_ptr(),
            std::ptr::null_mut(),
            &mut kind,
            std::ptr::null_mut(),
            &mut length,
        ) != ERROR_SUCCESS
        {
            return None;
        }
        let mut buffer = vec![0u8; length as usize + 2];
        let mut size = buffer.len() as u32;
        if RegQueryValueExW(
            handle,
            name.as_ptr(),
            std::ptr::null_mut(),
            &mut kind,
            buffer.as_mut_ptr(),
            &mut size,
        ) != ERROR_SUCCESS
        {
            return None;
        }
        buffer.truncate(size as usize);
        Some((kind, buffer))
    }

    pub unsafe fn set_string(root: usize, sub_key: &str, name: &str, value: &str, expand: bool) -> bool {
        let wide_sub = to_wide(sub_key);
        let mut handle: HKey = std::ptr::null_mut();
        let mut disp: u32 = 0;
        if RegCreateKeyExW(
            root as HKey,
            wide_sub.as_ptr(),
            0,
            std::ptr::null_mut(),
            0,
            0x20006, // KEY_WRITE
            std::ptr::null_mut(),
            &mut handle,
            &mut disp,
        ) != ERROR_SUCCESS {
            return false;
        }
        let wide_val = to_wide(value);
        let val_bytes: &[u8] = std::slice::from_raw_parts(
            wide_val.as_ptr() as *const u8,
            wide_val.len() * 2,
        );
        let kind = if expand { REG_EXPAND_SZ } else { REG_SZ };
        let rc = if name.is_empty() {
            RegSetValueExW(handle, std::ptr::null(), 0, kind, val_bytes.as_ptr(), val_bytes.len() as u32)
        } else {
            let wide_name = to_wide(name);
            RegSetValueExW(handle, wide_name.as_ptr(), 0, kind, val_bytes.as_ptr(), val_bytes.len() as u32)
        };
        RegCloseKey(handle);
        rc == ERROR_SUCCESS
    }

    pub unsafe fn delete_value(root: usize, sub_key: &str, name: &str) -> bool {
        let wide_sub = to_wide(sub_key);
        let mut handle: HKey = std::ptr::null_mut();
        if RegOpenKeyExW(root as HKey, wide_sub.as_ptr(), 0, 0x20006, &mut handle) != ERROR_SUCCESS {
            return true;
        }
        let rc = if name.is_empty() {
            RegDeleteValueW(handle, std::ptr::null())
        } else {
            let wide_name = to_wide(name);
            RegDeleteValueW(handle, wide_name.as_ptr())
        };
        RegCloseKey(handle);
        rc == ERROR_SUCCESS || rc == 2
    }

    pub unsafe fn shell_open(path: &str) -> bool {
        let operation = to_wide("open");
        let file = to_wide(path);
        let ret = ShellExecuteW(
            std::ptr::null_mut(),
            operation.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1, // SW_SHOWNORMAL, what os.startfile passes
        );
        (ret as isize) > 32
    }
}

#[cfg(windows)]
pub fn win_set_reg_string(root: usize, sub_key: &str, name: &str, value: &str, expand: bool) -> bool {
    unsafe { win::set_string(root, sub_key, name, value, expand) }
}

#[cfg(windows)]
pub fn win_delete_reg_value(root: usize, sub_key: &str, name: &str) -> bool {
    unsafe { win::delete_value(root, sub_key, name) }
}

/// Decode a `REG_SZ`/`REG_EXPAND_SZ` payload as UTF-16 minus the NULs.  Not
/// `#[cfg(windows)]`: it is a pure byte function, and gating it would make the
/// crate auditor count this module as partly unreachable on the host build.
/// // WIRING: decode_reg_sz
pub fn decode_reg_sz(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .chunks(2)
        .filter_map(|pair| {
            if pair.len() == 2 {
                Some(u16::from_le_bytes([pair[0], pair[1]]))
            } else {
                None
            }
        })
        .collect();
    String::from_utf16_lossy(&units)
        .trim_end_matches('\0')
        .to_string()
}

/// A registry value rendered the way `str(ver)` renders it: `REG_SZ` verbatim,
/// integers decimal, anything else treated as the exception Python swallows.
/// // WIRING: registry_string_at
#[cfg(windows)]
pub fn registry_string_at(root: usize, subkey: &str, name: &str) -> Option<String> {
    unsafe {
        let handle = win::open(root, subkey)?;
        let wide = win::to_wide(name);
        let found = win::query(handle, &wide).and_then(|(kind, bytes)| match kind {
            win::REG_SZ | win::REG_EXPAND_SZ => Some(decode_reg_sz(&bytes)),
            win::REG_DWORD if bytes.len() >= 4 => {
                let raw: [u8; 4] = bytes[..4].try_into().ok()?;
                Some(u32::from_le_bytes(raw).to_string())
            }
            win::REG_QWORD if bytes.len() >= 8 => {
                let raw: [u8; 8] = bytes[..8].try_into().ok()?;
                Some(u64::from_le_bytes(raw).to_string())
            }
            _ => None,
        });
        win::close(handle);
        found
    }
}

#[cfg(not(windows))]
pub fn registry_string_at(_root: usize, _subkey: &str, _name: &str) -> Option<String> {
    None
}

/// Python's `if ver and ver != '0.0.0.0'` over a `QueryValueEx` result, given
/// `(is_integer, str(ver))`.  `REG_SZ ''` and `REG_DWORD 0` are both falsy but
/// for different reasons, and the *string* `"0"` is truthy — the type tag is what
/// keeps them apart once the value has been rendered.
pub fn pv_python_string(is_integer: bool, rendered: &str) -> Option<String> {
    if rendered.is_empty() || (is_integer && rendered == "0") {
        return None;
    }
    if rendered == "0.0.0.0" {
        return None;
    }
    Some(rendered.to_string())
}

/// `winreg.QueryValueEx(key, 'pv')[0]` rendered as `str(ver)`, or `None` when
/// Python's truthiness test would have rejected it.  // WIRING: registry_string_at
#[cfg(windows)]
pub fn registry_pv_at(root: usize, subkey: &str, name: &str) -> Option<String> {
    unsafe {
        let handle = win::open(root, subkey)?;
        let wide = win::to_wide(name);
        let found = win::query(handle, &wide).and_then(|(kind, bytes)| match kind {
            win::REG_SZ | win::REG_EXPAND_SZ => pv_python_string(false, &decode_reg_sz(&bytes)),
            win::REG_DWORD if bytes.len() >= 4 => pv_python_string(
                true,
                &u32::from_le_bytes(bytes[..4].try_into().ok()?).to_string(),
            ),
            win::REG_QWORD if bytes.len() >= 8 => pv_python_string(
                true,
                &u64::from_le_bytes(bytes[..8].try_into().ok()?).to_string(),
            ),
            _ => None,
        });
        win::close(handle);
        found
    }
}

#[cfg(not(windows))]
pub fn registry_pv_at(_root: usize, _subkey: &str, _name: &str) -> Option<String> {
    None
}

/// `(was an integer value, str(value))` for `QueryValueEx`, i.e. the type tag the
/// Python code still has when it writes `int(val)` / `if ver`.  `(false, None)`
/// means "the key or the value is missing", which is Python's `except` path.
#[cfg(windows)]
pub fn registry_value_at(root: usize, subkey: &str, name: &str) -> (bool, Option<String>) {
    unsafe {
        let handle = match win::open(root, subkey) {
            Some(handle) => handle,
            None => return (false, None),
        };
        let wide = win::to_wide(name);
        let found = win::query(handle, &wide).and_then(|(kind, bytes)| match kind {
            win::REG_SZ | win::REG_EXPAND_SZ => Some((false, decode_reg_sz(&bytes))),
            win::REG_DWORD if bytes.len() >= 4 => Some((
                true,
                u32::from_le_bytes(bytes[..4].try_into().ok()?).to_string(),
            )),
            win::REG_QWORD if bytes.len() >= 8 => Some((
                true,
                u64::from_le_bytes(bytes[..8].try_into().ok()?).to_string(),
            )),
            _ => None,
        });
        win::close(handle);
        found
            .map(|(is_integer, text)| (is_integer, Some(text)))
            .unwrap_or((false, None))
    }
}

#[cfg(not(windows))]
pub fn registry_value_at(_root: usize, _subkey: &str, _name: &str) -> (bool, Option<String>) {
    (false, None)
}

/// `QueryValueEx` forced to an integer, i.e. `int(val)` in Python: a `REG_DWORD`
/// directly, a numeric `REG_SZ` parsed, anything else `None` (Python's
/// `ValueError` path).  // WIRING: registry_u32_at
#[cfg(windows)]
pub fn registry_u32_at(root: usize, subkey: &str, name: &str) -> Option<u32> {
    unsafe {
        let handle = win::open(root, subkey)?;
        let wide = win::to_wide(name);
        let found = win::query(handle, &wide).and_then(|(kind, bytes)| match kind {
            win::REG_DWORD if bytes.len() >= 4 => {
                Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
            }
            win::REG_SZ | win::REG_EXPAND_SZ => decode_reg_sz(&bytes).trim().parse::<u32>().ok(),
            _ => None,
        });
        win::close(handle);
        found
    }
}

#[cfg(not(windows))]
pub fn registry_u32_at(_root: usize, _subkey: &str, _name: &str) -> Option<u32> {
    None
}

/// HKLM-flavoured shorthand used by `platform_version()`.
#[cfg(windows)]
pub fn registry_string(subkey: &str, name: &str) -> Option<String> {
    registry_string_at(HKLM, subkey, name)
}

#[cfg(not(windows))]
pub fn registry_string(_subkey: &str, _name: &str) -> Option<String> {
    None
}

/// HKLM-flavoured shorthand for integer values.
#[cfg(windows)]
pub fn registry_u32(subkey: &str, name: &str) -> Option<u32> {
    registry_u32_at(HKLM, subkey, name)
}

#[cfg(not(windows))]
pub fn registry_u32(_subkey: &str, _name: &str) -> Option<u32> {
    None
}

/// `os.startfile` on Windows; `false` elsewhere (Linux/macOS `open_path` are
/// separate functions above).
pub fn shell_open(path: &str) -> bool {
    #[cfg(windows)]
    {
        unsafe { win::shell_open(path) }
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        false
    }
}

// ---------------------------------------------------------------------------
// `src/readmd_modules/linux_native.py`
// ---------------------------------------------------------------------------

/// `linux_native.IS_LINUX` / `is_linux()`.
pub fn linux_is_linux() -> bool {
    IS_LINUX
}

/// `linux_native.is_wayland()`.  Both reads are the one-argument
/// `environ.get(k)`, so absent and `''` are equally falsy and the `or` falls
/// through to `XDG_SESSION_TYPE == 'wayland'` (exact, case-sensitive:
/// `'Wayland'` is False).  // WIRING: linux_is_wayland_from
pub fn linux_is_wayland_from(display: Option<&str>, session: Option<&str>) -> bool {
    if display.map(|value| !value.is_empty()).unwrap_or(false) {
        return true;
    }
    session == Some("wayland")
}

pub fn linux_is_wayland() -> bool {
    linux_is_wayland_from(
        env_get("WAYLAND_DISPLAY").as_deref(),
        env_get("XDG_SESSION_TYPE").as_deref(),
    )
}

/// `linux_native.architecture()` — ReadMD's own vocabulary, fed by
/// `platform.machine()`.  // WIRING: linux_architecture_from
pub fn linux_architecture_from(machine: &str) -> String {
    let lowered = py_lower(machine);
    if lowered == "arm64" || lowered == "aarch64" {
        return "arm64".to_string();
    }
    if lowered.starts_with("armv") {
        return "arm".to_string();
    }
    if lowered.contains("loongarch") {
        return "loongarch64".to_string();
    }
    if lowered.contains("mips") {
        return "mips64el".to_string();
    }
    if lowered.contains("sw_64") || lowered.contains("sw64") {
        return "sw64".to_string();
    }
    if lowered == "x86_64" || lowered == "amd64" {
        return "x86_64".to_string();
    }
    // `return machine or 'unknown'`
    if lowered.is_empty() {
        "unknown".to_string()
    } else {
        lowered
    }
}

pub fn linux_architecture() -> String {
    linux_architecture_from(&platform_machine())
}

/// `str.partition(sep)` — (before, sep, after); `sep` is `""` when the needle
/// is absent.  // WIRING: partition
pub fn partition(hay: &str, needle: char) -> (&str, bool, &str) {
    match hay.find(needle) {
        Some(index) => (&hay[..index], true, &hay[index + needle.len_utf8()..]),
        None => (hay, false, hay),
    }
}

/// The KEY=value fold of `detect_distro_info()`: every line is `strip()`ed and
/// partitioned on the *first* `=`, keys are upper-cased, later lines win, and
/// the value loses surrounding whitespace, then `"` then `'` (that order).
/// Comment lines are **not** skipped by Python, so they are not skipped here.
/// // WIRING: parse_os_release
pub fn parse_os_release(content: &str) -> std::collections::BTreeMap<String, String> {
    let mut values = std::collections::BTreeMap::new();
    for line in content.split('\n') {
        let (key, sep, value) = partition(py_strip(line), '=');
        if !sep {
            continue;
        }
        let cleaned = py_strip(value).trim_matches('"').trim_matches('\'').to_string();
        values.insert(py_upper(key), cleaned);
    }
    values
}

/// The `{'id', 'version_id', 'pretty_name'}` dict of `detect_distro_info()`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DistroInfo {
    pub id: String,
    pub version_id: String,
    pub pretty_name: String,
}

/// `linux_native.detect_distro()`'s ordered substring scan over the lower-cased
/// `/etc/os-release` body.  Order is load-bearing — e.g. an Ubuntu-based Mint
/// reports `ubuntu` because `ubuntu` is tested before `arch`.
/// // WIRING: linux_distro_from_content
pub fn linux_distro_from_content(content: &str) -> String {
    let text = py_lower(content);
    if text.contains("uos") || text.contains("uniontech") {
        return "uos".to_string();
    }
    if text.contains("kylin") || text.contains("neokylin") {
        return "kylinos".to_string();
    }
    if text.contains("deepin") {
        return "deepin".to_string();
    }
    if text.contains("openeuler") {
        return "openeuler".to_string();
    }
    if text.contains("anolis") {
        return "anolis".to_string();
    }
    if text.contains("ubuntu") {
        return "ubuntu".to_string();
    }
    if text.contains("debian") {
        return "debian".to_string();
    }
    if text.contains("fedora") {
        return "fedora".to_string();
    }
    if text.contains("arch") {
        return "arch".to_string();
    }
    // The `except Exception` and the missing-file path both land here too.
    "generic-linux".to_string()
}

pub fn linux_detect_distro() -> String {
    if !IS_LINUX {
        return "unknown".to_string();
    }
    match read_text_lossy("/etc/os-release") {
        Some(text) => linux_distro_from_content(&text),
        None => "generic-linux".to_string(),
    }
}

/// `linux_native.detect_distro_info()` given the `/etc/os-release` body
/// (`""` when `os.path.exists()` says no).  // WIRING: linux_distro_info_from
pub fn linux_distro_info_from(content: &str) -> DistroInfo {
    let mut info = DistroInfo {
        id: linux_distro_from_content(content),
        version_id: String::new(),
        pretty_name: String::new(),
    };
    let values = parse_os_release(content);
    if let Some(id) = values.get("ID") {
        // `if values.get('ID'):` — an empty ID leaves detect_distro()'s answer.
        if !id.is_empty() {
            let lowered = py_lower(id);
            if lowered.contains("uos") || lowered.contains("uniontech") {
                info.id = "uos".to_string();
            } else if lowered.contains("kylin") || lowered.contains("neokylin") {
                info.id = "kylinos".to_string();
            } else if lowered.contains("deepin") {
                info.id = "deepin".to_string();
            } else {
                info.id = lowered;
            }
        }
    }
    info.version_id = values.get("VERSION_ID").cloned().unwrap_or_default();
    info.pretty_name = values.get("PRETTY_NAME").cloned().unwrap_or_default();
    info
}

pub fn linux_detect_distro_info() -> DistroInfo {
    if !IS_LINUX {
        // `info['id'] = detect_distro()` is `'unknown'` off Linux and the
        // `/etc/os-release` block never runs.
        return DistroInfo {
            id: "unknown".to_string(),
            version_id: String::new(),
            pretty_name: String::new(),
        };
    }
    linux_distro_info_from(&read_text_lossy("/etc/os-release").unwrap_or_default())
}

/// `re.search(r'ft-?\d{3,4}', text)`: a literal `ft`, one optional `-`, then 3
/// or 4 digits.  `None`-like results for `ft20` (too few) and `ftd-2000` (the
/// `-?` cannot skip a letter).  // WIRING: contains_ft_digits
pub fn contains_ft_digits(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    for start in 0..chars.len().saturating_sub(1) {
        if chars[start] != 'f' || chars[start + 1] != 't' {
            continue;
        }
        let mut pos = start + 2;
        if chars.get(pos) == Some(&'-') {
            pos += 1;
        }
        let mut digits = 0usize;
        while digits < 4 && chars.get(pos + digits).map(|c| py_re_digit(*c)) == Some(true) {
            digits += 1;
        }
        if digits >= 3 {
            return true;
        }
    }
    false
}

/// `re.search(r'hi36\d{2}', text)` and friends: a fixed prefix plus exactly two
/// more digits.  // WIRING: contains_prefix_digits
pub fn contains_prefix_digits(text: &str, prefix: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    let head: Vec<char> = prefix.chars().collect();
    if head.is_empty() {
        return false;
    }
    for start in 0..chars.len() {
        if chars[start] != head[0] {
            continue;
        }
        let mut index = 0usize;
        let mut pos = start;
        while index < head.len() && pos < chars.len() && chars[pos] == head[index] {
            index += 1;
            pos += 1;
        }
        if index != head.len() {
            continue;
        }
        if chars.get(pos).map(|c| py_re_digit(*c)) == Some(true)
            && chars.get(pos + 1).map(|c| py_re_digit(*c)) == Some(true)
        {
            return true;
        }
    }
    false
}

/// `contains(text, needles) || ft-?\d{3,4} in text` — the Phytium alternation of
/// `detect_cpu_vendor()`'s first `re.search`.  // WIRING: is_phytium_text
pub fn is_phytium_text(text: &str) -> bool {
    ["phytium", "feiteng", "tengyun", "d2000", "e2000", "s2500"]
        .iter()
        .any(|needle| text.contains(needle))
        || contains_ft_digits(text)
}

/// The whole `detect_cpu_vendor()` decision over the already-joined,
/// already-lower-cased probe text.  The alternation order is the return-value
/// order, so `intel` beats `amd` in `"intel amd"`.  // WIRING: linux_cpu_vendor_from_text
pub fn linux_cpu_vendor_from_text(text: &str) -> String {
    let text = py_lower(text);
    if is_phytium_text(&text) {
        return "phytium".to_string();
    }
    if text.contains("kunpeng")
        || text.contains("kirin")
        || contains_prefix_digits(&text, "hi36")
        || contains_prefix_digits(&text, "hi62")
        || contains_prefix_digits(&text, "hi37")
    {
        return "kunpeng".to_string();
    }
    if ["loongson", "godson", "3a5000", "3c5000", "3a6000"]
        .iter()
        .any(|needle| text.contains(needle))
    {
        return "loongson".to_string();
    }
    if ["zhaoxin", "centaurhauls", "kaihua", "kaixian"]
        .iter()
        .any(|needle| text.contains(needle))
    {
        return "zhaoxin".to_string();
    }
    if text.contains("hygon") || text.contains("dhyana") {
        return "hygon".to_string();
    }
    if text.contains("intel") {
        return "intel".to_string();
    }
    if text.contains("amd") {
        return "amd".to_string();
    }
    String::new()
}

/// `' '.join(samples).lower()` of `detect_cpu_vendor()`.  // WIRING: linux_cpu_vendor_from_samples
pub fn linux_cpu_vendor_from_samples(samples: &[String]) -> String {
    linux_cpu_vendor_from_text(&samples.join(" "))
}

pub fn linux_detect_cpu_vendor() -> String {
    if !IS_LINUX {
        return String::new();
    }
    let mut samples: Vec<String> = Vec::new();
    if path_exists("/proc/cpuinfo") {
        if let Some(text) = read_text_lossy("/proc/cpuinfo") {
            samples.push(text);
        }
    }
    for probe in [
        "/proc/device-tree/model",
        "/proc/device-tree/vendor",
        "/sys/devices/soc0/machine",
    ] {
        if path_exists(probe) {
            if let Some(bytes) = read_first_bytes(probe, 512) {
                samples.push(decode_ascii_ignore(&bytes));
            }
        }
    }
    linux_cpu_vendor_from_samples(&samples)
}

/// `bytes.decode('ascii', errors='ignore')` — the device-tree reads are ASCII,
/// not UTF-8, so high bytes vanish instead of being replaced.
/// // WIRING: decode_ascii_ignore
fn decode_ascii_ignore(bytes: &[u8]) -> String {
    bytes
        .iter()
        .filter(|byte| **byte < 0x80)
        .map(|byte| *byte as char)
        .collect()
}

/// `bool(re.search(r'v\s*10|(^|[^\d])10([^\d]|$)', version_id, re.I))`.
/// The second alternative is a whole-token test, which is why `'110'` is False
/// while `'a10'`, `'10a'` and `'X10'` are True.  // WIRING: kylin_v10_search
pub fn kylin_v10_search(version_id: &str) -> bool {
    let chars: Vec<char> = version_id.chars().collect();
    for start in 0..chars.len() {
        // `v\s*10`
        if chars[start].eq_ignore_ascii_case(&'v') {
            let mut pos = start + 1;
            while chars.get(pos).map(|c| c.is_whitespace()) == Some(true) {
                pos += 1;
            }
            if chars.get(pos) == Some(&'1') && chars.get(pos + 1) == Some(&'0') {
                return true;
            }
        }
        // `(^|[^\d])10([^\d]|$)`
        if chars[start] == '1' && chars.get(start + 1) == Some(&'0') {
            let before_ok = start == 0 || !py_re_digit(chars[start - 1]);
            let after = start + 2;
            let after_ok = after == chars.len() || !py_re_digit(chars[after]);
            if before_ok && after_ok {
                return true;
            }
        }
    }
    false
}

/// `linux_native.is_kylin_v10()` / the same expression inlined in
/// `diagnose_system()`.  // WIRING: linux_is_kylin_v10_from
pub fn linux_is_kylin_v10_from(id: &str, version_id: &str) -> bool {
    id == "kylinos" && kylin_v10_search(version_id)
}

pub fn linux_is_kylin_v10() -> bool {
    if !IS_LINUX {
        return false;
    }
    let info = linux_detect_distro_info();
    linux_is_kylin_v10_from(&info.id, &info.version_id)
}

/// `is_phytium()`, `is_uos()`, `is_kylin()`, `is_deepin()` are all one-string
/// decisions; the last one folds Deepin *and* UOS together.
/// // WIRING: linux_predicates_from
pub fn linux_predicates_from(distro: &str, cpu_vendor: &str) -> (bool, bool, bool, bool) {
    (
        cpu_vendor == "phytium",
        distro == "uos",
        distro == "kylinos",
        distro == "deepin" || distro == "uos",
    )
}

/// `linux_native.detect_system_dark_mode()`: three `gsettings get` probes in a
/// fixed order, each swallowed independently.  Note the needles differ per
/// schema and the third one is `prefer-dark`, not `dark`.
/// // WIRING: linux_dark_mode_from
pub fn linux_dark_mode_from(probes: &[Option<String>]) -> bool {
    let needles: [&[&str]; 3] = [
        &["dark"],
        &["dark", "black"],
        &["prefer-dark"],
    ];
    for (index, probe) in probes.iter().enumerate() {
        let text = match probe {
            Some(value) => value,
            None => continue,
        };
        let out = py_lower(py_strip(text));
        match needles.get(index) {
            Some(group) if group.iter().any(|needle| out.contains(needle)) => return true,
            Some(_) => {}
            None => break,
        }
    }
    false
}

pub fn linux_detect_system_dark_mode() -> bool {
    if !IS_LINUX || shutil_which("gsettings").is_none() {
        return false;
    }
    let mut probes: Vec<Option<String>> = Vec::new();
    for (schema, key) in [
        ("org.deepin.dde.appearance", "theme-type"),
        ("org.ukui.style", "style-name"),
        ("org.gnome.desktop.interface", "color-scheme"),
    ] {
        probes.push(run_captured(vec![
            "gsettings".to_string(),
            "get".to_string(),
            schema.to_string(),
            key.to_string(),
        ]));
    }
    linux_dark_mode_from(&probes)
}

/// Everything `setup_linux_env()` touches, keyed like `os.environ`.
/// // WIRING: linux_setup_keys
pub const LINUX_SETUP_KEYS: [&str; 6] = [
    "GDK_BACKEND",
    "WEBKIT_DISABLE_COMPOSITING_MODE",
    "WEBKIT_DISABLE_DMABUF_RENDERER",
    "LIBGL_ALWAYS_SOFTWARE",
    "GALLIUM_DRIVER",
    "MESA_LOADER_DRIVER_OVERRIDE",
];

/// `set_env_default()` inside `setup_linux_env()`: a *present but empty* value
/// is overwritten (CI exports empty compatibility switches), a non-empty one is
/// never touched — so the first writer wins.  // WIRING: set_env_default
pub fn set_env_default(env: &mut EnvMap, name: &str, value: &str) {
    match env.get(name) {
        Some(current) if !current.is_empty() => {}
        _ => {
            env.insert(name.to_string(), value.to_string());
        }
    }
}

pub type EnvMap = std::collections::BTreeMap<String, String>;

/// `linux_native.setup_linux_env()` as a pure function of the four values
/// Python reads (`is_wayland()`, `is_kylin_v10()`, `architecture()`,
/// `detect_cpu_vendor()`) plus the starting environment.
/// `legacy_gpu` is `kylin_v10 and arch == 'arm64' and phytium`, *or*
/// `READMD_SOFTWARE_WEBKIT ∈ {'1','true','yes'}` case-insensitively — and note
/// that in the legacy branch the `GDK_BACKEND` write is a no-op when Wayland
/// already chose `'wayland,x11'`.  // WIRING: linux_env_defaults
pub fn linux_env_defaults(
    initial: &EnvMap,
    wayland: bool,
    kylin_v10: bool,
    architecture: &str,
    cpu_vendor: &str,
) -> EnvMap {
    let mut env = initial.clone();
    set_env_default(&mut env, "GDK_BACKEND", if wayland { "wayland,x11" } else { "x11" });
    let flag = py_lower(
        &env.get("READMD_SOFTWARE_WEBKIT")
            .cloned()
            .unwrap_or_default(),
    );
    let legacy_gpu = (kylin_v10 && architecture == "arm64" && cpu_vendor == "phytium")
        || flag == "1"
        || flag == "true"
        || flag == "yes";
    if legacy_gpu {
        set_env_default(&mut env, "GDK_BACKEND", "x11");
        set_env_default(&mut env, "WEBKIT_DISABLE_COMPOSITING_MODE", "1");
        set_env_default(&mut env, "WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        set_env_default(&mut env, "LIBGL_ALWAYS_SOFTWARE", "1");
        set_env_default(&mut env, "GALLIUM_DRIVER", "llvmpipe");
        set_env_default(&mut env, "MESA_LOADER_DRIVER_OVERRIDE", "swrast");
    } else {
        set_env_default(&mut env, "WEBKIT_DISABLE_COMPOSITING_MODE", "0");
    }
    set_env_default(&mut env, "GDK_DPI_SCALE", "1");
    env
}

/// `linux_native.setup_linux_env()` applied to the process environment.
pub fn linux_setup_env() {
    if !IS_LINUX {
        return;
    }
    let initial = std::env::vars().collect::<EnvMap>();
    let info = linux_detect_distro_info();
    let result = linux_env_defaults(
        &initial,
        linux_is_wayland(),
        linux_is_kylin_v10_from(&info.id, &info.version_id),
        &linux_architecture(),
        &linux_detect_cpu_vendor(),
    );
    for (name, value) in result {
        if initial.get(&name) != Some(&value) {
            std::env::set_var(&name, &value);
        }
    }
}

/// The ordered `gi.require_version` preference of `probe_webkit_version()`:
/// `WebKit2` 4.1 → 4.0 → 6.0, then the `WebKit` 6.0 fallback (which also
/// reports the bare string `'6.0'`).  // WIRING: pick_webkit_version
pub fn pick_webkit_version<F>(can_load: F) -> Option<String>
where
    F: Fn(&str, &str) -> bool,
{
    for (namespace, version) in
        [("WebKit2", "4.1"), ("WebKit2", "4.0"), ("WebKit2", "6.0"), ("WebKit", "6.0")]
    {
        if can_load(namespace, version) {
            return Some(version.to_string());
        }
    }
    None
}

/// The directories `gi` would search, i.e. `GI_TYPELIB_PATH` plus the distro
/// defaults.  Provided so the wiring pass can implement the real typelib probe
/// without re-deriving the search order.  // WIRING: linux_girepository_dirs
pub fn linux_girepository_dirs() -> Vec<String> {
    let mut dirs: Vec<String> = Vec::new();
    for entry in env_get("GI_TYPELIB_PATH")
        .unwrap_or_default()
        .split(path_sep())
    {
        if !entry.is_empty() {
            dirs.push(entry.to_string());
        }
    }
    for default in [
        "/usr/lib/x86_64-linux-gnu/girepository-1.0",
        "/usr/lib64/girepository-1.0",
        "/usr/lib/girepository-1.0",
        "/usr/lib/aarch64-linux-gnu/girepository-1.0",
    ] {
        dirs.push(default.to_string());
    }
    dirs
}

/// `linux_native.probe_webkit_version()`.
///
/// NOT PORTED: the answer comes from `import gi` + `gi.require_version` + a real
/// `from gi.repository import WebKit2`, i.e. from Python's import machinery and
/// typelib loading.  A Rust process has no `gi`, which is exactly the
/// `except Exception` path that returns `None`, so `None` is the faithful answer
/// for this build — but the wiring pass must not read that as "the machine has
/// no WebKitGTK".  // WIRING: pick_webkit_version + linux_girepository_dirs
pub fn linux_probe_webkit_version() -> Option<String> {
    if !IS_LINUX {
        return None;
    }
    pick_webkit_version(|_namespace, _version| false)
}

/// `linux_native.probe_qt_webengine()`.
///
/// NOT PORTED: it is `__import__('PyQt5.QtWebEngineWidgets')` and friends — the
/// question is "does this *Python* install have the binding", which has no
/// answer for a standalone Rust binary.  `false` mirrors the all-`Exception`
/// path; the wiring pass should consult the kernel's own WebEngine probe instead
/// of this function.
pub fn linux_probe_qt_webengine() -> bool {
    false
}

/// The 15 `shutil.which` names of `find_app_browser()`, in order.  The bare
/// `'browser'` entry is what makes `launch_browser_app` send Chromium flags to
/// any distro that ships a command called `*browser*`.
/// // WIRING: LINUX_BROWSER_CANDIDATES
pub const LINUX_BROWSER_CANDIDATES: [&str; 15] = [
    "kylin-browser",
    "uos-browser",
    "browser",
    "google-chrome-stable",
    "google-chrome",
    "chromium-browser",
    "chromium",
    "microsoft-edge-stable",
    "microsoft-edge",
    "brave-browser",
    "opera",
    "vivaldi",
    "epiphany-browser",
    "epiphany",
    "firefox",
];

/// `linux_native.find_app_browser()`.  Note there is **no** `IS_LINUX` guard in
/// Python, so this searches `PATH` on every platform.
pub fn linux_find_app_browser() -> Option<String> {
    for name in LINUX_BROWSER_CANDIDATES.iter() {
        if let Some(found) = shutil_which(name) {
            if path_is_file(&found) && path_is_executable(&found) {
                return Some(found);
            }
        }
    }
    None
}

/// The argv `launch_browser_app()` spawns.  The Chromium token list is a
/// substring test on `basename(browser).lower()`, so `somebrowser`,
/// `chromium-browser` and `Epiphany-Browser` all get the `--app=` form; only a
/// name without any of those tokens reaches `epiphany` / `firefox` / the bare
/// `[browser, url]` tail.  // WIRING: linux_browser_app_command
pub fn linux_browser_app_command(
    browser: &str,
    url: &str,
    user_data_dir: &str,
    width: u32,
    height: u32,
) -> Vec<String> {
    let name = py_lower(basename_posix(browser));
    let chromium_like = ["chrome", "chromium", "kylin", "uos", "edge", "brave", "browser", "opera", "vivaldi"]
        .iter()
        .any(|token| name.contains(token));
    if chromium_like {
        return vec![
            browser.to_string(),
            format!("--app={}", url),
            format!("--user-data-dir={}", user_data_dir),
            format!("--window-size={},{}", width, height),
            "--no-first-run".to_string(),
            "--no-default-browser-check".to_string(),
            "--disable-sync".to_string(),
            "--disable-background-networking".to_string(),
            "--disable-features=Translate".to_string(),
        ];
    }
    if name.contains("epiphany") {
        return vec![browser.to_string(), format!("--application-mode={}", url)];
    }
    if name.contains("firefox") {
        return vec![browser.to_string(), "--new-window".to_string(), url.to_string()];
    }
    vec![browser.to_string(), url.to_string()]
}

/// `linux_native.launch_browser_app()`.
pub fn linux_launch_browser_app(url: &str, width: u32, height: u32) -> Option<u32> {
    let browser = match linux_find_app_browser() {
        Some(found) => found,
        None => {
            native_log("info", "no app-mode browser; would fall back to webbrowser.open");
            return None;
        }
    };
    let user_data_dir = path_join(&gettempdir(), "readmd_browser_app_profile");
    ensure_dir(&user_data_dir);
    let command = linux_browser_app_command(&browser, url, &user_data_dir, width, height);
    native_log("info", &command.join(" "));
    match spawn_detached(&command, true) {
        Some(pid) => Some(pid),
        None => {
            native_log("warning", "Failed to launch browser app");
            None
        }
    }
}

/// The `probe_gui_backends()` dict.  `gtk_webkit` keeps the tri-state
/// `None` / `Some("")` / `Some(version)` Python distinguishes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinuxBackends {
    pub gtk_webkit: Option<String>,
    pub qt_webengine: bool,
    pub app_browser: Option<String>,
    pub xdg_open: bool,
    pub preferred_backend: String,
}

/// `linux_native.probe_gui_backends()` given the four probes.  The chain is
/// `webkit → qt → browser-app → xdg-open → unknown`.  // WIRING: linux_backends_from
pub fn linux_backends_from(
    gtk_webkit: Option<String>,
    qt_webengine: bool,
    app_browser: Option<String>,
    xdg_open: bool,
) -> LinuxBackends {
    let preferred = if gtk_webkit.as_deref().map(|v| !v.is_empty()).unwrap_or(false) {
        "gtk"
    } else if qt_webengine {
        "qt"
    } else if app_browser.is_some() {
        "browser-app"
    } else if xdg_open {
        "xdg-open"
    } else {
        "unknown"
    };
    LinuxBackends {
        gtk_webkit,
        qt_webengine,
        app_browser,
        xdg_open,
        preferred_backend: preferred.to_string(),
    }
}

/// Everything `linux_native.diagnose_system()` returns.  `cpu_vendor` is the
/// value *after* Python's `cpu_vendor or 'generic'` fold, so the struct is
/// field-for-field the dict Python builds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinuxDiagnosis {
    pub is_linux: bool,
    pub distro: DistroInfo,
    pub architecture: String,
    pub cpu_vendor: String,
    pub is_phytium: bool,
    pub is_kylin_v10: bool,
    pub is_uos: bool,
    pub is_deepin: bool,
    pub wayland: bool,
    pub system_dark_mode: bool,
    pub backends: LinuxBackends,
    pub status: String,
}

/// `cpu_vendor or 'generic'` — the report never shows an empty vendor.
/// // WIRING: linux_cpu_vendor_display
pub fn linux_cpu_vendor_display(cpu_vendor: &str) -> String {
    if cpu_vendor.is_empty() {
        "generic".to_string()
    } else {
        cpu_vendor.to_string()
    }
}

/// The `status` expression: WebKitGTK/QtWebEngine are `ready`, a browser or
/// `xdg-open` is only `degraded`, nothing at all is `blocked`.
/// // WIRING: linux_status_from
pub fn linux_status_from(backends: &LinuxBackends) -> &'static str {
    let webkit = backends
        .gtk_webkit
        .as_deref()
        .map(|value| !value.is_empty())
        .unwrap_or(false);
    if webkit || backends.qt_webengine {
        "ready"
    } else if backends.app_browser.is_some() || backends.xdg_open {
        "degraded"
    } else {
        "blocked"
    }
}

/// `linux_native.diagnose_system()`.
pub fn linux_diagnose_system() -> LinuxDiagnosis {
    // Two different distro answers exist: `detect_distro()` scans the whole
    // `/etc/os-release` body for vendor needles, `detect_distro_info()['id']`
    // lets the `ID=` line override that.  `is_uos()` and `is_deepin()` call the
    // *first*, `is_kylin_v10` the *second*, and they genuinely disagree -- an
    // `ID=ubuntu` file mentioning Deepin in `HOME_URL` is `is_deepin: true`
    // beside `distro.id == "ubuntu"`.
    let content_distro = linux_detect_distro();
    let distro = linux_detect_distro_info();
    let architecture = linux_architecture();
    let cpu_vendor = linux_detect_cpu_vendor();
    let backends = linux_backends_from(
        linux_probe_webkit_version(),
        linux_probe_qt_webengine(),
        linux_find_app_browser(),
        shutil_which("xdg-open").is_some(),
    );
    let status = linux_status_from(&backends);
    LinuxDiagnosis {
        is_linux: IS_LINUX,
        is_phytium: cpu_vendor == "phytium",
        is_kylin_v10: linux_is_kylin_v10_from(&distro.id, &distro.version_id),
        is_uos: linux_predicates_from(&content_distro, &cpu_vendor).1,
        is_deepin: linux_predicates_from(&content_distro, &cpu_vendor).3,
        wayland: linux_is_wayland(),
        system_dark_mode: linux_detect_system_dark_mode(),
        architecture,
        cpu_vendor: linux_cpu_vendor_display(&cpu_vendor),
        distro,
        backends,
        status: status.to_string(),
    }
}

/// `linux_native.format_diagnosis_report()`.  // WIRING: linux_format_diagnosis_report
pub fn linux_format_diagnosis_report(diag: &LinuxDiagnosis) -> String {
    let backends = &diag.backends;
    let distro = &diag.distro;
    let webkit_ready = backends
        .gtk_webkit
        .as_deref()
        .map(|value| !value.is_empty())
        .unwrap_or(false);
    [
        rule('='),
        " ReadMD 信创与 Linux 操作系统原生适配环境诊断报告".to_string(),
        rule('='),
        format!(
            "[*] 操作系统类型: {} (ID: {}, Version: {})",
            if distro.pretty_name.is_empty() { &distro.id } else { &distro.pretty_name },
            distro.id,
            if distro.version_id.is_empty() { "N/A" } else { &distro.version_id }
        ),
        format!(
            "[*] 处理器架构: {} (CPU 厂商/特性: {})",
            diag.architecture,
            diag.cpu_vendor
        ),
        format!(
            "[*] 银河麒麟 V10: {}",
            if diag.is_kylin_v10 { "是 (已启用专有兼容层)" } else { "否" }
        ),
        format!(
            "[*] 统信 UOS / 深度: {}",
            if diag.is_uos || diag.is_deepin { "是 (已启用 DDE 原生适配)" } else { "否" }
        ),
        format!(
            "[*] 飞腾 Phytium 处理器: {}",
            if diag.is_phytium { "是 (已启用 Mesa llvmpipe 渲染自愈防花屏)" } else { "否" }
        ),
        format!(
            "[*] 显示服务器: {}",
            if diag.wayland { "Wayland (双协议自适应)" } else { "X11" }
        ),
        format!(
            "[*] 桌面深色模式: {}",
            if diag.system_dark_mode { "已开启" } else { "浅色/默认" }
        ),
        rule('-'),
        "[*] 图形引擎探测 (四重自愈双轨矩阵):".to_string(),
        format!(
            "  - WebKitGTK 原生引擎: {}",
            if webkit_ready {
                format!("已就绪 (版本: {})", backends.gtk_webkit.clone().unwrap_or_default())
            } else {
                "未安装或缺少绑定 (将自动平滑降级)".to_string()
            }
        ),
        format!(
            "  - QtWebEngine 原生引擎: {}",
            if backends.qt_webengine { "已就绪" } else { "未就绪" }
        ),
        format!(
            "  - 独立 Browser App 模式: {}",
            match &backends.app_browser {
                // Python tests truthiness, so an empty path means "not found".
                Some(found) if !found.is_empty() => format!("已就绪 ({})", found),
                _ => "未找到适配浏览器".to_string(),
            }
        ),
        format!(
            "  - 系统默认浏览器 (xdg-open): {}",
            if backends.xdg_open { "可用" } else { "未找到" }
        ),
        format!("  - 自动首选启动链路: {}", backends.preferred_backend),
        rule('-'),
        format!(
            "[*] 综合就绪状态: {}",
            if diag.status == "ready" {
                "[OK] 原生全生态开箱即用"
            } else if diag.status == "degraded" {
                "[WARNING] 仅有浏览器降级，完整功能需要 WebKitGTK/QtWebEngine"
            } else {
                "[BLOCKED] 缺少原生图形引擎"
            }
        ),
        rule('='),
    ]
    .join("\n")
}

/// The `notify-send` argv of `show_notification()` — title *and* message are
/// positional, the `-a`/`-i` flags come last.  // WIRING: linux_notification_argv
pub fn linux_notification_argv(title: &str, message: &str) -> Vec<String> {
    vec![
        "notify-send".to_string(),
        title.to_string(),
        message.to_string(),
        "-a".to_string(),
        "ReadMD".to_string(),
        "-i".to_string(),
        "readmd".to_string(),
    ]
}

/// `linux_native.show_notification()`.  Python's `Popen` here has no DEVNULL.
pub fn linux_show_notification(title: &str, message: &str) -> bool {
    if !IS_LINUX {
        return false;
    }
    if shutil_which("notify-send").is_none() {
        return false;
    }
    spawn_detached(&linux_notification_argv(title, message), false).is_some()
}

/// `linux_native.open_path()`: `xdg-open` on the *directory* of a file, on the
/// directory itself for a directory.  // WIRING: linux_open_target
pub fn linux_open_target(path: &str) -> String {
    let norm = normpath(path);
    if path_is_dir(&norm) {
        norm
    } else {
        dirname_posix(&norm)
    }
}

pub fn linux_open_path(path: &str) -> bool {
    if !IS_LINUX || !path_exists(path) {
        return false;
    }
    let target = linux_open_target(path);
    match shutil_which("xdg-open") {
        Some(_) => spawn_detached(
            &["xdg-open".to_string(), target],
            false,
        )
        .is_some(),
        None => false,
    }
}

// ---------------------------------------------------------------------------
// `src/readmd_modules/macos_native.py`
// ---------------------------------------------------------------------------

/// `macos_native.IS_MAC` / `is_macos()`.
pub fn macos_is_macos() -> bool {
    IS_MAC
}

/// The `{'major', 'minor', 'name', 'version_str'}` dict of
/// `get_macos_version_info()`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MacosVersionInfo {
    pub major: u64,
    pub minor: u64,
    pub name: String,
    pub version_str: String,
}

impl Default for MacosVersionInfo {
    /// `{'major': 0, 'minor': 0, 'name': 'macOS (Unknown)', 'version_str': ''}`
    fn default() -> Self {
        MacosVersionInfo {
            major: 0,
            minor: 0,
            name: "macOS (Unknown)".to_string(),
            version_str: String::new(),
        }
    }
}

/// `str.isdigit()` — non-empty and every code point in the captured
/// `isdigit_codepoints` set, i.e. category `Nd` **union** the 20
/// `Numeric_Type=Digit` runs of `DIGIT_SUPPLEMENT_RUNS`.  Rust's `is_numeric`
/// is `Nd | Nl | No`, which both over-accepts (`U+2160` ROMAN NUMERAL ONE is
/// `isdigit() == False` in Python) and under-accepts nothing; the exact
/// equality `nd | supplement == isdigit` was verified over all 81 captured runs.
/// // WIRING: py_isdigit
pub fn py_isdigit(text: &str) -> bool {
    !text.is_empty() && text.chars().all(char_isdigit)
}

/// `int(text)` for a `text` that has already passed [`py_isdigit`].  Python
/// accepts every Unicode `Nd` digit here and raises `ValueError` on the
/// `isdigit()`-but-not-`Nd` extras (`'²'`, `'①'`, …), which is the *opposite* of
/// what `str::parse::<u64>()` does — hence this helper.  `None` is the
/// `ValueError`.  Results saturate at `u64::MAX` because Python has bignums.
pub fn py_int_of_digits(text: &str) -> Option<u64> {
    let mut value = 0u64;
    for ch in text.chars() {
        value = value
            .saturating_mul(10)
            .saturating_add(u64::from(unicode_decimal_digit(ch)?));
    }
    Some(value)
}

/// `macos_native.get_macos_version_info()` given `platform.mac_ver()[0]`.
///
/// `version_str` is assigned *before* the parts are parsed, so an unparsable
/// component (Python's `int()` `ValueError`) yields
/// `{'major': 0, 'minor': 0, 'name': 'macOS (Unknown)', 'version_str': ver}` —
/// the partially-filled dict, not the default one.  Components too large for
/// `u64` are the only input class where Rust and Python can disagree, and only
/// in the unused `major`/`minor` fields.  // WIRING: macos_version_info_from
pub fn macos_version_info_from(ver: &str) -> MacosVersionInfo {
    let mut info = MacosVersionInfo {
        version_str: ver.to_string(),
        ..MacosVersionInfo::default()
    };
    let mut parts: Vec<u64> = Vec::new();
    for piece in ver.split('.') {
        if !py_isdigit(piece) {
            continue;
        }
        match py_int_of_digits(piece) {
            Some(value) => parts.push(value),
            None => return info,
        }
    }
    let major = *parts.first().unwrap_or(&0);
    let minor = if parts.len() > 1 { parts[1] } else { 0 };
    info.major = major;
    info.minor = minor;
    info.name = match major {
        15 => "macOS 15 Sequoia".to_string(),
        14 => "macOS 14 Sonoma".to_string(),
        13 => "macOS 13 Ventura".to_string(),
        12 => "macOS 12 Monterey".to_string(),
        11 => "macOS 11 Big Sur".to_string(),
        // `'macOS 10.%d' % info['minor']` — keyed on major only, so 10.16 and
        // 10.9 both render through the *minor* number.
        10 => format!("macOS 10.{}", minor),
        _ => format!("macOS {}", ver),
    };
    info
}

pub fn macos_get_version_info() -> MacosVersionInfo {
    if !IS_MAC {
        return MacosVersionInfo::default();
    }
    macos_version_info_from(&mac_ver_string())
}

/// `platform.mac_ver()[0]`.  CPython derives it from `uname -r`'s release plus
/// the product-version file; the Darwin release→version map (`24.x` → `15.x`)
/// is what `platform` applies.  // WIRING: mac_ver_string
#[cfg(target_os = "macos")]
pub fn mac_ver_string() -> String {
    let release = match run_captured(vec![
        "uname".to_string(),
        "-r".to_string(),
    ]) {
        Some(text) => py_strip(&text).to_string(),
        None => return String::new(),
    };
    // `platform.mac_ver()` shells out to `sw_vers -productVersion` and prefers
    // it; `uname -r` alone would report the Darwin release, not the macOS one.
    match run_captured(vec!["sw_vers".to_string(), "-productVersion".to_string()]) {
        Some(text) => py_strip(&text).to_string(),
        None => release,
    }
}

#[cfg(not(target_os = "macos"))]
pub fn mac_ver_string() -> String {
    String::new()
}

/// `macos_native.architecture()`.  Unlike the Linux fold there is no
/// pass-through: anything but `arm64`/`aarch64` is reported as `x86_64`, so an
/// Intel binary under Rosetta and a genuine M-series build are told apart only
/// by the runtime `machine`.  // WIRING: macos_architecture_from
pub fn macos_architecture_from(machine: &str) -> String {
    let lowered = py_lower(machine);
    if lowered == "arm64" || lowered == "aarch64" {
        "arm64".to_string()
    } else {
        "x86_64".to_string()
    }
}

pub fn macos_architecture() -> String {
    macos_architecture_from(&platform_machine())
}

/// `macos_native.is_apple_silicon()`.  // WIRING: macos_is_apple_silicon_from
pub fn macos_is_apple_silicon_from(architecture: &str) -> bool {
    architecture == "arm64"
}

/// `macos_native.detect_system_dark_mode()` given the `defaults` stdout
/// (`None` = non-zero exit / timeout, which is the light-mode answer).
/// // WIRING: macos_dark_mode_from
pub fn macos_dark_mode_from(output: Option<&str>) -> bool {
    match output {
        Some(text) => py_lower(py_strip(text)).contains("dark"),
        None => false,
    }
}

pub fn macos_detect_system_dark_mode() -> bool {
    if !IS_MAC {
        return false;
    }
    macos_dark_mode_from(
        run_captured(vec![
            "defaults".to_string(),
            "read".to_string(),
            "-g".to_string(),
            "AppleInterfaceStyle".to_string(),
        ])
        .as_deref(),
    )
}

/// `macos_native.probe_webkit_available()`.
///
/// NOT PORTED: it is `import WebKit` + `from AppKit import NSWorkspace`, i.e.
/// "does this Python install have PyObjC".  A Rust binary reaches Python's
/// `except Exception` branch, so `false` is the faithful value for this build —
/// and the wiring pass must read it as "no PyObjC", not "no WKWebView".
pub fn macos_probe_webkit_available() -> bool {
    false
}

/// The five hard-coded `.app` bundles of `find_app_browser()`.
/// // WIRING: MACOS_BROWSER_CANDIDATES
pub const MACOS_BROWSER_CANDIDATES: [&str; 5] = [
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
    "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
    "/Applications/Arc.app/Contents/MacOS/Arc",
    "/Applications/Vivaldi.app/Contents/MacOS/Vivaldi",
];

/// `macos_native.find_app_browser()`.
pub fn macos_find_app_browser() -> Option<String> {
    if !IS_MAC {
        return None;
    }
    MACOS_BROWSER_CANDIDATES
        .iter()
        .find(|cand| path_is_file(cand) && path_is_executable(cand))
        .map(|cand| (*cand).to_string())
}

/// The argv of `launch_browser_app()` — two flags shorter than the Windows and
/// Linux forms (no `--disable-sync`, no background-networking/Translate flags).
/// // WIRING: macos_browser_app_command
pub fn macos_browser_app_command(
    browser: &str,
    url: &str,
    user_data_dir: &str,
    width: u32,
    height: u32,
) -> Vec<String> {
    vec![
        browser.to_string(),
        format!("--app={}", url),
        format!("--user-data-dir={}", user_data_dir),
        format!("--window-size={},{}", width, height),
        "--no-first-run".to_string(),
        "--no-default-browser-check".to_string(),
    ]
}

/// `macos_native.launch_browser_app()`.  `None` with no browser means Python
/// ran `open <url>` and returned `None`; the same `None` covers a failed spawn.
pub fn macos_launch_browser_app(url: &str, width: u32, height: u32) -> Option<u32> {
    let browser = match macos_find_app_browser() {
        Some(found) => found,
        None => {
            if IS_MAC {
                let _ = spawn_detached(&["open".to_string(), url.to_string()], false);
            }
            return None;
        }
    };
    let user_data_dir = path_join(&gettempdir(), "readmd_mac_app_profile");
    ensure_dir(&user_data_dir);
    let command = macos_browser_app_command(&browser, url, &user_data_dir, width, height);
    native_log("info", &command.join(" "));
    match spawn_detached(&command, true) {
        Some(pid) => Some(pid),
        None => {
            native_log("warning", "Failed to launch macOS browser app");
            if IS_MAC {
                let _ = spawn_detached(&["open".to_string(), url.to_string()], false);
            }
            None
        }
    }
}

/// `str(x).replace('"', '\\"')` — the only escaping AppleScript gets in
/// `show_error()` / `show_notification()`.  // WIRING: applescript_escape
pub fn applescript_escape(text: &str) -> String {
    text.replace('"', "\\\"")
}

/// `'display alert "%s" message "%s" as critical'` of `show_error()`.
/// // WIRING: macos_error_script
pub fn macos_error_script(title: &str, message: &str) -> String {
    format!(
        "display alert \"{}\" message \"{}\" as critical",
        applescript_escape(title),
        applescript_escape(message)
    )
}

/// `'display notification "%s" with title "%s"'` of `show_notification()` —
/// note the *message* comes first.  // WIRING: macos_notification_script
pub fn macos_notification_script(title: &str, message: &str) -> String {
    format!(
        "display notification \"{}\" with title \"{}\"",
        applescript_escape(message),
        applescript_escape(title)
    )
}

/// `macos_native.show_error()`.  The NSAlert branch needs PyObjC, which a Rust
/// process never has, so this *is* Python's `except` path: the osascript
/// fallback.  `show_error` therefore still returns `True` on macOS.
pub fn macos_show_error(title: &str, message: &str) -> bool {
    if !IS_MAC {
        return false;
    }
    spawn_detached(
        &["osascript".to_string(), "-e".to_string(), macos_error_script(title, message)],
        false,
    )
    .is_some()
}

/// `macos_native.show_notification()`.
pub fn macos_show_notification(title: &str, message: &str) -> bool {
    if !IS_MAC {
        return false;
    }
    spawn_detached(
        &[
            "osascript".to_string(),
            "-e".to_string(),
            macos_notification_script(title, message),
        ],
        false,
    )
    .is_some()
}

/// `macos_native._file_url()`.  NOT PORTED — it returns an `NSURL` built by
/// Foundation; every caller falls through to its `open`/`open -R` branch.
pub fn macos_file_url(path: &str) -> Option<String> {
    let _ = path;
    None
}

/// `macos_native.open_path()` — the NSWorkspace import fails, so the effective
/// behaviour is `Popen(['open', os.path.abspath(path)])`.  Note there is **no**
/// `os.path.exists` check here, unlike the Windows and Linux functions.
pub fn macos_open_path(path: &str) -> bool {
    if !IS_MAC {
        return false;
    }
    spawn_detached(&["open".to_string(), abspath(path)], false).is_some()
}

/// `macos_native.reveal_path()` — `open -R <abspath>`.
pub fn macos_reveal_path(path: &str) -> bool {
    if !IS_MAC {
        return false;
    }
    spawn_detached(&["open".to_string(), "-R".to_string(), abspath(path)], false).is_some()
}

/// Everything `macos_native.diagnose_system()` returns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MacosDiagnosis {
    pub is_macos: bool,
    pub version_info: MacosVersionInfo,
    pub architecture: String,
    pub is_apple_silicon: bool,
    pub webkit_available: bool,
    pub app_browser: Option<String>,
    pub system_dark_mode: bool,
    pub preferred_backend: String,
    pub status: String,
}

/// `preferred_backend` / `status` from the two probes.  The first assignment in
/// Python (`preferred = 'cocoa-wkwebview'`) is dead — the chain always re-binds
/// it — and is preserved here only as the same three-way choice.
/// // WIRING: macos_backend_from
pub fn macos_backend_from(webkit_ok: bool, app_browser: Option<&String>) -> (&'static str, &'static str) {
    if webkit_ok {
        ("cocoa-wkwebview", "ready")
    } else if app_browser.is_some() {
        ("browser-app", "degraded")
    } else {
        ("default-browser", "blocked")
    }
}

/// `macos_native.diagnose_system()`.
pub fn macos_diagnose_system() -> MacosDiagnosis {
    let architecture = macos_architecture();
    let webkit_ok = macos_probe_webkit_available();
    let app_browser = macos_find_app_browser();
    let (preferred, status) = macos_backend_from(webkit_ok, app_browser.as_ref());
    MacosDiagnosis {
        is_macos: IS_MAC,
        version_info: macos_get_version_info(),
        is_apple_silicon: macos_is_apple_silicon_from(&architecture),
        architecture,
        webkit_available: webkit_ok,
        app_browser,
        system_dark_mode: macos_detect_system_dark_mode(),
        preferred_backend: preferred.to_string(),
        status: status.to_string(),
    }
}

/// `macos_native.format_diagnosis_report()`.  // WIRING: macos_format_diagnosis_report
pub fn macos_format_diagnosis_report(diag: &MacosDiagnosis) -> String {
    let ver = &diag.version_info;
    [
        rule('='),
        " ReadMD macOS 操作系统原生适配与图形引擎诊断报告".to_string(),
        rule('='),
        format!(
            "[*] 操作系统版本: {} ({})",
            ver.name,
            if ver.version_str.is_empty() { "N/A" } else { &ver.version_str }
        ),
        format!(
            "[*] 芯片架构: {} (Apple Silicon: {})",
            diag.architecture,
            if diag.is_apple_silicon { "是 (M 系列芯片原生运行)" } else { "否 (Intel x86_64)" }
        ),
        format!(
            "[*] 系统深色模式: {}",
            if diag.system_dark_mode { "已开启 (Dark Mode)" } else { "浅色/默认" }
        ),
        rule('-'),
        "[*] 渲染引擎探测 (双轨自愈矩阵):".to_string(),
        format!(
            "  - Cocoa WKWebView 原生引擎: {}",
            if diag.webkit_available { "已就绪 (PyObjC WKWebView + 私网隔离沙箱)" } else { "未就绪" }
        ),
        format!(
            "  - 独立 Browser App 模式: {}",
            match &diag.app_browser {
                // Python tests truthiness, so an empty path means "not found".
                Some(found) if !found.is_empty() => format!("已就绪 ({})", found),
                _ => "未找到适配 Chromium 浏览器".to_string(),
            }
        ),
        format!("  - 自动首选启动链路: {}", diag.preferred_backend),
        rule('-'),
        format!(
            "[*] 综合就绪状态: {}",
            if diag.status == "ready" {
                "[OK] 原生全生态开箱即用"
            } else if diag.status == "degraded" {
                "[WARNING] 仅有浏览器降级，完整功能需要 Cocoa WKWebView"
            } else {
                "[BLOCKED] 缺少 Cocoa WKWebView"
            }
        ),
        rule('='),
    ]
    .join("\n")
}

// ---------------------------------------------------------------------------
// `src/readmd_modules/system_native.py`
// ---------------------------------------------------------------------------

/// `system_native.get_current_platform_flavor()`.  `startswith('linux')` means
/// `linux2` (Python 2) and `linux` both map to `linux`; anything else —
/// `freebsd13`, `cygwin`, `win64` — maps to `unknown`, which is what selects the
/// generic report.  // WIRING: platform_flavor_of
pub fn platform_flavor_of(platform: &str) -> &'static str {
    if platform == "win32" {
        "windows"
    } else if platform == "darwin" {
        "macos"
    } else if platform.starts_with("linux") {
        "linux"
    } else {
        "unknown"
    }
}

pub fn get_current_platform_flavor() -> String {
    platform_flavor_of(sys_platform()).to_string()
}

/// The `architecture` of the unsupported-diagnosis dict:
/// `(platform.machine() or 'unknown').lower()` — the raw machine *lowered*, so
/// `'AMD64'` becomes `'amd64'`, *not* `'x86_64'`.  // WIRING: unsupported_architecture
pub fn unsupported_architecture(machine: &str) -> String {
    if machine.is_empty() {
        "unknown".to_string()
    } else {
        py_lower(machine)
    }
}

/// `system_native.get_unified_diagnosis()`.  The dict each platform returns is
/// the platform module's own `diagnose_system()` plus `platform_flavor`, and the
/// no-flavour fallback keeps only four keys — hence an enum rather than a struct
/// with optional fields.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UnifiedDiagnosis {
    Windows(WindowsDiagnosis),
    Macos(MacosDiagnosis),
    Linux(LinuxDiagnosis),
    Unsupported {
        platform_flavor: String,
        platform: String,
        architecture: String,
    },
}

impl UnifiedDiagnosis {
    /// `data['platform_flavor']`.  // WIRING: unified_platform_flavor
    pub fn platform_flavor(&self) -> String {
        match self {
            UnifiedDiagnosis::Windows(_) => "windows".to_string(),
            UnifiedDiagnosis::Macos(_) => "macos".to_string(),
            UnifiedDiagnosis::Linux(_) => "linux".to_string(),
            UnifiedDiagnosis::Unsupported { platform_flavor, .. } => platform_flavor.clone(),
        }
    }

    /// `data['status']`.  // WIRING: unified_status
    pub fn status(&self) -> String {
        match self {
            UnifiedDiagnosis::Windows(diag) => diag.status.clone(),
            UnifiedDiagnosis::Macos(diag) => diag.status.clone(),
            UnifiedDiagnosis::Linux(diag) => diag.status.clone(),
            UnifiedDiagnosis::Unsupported { .. } => "unsupported".to_string(),
        }
    }

    /// `data['architecture']`.  // WIRING: unified_architecture
    pub fn architecture(&self) -> String {
        match self {
            UnifiedDiagnosis::Windows(diag) => diag.architecture.clone(),
            UnifiedDiagnosis::Macos(diag) => diag.architecture.clone(),
            UnifiedDiagnosis::Linux(diag) => diag.architecture.clone(),
            UnifiedDiagnosis::Unsupported { architecture, .. } => architecture.clone(),
        }
    }
}

/// `system_native.get_unified_diagnosis()`.  The `try:` around each delegate in
/// Python can only fail on an import error, which cannot happen here, so there
/// is no fallback path beyond the unknown flavour.
pub fn get_unified_diagnosis() -> UnifiedDiagnosis {
    match get_current_platform_flavor().as_str() {
        "windows" => UnifiedDiagnosis::Windows(windows_diagnose_system()),
        "macos" => UnifiedDiagnosis::Macos(macos_diagnose_system()),
        "linux" => UnifiedDiagnosis::Linux(linux_diagnose_system()),
        flavor => UnifiedDiagnosis::Unsupported {
            platform_flavor: flavor.to_string(),
            platform: sys_platform().to_string(),
            architecture: unsupported_architecture(&platform_machine()),
        },
    }
}

/// The generic report `format_unified_report()` returns for an unknown flavour.
/// Unlike the dict, this one does **not** lowercase `platform.machine()` —
/// `'AMD64'` is printed verbatim — and `'unknown'` replaces only an empty
/// machine.  // WIRING: unsupported_report
pub fn unsupported_report(platform: &str, machine: &str) -> String {
    let shown = if machine.is_empty() { "unknown" } else { machine };
    [
        rule('='),
        " ReadMD 通用操作系统环境诊断报告".to_string(),
        rule('='),
        format!("[*] 操作系统平台: {}", platform),
        format!("[*] 处理器架构: {}", shown),
        rule('='),
    ]
    .join("\n")
}

/// `system_native.format_unified_report()`.
pub fn format_unified_report() -> String {
    match get_current_platform_flavor().as_str() {
        "windows" => windows_format_diagnosis_report(&windows_diagnose_system()),
        "macos" => macos_format_diagnosis_report(&macos_diagnose_system()),
        "linux" => linux_format_diagnosis_report(&linux_diagnose_system()),
        _ => unsupported_report(sys_platform(), &platform_machine()),
    }
}

/// `system_native.launch_native_app_window()`.  Windows and macOS take
/// `width`/`height`; Linux's `launch_browser_app()` also carries a
/// `window_title` parameter that the unified caller never passes.
pub fn launch_native_app_window(url: &str, width: u32, height: u32) -> Option<u32> {
    match get_current_platform_flavor().as_str() {
        "windows" => windows_launch_browser_app(url, width, height),
        "macos" => macos_launch_browser_app(url, width, height),
        "linux" => linux_launch_browser_app(url, width, height),
        _ => {
            native_log("info", "no native app-mode launcher; would fall back to webbrowser.open");
            None
        }
    }
}
