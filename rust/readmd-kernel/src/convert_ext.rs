//! Rust port of `src/readmd_modules/convert_ext.py` (lane `convert-ext-s1`).
//!
//! This module replicates, line-for-line, the pure-stdlib extended converters
//! that the Python authority ships: the FlyingMouse-ported EPUB -> Markdown
//! pipeline (`epub_to_markdown` / `html_to_markdown_clean`), the word-coordinate
//! grid clustering used to recover complex PDF tables
//! (`cluster_words_into_table` / `extract_page_table_cluster` /
//! `ClusteredTableProxy`) and the audio/video transcription wrapper
//! (`media_to_markdown`).
//!
//! Fidelity notes for this port:
//!   * Python `re.M`/back-references are handled by hand where the `regex` crate
//!     cannot express them (`<(b|strong)...</\1>` uses a back-reference).
//!   * Python `str.strip()` treats U+001C..U+001F as whitespace; Rust's
//!     `str::trim` does not.  [`py_strip`] reproduces CPython exactly.
//!   * EPUB is read as raw `Vec<u8>`/`&[u8]`; a `zipfile.is_zipfile` gate and a
//!     hand-written central-directory reader stand in for `zipfile`.
//!   * The Python `epub_to_markdown` first delegates to
//!     `rich_documents.epub_to_md` (a bs4/markdownify implementation).  bs4 and
//!     markdownify are Python-only, so a pure-Rust process always falls through
//!     the `except ImportError` branch to the pure-stdlib body, which is what is
//!     implemented here.  The bs4 delegate is separately ported (under a
//!     different name) as the private `convert::epub_to_md`.

use std::sync::OnceLock;

use regex::Regex;

use crate::convert::{basename, dirname};

// ---------------------------------------------------------------------------
// CPython string-model helpers
// ---------------------------------------------------------------------------

/// `str.isspace()` for a single code point (CPython `Py_UNICODE_ISSPACE`).
///
/// Notably wider than Rust's `char::is_whitespace`: it includes the ASCII
/// information separators U+001C..U+001F while excluding nothing that Rust
/// includes.
pub fn py_isspace(c: char) -> bool {
    matches!(
        c as u32,
        0x09..=0x0d
            | 0x1c..=0x1f
            | 0x20
            | 0x85
            | 0xa0
            | 0x1680
            | 0x2000..=0x200a
            | 0x2028
            | 0x2029
            | 0x202f
            | 0x205f
            | 0x3000
    )
}

/// `s.strip()` — trim ASCII *and* Unicode whitespace from both ends.
pub fn py_strip(s: &str) -> &str {
    let mut start = 0usize;
    let bytes = s.as_bytes();
    // Find first non-space char (advance by char width, never split a code point).
    for (idx, ch) in s.char_indices() {
        if !py_isspace(ch) {
            start = idx;
            break;
        }
        start = idx + ch.len_utf8();
    }
    let mut end = s.len();
    // Trim trailing whitespace by scanning chars from the front (cheap, correct).
    let chars: Vec<(usize, char)> = s.char_indices().collect();
    for &(idx, ch) in chars.iter().rev() {
        if py_isspace(ch) {
            end = idx;
        } else {
            end = idx + ch.len_utf8();
            break;
        }
    }
    if start > end {
        // Everything was whitespace (or empty).
        let _ = bytes;
        return "";
    }
    &s[start..end]
}

/// Python `s[i:j]`: index by code point, clamp out-of-range, never panic and
/// never split a multi-byte character.  Negative start/stop are NOT handled here
/// (the ported code paths never use them); they are clamped to 0.
pub fn py_slice(s: &str, start: usize, stop: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    let n = chars.len();
    let a = start.min(n);
    let b = stop.min(n);
    if a >= b {
        return String::new();
    }
    chars[a..b].iter().collect()
}

/// `bytes.decode('utf-8', errors='ignore')` — drop every byte that is not part of
/// a valid UTF-8 sequence.  Rust's `from_utf8_lossy` would instead insert U+FFFD,
/// which is observable, so we ignore.
fn utf8_ignore(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    let mut i = 0usize;
    while i < bytes.len() {
        match std::str::from_utf8(&bytes[i..]) {
            Ok(s) => {
                out.push_str(s);
                break;
            }
            Err(e) => {
                let valid = e.valid_up_to();
                if valid > 0 {
                    out.push_str(std::str::from_utf8(&bytes[i..i + valid]).unwrap());
                }
                match e.error_len() {
                    Some(skip) => i += valid + skip,
                    None => break, // unexpected end of input
                }
            }
        }
    }
    out
}

/// Total ordering for coordinates so `sort_by` never has to unwrap `None` the way
/// `partial_cmp(...).unwrap()` would panic on NaN.  Equal values keep their
/// insertion order (Rust's `sort_by` is stable, matching Python's `sorted`).
fn cmp_f64(a: f64, b: f64) -> std::cmp::Ordering {
    a.partial_cmp(&b).unwrap_or(std::cmp::Ordering::Equal)
}

// ---------------------------------------------------------------------------
// Word / table data model
// ---------------------------------------------------------------------------

/// A single PDF word as consumed by [`cluster_words_into_table`] — mirrors the
/// `dict(x, y, width, height, text)` the Python builds.  `width`/`height` are
/// carried for fidelity but the clustering only reads `x`, `y` and `text`.
#[derive(Clone, Debug, PartialEq)]
pub struct Word {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub text: String,
}

impl Word {
    /// Convenience constructor matching the Python dict literal.
    pub fn new(x: f64, y: f64, width: f64, height: f64, text: &str) -> Self {
        Word { x, y, width, height, text: text.to_string() }
    }
}

/// One `page.get_text('words')` record: `[x0, y0, x1, y1, text, ...]`.
#[derive(Clone, Debug, PartialEq)]
pub struct RawWord {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
    pub text: String,
}

impl RawWord {
    pub fn new(x0: f64, y0: f64, x1: f64, y1: f64, text: &str) -> Self {
        RawWord { x0, y0, x1, y1, text: text.to_string() }
    }
}

/// `ClusteredTableProxy` — a tiny PyMuPDF-`Table`-shaped wrapper around the
/// clustering result so callers can treat it like a real table object.
#[derive(Clone, Debug, PartialEq)]
pub struct ClusteredTableProxy {
    md: String,
    /// `(min_x, min_y, max_x, max_y)` in PDF points.
    pub bbox: (f64, f64, f64, f64),
}

impl ClusteredTableProxy {
    /// `Table.extract()` — always empty for a synthetic clustered table.
    pub fn extract(&self) -> Vec<String> {
        Vec::new()
    }
    /// `Table.get_markdown()`.
    pub fn get_markdown(&self) -> &str {
        &self.md
    }
}

// ---------------------------------------------------------------------------
// cluster_words_into_table
// ---------------------------------------------------------------------------

/// The Python default `y_tolerance` for [`cluster_words_into_table`].
pub const DEFAULT_Y_TOLERANCE: f64 = 3.0;

/// Port of `convert_ext.cluster_words_into_table`.
///
/// Clusters PDF words into rows by their Y centre, greedily detects columns by a
/// 20pt X-gap, aligns every word into the nearest column, and renders a GFM
/// Markdown table.  Returns a plain text run when the geometry does not form a
/// table (see the two early-return branches).
pub fn cluster_words_into_table(words: &[Word], y_tolerance: f64) -> String {
    if words.is_empty() {
        return String::new();
    }

    // 1. Row clustering (stable sort by y, exactly like `sorted(words, key=y)`).
    let mut sorted_idx: Vec<usize> = (0..words.len()).collect();
    sorted_idx.sort_by(|&a, &b| cmp_f64(words[a].y, words[b].y));

    let tolerance = if y_tolerance > 6.0 { y_tolerance } else { 6.0 }; // max(y_tolerance, 6.0)

    let mut rows: Vec<Vec<usize>> = Vec::new();
    let mut current_row: Vec<usize> = vec![sorted_idx[0]];
    for &wi in &sorted_idx[1..] {
        // row_y = mean of current_row's y
        let sum: f64 = current_row.iter().map(|&i| words[i].y).fold(0.0, |a, b| a + b);
        let row_y = sum / (current_row.len() as f64);
        if (words[wi].y - row_y).abs() <= tolerance {
            current_row.push(wi);
        } else {
            current_row.sort_by(|&a, &b| cmp_f64(words[a].x, words[b].x));
            rows.push(std::mem::take(&mut current_row));
            current_row = vec![wi];
        }
    }
    if !current_row.is_empty() {
        current_row.sort_by(|&a, &b| cmp_f64(words[a].x, words[b].x));
        rows.push(current_row);
    }

    // Not a table yet: join the ORIGINAL input order (not the y-sorted order).
    if rows.len() < 2 {
        return words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" ");
    }

    // 2. Dynamic column detection.
    let mut all_x_starts: Vec<f64> = Vec::new();
    for row in &rows {
        for &wi in row {
            all_x_starts.push(words[wi].x);
        }
    }
    all_x_starts.sort_by(|a, b| cmp_f64(*a, *b));

    let mut col_splits: Vec<f64> = vec![all_x_starts[0]];
    for &x in &all_x_starts[1..] {
        if x - col_splits[col_splits.len() - 1] > 20.0 {
            col_splits.push(x);
        }
    }
    let num_cols = col_splits.len();

    if num_cols < 2 {
        // Single column: emit one text line per row, rows separated by blank line.
        return rows
            .iter()
            .map(|row| {
                row.iter()
                    .map(|&wi| words[wi].text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect::<Vec<_>>()
            .join("\n\n");
    }

    // 3. Grid alignment/fill.
    let mut grid = vec![vec![String::new(); num_cols]; rows.len()];
    for (r_idx, row) in rows.iter().enumerate() {
        for &wi in row {
            let mut best_c = 0usize;
            let mut min_dist = f64::INFINITY;
            for (c_idx, &c_x) in col_splits.iter().enumerate() {
                let dist = (words[wi].x - c_x).abs();
                if dist < min_dist {
                    min_dist = dist;
                    best_c = c_idx;
                }
            }
            // (existing + " " + text).strip()
            let joined = format!("{} {}", grid[r_idx][best_c], words[wi].text);
            grid[r_idx][best_c] = py_strip(&joined).to_string();
        }
    }

    // 4. Render the Markdown table.
    let mut md_table_lines: Vec<String> = Vec::new();
    let header_cells: Vec<&str> = grid[0].iter().map(|c| if c.is_empty() { "-" } else { c.as_str() }).collect();
    md_table_lines.push(format!("| {} |", header_cells.join(" | ")));
    let divider = format!("| {} |", vec!["---"; num_cols].join(" | "));
    md_table_lines.push(divider);

    for row_cells in &grid[1..] {
        let any_nonblank = row_cells.iter().any(|cell| !py_strip(cell).is_empty());
        if !any_nonblank {
            continue;
        }
        let rendered: Vec<String> = row_cells
            .iter()
            .map(|cell| {
                if cell.is_empty() {
                    " ".to_string()
                } else {
                    cell.replace('|', "\\|")
                }
            })
            .collect();
        md_table_lines.push(format!("| {} |", rendered.join(" | ")));
    }

    md_table_lines.join("\n")
}

// ---------------------------------------------------------------------------
// extract_page_table_cluster
// ---------------------------------------------------------------------------

/// Port of `convert_ext.extract_page_table_cluster`.
///
/// The Python takes a PyMuPDF `page` and only ever reads `page.get_text('words')`;
/// we accept that word list directly (there is no live PDF page in a pure-Rust
/// binary).  Returns a [`ClusteredTableProxy`] when the geometry forms a table,
/// otherwise `None`.  Any failure that would raise inside the Python `try/except`
/// collapses to `None` here.
pub fn extract_page_table_cluster(raw_words: &[RawWord]) -> Option<ClusteredTableProxy> {
    if raw_words.is_empty() || raw_words.len() < 6 {
        return None;
    }

    // Sort by y0 (index 1), stable.
    let mut order: Vec<usize> = (0..raw_words.len()).collect();
    order.sort_by(|&a, &b| cmp_f64(raw_words[a].y0, raw_words[b].y0));

    let mut candidate_rows: Vec<Vec<usize>> = Vec::new();
    let mut cur_row: Vec<usize> = vec![order[0]];
    for &wi in &order[1..] {
        let sum: f64 = cur_row.iter().map(|&i| raw_words[i].y0).fold(0.0, |a, b| a + b);
        let row_y = sum / (cur_row.len() as f64);
        if (raw_words[wi].y0 - row_y).abs() <= 6.0 {
            cur_row.push(wi);
        } else {
            candidate_rows.push(std::mem::take(&mut cur_row));
            cur_row = vec![wi];
        }
    }
    if !cur_row.is_empty() {
        candidate_rows.push(cur_row);
    }

    // Keep only rows that look like they have >=2 well-separated columns.
    let mut table_rows: Vec<Vec<usize>> = Vec::new();
    for r in &candidate_rows {
        if r.len() >= 2 {
            let mut sorted_r: Vec<usize> = r.clone();
            sorted_r.sort_by(|&a, &b| cmp_f64(raw_words[a].x0, raw_words[b].x0));
            let mut has_gap = false;
            for pair in sorted_r.windows(2) {
                // gap = next.x0 - prev.x1
                let gap = raw_words[pair[1]].x0 - raw_words[pair[0]].x1;
                if gap > 20.0 {
                    has_gap = true;
                    break;
                }
            }
            if has_gap {
                table_rows.push(r.clone());
            }
        }
    }

    if table_rows.len() < 2 {
        return None;
    }

    let table_words: Vec<usize> = table_rows.iter().flatten().copied().collect();
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for &wi in &table_words {
        let w = &raw_words[wi];
        if w.x0 < min_x {
            min_x = w.x0;
        }
        if w.y0 < min_y {
            min_y = w.y0;
        }
        if w.x1 > max_x {
            max_x = w.x1;
        }
        if w.y1 > max_y {
            max_y = w.y1;
        }
    }

    let words: Vec<Word> = table_words
        .iter()
        .map(|&wi| {
            let w = &raw_words[wi];
            Word::new(w.x0, w.y0, w.x1 - w.x0, w.y1 - w.y0, &w.text)
        })
        .collect();

    let md = cluster_words_into_table(&words, DEFAULT_Y_TOLERANCE);
    if !md.is_empty() && md.contains("| ---") {
        return Some(ClusteredTableProxy { md, bbox: (min_x, min_y, max_x, max_y) });
    }
    None
}

// ---------------------------------------------------------------------------
// html_to_markdown_clean
// ---------------------------------------------------------------------------

fn re_static(slot: &'static OnceLock<Regex>, pat: &str) -> &'static Regex {
    slot.get_or_init(|| Regex::new(pat).expect("convert_ext regex must compile"))
}

static RE_SCRIPT: OnceLock<Regex> = OnceLock::new();
static RE_STYLE: OnceLock<Regex> = OnceLock::new();
static RE_P: OnceLock<Regex> = OnceLock::new();
static RE_BR: OnceLock<Regex> = OnceLock::new();
static RE_LI: OnceLock<Regex> = OnceLock::new();
static RE_TAGSTRIP: OnceLock<Regex> = OnceLock::new();
static RE_NL3: OnceLock<Regex> = OnceLock::new();
static RE_HEAD: [OnceLock<Regex>; 6] = [
    OnceLock::new(),
    OnceLock::new(),
    OnceLock::new(),
    OnceLock::new(),
    OnceLock::new(),
    OnceLock::new(),
];

/// Case-insensitive compare of `word` (ASCII lower) against `chars[start..]`.
fn matches_ci(chars: &[char], start: usize, word: &[char]) -> bool {
    if start + word.len() > chars.len() {
        return false;
    }
    word.iter()
        .enumerate()
        .all(|(k, wc)| chars[start + k].eq_ignore_ascii_case(wc))
}

fn find_char(chars: &[char], start: usize, target: char) -> Option<usize> {
    (start..chars.len()).find(|&i| chars[i] == target)
}

/// Hand-rolled stand-in for `re.sub(r'<(A|B)[^>]*>([\s\S]*?)</\1>', repl, flags=re.I)`.
///
/// The `regex` crate has no back-references, so the closing tag is matched by
/// hand: the captured group is the first of `(a, b)` that case-insensitively
/// follows `<`, and the terminator is `</` + that same word + `>` (also
/// case-insensitive, because `re.I` applies to the back-reference).
fn sub_emphasis(html: &str, a: &str, b: &str, marker: &str) -> String {
    let chars: Vec<char> = html.chars().collect();
    let n = chars.len();
    let ca: Vec<char> = a.chars().collect();
    let cb: Vec<char> = b.chars().collect();
    let mut out: Vec<char> = Vec::with_capacity(n);
    let mut i = 0usize;
    while i < n {
        if chars[i] != '<' {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        // Ordered alternation: try `a` first, then `b` (matches Python `(a|b)`).
        let word: Option<(&[char], usize)> = if matches_ci(&chars, i + 1, &ca) {
            Some((ca.as_slice(), i + 1 + ca.len()))
        } else if matches_ci(&chars, i + 1, &cb) {
            Some((cb.as_slice(), i + 1 + cb.len()))
        } else {
            None
        };
        if let Some((w, after)) = word {
            // attrs `[^>]*` then '>'; content starts after that '>'.
            if let Some(gt) = find_char(&chars, after, '>') {
                let content_start = gt + 1;
                // terminator '</' + word(ci) + '>'
                if let Some((close_start, close_end)) =
                    find_close(&chars, content_start, w)
                {
                    for m in marker.chars() {
                        out.push(m);
                    }
                    out.extend_from_slice(&chars[content_start..close_start]);
                    for m in marker.chars() {
                        out.push(m);
                    }
                    i = close_end;
                    continue;
                }
            }
        }
        out.push('<');
        i += 1;
    }
    out.into_iter().collect()
}

fn find_close(chars: &[char], start: usize, word: &[char]) -> Option<(usize, usize)> {
    let mut j = start;
    while j + 1 < chars.len() {
        if chars[j] == '<' && chars[j + 1] == '/' {
            let wpos = j + 2;
            if matches_ci(chars, wpos, word) {
                let after = wpos + word.len();
                if after < chars.len() && chars[after] == '>' {
                    return Some((j, after + 1));
                }
            }
        }
        j += 1;
    }
    None
}

/// Port of `convert_ext.html_to_markdown_clean`.
pub fn html_to_markdown_clean(html_content: &str) -> String {
    let mut text = html_content.to_string();

    // Drop <script> and <style> blocks.
    let r = re_static(&RE_SCRIPT, r"(?i)<script[\s\S]*?</script>");
    text = r.replace_all(&text, "").into_owned();
    let r = re_static(&RE_STYLE, r"(?i)<style[\s\S]*?</style>");
    text = r.replace_all(&text, "").into_owned();

    // Headings, h6 -> h1 (matching `for i in range(6, 0, -1)`).
    for i in (1..=6).rev() {
        let re = &RE_HEAD[i - 1];
        let pat = format!(r"(?i)<h{i}[^>]*>([\s\S]*?)</h{i}>");
        let r = re_static(re, &pat);
        let hashes = "#".repeat(i);
        let repl = format!("\n\n{hashes} $1\n\n");
        text = r.replace_all(&text, &repl).into_owned();
    }

    // Paragraphs and line breaks.
    let r = re_static(&RE_P, r"(?i)<p[^>]*>([\s\S]*?)</p>");
    text = r.replace_all(&text, "\n\n$1\n\n").into_owned();
    let r = re_static(&RE_BR, r"(?i)<br\s*/?>");
    text = r.replace_all(&text, "\n").into_owned();

    // List items.
    let r = re_static(&RE_LI, r"(?i)<li[^>]*>([\s\S]*?)</li>");
    text = r.replace_all(&text, "\n- $1").into_owned();

    // Bold / italic (back-reference => hand-rolled).
    text = sub_emphasis(&text, "b", "strong", "**");
    text = sub_emphasis(&text, "i", "em", "*");

    // Strip every remaining tag.
    let r = re_static(&RE_TAGSTRIP, r"<[^>]+>");
    text = r.replace_all(&text, "").into_owned();

    // Named-entity replacement (order-sensitive: &amp; is decoded *after* the
    // others are inserted, and can re-trigger the later replacements).
    text = text
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"");

    // Collapse runs of 3+ newlines to exactly two, then strip.
    let r = re_static(&RE_NL3, r"\n{3,}");
    text = r.replace_all(&text, "\n\n").into_owned();

    py_strip(&text).to_string()
}

// ---------------------------------------------------------------------------
// EPUB -> Markdown (pure-stdlib fallback of epub_to_markdown)
// ---------------------------------------------------------------------------

/// `os.path.normpath(os.path.join(base, href))` for the separators Windows
/// accepts, then forward-slash normalised.  Lexical only (never touches the
/// filesystem), matching Python's `ntpath.normpath`.
fn norm_join(base: &str, href: &str) -> String {
    let joined = if href.is_empty() {
        // ntpath.join('dir', '') == 'dir\\' (a trailing separator).
        if base.is_empty() {
            "/".to_string()
        } else {
            format!("{base}/")
        }
    } else if is_abs(href) {
        href.to_string()
    } else if base.is_empty() {
        href.to_string()
    } else {
        format!("{base}/{href}")
    };
    normpath(&joined)
}

fn is_abs(p: &str) -> bool {
    let b = p.as_bytes();
    if b.first() == Some(&b'/') || b.first() == Some(&b'\\') {
        return true;
    }
    // Drive-letter form `C:\` or `C:/`.
    b.len() >= 3
        && b[0].is_ascii_alphabetic()
        && b[1] == b':'
        && (b[2] == b'\\' || b[2] == b'/')
}

fn normpath(p: &str) -> String {
    let unified = p.replace('\\', "/");
    let leading_slash = unified.starts_with('/');
    let mut parts: Vec<&str> = Vec::new();
    for comp in unified.split('/') {
        if comp.is_empty() || comp == "." {
            continue;
        }
        if comp == ".." {
            match parts.last() {
                Some(&last) if last != ".." => {
                    parts.pop();
                }
                _ => {
                    if !leading_slash {
                        parts.push(comp);
                    }
                }
            }
            continue;
        }
        parts.push(comp);
    }
    let body = parts.join("/");
    if leading_slash {
        format!("/{body}")
    } else if body.is_empty() {
        ".".to_string()
    } else {
        body
    }
}

/// `zipfile.is_zipfile` — the presence of an end-of-central-directory record.
fn is_zipfile_bytes(data: &[u8]) -> bool {
    data.windows(4).any(|w| w == b"PK\x05\x06")
}

/// Read one little-endian u16/u32 from a byte slice.
fn rd_u16(b: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([b[off], b[off + 1]])
}
fn rd_u32(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
}

struct ZipEntry {
    name: String,
    method: u16,
    comp_size: usize,
    local_offset: usize,
}

/// Parse the ZIP central directory into ordered entries (matching
/// `zipfile.ZipFile.namelist()` order for freshly-archived files).
fn zip_entries(data: &[u8]) -> Result<Vec<ZipEntry>, String> {
    // Locate EOCD (last `PK\x05\x06`).
    let eocd = (0..data.len().saturating_sub(3))
        .rev()
        .find(|&i| data[i..].starts_with(b"PK\x05\x06"))
        .ok_or("no end of central directory")?;
    if eocd + 22 > data.len() {
        return Err("truncated EOCD".to_string());
    }
    let cd_count = rd_u16(data, eocd + 10) as usize;
    let cd_offset = rd_u32(data, eocd + 16) as usize;

    let mut entries = Vec::with_capacity(cd_count);
    let mut p = cd_offset;
    for _ in 0..cd_count {
        if p + 46 > data.len() || !data[p..].starts_with(b"PK\x01\x02") {
            break;
        }
        let method = rd_u16(data, p + 10);
        let comp_size = rd_u32(data, p + 20) as usize;
        let name_len = rd_u16(data, p + 28) as usize;
        let extra_len = rd_u16(data, p + 30) as usize;
        let comment_len = rd_u16(data, p + 32) as usize;
        let local_offset = rd_u32(data, p + 42) as usize;
        let name_start = p + 46;
        if name_start + name_len > data.len() {
            break;
        }
        let name = utf8_ignore(&data[name_start..name_start + name_len]);
        entries.push(ZipEntry { name, method, comp_size, local_offset });
        p = name_start + name_len + extra_len + comment_len;
    }
    Ok(entries)
}

/// `zf.read(name)` — the decompressed bytes for an entry (stored or deflate).
fn zip_read(data: &[u8], entry: &ZipEntry) -> Result<Vec<u8>, String> {
    let lo = entry.local_offset;
    if lo + 30 > data.len() || !data[lo..].starts_with(b"PK\x03\x04") {
        return Err("bad local header".to_string());
    }
    let lname = rd_u16(data, lo + 26) as usize;
    let lext = rd_u16(data, lo + 28) as usize;
    let ds = lo + 30 + lname + lext;
    if ds + entry.comp_size > data.len() {
        return Err("truncated file data".to_string());
    }
    let raw = &data[ds..ds + entry.comp_size];
    match entry.method {
        0 => Ok(raw.to_vec()),
        8 => {
            use std::io::Read;
            let mut dec = flate2::read::DeflateDecoder::new(raw);
            let mut out = Vec::new();
            dec.read_to_end(&mut out).map_err(|e| format!("deflate: {e}"))?;
            Ok(out)
        }
        _ => Err(format!("unsupported compression method {}", entry.method)),
    }
}

struct ZipArchive {
    data: Vec<u8>,
    entries: Vec<ZipEntry>,
}

impl ZipArchive {
    fn namelist(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.name.as_str()).collect()
    }
    fn find(&self, name: &str) -> Option<&ZipEntry> {
        self.entries.iter().find(|e| e.name == name)
    }
    fn read(&self, name: &str) -> Result<Vec<u8>, String> {
        let e = self.find(name).ok_or_else(|| format!("KeyError: {name}"))?;
        zip_read(&self.data, e)
    }
}

/// Locate an XML attribute value on the first `<tag>` element (a real token, not
/// a prefix such as `<rootfiles>`) that carries it.  Returns `None` when no such
/// element exists (mirrors the ElementTree `.attrib[key]` `KeyError` that the
/// Python lets fall through to its default).
fn first_tag_attr(xml: &str, tag: &str, attr: &str) -> Option<String> {
    let open = format!("<{tag}");
    let mut pos = 0usize;
    while let Some(rel) = xml[pos..].find(&open) {
        let start = pos + rel;
        let after = start + open.len();
        // Require a token boundary after `<tag`: whitespace, '/', or '>'.
        match xml.as_bytes().get(after) {
            Some(c @ (b' ' | b'\t' | b'\n' | b'\r' | b'/' | b'>')) => {
                let _ = c;
                let rest = &xml[after..];
                if let Some(tag_end) = rest.find('>') {
                    let head = &rest[..tag_end];
                    if let Some(v) = extract_attr(head, attr) {
                        return Some(v);
                    }
                }
                // Token found but attribute absent: keep scanning further tags.
                pos = after;
            }
            _ => {
                // e.g. `<rootfiles>` when looking for `rootfile`.
                pos = after;
            }
        }
    }
    None
}

/// Pull `attr="value"` / `attr='value'` from a single tag header string.
fn extract_attr(head: &str, attr: &str) -> Option<String> {
    let bytes = head.as_bytes();
    let name = attr.as_bytes();
    let mut i = 0usize;
    while i + name.len() <= bytes.len() {
        // Require a delimiter before the attribute name.
        let delim_ok = i == 0 || matches!(bytes[i - 1], b' ' | b'\t' | b'\n' | b'\r' | b'"' | b'\'');
        if delim_ok && &bytes[i..i + name.len()] == name {
            let mut j = i + name.len();
            while j < bytes.len() && matches!(bytes[j], b' ' | b'\t' | b'\n' | b'\r') {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b'=' {
                j += 1;
                while j < bytes.len() && matches!(bytes[j], b' ' | b'\t' | b'\n' | b'\r') {
                    j += 1;
                }
                if j < bytes.len() && (bytes[j] == b'"' || bytes[j] == b'\'') {
                    let q = bytes[j];
                    let vstart = j + 1;
                    if let Some(rel) = bytes[vstart..].iter().position(|&c| c == q) {
                        let vend = vstart + rel;
                        if let Ok(s) = std::str::from_utf8(&bytes[vstart..vend]) {
                            return Some(s.to_string());
                        }
                    }
                }
            }
        }
        i += 1;
    }
    None
}

/// Iterate `<tag ...>` headers occurring inside the `<outer>...</outer>` region.
fn tags_in_region(xml: &str, outer: &str, tag: &str) -> Vec<String> {
    let mut out = Vec::new();
    let oopen = format!("<{outer}");
    let oclose = format!("</{outer}");
    let (region, from, to) = match (xml.find(&oopen), xml.find(&oclose)) {
        (Some(a), Some(b)) if b > a => {
            let start = a + oopen.len();
            (&xml[start..b], start, b)
        }
        _ => return out,
    };
    let _ = (from, to);
    let topen = format!("<{tag}");
    let mut pos = 0usize;
    while let Some(rel) = region[pos..].find(&topen) {
        let start = pos + rel;
        // Must be a full token: next byte after `<tag` is whitespace, '/' or '>'.
        let nb = region.as_bytes().get(start + topen.len());
        let is_token = matches!(nb, Some(b' ' | b'\t' | b'\n' | b'\r' | b'/' | b'>'));
        if !is_token {
            pos = start + topen.len();
            continue;
        }
        let gt = match region[start..].find('>') {
            Some(g) => start + g,
            None => break,
        };
        let slash = match region[start..].find("/>") {
            Some(s) => start + s,
            None => usize::MAX,
        };
        // If the '>' we found is actually the '>', the header is region[start..=gt].
        let _ = slash;
        out.push(region[start..=gt].to_string());
        pos = gt + 1;
    }
    out
}

/// Port of `convert_ext.epub_to_markdown`'s pure-stdlib body.
///
/// Returns `Err("无效的 EPUB 文件")` for a non-archival input, mirroring the
/// `ValueError` the Python raises when `zipfile.is_zipfile` fails.  Other I/O
/// failures surface as `Err` too (the Python lets them propagate).
pub fn epub_to_markdown(epub_path: &str) -> Result<String, String> {
    let data = std::fs::read(epub_path).map_err(|_| "无效的 EPUB 文件".to_string())?;
    if !is_zipfile_bytes(&data) {
        return Err("无效的 EPUB 文件".to_string());
    }
    let entries = zip_entries(&data)?;
    let zf = ZipArchive { data, entries };

    let mut md_parts: Vec<String> = Vec::new();

    // 1. container.xml -> opf_path (default 'content.opf' on any failure).
    let opf_path = match zf.read("META-INF/container.xml") {
        Ok(container) => {
            let txt = utf8_ignore(&container);
            first_tag_attr(&txt, "rootfile", "full-path").unwrap_or_else(|| "content.opf".to_string())
        }
        Err(_) => "content.opf".to_string(),
    };
    let opf_dir = dirname(&opf_path);

    // 2. OPF spine (fall back to scanning the namelist on any parse failure).
    let mut used_spine = true;
    let opf_bytes = match zf.read(&opf_path) {
        Ok(b) => b,
        Err(_) => {
            used_spine = false;
            Vec::new()
        }
    };
    if used_spine {
        let opf = utf8_ignore(&opf_bytes);
        // manifest id -> href; any item missing id/href aborts to fallback.
        let mut manifest: Vec<(String, String)> = Vec::new();
        for item in tags_in_region(&opf, "manifest", "item") {
            match (extract_attr(&item, "id"), extract_attr(&item, "href")) {
                (Some(id), Some(href)) => manifest.push((id, href)),
                _ => {
                    used_spine = false;
                    break;
                }
            }
        }
        if used_spine {
            for itemref in tags_in_region(&opf, "spine", "itemref") {
                let idref = match extract_attr(&itemref, "idref") {
                    Some(v) => v,
                    None => {
                        used_spine = false;
                        break;
                    }
                };
                if let Some((_, href)) = manifest.iter().find(|(id, _)| *id == idref) {
                    let doc_path = norm_join(&opf_dir, href);
                    if zf.find(&doc_path).is_some() {
                        if let Ok(bytes) = zf.read(&doc_path) {
                            let ch_md = html_to_markdown_clean(&utf8_ignore(&bytes));
                            if !ch_md.is_empty() {
                                md_parts.push(ch_md);
                            }
                        }
                    }
                }
            }
        }
    }

    if !used_spine {
        md_parts.clear();
        // Fallback: iterate every html/xhtml/htm member in namelist order.
        for name in zf.namelist() {
            if (name.ends_with(".html") || name.ends_with(".xhtml") || name.ends_with(".htm"))
                && !name.starts_with("__")
            {
                if let Ok(bytes) = zf.read(name) {
                    let ch_md = html_to_markdown_clean(&utf8_ignore(&bytes));
                    if !ch_md.is_empty() {
                        md_parts.push(ch_md);
                    }
                }
            }
        }
    }

    Ok(md_parts.join("\n\n---\n\n"))
}

// ---------------------------------------------------------------------------
// media_to_markdown
// ---------------------------------------------------------------------------

/// Port of `convert_ext.media_to_markdown`.
///
/// Python obtains the transcript text via `transcribe.transcribe(media_path)`,
/// but that attribute does not exist on the Python `transcribe` module (it only
/// exposes `transcribe_to_md`), and audio transcription is a runtime/whisper
/// concern that neither `transcribe.transcribe` nor the Rust `transcribe` module
/// can perform synchronously.  We therefore take the transcript text — exactly the
/// value Python binds to the local `text` — as a parameter and reproduce the
/// deterministic Markdown wrapper (basename + header + blockquote) verbatim.
pub fn media_to_markdown(media_path: &str, transcript: &str) -> String {
    let base_name = basename(media_path);
    format!("# 多媒体录音转写纪要: {base_name}\n\n> 来源文件: `{media_path}`\n\n{transcript}")
}

// ===========================================================================
// Tests — derived from the Python ground-truth in oracle.json
// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn w(x: f64, y: f64, text: &str) -> Word {
        Word::new(x, y, 10.0, 8.0, text)
    }
    fn wbox(x0: f64, y0: f64, x1: f64, y1: f64, text: &str) -> RawWord {
        RawWord::new(x0, y0, x1, y1, text)
    }

    // ---- cluster_words_into_table (oracle.json: cluster) ----

    #[test]
    fn cluster_empty_is_empty_string() {
        assert_eq!(cluster_words_into_table(&[], DEFAULT_Y_TOLERANCE), "");
    }

    #[test]
    fn cluster_single_word_joined_in_input_order() {
        let ws = vec![w(0.0, 0.0, "hello")];
        assert_eq!(cluster_words_into_table(&ws, DEFAULT_Y_TOLERANCE), "hello");
    }

    #[test]
    fn cluster_one_row_uses_original_order_not_sorted() {
        let ws = vec![w(50.0, 0.0, "beta"), w(10.0, 0.0, "alpha"), w(30.0, 0.0, "gamma")];
        assert_eq!(cluster_words_into_table(&ws, DEFAULT_Y_TOLERANCE), "beta alpha gamma");
    }

    #[test]
    fn cluster_grid_2x3() {
        let ws = vec![
            w(0.0, 0.0, "A"), w(100.0, 0.0, "B"), w(220.0, 0.0, "C"),
            w(0.0, 20.0, "1"), w(100.0, 20.0, "2"), w(220.0, 20.0, "3"),
        ];
        assert_eq!(
            cluster_words_into_table(&ws, DEFAULT_Y_TOLERANCE),
            "| A | B | C |\n| --- | --- | --- |\n| 1 | 2 | 3 |"
        );
    }

    #[test]
    fn cluster_missing_cell_header_dash_body_space() {
        let ws = vec![
            w(0.0, 0.0, "h0"), w(100.0, 0.0, "h100"), w(220.0, 0.0, "h220"),
            w(0.0, 20.0, "r1c0"), w(220.0, 20.0, "r1c2"),
        ];
        assert_eq!(
            cluster_words_into_table(&ws, DEFAULT_Y_TOLERANCE),
            "| h0 | h100 | h220 |\n| --- | --- | --- |\n| r1c0 |   | r1c2 |"
        );
    }

    #[test]
    fn cluster_same_column_concatenates() {
        let ws = vec![
            w(0.0, 0.0, "left1"), w(4.0, 0.0, "left2"), w(100.0, 0.0, "right"),
            w(0.0, 20.0, "a"), w(100.0, 20.0, "b"),
        ];
        assert_eq!(
            cluster_words_into_table(&ws, DEFAULT_Y_TOLERANCE),
            "| left1 left2 | right |\n| --- | --- |\n| a | b |"
        );
    }

    #[test]
    fn cluster_tiny_gap_is_single_column_text() {
        let ws = vec![
            w(0.0, 0.0, "aa"), w(10.0, 0.0, "bb"),
            w(0.0, 20.0, "cc"), w(10.0, 20.0, "dd"),
        ];
        assert_eq!(cluster_words_into_table(&ws, DEFAULT_Y_TOLERANCE), "aa bb\n\ncc dd");
    }

    #[test]
    fn cluster_boundary_6_0_same_row() {
        let ws = vec![
            w(0.0, 0.0, "t1"), w(100.0, 0.0, "t2"),
            w(0.0, 6.0, "t3"), w(100.0, 6.0, "t4"),
            w(0.0, 20.0, "t5"), w(100.0, 20.0, "t6"),
        ];
        assert_eq!(
            cluster_words_into_table(&ws, DEFAULT_Y_TOLERANCE),
            "| t1 t3 | t2 t4 |\n| --- | --- |\n| t5 | t6 |"
        );
    }

    #[test]
    fn cluster_boundary_6_01_splits_rows() {
        let ws = vec![
            w(0.0, 0.0, "t1"), w(100.0, 0.0, "t2"),
            w(0.0, 6.01, "t3"), w(100.0, 6.01, "t4"),
            w(0.0, 20.0, "t5"), w(100.0, 20.0, "t6"),
        ];
        assert_eq!(
            cluster_words_into_table(&ws, DEFAULT_Y_TOLERANCE),
            "| t1 | t2 |\n| --- | --- |\n| t3 | t4 |\n| t5 | t6 |"
        );
    }

    #[test]
    fn cluster_nonlatin_and_pipe_escape() {
        let ws = vec![
            w(0.0, 0.0, "名"), w(100.0, 0.0, "字"),
            w(0.0, 20.0, "a|b"), w(100.0, 20.0, "普通"),
        ];
        assert_eq!(
            cluster_words_into_table(&ws, DEFAULT_Y_TOLERANCE),
            "| 名 | 字 |\n| --- | --- |\n| a\\|b | 普通 |"
        );
    }

    #[test]
    fn cluster_space_only_cell_becomes_blank() {
        let ws = vec![
            w(0.0, 0.0, "H1"), w(100.0, 0.0, "H2"),
            w(0.0, 20.0, " "), w(100.0, 20.0, "x"),
        ];
        assert_eq!(
            cluster_words_into_table(&ws, DEFAULT_Y_TOLERANCE),
            "| H1 | H2 |\n| --- | --- |\n|   | x |"
        );
    }

    #[test]
    fn cluster_large_tolerance_collapses_all_rows() {
        let ws = vec![
            w(0.0, 0.0, "A"), w(100.0, 0.0, "B"), w(220.0, 0.0, "C"),
            w(0.0, 20.0, "1"), w(100.0, 20.0, "2"), w(220.0, 20.0, "3"),
        ];
        assert_eq!(cluster_words_into_table(&ws, 20.0), "A B C 1 2 3");
    }

    // ---- html_to_markdown_clean (oracle.json: html) ----

    fn html(s: &str) -> String {
        html_to_markdown_clean(s)
    }

    #[test]
    fn html_strips_script_and_style() {
        assert_eq!(html("<div><script>var x=1;</script><style>p{}</style>keep</div>"), "keep");
    }

    #[test]
    fn html_headings() {
        assert_eq!(html("<h1>One</h1><h2 class=z>Two</h2><h3>Three</h3>"), "# One\n\n## Two\n\n### Three");
    }

    #[test]
    fn html_paragraph_break_and_list() {
        assert_eq!(
            html("<p>para one</p><br>line<p>para two</p><ul><li>a</li><li>b</li></ul>"),
            "para one\n\nline\n\npara two\n\n- a\n- b"
        );
    }

    #[test]
    fn html_bold_italic() {
        assert_eq!(html("<b>bold</b> and <strong>s</strong> and <i>it</i> and <em>e</em>"), "**bold** and **s** and *it* and *e*");
    }

    #[test]
    fn html_bold_case_insensitive_backref() {
        assert_eq!(html("<B>upper</b> x <STRONG>q</Strong>"), "**upper** x **q**");
    }

    #[test]
    fn html_bold_b_prefix_quirk() {
        // `<badge>` is consumed as an open `<b>` tag (group1='b', [^>]*='adge').
        assert_eq!(html("<badge>notreally</b>"), "**notreally**");
    }

    #[test]
    fn html_nested_bold_in_paragraph() {
        assert_eq!(html("<p>hello <b>world</b> bye</p>"), "hello **world** bye");
    }

    #[test]
    fn html_entities_double_decode() {
        assert_eq!(
            html("a &amp; b &lt;tag&gt; &nbsp;&quot;q&quot; and &amp;lt;double"),
            "a & b <tag>  \"q\" and <double"
        );
    }

    #[test]
    fn html_left_angle_math_eaten() {
        assert_eq!(html("2 < 3 > 4 real <em>ok</em>"), "2  4 real *ok*");
    }

    #[test]
    fn html_newline_collapse() {
        assert_eq!(html("<p>a</p><p>b</p><p>c</p>"), "a\n\nb\n\nc");
    }

    #[test]
    fn html_strip_includes_nbsp() {
        assert_eq!(html("   \t\n leading and trailing \u{a0}  "), "leading and trailing");
    }

    #[test]
    fn html_unicode_content_preserved() {
        assert_eq!(html("<h1>标题 标题</h1><p>日本語 テスト</p>"), "# 标题 标题\n\n日本語 テスト");
    }

    #[test]
    fn html_bold_with_attributes() {
        assert_eq!(html("<b class=\"x\" id=y>bolded</b> tail"), "**bolded** tail");
    }

    #[test]
    fn html_italic_nested_bold() {
        assert_eq!(html("<i><b>bi</b> plain</i>"), "***bi** plain*");
    }

    #[test]
    fn html_paragraph_multiline() {
        assert_eq!(html("<p>line1\nline2</p>"), "line1\nline2");
    }

    #[test]
    fn html_list_item_with_bold() {
        assert_eq!(html("<li>a <b>strong</b> b</li>"), "- a **strong** b");
    }

    #[test]
    fn html_unclosed_bold_stripped() {
        assert_eq!(html("<b>never closed then text"), "never closed then text");
    }

    #[test]
    fn html_heading_content_with_tags() {
        assert_eq!(html("<h2>Title <small>sub</small></h2>"), "## Title sub");
    }

    #[test]
    fn html_br_variants() {
        assert_eq!(html("a<br/>b<br />c<br>d"), "a\nb\nc\nd");
    }

    // ---- extract_page_table_cluster (oracle.json: extract) ----

    #[test]
    fn extract_fewer_than_six_words_is_none() {
        let ws = vec![wbox(0.0, 0.0, 10.0, 8.0, "a"); 5];
        assert!(extract_page_table_cluster(&ws).is_none());
    }

    #[test]
    fn extract_single_row_is_none() {
        let ws: Vec<RawWord> = (0..6).map(|i| wbox(0.0, 0.0, 10.0, 8.0, &format!("w{i}"))).collect();
        assert!(extract_page_table_cluster(&ws).is_none());
    }

    #[test]
    fn extract_one_column_is_none() {
        let ys = [0.0, 3.0, 6.0, 9.0, 12.0, 15.0];
        let ws: Vec<RawWord> = ys.iter().enumerate().map(|(i, &y)| wbox(0.0, y, 20.0, y + 10.0, &format!("w{i}"))).collect();
        assert!(extract_page_table_cluster(&ws).is_none());
    }

    #[test]
    fn extract_table_ok_matches_oracle() {
        let mut ws: Vec<RawWord> = Vec::new();
        for (yy, cells) in [(0.0f64, ["H1", "H2", "H3"]), (20.0, ["a", "b", "c"])] {
            for (k, xx) in [0.0f64, 100.0, 220.0].iter().enumerate() {
                ws.push(wbox(*xx, yy, xx + 30.0, yy + 10.0, cells[k]));
            }
        }
        let p = extract_page_table_cluster(&ws).expect("table");
        assert_eq!(p.get_markdown(), "| H1 | H2 | H3 |\n| --- | --- | --- |\n| a | b | c |");
        assert_eq!(p.bbox, (0.0, 0.0, 250.0, 30.0));
        assert!(p.extract().is_empty());
    }

    // ---- epub_to_markdown (oracle.json: epub) ----

    fn write_tmp(bytes: &[u8]) -> (tempfile::NamedTempFile, String) {
        use std::io::Write;
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(bytes).unwrap();
        f.flush().unwrap();
        let path = f.path().to_string_lossy().to_string();
        (f, path)
    }

    #[test]
    fn epub_basic_spine_order() {
        let entries = vec![
            ("mimetype", b"application/epub+zip".to_vec()),
            (
                "META-INF/container.xml",
                b"<?xml version='1.0'?>\n<container version='1.0' xmlns='urn:oasis:names:tc:opendocument:xmlns:container'>\n <rootfiles><rootfile full-path='OEBPS/content.opf' media-type='application/oebps-package+xml'/></rootfiles>\n</container>".to_vec(),
            ),
            (
                "OEBPS/content.opf",
                b"<?xml version='1.0'?>\n<package xmlns='http://www.idpf.org/2007/opf' version='3.0'>\n <metadata><dc:title xmlns:dc='http://purl.org/dc/elements/1.1/'>t</dc:title></metadata>\n <manifest>\n  <item id='c1' href='Text/chap1.xhtml' media-type='application/xhtml+xml'/>\n  <item id='c2' href='Text/chap2.xhtml' media-type='application/xhtml+xml'/>\n </manifest>\n <spine><itemref idref='c1'/><itemref idref='c2'/></spine>\n</package>".to_vec(),
            ),
            (
                "OEBPS/Text/chap1.xhtml",
                b"<html><head><title>x</title></head><body><h1>Chapter 1</h1><p>first para</p></body></html>".to_vec(),
            ),
            (
                "OEBPS/Text/chap2.xhtml",
                b"<html><body><h2>Chapter 2</h2><ul><li>one</li><li>two</li></ul></body></html>".to_vec(),
            ),
        ];
        let bytes = build_zip(&entries);
        let (_f, path) = write_tmp(&bytes);
        let md = epub_to_markdown(&path).unwrap();
        assert_eq!(md, "x\n\n# Chapter 1\n\nfirst para\n\n---\n\n## Chapter 2\n\n- one\n- two");
    }

    #[test]
    fn epub_spine_href_missing_member_skipped() {
        let entries = vec![
            (
                "META-INF/container.xml",
                b"<container><rootfiles><rootfile full-path='OEBPS/content.opf'/></rootfiles></container>".to_vec(),
            ),
            (
                "OEBPS/content.opf",
                b"<package><manifest>\n<item id='c1' href='Text/gone.xhtml'/>\n<item id='c2' href='Text/chap2.xhtml'/>\n</manifest><spine><itemref idref='c1'/><itemref idref='c2'/></spine></package>".to_vec(),
            ),
            (
                "OEBPS/Text/chap2.xhtml",
                b"<html><body><h2>Chapter 2</h2><ul><li>one</li><li>two</li></ul></body></html>".to_vec(),
            ),
        ];
        let (_f, path) = write_tmp(&build_zip(&entries));
        assert_eq!(epub_to_markdown(&path).unwrap(), "## Chapter 2\n\n- one\n- two");
    }

    #[test]
    fn epub_no_container_falls_back_to_namelist() {
        let entries = vec![
            (
                "OEBPS/Text/chap1.xhtml",
                b"<html><head><title>x</title></head><body><h1>Chapter 1</h1><p>first para</p></body></html>".to_vec(),
            ),
            (
                "OEBPS/Text/chap2.xhtml",
                b"<html><body><h2>Chapter 2</h2><ul><li>one</li><li>two</li></ul></body></html>".to_vec(),
            ),
            ("notes.txt", b"hi".to_vec()),
        ];
        let (_f, path) = write_tmp(&build_zip(&entries));
        assert_eq!(epub_to_markdown(&path).unwrap(), "x\n\n# Chapter 1\n\nfirst para\n\n---\n\n## Chapter 2\n\n- one\n- two");
    }

    #[test]
    fn epub_malformed_opf_item_missing_href_falls_back() {
        let entries = vec![
            (
                "META-INF/container.xml",
                b"<container><rootfiles><rootfile full-path='OEBPS/content.opf'/></rootfiles></container>".to_vec(),
            ),
            ("OEBPS/content.opf", b"<package><manifest><item id='c1'/></manifest><spine><itemref idref='c1'/></spine></package>".to_vec()),
            (
                "OEBPS/Text/chap1.xhtml",
                b"<html><head><title>x</title></head><body><h1>Chapter 1</h1><p>first para</p></body></html>".to_vec(),
            ),
        ];
        let (_f, path) = write_tmp(&build_zip(&entries));
        assert_eq!(epub_to_markdown(&path).unwrap(), "x\n\n# Chapter 1\n\nfirst para");
    }

    #[test]
    fn epub_not_a_zip_raises_value_error() {
        let (_f, path) = write_tmp(b"this is definitely not a zip file");
        assert_eq!(epub_to_markdown(&path).unwrap_err(), "无效的 EPUB 文件");
    }

    #[test]
    fn epub_blank_chapter_skipped() {
        let entries = vec![
            (
                "META-INF/container.xml",
                b"<container><rootfiles><rootfile full-path='OEBPS/content.opf'/></rootfiles></container>".to_vec(),
            ),
            (
                "OEBPS/content.opf",
                b"<package><manifest>\n<item id='c1' href='Text/chap1.xhtml'/>\n<item id='c2' href='Text/chap2.xhtml'/>\n</manifest><spine><itemref idref='c1'/><itemref idref='c2'/></spine></package>".to_vec(),
            ),
            ("OEBPS/Text/chap1.xhtml", b"<html><body></body></html>".to_vec()),
            (
                "OEBPS/Text/chap2.xhtml",
                b"<html><body><h2>Chapter 2</h2><ul><li>one</li><li>two</li></ul></body></html>".to_vec(),
            ),
        ];
        let (_f, path) = write_tmp(&build_zip(&entries));
        assert_eq!(epub_to_markdown(&path).unwrap(), "## Chapter 2\n\n- one\n- two");
    }

    /// Decode the exact archive Python's `zipfile` produced for `basic` and run
    /// it through the same code path, proving the hand-written ZIP reader matches
    /// `zipfile.ZipFile.read`.
    #[test]
    fn epub_real_python_zip_archive_decodes_identically() {
        // Hex of an EPUB archive actually produced by Python's `zipfile`
        // (see scratch/rust_parity/convert_ext_s1/oracle.json epub_bytes_hex).
        let hex = "504b03041400000000004022375d6f61ab2c1400000014000000080000006d696d65747970656170706c69636174696f6e2f657075622b7a6970504b03041400000000004022375dfe648626e1000000e1000000160000004d4554412d494e462f636f6e7461696e65722e786d6c3c3f786d6c2076657273696f6e3d27312e30273f3e0a3c636f6e7461696e65722076657273696f6e3d27312e302720786d6c6e733d2775726e3a6f617369733a6e616d65733a74633a6f70656e646f63756d656e743a786d6c6e733a636f6e7461696e6572273e0a203c726f6f7466696c65733e3c726f6f7466696c652066756c6c2d706174683d274f454250532f636f6e74656e742e6f706627206d656469612d747970653d276170706c69636174696f6e2f6f656270732d7061636b6167652b786d6c272f3e3c2f726f6f7466696c65733e0a3c2f636f6e7461696e65723e504b03041400000000004022375df120f6baa4010000a4010000110000004f454250532f636f6e74656e742e6f70663c3f786d6c2076657273696f6e3d27312e30273f3e0a3c7061636b61676520786d6c6e733d27687474703a2f2f7777772e696470662e6f72672f323030372f6f7066272076657273696f6e3d27332e30273e0a203c6d657461646174613e3c64633a7469746c6520786d6c6e733a64633d27687474703a2f2f7075726c2e6f72672f64632f656c656d656e74732f312e312f273e743c2f64633a7469746c653e3c2f6d657461646174613e0a203c6d616e69666573743e0a20203c6974656d2069643d2763312720687265663d27546578742f63686170312e7868746d6c27206d656469612d747970653d276170706c69636174696f6e2f7868746d6c2b786d6c272f3e0a20203c6974656d2069643d2763322720687265663d27546578742f63686170322e7868746d6c27206d656469612d747970653d276170706c69636174696f6e2f7868746d6c2b786d6c272f3e0a203c2f6d616e69666573743e0a203c7370696e653e3c6974656d7265662069647265663d276331272f3e3c6974656d7265662069647265663d276332272f3e3c2f7370696e653e0a3c2f7061636b6167653e504b03041400000000004022375d36510ce35a0000005a000000160000004f454250532f546578742f63686170312e7868746d6c3c68746d6c3e3c686561643e3c7469746c653e783c2f7469746c653e3c2f686561643e3c626f64793e3c68313e4368617074657220313c2f68313e3c703e666972737420706172613c2f703e3c2f626f64793e3c2f68746d6c3e504b03041400000000004022375df9d9c0ce4d0000004d000000160000004f454250532f546578742f63686170322e7868746d6c3c68746d6c3e3c626f64793e3c68323e4368617074657220323c2f68323e3c756c3e3c6c693e6f6e653c2f6c693e3c6c693e74776f3c2f6c693e3c2f756c3e3c2f626f64793e3c2f68746d6c3e504b010214001400000000004022375d6f61ab2c14000000140000000800000000000000000000008001000000006d696d6574797065504b010214001400000000004022375dfe648626e1000000e100000016000000000000000000000080013a0000004d4554412d494e462f636f6e7461696e65722e786d6c504b010214001400000000004022375df120f6baa4010000a401000011000000000000000000000080014f0100004f454250532f636f6e74656e742e6f7066504b010214001400000000004022375d36510ce35a0000005a0000001600000000000000000000008001220300004f454250532f546578742f63686170312e7868746d6c504b010214001400000000004022375df9d9c0ce4d0000004d0000001600000000000000000000008001b00300004f454250532f546578742f63686170322e7868746d6c504b0506000000000500050041010000310400000000";
        let mut bytes = Vec::with_capacity(hex.len() / 2);
        let hb = hex.as_bytes();
        let mut i = 0;
        while i < hb.len() {
            let hi = (hb[i] as char).to_digit(16).unwrap();
            let lo = (hb[i + 1] as char).to_digit(16).unwrap();
            bytes.push((hi * 16 + lo) as u8);
            i += 2;
        }
        let (_f, path) = write_tmp(&bytes);
        assert_eq!(
            epub_to_markdown(&path).unwrap(),
            "x\n\n# Chapter 1\n\nfirst para\n\n---\n\n## Chapter 2\n\n- one\n- two"
        );
    }

    // ---- media_to_markdown ----

    #[test]
    fn media_wrapper_matches_python_format() {
        let md = media_to_markdown("/data/rec/meeting.mp3", "hello world");
        assert_eq!(
            md,
            "# 多媒体录音转写纪要: meeting.mp3\n\n> 来源文件: `/data/rec/meeting.mp3`\n\nhello world"
        );
    }

    #[test]
    fn media_basename_handles_windows_separator() {
        let md = media_to_markdown("C:\\tmp\\a.mp3", "");
        assert!(md.starts_with("# 多媒体录音转写纪要: a.mp3\n"));
    }

    // ---- string-model helpers ----

    #[test]
    fn py_slice_clamps_and_indexes_by_char() {
        assert_eq!(py_slice("héllo", 1, 3), "él");
        assert_eq!(py_slice("日本語", 1, 100), "本語");
        assert_eq!(py_slice("abc", 5, 1), "");
    }

    #[test]
    fn py_strip_removes_python_only_whitespace() {
        assert_eq!(py_strip("\u{1c}x\u{1d}"), "x"); // ASCII info separators
        assert_eq!(py_strip("\u{a0}y"), "y"); // NBSP
        assert_eq!(py_strip("   "), "");
    }

    #[test]
    fn norm_join_matches_python() {
        assert_eq!(norm_join("OEBPS", "Text/chap1.xhtml"), "OEBPS/Text/chap1.xhtml");
        assert_eq!(norm_join("", "a/b.xhtml"), "a/b.xhtml");
        assert_eq!(norm_join("OEBPS", "../Text/chap1.xhtml"), "Text/chap1.xhtml");
    }

    // A minimal STORED-ZIP builder used only by the tests above.
    fn build_zip(entries: &[(&str, Vec<u8>)]) -> Vec<u8> {
        fn crc32(data: &[u8]) -> u32 {
            let mut crc = !0u32;
            for &b in data {
                crc ^= b as u32;
                for _ in 0..8 {
                    let mask = (crc & 1).wrapping_neg();
                    crc = (crc >> 1) ^ (0xEDB88320 & mask);
                }
            }
            !crc
        }
        fn push16(v: &mut Vec<u8>, x: u16) { v.extend_from_slice(&x.to_le_bytes()); }
        fn push32(v: &mut Vec<u8>, x: u32) { v.extend_from_slice(&x.to_le_bytes()); }

        let mut out = Vec::new();
        let mut central = Vec::new();
        let mut count = 0u16;
        for (name, data) in entries {
            let nb = name.as_bytes();
            let local_off = out.len() as u32;
            push32(&mut out, 0x04034b50);
            push16(&mut out, 20); push16(&mut out, 0); push16(&mut out, 0); // ver, flags, method=stored
            push16(&mut out, 0); push16(&mut out, 0); // time, date
            push32(&mut out, crc32(data));
            push32(&mut out, data.len() as u32);
            push32(&mut out, data.len() as u32);
            push16(&mut out, nb.len() as u16);
            push16(&mut out, 0);
            out.extend_from_slice(nb);
            out.extend_from_slice(data);

            push32(&mut central, 0x02014b50);
            push16(&mut central, 20); push16(&mut central, 20); push16(&mut central, 0); push16(&mut central, 0);
            push16(&mut central, 0); push16(&mut central, 0);
            push32(&mut central, crc32(data));
            push32(&mut central, data.len() as u32);
            push32(&mut central, data.len() as u32);
            push16(&mut central, nb.len() as u16);
            push16(&mut central, 0); push16(&mut central, 0);
            push16(&mut central, 0); push16(&mut central, 0); push32(&mut central, 0);
            push32(&mut central, local_off);
            central.extend_from_slice(nb);
            count += 1;
        }
        let cd_off = out.len() as u32;
        out.extend_from_slice(&central);
        let cd_size = central.len() as u32;
        push32(&mut out, 0x06054b50);
        push16(&mut out, 0); push16(&mut out, 0);
        push16(&mut out, count); push16(&mut out, count);
        push32(&mut out, cd_size); push32(&mut out, cd_off);
        push16(&mut out, 0);
        out
    }
}
