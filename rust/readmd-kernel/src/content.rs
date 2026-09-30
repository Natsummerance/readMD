//! Document authority: the filesystem is the source of truth, the SQLite store
//! in [`crate::store`] is a derived index kept in sync by this module.

use crate::error::{Error, Result};
use crate::{paths, App};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// CPython whitespace primitives
//
// Every `strip()` / `lstrip()` / `rstrip()` / bare `split()` in this module's
// Python authorities — `readmd_core/toc_engine.py:45,62`, `source_map.py:28`,
// `mdcheck.py:60`, `readmd_modules/link_indexer.py:111,120,132,150-152`,
// `readmd_core/dialogs.py:64`, `readmd.py:3155` — is CPython's, and CPython
// counts U+001C..U+001F (file/group/record/unit separators) as whitespace on
// top of Unicode `White_Space`.  Measured on CPython 3.11.15:
// `'\x1c'.isspace()` is True, `' \x1c '.strip()` is `''`,
// `'a\x1cb'.split()` is `['a','b']`, while `'\x1a'.isspace()` is False.
// Rust's `trim*` / `split_whitespace` / `char::is_whitespace` are narrower, so
// each of those sites has to go through these helpers.
// ---------------------------------------------------------------------------

#[inline]
fn py_isspace(ch: char) -> bool {
    ch.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&ch)
}

/// Py `str.lstrip()`.
fn py_strip_start(s: &str) -> &str {
    s.trim_start_matches(py_isspace as fn(char) -> bool)
}

/// Py `str.strip()`.
fn py_strip(s: &str) -> &str {
    s.trim_matches(py_isspace as fn(char) -> bool)
}

/// Py `s.split()[0] if s.split() else ''` — the leading token delimited by
/// CPython whitespace runs.
fn py_first_token(s: &str) -> &str {
    let t = py_strip_start(s);
    for (i, c) in t.char_indices() {
        if py_isspace(c) {
            return &t[..i];
        }
    }
    t
}

pub const MD_EXTS: &[&str] = &["md", "markdown", "mdown", "mkd", "mdx", "txt"];
pub const TEXT_EXTS: &[&str] = &[
    "js", "mjs", "cjs", "ts", "tsx", "jsx", "py", "rs", "go", "java", "kt", "c", "h", "cc",
    "cpp", "hpp", "cs", "rb", "php", "swift", "sh", "bash", "zsh", "ps1", "bat", "cmd", "html",
    "htm", "css", "scss", "less", "sass", "json", "jsonc", "yaml", "yml", "toml", "ini", "cfg",
    "conf", "env", "xml", "csv", "tsv", "sql", "graphql", "lua", "pl", "pm", "r", "jl", "dart",
    "scala", "groovy", "vue", "svelte", "astro", "ex", "exs", "erl", "hs", "ml", "nim", "zig",
    "tf", "dockerfile", "makefile", "cmake", "gradle", "properties", "gitignore", "editorconfig",
    "log", "svg", "xaml", "asm", "s", "v", "vhdl", "f90", "f", "pas", "d", "cr", "re", "rei",
];
pub const IMAGE_EXTS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "bmp", "ico", "tiff", "tif", "avif", "apng", "jfif",
];
pub const BINARY_EXTS: &[&str] = &[
    "doc", "docx", "ppt", "pptx", "xls", "xlsx", "pdf", "epub", "mobi", "rtf", "odt", "ods",
    "odp", "zip", "tar", "gz", "7z", "rar", "exe", "dll", "so", "dylib", "bin", "class", "jar",
    "woff", "woff2", "ttf", "otf", "eot", "mp3", "mp4", "wav", "avi", "mov", "mkv", "flac",
    "ogg", "webm", "pyc", "pdb", "db", "sqlite", "sqlite3",
];

pub fn ext_of(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default()
}

pub fn kind_of(path: &Path) -> &'static str {
    let ext = ext_of(path);
    if ext.is_empty() {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        return match name.as_str() {
            "makefile" | "dockerfile" | "readme" | "license" | "cmakelists.txt" => "code",
            _ => "unknown",
        };
    }
    if MD_EXTS.contains(&ext.as_str()) {
        "markdown"
    } else if IMAGE_EXTS.contains(&ext.as_str()) {
        "image"
    } else if BINARY_EXTS.contains(&ext.as_str()) {
        "binary"
    } else if TEXT_EXTS.contains(&ext.as_str()) {
        "code"
    } else {
        "unknown"
    }
}

pub fn is_readable(path: &Path) -> bool {
    matches!(kind_of(path), "markdown" | "code")
}

pub fn modified_millis(path: &Path) -> i64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .map(unix_millis)
        .unwrap_or(0)
}

pub fn unix_millis(t: SystemTime) -> i64 {
    t.duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Read a text file, detecting its encoding (BOMs, UTF-8, GB18030, Big5,
/// Windows-1252) instead of decoding everything as lossy UTF-8.
pub fn read_text(path: &Path) -> Result<String> {
    Ok(read_text_detect(path)?.0)
}

/// [`read_text`] plus the detected encoding name.
pub fn read_text_detect(path: &Path) -> Result<(String, &'static str)> {
    let bytes = std::fs::read(path)?;
    Ok(crate::text_encoding::detect_and_decode(&bytes))
}

pub fn strip_bom(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        return String::from_utf8_lossy(&bytes[3..]).into_owned();
    }
    if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
        let wide: Vec<u16> = bytes
            .chunks_exact(2)
            .skip(1)
            .map(|c| {
                if bytes.starts_with(&[0xff, 0xfe]) {
                    u16::from_le_bytes([c[0], c[1]])
                } else {
                    u16::from_be_bytes([c[0], c[1]])
                }
            })
            .collect();
        return String::from_utf16_lossy(&wide);
    }
    String::from_utf8_lossy(bytes).into_owned()
}

/// Atomic write: tmp file in the same directory then rename, falling back to a
/// direct copy when the platform refuses to rename over an open handle.
pub fn write_text_atomic(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = sibling_tmp(path);
    std::fs::write(&tmp, content)?;
    match std::fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(_) => {
            let out = std::fs::write(path, content);
            let _ = std::fs::remove_file(&tmp);
            out.map_err(Error::from)
        }
    }
}

pub fn write_bytes_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = sibling_tmp(path);
    std::fs::write(&tmp, bytes)?;
    match std::fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(_) => {
            let out = std::fs::write(path, bytes);
            let _ = std::fs::remove_file(&tmp);
            out.map_err(Error::from)
        }
    }
}

fn sibling_tmp(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("readmd.tmp");
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    path.with_file_name(format!(".{name}.{nanos}.tmp"))
}

// ---------------------------------------------------- legacy contract surface
//
// `readmd.py` builds the `/api/file` body and the `/api/save` result inline,
// and the parity harness diffs the *key set* of both.  These items exist so the
// P1 handlers can answer with exactly those keys instead of the richer kernel
// shape [`describe`] returns.

/// `readmd.py:115` `CODE_CONFIG_EXTS`, dotted and lower case.
pub const CODE_CONFIG_EXTS: &[&str] = &[
    ".toml", ".yaml", ".yml", ".json", ".json5", ".jsonc", ".ini", ".cfg", ".conf", ".config",
    ".env", ".properties", ".xml", ".plist", ".inf", ".bat", ".cmd", ".ps1", ".psm1", ".sh",
    ".bash", ".zsh", ".fish", ".vbs", ".py", ".js", ".mjs", ".cjs", ".ts", ".tsx", ".jsx", ".c",
    ".cpp", ".h", ".hpp", ".cc", ".cxx", ".cs", ".java", ".kt", ".kts", ".rs", ".go", ".rb",
    ".php", ".swift", ".lua", ".r", ".m", ".dart", ".sql", ".dockerfile", ".makefile", ".gradle",
    ".html", ".htm", ".css", ".scss", ".sass", ".less", ".vue", ".svelte", ".log", ".out",
    ".err", ".diff", ".patch", ".gitignore", ".gitattributes", ".editorconfig", ".npmrc", ".rst",
    ".asciidoc", ".adoc", ".bib", ".csv", ".tsv",
];

/// `readmd.py:957` `SAVE_EXTENSIONS`.
pub const SAVE_EXTENSIONS: &[&str] = &[".md", ".markdown", ".mdown", ".mkd", ".mdx", ".txt"];

/// `src.readmd_modules.convert.EXT_TO_LANG`, the lookup `_api_file` uses for
/// `code_lang`.  `.txt` / `.html` really are absent, so those report `""`.
pub const CODE_LANG_BY_EXT: &[(&str, &str)] = &[
    (".toml", "toml"), (".yaml", "yaml"), (".yml", "yaml"), (".json", "json"),
    (".json5", "json5"), (".jsonc", "jsonc"), (".ini", "ini"), (".cfg", "ini"),
    (".conf", "ini"), (".config", "xml"), (".env", "bash"), (".properties", "properties"),
    (".xml", "xml"), (".plist", "xml"), (".inf", "ini"), (".bat", "batch"),
    (".cmd", "batch"), (".ps1", "powershell"), (".psm1", "powershell"), (".sh", "bash"),
    (".bash", "bash"), (".zsh", "bash"), (".fish", "fish"), (".vbs", "vbscript"),
    (".py", "python"), (".js", "javascript"), (".mjs", "javascript"), (".cjs", "javascript"),
    (".ts", "typescript"), (".tsx", "tsx"), (".jsx", "jsx"), (".c", "c"),
    (".cpp", "cpp"), (".h", "c"), (".hpp", "cpp"), (".cc", "cpp"), (".cxx", "cpp"),
    (".cs", "csharp"), (".java", "java"), (".kt", "kotlin"), (".kts", "kotlin"),
    (".rs", "rust"), (".go", "go"), (".rb", "ruby"), (".php", "php"),
    (".swift", "swift"), (".lua", "lua"), (".r", "r"), (".m", "objectivec"),
    (".dart", "dart"), (".sql", "sql"), (".dockerfile", "dockerfile"), (".makefile", "makefile"),
    (".gradle", "groovy"), (".css", "css"), (".scss", "scss"), (".sass", "sass"),
    (".less", "less"), (".vue", "vue"), (".svelte", "svelte"), (".log", "log"),
    (".out", "log"), (".err", "log"), (".diff", "diff"), (".patch", "diff"),
    (".gitignore", "gitignore"), (".gitattributes", "gitignore"), (".editorconfig", "ini"),
    (".npmrc", "ini"), (".rst", "rst"), (".asciidoc", "asciidoc"), (".adoc", "asciidoc"),
    (".bib", "bibtex"), (".tex", "latex"), (".latex", "latex"), (".csv", "csv"), (".tsv", "tsv"),
];

pub fn is_code_config_ext(dotted_ext: &str) -> bool {
    CODE_CONFIG_EXTS.contains(&dotted_ext)
}

/// `EXT_TO_LANG.get(ext, '')`.
pub fn code_lang_of(dotted_ext: &str) -> String {
    CODE_LANG_BY_EXT
        .iter()
        .find(|(k, _)| *k == dotted_ext)
        .map(|(_, v)| (*v).to_string())
        .unwrap_or_default()
}

/// `os.path.basename` over a client-supplied path, so `/` works on Windows too.
pub fn py_basename(path: &str) -> String {
    let trimmed = path.trim_end_matches(['/', '\\']);
    match trimmed.rfind(['/', '\\']) {
        Some(i) => trimmed[i + 1..].to_string(),
        None => trimmed.to_string(),
    }
}

/// `os.path.dirname`, mirroring the client's own separator style.
pub fn py_dirname(path: &str) -> String {
    let trimmed = path.trim_end_matches(['/', '\\']);
    match trimmed.rfind(['/', '\\']) {
        Some(i) => trimmed[..i].to_string(),
        None => String::new(),
    }
}

/// `os.path.splitext(name)[1].lower()`.
pub fn py_splitext_lower(name: &str) -> String {
    match name.rfind('.') {
        Some(i) if i > 0 => name[i..].to_ascii_lowercase(),
        _ => String::new(),
    }
}

/// `len(text.splitlines())` for the CR / LF / CRLF terminators: a trailing
/// newline does not add an empty line, and an empty string has no lines.
pub fn py_splitlines_len(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    let mut lines = 1usize;
    let mut prev_cr = false;
    for c in text.chars() {
        if prev_cr {
            prev_cr = false;
            if c == '\n' {
                continue;
            }
        }
        if c == '\r' {
            prev_cr = true;
            lines += 1;
        } else if c == '\n' {
            lines += 1;
        }
    }
    if text.ends_with(['\n', '\r']) {
        lines -= 1;
    }
    lines
}

/// `readmd.py:941` `read_text`, which returns `(text, encoding)`.
///
/// The decoder priority is UTF-8 BOM, strict UTF-8, GB18030, Big5, Latin-1.
/// GB18030/Big5 need codepage tables the vendored crate set has none of, so
/// those two lanes fall through to Latin-1 (Python's own final step); see the
/// P1 report's unresolved section.
pub fn read_text_with_encoding(path: &Path) -> Result<(String, String)> {
    let data = std::fs::read(path)?;
    if data.starts_with(&[0xef, 0xbb, 0xbf]) {
        let text = String::from_utf8_lossy(&data[3..]).into_owned();
        return Ok((text, "utf-8-sig".into()));
    }
    if let Ok(text) = std::str::from_utf8(&data) {
        return Ok((text.to_string(), "utf-8".into()));
    }
    Ok((latin1_decode(&data), "latin-1".into()))
}

fn latin1_decode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| char::from(*b)).collect()
}

/// `_api_file` (`readmd.py:3011`) body.  `raw` is the path string the client
/// asked for; the response echoes it and derives `name` / `dir` from it, while
/// reads and stats use the same string as a filesystem path, exactly like
/// Python does with `p`.
pub fn file_payload(raw: &str, meta_only: bool) -> Result<Value> {
    let meta = std::fs::metadata(raw)?;
    let name = py_basename(raw);
    let ext = py_splitext_lower(&name);
    let is_code = is_code_config_ext(&ext);
    let code_lang = if is_code { code_lang_of(&ext) } else { String::new() };
    let mut d = json!({
        "path": raw,
        "name": name,
        "dir": py_dirname(raw),
        "mtime": unix_seconds(&meta),
        "size": meta.len() as i64,
        "is_code": is_code,
        "code_lang": code_lang,
        "ext": ext,
    });
    if meta_only {
        return Ok(d);
    }
    let (text, enc) = read_text_with_encoding(Path::new(raw))?;
    let (fixes, stats, fixed_text) = if is_code {
        let stats = json!({ "lines": py_splitlines_len(&text), "chars": text.chars().count() });
        (json!([]), stats, text.clone())
    } else {
        let fr = crate::readmd_fix::fix_markdown(&text);
        (json!(fr.fixes), fr.stats, fr.text.clone())
    };
    let original: Value = if fixed_text == text { Value::Null } else { json!(text) };
    if let Some(map) = d.as_object_mut() {
        map.insert("encoding".into(), json!(enc));
        map.insert("content".into(), json!(fixed_text));
        map.insert("original".into(), original);
        map.insert("fixes".into(), fixes);
        map.insert("stats".into(), stats);
        // `structured` is set by the txtmd lane, which has no kernel port yet.
        map.insert("structured".into(), json!(false));
    }
    Ok(d)
}

fn unix_seconds(meta: &std::fs::Metadata) -> f64 {
    meta.modified()
        .map(|t| match t.duration_since(UNIX_EPOCH) {
            Ok(d) => d.as_secs_f64(),
            Err(e) => -(e.duration().as_secs_f64()),
        })
        .unwrap_or(0.0)
}

/// `os.path.join(a, b)` over client-supplied strings.
fn py_join(a: &str, b: &str) -> String {
    if a.is_empty() {
        return b.to_string();
    }
    if a.ends_with(['/', '\\']) {
        return format!("{a}{b}");
    }
    format!("{a}{}{b}", std::path::MAIN_SEPARATOR)
}

/// `Handler._api_list` (`readmd.py:3095`) file collection: `os.walk` from `p`,
/// directories filtered of `.`/`_` prefixes in scandir order, file names
/// sorted, `MD_EXTS` suffixes collected, up to `limit` items.
pub fn list_markdown_files(p: &str, limit: usize) -> Vec<String> {
    let mut files: Vec<String> = Vec::new();
    walk_md(p, p, limit, &mut files);
    files.truncate(limit);
    files
}

/// One `os.walk` level.  Returns `false` when the caller should stop.
fn walk_md(root: &str, origin: &str, limit: usize, files: &mut Vec<String>) -> bool {
    // `depth = root[len(p):].count(os.sep)`; the separator is the OS one, so a
    // caller that passed `/` separators on Windows never trips the cut, exactly
    // like the legacy server.
    let rest = root.get(origin.len()..).unwrap_or("");
    if rest.matches(std::path::MAIN_SEPARATOR).count() >= 4 {
        return true;
    }
    let Ok(read) = std::fs::read_dir(root) else {
        return files.len() < limit;
    };
    let mut dirs: Vec<String> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    for entry in read.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || name.starts_with('_') {
            continue;
        }
        if entry.path().is_dir() {
            dirs.push(name);
        } else {
            names.push(name);
        }
    }
    names.sort();
    for n in names {
        // `readmd.py: n.lower().endswith(MD_EXTS)` — the Python tuple holds the
        // dotted suffixes (`.md`, `.markdown`, ...), so require the dot here too.
        let lower = n.to_ascii_lowercase();
        if MD_EXTS.iter().any(|e| lower.ends_with(&format!(".{e}"))) {
            files.push(py_join(root, &n));
            if files.len() >= limit {
                break;
            }
        }
    }
    if files.len() >= limit {
        return false;
    }
    for d in dirs {
        if !walk_md(&py_join(root, &d), origin, limit, files) {
            return false;
        }
    }
    true
}

/// `dir` -> sorted entry list, directories first then name asc.
pub fn list_dir(dir: &Path) -> Result<Value> {
    let mut dirs: Vec<Value> = Vec::new();
    let mut files: Vec<Value> = Vec::new();
    let read = std::fs::read_dir(dir)?;
    for entry in read.flatten() {
        let p = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let is_dir = p.is_dir();
        let meta = std::fs::metadata(&p).ok();
        let size = meta.as_ref().map(|m| m.len() as i64).unwrap_or(0);
        let mtime = modified_millis(&p);
        let value = json!({
            "name": name,
            "path": p.to_string_lossy(),
            "dir": is_dir,
            "size": if is_dir { Value::Null } else { json!(size) },
            "mtime": mtime as f64 / 1000.0,
            "mtimeMs": mtime,
            "kind": if is_dir { "directory" } else { kind_of(&p) },
            "ext": ext_of(&p),
        });
        if is_dir {
            dirs.push(value);
        } else {
            files.push(value);
        }
    }
    sort_entries(&mut dirs);
    sort_entries(&mut files);
    let mut entries = dirs;
    entries.extend(files);
    Ok(json!({
        "ok": true,
        "path": dir.to_string_lossy(),
        "entries": entries,
        "count": entries.len(),
    }))
}

fn sort_entries(list: &mut [Value]) {
    list.sort_by(|a, b| {
        let an = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let bn = b.get("name").and_then(|v| v.as_str()).unwrap_or("");
        an.to_ascii_lowercase().cmp(&bn.to_ascii_lowercase())
    });
}

pub const MAX_TREE_NODES: usize = 5_000;
pub const MAX_TREE_DEPTH: usize = 12;

/// Recursive workspace tree, hidden files and dot-dirs skipped.
pub fn tree(root: &Path) -> Value {
    let mut budget = MAX_TREE_NODES;
    node(root, 0, &mut budget)
}

fn node(path: &Path, depth: usize, budget: &mut usize) -> Value {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned());
    let mut obj = json!({
        "name": name,
        "path": path.to_string_lossy(),
        "dir": path.is_dir(),
    });
    if *budget == 0 {
        obj["truncated"] = json!(true);
        return obj;
    }
    *budget -= 1;
    if path.is_dir() && depth < MAX_TREE_DEPTH {
        let mut kids = Vec::new();
        if let Ok(read) = std::fs::read_dir(path) {
            let mut entries: Vec<PathBuf> = read
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    !p.file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| n.starts_with('.'))
                        .unwrap_or(false)
                })
                .collect();
            entries.sort();
            for p in entries {
                if *budget == 0 {
                    break;
                }
                kids.push(node(&p, depth + 1, budget));
            }
        }
        obj["children"] = json!(kids);
    } else if path.is_dir() {
        obj["children"] = json!([]);
    }
    obj
}

/// CJK-aware word count: latin/digit runs count as one, each CJK char counts one.
pub fn count_words(text: &str) -> i64 {
    let mut words: i64 = 0;
    let mut in_word = false;
    for c in text.chars() {
        if crate::store::is_cjk_char(c) {
            words += 1;
            in_word = false;
        } else if c.is_alphanumeric() || c == '_' || c == '\'' || c == '-' {
            if !in_word {
                words += 1;
                in_word = true;
            }
        } else {
            in_word = false;
        }
    }
    words
}

pub fn render_html(markdown: &str) -> String {
    let mut options = pulldown_cmark::Options::empty();
    options |= pulldown_cmark::Options::ENABLE_TABLES;
    options |= pulldown_cmark::Options::ENABLE_FOOTNOTES;
    options |= pulldown_cmark::Options::ENABLE_STRIKETHROUGH;
    options |= pulldown_cmark::Options::ENABLE_TASKLISTS;
    options |= pulldown_cmark::Options::ENABLE_SMART_PUNCTUATION;
    let parser = pulldown_cmark::Parser::new_ext(markdown, options);
    let mut out = String::with_capacity(markdown.len() + 256);
    pulldown_cmark::html::push_html(&mut out, parser);
    out
}

/// Rough markdown -> plain text, used for search snippets and word counts.
pub fn to_plain(markdown: &str) -> String {
    let mut out = String::with_capacity(markdown.len());
    let mut fence = false;
    for line in markdown.lines() {
        // `source_map.py:28` / `toc_engine.py:45` fence on `line.strip()`.
        let trimmed = py_strip_start(line);
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fence = !fence;
            continue;
        }
        if fence {
            continue;
        }
        let mut body = trimmed.to_string();
        if let Some(rest) = body.strip_prefix('#') {
            // `_extract_title`'s `line_s[2:].strip()` (link_indexer.py:185).
            body = py_strip_start(rest).to_string();
        }
        for token in ["**", "__", "`", "~~"] {
            body = body.replace(token, "");
        }
        out.push_str(&body);
        out.push('\n');
    }
    while out.ends_with("\n\n") {
        out.pop();
    }
    out
}

#[derive(Debug, Clone, PartialEq)]
pub struct Heading {
    pub level: u8,
    pub text: String,
    pub line: usize,
    pub id: String,
}

pub fn headings(markdown: &str) -> Vec<Heading> {
    let mut out = Vec::new();
    let mut used: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut fence = false;
    for (idx, line) in markdown.lines().enumerate() {
        // Py 45 `stripped = line.strip()` in `extract_headings`.
        let trimmed = py_strip_start(line);
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fence = !fence;
            continue;
        }
        if fence || !trimmed.starts_with('#') {
            continue;
        }
        let hashes = trimmed.len() - trimmed.trim_start_matches('#').len();
        if hashes > 6 {
            continue;
        }
        // `trim_start_matches('#')` mirrors an explicit `lstrip('#')` (character
        // class, not whitespace); the trailing strip is Py 62 `title = ...
        // .strip()`.
        let text = py_strip(trimmed.trim_start_matches('#')).to_string();
        if text.is_empty() {
            continue;
        }
        let base = slug(&text);
        let id = match used.get(&base) {
            Some(n) => {
                let n = *n + 1;
                used.insert(base.clone(), n);
                format!("{base}-{n}")
            }
            None => {
                used.insert(base.clone(), 0);
                base
            }
        };
        out.push(Heading { level: hashes as u8, text, line: idx + 1, id });
    }
    out
}

pub fn slug(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if c == ' ' || c == '-' || c == '_' {
            out.push('-');
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "section".to_string()
    } else {
        trimmed
    }
}

/// Minimal YAML front matter: `key: value` scalars plus block lists.
pub fn front_matter(markdown: &str) -> Value {
    let mut map = serde_json::Map::new();
    let mut lines = markdown.lines();
    // Py 1299 `lines[0].strip() == '---'` (texmd.py).
    if lines.next().map(|l| py_strip(l)) != Some("---") {
        return Value::Object(map);
    }
    for line in lines.by_ref() {
        // Py 1301/60: the authority strips the line ONCE (`lines[i].strip()`,
        // `k, v = line.split(':', 1)` after a `strip()`), and that single strip is
        // CPython's — a terminator whose only leading character is U+001C still
        // reads as `---`.
        let t = py_strip(line);
        if t == "---" || t == "..." {
            return Value::Object(map);
        }
        let trimmed = t;
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("- ") {
            map.entry("_list").or_insert_with(|| json!([]));
            if let Some(arr) = map.get_mut("_list").and_then(|v| v.as_array_mut()) {
                arr.push(json!(scalar(py_strip(rest))));
            }
            continue;
        }
        let Some((key, raw)) = trimmed.split_once(':') else {
            continue;
        };
        // Py 60/261 `k.strip()` / `key.strip()`.
        let key = py_strip(key).to_string();
        let raw = py_strip(raw);
        if raw.is_empty() {
            map.insert(key, Value::Null);
        } else if raw.starts_with('[') && raw.ends_with(']') {
            let inner = &raw[1..raw.len() - 1];
            let items: Vec<Value> = inner
                .split(',')
                .map(|p| scalar(py_strip(p)))
                .filter(|v| !v.is_null())
                .collect();
            map.insert(key, Value::Array(items));
        } else {
            map.insert(key, scalar(raw));
        }
    }
    Value::Object(map)
}

fn scalar(raw: &str) -> Value {
    // Py 61 `v.strip().strip('"\'')` — the first strip is CPython's.
    let t = py_strip(raw);
    if t.is_empty() {
        return Value::Null;
    }
    let unquoted = if (t.starts_with('"') && t.ends_with('"') && t.len() > 1)
        || (t.starts_with('\'') && t.ends_with('\'') && t.len() > 1)
    {
        &t[1..t.len() - 1]
    } else {
        t
    };
    if t.starts_with('"') || t.starts_with('\'') {
        return json!(unquoted);
    }
    match unquoted {
        "true" | "True" | "yes" => return json!(true),
        "false" | "False" | "no" => return json!(false),
        "null" | "~" | "Null" => return Value::Null,
        _ => {}
    }
    if let Ok(n) = unquoted.parse::<i64>() {
        return json!(n);
    }
    if let Ok(f) = unquoted.parse::<f64>() {
        if f.is_finite() {
            return json!(f);
        }
    }
    json!(unquoted)
}

/// `[[wikilink]]` and `[text](target)` targets, local files only.
pub fn extract_links(markdown: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let bytes: Vec<char> = markdown.chars().collect();
    let mut fence = false;
    let mut i = 0usize;
    while i < bytes.len() {
        let line_start = i;
        let mut line_end = i;
        while line_end < bytes.len() && bytes[line_end] != '\n' {
            line_end += 1;
        }
        let line: String = bytes[line_start..line_end].iter().collect();
        // Py `extract_links` masks fences with `_RE_FENCED_CODE`; the line-level
        // fence test here stands in for `stripped.startswith(...)`, whose
        // `strip()` is CPython's.
        let trimmed = py_strip_start(&line);
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fence = !fence;
            i = line_end + 1;
            continue;
        }
        if !fence {
            for target in wiki_targets(&line) {
                push_target(&mut out, &target);
            }
            for target in md_targets(&line) {
                push_target(&mut out, &target);
            }
        }
        i = line_end + 1;
    }
    out
}

fn wiki_targets(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(open) = rest.find("[[") {
        let tail = &rest[open + 2..];
        let Some(close) = tail.find("]]") else { break };
        let inner = &tail[..close];
        // Py 120 `target_part = target_part.strip()`.
        let target = py_strip(inner.split('|').next().unwrap_or(""));
        if !target.is_empty() {
            out.push(target.to_string());
        }
        rest = &tail[close + 2..];
    }
    out
}

fn md_targets(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(open) = rest.find("](") {
        let after = open + 2;
        if !rest[..open].contains('[') {
            rest = &rest[after..];
            continue;
        }
        let tail = &rest[after..];
        let Some(close) = tail.find(')') else { break };
        // Py 150/`mdcheck.py:90` keep the first token of the target (`rel.split(' ')[0]`
        // drops the optional title) and then `match.group(2).strip()`.  Python's
        // `\s` in `_RE_MD_LINK` includes U+001C..U+001F, so the token boundary has
        // to be CPython's too; `py_first_token` already yields a stripped token.
        let target = py_first_token(&tail[..close]);
        if !target.is_empty() {
            out.push(target.to_string());
        }
        rest = &tail[close + 1..];
    }
    out
}

fn push_target(out: &mut Vec<String>, raw: &str) {
    let target = raw.trim_matches('<').trim_end_matches('>');
    if target.is_empty() || target.starts_with('#') {
        return;
    }
    let lower = target.to_ascii_lowercase();
    if lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("mailto:")
        || lower.starts_with("data:")
        || lower.starts_with("tel:")
        || lower.starts_with("javascript:")
    {
        return;
    }
    let cleaned = target.split('#').next().unwrap_or(target);
    let cleaned = cleaned.split('?').next().unwrap_or(cleaned);
    if cleaned.is_empty() {
        return;
    }
    let decoded = percent_encoding::percent_decode_str(cleaned)
        .decode_utf8_lossy()
        .into_owned();
    if !out.contains(&decoded) {
        out.push(decoded);
    }
}

pub fn title_of(path: &Path, markdown: &str) -> String {
    if let Some(h) = headings(markdown).into_iter().next() {
        return h.text;
    }
    if let Some(map) = front_matter(markdown).as_object().cloned() {
        for key in ["title", "name"] {
            if let Some(v) = map.get(key).and_then(|v| v.as_str()) {
                // Py 64/`skills.py:98` `str(...).strip()` then a falsiness test.
                if !py_strip(v).is_empty() {
                    return py_strip(v).to_string();
                }
            }
        }
    }
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("untitled")
        .to_string()
}

pub struct MarkdownCheck {
    pub fixed_text: String,
    pub original: Option<String>,
    pub fixes: Vec<String>,
}

pub fn check_markdown_syntax(text: &str) -> MarkdownCheck {
    let mut fixes = Vec::new();
    if text.is_empty() {
        return MarkdownCheck {
            fixed_text: text.to_string(),
            original: None,
            fixes,
        };
    }

    let mut lines: Vec<String> = text.lines().map(|s| s.to_string()).collect();

    // 1. Unclosed code fences
    let mut open_fence_char: Option<char> = None;
    let mut open_fence_len = 0;
    let mut open_line = 0;

    for (i, line) in lines.iter().enumerate() {
        // `_FENCE_RE = ^(\s{0,3})(`{3,}|~{3,})\s*(\S*)\s*$` — every `\s`/`\S`
        // there is CPython's, so the indent this trim represents is too.
        let trimmed = py_strip_start(line);
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            let ch = trimmed.chars().next().unwrap();
            let count = trimmed.chars().take_while(|&c| c == ch).count();
            if let Some(fc) = open_fence_char {
                if fc == ch && count >= open_fence_len {
                    open_fence_char = None;
                    open_fence_len = 0;
                    open_line = 0;
                }
            } else {
                open_fence_char = Some(ch);
                open_fence_len = count;
                open_line = i + 1;
            }
        }
    }

    if let Some(_) = open_fence_char {
        lines.push("```".to_string());
        fixes.push(format!("代码围栏未闭合（从第 {} 行开始），已在文末补全", open_line));
    }

    // 2. Collapse more than 2 consecutive blank lines
    let mut new_lines = Vec::new();
    let mut blank_count = 0;
    for line in &lines {
        // Py `mdcheck.py:60` `if not line.strip():`
        if py_strip(line).is_empty() {
            blank_count += 1;
            if blank_count > 2 {
                continue;
            }
        } else {
            blank_count = 0;
        }
        new_lines.push(line.clone());
    }
    if new_lines.len() != lines.len() {
        fixes.push("连续空行过多，已折叠为至多 2 行".to_string());
        lines = new_lines;
    }

    // 3. Delimiter checks (warnings)
    let joined = lines.join("\n");
    let dd_count = joined.matches("$$").count();
    if dd_count % 2 != 0 {
        fixes.push("$$ 显示公式定界符数量为奇数，可能有一处公式未闭合".to_string());
    }

    if joined.contains('\u{FFFD}') {
        fixes.push("文本中包含替换符（\u{FFFD}），可能源文档编码或字体不支持".to_string());
    }

    let is_changed = joined != text;
    MarkdownCheck {
        original: if is_changed { Some(text.to_string()) } else { None },
        fixed_text: joined,
        fixes,
    }
}

/// Metadata + optional content for `/api/file`.
pub fn describe(app: &App, canonical: &Path, include_content: bool) -> Result<Value> {
    let meta = std::fs::metadata(canonical)?;
    let kind = kind_of(canonical);
    let display = app.paths.display_path(canonical);
    let mut value = json!({
        "ok": true,
        "path": display,
        "absPath": canonical.to_string_lossy(),
        "name": canonical.file_name().and_then(|n| n.to_str()).unwrap_or(""),
        "ext": ext_of(canonical),
        "dir": canonical.parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default(),
        "kind": kind,
        "size": meta.len() as i64,
        "mtime": modified_millis(canonical) as f64 / 1000.0,
        "mtimeMs": modified_millis(canonical),
        "writable": !meta.permissions().readonly(),
    });
    if !include_content && is_readable(canonical) {
        if let Ok(text) = read_text(canonical) {
            value["title"] = json!(title_of(canonical, &text));
        }
    }
    if include_content && is_readable(canonical) {
        let (text, encoding) = read_text_detect(canonical)?;
        // The editor sends this back with `/api/save` so the file keeps its
        // original encoding.
        value["encoding"] = json!(encoding);
        let is_txt = ext_of(canonical).eq_ignore_ascii_case("txt");
        if is_txt {
            // Plain text is shown structured (headings, lists, paragraphs
            // inferred), exactly like the converter's `.txt` lane; the editor
            // edits and saves the untouched original.
            let (md, _) = crate::convert::txt_to_markdown(&text);
            value["content"] = json!(md);
            value["original"] = json!(text);
            value["structured"] = json!(true);
            value["fixes"] = json!([]);
            value["words"] = json!(count_words(&text));
            value["title"] = json!(title_of(canonical, &md));
            value["headings"] = json!(headings(&md)
                .iter()
                .map(|h| json!({ "level": h.level, "text": h.text, "line": h.line, "id": h.id }))
                .collect::<Vec<_>>());
            value["frontmatter"] = front_matter(&md);
        } else if kind == "markdown" {
            let fix = crate::readmd_fix::fix_markdown(&text);
            let fixed_text = fix.text;
            let orig = if fixed_text == text { Value::Null } else { json!(text) };
            value["content"] = json!(fixed_text);
            value["original"] = orig;
            value["fixes"] = json!(fix.fixes);
            value["stats"] = fix.stats;
            value["words"] = json!(count_words(&fixed_text));
            value["title"] = json!(title_of(canonical, &fixed_text));
            value["headings"] = json!(headings(&fixed_text)
                .iter()
                .map(|h| json!({ "level": h.level, "text": h.text, "line": h.line, "id": h.id }))
                .collect::<Vec<_>>());
            value["frontmatter"] = front_matter(&fixed_text);
        } else {
            value["content"] = json!(text);
            value["original"] = Value::Null;
            value["fixes"] = json!([]);
            value["words"] = json!(count_words(&text));
            value["title"] = json!(title_of(canonical, &text));
            value["headings"] = json!(headings(&text)
                .iter()
                .map(|h| json!({ "level": h.level, "text": h.text, "line": h.line, "id": h.id }))
                .collect::<Vec<_>>());
            value["frontmatter"] = front_matter(&text);
        }
    } else if include_content {
        let ext = ext_of(canonical).to_ascii_lowercase();
        let is_convertible = matches!(
            ext.as_str(),
            "pdf" | "docx" | "doc" | "pptx" | "xlsx" | "epub" | "rtf" | "odt" | "tex" | "latex"
        );
        let mut converted_ok = false;
        if is_convertible {
            let path_str = canonical.to_string_lossy();
            if let Ok(res) = crate::convert::convert_verbose(&path_str, true) {
                // `readmd.py:3155` `if not text.strip():`
                if res.success
                    && res
                        .content
                        .as_ref()
                        .map(|c| !py_strip(c).is_empty())
                        .unwrap_or(false)
                {
                    let text = res.content.unwrap();
                    value["content"] = json!(text);
                    value["original"] = json!(text);
                    value["fixes"] = json!([]);
                    value["stats"] = json!({});
                    value["words"] = json!(count_words(&text));
                    value["title"] = json!(title_of(canonical, &text));
                    value["headings"] = json!(headings(&text)
                        .iter()
                        .map(|h| json!({ "level": h.level, "text": h.text, "line": h.line, "id": h.id }))
                        .collect::<Vec<_>>());
                    value["frontmatter"] = front_matter(&text);
                    value["is_markdown"] = json!(true);
                    value["converted"] = json!(true);
                    value["source"] = json!("convert");
                    value["binary"] = json!(false);
                    converted_ok = true;
                }
            }
        }
        if !converted_ok {
            value["content"] = Value::Null;
            value["binary"] = json!(true);
        }
    }
    Ok(value)
}

/// Write + reindex. Returns the same payload shape as [`describe`].
pub fn save(app: &App, canonical: &Path, content: &str) -> Result<Value> {
    write_text_atomic(canonical, content)?;
    index(app, canonical)?;
    describe(app, canonical, false)
}

/// (Re)index an existing file into the derived store.
pub fn index(app: &App, canonical: &Path) -> Result<i64> {
    let display = app.paths.display_path(canonical);
    let kind = kind_of(canonical);
    let meta = std::fs::metadata(canonical).ok();
    let size = meta.as_ref().map(|m| m.len() as i64).unwrap_or(0);
    let mtime = modified_millis(canonical);
    if matches!(kind, "markdown" | "code") {
        let text = read_text(canonical).unwrap_or_default();
        let title = title_of(canonical, &text);
        let id = app.store.upsert_document(
            &display,
            &title,
            mtime,
            size,
            count_words(&text),
            kind,
            &text,
        )?;
        let targets = resolve_link_targets(app, canonical, &extract_links(&text));
        app.store.replace_links(&display, &targets)?;
        Ok(id)
    } else {
        Ok(0)
    }
}

/// Resolve relative link targets into workspace display paths (indexed or not).
pub fn resolve_link_targets(app: &App, source: &Path, targets: &[String]) -> Vec<String> {
    let base = source.parent().unwrap_or(Path::new("."));
    targets
        .iter()
        .filter_map(|t| {
            let candidate = if Path::new(t).is_absolute() {
                PathBuf::from(t)
            } else {
                base.join(t)
            };
            let canonical = paths::canonical_existing(&candidate).ok()?;
            Some(app.paths.display_path(&canonical))
        })
        .collect()
}

/// Full workspace rescan used by `/api/links/index`.
pub fn reindex_workspace(app: &App, limit: usize) -> Result<Value> {
    let mut indexed = 0usize;
    let mut skipped = 0usize;
    let mut errors = Vec::new();
    let walker = walkdir::WalkDir::new(&app.paths.workspace)
        .max_depth(MAX_TREE_DEPTH)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !(name.starts_with('.') || name == "node_modules" || name == "__pycache__" || name == "target" || name == "dist")
        });
    for entry in walker.flatten() {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if !is_readable(path) {
            skipped += 1;
            continue;
        }
        if app.paths.check_allowed(&path.to_path_buf()).is_err() {
            skipped += 1;
            continue;
        }
        if indexed >= limit {
            break;
        }
        match index(app, path) {
            Ok(_) => indexed += 1,
            Err(e) => {
                if errors.len() < 20 {
                    errors.push(json!({
                        "path": app.paths.display_path(path),
                        "error": e.to_string(),
                    }));
                }
            }
        }
    }
    Ok(json!({
        "ok": true,
        "indexed": indexed,
        "skipped": skipped,
        "errors": errors,
        "stats": app.store.stats().unwrap_or_else(|_| json!(null)),
    }))
}

/// Find a document by (possibly extension-less) name inside the workspace.
pub fn find_by_name(app: &App, name: &str) -> Option<PathBuf> {
    // Client-supplied document name: every such parameter is `.strip()`ed in
    // the authority (`readmd.py:2104/2120` `qs.get(...)[0].strip()`).  The
    // `trim_start_matches("./")` is an explicit prefix, not whitespace.
    let needle = py_strip(name).trim_start_matches("./").replace('\\', "/");
    if needle.is_empty() {
        return None;
    }
    let want_exact = needle.contains('/') || Path::new(&needle).extension().is_some();
    let stem = Path::new(&needle)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(&needle)
        .to_ascii_lowercase();
    let mut fallback: Option<PathBuf> = None;
    let walker = walkdir::WalkDir::new(&app.paths.workspace)
        .max_depth(6)
        .into_iter()
        .filter_entry(|e| {
            let n = e.file_name().to_string_lossy();
            !(n.starts_with('.') || n == "node_modules")
        });
    for entry in walker.flatten() {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let file_name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if want_exact {
            if file_name == stem || file_name == needle.to_ascii_lowercase() {
                return Some(path.to_path_buf());
            }
            continue;
        }
        if file_name == format!("{stem}.md") || file_name == format!("{stem}.markdown") {
            return Some(path.to_path_buf());
        }
        if file_name.starts_with(&stem) && fallback.is_none() {
            fallback = Some(path.to_path_buf());
        }
    }
    fallback
}

/// `dir/name.ext` with ` (2)`, ` (3)`.. suffixes until free.
pub fn unique_path(dir: &Path, name: &str, ext: &str) -> PathBuf {
    let stem = Path::new(name)
        .file_stem()
        .and_then(|s| s.to_str())
        // `dialogs.py:64` `stem = stem.strip() or 'untitled'` — CPython's strip,
        // and the *stripped* stem is what the filename is built from (measured:
        // format_save_filename('\x1ckeep name\x1d', 'md') == 'keep name.md').
        .map(py_strip)
        .filter(|s| !s.is_empty())
        .unwrap_or("untitled");
    let clean_ext = ext.trim_start_matches('.').to_ascii_lowercase();
    let file_name = if clean_ext.is_empty() {
        stem.to_string()
    } else {
        format!("{stem}.{clean_ext}")
    };
    let candidate = dir.join(&file_name);
    if !candidate.exists() {
        return candidate;
    }
    for n in 2..1000 {
        let alt = if clean_ext.is_empty() {
            format!("{stem} ({n})")
        } else {
            format!("{stem} ({n}).{clean_ext}")
        };
        let candidate = dir.join(alt);
        if !candidate.exists() {
            return candidate;
        }
    }
    dir.join(file_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tempdir(tag: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("readmd-content-{tag}-{nanos}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn app_in(dir: &Path) -> App {
        let paths = paths::AppPaths::with_dirs(&dir.join("data"), dir, &dir.join("assets"));
        App::bootstrap(paths).unwrap()
    }

    #[test]
    fn kinds_classify_by_extension() {
        assert_eq!(kind_of(Path::new("a/b.md")), "markdown");
        assert_eq!(kind_of(Path::new("a/b.rs")), "code");
        assert_eq!(kind_of(Path::new("a/b.PNG")), "image");
        assert_eq!(kind_of(Path::new("a/b.pdf")), "binary");
        assert_eq!(kind_of(Path::new("Makefile")), "code");
    }

    #[test]
    fn bom_and_atomic_write_roundtrip() {
        let dir = tempdir("bom");
        let file = dir.join("x.md");
        write_bytes_atomic(&file, &[0xef, 0xbb, 0xbf, b'#', b' ', b'H', b'i']).unwrap();
        assert_eq!(read_text(&file).unwrap(), "# Hi");
        write_text_atomic(&file, "# Hi\n\nbody").unwrap();
        assert!(!file.with_file_name(".x.md.tmp").exists());
        assert_eq!(read_text(&file).unwrap(), "# Hi\n\nbody");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn word_count_handles_mixed_scripts() {
        assert_eq!(count_words("hello world"), 2);
        assert_eq!(count_words("你好世界"), 4);
        assert_eq!(count_words("hello 世界 ok"), 4);
    }

    #[test]
    fn headings_skip_fenced_code_and_dedupe_ids() {
        let md = "# Title\n```\n# not a heading\n```\n## Title\n";
        let got = headings(md);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].id, "title");
        assert_eq!(got[1].id, "title-1");
    }

    #[test]
    fn front_matter_parses_scalars_and_inline_lists() {
        let md = "---\ntitle: Hello World\ndraft: false\ntags: [a, b]\n---\n\n# Body\n";
        let fm = front_matter(md);
        assert_eq!(fm["title"], json!("Hello World"));
        assert_eq!(fm["draft"], json!(false));
        assert_eq!(fm["tags"].as_array().map(|a| a.len()), Some(2));
    }

    #[test]
    fn extract_links_keeps_local_targets_only() {
        let md = "see [[Note One]] and [link](other.md) plus [x](https://e.com/y) ![img](i.png)\n";
        let links = extract_links(md);
        assert!(links.contains(&"other.md".to_string()));
        assert!(links.contains(&"Note One".to_string()));
        assert!(!links.iter().any(|l| l.starts_with("https")));
    }

    #[test]
    fn render_html_emits_tables_and_strikethrough() {
        let html = render_html("a|b\n--|--\n1|2\n\n~~gone~~\n");
        assert!(html.contains("<table>"));
        assert!(html.contains("<del>gone</del>"));
    }

    #[test]
    fn save_indexes_and_links_documents() {
        let dir = tempdir("save");
        let app = app_in(&dir);
        let other = dir.join("other.md");
        write_text_atomic(&other, "# Other\n").unwrap();
        let note = dir.join("note.md");
        let payload = save(&app, &note, "# Note\n\n[other](other.md) 你好世界\n").unwrap();
        assert_eq!(payload["title"], json!("Note"));
        let hits = app.store.search("你好", 10).unwrap();
        assert!(hits.iter().any(|h| h.path.ends_with("note.md")));
        let back = app.store.backlinks(&app.paths.display_path(&other)).unwrap();
        assert_eq!(back.len(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn list_dir_and_tree_cover_a_workspace() {
        let dir = tempdir("list");
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        write_text_atomic(&dir.join("sub/a.md"), "# A\n").unwrap();
        write_text_atomic(&dir.join("top.md"), "# T\n").unwrap();
        let listed = list_dir(&dir).unwrap();
        assert_eq!(listed["count"].as_i64(), Some(2));
        assert_eq!(listed["entries"][0]["name"], json!("sub"));
        let tree = tree(&dir);
        assert_eq!(tree["children"].as_array().map(|a| a.len()), Some(2));
    }

    #[test]
    fn find_by_name_resolves_extensionless() {
        let dir = tempdir("find");
        let app = app_in(&dir);
        write_text_atomic(&dir.join("Deep One.md"), "# Deep\n").unwrap();
        let found = find_by_name(&app, "deep one").unwrap();
        assert!(found.ends_with("Deep One.md"));
    }

    #[test]
    fn unique_path_avoids_collisions() {
        let dir = tempdir("unique");
        write_text_atomic(&dir.join("a.md"), "").unwrap();
        let next = unique_path(&dir, "a", "md");
        assert_eq!(next.file_name().unwrap().to_str().unwrap(), "a (2).md");
    }
}

// ===========================================================================
// W-E9 whitespace-parity tests.  Every expectation is the CPython answer
// produced by the mirrors in scratch/rust_parity/we9_expect.py, whose
// whitespace calls are the authorities' own (`line.strip()` in
// readmd_core/source_map.py:28 / toc_engine.py:45,62, `if not line.strip()` in
// mdcheck.py:60, `target_part.strip()` in link_indexer.py:120,
// `stem.strip() or 'untitled'` in dialogs.py:64, `encoding or "utf-8"` in
// file_writer.py:55).
// ===========================================================================
#[cfg(test)]
mod we9_tests {
    use super::*;
    use std::path::Path;
    use std::panic::catch_unwind;

    const WS: [&str; 10] = [
        "\u{1c}", "\u{1d}", "\u{1e}", "\u{1f}", "\u{a0}", "\u{3000}", "\u{2028}",
        "\t", "\r", "",
    ];

    #[test]
    fn we9_helper_set_is_cpython_exact() {
        for c in ['\u{1c}', '\u{1d}', '\u{1e}', '\u{1f}', '\u{a0}', '\u{3000}',
                  '\u{2028}', '\u{85}', '\u{b}', '\t', '\r', '\n', ' '] {
            assert!(py_isspace(c));
        }
        for c in ['\u{1a}', '\u{1b}', 'x'] {
            assert!(!py_isspace(c));
        }
        assert_eq!(py_strip("\u{1c}foo\u{1d}"), "foo");
        assert_eq!(py_strip(""), "");
        assert_eq!(py_strip_start("\u{1c} \u{1d}x"), "x");
        // Py `s.split()[0] if s.split() else ''`
        assert_eq!(py_first_token("b\u{1c}c"), "b");
        assert_eq!(py_first_token("\u{a0}\u{3000}path.md"), "path.md");
        assert_eq!(py_first_token("  "), "");
        assert_eq!(py_first_token("\u{1c}"), "");
        assert_eq!(py_first_token(""), "");
    }

    #[test]
    fn we9_to_plain_fence_uses_cpython_strip() {
        // measured mirrors: a separator-indented fence really opens, so the body
        // between the fences disappears from the plain text
        assert_eq!(to_plain("\u{1c}```python\nprint(1)\n```\n# T\n"), "T\n");
        assert_eq!(to_plain("```a\nx\n\u{1d}```\nkeep\n"), "keep\n");
        assert_eq!(to_plain("\u{1f}```z\nq\n```\nx"), "x\n");
        // Every line is fence or fence body -> nothing survives.  (`''` rather
        // than the mirror's `'\n'`: `str::lines()` drops the trailing empty
        // field a Python `split('\n')` keeps — recorded in fixreport-wE9.)
        assert_eq!(to_plain("\u{a0}```f\nhidden\n```\n"), "");
        assert_eq!(to_plain("\t# Heading\u{1e}\nbody\n"), "Heading\u{1e}\nbody\n");
        // No line survives a fenced document, and `str::lines()` never yields the
        // trailing empty field a Python `split('\n')` would (that lines-vs-
        // splitlines divergence is reported, not converted, in fixreport-wE9).
        assert_eq!(to_plain("```a\n# not a heading\n```\n"), "");
        assert_eq!(to_plain(""), "");
        assert_eq!(to_plain("# \u{1c}T\u{1e}\n\nbody\n"), "T\u{1e}\n\nbody\n");
    }

    #[test]
    fn we9_headings_strip_and_fence() {
        let h = |md: &str| -> Vec<(u8, String, usize)> {
            headings(md).into_iter().map(|x| (x.level, x.text, x.line)).collect()
        };
        // measured: `# One` and (separator-indented) `# Two`, the fenced `# Three`
        // is masked because `\x1c```` ... no: here the plain fence masks it
        assert_eq!(h("# One\n\u{1c}# Two\n```\n# Three\n```\n"),
                   vec![(1, "One".into(), 1), (1, "Two".into(), 2)]);
        assert_eq!(h("\u{1e}#\u{1f}\n## Sub \u{1d}\n##\u{1c}\n"), vec![(2, "Sub".into(), 2)]);
        assert_eq!(h("   \u{1c}#### Four   \u{1c}\n"), vec![(4, "Four".into(), 1)]);
        assert_eq!(h(""), Vec::new());
        assert_eq!(h("# \u{a0}NBSP\u{a0}\n"), vec![(1, "NBSP".into(), 1)]);
        assert_eq!(h("###### six\n####### seven\n"), vec![(6, "six".into(), 1)]);
        assert_eq!(h("# T\r\u{1c}# U\n"), vec![(1, "T\r\u{1c}# U".into(), 1)]);
        // the separator-indented fence masks the heading after it
        assert_eq!(h("#### \u{1e}Deep\u{1f}\n\u{1c}```\n# hidden\n```\n"),
                   vec![(4, "Deep".into(), 1)]);
        // slug is a character-class trim ('-'), never a whitespace trim: unchanged
        assert_eq!(slug("--a b--"), "a-b");
    }

    #[test]
    fn we9_front_matter_uses_cpython_strip() {
        let fm = front_matter("---\ntitle:\u{1c}Hello\u{1d}\n---\nbody\n");
        assert_eq!(fm.get("title").and_then(|v| v.as_str()), Some("Hello"));
        let fm2 = front_matter("---\n\u{1c}keys:  v \u{1f}\n---\n");
        assert_eq!(fm2.get("keys").and_then(|v| v.as_str()), Some("v"));
        // measured: a terminator whose only leading character is a separator still
        // closes the block, so nothing after it is parsed
        let fm3 = front_matter("---\n\u{1c}---\nleak: 1\n");
        assert_eq!(fm3.get("leak"), None);
        assert_eq!(fm3.as_object().map(|m| m.len()), Some(0));
        // measured: the opening delimiter may itself be separator-padded
        let fm4 = front_matter("\u{1c}---\nt: 1\n---\n");
        assert_eq!(fm4.get("t").and_then(|v| v.as_i64()), Some(1));
        // measured: block list and inline array
        let fm5 = front_matter("---\nlist:\n  - \u{1c}a\n  - b\u{1e}\n---\n");
        assert_eq!(fm5.get("_list"), Some(&json!(["a", "b"])));
        let fm6 = front_matter("---\narr: [\u{1c}a, b ,\u{1d}]\n---\n");
        assert_eq!(fm6.get("arr"), Some(&json!(["a", "b"])));
        // absent-key vs null vs "" are three states: `n:` is null
        let fm7 = front_matter("---\nn:\n---\n");
        assert_eq!(fm7.get("n"), Some(&Value::Null));
        // measured scalars
        let fm8 = front_matter("---\nq: \"x\"\ns: 3.5\nb: yes\nz: null\n---\n");
        assert_eq!(fm8.get("q").and_then(|v| v.as_str()), Some("x"));
        assert_eq!(fm8.get("s").and_then(|v| v.as_f64()), Some(3.5));
        assert_eq!(fm8.get("b"), Some(&json!(true)));
        assert_eq!(fm8.get("z"), Some(&Value::Null));
        // no front matter at all
        assert!(front_matter("plain\n").as_object().map(|m| m.is_empty()).unwrap_or(false));
        assert!(front_matter("").as_object().map(|m| m.is_empty()).unwrap_or(false));
    }

    #[test]
    fn we9_extract_links_uses_cpython_strip_and_split() {
        // measured: [[a\x1cb]] keeps the INTERIOR separator, [[\x1cnote]] loses the
        // leading one, and `( \x1cfoo.md)` is the first `split()` token
        assert_eq!(extract_links("[[a\u{1c}b]]\n[[\u{1c}note]]\n[x](\u{1c}foo.md)\n[y](bar.md \u{1d} \"t\")\n"),
                   vec!["a\u{1c}b", "note", "foo.md", "bar.md"]);
        // measured: a separator-indented fence masks the block after it
        assert_eq!(extract_links("[[a]]\n\u{1c}```\n[[b]]\n```\n[[c]]"),
                   vec!["a", "c"]);
        assert_eq!(extract_links("\u{1d}```lang\n[[x]]\n```\n[[y]]"), vec!["y"]);
        assert_eq!(extract_links("\u{1c}[[in-fence]]\n```\n[[masked]]\n```\n[[after]]\n"),
                   vec!["in-fence", "after"]);
        // measured: all-blank targets yield nothing
        assert_eq!(extract_links("[a](\u{1c})\n[b](  )\n"), Vec::<String>::new());
        assert_eq!(extract_links("[[]]\n[[|\u{1c}]]\n"), Vec::<String>::new());
        assert_eq!(extract_links(""), Vec::<String>::new());
        // measured: NBSP / ideographic space lead a token
        assert_eq!(extract_links("[t](\u{a0}path.md)\n"), vec!["path.md"]);
        assert_eq!(extract_links("[c](\u{1e}b.md)\n"), vec!["b.md"]);
        // `<` / `>` wrapper is a character class, not whitespace
        assert_eq!(extract_links("[d](<q.md>)"), vec!["q.md"]);
    }

    #[test]
    fn we9_check_markdown_blank_collapse() {
        // measured (mdcheck.py:57-69 `if not line.strip()`)
        let c = check_markdown_syntax("a\n\u{1c}\n\u{1d}\n\u{1e}\n\u{1f}\nb");
        assert_eq!(c.fixed_text, "a\n\u{1c}\n\u{1d}\nb");
        assert_eq!(c.original.as_deref(), Some("a\n\u{1c}\n\u{1d}\n\u{1e}\n\u{1f}\nb"));
        let c2 = check_markdown_syntax("x\n \n\t\n\r\ny");
        assert_eq!(c2.fixed_text, "x\n \n\t\ny");
        let c3 = check_markdown_syntax("a\n\u{a0}\n\u{3000}\n\u{2028}\nb");
        assert_eq!(c3.fixed_text, "a\n\u{a0}\n\u{3000}\nb");
        // measured: a single separator-only line is one blank, nothing to collapse
        let c4 = check_markdown_syntax("\u{1c}\u{1d}\u{1e}\u{1f}");
        assert_eq!(c4.fixed_text, "\u{1c}\u{1d}\u{1e}\u{1f}");
        assert!(c4.original.is_none());
        let c5 = check_markdown_syntax("");
        assert_eq!(c5.fixed_text, "");
        assert!(c5.original.is_none());
        // four real blanks collapse to two
        assert_eq!(check_markdown_syntax("\n\n\n\n").fixed_text, "\n");
    }

    #[test]
    fn we9_title_of_prefers_heading_then_front_matter() {
        let p = Path::new("C:/tmp/doc.md");
        // measured: heading text is CPython-stripped
        assert_eq!(title_of(p, "\u{1c}# \u{1e}Real Title\u{1f}\n"), "Real Title");
        assert_eq!(title_of(p, "---\ntitle: \u{1c}FM Title\u{1d}\n---\n"), "FM Title");
        // a whitespace-only front-matter title is falsy -> the file stem wins
        assert_eq!(title_of(p, "---\ntitle: \u{1c}\u{1d}\n---\n"), "doc");
        assert_eq!(title_of(p, "\u{1c}\u{1d}\n"), "doc");
        assert_eq!(title_of(p, "# \u{a0}NBSP Title\u{3000}\n"), "NBSP Title");
    }

    #[test]
    fn we9_unique_path_blank_stem_is_untitled() {
        // dialogs.py:64 `stem.strip() or 'untitled'`
        let dir = Path::new("C:/readmd-we9-nonexistent-dir");
        for w in ["\u{1c}", "\u{1c}\u{1d}\u{1e}\u{1f}", "\u{a0}", "\u{3000}", " ", "\t", ""] {
            let got = unique_path(dir, w, "md");
            assert_eq!(
                got.file_name().and_then(|s| s.to_str()),
                Some("untitled.md"),
                "stem {:?} must fall back", w);
        }
        // measured: dialogs.format_save_filename('\x1ckeep name\x1d','md') ==
        // 'keep name.md' — the separators are eaten, the name is not 'untitled'
        assert_eq!(unique_path(dir, "\u{1c}keep name\u{1d}", "md").file_name().unwrap().to_str(),
                   Some("keep name.md"));
        // a real stem survives, only its CPython-whitespace edges are eaten
        assert_eq!(unique_path(dir, "Report", "md").file_name().unwrap().to_str(), Some("Report.md"));
    }

    #[test]
    fn we9_matrix_never_panics() {
        let mut runs = 0usize;
        for w in WS.iter() {
            for md in [
                format!("{0}# T{0}\n```{0}\ncode\n{0}```\n[[x{0}y]](a{0}b.md)\n", w),
                format!("---\ntitle:{0}v{0}\n{0}---\nafter: 1\n", w),
                format!("a\n{0}\n{0}\n{0}\n{0}\nb", w),
                format!("[t](<{0}{1}{0}>)\n", w, "p.md"),
                format!("{0}", w),
            ] {
                runs += 1;
                let m = md.clone();
                assert!(catch_unwind(move || to_plain(&m)).is_ok(), "to_plain {:?}", md);
                let m = md.clone();
                assert!(catch_unwind(move || headings(&m)).is_ok(), "headings {:?}", md);
                let m = md.clone();
                assert!(catch_unwind(move || front_matter(&m)).is_ok(), "front_matter {:?}", md);
                let m = md.clone();
                assert!(catch_unwind(move || extract_links(&m)).is_ok(), "extract_links {:?}", md);
                let m = md.clone();
                assert!(catch_unwind(move || check_markdown_syntax(&m)).is_ok(), "check {:?}", md);
                let m = md.clone();
                assert!(catch_unwind(move || title_of(Path::new("d.md"), &m)).is_ok(), "title {:?}", md);
                let m = md.clone();
                assert!(catch_unwind(move || count_words(&m)).is_ok(), "words {:?}", md);
            }
        }
        assert_eq!(runs, 50);
    }
}
