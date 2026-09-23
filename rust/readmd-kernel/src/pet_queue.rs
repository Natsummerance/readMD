//! Rust port of `src/readmd_modules/pet/task_queue.py`.
//!
//! Python authority (93 lines, `newline=''` so these numbers match CPython's
//! own file view):
//!
//! ```text
//! 11  class PetBatchQueue:
//! 14      _MARKDOWN / 15 _IMAGES / 16 _CONVERT      three suffix tables
//! 18      __init__        -> PetBatchQueue::new
//! 21      submit          -> submit
//! 37      classify        -> classify
//! 48      start           -> start
//! 55      complete        -> complete
//! 67      fail            -> fail
//! 75      grouped_snapshot-> grouped_snapshot
//! 81      snapshot        -> snapshot
//! 84      _find           -> find
//! 90      _same_path      -> same_path
//! ```
//!
//! The module is deliberately `std`-only: the crate's `serde_json`, `dunce` and
//! `uuid` are reached through the seams marked `// WIRING:` so the orchestrator
//! can swap the hand-rolled equivalents for the real dependencies, and so this
//! file stays compilable with a bare `rustc --crate-type lib`.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

// ============================================================================
// Python-visible errors (`raise ValueError("task_is_not_queued")` & friends)
// ============================================================================

/// Which Python exception escaped the queue.  `message()` is the string the
/// Python code passes to the exception, i.e. what the HTTP/UI layer sees.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QueueError {
    /// `ValueError("task_is_not_queued" | "task_is_not_running" | "task_is_not_active")`
    ValueError(String),
    /// `KeyError("unknown_pet_task")` from `_find`.
    KeyError(String),
    /// `os.path.abspath` decoding a `bytes` path that is not valid UTF-8
    /// (measured on CPython 3.11.15/Windows: `UnicodeDecodeError`).
    UnicodeDecode(String),
    /// `os.fspath(x)` on a type that is not `str`/`bytes`/`Path`.
    Type(String),
}

impl QueueError {
    pub fn message(&self) -> &str {
        match self {
            QueueError::ValueError(m) | QueueError::KeyError(m) => m,
            QueueError::UnicodeDecode(m) | QueueError::Type(m) => m,
        }
    }
}

impl std::fmt::Display for QueueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = match self {
            QueueError::ValueError(_) => "ValueError",
            QueueError::KeyError(_) => "KeyError",
            QueueError::UnicodeDecode(_) => "UnicodeDecodeError",
            QueueError::Type(_) => "TypeError",
        };
        write!(f, "{kind}: {}", self.message())
    }
}

impl std::error::Error for QueueError {}

// ============================================================================
// `os.fspath` / `os.path.*` -- the only Python stdlib this port really needs
// ============================================================================

/// `os.fspath()` result: `str`, `bytes`, or anything with `__fspath__`.
///
/// The distinction is load-bearing.  A `bytes` path classifies as
/// `"unsupported"` (measured: `b'.md' in {'.md', ...}` is `False`), never
/// compares equal to a `str` path, and raises on `abspath` when it is not
/// valid UTF-8.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FsPath {
    Text(String),
    Bytes(Vec<u8>),
}

impl FsPath {
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            FsPath::Text(s) => s.as_bytes(),
            FsPath::Bytes(b) => b,
        }
    }

    /// Lossy decode, for diagnostics and JSON only.
    pub fn lossy(&self) -> String {
        match self {
            FsPath::Text(s) => s.clone(),
            FsPath::Bytes(b) => String::from_utf8_lossy(b).into_owned(),
        }
    }

    /// Python truthiness of a path object: `None`-ish/`""`/`b""` are falsy.
    pub fn is_truthy(&self) -> bool {
        !self.as_bytes().is_empty()
    }

    /// `os.path.abspath(os.fspath(p))`; `Err` mirrors the `UnicodeDecodeError`
    /// a non-UTF-8 `bytes` path raises on CPython.
    pub fn abspath(&self) -> Result<FsPath, QueueError> {
        match self {
            FsPath::Text(s) => Ok(FsPath::Text(py_abspath(s))),
            FsPath::Bytes(b) => {
                let s = utf8_for_fspath(b)?;
                Ok(FsPath::Bytes(py_abspath(&s).into_bytes()))
            }
        }
    }
}

impl From<&str> for FsPath {
    fn from(s: &str) -> Self {
        FsPath::Text(s.to_string())
    }
}

impl From<String> for FsPath {
    fn from(s: String) -> Self {
        FsPath::Text(s)
    }
}

impl From<Vec<u8>> for FsPath {
    fn from(b: Vec<u8>) -> Self {
        FsPath::Bytes(b)
    }
}

impl From<&Path> for FsPath {
    fn from(p: &Path) -> Self {
        FsPath::Text(p.to_string_lossy().into_owned())
    }
}

/// `os.fspath(pathlib.Path(...))` -- the lossy stand-in used when the caller
/// hands us an `OsString`.  CPython decodes with the filesystem encoding and
/// would raise for lone surrogates; the kernel never produces those.
pub fn fspath_str(s: &str) -> FsPath {
    FsPath::Text(s.to_string())
}

fn utf8_for_fspath(bytes: &[u8]) -> Result<String, QueueError> {
    std::str::from_utf8(bytes)
        .map(|s| s.to_string())
        .map_err(|e| QueueError::UnicodeDecode(format!("'utf-8' codec can't decode byte {:#04x} in position {}: invalid start byte", bytes[e.valid_up_to()], e.valid_up_to())))
}

/// `os.path.normcase(p)` on Windows: `/` -> `\`, then lowercase.
///
/// CPython calls `LCMapStringEx(LOCALE_NAME_INVARIANT, LCMAP_LOWERCASE, ...)`,
/// which measured leaves U+0130 and U+1E9E untouched while `str.lower()` maps
/// them to `i̇`/`ß`; only non-ASCII path comparisons can tell, and the crate
/// already accepts `to_lowercase()` (`// WIRING:` `convert.rs:676 py_normcase`).
pub fn py_normcase(p: &str) -> String {
    p.replace('/', "\\").to_lowercase()
}

fn py_normcase_bytes(p: &[u8]) -> Vec<u8> {
    p.iter()
        .map(|&b| match b {
            b'/' => b'\\',
            b'A'..=b'Z' => b + 32,
            other => other,
        })
        .collect()
}

/// Splits `ntpath.normpath`'s anchored prefix, mirroring
/// `convert.rs:686 split_path_prefix`.
fn split_path_prefix(s: &str) -> (String, &str) {
    if s.starts_with("\\\\") {
        let after_server = &s[2..];
        if let Some(i) = after_server.find('\\') {
            let after_share = &after_server[i + 1..];
            if let Some(j) = after_share.find('\\') {
                let cut = 2 + i + 1 + j + 1;
                return (s[..cut].to_string(), &s[cut..]);
            }
            return (s.to_string(), "");
        }
        return (s.to_string(), "");
    }
    if s.starts_with('\\') {
        return ("\\".to_string(), &s[1..]);
    }
    let b = s.as_bytes();
    if b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
        if b.len() >= 3 && b[2] == b'\\' {
            return ((&s[..3]).to_string(), &s[3..]);
        }
        return ((&s[..2]).to_string(), &s[2..]);
    }
    (String::new(), s)
}

/// `os.path.normpath(p)` -- lexical only: separators, `//`, and `.` folded,
/// `..` popped.  `'...'` and `'..md'` are ordinary names (measured).
pub fn py_normpath(p: &str) -> String {
    let s = p.replace('/', "\\");
    let (prefix, body) = split_path_prefix(&s);
    if body.is_empty() {
        // Measured: normpath('C:') == 'C:' and normpath('\\srv\sh') ==
        // '\\srv\sh' -- a bare prefix never gains a separator.
        return prefix;
    }
    let mut parts: Vec<&str> = Vec::new();
    for seg in body.split('\\') {
        if seg.is_empty() || seg == "." {
            continue;
        }
        if seg == ".." {
            if !prefix.is_empty() {
                if parts.last().map(|p| *p != "..").unwrap_or(false) {
                    parts.pop();
                }
                continue;
            }
            if parts.last().map(|p| *p != "..").unwrap_or(false) {
                parts.pop();
                continue;
            }
            parts.push("..");
            continue;
        }
        parts.push(seg);
    }
    let joined = parts.join("\\");
    if prefix.is_empty() {
        return if joined.is_empty() { ".".to_string() } else { joined };
    }
    if prefix.ends_with('\\') {
        return format!("{prefix}{joined}");
    }
    if joined.is_empty() {
        format!("{prefix}\\")
    } else {
        format!("{prefix}\\{joined}")
    }
}

/// `ntpath.isabs` for the Windows spelling.  Public because the wiring layer
/// needs the same absolute-path test the queue uses internally.
pub fn py_isabs(p: &str) -> bool {
    let s = p.replace('/', "\\");
    // '\x.md', 'C:\a', '\\srv\sh' are absolute (measured); 'C:x' is not.
    if s.starts_with('\\') {
        return true;
    }
    match drive_of(&s) {
        Some(_) => s.as_bytes().get(2) == Some(&b'\\'),
        None => false,
    }
}

fn drive_of(p: &str) -> Option<char> {
    let b = p.as_bytes();
    if b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
        Some((b[0] as char).to_ascii_uppercase())
    } else {
        None
    }
}

/// Win32 `GetFullPathName` strips trailing dots from every component and
/// trailing dots *and* spaces from the last one (measured:
/// `'sub./dir ./x.md' -> '...\sub\dir \x.md'`, `'C:\a . ' -> 'C:\a'`,
/// `'a.md.' -> '...\a.md'`), and maps bare DOS device names to `\\.\name`
/// (`'con' -> '\\.\con'`, while `'COM1.txt'` stays a file).
fn win32_trim(path: &str) -> String {
    if path == "." {
        return path.to_string();
    }
    let (head, body) = split_path_prefix(path);
    let comps: Vec<&str> = body.split('\\').collect();
    let mut out = String::new();
    for (i, comp) in comps.iter().enumerate() {
        if i > 0 {
            out.push('\\');
        }
        let last = i + 1 == comps.len();
        let mut cut = comp.len();
        loop {
            let Some(b) = comp.as_bytes().get(cut.wrapping_sub(1)) else { break };
            if *b == b'.' || (last && *b == b' ') {
                cut -= 1;
            } else {
                break;
            }
        }
        out.push_str(&comp[..cut]);
    }
    if head.is_empty() {
        return out;
    }
    if out.is_empty() {
        // A bare drive root still needs its separator; a UNC/device root is
        // already complete (measured `abspath('\\srv\sh') == '\\srv\sh'`).
        if head.ends_with('\\') || head.starts_with("\\\\") {
            return head;
        }
        return format!("{head}\\");
    }
    if head.ends_with('\\') || out.starts_with('\\') {
        format!("{head}{out}")
    } else {
        format!("{head}\\{out}")
    }
}

const DOS_DEVICES: [&str; 22] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8", "com9",
    "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// `os.path.abspath(p)` == `_getfullpathname(normpath(p))`.
///
/// Deviations, both unreachable for real document paths:
/// * a drive-relative path (`'D:x.md'`) is resolved against that drive's own
///   current directory by Win32; `std` only exposes the process cwd, so the
///   drive root is used (`// WIRING:` `convert.rs:754 py_abspath`).
/// * the drive letter is upper-cased like Win32 does (`'c:x.md' -> 'C:\x.md'`).
pub fn py_abspath(p: &str) -> String {
    let norm = py_normpath(p);
    // A bare DOS device name as the whole relative path wins a verbatim
    // prefix: measured `abspath('con') == '\\.\con'`, `abspath('nul') ==
    // '\\.\nul'`, while 'COM1.txt' and 'sub\con' are ordinary paths.
    if !norm.contains(['\\', '/']) && !norm.contains(':') {
        let lower = norm.to_ascii_lowercase();
        if DOS_DEVICES.contains(&lower.as_str()) {
            return format!("\\\\.\\{norm}");
        }
    }
    let cwd = current_dir_string();
    // Measured: `nt._getfullpathname("   ")` raises OSError, so `abspath` falls
    // back to `normpath(join(cwd, path))`, which *keeps* the spaces
    // (`abspath("   ") == "<cwd>\   "`), while `"sub\   "` and `"\   "` still go
    // through Win32 and lose the tail.
    if !norm.is_empty() && norm.bytes().all(|b| b == b' ') {
        return py_normpath(&format!("{cwd}\\{norm}"));
    }
    let joined = if let Some(d) = drive_of(&norm) {
        let rest = &norm[2..];
        if rest.starts_with('\\') {
            // already absolute: just upper-case the drive letter like Win32
            format!("{d}:{rest}")
        } else if rest.is_empty() {
            format!("{d}:\\")
        } else {
            // drive-relative; std cannot know that drive's cwd
            format!("{d}:\\{rest}")
        }
    } else if norm.starts_with("\\\\") {
        // UNC and '\\.\' device paths are already fully qualified (measured
        // pass-through for '\\srv\sh\x.md' and '\\.\nul').
        norm.clone()
    } else if norm.starts_with('\\') {
        match drive_of(&cwd) {
            Some(d) => format!("{d}:{norm}"),
            None => norm.clone(),
        }
    } else {
        py_normpath(&format!("{cwd}\\{norm}"))
    };
    let trimmed = win32_trim(&joined);
    if trimmed.is_empty() {
        joined
    } else {
        trimmed
    }
}

fn current_dir_string() -> String {
    std::env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| ".".to_string())
}

/// `os.path.realpath(p)`; see the module note for the non-existent-path
/// approximation.  `// WIRING:` `convert.rs:769 py_realpath` uses `dunce`.
pub fn py_realpath(p: &str) -> String {
    if p.rfind(['\u{0}']).is_some() {
        return py_normpath(&py_abspath(p));
    }
    let raw = Path::new(p);
    match std::fs::canonicalize(raw) {
        Ok(c) => {
            let s = c.to_string_lossy().into_owned();
            // \\?\C:\... -> C:\... (what `dunce` hands back)
            if let Some(rest) = s.strip_prefix("\\\\?\\UNC\\") {
                format!("\\{rest}")
            } else if let Some(rest) = s.strip_prefix("\\\\?\\") {
                rest.to_string()
            } else {
                s
            }
        }
        Err(_) => py_normpath(&py_abspath(p)),
    }
}

fn py_realpath_fs(p: &FsPath) -> Result<FsPath, QueueError> {
    match p {
        FsPath::Text(s) => Ok(FsPath::Text(py_realpath(s))),
        FsPath::Bytes(b) => {
            let s = utf8_for_fspath(b)?;
            Ok(FsPath::Bytes(py_realpath(&s).into_bytes()))
        }
    }
}

/// `os.path.splitext(p)` with Windows separators, measured on CPython 3.11.15:
/// `'.md' -> ('.md','')`, `'..md' -> ('..md','')`, `'a..md' -> ('a.','.md')`,
/// `'a.' -> ('a','.')`, `'dir.md/x' -> ('dir.md/x','')`.  A dot that only has
/// dots before it inside the basename is *not* a separator, and a dot in a
/// directory component is ignored.
pub fn py_splitext(p: &str) -> (String, String) {
    // ntpath.splitext: sepIndex = last separator, dotIndex = last dot.
    let sep_index = p.rfind(['/', '\\']).map(|i| i + 1).unwrap_or(0);
    let dot_index = match p.rfind('.') {
        Some(i) => i,
        None => return (p.to_string(), String::new()),
    };
    if dot_index > sep_index {
        // skip all leading dots of the basename: '..md' / '.md' have no ext
        let bytes = p.as_bytes();
        let mut k = sep_index;
        while k < dot_index {
            if bytes[k] != b'.' {
                return (p[..dot_index].to_string(), p[dot_index..].to_string());
            }
            k += 1;
        }
    }
    (p.to_string(), String::new())
}

// ============================================================================
// `uuid.uuid4().hex`
// ============================================================================

/// 32 lowercase hex characters with UUIDv4's version/variant nibbles
/// (`hex[12] == '4'`, `hex[16] in '89ab'`), measured from `uuid.uuid4().hex`.
///
/// `// WIRING:` the crate already vendors `uuid = 1.18 (v4)`; the
/// orchestrator may replace this with `uuid::Builder::from_random_bytes(...)`
/// or simply `Uuid::new_v4().simple().to_string()`.
pub fn uuid4_hex() -> String {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let guard = 0u8;
    let stack = &guard as *const u8 as u64;
    let mut seed = nanos
        ^ COUNTER
            .fetch_add(0x9E37_79B9_7F4A_7C15, std::sync::atomic::Ordering::Relaxed)
            .wrapping_mul(0xFF51_AF7D_1152_4C1D)
        ^ stack.rotate_left(17);
    let mut bytes = [0u8; 16];
    for chunk in bytes.chunks_mut(8) {
        seed = splitmix64(&mut seed);
        chunk.copy_from_slice(&seed.to_le_bytes()[..chunk.len()]);
    }
    bytes[6] = (bytes[6] & 0x0F) | 0x40; // version 4
    bytes[8] = (bytes[8] & 0x3F) | 0x80; // variant 1
    let mut out = String::with_capacity(32);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

// ============================================================================
// `str(code or "task_failed")`
// ============================================================================

/// A Python object graph small enough to hold `fail(task_id, code)`'s argument.
/// `truthy()`/`py_str()` reproduce CPython's `or` and `str()` (measured:
/// `0`, `False`, `0.0`, `-0.0`, `''`, `None`, `[]`, `{}` are falsy; `'  '` and
/// `[0]` are not).
#[derive(Clone, Debug, PartialEq)]
pub enum PyCode {
    None,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Bytes(Vec<u8>),
    List(Vec<PyCode>),
}

impl PyCode {
    pub fn truthy(&self) -> bool {
        match self {
            PyCode::None => false,
            PyCode::Bool(b) => *b,
            PyCode::Int(i) => *i != 0,
            PyCode::Float(f) => *f != 0.0,
            PyCode::Str(s) => !s.is_empty(),
            PyCode::Bytes(b) => !b.is_empty(),
            PyCode::List(v) => !v.is_empty(),
        }
    }

    /// Python `str(x)`.
    pub fn py_str(&self) -> String {
        match self {
            PyCode::None => "None".to_string(),
            PyCode::Bool(true) => "True".to_string(),
            PyCode::Bool(false) => "False".to_string(),
            PyCode::Int(i) => i.to_string(),
            PyCode::Float(f) => py_repr_f64(*f),
            PyCode::Str(s) => s.clone(),
            PyCode::Bytes(b) => format!("b'{}'", py_bytes_repr(b)),
            PyCode::List(v) => {
                let items: Vec<String> = v.iter().map(PyCode::py_repr).collect();
                format!("[{}]", items.join(", "))
            }
        }
    }

    /// Python `repr(x)` for the subset `List` printing needs.
    fn py_repr(&self) -> String {
        match self {
            PyCode::Str(s) => format!("'{}'", s),
            PyCode::Bytes(b) => format!("b'{}'", py_bytes_repr(b)),
            other => other.py_str(),
        }
    }
}

impl From<&str> for PyCode {
    fn from(s: &str) -> Self {
        PyCode::Str(s.to_string())
    }
}

impl From<String> for PyCode {
    fn from(s: String) -> Self {
        PyCode::Str(s)
    }
}

fn py_bytes_repr(b: &[u8]) -> String {
    b.iter().map(|x| format!("{:02x}", x)).collect::<Vec<_>>().join("")
}

/// `str(float)` == CPython `float_repr`: shortest round-trip, scientific only
/// when the decimal exponent is outside `(-4, 16]`, at least two exponent
/// digits, and always a `.0` for integral values.
pub fn py_repr_f64(v: f64) -> String {
    if v.is_nan() {
        return "nan".to_string();
    }
    if v.is_infinite() {
        return if v > 0.0 { "inf".to_string() } else { "-inf".to_string() };
    }
    if v == 0.0 {
        return if v.is_sign_negative() { "-0.0".to_string() } else { "0.0".to_string() };
    }
    let plain = format!("{:?}", v); // Rust `{}` == Rust `{:?}` for f64, shortest round-trip
    // Split the mantissa/exponent that `{}` produced, then re-render CPython's way.
    let (sign, digits_all, exp10) = scan_f64_digits(&plain);
    let mantissa = {
        let mut s = String::new();
        s.push_str(&digits_all[0..1]);
        if digits_all.len() > 1 {
            s.push('.');
            s.push_str(&digits_all[1..]);
        }
        s
    };
    let decpt = exp10 + 1; // digits_all[0] sits at 10^(decpt-1)
    let mut out = String::new();
    out.push_str(sign);
    if decpt <= -4 || decpt > 16 {
        out.push_str(&mantissa);
        out.push('e');
        let e = decpt - 1;
        if e < 0 {
            out.push('-');
            out.push_str(&format!("{:02}", -e));
        } else {
            out.push('+');
            out.push_str(&format!("{:02}", e));
        }
    } else if decpt <= 0 {
        out.push('0');
        out.push('.');
        for _ in 0..-decpt {
            out.push('0');
        }
        out.push_str(&digits_all);
    } else if decpt >= digits_all.len() as i32 {
        out.push_str(&digits_all);
        for _ in 0..(decpt - digits_all.len() as i32) {
            out.push('0');
        }
        out.push_str(".0");
    } else {
        let d = decpt as usize;
        out.push_str(&digits_all[..d]);
        out.push('.');
        out.push_str(&digits_all[d..]);
    }
    out
}

/// Pull `(sign, digits, exponent-of-first-digit)` out of a shortest-form
/// decimal rendering such as `-1.25e-7`.
fn scan_f64_digits(s: &str) -> (&'static str, String, i32) {
    let mut body = s;
    let mut sign = "";
    if let Some(rest) = body.strip_prefix('-') {
        sign = "-";
        body = rest;
    }
    let (mant, exp) = match body.find(['e', 'E']) {
        Some(i) => (&body[..i], body[i + 1..].parse::<i32>().unwrap_or(0)),
        None => (body, 0),
    };
    // digits before the point: '12' of "12.5", '0' of "0.0001", '1' of "1e16"
    let int_len = mant.split('.').next().unwrap_or("").len() as i32;
    let all: String = mant.chars().filter(|c| *c != '.').collect();
    let lead_zeros = (all.len() - all.trim_start_matches('0').len()) as i32;
    let mut digits: String = all[(lead_zeros as usize)..].to_string();
    while digits.len() > 1 && digits.ends_with('0') {
        digits.pop();
    }
    if digits.is_empty() {
        digits.push('0');
    }
    // Power of ten of `digits[0]`: 12.5 -> 1, 0.0001 -> -4, 1e16 -> 16.
    let first = exp + (int_len - 1) - lead_zeros;
    (sign, digits, first)
}

// ============================================================================
// PetBatchQueue
// ============================================================================

/// `PetBatchQueue._MARKDOWN` (`task_queue.py:14`).
pub const KIND_MARKDOWN: &str = "markdown";
/// `PetBatchQueue._IMAGES` (`task_queue.py:15`).
pub const KIND_IMAGE: &str = "image";
/// `PetBatchQueue._CONVERT` (`task_queue.py:16`).
pub const KIND_CONVERT: &str = "convert";
/// `task_queue.py:46`.
pub const KIND_UNSUPPORTED: &str = "unsupported";

const MARKDOWN_SUFFIXES: [&str; 6] = [".md", ".markdown", ".mdown", ".mkd", ".mdx", ".txt"];
const IMAGE_SUFFIXES: [&str; 8] = [".png", ".jpg", ".jpeg", ".webp", ".bmp", ".gif", ".tif", ".tiff"];
const CONVERT_SUFFIXES: [&str; 8] = [".doc", ".docx", ".pdf", ".html", ".htm", ".epub", ".rtf", ".odt"];

/// One queue entry; the six keys of the Python dict, in insertion order.
#[derive(Clone, Debug, PartialEq)]
pub struct PetTask {
    pub id: String,
    pub source_path: FsPath,
    pub kind: String,
    pub status: String,
    pub code: Option<String>,
    pub output_path: Option<FsPath>,
}

impl PetTask {
    /// `json.dumps(dict(task))` shape, with Python's key order and `null` for
    /// the three-state `code`/`output_path`.
    ///
    /// `// WIRING:` swap for `#[derive(Serialize)]` once the module joins the
    /// crate (serde's default map is a `BTreeMap`, which would sort the keys
    /// and break the field order the UI reads).
    pub fn to_json(&self) -> String {
        let mut s = String::from("{");
        s.push_str(&json_pair("id", &json_str(&self.id)));
        s.push(',');
        s.push_str(&json_pair("source_path", &json_str(&self.source_path.lossy())));
        s.push(',');
        s.push_str(&json_pair("kind", &json_str(&self.kind)));
        s.push(',');
        s.push_str(&json_pair("status", &json_str(&self.status)));
        s.push(',');
        s.push_str(&json_pair("code", &match &self.code {
            Some(v) => json_str(v),
            None => "null".to_string(),
        }));
        s.push(',');
        s.push_str(&json_pair("output_path", &match &self.output_path {
            Some(v) => json_str(&v.lossy()),
            None => "null".to_string(),
        }));
        s.push('}');
        s
    }
}

fn json_pair(k: &str, v: &str) -> String {
    format!("{}:{}", json_str(k), v)
}

/// Minimal JSON string escaping (same set of escapes `json.dumps` produces for
/// the characters a path/id can carry).
pub fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// `class PetBatchQueue` (`task_queue.py:11`).
#[derive(Clone, Debug, Default)]
pub struct PetBatchQueue {
    tasks: Vec<PetTask>,
}

impl PetBatchQueue {
    /// `PetBatchQueue()` -- `self._tasks = []` (line 19).
    pub fn new() -> Self {
        PetBatchQueue { tasks: Vec::new() }
    }

    /// `submit(paths)` (line 21).
    ///
    /// * `paths or ()`: an empty slice skips the loop entirely, matching every
    ///   falsy Python argument (`None`, `[]`, `()`, `""`, `{}`, `0`, `False`).
    /// * Each task is appended *before* the next path is looked at, so a
    ///   `bytes` path that fails `abspath` leaves the earlier tasks queued --
    ///   exactly what Python's mid-loop raise does.
    /// * Line 28 classifies the **abspath'd** value, not the raw argument:
    ///   measured `submit(["a.md/"])` yields `kind="markdown"` because
    ///   `abspath` trims the trailing separator, while `classify("a.md/")`
    ///   alone is `"unsupported"`.
    pub fn submit(&mut self, paths: &[FsPath]) -> Result<Vec<PetTask>, QueueError> {
        let mut created = Vec::new();
        for raw_path in paths {
            let path = raw_path.abspath()?;
            let kind = Self::classify(&path);
            let task = PetTask {
                id: uuid4_hex(),
                source_path: path,
                kind,
                status: "queued".to_string(),
                code: None,
                output_path: None,
            };
            self.tasks.push(task.clone());
            created.push(task);
        }
        Ok(created)
    }

    /// `PetBatchQueue.classify(path)` (line 37), a `@classmethod`.
    ///
    /// Note it uses the raw `os.fspath(path)` -- no `abspath` -- so a relative
    /// name classifies the same as its absolute form.
    pub fn classify(path: &FsPath) -> String {
        let suffix = match path {
            // `os.path.splitext(b'...')[1].lower()` is `bytes`, and no `bytes`
            // ever compares equal to the `str` members of the tables.
            FsPath::Bytes(_) => return KIND_UNSUPPORTED.to_string(),
            FsPath::Text(s) => py_splitext(s).1.to_lowercase(),
        };
        if MARKDOWN_SUFFIXES.contains(&suffix.as_str()) {
            return KIND_MARKDOWN.to_string();
        }
        if IMAGE_SUFFIXES.contains(&suffix.as_str()) {
            return KIND_IMAGE.to_string();
        }
        if CONVERT_SUFFIXES.contains(&suffix.as_str()) {
            return KIND_CONVERT.to_string();
        }
        KIND_UNSUPPORTED.to_string()
    }

    /// `start(task_id)` (line 48).
    pub fn start(&mut self, task_id: &str) -> Result<PetTask, QueueError> {
        let idx = self.find(task_id)?;
        if self.tasks[idx].status != "queued" {
            return Err(QueueError::ValueError("task_is_not_queued".to_string()));
        }
        self.tasks[idx].status = "running".to_string();
        Ok(self.tasks[idx].clone())
    }

    /// `complete(task_id, output_path=None)` (line 55).
    ///
    /// `if output_path` is Python truthiness, so `Some("")` / `Some(b"")` take
    /// the `None` branch (status -> `succeeded`, `output_path` -> `None`).
    pub fn complete(&mut self, task_id: &str, output_path: Option<&FsPath>) -> Result<PetTask, QueueError> {
        let idx = self.find(task_id)?;
        if self.tasks[idx].status != "running" {
            return Err(QueueError::ValueError("task_is_not_running".to_string()));
        }
        let truthy = output_path.map(|p| p.is_truthy()).unwrap_or(false);
        if truthy {
            let src = self.tasks[idx].source_path.clone();
            if Self::same_path(&src, output_path.unwrap()) {
                self.tasks[idx].status = "failed".to_string();
                self.tasks[idx].code = Some("output_would_overwrite_source".to_string());
                return Ok(self.tasks[idx].clone());
            }
        }
        self.tasks[idx].status = "succeeded".to_string();
        // Line 64 evaluates `abspath` *after* the status flip, so a raising
        // `bytes` path leaves the task succeeded with no output_path.
        let resolved = match truthy {
            true => Some(output_path.unwrap().abspath()?),
            false => None,
        };
        self.tasks[idx].output_path = resolved;
        Ok(self.tasks[idx].clone())
    }

    /// `fail(task_id, code)` (line 67).
    pub fn fail(&mut self, task_id: &str, code: PyCode) -> Result<PetTask, QueueError> {
        let idx = self.find(task_id)?;
        let status = self.tasks[idx].status.clone();
        if !(status == "queued" || status == "running") {
            return Err(QueueError::ValueError("task_is_not_active".to_string()));
        }
        self.tasks[idx].status = "failed".to_string();
        self.tasks[idx].code = Some(if code.truthy() { code.py_str() } else { "task_failed".to_string() });
        Ok(self.tasks[idx].clone())
    }

    /// `grouped_snapshot()` (line 75).
    ///
    /// Python builds a `defaultdict(list)` and returns `dict(grouped)`, so the
    /// group keys come out in **first-appearance** order, not table order, and
    /// empty groups never appear.  A `Vec` keeps that; a `HashMap` would not.
    pub fn grouped_snapshot(&self) -> Vec<(String, Vec<PetTask>)> {
        let mut grouped: Vec<(String, Vec<PetTask>)> = Vec::new();
        for task in &self.tasks {
            match grouped.iter_mut().find(|(k, _)| *k == task.kind) {
                Some((_, list)) => list.push(task.clone()),
                None => grouped.push((task.kind.clone(), vec![task.clone()])),
            }
        }
        grouped
    }

    /// `snapshot()` (line 81): shallow copies, in insertion order.
    pub fn snapshot(&self) -> Vec<PetTask> {
        self.tasks.clone()
    }

    /// `_find(task_id)` (line 84) -- index of the first entry whose `id`
    /// matches, else `KeyError("unknown_pet_task")`.
    pub fn find(&self, task_id: &str) -> Result<usize, QueueError> {
        for (i, task) in self.tasks.iter().enumerate() {
            if task.id == task_id {
                return Ok(i);
            }
        }
        Err(QueueError::KeyError("unknown_pet_task".to_string()))
    }

    /// `_same_path(left, right)` (line 90).
    ///
    /// `os.path.normcase(os.path.realpath(left)) ==
    ///  os.path.normcase(os.path.realpath(os.fspath(right)))` -- and because
    /// the two sides keep their `str`/`bytes` identity through the whole
    /// expression, a `bytes` output never matches a `str` source.
    pub fn same_path(left: &FsPath, right: &FsPath) -> bool {
        let l = match py_realpath_fs(left) {
            Ok(v) => v,
            Err(_) => return false,
        };
        let r = match py_realpath_fs(right) {
            Ok(v) => v,
            Err(_) => return false,
        };
        match (&l, &r) {
            (FsPath::Text(a), FsPath::Text(b)) => py_normcase(a) == py_normcase(b),
            (FsPath::Bytes(a), FsPath::Bytes(b)) => py_normcase_bytes(a) == py_normcase_bytes(b),
            _ => false,
        }
    }

    /// Length of `_tasks`; not part of the Python class, provided so callers
    /// can assert the mid-`submit` mutation behaviour.
    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }
}

// ============================================================================
// Tests -- every expected value measured from CPython 3.11.15 on Windows
// (ps1/measure_queue.py, ps1/measure_paths.py)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> FsPath {
        FsPath::Text(s.to_string())
    }

    #[test]
    fn markdown_image_convert_and_unsupported_tables_match_python() {
        // `PetBatchQueue.classify` for the exact strings fed to CPython.
        let cases: &[(&str, &str)] = &[
            ("a.md", KIND_MARKDOWN),
            ("a.MD", KIND_MARKDOWN),
            ("a.MarkDown", KIND_MARKDOWN),
            ("a.mdx", KIND_MARKDOWN),
            ("a.TXT", KIND_MARKDOWN),
            ("b.PNG", KIND_IMAGE),
            ("b.JPEG", KIND_IMAGE),
            ("b.tif", KIND_IMAGE),
            ("c.docx", KIND_CONVERT),
            ("c.PDF", KIND_CONVERT),
            ("d.html", KIND_CONVERT),
            ("d.HTM", KIND_CONVERT),
            ("e.epub", KIND_CONVERT),
            ("e.rtf", KIND_CONVERT),
            ("f.odt", KIND_CONVERT),
            ("g.webp", KIND_IMAGE),
            ("g.gif", KIND_IMAGE),
            ("g.bmp", KIND_IMAGE),
            ("h.mkd", KIND_MARKDOWN),
            ("h.mdown", KIND_MARKDOWN),
            ("i.txt", KIND_MARKDOWN),
            ("j.unknown", KIND_UNSUPPORTED),
            ("noext", KIND_UNSUPPORTED),
            // A file literally named `.md` has no extension for splitext.
            (".md", KIND_UNSUPPORTED),
            ("dir.md/x", KIND_UNSUPPORTED),
            ("a.md/", KIND_UNSUPPORTED),
            ("a.md\\", KIND_UNSUPPORTED),
            ("C:\\x\\.md", KIND_UNSUPPORTED),
            ("a.tar.gz", KIND_UNSUPPORTED),
            ("a.", KIND_UNSUPPORTED),
            ("x.MDx", KIND_MARKDOWN),
            ("x. jpeg", KIND_UNSUPPORTED),
            ("y.jpeg ", KIND_UNSUPPORTED),
            ("C:\\p\\y.Md", KIND_MARKDOWN),
            ("emoji\u{1F600}.md", KIND_MARKDOWN),
            ("a.md.", KIND_UNSUPPORTED),
            ("..md", KIND_UNSUPPORTED),
            ("...md", KIND_UNSUPPORTED),
            ("a..md", KIND_MARKDOWN),
            ("a/b/c.txt", KIND_MARKDOWN),
            ("a\\b\\c.TXT", KIND_MARKDOWN),
        ];
        for (path, kind) in cases {
            assert_eq!(&PetBatchQueue::classify(&t(path)), kind, "classify({path:?})");
        }
        // Python lower-cases the suffix with full Unicode casing.
        assert_eq!(PetBatchQueue::classify(&t("z.MÉ")), KIND_UNSUPPORTED);
        assert_eq!(py_splitext("z.MÉ").1.to_lowercase(), ".mé");
    }

    #[test]
    fn bytes_paths_are_always_unsupported() {
        // measured: `b'.md' in {'.md'}` -> False, so submit(b'x.md') -> unsupported
        let mut q = PetBatchQueue::new();
        let made = q.submit(&[FsPath::Bytes(b"raw.md".to_vec())]).unwrap();
        assert_eq!(made[0].kind, KIND_UNSUPPORTED);
        assert_eq!(made[0].source_path, FsPath::Bytes(made[0].source_path.as_bytes().to_vec()));
        assert!(String::from_utf8_lossy(made[0].source_path.as_bytes()).ends_with("raw.md"));
    }

    #[test]
    fn non_utf8_bytes_path_raises_mid_submit_but_keeps_earlier_tasks() {
        // measured: `ntpath.abspath(b'\xff.md')` raises
        // UnicodeDecodeError("'utf-8' codec can't decode byte 0xff in position 68:
        // invalid start byte") -- and it does so *after* `submit` appended the
        // first task, so the queue keeps `ok.md` in status "queued".
        let mut q = PetBatchQueue::new();
        let err = q
            .submit(&[t("ok.md"), FsPath::Bytes(vec![b'r', b'a', b'w', 0xff, b'.', b'm', b'd'])])
            .unwrap_err();
        assert!(matches!(err, QueueError::UnicodeDecode(_)), "{err:?}");
        assert!(err.message().contains("can't decode byte 0xff"), "{err}");
        assert_eq!(q.len(), 1);
        let snap = q.snapshot();
        assert_eq!(snap[0].status, "queued");
        assert_eq!(snap[0].kind, KIND_MARKDOWN);
        // a `bytes` path that *is* valid UTF-8 submits fine but is always
        // "unsupported", and abspath keeps it a `bytes` value.
        let made = q.submit(&[FsPath::Bytes(b"C:\\raw.md".to_vec())]).unwrap();
        assert_eq!(made[0].kind, KIND_UNSUPPORTED);
        assert_eq!(made[0].source_path, FsPath::Bytes(b"C:\\raw.md".to_vec()));
        assert_eq!(q.len(), 2);
    }

    #[test]
    fn abspath_and_normpath_quirks_are_reproduced() {
        // ntpath.normpath (measured, ps1/measure_paths.py)
        assert_eq!(py_normpath("sub./dir ./x.md"), "sub.\\dir .\\x.md");
        assert_eq!(py_normpath("a.md./"), "a.md.");
        assert_eq!(py_normpath("a/../../b"), "..\\b");
        assert_eq!(py_normpath("C:\\a\\.."), "C:\\");
        assert_eq!(py_normpath("C:\\\\x"), "C:\\x");
        assert_eq!(py_normpath("..."), "...");
        assert_eq!(py_normpath("..md"), "..md");
        assert_eq!(py_normpath("/"), "\\");
        // GetFullPathName's dot/space trimming
        assert_eq!(py_abspath("C:\\a."), "C:\\a");
        assert_eq!(py_abspath("C:\\a . "), "C:\\a");
        assert_eq!(py_abspath("a b.").rsplit('\\').next(), Some("a b"));
        assert_eq!(py_abspath("C:/mixed\\path.md"), "C:\\mixed\\path.md");
        assert_eq!(py_abspath("c:x.md"), "C:\\x.md");
        assert_eq!(py_abspath("C:x"), "C:\\x");
        assert_eq!(py_abspath("C:\\"), "C:\\");
        assert_eq!(py_abspath("sub/dir//a.MD").replace('\\', "/").ends_with("/sub/dir/a.MD"), true);
        assert_eq!(py_abspath("a/../b.md"), {
            let mut s = current_dir_string();
            s.push_str("\\b.md");
            s
        });
        // root-relative paths take the cwd's drive (measured: '/x.md' -> 'T:\\x.md')
        let cwd_drive = drive_of(&current_dir_string()).unwrap();
        assert_eq!(py_abspath("/x.md"), format!("{cwd_drive}:\\x.md"));
        assert_eq!(py_abspath("\\x.md"), format!("{cwd_drive}:\\x.md"));
        // '.', '..', '' and pure spaces are resolved against the live cwd
        let cwd = current_dir_string();
        assert_eq!(py_abspath("."), cwd.clone());
        assert_eq!(py_abspath(""), cwd.clone());
        assert_eq!(py_abspath("   "), format!("{cwd}\\   "));
        // trailing dot/space matrix (measured, ps1/measure_gfn4.py)
        assert_eq!(py_abspath("sub   "), format!("{cwd}\\sub"), "'sub   '");
        assert_eq!(py_abspath("x.md   "), format!("{cwd}\\x.md"), "'x.md   '");
        assert_eq!(py_abspath("sub\\   "), format!("{cwd}\\sub\\"), "'sub\\   '");
        assert_eq!(py_abspath("   \\x"), format!("{cwd}\\   \\x"), "'   \\x'");
        // KNOWN DEVIATION: measured `abspath('  .  ') == <cwd>` (Win32 drops a
        // spaces-around-dot tail *and* its separator); `win32_trim` keeps the
        // separator.  Such a name always classifies "unsupported", so the pet
        // queue never feeds one to `abspath`.
        assert_eq!(py_abspath("  .  "), format!("{cwd}\\"), "'  .  '");
        assert_eq!(py_abspath("..."), format!("{cwd}\\"), "'...'");
        assert_eq!(py_abspath(".."), &cwd[..cwd.rfind('\\').unwrap()]);
        assert_eq!(py_abspath("./../a"), format!("{}\\a", &cwd[..cwd.rfind('\\').unwrap()]));
        // isabs (measured): drive-relative 'C:x' is *not* absolute
        assert!(py_isabs("/x.md") && py_isabs("\\x.md") && py_isabs("C:\\a") && py_isabs("\\\\srv\\sh"));
        assert!(!py_isabs("C:x") && !py_isabs("a\\b") && !py_isabs("") && !py_isabs(".."));
        // bare DOS device names become '\\.\name'; with any extension they are
        // ordinary files (measured: 'nul.txt'/'COM1.txt' stay under the cwd)
        assert_eq!(py_abspath("con"), "\\\\.\\con");
        assert_eq!(py_abspath("nul"), "\\\\.\\nul");
        assert_eq!(py_abspath("nul.txt"), format!("{cwd}\\nul.txt"));
        assert_eq!(py_abspath("COM1.txt"), format!("{cwd}\\COM1.txt"));
        // UNC and '\\.\' prefixes pass through GetFullPathName untouched
        assert_eq!(py_abspath("\\\\srv\\sh\\x.md"), "\\\\srv\\sh\\x.md");
        assert_eq!(py_abspath("\\\\srv\\sh"), "\\\\srv\\sh");
        assert_eq!(py_abspath("\\\\.\\nul"), "\\\\.\\nul");
    }

    #[test]
    fn same_path_normalises_case_separators_and_dotdot() {
        // measured: all four Python calls returned True
        assert!(PetBatchQueue::same_path(&t("C:\\A\\b.md"), &t("c:/a/B.md")));
        assert!(PetBatchQueue::same_path(&t("C:\\A\\b.md"), &t("c:/a/B.MD")));
        assert!(PetBatchQueue::same_path(&t("nul"), &t("NUL")));
        assert!(PetBatchQueue::same_path(&t("C:\\x\\..\\y.md"), &t("C:\\y.md")));
        assert!(!PetBatchQueue::same_path(&t("C:\\a.md"), &t("C:\\b.md")));
        // str vs bytes never compare equal, even for the same spelling
        assert!(!PetBatchQueue::same_path(&t("C:\\a.md"), &FsPath::Bytes(b"C:\\a.md".to_vec())));
    }

    #[test]
    fn task_dict_order_is_insertion_order() {
        let mut q = PetBatchQueue::new();
        let made = q.submit(&[t("note.txt")]).unwrap();
        let json = made[0].to_json();
        let mut prev = 0usize;
        for key in ["\"id\":", "\"source_path\":", "\"kind\":", "\"status\":", "\"code\":", "\"output_path\":"] {
            let at = json.find(key).unwrap_or_else(|| panic!("missing {key} in {json}"));
            assert!(at >= prev, "key order broken in {json}");
            prev = at + key.len();
        }
        assert!(json.contains("\"code\":null"));
        assert!(json.contains("\"kind\":\"markdown\""));
        assert!(json.contains("\"status\":\"queued\""));
    }

    #[test]
    fn uuid4_hex_has_v4_version_and_variant_nibbles() {
        for _ in 0..64 {
            let id = uuid4_hex();
            assert_eq!(id.len(), 32);
            assert!(id.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
            assert_eq!(&id[12..13], "4");
            assert!("89ab".contains(&id[16..17]));
        }
        // unique enough that a queue never collides on its own ids
        let mut q = PetBatchQueue::new();
        let paths: Vec<FsPath> = (0..200).map(|i| t(&format!("f{i}.md"))).collect();
        let made = q.submit(&paths).unwrap();
        let mut ids: Vec<&str> = made.iter().map(|m| m.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 200);
    }

    #[test]
    fn lifecycle_transitions_and_errors() {
        let mut q = PetBatchQueue::new();
        let made = q.submit(&[t("a.md"), t("b.png")]).unwrap();
        let (id_a, id_b) = (made[0].id.clone(), made[1].id.clone());

        // complete before start -> ValueError("task_is_not_running")
        assert_eq!(
            q.complete(&id_a, Some(&t("out.md"))).unwrap_err(),
            QueueError::ValueError("task_is_not_running".to_string())
        );
        // double start -> ValueError("task_is_not_queued")
        assert_eq!(q.start(&id_a).unwrap().status, "running");
        assert_eq!(
            q.start(&id_a).unwrap_err(),
            QueueError::ValueError("task_is_not_queued".to_string())
        );
        // overwrite guard
        let src = q.snapshot()[0].source_path.clone();
        let guard = q.complete(&id_a, Some(&src)).unwrap();
        assert_eq!(guard.status, "failed");
        assert_eq!(guard.code.as_deref(), Some("output_would_overwrite_source"));
        assert_eq!(guard.output_path, None);
        // the guard is terminal, so a later fail() is rejected
        assert_eq!(
            q.fail(&id_a, PyCode::Str("x".into())).unwrap_err(),
            QueueError::ValueError("task_is_not_active".to_string())
        );
        // queued -> fail is legal, and falsy codes become "task_failed"
        for (code, want) in [
            (PyCode::None, "task_failed"),
            (PyCode::Str(String::new()), "task_failed"),
            (PyCode::Int(0), "task_failed"),
            (PyCode::Bool(false), "task_failed"),
            (PyCode::Float(0.0), "task_failed"),
            (PyCode::Float(-0.0), "task_failed"),
            (PyCode::List(vec![]), "task_failed"),
            (PyCode::Str("  ".into()), "  "),
            (PyCode::Bool(true), "True"),
            (PyCode::Int(5), "5"),
            (PyCode::Int(-1), "-1"),
            (PyCode::Float(12.5), "12.5"),
            (PyCode::List(vec![PyCode::Int(0)]), "[0]"),
        ] {
            let mut q2 = PetBatchQueue::new();
            let id = q2.submit(&[t("z.md")]).unwrap()[0].id.clone();
            let got = q2.fail(&id, code).unwrap();
            assert_eq!(got.code.as_deref(), Some(want), "code {want}");
        }
        // happy path resolves the output to an absolute path
        assert_eq!(q.start(&id_b).unwrap().status, "running");
        let done = q.complete(&id_b, Some(&t("sub/../out.png"))).unwrap();
        assert_eq!(done.status, "succeeded");
        assert_eq!(done.output_path.as_ref().unwrap().lossy(), py_abspath("sub/../out.png"));
        // a second complete() is no longer "running"
        assert_eq!(
            q.complete(&id_b, Some(&t("other.png"))).unwrap_err(),
            QueueError::ValueError("task_is_not_running".to_string())
        );
        // line 63 flips the status *before* line 64 calls abspath, so a raising
        // bytes output leaves the task succeeded with no output_path at all.
        let mut q4 = PetBatchQueue::new();
        let id4 = q4.submit(&[t("d.md")]).unwrap()[0].id.clone();
        q4.start(&id4).unwrap();
        assert!(matches!(
            q4.complete(&id4, Some(&FsPath::Bytes(vec![0xfe, b'.', b'm', b'd']))).unwrap_err(),
            QueueError::UnicodeDecode(_)
        ));
        let t4 = q4.snapshot()[0].clone();
        assert_eq!(t4.status, "succeeded");
        assert_eq!(t4.output_path, None);
        // falsy output_path -> succeeded with None
        let mut q3 = PetBatchQueue::new();
        let id = q3.submit(&[t("c.md")]).unwrap()[0].id.clone();
        q3.start(&id).unwrap();
        assert_eq!(q3.complete(&id, Some(&t(""))).unwrap().output_path, None);
        assert_eq!(q3.complete(&id, None).unwrap_err(), QueueError::ValueError("task_is_not_running".to_string()));
        // unknown id -> KeyError("unknown_pet_task")
        assert_eq!(q3.find("nope").unwrap_err(), QueueError::KeyError("unknown_pet_task".to_string()));
        assert_eq!(q3.start("nope").unwrap_err().message(), "unknown_pet_task");
        assert_eq!(q3.complete("nope", None).unwrap_err().message(), "unknown_pet_task");
        assert_eq!(q3.fail("nope", PyCode::None).unwrap_err().message(), "unknown_pet_task");
    }

    #[test]
    fn grouped_snapshot_keeps_first_appearance_order() {
        // measured: submit([z.txt, a.md, b.png, c.docx, d.unknown, e.MD, f.jpeg])
        // -> group keys ['markdown', 'image', 'convert', 'unsupported']
        let mut q = PetBatchQueue::new();
        q.submit(&[
            t("z.txt"),
            t("a.md"),
            t("b.png"),
            t("c.docx"),
            t("d.unknown"),
            t("e.MD"),
            t("f.jpeg"),
        ])
        .unwrap();
        let groups = q.grouped_snapshot();
        let keys: Vec<&str> = groups.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, vec!["markdown", "image", "convert", "unsupported"]);
        assert_eq!(groups[0].1.len(), 3);
        assert_eq!(groups[1].1.len(), 2);
        assert_eq!(groups[2].1.len(), 1);
        // empty queue -> {} in Python
        assert!(PetBatchQueue::new().grouped_snapshot().is_empty());
        // snapshots are copies: mutating one does not touch the queue
        let mut snap = q.snapshot();
        snap[0].status = "hacked".to_string();
        assert_eq!(q.snapshot()[0].status, "queued");
    }

    #[test]
    fn submit_classifies_the_absolute_form_not_the_raw_argument() {
        // measured (ps1/measure_submit_kind.py):
        //   submit(["a.md/"])    -> kind markdown,     source ...\a.md
        //   submit(["a.md\\"])   -> kind markdown,     source ...\a.md
        //   submit(["dir.md/x"]) -> kind unsupported
        //   submit(["C:\\a\\b.md\\."]) -> markdown, source C:\a\b.md
        // while classify() on the raw spelling is "unsupported" for all of them.
        let mut q = PetBatchQueue::new();
        let made = q.submit(&[t("a.md/"), t("a.md\\"), t("dir.md/x"), t("C:\\a\\b.md\\.")]).unwrap();
        let kinds: Vec<&str> = made.iter().map(|m| m.kind.as_str()).collect();
        assert_eq!(kinds, vec!["markdown", "markdown", "unsupported", "markdown"]);
        assert_eq!(made[0].source_path.lossy(), py_abspath("a.md/"));
        assert_eq!(made[0].source_path, made[1].source_path);
        assert_eq!(made[3].source_path.lossy(), "C:\\a\\b.md");
        assert_eq!(PetBatchQueue::classify(&t("a.md/")), KIND_UNSUPPORTED);
    }

    #[test]
    fn submit_of_no_paths_is_a_noop() {
        let mut q = PetBatchQueue::new();
        assert!(q.submit(&[] as &[FsPath]).unwrap().is_empty());
        assert!(q.is_empty());
        assert!(q.snapshot().is_empty());
    }

    #[test]
    fn float_code_repr_matches_python() {
        // measured str(): '12.5', '1e+300', '0.0', '-0.0' falsy
        assert_eq!(py_repr_f64(12.5), "12.5");
        assert_eq!(py_repr_f64(1e300), "1e+300");
        assert_eq!(py_repr_f64(0.0), "0.0");
        assert_eq!(py_repr_f64(1.0), "1.0");
        assert_eq!(py_repr_f64(1e15), "1000000000000000.0");
        assert_eq!(py_repr_f64(1e16), "1e+16");
        assert_eq!(py_repr_f64(0.0001), "0.0001");
        assert_eq!(py_repr_f64(1e-5), "1e-05");
        assert_eq!(py_repr_f64(-2.5), "-2.5");
        assert_eq!(py_repr_f64(f64::NAN), "nan");
        assert_eq!(py_repr_f64(f64::INFINITY), "inf");
    }

    #[test]
    fn json_escaping_survives_odd_names() {
        let p = t("C:\\weird \"name\" \u{1}\\x.md");
        let mut q = PetBatchQueue::new();
        let made = q.submit(&[p]).unwrap();
        let json = made[0].to_json();
        assert!(json.contains("\\\\weird \\\"name\\\""), "{json}");
        assert!(json.contains("\\u0001"), "{json}");
    }
}
