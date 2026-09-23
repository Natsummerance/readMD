//! Parity port of `src/readmd_modules/link_indexer.py` — the local backlink
//! index and knowledge-graph engine.
//!
//! Every rule is a transcription of the reference implementation, which is the
//! single source of truth for the `/api/links/*` contract:
//!
//! * [`extract_links`] mirrors `LinkIndexer.extract_links` (code masking, then
//!   the wikilink pass, then the markdown-link pass, per line).
//! * [`LinkIndexer::index_directory`] mirrors `index_directory` including
//!   `os.walk` discovery order, mtime incrementality and the deferred
//!   re-resolution pass (its step 6).
//! * [`LinkIndexer::resolve_target`] mirrors the resolution ladder: relative
//!   path, `.md` completion, then lowercased-basename fallback (first hit in
//!   walk order wins).
//! * [`LinkIndexer::get_graph_data`] mirrors the `LIMIT`-clamped document
//!   selection, where *rowid order* decides which documents survive
//!   truncation — not path order.
//!
//! Deliberate reproductions of reference quirks (do not "fix" them here):
//! `size` counts characters of the newline-translated text; `[[#heading]]`
//! resolves to its own file; deadlink graph nodes may push `total_nodes` above
//! `max_nodes`; `is_wikilink` is a SQLite integer so database-backed rows
//! serialise it as `1`/`0` while graph edges serialise a real boolean.

use rusqlite::{params, Connection, Row};
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// `MD_EXTENSIONS` — matched against the whole lowercased filename.
pub const MD_EXTENSIONS: &[&str] = &[".md", ".markdown", ".mdown", ".mkd"];

/// Separator used by the reference `os.path` on this platform.
pub const SEP: char = if cfg!(windows) { '\\' } else { '/' };

fn is_sep(c: char) -> bool {
    c == '/' || (cfg!(windows) && c == '\\')
}

fn is_path_sep(c: char) -> bool {
    if cfg!(windows) {
        c == '\\'
    } else {
        c == '/'
    }
}

// --------------------------------------------------------------------------
// python compatibility primitives (ntpath / str semantics used by the module)
// --------------------------------------------------------------------------

/// `Py_UNICODE_ISSPACE`: the Unicode `White_Space` property plus U+001C..U+001F
/// that CPython also counts as whitespace.
pub fn py_isspace(c: char) -> bool {
    c.is_whitespace() || matches!(c, '\u{1c}'..='\u{1f}')
}

/// `str.strip()` with CPython's whitespace set.
pub fn py_strip(s: &str) -> &str {
    s.trim_matches(py_isspace as fn(char) -> bool)
}

/// `str.splitlines()` boundaries — a superset of `\n`, used by
/// [`extract_title`] exactly like `_extract_title`.
fn is_line_break(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{b}' | '\u{c}' | '\u{1c}'..='\u{1e}' | '\u{85}' | '\u{2028}' | '\u{2029}')
}

/// `str.splitlines()` without keepends.
pub fn py_splitlines(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut iter = text.char_indices().peekable();
    while let Some((i, c)) = iter.next() {
        if !is_line_break(c) {
            continue;
        }
        out.push(&text[start..i]);
        let mut next_past = i + c.len_utf8();
        if c == '\r' {
            if let Some(&(j, '\n')) = iter.peek() {
                iter.next();
                next_past = j + 1;
            }
        }
        start = next_past;
    }
    if start < text.len() {
        out.push(&text[start..]);
    }
    out
}

/// `ntpath.splitdrive` (CPython 3.11): a UNC/device prefix runs up to the
/// fourth separator, a drive letter is the two characters before `:`, anything
/// else has an empty drive.
fn nt_splitdrive(p: &str) -> (String, String) {
    let normp: Vec<char> = p.chars().map(|c| if c == '/' { '\\' } else { c }).collect();
    if p.chars().count() >= 2 {
        if normp[0] == '\\' && normp[1] == '\\' {
            let unc: [char; 8] = ['\\', '\\', '?', '\\', 'U', 'N', 'C', '\\'];
            let start = if normp.len() >= 8
                && normp[..8].iter().zip(unc.iter()).all(|(a, b)| a.to_ascii_uppercase() == *b)
            {
                8
            } else {
                2
            };
            if let Some(index) = (start..normp.len()).find(|&i| normp[i] == '\\') {
                if let Some(index2) = (index + 1..normp.len()).find(|&i| normp[i] == '\\') {
                    let cut = byte_index_at_char(p, index2);
                    return (p[..cut].to_string(), p[cut..].to_string());
                }
            }
            return (p.to_string(), String::new());
        }
        if normp[1] == ':' {
            let cut = byte_index_at_char(p, 2);
            return (p[..cut].to_string(), p[cut..].to_string());
        }
    }
    (String::new(), p.to_string())
}

/// Collapse `.`/`..`/repeated separators exactly like `ntpath.normpath`'s
/// component loop. `absolute` (the prefix already ends in a separator) blocks
/// `..` from climbing above the anchor.
fn norm_comps(rest: &str, absolute: bool) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for comp in rest.split(is_path_sep) {
        if comp.is_empty() || comp == "." {
            continue;
        }
        if comp == ".." {
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
        parts.push(comp);
    }
    parts.join(&SEP.to_string())
}

/// `os.path.normpath`.
pub fn py_normpath(path: &str) -> String {
    if !cfg!(windows) {
        return posix_normpath(path);
    }
    if path.is_empty() {
        return ".".to_string();
    }
    let normp = path.replace('/', "\\");
    let (prefix0, rest0) = nt_splitdrive(&normp);
    let mut prefix = prefix0;
    let mut rest = rest0;
    // `if path.startswith(sep): prefix += sep; path = path.lstrip(sep)`
    let absolute = rest.starts_with('\\');
    if absolute {
        prefix.push('\\');
        rest = rest.trim_start_matches('\\').to_string();
    }
    let body = norm_comps(&rest, absolute);
    if prefix.is_empty() && body.is_empty() {
        return ".".to_string();
    }
    format!("{prefix}{body}")
}

/// `posixpath.normpath`, for non-Windows builds of the kernel.
fn posix_normpath(path: &str) -> String {
    if path.is_empty() {
        return ".".to_string();
    }
    let mut slashes = 0usize;
    if path.starts_with('/') {
        slashes = 1;
        if path.starts_with("//") && !path.starts_with("///") {
            slashes = 2;
        }
    }
    let rest = &path[slashes..];
    let body = norm_comps_posix(rest, slashes > 0);
    if body.is_empty() {
        return if slashes > 0 { "/".repeat(slashes) } else { ".".to_string() };
    }
    format!("{}{}", "/".repeat(slashes), body)
}

fn norm_comps_posix(rest: &str, absolute: bool) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for comp in rest.split('/') {
        if comp.is_empty() || comp == "." {
            continue;
        }
        if comp == ".." {
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
        parts.push(comp);
    }
    parts.join("/")
}

fn current_dir() -> String {
    std::env::current_dir()
        .ok()
        .and_then(|p| p.to_str().map(str::to_string))
        .unwrap_or_else(|| ".".to_string())
}

/// `os.path.abspath` (`nt._getfullpathname` on Windows).
pub fn py_abspath(path: &str) -> String {
    let cwd = current_dir();
    if !cfg!(windows) {
        return py_normpath(&if path.starts_with('/') {
            path.to_string()
        } else {
            py_join(&cwd, path)
        });
    }
    if path.is_empty() {
        return py_normpath(&cwd);
    }
    let flat = path.replace('/', "\\");
    let (drv, rest) = nt_splitdrive(&flat);
    let cwd_drive = nt_splitdrive(&cwd).0;
    let same_drive =
        !cwd_drive.is_empty() && cwd_drive.to_ascii_lowercase() == drv.to_ascii_lowercase();
    if drv.is_empty() {
        if rest.starts_with('\\') {
            // Rooted on the current drive: the reference keeps only the drive.
            return py_normpath(&format!("{cwd_drive}{flat}"));
        }
        return py_normpath(&py_join(&cwd, path));
    }
    if rest.starts_with('\\') {
        return py_normpath(&flat);
    }
    if rest.is_empty() {
        // `C:` / `T:` / `\\srv\share`
        if drv.chars().count() == 2 && drv.ends_with(':') {
            return if same_drive {
                py_normpath(&cwd)
            } else {
                py_normpath(&format!("{drv}\\"))
            };
        }
        return py_normpath(&flat);
    }
    // Drive-relative: Windows only keeps a per-process cwd for the drive the
    // current directory is on, other drives resolve to their root.
    if same_drive {
        return py_normpath(&py_join(&cwd, &rest));
    }
    py_normpath(&format!("{drv}\\{rest}"))
}

/// `os.path.join` restricted to the two-argument shapes the reference uses.
pub fn py_join(a: &str, b: &str) -> String {
    if !cfg!(windows) {
        if b.starts_with('/') || a.is_empty() {
            return b.to_string();
        }
        return if a.ends_with('/') {
            format!("{a}{b}")
        } else {
            format!("{a}/{b}")
        };
    }
    let (result_drive, result_path) = nt_splitdrive(a);
    let (p_drive, p_path) = nt_splitdrive(b);
    let second_is_absolute = !p_path.is_empty() && is_sep(p_path.chars().next().unwrap());
    let mut drive = result_drive.clone();
    let path = if second_is_absolute {
        // `if p_path and p_path[0] in seps`: absolute remainder wins, the
        // first path only contributes its drive when the second has none.
        if !p_drive.is_empty() || drive.is_empty() {
            drive = p_drive.clone();
        }
        p_path.clone()
    } else if !p_drive.is_empty() && p_drive != result_drive {
        if p_drive.to_ascii_lowercase() != result_drive.to_ascii_lowercase() {
            // Different drives => ignore the first path entirely
            drive = p_drive.clone();
            p_path.clone()
        } else {
            // Same drive in a different case
            drive = p_drive.clone();
            join_append(result_path, &p_path)
        }
    } else {
        join_append(result_path, &p_path)
    };
    // add separator between UNC and non-absolute path
    if !path.is_empty() && !is_sep(path.chars().next().unwrap()) && !drive.is_empty() && !drive.ends_with(':') {
        return format!("{drive}{SEP}{path}");
    }
    format!("{drive}{path}")
}

/// `result_path[-1] not in seps -> result_path += sep` then concatenate.
fn join_append(mut result_path: String, p_path: &str) -> String {
    if !result_path.is_empty() && !is_sep(result_path.chars().last().unwrap()) {
        result_path.push(SEP);
    }
    result_path.push_str(p_path);
    result_path
}

/// `os.path.split`, the exact `ntpath.split`/`posixpath.split` algorithm.
pub fn py_split(path: &str) -> (String, String) {
    if !cfg!(windows) {
        let i = match path.rfind('/') {
            Some(x) => x + 1,
            None => 0,
        };
        let head = &path[..i];
        let tail = &path[i..];
        let mut h = head.to_string();
        if !h.is_empty() && !h.chars().all(|c| c == '/') {
            h = head.trim_end_matches('/').to_string();
        }
        return (h, tail.to_string());
    }
    let (drive, rest) = nt_splitdrive(path);
    let cs: Vec<char> = rest.chars().collect();
    // `i` = index just past the last separator of the remainder
    let mut i = cs.len();
    while i > 0 && !is_sep(cs[i - 1]) {
        i -= 1;
    }
    let raw_head: String = cs[..i].iter().collect();
    let tail: String = cs[i..].iter().collect();
    // `head.rstrip(seps) or head`
    let stripped = raw_head.trim_end_matches(is_sep).to_string();
    let head = if stripped.is_empty() { raw_head } else { stripped };
    (format!("{drive}{head}"), tail)
}

/// `os.path.dirname(p)` = `split(p)[0]`.
pub fn py_dirname(path: &str) -> String {
    py_split(path).0
}

/// `os.path.basename(p)` = `split(p)[1]`.
pub fn py_basename(path: &str) -> String {
    py_split(path).1
}

/// `os.path.splitext` via `genericpath._splitext`.
pub fn py_splitext(path: &str) -> (String, String) {
    let cs: Vec<char> = path.chars().collect();
    let mut sep_index: isize = -1;
    let mut dot_index: isize = -1;
    for (i, c) in cs.iter().enumerate() {
        if is_sep(*c) {
            sep_index = i as isize;
        } else if *c == '.' {
            dot_index = i as isize;
        }
    }
    if dot_index > sep_index {
        let dot = dot_index as usize;
        let mut filename_index = (sep_index + 1) as usize;
        // `while filenameIndex < dotIndex: if p[filenameIndex] != '.': return split`
        let mut found = false;
        while filename_index < dot {
            if cs[filename_index] != '.' {
                found = true;
                break;
            }
            filename_index += 1;
        }
        if found {
            let cut = byte_index_at_char(path, dot);
            return (path[..cut].to_string(), path[cut..].to_string());
        }
    }
    (path.to_string(), String::new())
}

fn byte_index_at_char(s: &str, char_idx: usize) -> usize {
    s.char_indices().nth(char_idx).map(|(i, _)| i).unwrap_or(s.len())
}

/// `ntpath.normcase`: slashes to backslashes, invariant lowercase.
fn py_normcase(s: &str) -> String {
    s.replace('/', "\\").to_lowercase()
}

/// `os.path.relpath(path, start)`.
pub fn py_relpath(path: &str, start: &str) -> String {
    let start_abs = py_abspath(&py_normpath(start));
    let path_abs = py_abspath(&py_normpath(path));
    let (start_drive, start_rest) = nt_splitdrive(&start_abs);
    let (path_drive, path_rest) = nt_splitdrive(&path_abs);
    if py_normcase(&start_drive) != py_normcase(&path_drive) {
        // The reference raises ValueError here; `_get_node_id` never reaches it
        // because it only calls relpath after a `startswith` test.
        return py_basename(path);
    }
    let start_list: Vec<&str> = start_rest.split(is_path_sep).filter(|x| !x.is_empty()).collect();
    let path_list: Vec<&str> = path_rest.split(is_path_sep).filter(|x| !x.is_empty()).collect();
    let mut i = 0usize;
    while i < start_list.len() && i < path_list.len() {
        if py_normcase(start_list[i]) != py_normcase(path_list[i]) {
            break;
        }
        i += 1;
    }
    let mut rel_list: Vec<String> = vec!["..".to_string(); start_list.len() - i];
    rel_list.extend(path_list[i..].iter().map(|s| s.to_string()));
    if rel_list.is_empty() {
        return ".".to_string();
    }
    let mut out = rel_list[0].clone();
    for part in &rel_list[1..] {
        out = py_join(&out, part);
    }
    out
}

/// `os.path.relpath(full_path, norm_root).replace('\\', '/')`, the body of
/// `_get_node_id` once the path is known to sit under the root.
pub fn py_relpath_under(path: &str, start: &str) -> String {
    py_relpath(path, start).replace('\\', "/")
}

/// `_root_pattern`: the rooted directory prefix with SQL wildcards escaped.
pub fn root_pattern(root_dir: &str) -> String {
    let prefix = py_join(&py_abspath(root_dir), "");
    let escaped = prefix.replace('!', "!!").replace('%', "!%").replace('_', "!_");
    format!("{escaped}%")
}

/// `str(bool)`/`str(int)`/`str(None)` for `str(body.get('dir',''))`.
pub fn py_str_value(v: &Value) -> String {
    match v {
        Value::Null => "None".to_string(),
        Value::Bool(b) => (if *b { "True" } else { "False" }).to_string(),
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Array(items) => {
            format!("[{}]", items.iter().map(py_str_repr).collect::<Vec<_>>().join(", "))
        }
        Value::Object(map) => format!(
            "{{{}}}",
            map.iter().map(|(k, v)| format!("'{k}': {}", py_str_repr(v))).collect::<Vec<_>>().join(", ")
        ),
    }
}

fn py_str_repr(v: &Value) -> String {
    match v {
        Value::String(s) => format!("'{s}'"),
        other => py_str_value(other),
    }
}

/// `bool(x)` for JSON values.
pub fn py_truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => match n.as_f64() {
            Some(f) => f != 0.0,
            None => true,
        },
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

/// `int(str.strip())`, including CPython's digit-separator rule.
pub fn py_int(s: &str) -> Option<i64> {
    let t = py_strip(s);
    let bytes = t.as_bytes();
    let mut negative = false;
    let mut start = 0usize;
    if matches!(bytes.first(), Some(b'+') | Some(b'-')) {
        negative = bytes[0] == b'-';
        start = 1;
    }
    let digits = &t[start..];
    if digits.is_empty() || digits.starts_with('_') || digits.ends_with('_') || digits.contains("__") {
        return None;
    }
    let mut acc: i64 = 0;
    for c in digits.chars() {
        if c == '_' {
            continue;
        }
        acc = acc.saturating_mul(10).saturating_add((c as u8 - b'0') as i64);
    }
    Some(if negative { -acc } else { acc })
}

/// `urllib.parse.unquote`: percent decoding whose UTF-8 replacement happens
/// per ASCII run, exactly like CPython's `_asciire` split.
pub fn py_unquote(s: &str) -> String {
    if !s.contains('%') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    for (is_ascii, chunk) in ascii_runs(s) {
        if !is_ascii {
            out.push_str(chunk);
            continue;
        }
        if chunk.contains('%') {
            out.push_str(&String::from_utf8_lossy(&unquote_to_bytes(chunk)));
        } else {
            out.push_str(chunk);
        }
    }
    out
}

fn ascii_runs(s: &str) -> Vec<(bool, &str)> {
    let bytes = s.as_bytes();
    let mut out: Vec<(bool, &str)> = Vec::new();
    if bytes.is_empty() {
        return out;
    }
    let mut start = 0usize;
    let mut cur = bytes[0].is_ascii();
    let mut i = 0usize;
    while i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii() == cur {
            i += 1;
        }
        if i > start {
            out.push((cur, &s[start..i]));
            start = i;
            cur = !cur;
        }
    }
    let _ = start;
    out
}

/// `urllib.parse.unquote_to_bytes`.
fn unquote_to_bytes(s: &str) -> Vec<u8> {
    let mut res: Vec<u8> = Vec::with_capacity(s.len());
    let mut parts = s.split('%');
    res.extend(parts.next().unwrap_or("").bytes());
    for item in parts {
        let ib = item.as_bytes();
        match hex_byte(&ib[..ib.len().min(2)]) {
            Some(b) => {
                res.push(b);
                res.extend(&ib[2..]);
            }
            None => {
                res.push(b'%');
                res.extend(ib);
            }
        }
    }
    res
}

fn hex_byte(b: &[u8]) -> Option<u8> {
    if b.len() != 2 {
        return None;
    }
    let hi = (b[0] as char).to_digit(16)?;
    let lo = (b[1] as char).to_digit(16)?;
    Some((hi * 16 + lo) as u8)
}

/// `open(p, 'r', encoding='utf-8', errors='replace').read()` — lossy decode
/// plus universal-newline translation.
pub fn read_text_pythonic(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    let decoded = String::from_utf8_lossy(&bytes).into_owned();
    if decoded.contains('\r') {
        Some(decoded.replace("\r\n", "\n").replace('\r', "\n"))
    } else {
        Some(decoded)
    }
}

fn mtime_of(path: &Path) -> Option<f64> {
    let t = fs::metadata(path).ok()?.modified().ok()?;
    match t.duration_since(UNIX_EPOCH) {
        Ok(d) => Some(d.as_secs_f64()),
        Err(e) => Some(-e.duration().as_secs_f64()),
    }
}

fn now_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

// --------------------------------------------------------------------------
// extraction
// --------------------------------------------------------------------------

/// One link, the `dict` emitted by `extract_links`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedLink {
    pub target_raw: String,
    pub target_clean: String,
    pub alias: Option<String>,
    pub heading: Option<String>,
    pub is_wikilink: bool,
    pub line_no: i64,
}

impl ExtractedLink {
    pub fn to_json(&self) -> Value {
        json_obj([
            ("target_raw", Value::String(self.target_raw.clone())),
            ("target_clean", Value::String(self.target_clean.clone())),
            ("alias", opt_str(&self.alias)),
            ("heading", opt_str(&self.heading)),
            ("is_wikilink", Value::Bool(self.is_wikilink)),
            ("line_no", Value::from(self.line_no)),
        ])
    }
}

fn opt_str(v: &Option<String>) -> Value {
    match v {
        Some(s) => Value::String(s.clone()),
        None => Value::Null,
    }
}

fn json_obj<const N: usize>(items: [(&str, Value); N]) -> Value {
    let mut m = Map::new();
    for (k, v) in items {
        m.insert(k.to_string(), v);
    }
    Value::Object(m)
}

/// `_RE_FENCED_CODE` (` ``` ` or `~~~` spans, non-greedy, newlines kept).
fn mask_fenced(text: &str) -> String {
    let cs: Vec<char> = text.chars().collect();
    let n = cs.len();
    let tick: [char; 3] = ['`', '`', '`'];
    let tilde: [char; 3] = ['~', '~', '~'];
    let mut out = String::with_capacity(text.len());
    let mut i = 0usize;
    while i < n {
        let marker = if i + 3 <= n && cs[i..i + 3] == tick {
            tick
        } else if i + 3 <= n && cs[i..i + 3] == tilde {
            tilde
        } else {
            out.push(cs[i]);
            i += 1;
            continue;
        };
        match fence_close(&cs, i, &marker) {
            Some(end) => {
                for c in &cs[i..end] {
                    out.push(if *c == '\n' { '\n' } else { ' ' });
                }
                i = end;
            }
            None => {
                out.push(cs[i]);
                i += 1;
            }
        }
    }
    out
}

/// End (exclusive) of the shortest `marker…marker` span opened at `open`.
fn fence_close(cs: &[char], open: usize, marker: &[char; 3]) -> Option<usize> {
    let n = cs.len();
    let mut j = open + 3;
    while j + 3 <= n {
        if cs[j..j + 3] == *marker {
            return Some(j + 3);
        }
        j += 1;
    }
    None
}

/// `_RE_INLINE_CODE`: `` `[^`\n]+` `` becomes spaces.
fn mask_inline(text: &str) -> String {
    let cs: Vec<char> = text.chars().collect();
    let n = cs.len();
    let mut out = cs.clone();
    let mut i = 0usize;
    while i < n {
        if cs[i] == '`' {
            let mut j = i + 1;
            while j < n && cs[j] != '`' && cs[j] != '\n' {
                j += 1;
            }
            if j > i + 1 && j < n && cs[j] == '`' {
                for k in i..=j {
                    out[k] = ' ';
                }
                i = j + 1;
                continue;
            }
        }
        i += 1;
    }
    out.into_iter().collect()
}

/// `_mask_code_blocks`.
pub fn mask_code_blocks(text: &str) -> String {
    mask_inline(&mask_fenced(text))
}

/// `_RE_WIKILINK = \[\[([^\]\n]+)\]\]` on one line, leftmost-first.
fn wikilinks_in_line(line: &str) -> Vec<String> {
    let cs: Vec<char> = line.chars().collect();
    let n = cs.len();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i + 5 <= n {
        if !(cs[i] == '[' && cs[i + 1] == '[') {
            i += 1;
            continue;
        }
        let mut j = i + 2;
        while j < n && cs[j] != ']' {
            j += 1;
        }
        if j > i + 2 && j + 1 < n && cs[j] == ']' && cs[j + 1] == ']' {
            out.push(cs[i + 2..j].iter().collect());
            i = j + 2;
            continue;
        }
        i += 1;
    }
    out
}

/// `_RE_MD_LINK = (?<!!)\[([^\]]+)\]\(([^)\s]+)(?:\s+"[^"]*")?\)` on one line.
/// The reference's greedy runs leave no productive backtracking (the maximal
/// run is the only viable extent, and the title branch and the bare `)` branch
/// are mutually exclusive), so this scan is deterministic.
fn md_links_in_line(line: &str) -> Vec<(String, String)> {
    let cs: Vec<char> = line.chars().collect();
    let n = cs.len();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < n {
        if cs[i] != '[' || (i > 0 && cs[i - 1] == '!') {
            i += 1;
            continue;
        }
        let mut j = i + 1;
        while j < n && cs[j] != ']' {
            j += 1;
        }
        if j == i + 1 || j >= n || j + 1 >= n || cs[j + 1] != '(' {
            i += 1;
            continue;
        }
        let alias: String = cs[i + 1..j].iter().collect();
        let mut k = j + 2;
        while k < n && cs[k] != ')' && !py_isspace(cs[k]) {
            k += 1;
        }
        if k == j + 2 || k >= n {
            i += 1;
            continue;
        }
        let url: String = cs[j + 2..k].iter().collect();
        let mut end = None;
        if cs[k] == ')' {
            end = Some(k + 1);
        } else {
            let mut p = k;
            while p < n && py_isspace(cs[p]) {
                p += 1;
            }
            if p < n && cs[p] == '"' {
                let mut q = p + 1;
                while q < n && cs[q] != '"' {
                    q += 1;
                }
                if q < n && q + 1 < n && cs[q + 1] == ')' {
                    end = Some(q + 2);
                }
            }
        }
        match end {
            Some(e) => {
                out.push((alias, url));
                i = e;
            }
            None => i += 1,
        }
    }
    out
}

/// `_RE_EXTERNAL_URL.search` — anchored, so a prefix test.
fn is_external_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    ["http://", "https://", "ftp://", "mailto:", "data:", "#"]
        .iter()
        .any(|p| lower.starts_with(p))
}

/// `LinkIndexer.extract_links`.
pub fn extract_links(content: &str) -> Vec<ExtractedLink> {
    if content.is_empty() {
        return Vec::new();
    }
    let masked = mask_code_blocks(content);
    let mut links = Vec::new();
    for (idx, line) in masked.split('\n').enumerate() {
        let line_no = idx as i64 + 1;
        // Py 111 `if not line.strip(): continue` — a line of bare U+001C..U+001F
        // separators is blank to CPython but not to Rust's `trim()`.
        if py_strip(line).is_empty() {
            continue;
        }
        // 1. `[[wikilinks]]`
        for raw in wikilinks_in_line(line) {
            let raw_inner = py_strip(&raw).to_string();
            if raw_inner.is_empty() {
                continue;
            }
            let (target_part, alias) = match raw_inner.split_once('|') {
                Some((t, a)) => {
                    let a = py_strip(a).to_string();
                    (py_strip(t).to_string(), if a.is_empty() { None } else { Some(a) })
                }
                None => (raw_inner.clone(), None),
            };
            let (mut target_clean, heading) = match target_part.split_once('#') {
                Some((t, h)) => {
                    let h = py_strip(h).to_string();
                    (py_strip(t).to_string(), if h.is_empty() { None } else { Some(h) })
                }
                None => (target_part, None),
            };
            if target_clean.is_empty() {
                if let Some(h) = &heading {
                    target_clean = format!("#{h}");
                }
            }
            links.push(ExtractedLink {
                target_raw: raw_inner,
                target_clean,
                alias,
                heading,
                is_wikilink: true,
                line_no,
            });
        }
        // 2. `[text](url)`
        for (alias_text, raw_url) in md_links_in_line(line) {
            let alias_text = py_strip(&alias_text).to_string();
            let raw_url = py_strip(&raw_url).to_string();
            if raw_url.is_empty() || is_external_url(&raw_url) {
                continue;
            }
            let url = py_unquote(&raw_url);
            let (target_clean, heading) = match url.split_once('#') {
                Some((t, h)) => {
                    let h = py_strip(h).to_string();
                    (py_strip(t).to_string(), if h.is_empty() { None } else { Some(h) })
                }
                None => (url, None),
            };
            if target_clean.is_empty() {
                continue;
            }
            links.push(ExtractedLink {
                target_raw: raw_url,
                target_clean,
                alias: if alias_text.is_empty() { None } else { Some(alias_text) },
                heading,
                is_wikilink: false,
                line_no,
            });
        }
    }
    links
}

/// `_extract_title` — note it never returns null: the fallback is the basename.
pub fn extract_title(content: &str, fallback: &str) -> String {
    for line in py_splitlines(content) {
        let line_s = py_strip(line);
        if line_s.starts_with("# ") {
            return py_strip(&line_s[2..]).to_string();
        }
    }
    fallback.to_string()
}

// --------------------------------------------------------------------------
// indexer
// --------------------------------------------------------------------------

/// `LinkIndexer` over a SQLite file laid out exactly like the reference.
pub struct LinkIndexer {
    conn: Mutex<Connection>,
    pub db_path: String,
}

/// `index_directory` stats.
#[derive(Debug, Clone, Default, Copy)]
pub struct IndexStats {
    pub scanned_count: i64,
    pub indexed_count: i64,
    pub deleted_count: i64,
}

impl IndexStats {
    pub fn to_json(&self) -> Value {
        json_obj([
            ("scanned_count", Value::from(self.scanned_count)),
            ("indexed_count", Value::from(self.indexed_count)),
            ("deleted_count", Value::from(self.deleted_count)),
        ])
    }
}

struct GraphLink {
    source_path: String,
    target_clean: String,
    target_path: Option<String>,
    alias: Option<String>,
    is_wikilink: i64,
}

impl LinkIndexer {
    /// `LinkIndexer.__init__`.
    pub fn new<P: AsRef<Path>>(db_path: P) -> Result<LinkIndexer, String> {
        let abs = py_abspath(&db_path.as_ref().to_string_lossy());
        if let Some(i) = abs.rfind(['/', '\\']) {
            let _ = fs::create_dir_all(&abs[..i + 1]);
        }
        let conn = Connection::open(&abs).map_err(|e| format!("Failed to open database: {e}"))?;
        let indexer = LinkIndexer { conn: Mutex::new(conn), db_path: abs };
        indexer.init_db()?;
        Ok(indexer)
    }

    /// `_init_db`.
    fn init_db(&self) -> Result<(), String> {
        let conn = self.lock()?;
        let _: Option<String> = conn.query_row("PRAGMA journal_mode=WAL;", [], |r| r.get(0)).ok();
        conn.execute("PRAGMA synchronous=NORMAL;", []).map_err(|e| e.to_string())?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS documents (
                doc_id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT UNIQUE NOT NULL,
                title TEXT,
                mtime REAL NOT NULL,
                size INTEGER NOT NULL,
                scanned_at REAL NOT NULL
            )",
            [],
        )
        .map_err(|e| e.to_string())?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS links (
                link_id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_path TEXT NOT NULL,
                target_raw TEXT NOT NULL,
                target_clean TEXT NOT NULL,
                target_path TEXT,
                alias TEXT,
                heading TEXT,
                is_wikilink INTEGER NOT NULL DEFAULT 1,
                line_no INTEGER NOT NULL DEFAULT 1,
                FOREIGN KEY(source_path) REFERENCES documents(path) ON DELETE CASCADE
            )",
            [],
        )
        .map_err(|e| e.to_string())?;
        for sql in [
            "CREATE INDEX IF NOT EXISTS idx_links_source ON links(source_path);",
            "CREATE INDEX IF NOT EXISTS idx_links_target_path ON links(target_path);",
            "CREATE INDEX IF NOT EXISTS idx_links_target_clean ON links(target_clean);",
        ] {
            conn.execute(sql, []).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>, String> {
        self.conn.lock().map_err(|_| "Lock poisoned".to_string())
    }

    /// `resolve_target`.
    pub fn resolve_target(
        &self,
        target_clean: &str,
        source_path: &str,
        known_files: &HashSet<String>,
        basenames: &HashMap<String, Vec<String>>,
    ) -> Option<String> {
        if target_clean.is_empty() || target_clean.starts_with('#') {
            return Some(source_path.to_string());
        }
        let cand1 = py_normpath(&py_join(&py_dirname(source_path), target_clean));
        if known_files.contains(&cand1) {
            return Some(cand1);
        }
        let cand1_md = format!("{cand1}.md");
        if known_files.contains(&cand1_md) {
            return Some(cand1_md);
        }
        let clean_norm = py_normpath(target_clean).to_lowercase();
        let base = py_basename(&clean_norm);
        let base_no_ext = if MD_EXTENSIONS.iter().any(|e| base.ends_with(e)) {
            py_splitext(&base).0
        } else {
            base.clone()
        };
        if let Some(list) = basenames.get(&base) {
            return list.first().cloned();
        }
        let probe = format!("{base_no_ext}.md");
        if let Some(list) = basenames.get(&probe) {
            return list.first().cloned();
        }
        None
    }

    /// `index_directory`.
    pub fn index_directory(&self, root_dir: &str, force: bool) -> Result<IndexStats, String> {
        let root = py_abspath(root_dir);
        if !Path::new(&root).is_dir() {
            return Ok(IndexStats::default());
        }
        // 1. discovery, in os.walk order
        let found = walk_markdown_pair(&root);
        // 2. filename lookup tables
        let known_files: HashSet<String> = found.order.iter().cloned().collect();
        let mut basenames: HashMap<String, Vec<String>> = HashMap::new();
        for p in &found.order {
            basenames.entry(py_basename(p).to_lowercase()).or_default().push(p.clone());
        }
        let pattern = root_pattern(&root);
        let conn = self.lock()?;
        // 3. rows already on file
        let db_docs: Vec<(String, f64)> = {
            let mut stmt = conn
                .prepare("SELECT path, mtime FROM documents WHERE path LIKE ?1 ESCAPE '!'")
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map(params![pattern], |row| Ok((row.get(0)?, row.get(1)?)))
                .map_err(|e| e.to_string())?;
            rows.filter_map(|r| r.ok()).collect()
        };
        let db_map: HashMap<String, f64> = db_docs.iter().cloned().collect();
        // 4. prune vanished documents
        let deleted_paths: Vec<String> =
            db_docs.iter().filter(|(p, _)| !found.mtimes.contains_key(p)).map(|(p, _)| p.clone()).collect();
        for p in &deleted_paths {
            conn.execute("DELETE FROM documents WHERE path = ?1", params![p]).map_err(|e| e.to_string())?;
            conn.execute("DELETE FROM links WHERE source_path = ?1", params![p]).map_err(|e| e.to_string())?;
        }
        // 5. (re)parse new and stale files
        let mut to_index: Vec<String> = Vec::new();
        for p in &found.order {
            let stale = match db_map.get(p) {
                None => true,
                Some(old) => (old - found.mtimes[p]).abs() > 0.001,
            };
            if force || stale {
                to_index.push(p.clone());
            }
        }
        let now = now_seconds();
        let mut indexed_count = 0i64;
        for p in &to_index {
            let content = match read_text_pythonic(Path::new(p)) {
                Some(c) => c,
                None => continue,
            };
            let size = content.chars().count() as i64;
            let title = extract_title(&content, &py_basename(p));
            let mtime = found.mtimes.get(p).copied().unwrap_or(0.0);
            let extracted = extract_links(&content);
            conn.execute(
                "INSERT INTO documents (path, title, mtime, size, scanned_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(path) DO UPDATE SET
                     title = excluded.title,
                     mtime = excluded.mtime,
                     size = excluded.size,
                     scanned_at = excluded.scanned_at",
                params![p, title, mtime, size, now],
            )
            .map_err(|e| e.to_string())?;
            conn.execute("DELETE FROM links WHERE source_path = ?1", params![p]).map_err(|e| e.to_string())?;
            for item in &extracted {
                let resolved = self.resolve_target(&item.target_clean, p, &known_files, &basenames);
                conn.execute(
                    "INSERT INTO links (
                        source_path, target_raw, target_clean, target_path,
                        alias, heading, is_wikilink, line_no
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        p,
                        item.target_raw,
                        item.target_clean,
                        resolved,
                        item.alias,
                        item.heading,
                        item.is_wikilink as i64,
                        item.line_no
                    ],
                )
                .map_err(|e| e.to_string())?;
            }
            indexed_count += 1;
        }
        drop(conn);
        // 6. re-point dangling links now that more files are known
        if indexed_count > 0 || !deleted_paths.is_empty() {
            let conn = self.lock()?;
            let rows: Vec<(i64, String, String)> = {
                let mut stmt = conn
                    .prepare(
                        "SELECT link_id, source_path, target_clean FROM links
                         WHERE source_path LIKE ?1 ESCAPE '!'",
                    )
                    .map_err(|e| e.to_string())?;
                let rows = stmt
                    .query_map(params![pattern], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
                    .map_err(|e| e.to_string())?;
                rows.filter_map(|r| r.ok()).collect()
            };
            for (link_id, source_path, target_clean) in rows {
                let res = self.resolve_target(&target_clean, &source_path, &known_files, &basenames);
                conn.execute("UPDATE links SET target_path = ?1 WHERE link_id = ?2", params![res, link_id])
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(IndexStats {
            scanned_count: found.mtimes.len() as i64,
            indexed_count,
            deleted_count: deleted_paths.len() as i64,
        })
    }

    /// `get_forward_links` — rows keyed and typed as the reference `dict(row)`.
    pub fn get_forward_links(&self, file_path: &str) -> Result<Vec<Value>, String> {
        let norm = py_normpath(&py_abspath(file_path));
        let conn = self.lock()?;
        let mut stmt = conn
            .prepare(
                "SELECT link_id, target_raw, target_clean, target_path, alias, heading, is_wikilink, line_no
                 FROM links WHERE source_path = ?1
                 ORDER BY line_no ASC, link_id ASC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![norm], |row| {
                Ok(json_obj([
                    ("link_id", Value::from(row.get::<_, i64>(0)?)),
                    ("target_raw", row_value(row, 1)?),
                    ("target_clean", row_value(row, 2)?),
                    ("target_path", row_value(row, 3)?),
                    ("alias", row_value(row, 4)?),
                    ("heading", row_value(row, 5)?),
                    ("is_wikilink", Value::from(row.get::<_, i64>(6)?)),
                    ("line_no", Value::from(row.get::<_, i64>(7)?)),
                ]))
            })
            .map_err(|e| e.to_string())?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// `get_backlinks`.
    pub fn get_backlinks(&self, file_path: &str) -> Result<Vec<Value>, String> {
        let norm = py_normpath(&py_abspath(file_path));
        let conn = self.lock()?;
        let mut stmt = conn
            .prepare(
                "SELECT l.link_id, l.source_path, d.title as source_title, l.target_raw,
                        l.target_clean, l.alias, l.heading, l.is_wikilink, l.line_no
                 FROM links l
                 LEFT JOIN documents d ON l.source_path = d.path
                 WHERE l.target_path = ?1
                 ORDER BY d.title ASC, l.line_no ASC, l.link_id ASC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![norm], |row| {
                Ok(json_obj([
                    ("link_id", Value::from(row.get::<_, i64>(0)?)),
                    ("source_path", row_value(row, 1)?),
                    ("source_title", row_value(row, 2)?),
                    ("target_raw", row_value(row, 3)?),
                    ("target_clean", row_value(row, 4)?),
                    ("alias", row_value(row, 5)?),
                    ("heading", row_value(row, 6)?),
                    ("is_wikilink", Value::from(row.get::<_, i64>(7)?)),
                    ("line_no", Value::from(row.get::<_, i64>(8)?)),
                ]))
            })
            .map_err(|e| e.to_string())?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// `get_deadlinks`. The reference sorts only by `(source_path, line_no)`
    /// after an index-ordered scan, so ties fall back to `link_id`; pinning the
    /// tiebreaker here keeps the row order plan-independent.
    pub fn get_deadlinks(&self, root_dir: Option<&str>) -> Result<Vec<Value>, String> {
        let conn = self.lock()?;
        let base = "SELECT l.link_id, l.source_path, d.title as source_title, l.target_raw,
                           l.target_clean, l.alias, l.line_no
                    FROM links l
                    LEFT JOIN documents d ON l.source_path = d.path
                    WHERE l.target_path IS NULL";
        let sql = match root_dir {
            Some(_) => format!(
                "{base} AND l.source_path LIKE ?1 ESCAPE '!'
                 ORDER BY l.source_path ASC, l.line_no ASC, l.link_id ASC"
            ),
            None => format!("{base} ORDER BY l.source_path ASC, l.line_no ASC, l.link_id ASC"),
        };
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let mapped = |row: &Row| -> rusqlite::Result<Value> {
            Ok(json_obj([
                ("link_id", Value::from(row.get::<_, i64>(0)?)),
                ("source_path", row_value(row, 1)?),
                ("source_title", row_value(row, 2)?),
                ("target_raw", row_value(row, 3)?),
                ("target_clean", row_value(row, 4)?),
                ("alias", row_value(row, 5)?),
                ("line_no", Value::from(row.get::<_, i64>(6)?)),
            ]))
        };
        let rows = match root_dir {
            Some(dir) => {
                let pattern = root_pattern(dir);
                stmt.query_map(params![pattern], mapped)
            }
            None => stmt.query_map([], mapped),
        }
        .map_err(|e| e.to_string())?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// `get_graph_data`, including the rowid-ordered `LIMIT` truncation.
    pub fn get_graph_data(&self, root_dir: Option<&str>, max_nodes: i64) -> Result<Value, String> {
        let conn = self.lock()?;
        let norm = root_dir.map(|d| py_normpath(&py_abspath(d)));
        let docs: Vec<(String, Value)> = {
            let (sql, pattern): (&str, Option<String>) = match &norm {
                Some(dir) => (
                    "SELECT path, title FROM documents WHERE path LIKE ?1 ESCAPE '!'
                     ORDER BY doc_id ASC LIMIT ?2",
                    Some(root_pattern(dir)),
                ),
                None => (
                    "SELECT path, title FROM documents ORDER BY doc_id ASC LIMIT ?1",
                    None,
                ),
            };
            let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
            let rows = match &pattern {
                Some(p) => stmt.query_map(params![p, max_nodes], row_path_title),
                None => stmt.query_map(params![max_nodes], row_path_title),
            }
            .map_err(|e| e.to_string())?;
            rows.filter_map(|r| r.ok()).collect()
        };
        let links: Vec<GraphLink> = {
            let (sql, pattern): (&str, Option<String>) = match &norm {
                Some(dir) => (
                    "SELECT source_path, target_clean, target_path, alias, is_wikilink
                     FROM links WHERE source_path LIKE ?1 ESCAPE '!' ORDER BY link_id ASC",
                    Some(root_pattern(dir)),
                ),
                None => (
                    "SELECT source_path, target_clean, target_path, alias, is_wikilink
                     FROM links ORDER BY link_id ASC",
                    None,
                ),
            };
            let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
            let rows = match &pattern {
                Some(p) => stmt.query_map(params![p], graph_link_row),
                None => stmt.query_map([], graph_link_row),
            }
            .map_err(|e| e.to_string())?;
            rows.filter_map(|r| r.ok()).collect()
        };

        // `nodes_map` is an insertion-ordered dict in the reference.
        let mut nodes: Vec<Value> = Vec::new();
        let mut node_index: HashMap<String, usize> = HashMap::new();
        for (p, title) in &docs {
            let nid = node_id(Some(p), &py_basename(p), norm.as_deref());
            let label = match title {
                Value::String(s) if !s.is_empty() => s.clone(),
                _ => py_basename(p),
            };
            let entry = json_obj([
                ("id", Value::String(nid.clone())),
                ("path", Value::String(p.clone())),
                ("label", Value::String(label)),
                ("is_deadlink", Value::Bool(false)),
                ("link_count", Value::from(0i64)),
                ("backlink_count", Value::from(0i64)),
            ]);
            match node_index.get(&nid) {
                Some(&i) => nodes[i] = entry,
                None => {
                    node_index.insert(nid, nodes.len());
                    nodes.push(entry);
                }
            }
        }
        let mut edges: Vec<Value> = Vec::new();
        for l in &links {
            let src_id = node_id(Some(&l.source_path), &py_basename(&l.source_path), norm.as_deref());
            let src_i = match node_index.get(&src_id) {
                Some(i) => *i,
                None => continue,
            };
            let tgt_id = match &l.target_path {
                Some(tp) => node_id(Some(tp), &py_basename(tp), norm.as_deref()),
                None => l.target_clean.clone(),
            };
            let tgt_i = match node_index.get(&tgt_id) {
                Some(i) => *i,
                None => {
                    let entry = match &l.target_path {
                        Some(tp) => json_obj([
                            ("id", Value::String(tgt_id.clone())),
                            ("path", Value::String(tp.clone())),
                            ("label", Value::String(py_basename(tp))),
                            ("is_deadlink", Value::Bool(false)),
                            ("link_count", Value::from(0i64)),
                            ("backlink_count", Value::from(0i64)),
                        ]),
                        None => json_obj([
                            ("id", Value::String(tgt_id.clone())),
                            ("path", Value::Null),
                            ("label", Value::String(tgt_id.clone())),
                            ("is_deadlink", Value::Bool(true)),
                            ("link_count", Value::from(0i64)),
                            ("backlink_count", Value::from(0i64)),
                        ]),
                    };
                    node_index.insert(tgt_id.clone(), nodes.len());
                    nodes.push(entry);
                    nodes.len() - 1
                }
            };
            bump(&mut nodes[src_i], "link_count");
            bump(&mut nodes[tgt_i], "backlink_count");
            edges.push(json_obj([
                ("source", Value::String(src_id)),
                ("target", Value::String(tgt_id)),
                ("label", Value::String(l.alias.clone().unwrap_or_default())),
                ("is_wikilink", Value::Bool(l.is_wikilink != 0)),
            ]));
        }
        let mut total_dead = 0i64;
        for n in nodes.iter_mut() {
            let lc = n.get("link_count").and_then(Value::as_i64).unwrap_or(0);
            let bc = n.get("backlink_count").and_then(Value::as_i64).unwrap_or(0);
            if n.get("is_deadlink").and_then(Value::as_bool) == Some(true) {
                total_dead += 1;
            }
            if let Some(obj) = n.as_object_mut() {
                obj.insert("degree".to_string(), Value::from(lc + bc));
            }
        }
        let total_edges = edges.len() as i64;
        let total_nodes = nodes.len() as i64;
        Ok(json_obj([
            ("nodes", Value::Array(nodes)),
            ("edges", Value::Array(edges)),
            (
                "stats",
                json_obj([
                    ("total_nodes", Value::from(total_nodes)),
                    ("total_edges", Value::from(total_edges)),
                    ("deadlinks_count", Value::from(total_dead)),
                ]),
            ),
        ]))
    }
}

fn bump(node: &mut Value, key: &str) {
    if let Some(obj) = node.as_object_mut() {
        let cur = obj.get(key).and_then(Value::as_i64).unwrap_or(0);
        obj.insert(key.to_string(), Value::from(cur + 1));
    }
}

fn row_value(row: &Row<'_>, idx: usize) -> rusqlite::Result<Value> {
    Ok(match row.get::<_, Option<String>>(idx)? {
        Some(s) => Value::String(s),
        None => Value::Null,
    })
}

fn row_path_title(row: &Row<'_>) -> rusqlite::Result<(String, Value)> {
    Ok((row.get::<_, String>(0)?, row_value(row, 1)?))
}

fn graph_link_row(row: &Row<'_>) -> rusqlite::Result<GraphLink> {
    Ok(GraphLink {
        source_path: row.get(0)?,
        target_clean: row.get(1)?,
        target_path: row.get(2)?,
        alias: row.get(3)?,
        is_wikilink: row.get::<_, i64>(4)?,
    })
}

/// `_get_node_id`.
fn node_id(full_path: Option<&String>, fallback_name: &str, root: Option<&str>) -> String {
    let p = match full_path {
        Some(p) => p,
        None => return fallback_name.to_string(),
    };
    // `if root_dir and full_path.startswith(norm_root): return relpath(...)`
    if let Some(r) = root {
        if p.starts_with(r) {
            return py_relpath_under(p, r);
        }
    }
    py_basename(p)
}

/// `os.walk` limited to what the reference keeps, preserving enumeration order
/// (the first hit of a duplicated basename wins during resolution).
fn walk_markdown(root: &str) -> (Vec<String>, HashMap<String, f64>) {
    let mut order: Vec<String> = Vec::new();
    let mut mtimes: HashMap<String, f64> = HashMap::new();
    walk_into(root, &mut order, &mut mtimes);
    (order, mtimes)
}

/// CPython's `sys.getrecursionlimit()` default, which is also the ceiling of
/// the reference walk: `os.walk` consumes one Python frame per directory level,
/// so it cannot finish a tree deeper than this. Measured on this box
/// (python 3.11.15): a 220-level chain walks cleanly (221 yields), while
/// *building* a 1500-level chain already dies in `os.makedirs` with
/// `RecursionError`. Stopping here therefore rejects no tree the reference ever
/// indexed, and because the walk below is iterative it costs no stack either:
/// this is only the last line of defense behind the ancestor cycle guard.
const MAX_WALK_DEPTH: usize = 1_000;

/// One pending directory on [`walk_into`]'s explicit worklist.
struct WalkFrame {
    dir: PathBuf,
    depth: usize,
    /// Canonical identities of every directory above this one, innermost first.
    ancestors: Ancestors,
}

/// Persistent ancestor chain for the cycle test. A linked list rather than a
/// `Vec` per frame so a wide tree does not copy its ancestry on every push.
#[derive(Clone, Default)]
struct Ancestors {
    head: Option<std::rc::Rc<AncestorNode>>,
}

struct AncestorNode {
    key: String,
    parent: Option<std::rc::Rc<AncestorNode>>,
}

impl Ancestors {
    fn extended(&self, key: String) -> Self {
        Ancestors {
            head: Some(std::rc::Rc::new(AncestorNode { key, parent: self.head.clone() })),
        }
    }

    fn holds(&self, key: &str) -> bool {
        let mut node = self.head.as_deref();
        while let Some(current) = node {
            if current.key == key {
                return true;
            }
            node = current.parent.as_deref();
        }
        false
    }
}

/// Canonical identity of a directory, used **only** as a cycle key and never as
/// a path to open: `fs::canonicalize` resolves junctions and symlinks, which is
/// exactly what makes "this directory is one of my own ancestors" decidable.
/// When it fails (permissions, a directory deleted mid-walk) the raw path is
/// used instead and `MAX_WALK_DEPTH` still bounds the walk.
fn cycle_key(path: &Path) -> String {
    fs::canonicalize(path)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| path.to_string_lossy().to_string())
}

#[cfg(not(windows))]
fn dir_entry_file_type(entry: &fs::DirEntry) -> Option<fs::FileType> {
    entry
        .file_type()
        .ok()
        .or_else(|| fs::symlink_metadata(entry.path()).ok().map(|m| m.file_type()))
}

/// `os.walk`'s own `followlinks=False` test: is this directory reached *through
/// a link the reference refuses to enter*?
///
/// On POSIX the answer is `file_type().is_symlink()`, which is exactly CPython's
/// behaviour there. On Windows this toolchain's `std` cannot answer it at all:
/// measured here with a probe, a junction reports
/// `entry{symlink=true, dir_link=true, file_link=false, is_dir=false}`, i.e.
/// `is_symlink`/`is_symlink_dir` are true for **both** kinds of reparse point,
/// and neither `FileTypeExt::is_junction_dir` nor `MetadataExt::reparse_tag`
/// exists in this `std`, so `IO_REPARSE_TAG_SYMLINK` and
/// `IO_REPARSE_TAG_MOUNT_POINT` are indistinguishable without a Win32 crate.
///
/// That matters because the reference treats the two kinds oppositely: measured
/// on this box (python 3.11.15) a junction reports `os.path.islink() == False`
/// and `DirEntry.is_symlink() == False`, so `os.walk(followlinks=False)` **does**
/// descend into it and indexes `loop\a.md`. Skipping every reparse point would
/// therefore stop indexing files Python indexes — and because
/// [`LinkIndexer::index_directory`] deletes the `documents` rows of paths that
/// vanish from the walk, it would delete those documents out of a live index.
/// So Windows keeps descending (what this walker did before, and the only
/// behaviour matching the measured reference) and relies on the ancestor cycle
/// guard plus [`MAX_WALK_DEPTH`] for termination.
#[cfg(windows)]
fn os_walk_skips_dir(_entry: &fs::DirEntry) -> bool {
    false
}

#[cfg(not(windows))]
fn os_walk_skips_dir(entry: &fs::DirEntry) -> bool {
    matches!(dir_entry_file_type(entry), Some(file_type) if file_type.is_symlink())
}

/// `os.walk` limited to what the reference keeps. Iterative rather than
/// recursive on purpose: a worklist cannot overflow the 1-2 MiB stacks this
/// crate runs on, so no depth has to be refused that Python would have walked.
fn walk_into(dir: &str, order: &mut Vec<String>, mtimes: &mut HashMap<String, f64>) {
    let mut stack: Vec<WalkFrame> = vec![WalkFrame {
        dir: PathBuf::from(dir),
        depth: 0,
        ancestors: Ancestors::default(),
    }];
    while let Some(frame) = stack.pop() {
        let read = match fs::read_dir(&frame.dir) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let ancestors = frame.ancestors.extended(cycle_key(&frame.dir));
        let mut subdirs: Vec<PathBuf> = Vec::new();
        for entry in read.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if path.is_dir() {
                // `dirs[:] = [d for d in dirs if not d.startswith(('.', '_')) and d != 'node_modules']`
                if !name.starts_with('.') && !name.starts_with('_') && name != "node_modules" {
                    // `followlinks=False`: kept in `dirs` (so it is never
                    // indexed as a file, exactly like a link named `x.md`
                    // measured in `dirnames`), but not entered.
                    if !os_walk_skips_dir(&entry) {
                        subdirs.push(path);
                    }
                }
                continue;
            }
            if !MD_EXTENSIONS.iter().any(|e| name.to_lowercase().ends_with(e)) {
                continue;
            }
            let norm = py_normpath(&path.to_string_lossy());
            match mtime_of(&path) {
                Some(m) => {
                    if mtimes.insert(norm.clone(), m).is_none() {
                        order.push(norm);
                    }
                }
                None => {}
            }
        }
        if frame.depth >= MAX_WALK_DEPTH {
            continue;
        }
        // Top-down order: this level's files were emitted above, then each kept
        // directory's whole subtree in enumeration order — a reversed LIFO push
        // reproduces that without recursion.
        let depth = frame.depth + 1;
        for subdir in subdirs.into_iter().rev() {
            if ancestors.holds(&cycle_key(&subdir)) {
                // A link (or junction) back into one of its own ancestors:
                // `os.walk` either dies at `RecursionError` or silently aborts
                // the whole walk on the path-length wall here, so pruning costs
                // nothing the reference kept.
                continue;
            }
            stack.push(WalkFrame { dir: subdir, depth, ancestors: ancestors.clone() });
        }
    }
}

/// `Discovered` pair helper alias so `index_directory` reads like the reference.
struct Discovered {
    order: Vec<String>,
    mtimes: HashMap<String, f64>,
}

fn walk_markdown_pair(root: &str) -> Discovered {
    let (order, mtimes) = walk_markdown(root);
    Discovered { order, mtimes }
}

// --------------------------------------------------------------------------
// module singleton (`get_indexer`)
// --------------------------------------------------------------------------

static INDEXER: OnceLock<Mutex<Option<Arc<LinkIndexer>>>> = OnceLock::new();

fn slot() -> &'static Mutex<Option<Arc<LinkIndexer>>> {
    INDEXER.get_or_init(|| Mutex::new(None))
}

/// `get_indexer(db_path=None)`: an explicit path rebuilds the instance, exactly
/// like the reference.
pub fn get_indexer(db_path: Option<&Path>) -> Result<Arc<LinkIndexer>, String> {
    let mut guard = slot().lock().map_err(|_| "Lock poisoned".to_string())?;
    if db_path.is_some() || guard.is_none() {
        let path = match db_path {
            Some(p) => PathBuf::from(p),
            None => default_db_path(),
        };
        *guard = Some(Arc::new(LinkIndexer::new(&path)?));
    }
    Ok(guard.clone().expect("indexer installed"))
}

/// `DATA_DIR/index/link_index.db`.
pub fn default_db_path() -> PathBuf {
    let dir = crate::paths::data_dir().join("index");
    let _ = fs::create_dir_all(&dir);
    dir.join("link_index.db")
}

/// Drop the cached singleton (used by tests that repoint the database).
pub fn close_indexer() {
    if let Ok(mut guard) = slot().lock() {
        *guard = None;
    }
}

// ===========================================================================
// W-E9 whitespace-parity tests for `link_indexer.rs`.  Authority:
// src/readmd_modules/link_indexer.py — `_extract_title` (`line.strip()`,
// `line_s[2:].strip()`), `extract_links` (`if not line.strip()`,
// `match.group(1).strip()`, `target_part.strip()`, `heading.strip()`),
// `_RE_MD_LINK`'s unicode-mode `[^)\s]` / `\s+`, and `str.splitlines()`.
// Expected values are CPython's own (scratch/rust_parity/we9_expect.py,
// we9_probe.py).
// ===========================================================================
#[cfg(test)]
mod we9_tests {
    use super::*;
    use std::panic::catch_unwind;

    const WS: [char; 9] = ['\u{1c}', '\u{1d}', '\u{1e}', '\u{1f}', '\u{a0}',
                           '\u{3000}', '\u{2028}', '\t', '\r'];

    fn tuple(l: &ExtractedLink) -> (String, String, Option<String>, Option<String>, bool, i64) {
        (l.target_raw.clone(), l.target_clean.clone(), l.alias.clone(),
         l.heading.clone(), l.is_wikilink, l.line_no)
    }

    #[test]
    fn we9_py_isspace_and_strip_are_cpython_exact() {
        for c in WS.iter() {
            assert!(py_isspace(*c), "U+{:04X}", *c as u32);
        }
        for c in ['\u{1a}', '\u{1b}', 'x', '#'] {
            assert!(!py_isspace(c));
        }
        assert_eq!(py_strip("\u{1c}foo\u{1d}"), "foo");
        assert_eq!(py_strip("\u{a0}\u{3000}"), "");
        assert_eq!(py_strip(""), "");
        // `str.splitlines()` boundaries: 1c/1d/1e break, 1f does NOT (measured)
        assert_eq!(py_splitlines("\u{1c}# T\u{1d}"), vec!["", "# T"]);
        assert_eq!(py_splitlines("a\u{1e}b"), vec!["a", "b"]);
        assert_eq!(py_splitlines("a\u{1f}b"), vec!["a\u{1f}b"]);
        assert_eq!(py_splitlines("a\r\nb"), vec!["a", "b"]);
        assert_eq!(py_splitlines("a\u{85}b\u{2028}c"), vec!["a", "b", "c"]);
        assert!(py_splitlines("").is_empty());
        assert_eq!(py_splitlines("\u{1c}"), vec![""]);
    }

    #[test]
    fn we9_extract_title_matches_python() {
        // measured: `line.strip()` then `line_s[2:].strip()`
        assert_eq!(extract_title("\u{1c}# Title\u{1d}\n", "fb"), "Title");
        assert_eq!(extract_title("# \u{a0}NBSP\u{a0}\n", "fb"), "NBSP");
        assert_eq!(extract_title("   # indented  \u{1f}\n", "fb"), "indented");
        assert_eq!(extract_title("# T\r\nsecond\n", "fb"), "T");
        // `\x1e#` strips to `#`, which is not `# ` -> fallback
        assert_eq!(extract_title("\u{1e}#\nnear\n", "fb"), "fb");
        assert_eq!(extract_title("", "fb"), "fb");
        assert_eq!(extract_title("\u{1c}\u{1d}\u{1e}\u{1f}", "fb"), "fb");
    }

    #[test]
    fn we9_extract_links_wikilinks() {
        // measured: raw_inner.strip() eats the separators at the edges only
        let got: Vec<_> = extract_links("[[ \u{1c}a\u{1d}b | b \u{1e}]]\n").iter().map(tuple).collect();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].0, "a\u{1d}b | b");
        assert_eq!(got[0].1, "a\u{1d}b");
        assert_eq!(got[0].2.as_deref(), Some("b"));
        assert_eq!(got[0].5, 1);

        let got2: Vec<_> = extract_links("[[\u{1c}]]\n[[|]]\n[[ \u{1f}|\u{1e} ]]\n")
            .iter().map(tuple).collect();
        // measured: `[[\x1c]]` strips to an empty raw_inner and is skipped, while
        // `[|]` and `[ \x1f|\x1e ]` keep an empty clean target and no alias/heading
        assert_eq!(got2.len(), 2);
        assert_eq!(got2[0], ("|".into(), "".into(), None, None, true, 2));
        assert_eq!(got2[1], ("|".into(), "".into(), None, None, true, 3));

        // measured: `[[\x1c#Head\x1d]]` -> raw '#Head', clean '#Head' AND heading
        // 'Head': Py 133-136 re-prefixes a heading-only target without clearing it
        let got3: Vec<_> = extract_links("[[\u{1c}#Head\u{1d}]]\n").iter().map(tuple).collect();
        assert_eq!(got3.len(), 1);
        assert_eq!(got3[0].0, "#Head");
        assert_eq!(got3[0].1, "#Head");
        assert_eq!(got3[0].3.as_deref(), Some("Head"));

        // a heading part that is only separators is dropped (Py: `heading.strip() or None`)
        let got4: Vec<_> = extract_links("[[t.md#\u{1c}\u{1d}]]\n").iter().map(tuple).collect();
        assert_eq!(got4[0].1, "t.md");
        assert_eq!(got4[0].3, None);
        let got5: Vec<_> = extract_links("[[t.md#\u{1e}Sec\u{1f}]]\n").iter().map(tuple).collect();
        assert_eq!(got5[0].1, "t.md");
        assert_eq!(got5[0].3.as_deref(), Some("Sec"));
    }

    #[test]
    fn we9_extract_links_md_links() {
        // `[^)\s]` is CPython's class, so a separator cannot even START the url
        assert!(extract_links("[a](\u{1c}t.md)\n").is_empty());
        assert!(extract_links("[a](t\u{1c}b.md)\n").is_empty());
        let got: Vec<_> = extract_links("[\u{1c}note](target.md)\n").iter().map(tuple).collect();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].0, "target.md");
        assert_eq!(got[0].2.as_deref(), Some("note"));
        assert!(!got[0].4);
        // an optional title separated by separators is not a url continuation
        let got2: Vec<_> = extract_links("[x](rel/path.md \u{1d}\"Title\")\n").iter().map(tuple).collect();
        assert_eq!(got2[0].0, "rel/path.md");
        // `\x1c` alone as the alias is stripped away -> alias None (Py: `or None`)
        let got3: Vec<_> = extract_links("[\u{1c}\u{1d}](z.md)\n").iter().map(tuple).collect();
        assert_eq!(got3[0].2, None);
        // measured: `[^)\s]+` stops at the separator and the optional
        // `(?:\s+"[^"]*")?\)` cannot close, so the whole link fails to match
        let got4: Vec<_> = extract_links("[l](a%20b.md\u{1c}#H\u{1d})\n").iter().map(tuple).collect();
        assert!(got4.is_empty());
        // the same link without separators: percent-decoded, heading split
        let got5: Vec<_> = extract_links("[l](a%20b.md#H)\n").iter().map(tuple).collect();
        assert_eq!(got5[0].0, "a%20b.md#H");
        assert_eq!(got5[0].1, "a b.md");
        assert_eq!(got5[0].3.as_deref(), Some("H"));
    }

    #[test]
    fn we9_extract_links_masking_and_blank_lines() {
        // `if not line.strip(): continue` — a separator-only line is blank to
        // CPython (behaviourally inert, but the class has to be Python's)
        assert!(extract_links("\u{1c}\u{1d}\u{1e}\u{1f}\n").is_empty());
        assert!(extract_links("\u{a0}\n\u{3000}\n\u{2028}\n\t\n\r\n").is_empty());
        assert!(extract_links("").is_empty());
        // fenced + inline code are masked, keeping the line numbering
        let got: Vec<_> = extract_links("```\n[[masked]]\n```\n[[live]]\n`[[inline]]`\n")
            .iter().map(tuple).collect();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].1, "live");
        assert_eq!(got[0].5, 4);
        // measured: mask keeps every character outside a code span (separators
        // included) and blanks a code span to spaces of the same length, keeping
        // its newlines so line_no stays exact
        assert_eq!(mask_code_blocks("a\u{1c}b\n"), "a\u{1c}b\n");
        assert_eq!(mask_code_blocks("\u{3000}```x\ny\n```\u{2028}z"),
                   "\u{3000}    \n \n   \u{2028}z");
        assert_eq!(mask_code_blocks("```py\ncode\n```\n"), "     \n    \n   \n");
    }

    #[test]
    fn we9_matrix_never_panics() {
        let mut runs = 0usize;
        for w in WS.iter().chain(['\u{1a}', 'x'].iter()) {
            for shape in 0..6 {
                let src = match shape {
                    0 => format!("[[{0}a{0}]]", w),
                    1 => format!("[{0}t]({0}u.md)", w),
                    2 => format!("[[t.md#{0}H{0}]]", w),
                    3 => format!("```\n[[x]]\n{0}```\n[[y]]", w),
                    4 => format!("[a](<%20{0}>)\n[[#{0}]]", w),
                    _ => format!("{0}", w),
                };
                runs += 1;
                let s = src.clone();
                assert!(catch_unwind(move || extract_links(&s)).is_ok(), "extract_links {:?}", src);
                let s = src.clone();
                assert!(catch_unwind(move || extract_title(&s, "fb").len()).is_ok(), "extract_title {:?}", src);
                let s = src.clone();
                assert!(catch_unwind(move || mask_code_blocks(&s)).is_ok(), "mask {:?}", src);
                let s = src.clone();
                assert!(catch_unwind(move || py_splitlines(&s).len()).is_ok(), "splitlines {:?}", src);
                let s = src.clone();
                assert!(catch_unwind(move || py_normpath(&s)).is_ok(), "normpath {:?}", src);
            }
        }
        assert_eq!(runs, 66);
    }
}

#[cfg(test)]
mod walker_link_tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "readmd-walk-{tag}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn touch(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "# t\n").unwrap();
    }

    /// `cmd /c mklink /J` — measured on this box, a junction needs no elevated
    /// privilege while a directory symlink does (`WinError 1314`).
    fn make_junction(link: &Path, target: &Path) -> bool {
        std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false)
    }

    fn entry_named(dir: &Path, name: &str) -> fs::DirEntry {
        fs::read_dir(dir)
            .unwrap()
            .flatten()
            .find(|e| e.file_name() == std::ffi::OsString::from(name))
            .expect("entry present")
    }

    /// Remove the links first (a link is removed, never its target) then the tree.
    fn cleanup(root: &Path, links: &[&Path]) {
        for link in links {
            let _ = fs::remove_dir(link);
            let _ = fs::remove_file(link);
        }
        let _ = fs::remove_dir_all(root);
    }

    /// Panic-safe teardown for a fixture that owns a junction. Calling
    /// [`cleanup`] as the last statement of a test leaks the junction *and* its
    /// temp tree whenever an assertion above it fails, which is precisely what a
    /// broken walker does; holding a guard tears down on unwind too.
    struct FixtureGuard {
        root: PathBuf,
        links: Vec<PathBuf>,
    }

    impl FixtureGuard {
        fn new(root: &Path) -> Self {
            FixtureGuard {
                root: root.to_path_buf(),
                links: Vec::new(),
            }
        }
    }

    impl Drop for FixtureGuard {
        fn drop(&mut self) {
            let links: Vec<&Path> = self.links.iter().map(PathBuf::as_path).collect();
            cleanup(&self.root, &links);
        }
    }

    /// Sorted, root-relative view of a discovery result — the comparison Python's
    /// `os.walk` file list is judged on.
    fn relative(root: &Path, order: &[String]) -> Vec<String> {
        let mut rel: Vec<String> = order
            .iter()
            .map(|p| match Path::new(p).strip_prefix(root) {
                Ok(r) => r.to_string_lossy().to_string(),
                Err(_) => p.clone(),
            })
            .collect();
        rel.sort();
        rel
    }

    #[test]
    fn we9_walk_junction_back_to_ancestor_terminates_with_the_real_tree() {
        let root = scratch("cycle");
        let mut fixture = FixtureGuard::new(&root);
        let a = root.join("a");
        touch(&root.join("root.md"));
        touch(&a.join("a.md"));
        touch(&a.join("b").join("b.md"));
        let link = a.join("loop");
        if !make_junction(&link, &a) {
            eprintln!("skipping: could not create a junction here");
            return;
        }
        fixture.links.push(link.clone());
        // Measured on this box (python 3.11.15, win32) for a `mklink /J`
        // junction: `os.path.islink() == False`, `DirEntry.is_symlink() == False`
        // and `DirEntry.is_dir(follow_symlinks=False) == True`, so
        // `os.walk(followlinks=False)` descends into it just as it does with
        // `followlinks=True`. Rust's `std` inverts exactly one of those labels:
        // the same junction reports `file_type().is_symlink() == true`. That one
        // flag is the whole delta, and it is why `os_walk_skips_dir` must not
        // consult it on Windows.
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            let meta = fs::symlink_metadata(&link).expect("junction still present");
            // 0x410 == FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT.
            // The reparse *tag* that would tell a junction (0xa0000003) from a
            // directory symlink (0xa000000c) is not exposed by this `std`
            // (`MetadataExt::reparse_tag` and `FileTypeExt::is_junction_dir`
            // both fail to compile here), hence the stub below.
            assert_eq!(
                meta.file_attributes() & 0x400,
                0x400,
                "the fixture must really be a reparse point, not a plain directory"
            );
            assert!(
                meta.file_type().is_symlink(),
                "std calls a junction a symlink where CPython calls it a directory"
            );
        }
        // `Path::is_dir()` follows the reparse point, which is what let the old
        // recursion walk straight into its own ancestor.
        assert!(link.is_dir());
        assert!(
            !os_walk_skips_dir(&entry_named(&a, "loop")),
            "CPython descends into a junction, so the link test must not skip it"
        );

        let (order, mtimes) = walk_markdown(&root.to_string_lossy());
        assert_eq!(
            relative(&root, &order),
            vec![
                format!("a{SEP}a.md"),
                format!("a{SEP}b{SEP}b.md"),
                "root.md".to_string(),
            ],
            "the loop is pruned at its own edge, not walked until the stack dies"
        );
        assert_eq!(mtimes.len(), 3);
    }

    #[test]
    fn we9_walk_junction_to_sibling_keeps_both_views_like_python() {
        let root = scratch("sibling");
        let mut fixture = FixtureGuard::new(&root);
        let a = root.join("a");
        touch(&a.join("a.md"));
        let link = root.join("loop");
        if !make_junction(&link, &a) {
            eprintln!("skipping: could not create a junction here");
            return;
        }
        fixture.links.push(link.clone());
        // Measured on this box (python 3.11.15): `os.walk` lists `loop` in
        // `dirnames` and yields `loop\a.md` next to `a\a.md`, and the visit set
        // for `followlinks=False` is *identical* to the one for
        // `followlinks=True` — a junction is never classified as a link, so the
        // flag has nothing to gate. A fix that skipped every reparse point would
        // index fewer files than the reference, and because
        // `index_directory` deletes the `documents` rows of paths that vanish
        // from the walk, it would delete those documents out of a live index.
        let (order, mtimes) = walk_markdown(&root.to_string_lossy());
        assert_eq!(
            relative(&root, &order),
            vec![format!("a{SEP}a.md"), format!("loop{SEP}a.md")],
            "both views survive, exactly as the reference yields them"
        );
        assert_eq!(mtimes.len(), 2, "each view is one distinct indexed path");
    }

    #[test]
    fn we9_walk_directory_symlink_is_recorded_but_not_descended() {
        let root = scratch("symlink");
        let real = root.join("real");
        touch(&real.join("x.md"));
        let mirror = root.join("mirror");
        #[cfg(windows)]
        let made = std::os::windows::fs::symlink_dir(&real, &mirror).is_ok();
        #[cfg(unix)]
        let made = std::os::unix::fs::symlink(&real, &mirror).is_ok();
        #[cfg(not(any(windows, unix)))]
        let made = false;
        if !made {
            eprintln!("skipping: no privilege to create a directory symlink here");
            cleanup(&root, &[]);
            return;
        }
        assert!(mirror.is_dir(), "Path::is_dir follows the symlink");
        let (order, _) = walk_markdown(&root.to_string_lossy());
        let rel = relative(&root, &order);
        #[cfg(not(windows))]
        {
            assert!(
                os_walk_skips_dir(&entry_named(&root, "mirror")),
                "os.walk(followlinks=False) never enters a directory symlink"
            );
            assert_eq!(rel, vec![format!("real{SEP}x.md")]);
        }
        // Windows knowingly diverges here until `std` exposes the reparse tag:
        // a link cannot be told from a junction, and junctions are walked by the
        // reference (see `os_walk_skips_dir`), so this over-indexes rather than
        // deleting documents out of the live index.
        #[cfg(windows)]
        assert!(rel.contains(&format!("real{SEP}x.md")), "{rel:?}");
        cleanup(&root, &[&mirror]);
    }

    #[test]
    fn we9_walk_deep_legal_tree_is_not_refused() {
        let root = scratch("deep");
        // 300 single-character levels: past the 260-char MAX_PATH wall, which is
        // the precondition for a per-directory recursion to run far enough to matter.
        let mut dir = root.clone();
        for _ in 0..300 {
            dir = dir.join("x");
        }
        touch(&dir.join("leaf.md"));
        touch(&root.join("top.md"));
        assert_eq!(MAX_WALK_DEPTH, 1_000, "cap stays at CPython's recursionlimit");
        let (order, _) = walk_markdown(&root.to_string_lossy());
        let rel = relative(&root, &order);
        assert_eq!(rel.len(), 2, "{rel:?}");
        assert!(rel.iter().any(|p| p.ends_with("leaf.md")), "{rel:?}");
        let _ = fs::remove_dir_all(&root);
    }
}

// ===========================================================================
// Graph / backlink / deadlink / `max_nodes` parity tests.  Authority:
// src/readmd_modules/link_indexer.py — `get_graph_data` (400-503),
// `get_backlinks` (360-373), `get_deadlinks` (375-398), `get_forward_links`
// (348-358), `resolve_target` (186-220), `_root_pattern` (29-32).  Every
// expected value below is CPython 3.11.15's own output for the identical
// fixture, captured on this box.
// ===========================================================================
#[cfg(test)]
mod graph_tests {
    use super::*;
    use serde_json::json;

    fn scratch(tag: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "readmd-graph-{tag}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Tears the fixture down even when an assertion above it fails.
    struct Tree(PathBuf);

    impl Drop for Tree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// `LinkIndexer(db_path=...)` under `<root>/.idx`, which `os.walk` skips.
    fn indexer_at(root: &Path) -> LinkIndexer {
        LinkIndexer::new(root.join(".idx").join("link_index.db")).unwrap()
    }

    /// Writes one fixture file with LF endings, creating parent directories.
    fn put(root: &Path, rel: &str, text: &str) -> PathBuf {
        let mut path = root.to_path_buf();
        for part in rel.split('/') {
            path = path.join(part);
        }
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, text).unwrap();
        path
    }

    /// The stored (walk-normalised) form of a fixture path.
    fn stored(root: &Path, rel: &str) -> String {
        let mut path = root.to_path_buf();
        for part in rel.split('/') {
            path = path.join(part);
        }
        path.to_string_lossy().to_string()
    }

    /// `<root>\<rel>` -> `<rel>` with forward slashes, as `_get_node_id` emits.
    fn rel_id(root: &Path, full: &str) -> String {
        let prefix = format!("{}{}", root.to_string_lossy(), SEP);
        let tail = full
            .strip_prefix(&prefix)
            .unwrap_or_else(|| panic!("`{full}` is not under `{prefix}`"));
        tail.replace(SEP, "/")
    }

    fn sorted_keys(obj: &Value) -> Vec<String> {
        let mut k: Vec<String> = obj.as_object().unwrap().keys().cloned().collect();
        k.sort();
        k
    }

    fn ids(graph: &Value) -> Vec<String> {
        graph["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n["id"].as_str().unwrap().to_string())
            .collect()
    }

    fn node<'a>(graph: &'a Value, id: &str) -> &'a Value {
        graph["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"].as_str() == Some(id))
            .unwrap_or_else(|| panic!("no node `{id}`; ids={:?}", ids(graph)))
    }

    /// Asserts one node dict against the reference: `path` is root-relative, or
    /// `None` for the `None`/null a deadlink node carries.
    #[track_caller]
    fn check_node(graph: &Value, root: &Path, id: &str, path: Option<&str>, label: &str, dead: bool, lc: i64, bc: i64, deg: i64) {
        let n = node(graph, id);
        assert_eq!(
            sorted_keys(n),
            ["backlink_count", "degree", "id", "is_deadlink", "label", "link_count", "path"],
            "node `{id}` key set"
        );
        assert_eq!(n["label"].as_str(), Some(label), "node `{id}` label");
        assert_eq!(n["is_deadlink"].as_bool(), Some(dead), "node `{id}` is_deadlink");
        assert_eq!(n["link_count"], json!(lc), "node `{id}` link_count");
        assert_eq!(n["backlink_count"], json!(bc), "node `{id}` backlink_count");
        assert_eq!(n["degree"], json!(deg), "node `{id}` degree");
        match path {
            Some(rel) => assert_eq!(rel_id(root, n["path"].as_str().unwrap()), rel, "node `{id}` path"),
            None => assert_eq!(n["path"], Value::Null, "node `{id}` path must be None"),
        }
    }

    fn check_stats(graph: &Value, nodes: i64, edges: i64, dead: i64) {
        let s = &graph["stats"];
        assert_eq!(sorted_keys(s), ["deadlinks_count", "total_edges", "total_nodes"]);
        assert_eq!(s["total_nodes"], json!(nodes));
        assert_eq!(s["total_edges"], json!(edges));
        assert_eq!(s["deadlinks_count"], json!(dead));
    }

    /// Edges as `source>target|label|is_wikilink`, in the reference's row order.
    fn edge_cells(graph: &Value) -> Vec<String> {
        graph["edges"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| {
                assert_eq!(
                    sorted_keys(e),
                    ["is_wikilink", "label", "source", "target"],
                    "edge key set"
                );
                format!(
                    "{}>{}|{}|{}",
                    e["source"].as_str().unwrap(),
                    e["target"].as_str().unwrap(),
                    e["label"].as_str().unwrap(),
                    e["is_wikilink"].as_bool().unwrap()
                )
            })
            .collect()
    }

    fn sorted_edges(graph: &Value) -> Vec<String> {
        let mut v = edge_cells(graph);
        v.sort();
        v
    }

    fn cell(v: &Value) -> String {
        match v {
            Value::Null => "-".to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Number(n) => n.to_string(),
            Value::String(s) => s.clone(),
            other => other.to_string(),
        }
    }

    /// `link_id` is a walk-order artefact, so it is checked separately: positive
    /// and unique, never compared against a literal.
    fn check_link_ids(rows: &[Value]) {
        let mut seen: Vec<i64> = rows.iter().map(|r| r["link_id"].as_i64().expect("link_id integer")).collect();
        assert!(seen.iter().all(|i| *i > 0), "link_id must be a positive rowid");
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), rows.len(), "link_id must be unique per row");
    }

    /// The four-document fixture used by the graph-build, forward-link and
    /// backlink tests; every number below is CPython's for this exact text.
    fn build_workspace(root: &Path) -> LinkIndexer {
        put(
            root,
            "home.md",
            "# Home\n[[notes]]\n[[Ghost]]\n[Text](docs/other.md)\n[dead](missing.md)\n\
             ![img](pic.png)\n[asset](asset.png)\n[[#Self Head]]\n`[[in code]]`\n\
             [ext](https://example.com/a.md)\n",
        );
        put(root, "notes.md", "# Notes\n[[home]]\n[back](../home.md)\n");
        put(root, "docs/other.md", "# Other Doc\n[[home]]\n[[Ghost]]\n");
        put(root, "sub/deep.md", "# Deep\n[[notes]]\n[[../home]]\n[[notes]]\n");
        put(root, "pic.png", "x");
        put(root, "asset.png", "x");
        let ix = indexer_at(root);
        let root_s = root.to_string_lossy().to_string();
        let stats = ix.index_directory(&root_s, false).unwrap();
        assert_eq!(
            stats.to_json(),
            json!({"scanned_count": 4, "indexed_count": 4, "deleted_count": 0}),
            "only the four markdown files are scanned"
        );
        ix
    }

    /// Graph build (`get_graph_data` 400-503): doc nodes first, phantom dead nodes
    /// appended in link order, degrees summed, `stats` a three-key dict.
    #[test]
    fn graph_build_nodes_edges_and_degrees_match_python() {
        let root = scratch("build");
        let _tree = Tree(root.clone());
        let ix = build_workspace(&root);
        let root_s = root.to_string_lossy().to_string();
        let g = ix.get_graph_data(Some(&root_s), 500).unwrap();

        assert_eq!(sorted_keys(&g), ["edges", "nodes", "stats"]);
        let mut got: Vec<String> = ids(&g);
        assert_eq!(
            {
                got.sort();
                got
            },
            [
                "Ghost", "asset.png", "docs/other.md", "home.md", "missing.md", "notes.md", "sub/deep.md"
            ],
            "node id set; `_get_node_id` relativises with forward slashes"
        );
        let all_ids = ids(&g);
        let mut head: Vec<&str> = all_ids[..4].iter().map(String::as_str).collect();
        head.sort();
        assert_eq!(
            head,
            ["docs/other.md", "home.md", "notes.md", "sub/deep.md"],
            "real documents are registered before any phantom node"
        );

        check_node(&g, &root, "home.md", Some("home.md"), "Home", false, 6, 5, 11);
        check_node(&g, &root, "notes.md", Some("notes.md"), "Notes", false, 2, 3, 5);
        check_node(&g, &root, "docs/other.md", Some("docs/other.md"), "Other Doc", false, 2, 1, 3);
        check_node(&g, &root, "sub/deep.md", Some("sub/deep.md"), "Deep", false, 3, 0, 3);
        check_node(&g, &root, "Ghost", None, "Ghost", true, 0, 2, 2);
        check_node(&g, &root, "missing.md", None, "missing.md", true, 0, 1, 1);
        check_node(&g, &root, "asset.png", None, "asset.png", true, 0, 1, 1);

        let mut want = vec![
            "docs/other.md>Ghost||true".to_string(),
            "docs/other.md>home.md||true".to_string(),
            "home.md>Ghost||true".to_string(),
            "home.md>asset.png|asset|false".to_string(),
            "home.md>docs/other.md|Text|false".to_string(),
            "home.md>home.md||true".to_string(),
            "home.md>missing.md|dead|false".to_string(),
            "home.md>notes.md||true".to_string(),
            "notes.md>home.md||true".to_string(),
            "notes.md>home.md|back|false".to_string(),
            "sub/deep.md>home.md||true".to_string(),
            "sub/deep.md>notes.md||true".to_string(),
            "sub/deep.md>notes.md||true".to_string(),
        ];
        want.sort();
        assert_eq!(sorted_edges(&g), want, "edge multiset");
        check_stats(&g, 7, 13, 3);
    }

    /// Resolution ladder (`resolve_target` 186-220) as seen through
    /// `get_forward_links` (348-358), ordered by `line_no`.
    #[test]
    fn graph_forward_links_show_the_relative_resolution_ladder() {
        let root = scratch("forward");
        let _tree = Tree(root.clone());
        let ix = build_workspace(&root);
        let home = stored(&root, "home.md");
        let rows = ix.get_forward_links(&home).unwrap();
        assert_eq!(
            sorted_keys(&rows[0]),
            ["alias", "heading", "is_wikilink", "line_no", "link_id", "target_clean", "target_path", "target_raw"]
        );
        assert_eq!(rows.len(), 6, "`![]()`, the https URL and the inline code emit no link");
        check_link_ids(&rows);
        let got: Vec<String> = rows
            .iter()
            .map(|r| {
                let path = match r["target_path"].as_str() {
                    Some(p) => rel_id(&root, p),
                    None => "-".to_string(),
                };
                format!(
                    "{}|{}|{}|{}|{}|{}|{}",
                    r["line_no"].as_i64().unwrap(),
                    cell(&r["target_raw"]),
                    cell(&r["target_clean"]),
                    path,
                    cell(&r["alias"]),
                    cell(&r["heading"]),
                    cell(&r["is_wikilink"])
                )
            })
            .collect();
        assert_eq!(
            got,
            vec![
                "2|notes|notes|notes.md|-|-|1",
                "3|Ghost|Ghost|-|-|-|1",
                "4|docs/other.md|docs/other.md|docs/other.md|Text|-|0",
                "5|missing.md|missing.md|-|dead|-|0",
                "7|asset.png|asset.png|-|asset|-|0",
                "8|#Self Head|#Self Head|home.md|-|Self Head|1",
            ],
            "extension omission, relative dir, dead-by-suffix, and the `[[#h]]` self-resolve"
        );
    }

    /// Backlinks (`get_backlinks` 360-373): `ORDER BY d.title ASC, l.line_no ASC`,
    /// `is_wikilink` an integer, and the `[[#heading]]` self-link included.
    #[test]
    fn graph_backlinks_are_title_ordered_and_keep_self_links() {
        let root = scratch("backlinks");
        let _tree = Tree(root.clone());
        let ix = build_workspace(&root);
        let home = stored(&root, "home.md");
        let rows = ix.get_backlinks(&home).unwrap();
        assert_eq!(
            sorted_keys(&rows[0]),
            [
                "alias", "heading", "is_wikilink", "line_no", "link_id", "source_path", "source_title",
                "target_clean", "target_raw"
            ]
        );
        assert_eq!(rows.len(), 5, "three inbound links plus the two from notes.md");
        check_link_ids(&rows);
        let got: Vec<String> = rows
            .iter()
            .map(|r| {
                format!(
                    "{}|{}|{}|{}|{}|{}|{}",
                    rel_id(&root, r["source_path"].as_str().unwrap()),
                    cell(&r["source_title"]),
                    r["line_no"].as_i64().unwrap(),
                    cell(&r["target_raw"]),
                    cell(&r["alias"]),
                    cell(&r["heading"]),
                    cell(&r["is_wikilink"])
                )
            })
            .collect();
        assert_eq!(
            got,
            vec![
                "sub/deep.md|Deep|3|../home|-|-|1",
                "home.md|Home|8|#Self Head|-|Self Head|1",
                "notes.md|Notes|2|home|-|-|1",
                "notes.md|Notes|3|../home.md|back|-|0",
                "docs/other.md|Other Doc|2|home|-|-|1",
            ],
            "rows sort by title, not by source path or link_id"
        );
    }

    /// Backlinks for the no-inbound cases: an indexed file nobody links to, a
    /// path never indexed, and a relative argument.
    #[test]
    fn graph_backlinks_are_empty_for_unlinked_unknown_and_relative_paths() {
        let root = scratch("back-empty");
        let _tree = Tree(root.clone());
        let ix = build_workspace(&root);
        assert!(ix.get_backlinks(&stored(&root, "sub/deep.md")).unwrap().is_empty(), "nobody links to deep.md");
        assert!(ix.get_backlinks(&stored(&root, "nowhere.md")).unwrap().is_empty(), "never indexed");
        assert!(ix.get_backlinks("home.md").unwrap().is_empty(), "abspath() is against the process cwd");
        assert!(ix.get_backlinks("").unwrap().is_empty(), "an empty argument is cwd itself");
        assert!(ix.get_forward_links("").unwrap().is_empty());
    }

    /// Deadlinks (`get_deadlinks` 375-398): seven keys (`heading` is not among
    /// them), `source_path ASC` + `line_no ASC`, and the descendant-only `LIKE`.
    #[test]
    fn graph_deadlinks_scope_to_descendants_only_and_report_no_heading_key() {
        let root = scratch("dead");
        let _tree = Tree(root.clone());
        put(&root, "docs/in.md", "# In\n[[ghost-in]]\n");
        put(&root, "docsx/out.md", "# Out\n[[ghost-out]]\n");
        put(&root, "top.md", "# Top\n[[ghost-top]]\n");
        let ix = indexer_at(&root);
        let root_s = root.to_string_lossy().to_string();
        assert_eq!(
            ix.index_directory(&root_s, false).unwrap().to_json(),
            json!({"scanned_count": 3, "indexed_count": 3, "deleted_count": 0})
        );

        let render = |rows: Vec<Value>| -> Vec<String> {
            assert_eq!(
                sorted_keys(&rows[0]),
                ["alias", "line_no", "link_id", "source_path", "source_title", "target_clean", "target_raw"],
                "the deadlink SELECT list has no `heading`"
            );
            check_link_ids(&rows);
            rows.iter()
                .map(|r| {
                    format!(
                        "{}|{}|{}|{}|{}|{}",
                        rel_id(&root, r["source_path"].as_str().unwrap()),
                        cell(&r["source_title"]),
                        r["line_no"].as_i64().unwrap(),
                        cell(&r["target_raw"]),
                        cell(&r["target_clean"]),
                        cell(&r["alias"])
                    )
                })
                .collect()
        };
        let all = render(ix.get_deadlinks(Some(&root_s)).unwrap());
        assert_eq!(
            all,
            vec![
                "docs/in.md|In|2|ghost-in|ghost-in|-",
                "docsx/out.md|Out|2|ghost-out|ghost-out|-",
                "top.md|Top|2|ghost-top|ghost-top|-",
            ],
            "ordered by absolute source_path, so `docs\\` sorts before `docsx\\`"
        );
        assert_eq!(ix.get_deadlinks(None).unwrap().len(), 3, "no root means no LIKE filter");
        assert_eq!(
            render(ix.get_deadlinks(Some(&stored(&root, "docs"))).unwrap()),
            vec!["docs/in.md|In|2|ghost-in|ghost-in|-"],
            "`_root_pattern` appends the separator, so `docsx` is not a descendant"
        );
        assert!(ix.get_deadlinks(Some(&stored(&root, "nope"))).unwrap().is_empty(), "unknown root -> []");
        // py:377 tests `if root_dir:`, and readmd.py:2144 folds `directory or None` first.

        let scoped = ix.get_graph_data(Some(&stored(&root, "docs")), 500).unwrap();
        check_node(&scoped, &root, "in.md", Some("docs/in.md"), "In", false, 1, 0, 1);
        check_node(&scoped, &root, "ghost-in", None, "ghost-in", true, 0, 1, 1);
        check_stats(&scoped, 2, 1, 1);
        assert_eq!(
            edge_cells(&scoped),
            vec!["in.md>ghost-in||true"],
            "node ids are relative to the *scoped* root"
        );
    }

    /// A vanished target re-opens its inbound links as deadlinks through step 6
    /// (`index_directory` 324-340), splitting one file into two graph nodes.
    #[test]
    fn graph_deadlinks_classify_missing_non_markdown_and_deleted_targets() {
        let root = scratch("deleted");
        let _tree = Tree(root.clone());
        put(&root, "a.md", "# A\n[[b]]\n[b](b.md)\n");
        put(&root, "b.md", "# B\n");
        let ix = indexer_at(&root);
        let root_s = root.to_string_lossy().to_string();
        assert_eq!(
            ix.index_directory(&root_s, false).unwrap().to_json(),
            json!({"scanned_count": 2, "indexed_count": 2, "deleted_count": 0})
        );
        assert!(ix.get_deadlinks(Some(&root_s)).unwrap().is_empty());
        let g1 = ix.get_graph_data(Some(&root_s), 500).unwrap();
        check_node(&g1, &root, "b.md", Some("b.md"), "B", false, 0, 2, 2);
        check_stats(&g1, 2, 2, 0);

        fs::remove_file(stored(&root, "b.md")).unwrap();
        assert_eq!(
            ix.index_directory(&root_s, false).unwrap().to_json(),
            json!({"scanned_count": 1, "indexed_count": 0, "deleted_count": 1}),
            "a.md is not re-parsed; only step 6 re-points its links"
        );
        let dead = ix.get_deadlinks(Some(&root_s)).unwrap();
        let got: Vec<String> = dead
            .iter()
            .map(|r| {
                format!(
                    "{}|{}|{}|{}|{}",
                    rel_id(&root, r["source_path"].as_str().unwrap()),
                    r["line_no"].as_i64().unwrap(),
                    cell(&r["target_raw"]),
                    cell(&r["target_clean"]),
                    cell(&r["alias"])
                )
            })
            .collect();
        assert_eq!(got, vec!["a.md|2|b|b|-", "a.md|3|b.md|b.md|b"]);
        let g2 = ix.get_graph_data(Some(&root_s), 500).unwrap();
        check_node(&g2, &root, "a.md", Some("a.md"), "A", false, 2, 0, 2);
        check_node(&g2, &root, "b", None, "b", true, 0, 1, 1);
        check_node(&g2, &root, "b.md", None, "b.md", true, 0, 1, 1);
        check_stats(&g2, 3, 2, 2);
        assert_eq!(edge_cells(&g2), vec!["a.md>b||true", "a.md>b.md|b|false"]);
        assert!(ix.get_backlinks(&stored(&root, "a.md")).unwrap().is_empty());
        assert!(ix.get_forward_links(&stored(&root, "a.md")).unwrap().iter().all(|r| r["target_path"].is_null()));
        assert_eq!(
            ix.index_directory(&stored(&root, "not-a-dir"), false).unwrap().to_json(),
            json!({"scanned_count": 0, "indexed_count": 0, "deleted_count": 0})
        );
    }

    /// `max_nodes` truncation (`get_graph_data` 400-408): the `LIMIT` clamps
    /// *documents* only, in `doc_id` order, and dropped-but-targeted docs return
    /// as phantom nodes labelled with their filename instead of their title.
    #[test]
    fn graph_max_nodes_keeps_doc_id_prefix_and_adds_phantom_targets() {
        let root = scratch("cap");
        let _tree = Tree(root.clone());
        let ix = indexer_at(&root);
        let root_s = root.to_string_lossy().to_string();
        // One document per pass, so `doc_id` order is the creation order.
        for (name, body) in [
            ("a.md", "# Alpha\n[[b]]\n[[z]]\n"),
            ("b.md", "# Bravo\n[[c]]\n"),
            ("c.md", "# Charlie\n[[a]]\n"),
            ("k4.md", "# Four\n[[a]]\n"),
            ("k5.md", "# Five\n[[a]]\n"),
        ] {
            put(&root, name, body);
            ix.index_directory(&root_s, false).unwrap();
        }
        let full = ix.get_graph_data(Some(&root_s), 500).unwrap();
        assert_eq!(
            ids(&full),
            ["a.md", "b.md", "c.md", "k4.md", "k5.md", "z"],
            "unclamped: five docs by doc_id plus the deadlink"
        );

        let one = ix.get_graph_data(Some(&root_s), 1).unwrap();
        assert_eq!(ids(&one), ["a.md", "b.md", "z", "c.md"]);
        check_node(&one, &root, "a.md", Some("a.md"), "Alpha", false, 2, 1, 3);
        check_node(&one, &root, "b.md", Some("b.md"), "b.md", false, 1, 1, 2);
        check_node(&one, &root, "c.md", Some("c.md"), "c.md", false, 1, 1, 2);
        check_node(&one, &root, "z", None, "z", true, 0, 1, 1);
        assert_eq!(
            edge_cells(&one),
            ["a.md>b.md||true", "a.md>z||true", "b.md>c.md||true", "c.md>a.md||true"],
            "a truncated-out doc keeps its own outgoing edge once it is a node"
        );
        check_stats(&one, 4, 4, 1);

        let two = ix.get_graph_data(Some(&root_s), 2).unwrap();
        assert_eq!(ids(&two), ["a.md", "b.md", "z", "c.md"]);
        check_node(&two, &root, "b.md", Some("b.md"), "Bravo", false, 1, 1, 2);
        check_node(&two, &root, "c.md", Some("c.md"), "c.md", false, 1, 1, 2);
        check_stats(&two, 4, 4, 1);

        let three = ix.get_graph_data(Some(&root_s), 3).unwrap();
        assert_eq!(ids(&three), ["a.md", "b.md", "c.md", "z"]);
        check_node(&three, &root, "c.md", Some("c.md"), "Charlie", false, 1, 1, 2);
        check_stats(&three, 4, 4, 1);
        assert_eq!(
            edge_cells(&three),
            ["a.md>b.md||true", "a.md>z||true", "b.md>c.md||true", "c.md>a.md||true"],
            "k4.md and k5.md are dropped with their sources"
        );
    }

    /// `max_nodes` reports nothing but the inflated counts: no `truncated` key,
    /// and the `[10, 2000]` clamp lives in `readmd.py:2105-2109`, not the module.
    #[test]
    fn graph_max_nodes_zero_and_unlimited_have_no_truncation_field() {
        let root = scratch("cap-zero");
        let _tree = Tree(root.clone());
        let ix = indexer_at(&root);
        let root_s = root.to_string_lossy().to_string();
        for (name, body) in [
            ("a.md", "# Alpha\n[[b]]\n[[z]]\n"),
            ("b.md", "# Bravo\n[[c]]\n"),
            ("c.md", "# Charlie\n[[a]]\n"),
            ("k4.md", "# Four\n[[a]]\n"),
            ("k5.md", "# Five\n[[a]]\n"),
        ] {
            put(&root, name, body);
            ix.index_directory(&root_s, false).unwrap();
        }
        let zero = ix.get_graph_data(Some(&root_s), 0).unwrap();
        assert_eq!(zero["nodes"].as_array().unwrap().len(), 0, "LIMIT 0 keeps no document");
        assert_eq!(zero["edges"].as_array().unwrap().len(), 0, "every source is then filtered out");
        check_stats(&zero, 0, 0, 0);
        assert_eq!(
            sorted_keys(&ix.get_graph_data(None, 0).unwrap()),
            ["edges", "nodes", "stats"],
            "the response never says that it truncated"
        );
        assert_eq!(ids(&ix.get_graph_data(Some(&root_s), 1).unwrap()), ["a.md", "b.md", "z", "c.md"], "no floor of 10");
        let none = ix.get_graph_data(None, 500).unwrap();
        assert_eq!(ids(&none), ids(&ix.get_graph_data(Some(&root_s), 500).unwrap()), "flat names here equal the relative ids");
        assert_eq!(none["nodes"][0]["path"], json!(stored(&root, "a.md")), "`path` stays absolute without a root");
        check_stats(&none, 6, 6, 1);
    }

    /// Duplicate basenames: resolution takes the first walk hit, and without a
    /// `root_dir` the node ids collapse two documents into one dict.
    #[test]
    fn graph_duplicate_basenames_collapse_without_a_root() {
        let root = scratch("dup");
        let _tree = Tree(root.clone());
        put(&root, "a/note.md", "# A Note\n");
        let ix = indexer_at(&root);
        let root_s = root.to_string_lossy().to_string();
        ix.index_directory(&root_s, false).unwrap();
        put(&root, "b/note.md", "# B Note\n");
        put(&root, "c.md", "# C\n[[note]]\n[[NOTE]]\n[[Note.MD]]\n");
        assert_eq!(
            ix.index_directory(&root_s, false).unwrap().to_json(),
            json!({"scanned_count": 3, "indexed_count": 2, "deleted_count": 0})
        );

        let fwd = ix.get_forward_links(&stored(&root, "c.md")).unwrap();
        assert_eq!(fwd.len(), 3);
        for row in &fwd {
            assert_eq!(
                rel_id(&root, row["target_path"].as_str().unwrap()),
                "a/note.md",
                "the lowercased-basename table yields its first walk entry"
            );
        }
        let rows = ix.get_backlinks(&stored(&root, "a/note.md")).unwrap();
        assert_eq!(rows.len(), 3, "the three spellings all resolve to a/note.md");
        assert!(ix.get_backlinks(&stored(&root, "b/note.md")).unwrap().is_empty(), "matching target_path is exact");

        let flat = ix.get_graph_data(None, 500).unwrap();
        assert_eq!(ids(&flat), ["note.md", "c.md"], "three documents collapse to two ids");
        check_node(&flat, &root, "note.md", Some("b/note.md"), "B Note", false, 0, 3, 3);
        check_node(&flat, &root, "c.md", Some("c.md"), "C", false, 3, 0, 3);
        assert_eq!(
            edge_cells(&flat),
            vec!["c.md>note.md||true"; 3],
            "the surviving node keeps the first slot but the last document's fields"
        );
        check_stats(&flat, 2, 3, 0);

        let scoped = ix.get_graph_data(Some(&root_s), 500).unwrap();
        assert_eq!(ids(&scoped), ["a/note.md", "c.md", "b/note.md"]);
        check_node(&scoped, &root, "b/note.md", Some("b/note.md"), "B Note", false, 0, 0, 0);
        check_stats(&scoped, 3, 3, 0);
    }
}
