//! ReadMD Import Processor - document modularisation and `@import` resolver.
//!
//! Rust port of `src/readmd_modules/import_processor.py`.  Supported syntaxes:
//! 1. sub-markdown chapter embedding: `@import "sub_chapter.md"`
//! 2. CSV / TSV data tables rendered as Markdown: `@import "dataset.csv"`
//! 3. source-file line-range slicing: `@import "app.py" {line_begin=10 line_end=30 lang=py}`
//! 4. diagram / vector sources: `.puml` `.plantuml` `.dot` `.viz` `.wavedrom` `.tikz` `.less`
//! 5. PDF page embedding: `.pdf` (see `PDF_RENDER_UNAVAILABLE` below)
//!
//! Safety rails, all mirrored from Python: document-root containment after path
//! resolution, circular-reference detection, the `MAX_IMPORT_DEPTH` recursion cap
//! and a cumulative read/output byte budget.
//!
//! The authoritative `@import` regex is applied **only outside Markdown code
//! fences** (`_process_outside_fences` in Python, [`process_outside_fences`] here).

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::codecs;
use crate::diagrams::{format_tikz_html, py_strip};

/// Python `MAX_IMPORT_DEPTH`.
pub const MAX_IMPORT_DEPTH: usize = 8;

/// Python `import_processor.py:284` - emitted whenever the cumulative budget has
/// already been blown, and also used as the truncation trailer.
const MSG_BUDGET_EXCEEDED: &str = "\n> **[ReadMD 错误]**: import_budget_exceeded\n";

/// The `.pdf` branch of Python renders a page to PNG with PyMuPDF (`fitz`) and
/// embeds a `data:image/png;base64,…` URL.  The Rust kernel has no PDF
/// rasterizer (`pdf-extract` only yields text), so the documented failure text
/// is emitted instead, exactly like Python does when `import fitz` fails.
const PDF_RENDER_UNAVAILABLE: &str =
    "PDF 页面渲染不可用 (fitz/PyMuPDF has no Rust equivalent in the kernel)";

/// A hard failure that aborts the whole request.  Python has exactly one such
/// path (`os.path.getsize` raising out of `replace_import`); everything else is
/// reported *inline* in the produced markdown, which is why the message table is
/// user-visible output and must stay byte-identical.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportError {
    pub code: String,
    pub error_code: ImportErrorCode,
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.error_code, self.code)
    }
}

impl std::error::Error for ImportError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImportErrorCode {
    #[serde(rename = "import_path_traversal")]
    ImportPathTraversal,
    #[serde(rename = "import_circular_reference")]
    ImportCircularReference,
    #[serde(rename = "import_file_not_found")]
    ImportFileNotFound,
    #[serde(rename = "import_depth_exceeded")]
    ImportDepthExceeded,
    #[serde(rename = "import_budget_exceeded")]
    ImportBudgetExceeded,
    #[serde(rename = "import_unsupported_encoding")]
    ImportUnsupportedEncoding,
    #[serde(rename = "invalid_line_range")]
    InvalidLineRange,
}

lazy_static! {
    /// Python `IMPORT_PATTERN` (`re.MULTILINE`).
    static ref IMPORT_PATTERN: Regex = Regex::new(
        r#"(?m)^[\t ]*@import\s+["']([^"']+)["'](?:\s*\{([^}]*)\})?[\t ]*\r?$"#
    )
    .expect("valid @import pattern");

    /// Python `parse_attributes` `token_pattern`.
    static ref ATTR_TOKEN_PATTERN: Regex = Regex::new(
        r#"([a-zA-Z_][a-zA-Z0-9_]*)\s*=\s*(\[[^\]]*\]|"[^"]*"|'[^']*'|[^\s,]+)"#
    )
    .expect("valid attribute token pattern");

    /// Python `` re.finditer(r'`+', …) `` - a maximal run of backticks.
    static ref BACKTICK_RUN: Regex = Regex::new(r"`+").expect("valid backtick run pattern");
}

// --------------------------------------------------------------------- budget

#[derive(Debug, Clone)]
pub struct ImportBudget {
    max_output_bytes: Option<usize>,
    max_read_bytes: Option<usize>,
    total_output_bytes: usize,
    total_read_bytes: usize,
    exceeded: bool,
}

impl ImportBudget {
    pub fn new(max_output_bytes: Option<usize>, max_read_bytes: Option<usize>) -> Self {
        Self {
            max_output_bytes,
            max_read_bytes,
            total_output_bytes: 0,
            total_read_bytes: 0,
            exceeded: false,
        }
    }

    fn can_read(&mut self, n_bytes: usize) -> bool {
        if self.exceeded {
            return false;
        }
        if let Some(limit) = self.max_read_bytes {
            if self.total_read_bytes + n_bytes > limit {
                self.exceeded = true;
                return false;
            }
        }
        true
    }

    fn record_read(&mut self, n_bytes: usize) {
        self.total_read_bytes += n_bytes;
    }

    fn can_output(&mut self, n_bytes: usize) -> bool {
        if self.exceeded {
            return false;
        }
        if let Some(limit) = self.max_output_bytes {
            if self.total_output_bytes + n_bytes > limit {
                self.exceeded = true;
                return false;
            }
        }
        true
    }

    fn record_output(&mut self, n_bytes: usize) {
        self.total_output_bytes += n_bytes;
    }
}

// ------------------------------------------------------------------ attributes

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttributeValue {
    String(String),
    Int(i64),
    Bool(bool),
    Array(Vec<AttributeValue>),
}

impl AttributeValue {
    /// Python `str(value)` as it would land inside an f-string.
    fn display(&self) -> String {
        match self {
            AttributeValue::String(s) => s.clone(),
            AttributeValue::Int(i) => i.to_string(),
            AttributeValue::Bool(b) => {
                if *b {
                    "True".to_string()
                } else {
                    "False".to_string()
                }
            }
            AttributeValue::Array(items) => {
                let inner: Vec<String> = items.iter().map(|v| v.display()).collect();
                format!("[{}]", inner.join(", "))
            }
        }
    }

    /// Python truthiness of a parsed attribute value.
    fn is_truthy(&self) -> bool {
        match self {
            AttributeValue::String(s) => !s.is_empty(),
            AttributeValue::Int(i) => *i != 0,
            AttributeValue::Bool(b) => *b,
            AttributeValue::Array(items) => !items.is_empty(),
        }
    }

    /// Python `_parse_positive_int` (returns `None` for bools, arrays, non-digit
    /// strings and non-positive integers).
    fn parse_positive_int(&self) -> Option<i64> {
        match self {
            AttributeValue::Int(i) => {
                if i > &0 {
                    Some(*i)
                } else {
                    None
                }
            }
            AttributeValue::Bool(_) => None,
            AttributeValue::Array(_) => None,
            AttributeValue::String(s) => {
                let s = py_strip(s);
                if is_ascii_digits(s) {
                    s.parse::<i64>().ok().filter(|v| v > &0)
                } else {
                    None
                }
            }
        }
    }
}

fn is_ascii_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit())
}

/// Python `parse_attributes`: `key=val`, `key=[…]`, `key="val"`.
pub fn parse_attributes(attr_str: &str) -> HashMap<String, AttributeValue> {
    let mut attrs: HashMap<String, AttributeValue> = HashMap::new();
    let attr_str = py_strip(attr_str);
    if attr_str.is_empty() {
        return attrs;
    }

    for cap in ATTR_TOKEN_PATTERN.captures_iter(attr_str) {
        // Group 1/2 always participate (both are required by the pattern).
        let k = cap[1].to_lowercase();
        let v = py_strip(&cap[2]);

        if v.len() >= 2 && v.starts_with('[') && v.ends_with(']') {
            // Python parses `[15, 18]` but keeps every non-digit item *verbatim*
            // (array items never have their quotes stripped).
            let items: Vec<AttributeValue> = v[1..v.len() - 1]
                .split(',')
                .map(|x| {
                    let s = py_strip(x);
                    match s.parse::<i64>() {
                        Ok(n) if is_ascii_digits(s) => AttributeValue::Int(n),
                        _ => AttributeValue::String(s.to_string()),
                    }
                })
                .collect();
            attrs.insert(k, AttributeValue::Array(items));
        } else if (v.starts_with('"') && v.ends_with('"'))
            || (v.starts_with('\'') && v.ends_with('\''))
        {
            // Python `v[1:-1]` on the single-character value `"` yields "".
            let inner = if v.len() >= 2 { &v[1..v.len() - 1] } else { "" };
            attrs.insert(k, AttributeValue::String(inner.to_string()));
        } else if v.to_lowercase() == "true" {
            attrs.insert(k, AttributeValue::Bool(true));
        } else if v.to_lowercase() == "false" {
            attrs.insert(k, AttributeValue::Bool(false));
        } else if is_ascii_digits(v) {
            let value = match v.parse::<i64>() {
                Ok(n) => AttributeValue::Int(n),
                Err(_) => AttributeValue::String(v.to_string()),
            };
            attrs.insert(k, value);
        } else {
            attrs.insert(k, AttributeValue::String(v.to_string()));
        }
    }

    attrs
}

// ------------------------------------------------------------- code-fence walk

/// Byte length of the CPython line terminator that starts at `b[i]`, or `None`.
///
/// `str.splitlines()` does *not* break on `\n`/`\r\n`/`\r` alone.  CPython
/// (`Objects/stringlib/ucsput.h`'s `IS_LINE_SEPARAATOR`, reached through
/// `Py_UNICODE_ISLINEBREAK`) breaks on eleven sequences: `\n` `\r` `\r\n`
/// `\v`(0x0B) `\f`(0x0C) `\x1c` `\x1d` `\x1e` `\x85`(NEL) `\u2028`(LS) and
/// `\u2029`(PS).  The three non-ASCII ones are matched as whole UTF-8
/// sequences (C2 85 / E2 80 A8 / E2 80 A9), so the byte scan can never split a
/// character: a lone 0x85 or 0xA8 is a continuation byte (0x80-0xBF) and never
/// appears where a lead byte is required.
fn line_break_len(b: &[u8], i: usize) -> Option<usize> {
    match b[i] {
        b'\n' | b'\r' | 0x0b | 0x0c | 0x1c | 0x1d | 0x1e => {
            if b[i] == b'\r' && i + 1 < b.len() && b[i + 1] == b'\n' {
                Some(2)
            } else {
                Some(1)
            }
        }
        0xc2 if i + 1 < b.len() && b[i + 1] == 0x85 => Some(2),
        0xe2 if i + 2 < b.len() && b[i + 1] == 0x80 && matches!(b[i + 2], 0xa8 | 0xa9) => Some(3),
        _ => None,
    }
}

/// One scan serves both `keepends` settings so the two can never drift.
/// A terminator at the very end contributes no extra line, exactly like CPython:
/// `'a\n'.splitlines(True) == ['a\n']` and `'a\n\n'.splitlines(True) == ['a\n', '\n']`.
fn split_lines(text: &str, keepends: bool) -> Vec<&str> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    while i < b.len() {
        if let Some(n) = line_break_len(b, i) {
            let stop = if keepends { i + n } else { i };
            out.push(&text[start..stop]);
            start = i + n;
            i = start;
        } else {
            i += 1;
        }
    }
    if start < text.len() {
        out.push(&text[start..]);
    }
    out
}

/// Python `str.splitlines(keepends=True)` (`import_processor.py:73`).
fn splitlines_keepends(text: &str) -> Vec<&str> {
    split_lines(text, true)
}

/// Python `str.splitlines()` — terminators dropped (`import_processor.py:233`).
fn splitlines(text: &str) -> Vec<&str> {
    split_lines(text, false)
}

/// Python `fence_start_re = re.compile(r'^[ \t]{0,3}(`{3,}|~{3,})')`.
fn fence_opener(line: &str) -> Option<(char, usize)> {
    let b = line.as_bytes();
    let mut i = 0usize;
    while i < b.len() && i < 3 && (b[i] == b' ' || b[i] == b'\t') {
        i += 1;
    }
    if i >= b.len() {
        return None;
    }
    let c = b[i];
    if c != b'`' && c != b'~' {
        return None;
    }
    let mut n = 0usize;
    while i + n < b.len() && b[i + n] == c {
        n += 1;
    }
    if n >= 3 {
        Some((c as char, n))
    } else {
        None
    }
}

/// Python `f'^[ \t]{{0,3}}\{fence_char}{{{fence_len},}}[ \t]*\r?$'`.
fn fence_closer(line: &str, fence_char: char, fence_len: usize) -> bool {
    let b = line.as_bytes();
    let fc = fence_char as u8;
    let mut i = 0usize;
    while i < b.len() && i < 3 && (b[i] == b' ' || b[i] == b'\t') {
        i += 1;
    }
    let mut n = 0usize;
    while i + n < b.len() && b[i + n] == fc {
        n += 1;
    }
    if n < fence_len {
        return false;
    }
    i += n;
    while i < b.len() && (b[i] == b' ' || b[i] == b'\t') {
        i += 1;
    }
    if i < b.len() && b[i] == b'\r' {
        i += 1;
    }
    // MULTILINE `$` also matches immediately before the terminating newline that
    // `splitlines(keepends=True)` left on the line.
    i >= b.len() || b[i] == b'\n'
}

/// Python `_process_outside_fences`: `transform` sees every maximal run of lines
/// that is *not* inside a ``` or ~~~ code fence; fence bodies pass through.
pub fn process_outside_fences(content: &str, transform: &mut dyn FnMut(&str) -> String) -> String {
    let lines = splitlines_keepends(content);
    if lines.is_empty() {
        return content.to_string();
    }

    let mut in_fence = false;
    let mut fence_char = '\0';
    let mut fence_len = 0usize;
    let mut out = String::with_capacity(content.len());
    let mut normal_chunk = String::new();

    for line in lines {
        if in_fence {
            out.push_str(line);
            if fence_closer(line, fence_char, fence_len) {
                in_fence = false;
                fence_char = '\0';
                fence_len = 0;
            }
            continue;
        }
        match fence_opener(line) {
            Some((c, n)) => {
                if !normal_chunk.is_empty() {
                    out.push_str(&transform(&normal_chunk));
                    normal_chunk.clear();
                }
                in_fence = true;
                fence_char = c;
                fence_len = n;
                out.push_str(line);
            }
            None => normal_chunk.push_str(line),
        }
    }

    if !normal_chunk.is_empty() {
        out.push_str(&transform(&normal_chunk));
    }
    out
}

/// Python `max((len(m.group(0)) for m in re.finditer(r'`+', body)), default=0)`.
fn longest_backtick_run(text: &str) -> usize {
    BACKTICK_RUN
        .find_iter(text)
        .map(|m| m.as_str().len())
        .max()
        .unwrap_or(0)
}

/// Python `_format_diagram_block`.
pub fn format_diagram_block(code: &str, lang: &str) -> String {
    let body = py_strip(code);
    let fence = "`".repeat(3.max(longest_backtick_run(body) + 1));
    format!("{}{}\n{}\n{}", fence, lang, body, fence)
}

// -------------------------------------------------------------------- CSV -> MD

/// Python `_format_markdown_cell`.
fn format_markdown_cell(raw: &str) -> String {
    if raw.is_empty() {
        return String::new();
    }
    let s = raw.replace("\r\n", "\n").replace('\r', "\n");
    let s = s.replace('\n', "<br>");
    let s = s.replace('|', "\\|");
    py_strip(&s).to_string()
}

/// A record from Python's `csv.reader` (default dialect, `doublequote=True`).
fn csv_parse(text: &str, delimiter: char) -> Vec<Vec<String>> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut touched = false;
    let mut chars = text.chars().peekable();

    loop {
        match chars.next() {
            None => break,
            Some('"') => {
                touched = true;
                if in_quotes {
                    if chars.peek() == Some(&'"') {
                        chars.next();
                        field.push('"');
                        continue;
                    } else {
                        in_quotes = false;
                        continue;
                    }
                }
                if field.is_empty() {
                    in_quotes = true;
                    continue;
                }
                field.push('"');
            }
            Some(c) if c == delimiter && !in_quotes => {
                touched = true;
                row.push(std::mem::take(&mut field));
            }
            Some(c @ ('\n' | '\r')) if !in_quotes => {
                if c == '\r' && chars.peek() == Some(&'\n') {
                    chars.next();
                }
                if touched {
                    row.push(std::mem::take(&mut field));
                    rows.push(std::mem::take(&mut row));
                } else {
                    // Python's reader yields an empty record for a blank line.
                    rows.push(Vec::new());
                }
                touched = false;
            }
            Some(c) => {
                touched = true;
                field.push(c);
            }
        }
    }

    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
}

/// Python `csv_to_markdown_table`.
pub fn csv_to_markdown_table(csv_content: &str, delimiter: char) -> String {
    let rows = csv_parse(py_strip(csv_content), delimiter);
    let num_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if num_cols == 0 {
        return String::new();
    }

    let mut md_lines: Vec<String> = Vec::with_capacity(rows.len() + 1);

    let header = &rows[0];
    let mut headers: Vec<String> = header.iter().map(|h| format_markdown_cell(h)).collect();
    headers.resize(num_cols, String::new());
    md_lines.push(format!("| {} |", headers.join(" | ")));

    md_lines.push(format!(
        "| {} |",
        vec!["---".to_string(); num_cols].join(" | ")
    ));

    for r in rows.iter().skip(1) {
        let mut cells: Vec<String> = r.iter().map(|c| format_markdown_cell(c)).collect();
        cells.truncate(num_cols);
        cells.resize(num_cols, String::new());
        md_lines.push(format!("| {} |", cells.join(" | ")));
    }

    md_lines.join("\n")
}

// -------------------------------------------------------------- code line slice

/// Python `slice_code_lines`.
pub fn slice_code_lines(
    code_content: &str,
    line_begin: Option<usize>,
    line_end: Option<usize>,
    lang: &str,
) -> String {
    let lines = splitlines(code_content);
    let total_lines = lines.len();

    let start = line_begin.map_or(1usize, |lb| lb.max(1));
    let end = line_end.map_or(total_lines, |le| le.min(total_lines));

    let sliced = if start > total_lines {
        String::new()
    } else {
        lines[(start - 1)..end].join("\n")
    };

    let fence = "`".repeat(3.max(longest_backtick_run(&sliced) + 1));
    format!("{}{}\n{}\n{}", fence, lang, sliced, fence)
}

// ------------------------------------------------------------------- path rules

/// Python `ntpath.isabs` (`sep in "/\\" or s[1:2] == ":"`).
fn python_is_abs(raw: &str) -> bool {
    let b = raw.as_bytes();
    match b.first() {
        Some(&c) if c == b'/' || c == b'\\' => true,
        _ => b.len() >= 2 && b[1] == b':' && (b[0] as char).is_ascii_alphabetic(),
    }
}

/// Python `ntpath.normpath` applied to an already-absolute path: collapse `.` and
/// `..`, drop duplicate separators, and use the platform separator.  No symlink
/// resolution (that is what `is_inside_root` adds, via `Path.resolve`).
fn normalize_lexical(raw: &Path) -> PathBuf {
    let unified: String = if cfg!(windows) {
        raw.to_string_lossy().replace('/', "\\")
    } else {
        raw.to_string_lossy().into_owned()
    };
    let sep = if cfg!(windows) { '\\' } else { '/' };
    let sep_s = sep.to_string();

    // The root prefix is never popped by `..` (ntpath.normpath behaviour).
    let mut root = String::new();
    let mut rest: &str = &unified;
    if unified.starts_with(&format!("{sep}{sep}")) {
        let body = &unified[2..];
        let mut it = body.splitn(3, sep);
        let server = it.next().unwrap_or("");
        let share = it.next().unwrap_or("");
        root = format!("{sep}{sep}{server}{sep}{share}{sep}");
        rest = it.next().unwrap_or("");
    } else if unified.len() >= 2 && unified.as_bytes()[1] == b':' {
        let drive = &unified[..2];
        let tail = &unified[2..];
        if tail.starts_with(sep) {
            root = format!("{drive}{sep}");
            rest = &tail[1..];
        } else {
            // drive-relative (rare for @import targets): keep it lexical
            root = drive.to_string();
            rest = tail;
        }
    } else if unified.starts_with(sep) {
        root = sep_s.clone();
        rest = &unified[1..];
    }

    let mut parts: Vec<&str> = Vec::new();
    for comp in rest.split(sep) {
        if comp.is_empty() || comp == "." {
            continue;
        }
        if comp == ".." {
            match parts.last() {
                Some(&"..") => {}
                Some(_) => {
                    parts.pop();
                    continue;
                }
                None => {}
            }
            if root.is_empty() {
                parts.push("..");
            }
            continue;
        }
        parts.push(comp);
    }

    let joined = format!("{}{}", root, parts.join(&sep_s));
    if joined.is_empty() {
        PathBuf::from(".")
    } else {
        PathBuf::from(joined)
    }
}

/// Python `os.path.abspath`.
fn abspath(raw: &str) -> PathBuf {
    let joined = if python_is_abs(raw) {
        PathBuf::from(raw)
    } else {
        match std::env::current_dir() {
            Ok(cwd) => cwd.join(raw),
            Err(_) => PathBuf::from(raw),
        }
    };
    normalize_lexical(&joined)
}

/// Python `_is_inside_root`: `target.resolve().relative_to(root.resolve())`.
/// `dunce::canonicalize` mirrors `Path.resolve(strict=False)` - it fails for
/// missing paths, in which case Python falls back to the cleaned absolute path.
pub fn is_inside_root(root: &Path, target: &Path) -> bool {
    let root_resolved = dunce::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let target_resolved = dunce::canonicalize(target).unwrap_or_else(|_| target.to_path_buf());
    target_resolved.starts_with(&root_resolved)
}

/// The literal Python puts around an `@import "path"` inside a message.
fn quoted_directive(raw_path: &str) -> String {
    format!("`@import \"{}\"`", raw_path)
}

// --------------------------------------------------------------------- processor

pub struct ImportProcessor {
    base_dir: PathBuf,
    max_output_bytes: Option<usize>,
    max_read_bytes: Option<usize>,
}

/// Mutable state shared by every `replace_import` call of one `process` run.
struct RunState<'a> {
    visited: &'a mut HashSet<PathBuf>,
    budget: &'a mut ImportBudget,
    err: Option<ImportError>,
}

impl ImportProcessor {
    /// Python `ImportProcessor.__init__` (minus the unused `allow_private` flag).
    pub fn new(
        base_dir: &str,
        max_output_bytes: Option<usize>,
        max_read_bytes: Option<usize>,
    ) -> Self {
        let base_dir = if base_dir.is_empty() {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        } else {
            abspath(base_dir)
        };
        Self {
            base_dir,
            max_output_bytes,
            max_read_bytes,
        }
    }

    /// Python `ImportProcessor.process(content, current_file)` with the default
    /// `visited` / `depth` / `budget` arguments.
    pub fn process(
        &self,
        content: &str,
        current_file: Option<&str>,
    ) -> Result<String, ImportError> {
        let mut visited: HashSet<PathBuf> = HashSet::new();
        let mut budget = ImportBudget::new(self.max_output_bytes, self.max_read_bytes);
        budget.record_output(content.len());
        self.process_with(content, current_file, &mut visited, 0, &mut budget)
    }

    /// Python `ImportProcessor.process` with every argument explicit.
    pub fn process_with(
        &self,
        content: &str,
        current_file: Option<&str>,
        visited: &mut HashSet<PathBuf>,
        depth: usize,
        budget: &mut ImportBudget,
    ) -> Result<String, ImportError> {
        let current_file = current_file.filter(|s| !s.is_empty());
        let (curr_dir, current_abs) = match current_file {
            Some(file) => {
                let abs = abspath(file);
                let dir = abs
                    .parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| self.base_dir.clone());
                (dir, Some(abs))
            }
            None => (self.base_dir.clone(), None),
        };
        if let Some(abs) = current_abs {
            visited.insert(abs);
        }

        let mut st = RunState {
            visited,
            budget,
            err: None,
        };
        let mut transformed = process_outside_fences(content, &mut |text| {
            self.expand_chunk(text, &curr_dir, depth, &mut st)
        });
        if let Some(e) = st.err.take() {
            return Err(e);
        }

        // Python `process` tail: hard-truncate to `max_output_bytes` and append the
        // budget marker (this runs for every recursion level).
        if let Some(limit) = budget.max_output_bytes {
            let bytes = transformed.as_bytes();
            if bytes.len() > limit {
                let tag = MSG_BUDGET_EXCEEDED.as_bytes();
                transformed = if limit >= tag.len() {
                    let cutoff = floor_char_boundary(&transformed, limit - tag.len());
                    let mut head = transformed[..cutoff].to_string();
                    head.push_str(MSG_BUDGET_EXCEEDED);
                    head
                } else {
                    // Python: `transformed = tag[:limit].decode(errors='ignore')`
                    let cutoff = floor_char_boundary(MSG_BUDGET_EXCEEDED, limit);
                    MSG_BUDGET_EXCEEDED[..cutoff].to_string()
                };
            }
        }

        Ok(transformed)
    }

    /// Python `re.sub(replace_import, text)` over one non-fenced chunk.
    fn expand_chunk(
        &self,
        text: &str,
        curr_dir: &Path,
        depth: usize,
        st: &mut RunState<'_>,
    ) -> String {
        let mut out = String::new();
        let mut last = 0usize;
        for caps in IMPORT_PATTERN.captures_iter(text) {
            let whole = match caps.get(0) {
                Some(m) => m,
                None => continue,
            };
            out.push_str(&text[last..whole.start()]);
            out.push_str(&self.replace_import(&caps, curr_dir, depth, st));
            last = whole.end();
        }
        out.push_str(&text[last..]);
        out
    }

    /// Python `replace_import(match)`.
    fn replace_import(
        &self,
        caps: &regex::Captures,
        curr_dir: &Path,
        depth: usize,
        st: &mut RunState<'_>,
    ) -> String {
        if depth >= MAX_IMPORT_DEPTH {
            return format!(
                "\n> **[ReadMD 警告]**: 达到最大 @import 嵌套深度限制 ({} 层)，已停止继续递归。\n",
                MAX_IMPORT_DEPTH
            );
        }
        if st.budget.exceeded {
            return MSG_BUDGET_EXCEEDED.to_string();
        }

        let raw_path = py_strip(&caps[1]).to_string();
        let raw_attrs = caps.get(2).map(|m| m.as_str()).unwrap_or("");
        let attrs = parse_attributes(raw_attrs);

        // 忽略 [TOC] 伪导入（留给 TOC 引擎处理）
        if raw_path.to_uppercase() == "[TOC]" {
            return caps[0].to_string();
        }

        let target_path = if python_is_abs(&raw_path) {
            normalize_lexical(&PathBuf::from(&raw_path))
        } else {
            normalize_lexical(&curr_dir.join(&raw_path))
        };

        // Enforce the documented document-root boundary after resolving links.
        if !is_inside_root(&self.base_dir, &target_path) {
            return format!(
                "\n> **[ReadMD 错误]**: 导入文件越权路径，已拒绝 {}\n",
                quoted_directive(&raw_path)
            );
        }

        // 循环引用防御
        if st.visited.contains(&target_path) {
            return format!(
                "\n> **[ReadMD 警告]**: 检测到循环引用 {}，已自动忽略。\n",
                quoted_directive(&raw_path)
            );
        }

        // 文件存在性检查
        if !target_path.is_file() {
            return format!(
                "\n> **[ReadMD 错误]**: 导入文件不存在 {} ({})\n",
                quoted_directive(&raw_path),
                target_path.display()
            );
        }

        let file_size = match fs::metadata(&target_path) {
            Ok(md) => md.len() as usize,
            Err(_) => {
                st.err.get_or_insert(ImportError {
                    code: raw_path.clone(),
                    error_code: ImportErrorCode::ImportFileNotFound,
                });
                return String::new();
            }
        };
        if !st.budget.can_read(file_size) {
            return MSG_BUDGET_EXCEEDED.to_string();
        }
        st.budget.record_read(file_size);

        let ext = target_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();

        // 3. 导入 PDF 页面切片
        if ext == "pdf" {
            let page_no = attrs
                .get("page_no")
                .or_else(|| attrs.get("page"))
                .filter(|v| v.is_truthy())
                .map(|v| v.display())
                .unwrap_or_else(|| "1".to_string());
            let chunk = format!(
                "\n> **[ReadMD 错误]**: 提取 PDF 页面失败 `{}` (页码 {}): {}\n",
                raw_path, page_no, PDF_RENDER_UNAVAILABLE
            );
            if !st.budget.can_output(chunk.len()) {
                return MSG_BUDGET_EXCEEDED.to_string();
            }
            st.budget.record_output(chunk.len());
            return chunk;
        }

        let raw_bytes = match fs::read(&target_path) {
            Ok(b) => b,
            Err(e) => {
                return format!(
                    "\n> **[ReadMD 错误]**: 读取文件失败 `{}`: {}\n",
                    raw_path, e
                );
            }
        };

        // W4-04: Python's own candidate loop (`import_processor.py:354-366`) -
        // `utf-8-sig` / `utf-8` / `gb18030` / `big5` in that order, strict, and
        // only `UnicodeDecodeError`/`LookupError` are caught.  A non-`str`
        // `encoding` attribute (`{encoding=true}` parses to a `bool`) is a
        // `TypeError` Python does not catch, so it must abort the request.
        let file_text = match python_decode_chain(attrs.get("encoding"), &raw_bytes) {
            Ok(Some(text)) => text,
            Ok(None) => {
                return format!(
                    "\n> **[ReadMD 错误]**: unsupported_encoding `{}`\n",
                    raw_path
                );
            }
            Err(py_type) => {
                st.err.get_or_insert(ImportError {
                    code: format!("decode() argument 'encoding' must be str, not {}", py_type),
                    error_code: ImportErrorCode::ImportUnsupportedEncoding,
                });
                return String::new();
            }
        };

        let res: String = match ext.as_str() {
            // 1. 导入子 Markdown
            "md" | "markdown" | "mdown" => {
                let mut sub_visited = st.visited.clone();
                let child = self.process_with(
                    &file_text,
                    Some(&target_path.to_string_lossy()),
                    &mut sub_visited,
                    depth + 1,
                    st.budget,
                );
                match child {
                    Ok(text) => {
                        if st.budget.exceeded {
                            return MSG_BUDGET_EXCEEDED.to_string();
                        }
                        return text;
                    }
                    Err(e) => {
                        st.err.get_or_insert(e);
                        return String::new();
                    }
                }
            }
            // 2. 导入 CSV / TSV 数据表
            "csv" => csv_to_markdown_table(&file_text, ','),
            "tsv" => csv_to_markdown_table(&file_text, '\t'),
            // 4. 导入 LESS 样式
            "less" => format!(
                "<style type=\"text/less\">\n{}\n</style>",
                py_strip(&file_text)
            ),
            // 5. 导入 TikZ 矢量图
            "tikz" => format_tikz_html(&file_text),
            "tex" if attrs.get("tikz").map(|v| v.is_truthy()).unwrap_or(false) => {
                format_tikz_html(&file_text)
            }
            // 6. 导入图表源码文件 (PUML / DOT / WaveDrom)
            "puml" | "plantuml" => format_diagram_block(&file_text, "puml"),
            "dot" | "viz" => format_diagram_block(&file_text, "viz"),
            "wavedrom" => format_diagram_block(&file_text, "wavedrom"),
            // 7. 导入源码切片或普通代码块
            _ => {
                let lang = match attrs.get("lang") {
                    Some(v) => v.display(),
                    None => ext.trim_start_matches('.').to_string(),
                };
                let mut lb: Option<usize> = None;
                let mut le: Option<usize> = None;
                if let Some(v) = attrs.get("line_begin") {
                    match v.parse_positive_int() {
                        Some(n) if n <= i64::from(u32::MAX) => lb = Some(n as usize),
                        _ => return "\n> **[ReadMD 错误]**: invalid_line_range\n".to_string(),
                    }
                }
                if let Some(v) = attrs.get("line_end") {
                    match v.parse_positive_int() {
                        Some(n) if n <= i64::from(u32::MAX) => le = Some(n as usize),
                        _ => return "\n> **[ReadMD 错误]**: invalid_line_range\n".to_string(),
                    }
                }
                if let (Some(b), Some(e)) = (lb, le) {
                    if e < b {
                        return "\n> **[ReadMD 错误]**: invalid_line_range\n".to_string();
                    }
                }
                slice_code_lines(&file_text, lb, le, &lang)
            }
        };

        if !st.budget.can_output(res.len()) {
            return MSG_BUDGET_EXCEEDED.to_string();
        }
        st.budget.record_output(res.len());
        res
    }
}

// ---------------------------------------------------------------------------------
// Character codecs.
//
// The gb18030 / gbk / big5 / cp1252 code tables and their decoders used to be a
// private copy inside this module (~490 generated lines plus ~150 lines of
// hand-written stepping).  `src/codecs.rs` is now the crate's single home for
// them: the same CPython 3.11.15 measurements emitted by
// `scratch/rust_parity/we8/gen_tables.py`, replayed there against WE8's
// differential corpus, plus `utf-16-le` / `cp437` / `utf-8-sig` and the
// `errors='replace'` / `'ignore'` handlers the other call sites need.
// Delegating keeps this ladder and `convert.py:955/1060/2407`, `txtmd.py:15`,
// `readmd_core/utils.py:99` and `readmd.py:947` on one decoder.
// ---------------------------------------------------------------------------------

/// `codecs.lookup(name).name`, restricted to the codecs `crate::codecs`
/// implements.  `None` is CPython's `LookupError`, which the candidate loop in
/// `import_processor.py:357-361` catches and continues past.  The name is
/// CPython-normalised, so `'UTF 8'` -> `utf-8` while `'utf.8'`, `'gb-18030'` and
/// `'CP-1252'` are misses.
fn lookup_codec_name(name: &str) -> Option<&'static str> {
    codecs::lookup_codec_name(name)
}


/// Python's `bytes.decode(name)` for the names the kernel implements, strict.
/// `None` stands for both `UnicodeDecodeError` and `LookupError`, because the
/// candidate loop only ever tests success.
fn decode_with_codec(canonical: &str, raw: &[u8]) -> Option<String> {
    codecs::try_decode_strict(raw, canonical)
}

/// CPython's type name inside `bytes.decode`'s `TypeError` message.
fn python_type_name(value: &AttributeValue) -> &'static str {
    match value {
        AttributeValue::String(_) => "str",
        AttributeValue::Int(_) => "int",
        AttributeValue::Bool(_) => "bool",
        AttributeValue::Array(_) => "list",
    }
}

/// `import_processor.py:354-366`, behaviour for behaviour:
///
/// ```python
/// user_enc = attrs.get('encoding')
/// encodings_to_try = [user_enc] if user_enc else ['utf-8-sig', 'utf-8', 'gb18030', 'big5']
/// for enc in encodings_to_try:
///     if not enc:
///         continue
///     try:
///         file_text = raw_bytes.decode(enc)
///         break
///     except (UnicodeDecodeError, LookupError):
///         continue
/// ```
///
/// * `Ok(Some(text))` - a candidate accepted the bytes.
/// * `Ok(None)` - every candidate raised `UnicodeDecodeError`/`LookupError`, and
///   Python emits the inline `unsupported_encoding` message.  This chain has
///   **no** `latin-1` and no `errors='replace'`: `readmd.py:941-953`
///   (`Handler.read_text`, `('utf-8','gb18030','big5','latin-1')` + `replace`)
///   is a different chain and must not be borrowed here.
/// * `Err(py_type)` - `raw_bytes.decode(<non-str>)` raises `TypeError`, which
///   the loop does *not* catch: it escapes to `readmd.py:1964-1966` and becomes
///   `_send_api_error(500, 'import_process_failed')`.
fn python_decode_chain(
    user_enc: Option<&AttributeValue>,
    raw: &[u8],
) -> Result<Option<String>, &'static str> {
    let default_chain: [&str; 4] = ["utf-8-sig", "utf-8", "gb18030", "big5"];
    let user_enc: Option<&str> = match user_enc {
        None => None,
        Some(value) if !value.is_truthy() => None,
        Some(AttributeValue::String(name)) => Some(name.as_str()),
        Some(other) => return Err(python_type_name(other)),
    };
    if let Some(name) = user_enc {
        return Ok(decode_via_candidate(name, raw));
    }
    for name in default_chain {
        if let Some(text) = decode_via_candidate(name, raw) {
            return Ok(Some(text));
        }
    }
    Ok(None)
}

/// One candidate of Python's loop: a `LookupError` (unknown codec name) and a
/// strict `UnicodeDecodeError` both mean "no text, carry on".
fn decode_via_candidate(name: &str, raw: &[u8]) -> Option<String> {
    decode_with_codec(lookup_codec_name(name)?, raw)
}

fn floor_char_boundary(text: &str, mut idx: usize) -> usize {
    if idx >= text.len() {
        return text.len();
    }
    while idx > 0 && !text.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

/// Python `process_markdown_imports` - the public shortcut used by
/// `readmd.py::_api_import_process`.
pub fn process_markdown_imports(
    content: &str,
    base_dir: &str,
    current_file: Option<&str>,
    max_output_bytes: Option<usize>,
    max_read_bytes: Option<usize>,
) -> Result<String, ImportError> {
    let processor = ImportProcessor::new(base_dir, max_output_bytes, max_read_bytes);
    processor.process(content, current_file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_splitlines_covers_all_eleven_cpython_line_breaks() {
        // Ground truth taken live from CPython 3.11 with
        //   python -c "print(repr(s.splitlines(True)))"
        // for every string below; Rust's `str::lines()` only knows \n and \r\n.
        assert_eq!(splitlines_keepends("a\u{b}b\u{c}c\x1d"), vec!["a\u{b}", "b\u{c}", "c\u{1d}"]);
        assert_eq!(
            splitlines_keepends("a\x1cb\x1db\x1ec"),
            vec!["a\u{1c}", "b\u{1d}", "b\u{1e}", "c"]
        );
        assert_eq!(
            splitlines_keepends("a\u{85}b\u{2028}c\u{2029}d"),
            vec!["a\u{85}", "b\u{2028}", "c\u{2029}", "d"]
        );
        assert_eq!(splitlines_keepends("a\r\nb"), vec!["a\r\n", "b"]);
        assert_eq!(splitlines_keepends("a\rb"), vec!["a\r", "b"]);
        // A trailing terminator must not invent an empty final line ...
        assert_eq!(splitlines_keepends("a\u{2028}"), vec!["a\u{2028}"]);
        assert_eq!(splitlines_keepends("a\n"), vec!["a\n"]);
        // ... but an *empty* final line before a terminator is real.
        assert_eq!(splitlines_keepends("a\n\n"), vec!["a\n", "\n"]);
        assert_eq!(splitlines_keepends(""), Vec::<&str>::new());
        // \x1f is NOT a line break, so it stays glued to its neighbours.
        assert_eq!(splitlines_keepends("x\x1e\x1f\u{85}y"), vec!["x\u{1e}", "\u{1f}\u{85}", "y"]);
        // Multibyte text on either side of a Unicode break keeps both halves.
        assert_eq!(splitlines_keepends("表\u{2029}演"), vec!["表\u{2029}", "演"]);

        // keepends=False drops exactly the terminator CPython dropped.
        assert_eq!(splitlines("a\u{b}b\u{c}c\x1d"), vec!["a", "b", "c"]);
        assert_eq!(splitlines("a\n\n"), vec!["a", ""]);
        assert_eq!(splitlines("a\u{85}b"), vec!["a", "b"]);
        assert_eq!(splitlines("a"), vec!["a"]);
        assert_eq!(splitlines(""), Vec::<&str>::new());
    }

    #[test]
    fn test_slice_code_lines_splits_on_unicode_breaks_too() {
        // `slice_code_lines` is the public consumer of `splitlines()`; a \u2028
        // has to count as a line boundary exactly like in Python.
        assert_eq!(
            slice_code_lines("one\u{2028}two\nthree", Some(2), Some(3), "text"),
            "```text\ntwo\nthree\n```"
        );
    }


    #[test]
    fn test_parse_attributes_simple() {
        let attrs = parse_attributes("line_begin=10 line_end=20");
        assert_eq!(attrs.get("line_begin"), Some(&AttributeValue::Int(10)));
        assert_eq!(attrs.get("line_end"), Some(&AttributeValue::Int(20)));
    }

    #[test]
    fn test_parse_attributes_array() {
        let attrs = parse_attributes("highlight=[15, 18]");
        assert!(matches!(
            attrs.get("highlight"),
            Some(AttributeValue::Array(_))
        ));
    }

    #[test]
    fn test_csv_to_markdown_table() {
        let csv = "name,age\ntom,20\njerry,25";
        let md = csv_to_markdown_table(csv, ',');
        assert!(md.contains("| name |"));
        assert!(md.contains("| tom |"));
    }

    // ------------------------------------------------------------- additions

    fn scratch_dir(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!(
            "readmd_import_{}_{}_{}",
            tag,
            std::process::id(),
            nanos
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create scratch dir");
        dir
    }

    #[test]
    fn test_parse_attributes_keeps_non_digit_array_items() {
        // Python: `[15, abc]` -> [15, 'abc']; the dropped-item filter_map was a bug.
        let attrs = parse_attributes("highlight=[15, abc]");
        assert_eq!(
            attrs.get("highlight"),
            Some(&AttributeValue::Array(vec![
                AttributeValue::Int(15),
                AttributeValue::String("abc".to_string()),
            ]))
        );
    }

    #[test]
    fn test_parse_attributes_lone_quote_does_not_panic() {
        // Python: `v[1:-1]` on the 1-char value `"` is "" (and used to panic here).
        let attrs = parse_attributes("bad=\"");
        assert_eq!(
            attrs.get("bad"),
            Some(&AttributeValue::String(String::new()))
        );
    }

    #[test]
    fn test_import_pattern_matches_both_quote_styles() {
        let caps = IMPORT_PATTERN.captures("@import 'a.md' {lang=py}").unwrap();
        assert_eq!(&caps[1], "a.md");
        assert_eq!(&caps[2], "lang=py");
        assert!(IMPORT_PATTERN.is_match("@import \"a.md\"\t \r\n"));
        // The optional group still participates for `{bad}`; it just parses to no
        // attributes (Python's token pattern needs `key=value`).
        assert_eq!(
            &IMPORT_PATTERN.captures("@import \"a.md\" {bad}").unwrap()[2],
            "bad"
        );
        assert!(parse_attributes("bad").is_empty());
        assert_eq!(
            &IMPORT_PATTERN.captures("@import \"a.md\" {}").unwrap()[2],
            ""
        );
        assert!(!IMPORT_PATTERN.is_match("text @import \"a.md\""));
        assert!(!IMPORT_PATTERN.is_match("@import \"a.md\" tail"));
    }

    #[test]
    fn test_fence_opener_and_closer() {
        assert_eq!(fence_opener("```py\n"), Some(('`', 3)));
        assert_eq!(fence_opener("   ~~~\n"), Some(('~', 3)));
        // two backticks then tildes: the group must start at column <= 3 with 3+
        // identical fence chars, so Python does not treat this line as a fence.
        assert_eq!(fence_opener("   ``~~~\n"), None);
        assert_eq!(fence_opener("    ```\n"), None);
        assert!(fence_closer("```\n", '`', 3));
        assert!(fence_closer("  ````  \r\n", '`', 3));
        assert!(!fence_closer("```\n", '`', 4));
        assert!(!fence_closer("``` py\n", '`', 3));
    }

    #[test]
    fn test_process_outside_fences_only_transforms_normal_text() {
        let src = "before\n```\n@import \"inside.md\"\n```\nafter\n";
        let got = process_outside_fences(src, &mut |t| t.to_uppercase());
        assert_eq!(got, "BEFORE\n```\n@import \"inside.md\"\n```\nAFTER\n");
    }

    #[test]
    fn test_format_diagram_block_fence_scales() {
        assert_eq!(
            format_diagram_block("digraph{}\n", "viz"),
            "```viz\ndigraph{}\n```"
        );
        // A body containing ``` must be wrapped in a 4-backtick fence.
        assert_eq!(
            format_diagram_block("a ``` b", "puml"),
            "````puml\na ``` b\n````"
        );
    }

    #[test]
    fn test_slice_code_lines_matches_python() {
        let code = "l1\nl2\nl3\nl4\n";
        assert_eq!(
            slice_code_lines(code, Some(2), Some(3), "py"),
            "```py\nl2\nl3\n```"
        );
        assert_eq!(slice_code_lines("a\nb", None, None, ""), "```\na\nb\n```");
        // start beyond EOF -> empty body, no line noise
        assert_eq!(
            slice_code_lines("a\nb", Some(9), None, "sh"),
            "```sh\n\n```"
        );
        // fence = max(3, longest backtick run + 1)
        assert_eq!(
            slice_code_lines("x``y\n", Some(1), Some(1), ""),
            "```\nx``y\n```"
        );
        assert_eq!(
            slice_code_lines("x```y\n", Some(1), Some(1), "rs"),
            "````rs\nx```y\n````"
        );
    }

    #[test]
    fn test_csv_matches_python_semantics() {
        // quoted comma, doubled quotes, ragged final row
        let md = csv_to_markdown_table("a,b\n\"x,y\",\"p\"\"q\"\n1,", ',');
        assert_eq!(md, "| a | b |\n| --- | --- |\n| x,y | p\"q |\n| 1 |  |");
        // an interior blank line yields an empty record padded to the column count
        let md2 = csv_to_markdown_table("a,b\n\n1,2", ',');
        assert_eq!(md2, "| a | b |\n| --- | --- |\n|  |  |\n| 1 | 2 |");
        // Python `.strip()`s the text first: trailing newlines add no phantom row
        assert_eq!(
            csv_to_markdown_table("a,b\n", ','),
            "| a | b |\n| --- | --- |"
        );
        assert_eq!(csv_to_markdown_table("", ','), "");
    }

    #[test]
    fn test_csv_cell_escapes_pipe_and_newline() {
        let md = csv_to_markdown_table("h\n\"a|b\nc\"", ',');
        assert_eq!(md, "| h |\n| --- |\n| a\\|b<br>c |");
    }

    #[test]
    fn test_process_requires_existing_file_message() {
        let dir = scratch_dir("missing");
        let out = process_markdown_imports(
            "@import \"gone.md\"\n",
            &dir.to_string_lossy(),
            None,
            None,
            None,
        )
        .unwrap();
        let want = format!(
            "\n> **[ReadMD 错误]**: 导入文件不存在 `@import \"gone.md\"` ({})\n",
            normalize_lexical(&dir.join("gone.md")).display()
        );
        // MULTILINE `$` never eats the line terminator, so the source "\n" stays.
        assert_eq!(out, format!("{want}\n"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_path_traversal_is_rejected() {
        let dir = scratch_dir("root");
        fs::write(dir.join("outside.md"), "SECRET").unwrap();
        let root = dir.join("sub");
        fs::create_dir(&root).unwrap();
        let out = process_markdown_imports(
            "@import \"../outside.md\"\n",
            &root.to_string_lossy(),
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(
            out,
            "\n> **[ReadMD 错误]**: 导入文件越权路径，已拒绝 `@import \"../outside.md\"`\n\n"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_markdown_recursion_and_circular_reference() {
        let dir = scratch_dir("recurse");
        fs::write(dir.join("child.md"), "child text\n").unwrap();
        fs::write(
            dir.join("parent.md"),
            "@import \"child.md\"\n@import \"parent.md\"\n",
        )
        .unwrap();
        let doc = format!("head\n@import \"{}\"\n", dir.join("parent.md").display());
        let out = process_markdown_imports(&doc, &dir.to_string_lossy(), None, None, None).unwrap();
        assert!(out.contains("child text"), "{out}");
        // `parent.md` is already visited when child.md imports it back.
        assert!(
            out.contains("检测到循环引用 `@import \"parent.md\"`，已自动忽略。"),
            "{out}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_circular_imports_do_not_leak_between_siblings() {
        // Python copies `visited` per recursion (sub_visited = set(visited)), so
        // importing the same file twice from one document is not "circular".
        let dir = scratch_dir("siblings");
        fs::write(dir.join("a.md"), "A\n").unwrap();
        let out = process_markdown_imports(
            "@import \"a.md\"\n@import \"a.md\"\n",
            &dir.to_string_lossy(),
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(out, "A\n\nA\n\n");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_depth_limit_is_per_import_not_whole_document() {
        let dir = scratch_dir("depth");
        fs::write(dir.join("leaf.md"), "leaf\n").unwrap();
        let mut visited: HashSet<PathBuf> = HashSet::new();
        let mut budget = ImportBudget::new(None, None);
        let p = ImportProcessor::new(&dir.to_string_lossy(), None, None);
        let out = p
            .process_with(
                "keep me\n@import \"leaf.md\"\nkeep me too\n",
                None,
                &mut visited,
                MAX_IMPORT_DEPTH,
                &mut budget,
            )
            .unwrap();
        assert_eq!(
            out,
            "keep me\n\n> **[ReadMD 警告]**: 达到最大 @import 嵌套深度限制 (8 层)，已停止继续递归。\n\nkeep me too\n"
        );
    }

    #[test]
    fn test_budget_exceeded_is_inline_text() {
        let dir = scratch_dir("budget");
        fs::write(
            dir.join("a.md"),
            "0123456789012345678901234567890123456789012345678901234567890123\n",
        )
        .unwrap();
        let out = process_markdown_imports(
            "@import \"a.md\"\n@import \"a.md\"\n",
            &dir.to_string_lossy(),
            None,
            Some(10),
            None,
        )
        .unwrap();
        // Python never raises: it truncates to the limit with the marker's own bytes.
        assert_eq!(out, "\n> **[Read");
        assert_eq!(out.len(), 10);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_read_budget_blocks_import_with_marker() {
        let dir = scratch_dir("readbudget");
        fs::write(dir.join("a.md"), "0123456789\n").unwrap();
        let out = process_markdown_imports(
            "@import \"a.md\"\n",
            &dir.to_string_lossy(),
            None,
            None,
            Some(4),
        )
        .unwrap();
        assert_eq!(out, format!("{}\n", MSG_BUDGET_EXCEEDED));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_toc_pseudo_import_is_left_alone() {
        let dir = scratch_dir("toc");
        let out = process_markdown_imports(
            "[TOC]\n@import \"[TOC]\"\n",
            &dir.to_string_lossy(),
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(out, "[TOC]\n@import \"[TOC]\"\n");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_invalid_line_range_when_value_present_but_unusable() {
        let dir = scratch_dir("range");
        fs::write(dir.join("s.py"), "a\nb\nc\n").unwrap();
        for attrs in [
            "line_begin=abc",
            "line_begin=x",
            "line_end=0",
            "line_begin=3 line_end=1",
        ] {
            let doc = format!("@import \"s.py\" {{{}}}\n", attrs);
            let out =
                process_markdown_imports(&doc, &dir.to_string_lossy(), None, None, None).unwrap();
            assert_eq!(
                out, "\n> **[ReadMD 错误]**: invalid_line_range\n\n",
                "{}",
                attrs
            );
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_quoted_line_range_string_is_accepted() {
        // Python _parse_positive_int('"1"') == 1
        let dir = scratch_dir("qrange");
        fs::write(dir.join("s.py"), "a\nb\nc\n").unwrap();
        let out = process_markdown_imports(
            "@import \"s.py\" {line_begin=\"2\" line_end=\"3\"}\n",
            &dir.to_string_lossy(),
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(out, "```py\nb\nc\n```\n");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_lang_default_drops_the_leading_dot() {
        let dir = scratch_dir("lang");
        fs::write(dir.join("app.py"), "print(1)\n").unwrap();
        let out = process_markdown_imports(
            "@import \"app.py\"\n",
            &dir.to_string_lossy(),
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(out, "```py\nprint(1)\n```\n");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_less_wavedrom_and_puml_branches() {
        let dir = scratch_dir("assets");
        fs::write(dir.join("s.less"), "  a{color:red}  ").unwrap();
        fs::write(dir.join("d.puml"), "@startuml\n").unwrap();
        fs::write(dir.join("d.wavedrom"), "{}\n").unwrap();
        let out = process_markdown_imports(
            "@import \"s.less\"\n@import \"d.puml\"\n@import \"d.wavedrom\"\n",
            &dir.to_string_lossy(),
            None,
            None,
            None,
        )
        .unwrap();
        assert!(
            out.contains("<style type=\"text/less\">\na{color:red}\n</style>"),
            "{out}"
        );
        assert!(out.contains("```puml\n@startuml\n```"), "{out}");
        assert!(out.contains("```wavedrom\n{}\n```"), "{out}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_unsupported_encoding_message() {
        let dir = scratch_dir("enc");
        fs::write(dir.join("bad.txt"), [0xFFu8, 0xFE, 0x00, 0x01]).unwrap();
        let out = process_markdown_imports(
            "@import \"bad.txt\"\n",
            &dir.to_string_lossy(),
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(
            out,
            "\n> **[ReadMD 错误]**: unsupported_encoding `bad.txt`\n\n"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_utf8_bom_is_stripped() {
        let dir = scratch_dir("bom");
        fs::write(dir.join("b.md"), "\u{feff}bom body\n").unwrap();
        let out = process_markdown_imports(
            "@import \"b.md\"\n",
            &dir.to_string_lossy(),
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(out, "bom body\n\n");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_import_inside_code_fence_is_not_expanded() {
        let dir = scratch_dir("fence");
        fs::write(dir.join("a.md"), "SHOULD NOT APPEAR\n").unwrap();
        let doc = "```md\n@import \"a.md\"\n```\n";
        let out = process_markdown_imports(doc, &dir.to_string_lossy(), None, None, None).unwrap();
        assert_eq!(out, doc);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_normalize_lexical_collapses_dot_dot() {
        let base = PathBuf::from(if cfg!(windows) {
            r"C:\docs\a"
        } else {
            "/docs/a"
        });
        assert_eq!(
            normalize_lexical(&base.join("../b")),
            PathBuf::from(if cfg!(windows) {
                r"C:\docs\b"
            } else {
                "/docs/b"
            })
        );
        assert_eq!(
            normalize_lexical(&base.join("././b")),
            PathBuf::from(if cfg!(windows) {
                r"C:\docs\a\b"
            } else {
                "/docs/a/b"
            })
        );
    }

    #[test]
    fn test_error_code_serialized_names() {
        assert_eq!(
            serde_json::to_string(&ImportErrorCode::ImportPathTraversal).unwrap(),
            "\"import_path_traversal\""
        );
        assert_eq!(
            serde_json::to_string(&ImportErrorCode::ImportUnsupportedEncoding).unwrap(),
            "\"import_unsupported_encoding\""
        );
        assert_eq!(
            serde_json::to_string(&ImportError {
                code: "x.md".to_string(),
                error_code: ImportErrorCode::InvalidLineRange,
            })
            .unwrap(),
            "{\"code\":\"x.md\",\"error_code\":\"invalid_line_range\"}"
        );
    }

    // ------------------------------------------------------------------ W4-04
    //
    // Every literal below is CPython 3.11.15's own answer, produced by
    // `python scratch/rust_parity/we8/probe_tests.py` (see the fix report's
    // `Measured Python ground truth` section for the transcript).

    /// The candidate loop's `Ok` arm: `None` means "every candidate raised
    /// `UnicodeDecodeError`/`LookupError`", i.e. Python's inline
    /// `unsupported_encoding` message.
    fn chain(user_enc: Option<&AttributeValue>, raw: &[u8]) -> Option<String> {
        python_decode_chain(user_enc, raw).unwrap()
    }

    fn enc(name: &str) -> AttributeValue {
        AttributeValue::String(name.to_string())
    }

    #[test]
    fn test_default_chain_is_python_order_not_utf8_only() {
        // CPython: '你好，世界！'.encode('gb18030') -> the `gb18030` candidate wins.
        let raw = [
            0xC4u8, 0xE3, 0xBA, 0xC3, 0xA3, 0xAC, 0xCA, 0xC0, 0xBD, 0xE7, 0xA3, 0xA1,
        ];
        assert_eq!(
            chain(None, &raw),
            Some("\u{4f60}\u{597d}\u{ff0c}\u{4e16}\u{754c}\u{ff01}".to_string())
        );
        // ... and the `utf-8` / `utf-8-sig` candidates really did fail first.
        assert_eq!(decode_with_codec("utf-8", &raw), None);
        assert_eq!(decode_with_codec("utf-8-sig", &raw), None);
    }

    #[test]
    fn test_big5_file_is_decoded_as_gb18030_because_the_order_decides() {
        // '\u4e2d\u6587\u6587\u672c\u6e2c\u8a66'.encode('big5'): a real Big5 file.
        // Every Big5 double is also a valid GB18030 double (measured: 0 Big5 codes
        // are absent from the gb18030 grid), so Python's `gb18030` entry fires
        // first and the answer is mojibake - `big5` is unreachable for such a file.
        // Pinning the mojibake is the parity requirement; "fixing" it is not.
        let raw = [
            0xA4u8, 0xA4, 0xA4, 0xE5, 0xA4, 0xE5, 0xA5, 0xBB, 0xB4, 0xFA, 0xB8, 0xD5,
        ];
        assert_eq!(
            chain(None, &raw),
            Some("\u{3044}\u{3085}\u{3085}\u{30bb}\u{4ee3}\u{521a}".to_string())
        );
        // The same bytes decoded by the codec Python never reaches:
        assert_eq!(
            decode_with_codec("big5", &raw),
            Some("\u{4e2d}\u{6587}\u{6587}\u{672c}\u{6e2c}\u{8a66}".to_string())
        );
    }

    #[test]
    fn test_bom_handling_matches_the_utf8_sig_codec() {
        // b'\xef\xbb\xbftitle'.decode('utf-8-sig') == 'title'
        assert_eq!(chain(None, b"\xef\xbb\xbftitle"), Some("title".to_string()));
        // b'\xef\xbb\xbf'.decode('utf-8-sig') == ''  (BOM only)
        assert_eq!(chain(None, b"\xef\xbb\xbf"), Some(String::new()));
        // b'x\xef\xbb\xbf'.decode('utf-8-sig') == 'x\ufeff'  (BOM only at offset 0)
        assert_eq!(chain(None, b"x\xef\xbb\xbf"), Some("x\u{feff}".to_string()));
        // A *partial* BOM is not text for utf-8-sig/utf-8, but EF BB is a valid
        // gb18030 double: b'\xef\xbb'.decode('utf-8-sig') raises,
        // b'\xef\xbb'.decode('gb18030') == '\u9518'.
        assert_eq!(decode_with_codec("utf-8-sig", b"\xef\xbb"), None);
        assert_eq!(chain(None, b"\xef\xbb"), Some("\u{9518}".to_string()));
    }

    #[test]
    fn test_empty_input_decodes_to_empty_text() {
        for codec in ["utf-8", "utf-8-sig", "gb18030", "big5", "ascii", "gbk"] {
            assert_eq!(
                decode_with_codec(codec, b""),
                Some(String::new()),
                "{}",
                codec
            );
        }
        assert_eq!(chain(None, b""), Some(String::new()));
    }

    #[test]
    fn test_trailing_and_overlong_sequences_reach_unsupported_encoding() {
        // Every vector below makes all four candidates raise, which is the only
        // way Python produces `unsupported_encoding` (no latin-1, no `replace`).
        for raw in [
            &b"abc\xd6"[..],                 // truncated: trailing lead
            &b"ab\x95\x32\x82"[..],          // truncated 4-byte gb18030
            &b"\xe0\x80\x80"[..],            // over-long UTF-8
            &b"\x81"[..],                    // lone gb18030 lead
            &b"caf\xe9"[..],                 // not ASCII, not a CJK double
            &[0x00u8, 0x01, 0x02, 0x80][..], // 0x80 is no valid single byte
        ] {
            assert_eq!(chain(None, raw), None, "{}", hex_escape(raw));
        }
    }

    #[test]
    fn test_gb18030_four_byte_page_gbk_rejects_it() {
        // '\U00020000汉字'.encode('gb18030'); only the 4-byte page yields U+20000.
        let raw = [0x95u8, 0x32, 0x82, 0x36, 0xBA, 0xBA, 0xD7, 0xD6];
        assert_eq!(
            chain(None, &raw),
            Some("\u{20000}\u{6c49}\u{5b57}".to_string())
        );
        assert_eq!(
            decode_with_codec("gb18030", &raw),
            Some("\u{20000}\u{6c49}\u{5b57}".to_string())
        );
        // gbk has no 4-byte form: 0x95 0x32 -> 0x30..0x39 is not a gbk trail.
        assert_eq!(decode_with_codec("gbk", &raw), None);
        // Linear index 49140 (bytes 84 39 81 30) is one of the 499,604 slots
        // CPython leaves unassigned, so the whole decode fails - it must not
        // degrade to `84 39` + `81 30`.  The linear-index arithmetic itself is
        // pinned in `codecs::tests::test_gb18030_four_byte_page_gbk_rejects_it`;
        // what this module owns is the ladder-visible outcome below.
        assert_eq!(decode_with_codec("gb18030", b"\x84\x39\x81\x30"), None);
        // A second digit commits to 4 bytes: bad third byte cannot fall back.
        assert_eq!(decode_with_codec("gb18030", b"\x95\x32\x41\x30"), None);
    }

    #[test]
    fn test_gbk_holes_only_affect_gbk() {
        // b'\xa1\x40'.decode('gb18030') == '\ue4c6' (PUA fill) but gbk leaves the
        // slot unassigned; big5 has it as '\u3000'.
        assert_eq!(
            decode_with_codec("gb18030", b"\xa1\x40"),
            Some("\u{e4c6}".to_string())
        );
        assert_eq!(decode_with_codec("gbk", b"\xa1\x40"), None);
        assert_eq!(
            decode_with_codec("big5", b"\xa1\x40"),
            Some("\u{3000}".to_string())
        );
        assert_eq!(chain(None, b"\xa1\x40"), Some("\u{e4c6}".to_string()));
        // 0xA1A1 is assigned in both.
        assert_eq!(
            decode_with_codec("gbk", b"\xa1\xa1"),
            Some("\u{3000}".to_string())
        );
    }

    #[test]
    fn test_codec_name_normalization_matches_cpython() {
        // `codecs.lookup(name).name` for the spellings the kernel implements.
        for (name, canonical) in [
            ("utf-8", "utf-8"),
            ("UTF 8", "utf-8"),
            ("utf--8", "utf-8"),
            ("UTF8", "utf-8"),
            (" utf-8 ", "utf-8"),
            ("utf_8_sig", "utf-8-sig"),
            ("big5-tw", "big5"),
            ("BIG5", "big5"),
            ("936", "gbk"),
            ("cp936", "gbk"),
            ("windows-1252", "cp1252"),
            ("l1", "iso8859-1"),
            ("latin", "iso8859-1"),
            ("ascii", "ascii"),
            ("gb18030", "gb18030"),
        ] {
            assert_eq!(lookup_codec_name(name), Some(canonical), "{}", name);
        }
        // Measured `LookupError`s: normalization collapses punctuation but never
        // removes it, and no alias/module carries these names.
        for name in [
            "utf.8",
            "gb-18030",
            "BIG 5",
            "cp-1252",
            "x-mac-roman",
            "",
            "   ",
            "utf8mb4",
        ] {
            assert_eq!(lookup_codec_name(name), None, "{}", name);
        }
    }

    #[test]
    fn test_explicit_encoding_selects_exactly_one_candidate() {
        // b'caf\xe9\xa1\x40' answers, measured per codec.
        let raw = b"caf\xe9\xa1\x40";
        for name in ["utf-8", "utf-8-sig", "ascii"] {
            assert_eq!(chain(Some(&enc(name)), raw), None, "{}", name);
        }
        for name in ["cp1252", "windows-1252", "l1", "latin", "iso-8859-1"] {
            assert_eq!(
                chain(Some(&enc(name)), raw),
                Some("caf\u{e9}\u{a1}@".to_string()),
                "{}",
                name
            );
        }
        assert_eq!(
            chain(Some(&enc("big5-tw")), raw),
            Some("caf\u{61bf}@".to_string())
        );
        assert_eq!(
            chain(Some(&enc("gbk")), raw),
            Some("caf\u{6924}@".to_string())
        );
        assert_eq!(
            chain(Some(&enc("gb18030")), raw),
            Some("caf\u{6924}@".to_string())
        );
        // A known codec the kernel has no decoder for behaves like a failure
        // (divergence `D-CODEC`: Python decodes it).
        assert_eq!(chain(Some(&enc("utf-16")), raw), None);
        assert_eq!(chain(Some(&enc("big5hkscs")), raw), None);
    }

    #[test]
    fn test_round_trip_parity_for_sampled_code_points() {
        // (codec, code point, bytes) - `python -c "...ch.encode(c)==bytes..."` all
        // round-tripped True in CPython (probe_tests.py output).
        let cases: &[(&str, char, &[u8])] = &[
            ("utf-8", 'A', b"A"),
            ("utf-8", '\u{e9}', b"\xc3\xa9"),
            ("utf-8", '\u{4e2d}', b"\xe4\xb8\xad"),
            ("utf-8", '\u{1f600}', b"\xf0\x9f\x98\x80"),
            ("utf-8", '\u{fffd}', b"\xef\xbf\xbd"),
            ("utf-8-sig", 'A', b"\xef\xbb\xbfA"),
            ("utf-8-sig", '\u{4e2d}', b"\xef\xbb\xbf\xe4\xb8\xad"),
            ("ascii", '~', b"~"),
            ("iso8859-1", '\u{ff}', b"\xff"),
            ("iso8859-1", '\u{a0}', b"\xa0"),
            ("cp1252", '\u{20ac}', b"\x80"),
            ("cp1252", '\u{2019}', b"\x92"),
            ("cp1252", '\u{ff}', b"\xff"),
            ("gb18030", 'A', b"A"),
            ("gb18030", '\u{4e2d}', b"\xd6\xd0"),
            ("gb18030", '\u{3000}', b"\xa1\xa1"),
            ("gb18030", '\u{e4c6}', b"\xa1\x40"),
            ("gb18030", '\u{20000}', b"\x95\x32\x82\x36"),
            ("gb18030", '\u{ffe5}', b"\xa3\xa4"),
            ("gbk", '\u{4e2d}', b"\xd6\xd0"),
            ("gbk", '\u{3000}', b"\xa1\xa1"),
            ("gbk", '\u{ffe5}', b"\xa3\xa4"),
            ("big5", 'A', b"A"),
            ("big5", '\u{4e2d}', b"\xa4\xa4"),
            ("big5", '\u{6587}', b"\xa4\xe5"),
            ("big5", '\u{8a66}', b"\xb8\xd5"),
            ("big5", '\u{ff41}', b"\xa2\xe9"),
        ];
        for (codec, ch, bytes) in cases {
            assert_eq!(
                decode_with_codec(codec, bytes),
                Some(ch.to_string()),
                "{} <- {}",
                codec,
                hex_escape(bytes)
            );
        }
        // cp1252's five undefined slots are the only bytes that cannot round-trip.
        for b in [0x81u8, 0x8D, 0x8F, 0x90, 0x9D] {
            assert_eq!(decode_with_codec("cp1252", &[b]), None, "{:#04X}", b);
        }
    }

    // The generated-table shape self-test that used to sit here
    // (`test_generated_tables_match_measured_counts`) moved with the tables to
    // `codecs::tests::test_generated_tables_match_measured_counts`, which pins
    // every count (126x190 gb18030 doubles / 23940 assigned, 88x157 big5 /
    // 13710, 207 disjoint ordered 4-byte runs / 1087996 of 1587600, 43 gbk-hole
    // rectangles / 2149 codes, 5 undefined cp1252 slots, 47 measured
    // spellings) and adds the cp437 bijection.

    #[test]
    fn test_non_str_encoding_is_pythons_type_error_not_an_inline_message() {
        // `bytes.decode(True)` raises `TypeError`, which the candidate loop does
        // not catch, so the whole request fails.
        assert_eq!(
            python_decode_chain(Some(&AttributeValue::Bool(true)), b"x"),
            Err("bool")
        );
        assert_eq!(
            python_decode_chain(Some(&AttributeValue::Int(8)), b"x"),
            Err("int")
        );
        assert_eq!(
            python_decode_chain(
                Some(&AttributeValue::Array(vec![AttributeValue::Int(1)])),
                b"x"
            ),
            Err("list")
        );
        // Falsy values take the *default* chain (`if user_enc else [...]`).
        for falsy in [
            AttributeValue::String(String::new()),
            AttributeValue::Bool(false),
            AttributeValue::Int(0),
            AttributeValue::Array(Vec::new()),
        ] {
            assert_eq!(
                chain(Some(&falsy), b"# title\nbody\n"),
                Some("# title\nbody\n".to_string()),
                "{:?}",
                falsy
            );
        }
    }

    #[test]
    fn test_gb18030_file_imports_end_to_end() {
        // Regression for W4-04 as the user sees it: this file used to come back as
        // `unsupported_encoding`.
        let dir = scratch_dir("gb18030file");
        let raw: Vec<u8> = vec![0xC4, 0xE3, 0xBA, 0xC3, 0x0A];
        fs::write(dir.join("chapter.txt"), &raw).unwrap();
        let out = process_markdown_imports(
            "@import \"chapter.txt\"\n",
            &dir.to_string_lossy(),
            None,
            None,
            None,
        )
        .unwrap();
        assert!(
            !out.contains("unsupported_encoding"),
            "Python decodes this file: {}",
            out
        );
        assert!(out.contains("\u{4f60}\u{597d}"), "{}", out);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_type_error_encoding_aborts_the_request() {
        let dir = scratch_dir("enctypeerr");
        fs::write(dir.join("chapter.txt"), b"plain ascii\n").unwrap();
        let err = process_markdown_imports(
            "@import \"chapter.txt\" {encoding=true}\n",
            &dir.to_string_lossy(),
            None,
            None,
            None,
        )
        .unwrap_err();
        assert_eq!(err.error_code, ImportErrorCode::ImportUnsupportedEncoding);
        // readmd.py:1964-1966 collapses this into
        // `{'ok': False, 'error_code': 'import_process_failed'}` at HTTP 500.
        assert_eq!(
            err.code,
            "decode() argument 'encoding' must be str, not bool"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    /// Lower-case hex of a byte slice, for assertion messages.
    fn hex_escape(raw: &[u8]) -> String {
        raw.iter()
            .map(|b| format!("{:02x}", b))
            .collect::<Vec<_>>()
            .join("")
    }

    /// Differential check against CPython over a generated corpus
    /// (`python scratch/rust_parity/we8/gen_diff_vectors.py`).  Skips when the
    /// scratch fixture is not present, so the crate never depends on it.
    #[test]
    fn test_diff_corpus_against_cpython() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../scratch/rust_parity/we8/_diff_vectors.txt");
        let text = match fs::read_to_string(&fixture) {
            Ok(text) => text,
            Err(_) => {
                eprintln!("diff corpus absent, skipped");
                return;
            }
        };
        let mut codec = "chain";
        let mut lines = 0usize;
        let mut bad: Vec<String> = Vec::new();
        for line in text.lines() {
            if line.is_empty() {
                continue;
            }
            if let Some(name) = line.strip_prefix("#codec ") {
                codec = name;
                continue;
            }
            if line.starts_with('#') {
                continue;
            }
            lines += 1;
            let mut parts = line.split('|');
            let (hexin, verdict, hexout) = (
                parts.next().unwrap(),
                parts.next().unwrap(),
                parts.next().unwrap_or(""),
            );
            let raw: Vec<u8> = (0..hexin.len())
                .step_by(2)
                .map(|k| u8::from_str_radix(&hexin[k..k + 2], 16).unwrap())
                .collect();
            let expect: Vec<u8> = (0..hexout.len())
                .step_by(2)
                .map(|k| u8::from_str_radix(&hexout[k..k + 2], 16).unwrap())
                .collect();
            let got: Option<String> = if codec == "chain" {
                python_decode_chain(None, &raw).unwrap()
            } else {
                decode_with_codec(codec, &raw)
            };
            let ok = match (&got, verdict) {
                (None, "NONE") => true,
                (None, "ERR") => true,
                (Some(text), verdict2) if verdict2 != "NONE" && verdict2 != "ERR" => {
                    // The chain reports which codec won; the bytes are what matter.
                    text.as_bytes() == expect.as_slice()
                }
                (Some(text), "OK") => text.as_bytes() == expect.as_slice(),
                _ => false,
            };
            if !ok && bad.len() < 12 {
                bad.push(format!("{} {} {}|{}", codec, hexin, verdict, hexout));
            }
        }
        eprintln!("diff corpus: {} lines, {} mismatches", lines, bad.len());
        for b in &bad {
            eprintln!("  mismatch: {}", b);
        }
        assert!(bad.is_empty(), "{} corpus mismatches", bad.len());
    }
}

// --------------------------------------------------------------------------
// The generated CJK / cp1252 code tables that used to live here now live in
// `src/codecs.rs` (`crate::codecs`), together with WE8's CPython-measured
// corpus replay and the table-shape self test
// (`codecs::tests::test_generated_tables_match_measured_counts`).
