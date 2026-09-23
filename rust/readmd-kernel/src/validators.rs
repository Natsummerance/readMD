// -*- coding: utf-8 -*-
/// ReadMD 输入与安全校验模块 - Rust 实现（简化版，无外部依赖）
/// 
/// 防止路径遍历、Shell 注入与 SSRF 风险

use std::path::{Path, PathBuf};
use regex::Regex;

/// 验证错误异常
#[derive(Debug, Clone)]
pub struct ValidationError {
    pub message: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ValidationError {}

impl ValidationError {
    pub fn new<S: Into<String>>(message: S) -> Self {
        ValidationError { message: message.into() }
    }
}

/// Windows treats both `\` and `/` as separators (`ntpath.cut_slash`).
fn is_sep_nt(c: char) -> bool {
    c == '\\' || c == '/'
}

/// `genericpath._splitext(p, sep, altsep, extsep)` (CPython 3.11) ported
/// literally.  Python binds it two ways:
///
/// * `ntpath.splitext`    → `sep='\\'`, `altsep='/'`  (both separate)
/// * `posixpath.splitext` → `sep='/'`,  `altsep=None` (only `/` separates)
///
/// The result is `(root, ext)` and **the dot belongs to `ext`**: `'.zip'`, never
/// `'zip'`.  That single fact is what `Path::extension()` gets wrong for this
/// module (it drops the dot, returns `None` for a trailing dot, and — unlike
/// Python — treats a trailing separator as part of nothing).
///
/// Behaviour, all measured against CPython rather than inferred:
///
/// | input            | root           | ext    | why                                  |
/// |------------------|----------------|--------|--------------------------------------|
/// | `note.md`        | `note`         | `.md`  | normal case                          |
/// | `noext`          | `noext`        | `''`   | no dot at all                        |
/// | `.bashrc`        | `.bashrc`      | `''`   | dotIndex == sepIndex+1 → leading dot |
/// | `x.`             | `x`            | `.`    | trailing dot *is* the extension      |
/// | `a.tar.gz`       | `a.tar`        | `.gz`  | only the **last** dot splits         |
/// | `.tar.gz`        | `.tar`         | `.gz`  | leading dots skipped, then a real dot|
/// | `a..b`           | `a.`           | `.b`   |                                      |
/// | `X.ZIP`          | `X`            | `.ZIP` | case preserved, caller lowers        |
/// | `dir.d/file`     | `dir.d/file`   | `''`   | dot precedes the separator           |
/// | `x.zip/`         | `x.zip/`       | `''`   | trailing separator wins over the dot |
/// | `''` / `.` / `..`/ `...` | itself | `''`   | nothing but leading dots             |
/// | `a. `            | `a`            | `. `   | no trimming of trailing blanks       |
fn splitext_generic(p: &str, nt: bool) -> (String, String) {
    // `sepIndex = p.rfind(sep)`; `if altsep: sepIndex = max(sepIndex,
    // p.rfind(altsep))`.  `isize` because Python's `rfind` yields -1 when
    // absent and the `dotIndex > sepIndex` comparison relies on it.
    let sep_index: isize = if nt {
        p.rfind(is_sep_nt)
    } else {
        p.rfind('/')
    }
    .map(|i| i as isize)
    .unwrap_or(-1);

    if let Some(dot_index) = p.rfind('.') {
        if (dot_index as isize) > sep_index {
            // `filenameIndex = sepIndex + 1`, then "skip all leading dots":
            // the extension only counts once a non-dot character precedes it
            // inside the final component.  Byte indexing is safe here — a
            // UTF-8 lead/continuation byte is never 0x2E, and `.` is
            // self-synchronising, so `dot_index` is a char boundary.
            let bytes = p.as_bytes();
            let mut filename_index = (sep_index + 1) as usize;
            while filename_index < dot_index {
                if bytes[filename_index] != b'.' {
                    return (p[..dot_index].to_string(), p[dot_index..].to_string());
                }
                filename_index += 1;
            }
        }
    }
    // `return p, p[:0]`
    (p.to_string(), String::new())
}

/// `os.path.splitext(path)` for the host platform.
pub fn py_splitext(path: &str) -> (String, String) {
    splitext_generic(path, cfg!(windows))
}

/// `os.path.splitext(path)[1].lower()` — used by `validators.py:50-51`.
pub fn py_splitext_lower(path: &str) -> String {
    py_splitext(path).1.to_lowercase()
}

/// `os.path.normcase`: on Windows `/` becomes `\` and the result is lowercased;
/// on POSIX it is the identity.
pub fn normcase(p: &str) -> String {
    #[cfg(windows)]
    {
        p.replace('/', "\\").to_lowercase()
    }
    #[cfg(not(windows))]
    {
        p.to_string()
    }
}

/// `ntpath.splitdrive` for the shapes ReadMD actually sees: a `X:` prefix, or
/// the `\\server\share` UNC prefix.  Returns `(drive, remainder)`.
pub fn split_drive(p: &str) -> (String, String) {
    let chars: Vec<char> = p.chars().collect();
    if chars.len() >= 2 && chars[1] == ':' && chars[0].is_ascii_alphabetic() {
        return (chars[..2].iter().collect(), chars[2..].iter().collect());
    }
    if chars.starts_with(&['\\', '\\']) {
        // UNC: drive is `\\server\share`.
        let mut i = 2usize;
        while i < chars.len() && !is_sep_nt(chars[i]) {
            i += 1;
        }
        if i < chars.len() {
            i += 1;
            while i < chars.len() && !is_sep_nt(chars[i]) {
                i += 1;
            }
        }
        return (chars[..i.min(chars.len())].iter().collect(), chars[i.min(chars.len())..].iter().collect());
    }
    (String::new(), p.to_string())
}

/// `ntpath.normpath` / `posixpath.normpath`: collapse separators, drop `.`
/// segments and resolve `..` where possible.
pub fn normpath(p: &str) -> String {
    #[cfg(not(windows))]
    {
        return posix_normpath(p);
    }
    #[cfg(windows)]
    {
        if p.is_empty() {
            return ".".to_string();
        }
        let sep = '\\';
        let (drive, rest) = split_drive(p);
        let chars: Vec<char> = rest.chars().collect();
        let lead = chars.iter().take_while(|c| is_sep_nt(**c)).count();
        let anchor = if lead > 0 { sep.to_string() } else { String::new() };
        let unc_prefix = if drive.is_empty() && lead >= 2 {
            format!("{sep}{sep}")
        } else {
            String::new()
        };
        let mut out: Vec<String> = Vec::new();
        for part in rest.split(is_sep_nt) {
            match part {
                "" | "." => continue,
                ".." => {
                    if let Some(last) = out.last() {
                        if last != ".." {
                            out.pop();
                            continue;
                        }
                    }
                    // An anchored root swallows `..` instead of climbing.
                    if anchor.is_empty() {
                        out.push("..".to_string());
                    }
                }
                other => out.push(other.to_string()),
            }
        }
        let mut result = drive.clone();
        result.push_str(&unc_prefix);
        if !unc_prefix.is_empty() {
            // `\\server\share` already carries the root separator.
        } else {
            result.push_str(&anchor);
        }
        result.push_str(&out.join("\\"));
        if result.is_empty() {
            ".".to_string()
        } else {
            result
        }
    }
}

/// `posixpath.normpath`.  Only reachable from the `#[cfg(not(windows))]` arm of
/// [`normpath`], hence gated the same way to keep it off the dead-code list.
#[cfg(not(windows))]
fn posix_normpath(p: &str) -> String {
    if p.is_empty() {
        return ".".to_string();
    }
    let absolute = p.starts_with('/');
    // Exactly two leading slashes are implementation defined and preserved.
    let lead = p.chars().take_while(|c| *c == '/').count();
    let mut out: Vec<String> = Vec::new();
    for part in p.split('/') {
        match part {
            "" | "." => continue,
            ".." => {
                if let Some(last) = out.last() {
                    if last != ".." {
                        out.pop();
                        continue;
                    }
                }
                if !absolute {
                    out.push("..".to_string());
                }
            }
            other => out.push(other.to_string()),
        }
    }
    let mut prefix = String::new();
    if absolute {
        prefix.push('/');
        if lead >= 2 {
            prefix.push('/');
        }
    }
    let body = out.join("/");
    let result = format!("{prefix}{body}");
    if result.is_empty() {
        ".".to_string()
    } else {
        result
    }
}

/// `ntpath.basename` / `posixpath.basename`.
pub fn basename(p: &str) -> String {
    #[cfg(windows)]
    {
        let (_, rest) = split_drive(p);
        match rest.rfind(is_sep_nt) {
            Some(i) => rest[i + 1..].to_string(),
            None => rest,
        }
    }
    #[cfg(not(windows))]
    {
        match p.rfind('/') {
            Some(i) => p[i + 1..].to_string(),
            None => p.to_string(),
        }
    }
}

/// `ntpath.dirname` / `posixpath.dirname`.
pub fn dirname(p: &str) -> String {
    #[cfg(windows)]
    {
        let (drive, rest) = split_drive(p);
        match rest.rfind(is_sep_nt) {
            None => drive,
            Some(i) => {
                let head = &rest[..i + 1];
                let tail = &rest[i + 1..];
                let mut kept = head.to_string();
                if !tail.is_empty() || !head.chars().all(is_sep_nt) {
                    kept = head.trim_end_matches(is_sep_nt).to_string();
                }
                format!("{drive}{kept}")
            }
        }
    }
    #[cfg(not(windows))]
    {
        match p.rfind('/') {
            None => String::new(),
            Some(i) => {
                let head = &p[..i + 1];
                let tail = &p[i + 1..];
                if !tail.is_empty() || !head.chars().all(|c| c == '/') {
                    head.trim_end_matches('/').to_string()
                } else {
                    head.to_string()
                }
            }
        }
    }
}

/// `os.path.join(a, b)` for the two-argument case used by the recent probe.
pub fn join_path(a: &str, b: &str) -> String {
    #[cfg(windows)]
    {
        if b.starts_with(is_sep_nt) {
            return b.to_string();
        }
        let (drive, rest) = split_drive(a);
        if rest.is_empty() && !drive.is_empty() {
            return format!("{drive}{b}");
        }
        if a.is_empty() || a.ends_with(is_sep_nt) {
            return format!("{a}{b}");
        }
        format!("{a}\\{b}")
    }
    #[cfg(not(windows))]
    {
        if b.starts_with('/') {
            return b.to_string();
        }
        if a.is_empty() {
            return b.to_string();
        }
        if a.ends_with('/') {
            return format!("{a}{b}");
        }
        format!("{a}/{b}")
    }
}

/// `os.path.realpath` without `strict`: resolve what exists, keep the tail.
pub fn realpath(p: &str) -> String {
    let candidate = Path::new(p);
    if let Ok(canon) = dunce::canonicalize(candidate) {
        return canon.to_string_lossy().into_owned();
    }
    // Windows `\\?\` prefixes would leak into comparisons, so fall back to an
    // absolute, normalised path when the target does not exist yet.
    abspath(p)
}

/// `os.path.abspath` = `normpath(join(cwd, path))`.
pub fn abspath(p: &str) -> String {
    #[cfg(windows)]
    let anchored = {
        // `ntpath.splitdrive` reports a non-empty drive for `X:...` and UNC
        // roots; only the drive decides whether the path is already anchored.
        let (drive, _) = split_drive(p);
        !drive.is_empty() || p.starts_with(is_sep_nt)
    };
    #[cfg(not(windows))]
    let anchored = p.starts_with('/');
    if anchored {
        return normpath(p);
    }
    let cwd = std::env::current_dir()
        .map(|c| c.to_string_lossy().into_owned())
        .unwrap_or_else(|_| ".".to_string());
    normpath(&join_path(&cwd, p))
}

/// `os.path.expanduser('~')`.
pub fn expanduser_home() -> String {
    #[cfg(windows)]
    {
        std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .unwrap_or_else(|_| ".".to_string())
    }
    #[cfg(not(windows))]
    {
        std::env::var("HOME").unwrap_or_else(|_| ".".to_string())
    }
}

/// `src/readmd_modules/validators.py: paths_within` — "without prefix tricks".
///
/// Both sides go through `normcase(realpath(...))` and only a directory-granular
/// containment counts, so `C:\docs` never matches `C:\docssecret`.
pub fn paths_within(path: &str, root: &str) -> bool {
    let path = normcase(&realpath(path));
    let root = normcase(&realpath(root));
    if path.is_empty() || root.is_empty() {
        return false;
    }
    #[cfg(windows)]
    let sep = '\\';
    #[cfg(not(windows))]
    let sep = '/';
    let split = |p: &str| -> (String, Vec<String>) {
        #[cfg(windows)]
        {
            let (drive, rest) = split_drive(p);
            let anchor = if rest.starts_with(is_sep_nt) { sep.to_string() } else { String::new() };
            let parts = rest.split(is_sep_nt).filter(|s| !s.is_empty()).map(|s| s.to_string()).collect();
            (format!("{drive}{anchor}"), parts)
        }
        #[cfg(not(windows))]
        {
            let anchor = if p.starts_with('/') { sep.to_string() } else { String::new() };
            let parts = p.split('/').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect();
            (anchor, parts)
        }
    };
    let (root_anchor, root_parts) = split(&root);
    let (path_anchor, path_parts) = split(&path);
    if root_anchor != path_anchor || root_parts.len() > path_parts.len() {
        return false;
    }
    // `os.path.commonpath((path, root)) == root` is exactly this component-wise
    // prefix test once both sides are normalised and of the same kind.
    root_parts.iter().zip(path_parts.iter()).all(|(a, b)| a == b)
}

/// 验证文件路径合法性，防止 null 字节截断与越权遍历
pub fn validate_file_path(
    path: &str,
    allowed_extensions: Option<Vec<String>>,
    allowed_dirs: Option<Vec<String>>,
) -> Result<String, ValidationError> {
    if path.is_empty() {
        return Err(ValidationError::new("路径不能为空"));
    }
    
    if path.contains('\x00') {
        return Err(ValidationError::new("路径包含非法控制字符"));
    }
    
    for c in path.chars() {
        if c < ' ' && !matches!(c, '\t' | '\r' | '\n') {
            return Err(ValidationError::new("路径包含非法控制字符"));
        }
    }
    
    let shell_pattern = Regex::new(r"[;&|`$]").expect("regex");
    if shell_pattern.is_match(path) {
        return Err(ValidationError::new("路径包含危险字符"));
    }
    
    let abs_path = dunce::canonicalize(Path::new(path))
        .unwrap_or_else(|_| Path::new(path).to_path_buf());
    let abs_path_str = abs_path.to_string_lossy().to_string();
    
    if let Some(dirs) = allowed_dirs {
        let abs_allowed: Vec<String> = dirs.iter()
            .filter_map(|d| dunce::canonicalize(Path::new(d)).ok())
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        
        let path_normalized = normalize_path_for_check(&abs_path_str);
        let allowed_normalized: Vec<String> = abs_allowed.iter()
            .map(|d| normalize_path_for_check(d))
            .collect();
        
        let within = allowed_normalized.iter()
            .any(|allowed| paths_within(&path_normalized, allowed));
        
        if !within {
            return Err(ValidationError::new(format!(
                "路径不在允许的目录范围内：{}", abs_path_str
            )));
        }
    } else {
        #[cfg(unix)]
        {
            let system_dirs = ["/etc", "/proc", "/sys", "/dev"];
            let path_normalized = normalize_path_for_check(&abs_path_str);
            
            let is_protected = system_dirs.iter()
                .any(|system_dir| {
                    paths_within(&path_normalized, system_dir) || path_normalized == *system_dir
                });
            
            if is_protected {
                return Err(ValidationError::new("不允许访问系统受保护目录"));
            }
        }
    }
    
    // `validators.py:49-54`.  Two Python details this gate originally missed:
    //
    // * `if allowed_extensions:` is a **truth test**, so `[]` means "no
    //   extension restriction at all", not "nothing is allowed" (measured:
    //   `validate_file_path('x.txt', allowed_extensions=[])` → accepted).
    // * `os.path.splitext` **keeps** the leading dot (`validators.py:50`), and
    //   line 52 normalises every allow-list entry *to* the dotted form, so
    //   `['.zip']`, `['zip']` and `['.ZIP']` are all the same gate.  Reading
    //   `Path::extension()` instead yielded `"zip"` with no dot and compared it
    //   against a list the code had just *added* a dot to — a set with which it
    //   can never intersect, so any `Some(..)` rejected every file.
    let allowed_extensions = allowed_extensions.filter(|exts| !exts.is_empty());

    if let Some(exts) = allowed_extensions {
        let ext = py_splitext(&abs_path_str).1;
        let ext_lower = ext.to_lowercase();
        let normalized_exts: Vec<String> = exts.iter()
            .map(|e| {
                let e_lower = e.to_lowercase();
                if e_lower.starts_with('.') {
                    e_lower
                } else {
                    format!(".{}", e_lower)
                }
            })
            .collect();
        
        if !normalized_exts.contains(&ext_lower) {
            // Python interpolates the *raw* `ext`, not `ext_lower`.
            return Err(ValidationError::new(format!("不支持的文件类型：{}", ext)));
        }
    }
    
    Ok(abs_path_str)
}

fn normalize_path_for_check(path: &str) -> String {
    #[cfg(windows)]
    {
        path.to_lowercase()
    }
    #[cfg(not(windows))]
    {
        path.to_string()
    }
}

/// 验证可执行命令与参数，防止 Shell 注入
pub fn validate_command(cmd: &str) -> Result<Vec<String>, ValidationError> {
    if cmd.is_empty() {
        return Err(ValidationError::new("命令不能为空"));
    }
    
    if cmd.contains('\x00') {
        return Err(ValidationError::new("命令包含非法字符"));
    }
    
    for c in cmd.chars() {
        if c < ' ' && !matches!(c, '\t' | '\r' | '\n') {
            return Err(ValidationError::new("命令包含非法字符"));
        }
    }
    
    let cmd_pattern = Regex::new(r"[;&|`$]").expect("regex");
    if cmd_pattern.is_match(cmd) {
        return Err(ValidationError::new("命令包含危险操作符"));
    }
    
    let cmd_parts: Vec<String> = split_command(cmd)?;
    
    if cmd_parts.is_empty() {
        return Err(ValidationError::new("命令不能为空"));
    }
    
    for arg in &cmd_parts {
        if arg.contains('\x00') {
            return Err(ValidationError::new("命令参数包含非法字符"));
        }
    }
    
    Ok(cmd_parts)
}

fn split_command(cmd: &str) -> Result<Vec<String>, ValidationError> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut quote_char = '"';
    let mut escape_next = false;
    
    for c in cmd.chars() {
        if escape_next {
            current.push(c);
            escape_next = false;
            continue;
        }
        
        match c {
            '\\' if in_quotes => {
                escape_next = true;
            }
            '"' | '\'' if !escape_next => {
                if !in_quotes {
                    in_quotes = true;
                    quote_char = c;
                } else if c == quote_char {
                    in_quotes = false;
                } else {
                    current.push(c);
                }
            }
            ' ' | '\t' if !in_quotes => {
                if !current.is_empty() {
                    parts.push(current.clone());
                    current.clear();
                }
            }
            _ => {
                current.push(c);
            }
        }
    }
    
    if !current.is_empty() {
        parts.push(current);
    }
    
    Ok(parts)
}

/// 验证 URL 合法性与 SSRF 防护
pub fn validate_url(url: &str, allow_private: bool) -> Result<String, ValidationError> {
    if url.is_empty() {
        return Err(ValidationError::new("URL 不能为空"));
    }
    
    let url = url.trim();
    
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(ValidationError::new("只支持 HTTP / HTTPS 协议"));
    }
    
    let hostname = extract_hostname(url)?;
    
    if hostname.is_empty() {
        return Err(ValidationError::new("无效的 URL: 缺少主机名"));
    }
    
    let host_lower = hostname.to_lowercase();
    
    if !allow_private {
        let private_hosts = ["localhost", "127.0.0.1", "::1", "0.0.0.0"];
        let private_suffixes = [".local", ".internal", ".localhost"];
        
        if private_hosts.contains(&host_lower.as_str()) {
            return Err(ValidationError::new("不允许访问本地或内部网络地址"));
        }
        
        if private_suffixes.iter().any(|s| host_lower.ends_with(s)) {
            return Err(ValidationError::new("不允许访问本地或内部网络地址"));
        }
        
        // Simple IP check using standard library
        if let Ok(ip) = hostname.parse::<std::net::IpAddr>() {
            if ip.is_loopback() {
                return Err(ValidationError::new("不允许访问内部私有网络地址"));
            }
            // Note: is_private and is_link_local require ipnet crate
            // For now, we skip these checks to avoid dependency
        }
    }
    
    Ok(url.to_string())
}

fn extract_hostname(url: &str) -> Result<String, ValidationError> {
    let url_without_scheme = url.trim_start_matches("http://")
        .trim_start_matches("https://");
    
    let path_start = url_without_scheme.find('/')
        .unwrap_or(url_without_scheme.len());
    let host_part = &url_without_scheme[..path_start];
    
    let hostname = host_part.split(':').next()
        .unwrap_or("")
        .trim();
    
    Ok(hostname.to_string())
}

// ------------------------------------------------ version / release parity
//
// Ported from `src/readmd_core/versioning.py` and `src/readmd_modules/updater.py`
// so `/api/update/check` selects and validates exactly what the Python service
// would have.

/// A prerelease identifier: SemVer gives numeric identifiers lower precedence
/// than words such as `beta`/`rc`, which is what the `(tag, value)` tuple in
/// Python encodes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreId {
    Num(u64),
    Word(String),
}

impl Ord for PreId {
    fn cmp(&self, other: &PreId) -> std::cmp::Ordering {
        match (self, other) {
            (PreId::Num(a), PreId::Num(b)) => a.cmp(b),
            (PreId::Word(a), PreId::Word(b)) => a.cmp(b),
            (PreId::Num(_), PreId::Word(_)) => std::cmp::Ordering::Less,
            (PreId::Word(_), PreId::Num(_)) => std::cmp::Ordering::Greater,
        }
    }
}

impl PartialOrd for PreId {
    fn partial_cmp(&self, other: &PreId) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// `(core, rank, prerelease)` — `rank` is `0` when a prerelease tag exists and
/// `1` otherwise, so a GA release compares above its own `rc`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub core: [u64; 3],
    pub rank: u8,
    pub prerelease: Vec<PreId>,
}

/// `versioning.parse_version` with the same grammar as
/// `^[vV]?(\d+)(?:\.(\d+))?(?:\.(\d+))?(?:-([0-9A-Za-z.-]+))?(?:\+[0-9A-Za-z.-]+)?$`.
pub fn parse_version(value: &str) -> Option<Version> {
    let text = value.trim();
    let bytes: Vec<char> = text.chars().collect();
    let mut i = 0usize;
    if bytes.first().copied().map(|c| c == 'v' || c == 'V').unwrap_or(false) {
        i += 1;
    }
    let mut core = [0u64; 3];
    let mut index = 0usize;
    loop {
        let start = i;
        while bytes.get(i).copied().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            i += 1;
        }
        if start == i {
            return None;
        }
        let digits: String = bytes[start..i].iter().collect();
        core[index] = digits.parse::<u64>().unwrap_or(u64::MAX);
        index += 1;
        if bytes.get(i).copied() == Some('.') && index < 3 {
            i += 1;
            continue;
        }
        break;
    }
    let mut prerelease: Vec<PreId> = Vec::new();
    if bytes.get(i).copied() == Some('-') {
        i += 1;
        let start = i;
        while bytes
            .get(i)
            .copied()
            .map(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
            .unwrap_or(false)
        {
            i += 1;
        }
        if start == i {
            return None;
        }
        let ident: String = bytes[start..i].iter().collect();
        for part in ident.split('.') {
            if !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()) {
                prerelease.push(PreId::Num(part.parse::<u64>().unwrap_or(u64::MAX)));
            } else {
                prerelease.push(PreId::Word(part.to_string()));
            }
        }
    }
    if bytes.get(i).copied() == Some('+') {
        i += 1;
        let start = i;
        while bytes
            .get(i)
            .copied()
            .map(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
            .unwrap_or(false)
        {
            i += 1;
        }
        if start == i {
            return None;
        }
    }
    if i != bytes.len() {
        return None;
    }
    let rank = if prerelease.is_empty() { 1 } else { 0 };
    Some(Version { core, rank, prerelease })
}

/// `versioning.compare_versions`: `-1`, `0`, `1`, or `None` when unparseable.
pub fn compare_versions(left: &str, right: &str) -> Option<i8> {
    let a = parse_version(left)?;
    let b = parse_version(right)?;
    Some(if a > b {
        1
    } else if a < b {
        -1
    } else {
        0
    })
}

/// `versioning.is_newer_version`: strict `>` comparison.
pub fn is_newer_version(latest: &str, current: &str) -> bool {
    compare_versions(latest, current) == Some(1)
}

/// `versioning.select_update_release` (`src/readmd_core/versioning.py:37-58`).
///
/// Per candidate: a non-dict or a truthy `draft` is skipped; the tag is read as
/// `str(tag_name or '')`, so JSON numbers are legal tags; a release whose tag
/// parses as a prerelease *or* whose `prerelease` flag is truthy is skipped
/// **only** when the running build is formal (`current[1] != 0`), because
/// "Prerelease builds may advance to another prerelease or to a formal release"
/// while "Formal builds stay on the formal channel".  The pick is
/// `max(candidates, key=lambda item: item[0])`, i.e. the highest parsed
/// `(core, rank, prerelease)` tuple with the **first** candidate winning a tie,
/// and `None` when no candidate survives.
pub fn select_update_release(current_version: &str, releases: &[serde_json::Value]) -> Option<serde_json::Value> {
    let current = parse_version(current_version)?;
    let current_is_prerelease = current.rank == 0;
    let mut best: Option<(Version, serde_json::Value)> = None;
    for release in releases {
        let obj = match release.as_object() {
            Some(o) => o,
            None => continue,
        };
        if truthy(obj.get("draft")) {
            continue;
        }
        let tag = py_str_or_empty(obj.get("tag_name"));
        let parsed = match parse_version(&tag) {
            Some(v) => v,
            None => continue,
        };
        if !current_is_prerelease && (parsed.rank == 0 || truthy(obj.get("prerelease"))) {
            continue;
        }
        match &best {
            Some((prev, _)) if *prev >= parsed => {}
            _ => best = Some((parsed, release.clone())),
        }
    }
    best.map(|(_, r)| r)
}

fn truthy(v: Option<&serde_json::Value>) -> bool {
    match v {
        Some(serde_json::Value::Bool(b)) => *b,
        Some(serde_json::Value::Number(n)) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Some(serde_json::Value::String(s)) => !s.is_empty(),
        Some(serde_json::Value::Array(a)) => !a.is_empty(),
        Some(serde_json::Value::Object(o)) => !o.is_empty(),
        Some(serde_json::Value::Null) | None => false,
    }
}

/// `str(value or '')` — exactly what `versioning.parse_version` does with its
/// argument (`src/readmd_core/versioning.py:14`), so `select_update_release`
/// (`versioning.py:52`, `release.get('tag_name')`) sees the same text for a
/// JSON number, boolean or container as CPython does.
///
/// * Falsy values (`null`, absent, `false`, `0`, `0.0`, `""`, `[]`, `{}`) become
///   `''` and therefore never parse — a `tag_name` of `0` is *not* version `0`.
/// * Integers print as plain decimal digits, so `25` is version `25.0.0` and
///   `-5` stays unparseable.
/// * Floats use CPython's `str()`, i.e. the shortest round-tripping decimal
///   (`2.5` -> `"2.5"`, `2.0` -> `"2.0"`, which Rust's `{:?}` also prints).  The
///   values CPython switches to exponent form for are returned as `""`:
///   `str(1e16) == '1e+16'` and `str(1e-05) == '1e-05'` never match
///   `_VERSION_RE`, so the release is skipped either way.
/// * `true` -> `"True"`, containers -> their JSON text; `_VERSION_RE` rejects
///   both, matching CPython's `"True"` / `"[1, 2]"`.
fn py_str_or_empty(v: Option<&serde_json::Value>) -> String {
    use serde_json::Value as J;
    match v {
        None | Some(J::Null) => String::new(),
        Some(J::Bool(b)) => if *b { "True".to_string() } else { String::new() },
        Some(J::String(s)) => s.clone(),
        Some(J::Number(n)) => {
            if n.is_i64() || n.is_u64() {
                // Python `0` is falsy, so `0 or ''` -> `''`.
                if n.to_string() == "0" {
                    return String::new();
                }
                return n.to_string();
            }
            let f = match n.as_f64() {
                Some(f) => f,
                None => return String::new(),
            };
            if f == 0.0 {
                return String::new();
            }
            let magnitude = if f < 0.0 { -f } else { f };
            if !f.is_finite() || magnitude >= 1e16 || magnitude < 1e-4 {
                // CPython renders these as `1e+16` / `1e-05` / `inf` / `nan`.
                return String::new();
            }
            format!("{:?}", f)
        }
        // Arrays and objects print as JSON text; a leading `[` / `{` is exactly
        // as unparseable as CPython's `"[1, 2]"` / `"{'a': 1}"`.
        Some(_) => v.map(|value| value.to_string()).unwrap_or_default(),
    }
}

/// `updater._is_official_release_url`: https + `github.com` + the repo's two
/// leading path segments, compared case-insensitively.
pub fn is_official_release_url(url: &str) -> bool {
    const REPO: &str = "Natsummerance/readMD";
    let (scheme, rest) = match url.split_once("://") {
        Some(pair) => pair,
        None => return false,
    };
    if scheme != "https" {
        return false;
    }
    let authority = rest.split('/').next().unwrap_or("");
    let hostname = authority.rsplit('@').next().unwrap_or("").split(':').next().unwrap_or("");
    if hostname != "github.com" {
        return false;
    }
    let segments: Vec<&str> = rest[authority.len()..].split('/').filter(|s| !s.is_empty()).collect();
    let expected: Vec<&str> = REPO.split('/').collect();
    segments.len() >= 2
        && segments[0].to_lowercase() == expected[0].to_lowercase()
        && segments[1].to_lowercase() == expected[1].to_lowercase()
}

// `updater._UPDATE_FILENAME_RE` (`src/readmd_modules/updater.py:68-71`) has
// exactly one Python call site — `_UPDATE_FILENAME_RE.fullmatch(target_filename)`
// inside `validate_update_source` (`updater.py:521`) — and its Rust port is
// `batch2::update_filename_re_ok` (`batch2.rs:722`), wired into the same trust
// gate at `batch2.rs:657`.  This module deliberately keeps no second copy: the
// helper that used to live here omitted `-` from the body class, which is how
// every real ReadMD asset is named, and capped the name at 182 characters
// instead of Python's `1 + {0,180} + '.' + extension`.  A test-only oracle for
// the pattern is pinned in `tests::update_filename_re_python_oracle` so the
// rule stays checkable from this file.  The pinned oracle is
// `tests::update_filename_re_python_oracle_updater_py_68_71`.

/// `updater._safe_update_target`: the four guards in order, then the join into
/// `<temp>/ReadMDUpdates/<name>`.
pub fn safe_update_target(filename: &str, temp_dir: &Path) -> Result<PathBuf, String> {
    if filename.contains('/') || filename.contains('\\') {
        return Err("更新文件名不能包含路径".to_string());
    }
    let name = filename.trim();
    if name.is_empty() || name == "." || name == ".." || name.chars().count() > 200 {
        return Err("无效的更新文件名".to_string());
    }
    if name
        .chars()
        .any(|c| matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || (c as u32) < 0x20)
    {
        return Err("更新文件名包含非法字符".to_string());
    }
    // `re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9._-]+\.(?:exe|zip|AppImage|deb)',
    // name)` (`updater.py:84`), which is a *different*, stricter gate than
    // `_UPDATE_FILENAME_RE`: it has no `re.IGNORECASE`, so only the exact
    // spelling `AppImage` is accepted (`.appimage`, `.APPIMAGE`, `.EXE` and
    // `.dmg` all answer `'不支持的更新包类型'`), and `[A-Za-z0-9._-]+` needs at
    // least one character after the mandatory first one, so a one-character
    // stem such as `x.exe` is refused while `xa.exe` passes.  The extension
    // alternatives contain no dot, so the `\.` the pattern consumes is always
    // the last dot of the name.
    let fullmatch_ok = match name.rsplit_once('.') {
        Some((stem, extension)) => {
            matches!(extension, "exe" | "zip" | "AppImage" | "deb")
                && {
                    let chars: Vec<char> = stem.chars().collect();
                    chars.len() >= 2
                        && chars[0].is_ascii_alphanumeric()
                        && chars[1..].iter().all(|c| {
                            c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')
                        })
                }
        }
        None => false,
    };
    if !fullmatch_ok {
        return Err("不支持的更新包类型".to_string());
    }
    Ok(temp_dir.join("ReadMDUpdates").join(name))
}

/// `updater.resolve_expected_sha`: first manifest line whose digest is exactly
/// 64 hex chars and whose (basename, case-folded) filename matches.
pub fn sha256sums_lookup(text: &str, asset_name: &str) -> Option<String> {
    let wanted = basename(asset_name).to_lowercase();
    for line in text.lines() {
        let Some((sha, name)) = sha256sums_line(line) else { continue };
        if basename(&name).to_lowercase() == wanted {
            return Some(sha.to_lowercase());
        }
    }
    None
}

/// One `^\s*\*?([A-Fa-f0-9]{64})\s+\*?(.+?)\s*$` match as `(sha, filename)`.
fn sha256sums_line(line: &str) -> Option<(&str, String)> {
    let trimmed_start = line.trim_start();
    let body = trimmed_start.strip_prefix('*').unwrap_or(trimmed_start);
    if body.len() < 64 {
        return None;
    }
    let (digest, rest) = body.split_at(64);
    if !digest.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let after = match rest.chars().next() {
        Some(c) if c.is_ascii_whitespace() => &rest[rest.len() - rest.trim_start_matches(char::is_whitespace).len()..],
        _ => return None,
    };
    let name = after.strip_prefix('*').unwrap_or(after).trim_end();
    if name.is_empty() {
        return None;
    }
    // The digest must be followed by whitespace and the name must not swallow
    // another digest-shaped token, matching the anchored non-greedy group.
    Some((digest, name.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_validate_file_path_valid() {
        let result = validate_file_path("/tmp/test.md", None, None);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_validate_file_path_null_byte() {
        let result = validate_file_path("/tmp/test\x00.md", None, None);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_validate_file_path_shell_injection() {
        let result = validate_file_path("/tmp/test;rm -rf *.md", None, None);
        assert!(result.is_err());
    }

    // ------------------------------------------------ G8: extension gate parity
    //
    // Reviewer finding, verified before fixing: `validators.rs` used to read
    // `Path::extension()` — which yields `"zip"`, **no** dot — and compare it
    // against allow-list entries it had just *prepended* a dot to.  The two sets
    // cannot intersect, so `Some(..)` rejected 100% of paths, while
    // `validators.py:49-54` (which uses `os.path.splitext`, dot preserved)
    // accepts them.  These tests pin the corrected behaviour to values measured
    // from CPython 3.11.15 on this box (`python -c "import os.path; …"`), never
    // inferred from the Rust.

    /// The bug's accept path: a `.zip` file against a `['.zip']` allow-list.
    /// Python (`readmd.py:3309-3313`) accepts it; pre-fix Rust refused it.
    #[test]
    fn test_extension_gate_accepts_dotted_zip_against_dotted_allow_list() {
        let ok = validate_file_path(
            "readmd-g8-absent-archive.zip",
            Some(vec![".zip".to_string()]),
            None,
        );
        assert!(ok.is_ok(), "the G8 accept path must work: {:?}", ok);
    }

    /// `os.path.splitext` on this box is `ntpath.splitext` =
    /// `genericpath._splitext(p, '\\', '/', '.')`.  Asserted for the nt
    /// separator set explicitly so the test says the same thing on every host.
    #[test]
    fn test_splitext_matches_measured_python_edge_cases() {
        let cases: &[(&str, &str, &str)] = &[
            // input, root, ext — as printed by `os.path.splitext`
            ("note.md", "note", ".md"),
            ("archive.zip", "archive", ".zip"),
            ("noext", "noext", ""),                        // no extension
            (".bashrc", ".bashrc", ""),                    // dotfile: NOT an ext
            ("x.", "x", "."),                              // trailing dot IS one
            ("file.", "file", "."),
            ("a.tar.gz", "a.tar", ".gz"),                  // only the LAST dot
            (".tar.gz", ".tar", ".gz"),                    // leading dots skipped
            ("a..b", "a.", ".b"),                          // then a real one counts
            ("X.ZIP", "X", ".ZIP"),                        // case untouched
            ("notes.Md", "notes", ".Md"),
            ("dir.d/file", "dir.d/file", ""),              // dot before separator
            ("path.with.dots/file.tar.gz", "path.with.dots/file.tar", ".gz"),
            ("x.zip/", "x.zip/", ""),                      // trailing separator wins
            ("x.zip/..", "x.zip/..", ""),
            ("MD/", "MD/", ""),
            ("weird name (1).md", "weird name (1)", ".md"),
            ("a. ", "a", ". "),                            // nothing is trimmed
            ("", "", ""),
            (".", ".", ""),
            ("..", "..", ""),
            ("...", "...", ""),
            ("C:\\", "C:\\", ""),
            ("C:\\a.", "C:\\a", "."),
            ("C:\\dir\\x.zip", "C:\\dir\\x", ".zip"),
            ("\\\\srv\\share\\a.zip", "\\\\srv\\share\\a", ".zip"),
            ("C:\\Program Files (x86)\\note.md", "C:\\Program Files (x86)\\note", ".md"),
        ];
        for (input, root, ext) in cases {
            let got = splitext_generic(input, true);
            assert_eq!((&got.0.as_str(), &got.1.as_str()), (root, ext), "nt splitext({:?})", input);
        }
    }

    /// `posixpath.splitext` binds the same generic with `sep='/'`, `altsep=None`,
    /// so a `\` inside a component is *not* a separator there and can end up
    /// inside the extension.  Pinned because this crate builds for Linux/macOS
    /// too, where `os.path` is `posixpath`.
    #[test]
    fn test_splitext_posix_flavor_matches_measured_python() {
        let cases: &[(&str, &str, &str)] = &[
            ("a.b\\c", "a", ".b\\c"),      // nt gives ("a.b\\c", "") — they diverge
            ("dir.d/file", "dir.d/file", ""),
            ("x.zip/", "x.zip/", ""),
            (".bashrc", ".bashrc", ""),
            ("a.tar.gz", "a.tar", ".gz"),
            ("C:\\x.zip", "C:\\x", ".zip"),
        ];
        for (input, root, ext) in cases {
            let got = splitext_generic(input, false);
            assert_eq!((&got.0.as_str(), &got.1.as_str()), (root, ext), "posix splitext({:?})", input);
        }
    }

    /// The whole gate, row by row against the authority.
    #[test]
    fn test_extension_gate_matrix_matches_python() {
        let rows: &[(&str, &[&str], bool)] = &[
            // (file, allowed_extensions, Python's measured verdict)
            ("readmd-g8-ARCHIVE.ZIP", &[".zip"], true),    // lowered both sides
            ("readmd-g8-archive.zip", &["zip"], true),     // dot-less entry → ".zip"
            ("readmd-g8-archive.zip", &[".ZIP"], true),
            ("readmd-g8-archive.zip", &[".zip", ".md"], true),
            ("readmd-g8-notes.zip", &["zip", "md"], true),
            ("readmd-g8-x.tar.gz", &[".gz"], true),        // last dot only
            ("readmd-g8-x.tar.gz", &[".tar.gz"], false),   // … so this does not fit
            ("readmd-g8-x.", &["."], true),                // trailing dot is "."
            ("readmd-g8-x.", &[""], true),                 // "" normalises to "."
            ("readmd-g8-.hidden.md", &[".md"], true),      // mid-name dot counts
            ("readmd-g8-a..b", &[".b"], true),
            ("readmd-g8-archive.zip", &[".md"], false),
            ("readmd-g8-noext", &[".zip"], false),         // ext "" is unmatched
            // A *leading* dot means no extension at all (`.bashrc` → `''`),
            // whereas the dot inside "readmd-g8-.hidden.md" is a real
            // extension (→ `.md`, the row above).  Measured, not assumed.
            (".readmd-g8-bashrc", &[".bashrc"], false),
            (".readmd-g8-bashrc", &[""], false),           // "" → ".", so still no
            (".readmd-g8-bashrc", &["zip"], false),
            ("readmd-g8-x.zip", &["."], false),            // "" never in the list
        ];
        for (path, exts, want_accept) in rows {
            let allowed: Vec<String> = exts.iter().map(|e| e.to_string()).collect();
            let got_accept = validate_file_path(path, Some(allowed), None).is_ok();
            assert_eq!(
                got_accept, *want_accept,
                "Python {}s {:?} against {:?}",
                if *want_accept { "accept" } else { "reject" },
                path,
                exts
            );
        }
    }

    /// `validators.py:49` is `if allowed_extensions:` — a truth test.  `[]`
    /// therefore means *no restriction*, not *nothing is allowed*.
    #[test]
    fn test_empty_allow_list_is_no_restriction_like_python() {
        for path in ["readmd-g8-x.txt", "readmd-g8-noext", "readmd-g8-.bashrc"] {
            let got = validate_file_path(path, Some(vec![]), None);
            assert!(got.is_ok(), "Python accepts {:?} with []: {:?}", path, got);
        }
    }

    /// Python interpolates the **raw** `ext` in the refusal, not the lowered one
    /// (`f'不支持的文件类型: {ext}'`, `validators.py:54`).
    #[test]
    fn test_extension_gate_error_reports_raw_extension() {
        let err = validate_file_path(
            "readmd-g8-REPORT.PDF",
            Some(vec![".zip".to_string()]),
            None,
        )
        .expect_err("measured: REJECT");
        assert!(
            err.message.ends_with(".PDF"),
            "expected the raw ext in the message, got {:?}",
            err.message
        );
    }
    
    #[test]
    fn test_validate_url_valid() {
        let result = validate_url("https://example.com/path", true);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_validate_url_invalid_scheme() {
        let result = validate_url("ftp://example.com", true);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_validate_url_ssrf() {
        let result = validate_url("http://127.0.0.1/admin", false);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_validate_command_valid() {
        let result = validate_command("echo hello world");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec!["echo", "hello", "world"]);
    }
    
    #[test]
    fn test_validate_command_shell_injection() {
        let result = validate_command("echo $(rm -rf /)");
        assert!(result.is_err());
    }

    // ------------------------------------------------ release filename / tag
    //
    // Every expected value below was produced by running CPython against the
    // real pattern / the real `src.readmd_core.versioning` helpers, not by
    // reading the Rust.

    /// `updater._UPDATE_FILENAME_RE` (`src/readmd_modules/updater.py:68-71`)
    /// transcribed literally: anchored `^`/`$`, `re.IGNORECASE` (`(?i)`), **no**
    /// `re.M`, body class `[A-Za-z0-9._()-]` **including the hyphen**, and
    /// `{0,180}` counting the characters *between* the mandatory first
    /// alphanumeric character and the extension dot.  `is_match` is a search,
    /// but a pattern anchored at both ends searches exactly like Python's
    /// `fullmatch` here (CPython still refuses `a.exe\n`, verified).
    ///
    /// This is the oracle for the rule, which lives in production code as
    /// `batch2::update_filename_re_ok` (`batch2.rs:722`) behind the trust gate
    /// at `batch2.rs:657`; the stale duplicate this module used to export is
    /// gone (see the note above `safe_update_target`).
    fn python_update_filename_re(name: &str) -> bool {
        regex::Regex::new(
            r"(?i)^[A-Za-z0-9][A-Za-z0-9._()-]{0,180}\.(?:exe|zip|dmg|deb|appimage)$",
        )
        .expect("updater.py:69-70")
        .is_match(name)
    }

    #[test]
    fn update_filename_re_python_oracle_updater_py_68_71() {
        // Longest accepted name: `1 + 180 + '.' + 'exe'` = 185 characters, and
        // `1 + 180 + '.' + 'appimage'` = 190 characters.
        let exe_max = format!("a{}.exe", "x".repeat(180));
        let appimage_max = format!("a{}.appimage", "x".repeat(180));
        assert_eq!(exe_max.chars().count(), 185);
        assert_eq!(appimage_max.chars().count(), 190);
        for name in [
            // Every one of these was `false` for the deleted helper, whose body
            // class omitted `-` and whose total cap was 182 characters: they are
            // how `match_release_asset` (`updater.py:124-201`) names packages,
            // so the gate refused every legitimate download.
            "ReadMD-2.3.9-win-x64.exe",
            "ReadMD-portable-2.4.0.exe",
            "ReadMDSetup-9.9.9.exe",
            "readmd_9.9.9_amd64.deb",
            "ReadMD-2.4.0-x86_64.AppImage",
            "ReadMD-macos-arm64.dmg",
            "ReadMD_1.0(1).exe",
            "a..exe",
            "a.exe",
            "A.EXE",
            "x.Zip",
            exe_max.as_str(),
            appimage_max.as_str(),
        ] {
            assert!(python_update_filename_re(name), "{name} must be accepted");
        }
        let exe_too_long = format!("a{}.exe", "x".repeat(181));
        let appimage_too_long = format!("a{}.appimage", "x".repeat(181));
        for name in [
            "-lead.exe",
            ".exe",
            "ReadMD (1).exe",
            "ReadMD;setup.exe",
            "a.zexe",
            "no_extension",
            "a.txt",
            "a.exe\n",
            "..",
            "x.exe.",
            "\u{e9}.exe",
            "x\0.exe",
            exe_too_long.as_str(),
            appimage_too_long.as_str(),
        ] {
            assert!(!python_update_filename_re(name), "{name} must be refused");
        }
    }

    /// `versioning.parse_version` does `str(value or '')` (`versioning.py:14`)
    /// and `select_update_release` feeds it `release.get('tag_name')`
    /// (`versioning.py:52`), so a JSON *number* is a legal tag.  The Rust read
    /// the field with `as_str()`, which turned every numeric tag into `""` and
    /// silently dropped the release.
    #[test]
    fn select_update_release_accepts_numeric_tags_versioning_py_14_52() {
        let pick = |tag: &serde_json::Value| {
            select_update_release("2.3.8", &[serde_json::json!({ "tag_name": tag.clone() })])
        };
        // `str(25)` == '25' -> core (25, 0, 0), newer than 2.3.8.
        assert_eq!(
            pick(&serde_json::json!(25)),
            Some(serde_json::json!({ "tag_name": 25 }))
        );
        // `str(2.5)` == '2.5' -> core (2, 5, 0).
        assert_eq!(
            pick(&serde_json::json!(2.5)),
            Some(serde_json::json!({ "tag_name": 2.5 }))
        );
        // Falsy tags collapse to `''` (`0 or ''`) and never parse: `0`, `0.0`,
        // `""`, `null`, `false`, `[]`, `{}` and an absent key alike.
        for tag in [
            serde_json::json!(0),
            serde_json::json!(0.0),
            serde_json::json!(""),
            serde_json::json!(null),
            serde_json::json!(false),
            serde_json::json!([]),
            serde_json::json!({}),
        ] {
            assert_eq!(pick(&tag), None, "{tag} must be dropped as falsy");
        }
        assert_eq!(
            select_update_release("2.3.8", &[serde_json::json!({})]),
            None,
            "`release.get('tag_name')` missing is `None` -> `''`"
        );
        // Truthy but unparseable: `str(True)` == 'True', `str(-5)` == '-5', and
        // CPython prints 1e16 / 1e-5 in exponent form, which `_VERSION_RE`
        // rejects.
        for tag in [
            serde_json::json!(true),
            serde_json::json!(-5),
            serde_json::json!(1e16),
            serde_json::json!(1e-5),
            serde_json::json!(["9.9.9"]),
        ] {
            assert_eq!(pick(&tag), None, "{tag} must be unparseable");
        }
        // A `v` prefix and whitespace are handled by the shared parser.
        assert_eq!(
            pick(&serde_json::json!(" v9.9.9 ")),
            Some(serde_json::json!({ "tag_name": " v9.9.9 " })),
            "`str(...).strip()` then `[vV]?`"
        );
    }

    /// `versioning.py:47-58`: `draft` truthiness, the one-way channel rule, the
    /// tie-break of `max`, and `None` when nothing survives.
    #[test]
    fn select_update_release_channel_and_tie_rules_versioning_py_47_58() {
        let releases = || {
            serde_json::json!([
                { "tag_name": "9.0.0", "name": "A" },
                { "tag_name": "9.0.0", "name": "B" },
            ])
            .as_array()
            .cloned()
            .unwrap()
        };
        // `max` keeps the *first* maximal candidate.
        assert_eq!(
            select_update_release("2.3.8", &releases())
                .and_then(|r| r.get("name").cloned()),
            Some(serde_json::json!("A")),
            "ties keep the first candidate (`versioning.py:58`)"
        );
        // A formal build never leaves the formal channel: a prerelease suffix or
        // a truthy `prerelease` flag both disqualify the candidate.
        let suffixed = serde_json::json!([
            { "tag_name": "9.0.0-rc.1" },
            { "tag_name": "9.0.0", "prerelease": true },
            { "tag_name": "9.0.0-beta.9" },
        ])
        .as_array()
        .cloned()
        .unwrap();
        assert_eq!(select_update_release("2.3.8", &suffixed), None);
        // ... while a prerelease build may advance to another prerelease *and*
        // to a `prerelease: true` formal-looking tag.
        assert_eq!(
            select_update_release("2.3.8-rc.1", &suffixed)
                .and_then(|r| r.get("tag_name").cloned()),
            Some(serde_json::json!("9.0.0")),
            "highest parsed tuple wins for a prerelease current build"
        );
        // Numeric prerelease identifiers rank below words (`versioning.py:23`),
        // so `-1` loses to `-rc` at the same core.
        let ids = serde_json::json!([
            { "tag_name": "9.0.0-1" },
            { "tag_name": "9.0.0-rc" },
        ])
        .as_array()
        .cloned()
        .unwrap();
        assert_eq!(
            select_update_release("2.3.8-rc.1", &ids)
                .and_then(|r| r.get("tag_name").cloned()),
            Some(serde_json::json!("9.0.0-rc")),
            "`(1, 'rc') > (0, 1)` in the prerelease tuple"
        );
        // `release.get('draft')` is truthiness, not `is True`.
        for draft in [
            serde_json::json!(true),
            serde_json::json!(1),
            serde_json::json!("yes"),
            serde_json::json!([0]),
        ] {
            let label = draft.to_string();
            let list = serde_json::json!([{ "tag_name": "9.0.0", "draft": draft }])
                .as_array()
                .cloned()
                .unwrap();
            assert_eq!(
                select_update_release("2.3.8", &list),
                None,
                "draft {label} must be skipped"
            );
        }
        // Non-dict candidates and an unparseable current version.
        assert_eq!(
            select_update_release("2.3.8", &serde_json::json!(["9.0.0"]).as_array().cloned().unwrap()),
            None,
            "`isinstance(release, dict)` guard (`versioning.py:50`)"
        );
        assert_eq!(
            select_update_release("not-a-version", &releases()),
            None,
            "`current is None` short-circuits (`versioning.py:45-46`)"
        );
    }

    /// `_safe_update_target`'s fourth guard (`updater.py:84`) is a *different*,
    /// stricter pattern than `_UPDATE_FILENAME_RE`: no `re.IGNORECASE`, so only
    /// the exact spelling `AppImage` is legal, and `[A-Za-z0-9._-]+` requires a
    /// second stem character.  The Rust case-folded the extension and used
    /// `skip(1).all(..)`, which passed an empty remainder, so it accepted
    /// `x.exe`, `xa.appimage` and `xa.APPIMAGE` — all of which CPython refuses.
    #[test]
    fn safe_update_target_fourth_guard_updater_py_84() {
        let temp = std::env::temp_dir();
        let guard = |name: &str| safe_update_target(name, &temp);
        for name in [
            "xa.exe",
            "ReadMD-portable-9.9.9.exe",
            "readmd_9.9.9_amd64.deb",
            "ReadMD-macos-x86_64.zip",
            "ReadMD-2.4.0-x86_64.AppImage",
            "xy.a.exe",
        ] {
            assert_eq!(
                guard(name),
                Ok(temp.join("ReadMDUpdates").join(name)),
                "{name} must pass `updater.py:84`"
            );
        }
        for name in [
            "x.exe",
            "xa.appimage",
            "xa.APPIMAGE",
            "x.dmg",
            "a.EXE",
            "ReadMD_1.0(1).exe",
            "a",
            "exe",
            "x.",
            ".exe",
        ] {
            assert_eq!(
                guard(name),
                Err("不支持的更新包类型".to_string()),
                "{name} must fail the extension/body guard"
            );
        }
        // The three earlier guards, in Python's order and with Python's message.
        assert_eq!(
            guard("sub\\x.exe"),
            Err("更新文件名不能包含路径".to_string()),
            "'\\\\' is checked on the *raw* name (`updater.py:77`)"
        );
        assert_eq!(
            guard(".."),
            Err("无效的更新文件名".to_string()),
            "`name in ('.', '..')` (`updater.py:80`)"
        );
        assert_eq!(
            guard(&format!("{}x.exe", "y".repeat(201))),
            Err("无效的更新文件名".to_string()),
            "`len(name) > 200` (`updater.py:80`)"
        );
        assert_eq!(
            guard("a:b.exe"),
            Err("更新文件名包含非法字符".to_string()),
            "`re.search(r'[\\\\/:*?\"<>|\\x00-\\x1f]', name)` (`updater.py:82`)"
        );
        assert_eq!(
            guard("  xa.exe  "),
            Ok(temp.join("ReadMDUpdates").join("xa.exe")),
            "`raw_name.strip()` happens before the guards, so the joined name is trimmed"
        );
    }
}
