//! Pure Rust implementation of ReadMD Markdown Auto-Repair and Syntax Normalizer.
//! Ported directly from `src/readmd_core/readmd_fix.py`.
//! Zero external runtime dependencies. Runs in microseconds.

use regex::Regex;
use serde_json::{json, Value};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct FixResult {
    pub text: String,
    pub fixes: Vec<String>,
    pub stats: Value,
}

lazy_static::lazy_static! {
    // Every `\s` below is written as `[\s\x1c-\x1f]`. CPython's `re` takes its
    // whitespace from `Py_UNICODE_ISSPACE`, which counts U+001C..U+001F (file /
    // group / record / unit separators); the `regex` crate's `\s` is
    // `\p{White_Space}` and does not. `py_isspace` is the same widening for the
    // non-regex sites. U+001A / U+001B deliberately stay out of both sets — U+001A
    // is the placeholder sentinel `mask_code_spans` injects into the text.
    static ref SEP_RE: Regex = Regex::new(r"^\|?[\s\x1c-\x1f]*:?-{2,}:?[\s\x1c-\x1f]*(\|[\s\x1c-\x1f]*:?-{2,}:?[\s\x1c-\x1f]*)*\|?$").unwrap();
    static ref HR_RE: Regex = Regex::new(r"^[\s\x1c-\x1f]*(\*{3,}|-{3,}|_{3,})[\s\x1c-\x1f]*$").unwrap();
    static ref LIST_ITEM_RE: Regex = Regex::new(r"^[*+][\s\x1c-\x1f]").unwrap();
    static ref HEADING_RE: Regex = Regex::new(r"^([\s\x1c-\x1f]*)(#{1,6})([^#\s\x1c-\x1f].*)$").unwrap();
    static ref LATEX_RE: Regex = Regex::new(r"[\\^_{}]").unwrap();
    static ref FENCE_RE: Regex = Regex::new(r"^([\s\x1c-\x1f]{0,3})(`{3,}|~{3,})(.*)$").unwrap();
    static ref RESTORE_RE: Regex = Regex::new(r"\x1a[A-Za-z0-9_]+\x1a").unwrap();
    // Py 93 `re.match(r'^([-*+]|\d+\.)\s', s)` — the list/ordered-list exemption in
    // _looks_like_code_indent. Python's \d is Unicode `Nd`; `regex` crate \d is \p{Nd}.
    static ref CODE_INDENT_LIST_RE: Regex = Regex::new(r"^([-*+]|\d+\.)[\s\x1c-\x1f]").unwrap();
    // Py 273 `re.match(r'^(:?)(-+)(:?)$', c)` in _norm_sep_cell.
    static ref SEP_CELL_RE: Regex = Regex::new(r"^(:?)(-+)(:?)$").unwrap();
    // Py 385-387 `unicodedata.category(ch).startswith(('P','S','Z'))`. Those three
    // general categories are exactly the Unicode property classes below, so the
    // fallback needs no new dependency — `regex` already ships Unicode tables.
    // `\p{Z}` is Zs/Zl/Zp and does NOT contain U+001C..U+001F (category Cc), so
    // this class stays as it is; Python reaches those four through `ch.isspace()`.
    static ref PUNCT_CLASS_RE: Regex = Regex::new(r"[\p{P}\p{S}\p{Z}]").unwrap();
}

// ----------------------------------------------------------------------------
// Python-exact string primitives
//
// Two unit mismatches used to live in this module, and both were crashes rather
// than cosmetic drift:
//   * CPython indexes and slices strings by CODE POINT. `str::find` hands back
//     BYTE offsets and `&s[..pos]` slices by byte, so every position that came
//     from `find` and went back into a slice was in the wrong unit as soon as the
//     text held one non-ASCII character: either a panic ("byte index N is not a
//     char boundary") or a silent wrong cut. Every position in this module is now
//     a code-point index and `char_to_byte` is the single place the units meet; it
//     clamps, the way Python slicing clamps instead of raising.
//   * `char::is_whitespace()` is narrower than `str.isspace()`.
// ----------------------------------------------------------------------------

/// CPython's `str.isspace()` for one code point: the Unicode `White_Space`
/// property plus U+001C..U+001F, which `char::is_whitespace()` omits. Measured on
/// CPython 3.11.15: `'\x1c'.isspace()` is True, `'\x1a'.isspace()` and
/// `'\x1b'.isspace()` are both False.
#[inline]
fn py_isspace(ch: char) -> bool {
    ch.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&ch)
}

/// Py `str.lstrip()`.
fn py_strip_start(s: &str) -> &str {
    for (i, c) in s.char_indices() {
        if !py_isspace(c) {
            return &s[i..];
        }
    }
    &s[s.len()..]
}

/// Py `str.rstrip()`.
fn py_strip_end(s: &str) -> &str {
    let mut end = 0usize;
    for (i, c) in s.char_indices() {
        if !py_isspace(c) {
            end = i + c.len_utf8();
        }
    }
    &s[..end]
}

/// Py `str.strip()`.
fn py_strip(s: &str) -> &str {
    py_strip_end(py_strip_start(s))
}

/// Py `re.match(r'^\s*', s).group(0)` — the leading whitespace run, i.e. a
/// table block's indent.
fn py_space_prefix(s: &str) -> &str {
    &s[..s.len() - py_strip_start(s).len()]
}

/// Byte offset of code point `n`, saturating at the end of the string exactly
/// where CPython's slicing saturates instead of raising.
fn char_to_byte(s: &str, n: usize) -> usize {
    s.char_indices().nth(n).map(|(i, _)| i).unwrap_or(s.len())
}

/// Py `s[n]` where `n` may be out of range: `None` stands for the `''` that
/// Python's `s[p - 1] if p > 0 else ''` / `s[p + len(d)] if ... else ''` guards
/// produce, and every caller treats "no character" and "empty string" alike.
fn char_at(s: &str, n: usize) -> Option<char> {
    s.chars().nth(n)
}

/// Py `s.find(sub)`: the index CPython reports is a CODE POINT offset, whereas
/// `str::find` reports a BYTE offset. Returning the code-point index keeps every
/// `s[:p]` / `s[p + 1:]` that consumes it in Python's unit; the cut itself is
/// re-based with `char_to_byte`.
fn py_find(s: &str, needle: &str) -> Option<usize> {
    s.find(needle).map(|byte| s[..byte].chars().count())
}

fn is_escaped(s: &str, pos: usize) -> bool {
    let bytes = s.as_bytes();
    let mut count = 0;
    let mut k = pos as isize - 1;
    while k >= 0 && bytes[k as usize] == b'\\' {
        count += 1;
        k -= 1;
    }
    count % 2 == 1
}

/// Py 47-58 `_unescaped_positions`. **Positions are code-point indices**, because
/// that is what Python's `str.find` yields and every consumer here feeds them
/// straight back into a slice. `is_escaped` still takes a byte offset (walking
/// bytes for `\\` is equivalent to walking code points — a backslash occupies
/// exactly one byte in UTF-8), so the conversion happens on the way out.
fn unescaped_positions(s: &str, sub: &str) -> Vec<usize> {
    let mut res = Vec::new();
    let mut i = 0;
    while let Some(j) = s[i..].find(sub) {
        let abs_j = i + j;
        if !is_escaped(s, abs_j) {
            res.push(s[..abs_j].chars().count());
        }
        i = abs_j + sub.len();
        if i >= s.len() {
            break;
        }
    }
    res
}

/// Py 61-64 `s[:pos] + esc + s[pos + len(d):]`. `pos` is a code point index and
/// `len(d)` a code-point count. `escape_delim` is reached with positions that are
/// stale by design — Py 443 mutates `s` and Py 448 then still escapes `strays`
/// computed against the pre-mutation string — and Python answers those cuts
/// because code-point slicing cannot land "inside" a character. The previous port
/// sliced by byte and panicked there.
fn escape_delim(s: &str, d: &str, pos: usize) -> String {
    let mut esc = String::new();
    for ch in d.chars() {
        esc.push('\\');
        esc.push(ch);
    }
    let a = char_to_byte(s, pos);
    let b = char_to_byte(s, pos + d.chars().count());
    format!("{}{}{}", &s[..a], esc, &s[b..])
}

/// Py 67-68 `_escape_at(s, pos, ch)`. `pos` is a code point index; the only
/// caller passes the position of a `$`, so `ch` is that dollar.
fn escape_at(s: &str, pos: usize) -> String {
    let a = char_to_byte(s, pos);
    let b = char_to_byte(s, pos + 1);
    format!("{}\\{}{}", &s[..a], &s[a..b], &s[b..])
}

/// Py 71-80. Already code-point faithful: `char_indices` walks the same sequence
/// Python's `range(len(s))` does, and the split point for `\\` + `ch` is a
/// character boundary by construction.
fn escape_all_unescaped(s: &str, ch: char) -> String {
    let mut out = String::new();
    for (i, c) in s.char_indices() {
        if c == ch && !is_escaped(s, i) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

fn has_latex(s: &str) -> bool {
    LATEX_RE.is_match(s)
}

fn looks_like_code_indent(line: &str) -> bool {
    if line.starts_with("    ") || line.starts_with('\t') {
        let s = py_strip_start(line);
        if s.starts_with("> ") || s.starts_with(">-") {
            return false;
        }
        // Py 93: `re.match(r'^([-*+]|\d+\.)\s', s)` — unordered AND ordered list
        // markers are exempt. The previous port used startswith("- ") etc., which
        // both missed `\d+\.` and missed a tab after the marker.
        if CODE_INDENT_LIST_RE.is_match(s) {
            return false;
        }
        return true;
    }
    false
}

// ----------------------------------------------------------------------------
// Code Masking
// ----------------------------------------------------------------------------

fn mask_code_spans(s: &str, start_idx: usize) -> (String, Vec<(String, String)>) {
    if !s.contains('`') {
        return (s.to_string(), Vec::new());
    }
    let mut spans = Vec::new();
    let mut out = String::new();
    let bytes = s.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    let mut idx = start_idx;

    while i < n {
        if bytes[i] != b'`' || is_escaped(s, i) {
            let mut j = if bytes[i] == b'`' { i + 1 } else { i };
            while j < n {
                if bytes[j] == b'`' && !is_escaped(s, j) {
                    break;
                }
                j += 1;
            }
            if j >= n {
                out.push_str(&s[i..]);
                break;
            }
            out.push_str(&s[i..j]);
            i = j;
        }
        let mut j = i;
        while j < n && bytes[j] == b'`' {
            j += 1;
        }
        let run_len = j - i;
        let mut m = j;
        let mut found = None;

        while m < n {
            if let Some(k) = s[m..].find('`') {
                let abs_k = m + k;
                let mut e = abs_k;
                while e < n && bytes[e] == b'`' {
                    e += 1;
                }
                if e - abs_k == run_len {
                    found = Some(e);
                    break;
                }
                m = e;
            } else {
                break;
            }
        }

        if let Some(end_pos) = found {
            let ph = format!("\x1aC{}\x1a", idx);
            spans.push((ph.clone(), s[i..end_pos].to_string()));
            out.push_str(&ph);
            i = end_pos;
            idx += 1;
        } else {
            out.push_str(&s[i..j]);
            i = j;
        }
    }

    (out, spans)
}

fn mask_all_code(text: &str) -> (String, Vec<(String, String)>) {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut spans = Vec::new();
    let mut out = Vec::new();
    let n = lines.len();
    let mut i = 0;
    let mut fence: Option<(char, usize)> = None;

    while i < n {
        let line = lines[i];
        if let Some((f_char, f_len)) = fence {
            if let Some(caps) = FENCE_RE.captures(line) {
                if let (Some(m2), Some(m3)) = (caps.get(2), caps.get(3)) {
                    if m2.as_str().starts_with(f_char)
                        && m2.as_str().len() >= f_len
                        && py_strip(m3.as_str()).is_empty()
                    {
                        fence = None;
                    }
                }
            }
            let ph = format!("\x1aF{}\x1a", spans.len());
            spans.push((ph.clone(), line.to_string()));
            out.push(ph);
            i += 1;
            continue;
        }

        if let Some(caps) = FENCE_RE.captures(line) {
            if let Some(m2) = caps.get(2) {
                let first_c = m2.as_str().chars().next().unwrap_or('`');
                if first_c == '`' || first_c == '~' {
                    let m3_str = caps.get(3).map(|m| m.as_str()).unwrap_or("");
                    if !(first_c == '`' && m3_str.contains('`')) {
                        fence = Some((first_c, m2.as_str().len()));
                        let ph = format!("\x1aF{}\x1a", spans.len());
                        spans.push((ph.clone(), line.to_string()));
                        out.push(ph);
                        i += 1;
                        continue;
                    }
                }
            }
        }

        let (masked_line, line_spans) = mask_code_spans(line, spans.len());
        spans.extend(line_spans);
        out.push(masked_line);
        i += 1;
    }

    (out.join("\n"), spans)
}

fn restore(text: &str, spans: &[(String, String)]) -> String {
    if spans.is_empty() {
        return text.to_string();
    }
    let map: HashMap<&str, &str> = spans.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for m in RESTORE_RE.find_iter(text) {
        out.push_str(&text[last..m.start()]);
        if let Some(&orig) = map.get(m.as_str()) {
            out.push_str(orig);
        } else {
            out.push_str(m.as_str());
        }
        last = m.end();
    }
    out.push_str(&text[last..]);
    out
}

// ----------------------------------------------------------------------------
// Tables
// ----------------------------------------------------------------------------

fn is_table_sep(line: &str) -> bool {
    // Py 218-219: `s = line.strip()` then `len(s) < 3`. Both `strip()` and `len()`
    // work on code points, so neither `str::trim()` nor `str::len()` is a
    // substitute.
    let s = py_strip(line);
    if !s.contains('|') || s.chars().count() < 3 {
        return false;
    }
    if !SEP_RE.is_match(s) {
        return false;
    }
    s.contains("--")
}

fn is_table_row(line: &str) -> bool {
    if !line.contains('|') {
        return false;
    }
    if py_strip_start(line).starts_with('|') {
        return true;
    }
    if line.matches('|').count() >= 2 {
        return true;
    }
    line.contains(" | ") || line.contains("| ") || line.contains(" |")
}

fn split_cells(row: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut cur = String::new();
    let chars: Vec<char> = row.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '\\' && i + 1 < chars.len() && chars[i + 1] == '|' {
            cur.push('|');
            i += 2;
            continue;
        }
        if ch == '|' {
            cells.push(cur.clone());
            cur.clear();
            i += 1;
            continue;
        }
        cur.push(ch);
        i += 1;
    }
    cells.push(cur);

    // Py 256-258: `cells[0].strip() == ''` / `row.lstrip().startswith('|')` /
    // `cells[-1].strip() == ''` / `row.rstrip().endswith('|')`.
    if !cells.is_empty() && py_strip(&cells[0]).is_empty() && py_strip_start(row).starts_with('|') {
        cells.remove(0);
    }
    if !cells.is_empty()
        && py_strip(&cells[cells.len() - 1]).is_empty()
        && py_strip_end(row).ends_with('|')
    {
        cells.pop();
    }
    cells
}

fn rebuild_row(cells: &[String]) -> String {
    format!("| {} |", cells.join(" | "))
}

fn escape_cell(c: &str) -> String {
    c.replace('|', "\\|")
}

fn norm_sep_cell(c: &str) -> String {
    // Py 271-276, verbatim:
    //     m = re.match(r'^(:?)(-+)(:?)$', c)
    //     if m and len(m.group(2)) < 3:
    //         return m.group(1) + '---' + m.group(3)
    //     return c
    // The `return c` fallback is load-bearing: a cell with 3+ dashes is left
    // byte-for-byte alone. The previous port rewrote every matching cell to exactly
    // "---", silently flattening `----`/`-----`, and invented a "---" for cells the
    // regex cannot match at all ("" or ":") because `-+` requires one dash.
    if let Some(caps) = SEP_CELL_RE.captures(c) {
        let dashes = caps.get(2).map(|m| m.as_str()).unwrap_or("");
        if dashes.chars().count() < 3 {
            let mut res = String::new();
            res.push_str(caps.get(1).map(|m| m.as_str()).unwrap_or(""));
            res.push_str("---");
            res.push_str(caps.get(3).map(|m| m.as_str()).unwrap_or(""));
            return res;
        }
    }
    c.to_string()
}

fn normalize_table(block: &[String], start_no: usize, fixes: &mut Vec<String>) -> (Vec<String>, bool) {
    // Py 280 `re.match(r'^\s*', block[0]).group(0)` — Python's `\s`, so the indent
    // may be a run of U+001C..U+001F that `str::trim_start` would leave in place.
    let indent = block.first().map(|l| py_space_prefix(l)).unwrap_or("");

    let sep_idx: Vec<usize> = block.iter().enumerate()
        .filter(|(_, l)| is_table_sep(l))
        .map(|(k, _)| k)
        .collect();

    let data_idx: Vec<usize> = block.iter().enumerate()
        .filter(|(_, l)| !is_table_sep(l) && is_table_row(l))
        .map(|(k, _)| k)
        .collect();

    if data_idx.is_empty() {
        return (block.to_vec(), false);
    }

    let mut max_cols = 1;
    for &k in data_idx.iter().chain(sep_idx.iter()) {
        let c_len = split_cells(&block[k]).len();
        if c_len > max_cols {
            max_cols = c_len;
        }
    }

    let mut changed = false;
    let mut row_diffs = 0;
    let mut new_block = Vec::with_capacity(block.len() + 1);

    for l in block.iter() {
        if is_table_sep(l) {
            // Py 296 `_norm_sep_cell(c.strip())`.
            let mut cells: Vec<String> = split_cells(l).into_iter().map(|c| norm_sep_cell(py_strip(&c))).collect();
            if cells.len() < max_cols {
                while cells.len() < max_cols {
                    cells.push("---".to_string());
                }
                changed = true;
            } else if cells.len() > max_cols {
                cells.truncate(max_cols);
                changed = true;
            }
            new_block.push(format!("{}{}", indent, rebuild_row(&cells)));
        } else if is_table_row(l) {
            // Py 305 `[c.strip() for c in _split_cells(l)]`.
            let mut cells: Vec<String> = split_cells(l).into_iter().map(|c| py_strip(&c).to_string()).collect();
            if cells.len() < max_cols {
                while cells.len() < max_cols {
                    cells.push(String::new());
                }
                changed = true;
                row_diffs += 1;
            } else if cells.len() > max_cols {
                let extra = cells[max_cols - 1..].join(" ");
                cells.truncate(max_cols - 1);
                cells.push(extra);
                changed = true;
                row_diffs += 1;
            }
            let escaped_cells: Vec<String> = cells.into_iter().map(|c| escape_cell(&c)).collect();
            new_block.push(format!("{}{}", indent, rebuild_row(&escaped_cells)));
        } else {
            new_block.push(l.clone());
        }
    }

    if new_block != block {
        changed = true;
    }

    if sep_idx.is_empty() && !data_idx.is_empty() {
        let sep_cells: Vec<String> = vec!["---".to_string(); max_cols];
        let sep_line = format!("{}{}", indent, rebuild_row(&sep_cells));
        new_block.insert(1, sep_line);
        changed = true;
        fixes.push(format!("[表格] 第 {} 行附近：缺少表头分隔行，已自动补全", start_no));
    }

    if row_diffs > 0 {
        fixes.push(format!(
            "[表格] 第 {}-{} 行：{} 行列数不齐，已对齐为 {} 列",
            start_no,
            start_no + block.len() - 1,
            row_diffs,
            max_cols
        ));
    }

    (new_block, changed)
}

fn process_tables(lines: &mut Vec<String>, fixes: &mut Vec<String>, stats: &mut HashMap<&'static str, usize>) {
    // Py 333: `n = len(lines)` is captured ONCE and never re-read, even though
    // Py 369 `lines[i:j] = new_block` grows the list when a header separator row is
    // inserted. The stale bound means the final `delta` lines are never considered.
    // The previous port re-read `lines.len()`, so it scanned those extra lines.
    // Quirk preserved deliberately.
    let n = lines.len();
    let mut i = 0;
    while i < n {
        let l = &lines[i];
        if !l.contains('|') || looks_like_code_indent(l) || py_strip_start(l).starts_with('>') || (!is_table_row(l) && !is_table_sep(l)) {
            i += 1;
            continue;
        }

        let mut j = i;
        while j < n {
            let lj = &lines[j];
            if !lj.contains('|') || looks_like_code_indent(lj) || py_strip_start(lj).starts_with('>') || (!is_table_row(lj) && !is_table_sep(lj)) {
                break;
            }
            j += 1;
        }

        let block = &lines[i..j];
        let data_count = block.iter().filter(|x| is_table_row(x)).count();
        let has_sep = block.iter().any(|x| is_table_sep(x));

        if data_count == 0 || (data_count == 1 && !has_sep && !py_strip_start(&block[0]).starts_with('|')) {
            i = j;
            continue;
        }

        let (new_block, changed) = normalize_table(block, i + 1, fixes);
        if changed {
            *stats.entry("table").or_insert(0) += 1;
            let new_len = new_block.len();
            lines.splice(i..j, new_block);
            j = i + new_len;
        }
        i = j;
    }
}

// ----------------------------------------------------------------------------
// Headings
// ----------------------------------------------------------------------------

fn process_headings(lines: &mut Vec<String>, fixes: &mut Vec<String>, stats: &mut HashMap<&'static str, usize>) {
    for idx in 0..lines.len() {
        let line = &lines[idx];
        if !line.contains('#') || looks_like_code_indent(line) {
            continue;
        }
        if let Some(caps) = HEADING_RE.captures(line) {
            let indent = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let hashes = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            let rest = caps.get(3).map(|m| m.as_str()).unwrap_or("");
            lines[idx] = format!("{}{} {}", indent, hashes, rest);
            fixes.push(format!("[标题] 第 {} 行：# 后缺少空格，已补全", idx + 1));
            *stats.entry("heading").or_insert(0) += 1;
        }
    }
}

// ----------------------------------------------------------------------------
// Emphasis
// ----------------------------------------------------------------------------

fn is_punct_or_space(ch: char) -> bool {
    // Py 381 `if not ch or ch.isspace()` — `py_isspace` is that predicate ported
    // (Rust's `char::is_whitespace()` drops U+001C..U+001F). Getting this set wrong
    // is invisible at a glance because `mask_code_spans` injects the NEIGHBOURING
    // sentinel U+001A into the text: U+001A must be non-space, U+001C must be
    // space. `wd3_py_isspace_set_is_exact` pins both ends.
    // (The `not ch` arm lives at the call site: classify_delim uses Option<char>
    // and `.unwrap_or(true)`, which is Python's empty-string case.)
    if py_isspace(ch) {
        return true;
    }
    const PUNCT: &str = "([{'\"-\u{2013}\u{2014}:;,!?/\\|~`@#$%^&*+=<>~（）【】《》“”‘’，。、；：？！…—·「」『』";
    if PUNCT.contains(ch) || ch.is_ascii_punctuation() {
        return true;
    }
    // Py 385-387: `unicodedata.category(ch).startswith(('P', 'S', 'Z'))`.
    // The three general categories P / S / Z are exactly the Unicode property
    // classes used here, so this needs no new dependency. Without it every
    // codepoint that is punctuation-like or symbol-like but not ASCII (→ Sm,
    // © So, € Sc, • Sm, full-width forms, CJK punctuation beyond the literal set)
    // was misjudged, which flips is_open/is_close in classify_delim and therefore
    // decides whether an unclosed `**` gets completed.
    PUNCT_CLASS_RE.is_match(ch.encode_utf8(&mut [0u8; 4]))
}

/// Py 390-408 `_classify`. `p` is a CODE-POINT index into `s`, matching Python's
/// `s[p - 1]` / `s[p + len(d)]`, and the two neighbour lookups go through
/// `char_at` so an out-of-range index yields `None` exactly where Python yields
/// `''`. `char::is_whitespace()` is replaced by `py_isspace` at all three sites
/// (Py 395, 400, 401).
fn classify_delim(s: &str, p: usize, d: &str, allow_intraword: bool) -> &'static str {
    let before = if p > 0 { char_at(s, p - 1) } else { None };
    let after = char_at(s, p + d.chars().count());

    if (before.is_none() || py_isspace(before.unwrap())) && (after.is_none() || py_isspace(after.unwrap())) {
        return "close";
    }

    let prev_boundary = before.map(is_punct_or_space).unwrap_or(true);
    let next_boundary = after.map(is_punct_or_space).unwrap_or(true);
    let is_open = prev_boundary && !after.map(py_isspace).unwrap_or(false);
    let is_close = next_boundary && !before.map(py_isspace).unwrap_or(false);

    if is_open && !is_close {
        "open"
    } else if is_close && !is_open {
        "close"
    } else if is_open && is_close {
        "both"
    } else if allow_intraword {
        "open"
    } else {
        "word"
    }
}

fn balance_delim(mut s: String, d: &str, allow_intraword: bool) -> (String, Vec<String>) {
    // Py 412 `if s.strip() == d`. `py_strip` (not `str::trim`) both matches
    // CPython's whitespace set and keeps the `escape_delim(&s, d, 0)` cut below on
    // a code-point boundary.
    if py_strip(&s) == d {
        return (escape_delim(&s, d, 0), vec![format!("转义多余的 {}", d)]);
    }
    let pos = unescaped_positions(&s, d);
    if pos.is_empty() {
        return (s, Vec::new());
    }

    let mut opens = Vec::new();
    let mut strays = Vec::new();

    for p in pos {
        let kind = classify_delim(&s, p, d, allow_intraword);
        match kind {
            "open" => opens.push(p),
            "close" | "both" => {
                if !opens.is_empty() {
                    opens.pop();
                } else {
                    strays.push(p);
                }
            }
            _ => {
                if !opens.is_empty() {
                    opens.pop();
                }
            }
        }
    }

    if opens.is_empty() && strays.is_empty() {
        return (s, Vec::new());
    }

    let mut log = Vec::new();
    if opens.len() % 2 == 0 && !opens.is_empty() && !strays.is_empty() {
        if let Some(p) = opens.pop() {
            s = escape_delim(&s, d, p);
            log.push(format!("转义多余的 {}", d));
        }
    }
    if opens.len() % 2 == 1 {
        s = format!("{}{}", py_strip_end(&s), d);
        log.push(format!("补全未闭合的 {}", d));
    }
    // Py 448 escapes `strays` with positions computed against the string as it was
    // BEFORE the `escape_open` / rstrip mutations above -- the offsets are stale on
    // purpose in Python, and the slice simply saturates. `char_to_byte` reproduces
    // that saturating behaviour point for point; "fixing" the offsets would diverge.
    for p in strays.into_iter().rev() {
        s = escape_delim(&s, d, p);
        log.push(format!("转义多余的 {}", d));
    }

    (s, log)
}

/// Py 454-485 `mask_pairs`. Lifts every ALREADY-PAIRED `d ... d` run out into a
/// placeholder so the next, shorter delimiter's balancing pass cannot see it.
/// There is no Rust equivalent of this in the previous port at all, which is the
/// single largest behavioural divergence in this module.
fn mask_pairs(mut s: String, d: &str, prefix: &str) -> (String, Vec<(String, String)>) {
    let pos = unescaped_positions(&s, d);
    // Py 457: bail unless the count is non-zero AND even.
    if pos.is_empty() || pos.len() % 2 != 0 {
        return (s, Vec::new());
    }
    let mut open_p: Option<usize> = None;
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    for &p in &pos {
        // Note: `_classify` is evaluated against the ORIGINAL `s`, before any
        // placeholder substitution (Py 462 runs inside the un-mutated loop).
        // Py 463-476: 'open' OVERWRITES a pending opener without emitting a pair --
        // unlike balance_delim there is no stack. close/both/word all behave alike.
        if classify_delim(&s, p, d, false) == "open" {
            open_p = Some(p);
        } else if let Some(a) = open_p {
            pairs.push((a, p));
            open_p = None;
        }
    }
    if pairs.is_empty() {
        return (s, Vec::new());
    }
    // Py 479 `pairs.sort(reverse=True)` -- descending lexicographic on (a, b), so
    // substituting from the right leaves the earlier CODE-POINT offsets valid.
    // (a, b) come from `unescaped_positions`, i.e. Python's `str.find` unit, so
    // every cut below is re-based through `char_to_byte` (Py 483-484).
    pairs.sort_by(|x, y| y.cmp(x));
    let mut spans: Vec<(String, String)> = Vec::new();
    for (i, &(a, b)) in pairs.iter().enumerate() {
        // Py 482 `'\x1a%s%d\x1a' % (prefix, i)` -- prefix is "3", "2" or "u".
        let ph = format!("\x1a{}{}\x1a", prefix, i);
        let ba = char_to_byte(&s, a);
        let bb = char_to_byte(&s, b + d.chars().count());
        spans.push((ph.clone(), s[ba..bb].to_string()));
        s = format!("{}{}{}", &s[..ba], ph, &s[bb..]);
    }
    (s, spans)
}

fn fix_emphasis_line(line: &str) -> (String, Vec<String>) {
    if HR_RE.is_match(line) {
        return (line.to_string(), Vec::new());
    }
    let mut log = Vec::new();
    let mut s = line.to_string();
    let mut all_spans: Vec<(String, String)> = Vec::new();

    // Py 496-509: balance, then immediately mask the resulting pairs, per delimiter.
    // The interleaving IS the semantics -- skipping mask_pairs makes the shorter
    // delimiter see text the longer one already resolved.
    let (s_3, l3) = balance_delim(s, "***", false);
    s = s_3;
    log.extend(l3);
    let (s_3m, sp3) = mask_pairs(s, "***", "3");
    s = s_3m;
    all_spans.extend(sp3);

    let (s_2, l2) = balance_delim(s, "**", false);
    s = s_2;
    log.extend(l2);
    let (s_2m, sp2) = mask_pairs(s, "**", "2");
    s = s_2m;
    all_spans.extend(sp2);

    let (s_u, lu) = balance_delim(s, "__", false);
    s = s_u;
    log.extend(lu);
    let (s_um, spu) = mask_pairs(s, "__", "u");
    s = s_um;
    all_spans.extend(spu);

    if !LIST_ITEM_RE.is_match(py_strip_start(&s)) {
        let (s_1, l1) = balance_delim(s, "*", false);
        s = s_1;
        log.extend(l1);
    }

    // Py 515-517: `for ph, orig in reversed(all_spans): s = s.replace(ph, orig)`.
    // Reverse order and str.replace's replace-all semantics both matter here.
    // The `\x1a<alnum>\x1a` shape is also matched by RESTORE_RE, so this unmask must
    // happen before fix_markdown's final restore() -- it does, and it did before.
    for (ph, orig) in all_spans.iter().rev() {
        s = s.replace(ph.as_str(), orig.as_str());
    }
    (s, log)
}

fn process_emphasis(lines: &mut Vec<String>, fixes: &mut Vec<String>, stats: &mut HashMap<&'static str, usize>) {
    for idx in 0..lines.len() {
        let line = &lines[idx];
        if (!line.contains('*') && !line.contains('_')) || looks_like_code_indent(line) {
            continue;
        }
        let (fixed, log) = fix_emphasis_line(line);
        if !log.is_empty() {
            lines[idx] = fixed;
            for m in &log {
                fixes.push(format!("[加粗] 第 {} 行：{}", idx + 1, m));
            }
            *stats.entry("bold").or_insert(0) += log.len();
        }
    }
}

// ----------------------------------------------------------------------------
// Math Formulas
// ----------------------------------------------------------------------------

fn balance_display_math(mut text: String) -> (String, Vec<String>) {
    // Py 538's `\s` is CPython's, which accepts U+001C..U+001F too.
    let re_empty = Regex::new(r"\$\$[\s\x1c-\x1f]*\n[\s\x1c-\x1f]*\$\$").unwrap();
    // Py 538 `re.sub(r'\$\$\s*\n\s*\$\$', '$$', text)`. In Python a replacement
    // string carries no `$` semantics, so `'$$'` writes TWO dollars. In the
    // `regex` crate `$$` is the escape for a literal SINGLE `$`, so the old port
    // collapsed `$$\n$$` to `$` — destroying a math delimiter instead of removing
    // an empty block. Four `$` yields two literal ones.
    // re.sub's default count=0 means "replace every occurrence" => replace_all.
    let replaced = re_empty.replace_all(&text, "$$$$").into_owned();
    if replaced != text {
        return (replaced, vec!["移除空白的 $$ 公式块".to_string()]);
    }

    let pos = unescaped_positions(&text, "$$");
    if pos.is_empty() || pos.len() % 2 == 0 {
        return (text, Vec::new());
    }

    let last = pos[pos.len() - 1];
    if pos.len() > 1 && (pos.len() - 1) % 2 == 1 {
        text = escape_delim(&text, "$$", last);
        return (text, vec!["转义多余的块级 $$".to_string()]);
    }

    // Py 553 `text[last + 2:]` with `last` a CODE-POINT index from `_unescaped_positions`.
    let cut = char_to_byte(&text, last + 2);
    let after = text[cut..].to_string();
    let lines: Vec<&str> = after.split('\n').collect();
    let content = if !lines.is_empty() && lines[0].is_empty() { &lines[1..] } else { &lines[..] };

    if py_strip(&after).is_empty() {
        text = format!("{}\n$$\n", &text[..cut]);
    } else if content.is_empty() || py_strip(content[0]).is_empty() {
        text = format!("{}\n$$\n{}", &text[..cut], after.trim_start_matches('\n'));
    } else {
        let mut n = 0usize;
        for l in content {
            if py_strip(l).is_empty() || l.starts_with("\x1aF") {
                break;
            }
            n += 1;
        }
        // Py 565-566: `head = lines[:1 + n]` / `rest = lines[1 + n:]`. The `1 +` is
        // UNCONDITIONAL and indexes `lines`, never `content`. The old port used
        // `offset = if lines[0].is_empty() { 1 } else { 0 }`, so for text like
        // `inline $$x = 1 tail` (lines[0] non-empty) head lost one line.
        let end_idx = (1 + n).min(lines.len());
        let head = &lines[..end_idx];
        let rest = &lines[end_idx..];
        // Py 567: `'\n'.join(head) + '\n$$\n' + ('\n'.join(rest) + '\n' if rest else '')`.
        // `if rest` is a LIST truthiness test, and the `+ '\n'` re-attaches the
        // newline that split('\n') consumed — dropping it lost the document's
        // trailing newline. CPython: `$$\nx = y + z\n` -> `$$\nx = y + z\n$$\n\n`.
        let mut after2 = head.join("\n");
        after2.push_str("\n$$\n");
        if !rest.is_empty() {
            after2.push_str(&rest.join("\n"));
            after2.push('\n');
        }
        text = format!("{}{}", &text[..cut], after2);
    }

    (text, vec!["补全未闭合的块级公式 $$".to_string()])
}

fn fix_math_line(line: &str) -> (String, Vec<String>) {
    let mut log = Vec::new();
    let mut s = line.to_string();

    let delimiters = [
        (r"\(", r"\)", r"\( ... \)"),
        (r"\[", r"\]", r"\[ ... \]"),
    ];

    for (op, cl, name) in delimiters {
        let o = s.matches(op).count();
        let c = s.matches(cl).count();
        if o > c && has_latex(&s) {
            s = format!("{}{}", py_strip_end(&s), cl);
            log.push(format!("补全未闭合的 {}", name));
        } else if c > o {
            if let Some(p) = py_find(&s, cl) {
                // Py 583-586, verbatim quirk:
                //     s = s[:p] + '\\\\' + s[p + 1:]
                // `cl` is two characters (`\)` or `\]`) but the slice advances by
                // only 1, so the `)` / `]` is KEPT and two backslashes are inserted.
                // The previous port "corrected" this to `p + cl.len()`, which deleted
                // the bracket. CPython: `text \) close only` -> `text \\) close only`.
                s = format!("{}\\\\{}", &s[..char_to_byte(&s, p)], &s[char_to_byte(&s, p + 1)..]);
                log.push(format!("转义多余的 {}", cl));
            }
        }
    }

    // Py 571 `for i, ch in enumerate(s)` -- `i` counts CODE POINTS. `is_escaped`
    // still wants the byte offset, hence carrying both.
    let mut dollars: Vec<usize> = Vec::new();
    for (ci, (bi, ch)) in s.char_indices().enumerate() {
        if ch == '$' && !is_escaped(&s, bi) {
            dollars.push(ci);
        }
    }

    if dollars.len() % 2 == 1 {
        if has_latex(&s) {
            let p0 = dollars[0];
            // Py 592 `s[p0 + 1] if p0 + 1 < len(s) else ''`.
            let after = char_at(&s, p0 + 1);
            if after.map(|c| !py_isspace(c)).unwrap_or(false) {
                s = format!("{}$", py_strip_end(&s));
                log.push("补全未闭合的行内公式 $".to_string());
            } else {
                s = escape_at(&s, p0);
                log.push("转义多余的 $".to_string());
            }
        } else {
            s = escape_all_unescaped(&s, '$');
            log.push("转义疑似货币的 $".to_string());
        }
    }

    (s, log)
}

fn process_math(lines: &mut Vec<String>, fixes: &mut Vec<String>, stats: &mut HashMap<&'static str, usize>) {
    let text = lines.join("\n");
    if text.contains("$$") {
        let (new_text, log) = balance_display_math(text);
        if !log.is_empty() {
            fixes.push(format!("[公式] {}", log[0]));
            *stats.entry("math").or_insert(0) += 1;
            *lines = new_text.split('\n').map(|s| s.to_string()).collect();
        }
    }

    let joined = lines.join("\n");
    // Py 612 tests all five: '$', '\(', '\[', '\)', '\]'. Omitting the two closers
    // meant a document holding only a stray `\]` returned early and was never
    // escaped. CPython: `just a \] here` -> `just a \\] here`.
    if !joined.contains('$')
        && !joined.contains(r"\(")
        && !joined.contains(r"\[")
        && !joined.contains(r"\)")
        && !joined.contains(r"\]")
    {
        return;
    }

    for idx in 0..lines.len() {
        let line = &lines[idx];
        if (!line.contains('$') && !line.contains('\\')) || looks_like_code_indent(line) {
            continue;
        }
        let (fixed, log2) = fix_math_line(line);
        if !log2.is_empty() {
            lines[idx] = fixed;
            for m in &log2 {
                fixes.push(format!("[公式] 第 {} 行：{}", idx + 1, m));
            }
            *stats.entry("math").or_insert(0) += log2.len();
        }
    }
}

// ----------------------------------------------------------------------------
// Main Entrypoint
// ----------------------------------------------------------------------------

pub fn fix_markdown(mut text: &str) -> FixResult {
    let mut fixes = Vec::new();
    let mut stats = HashMap::new();
    stats.insert("table", 0);
    stats.insert("bold", 0);
    stats.insert("math", 0);
    stats.insert("heading", 0);
    stats.insert("misc", 0);

    if text.starts_with('\u{feff}') {
        text = &text['\u{feff}'.len_utf8()..];
        *stats.entry("misc").or_insert(0) += 1;
        fixes.push("[通用] 已去除 UTF-8 BOM".to_string());
    }

    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let (masked, spans) = mask_all_code(&normalized);
    let mut lines: Vec<String> = masked.split('\n').map(|s| s.to_string()).collect();

    process_tables(&mut lines, &mut fixes, &mut stats);
    process_headings(&mut lines, &mut fixes, &mut stats);
    process_emphasis(&mut lines, &mut fixes, &mut stats);
    process_math(&mut lines, &mut fixes, &mut stats);

    let restored = restore(&lines.join("\n"), &spans);

        FixResult {
        text: restored,
        fixes,
        stats: json!({
            "table": stats.get("table").cloned().unwrap_or(0),
            "bold": stats.get("bold").cloned().unwrap_or(0),
            "math": stats.get("math").cloned().unwrap_or(0),
            "heading": stats.get("heading").cloned().unwrap_or(0),
            "misc": stats.get("misc").cloned().unwrap_or(0),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heading_fix() {
        let input = "#Title\n##Subtitle";
        let res = fix_markdown(input);
        assert_eq!(res.text, "# Title\n## Subtitle");
        assert_eq!(res.fixes.len(), 2);
    }

    #[test]
    fn test_bold_fix() {
        let input = "This has **unclosed bold text\nAnother line";
        let res = fix_markdown(input);
        assert!(res.text.contains("**unclosed bold text**"));
        assert!(res.fixes.iter().any(|f| f.contains("补全未闭合的 **")));
    }

    #[test]
    fn test_table_fix() {
        let input = "| Column 1 | Column 2 |\n| Val 1 |";
        let res = fix_markdown(input);
        assert!(res.text.contains("---"));
        assert!(res.fixes.iter().any(|f| f.contains("缺少表头分隔行")));
    }

    #[test]
    fn test_display_math_fix() {
        let input = "$$\nx = y + z\n";
        let res = fix_markdown(input);
        assert!(res.text.contains("$$"));
        assert!(res.fixes.iter().any(|f| f.contains("未闭合的块级公式")));
    }

    #[test]
    fn test_standalone_fix_input() {
        let input = "#Heading\n\n| a | b |\n| 1 | 2 |\n\nSome **unclosed bold and $$unclosed formula";
        let res = fix_markdown(input);
        println!("FIXED RESULT:\n{}", res.text);
        assert!(res.text.contains("# Heading"));
        assert!(res.text.contains("| --- | --- |"));
    }
}

// ==========================================================================
// Differential parity tests. Every expected value in this module was
// GENERATED by running the real authority, src/readmd_core/readmd_fix.py,
// under CPython 3.11.15 (scratch/rust_parity/wb4/gen_golden.py ->
// golden.json). No expected string below was typed by hand.
// ==========================================================================

#[cfg(test)]
mod parity_tests {
    use super::*;

    fn assert_case(name: &str, input: &str, want_text: &str, want_fixes: &[&str], want_stats: Value) {
        let res = fix_markdown(input);
        assert_eq!(res.text, want_text, "text mismatch for case `{}`", name);
        let got: Vec<&str> = res.fixes.iter().map(|s| s.as_str()).collect();
        assert_eq!(got, want_fixes.to_vec(), "fixes mismatch for case `{}`", name);
        assert_eq!(res.stats, want_stats, "stats mismatch for case `{}`", name);
    }

    /// CPython golden. Rule: Py 630 _process_headings / _HEADING_RE
    #[test]
    fn py_heading_basic() {
        assert_case(
            "heading_basic",
            "#Title\n##Subtitle\n### No change\n\n####### seven hashes\n#hashtag ok\n#- dash heading",
            "# Title\n## Subtitle\n### No change\n\n####### seven hashes\n# hashtag ok\n# - dash heading",
            &[
                "[标题] 第 1 行：# 后缺少空格，已补全",
                "[标题] 第 2 行：# 后缺少空格，已补全",
                "[标题] 第 6 行：# 后缺少空格，已补全",
                "[标题] 第 7 行：# 后缺少空格，已补全",
            ],
            json!({"table": 0, "bold": 0, "math": 0, "heading": 4, "misc": 0}),
        );
    }

    /// CPython golden. Rule: Py 33 third group must not start with # or space
    #[test]
    fn py_heading_hashword() {
        assert_case(
            "heading_hashword",
            "##Not a heading##\n##Real",
            "## Not a heading##\n## Real",
            &[
                "[标题] 第 1 行：# 后缺少空格，已补全",
                "[标题] 第 2 行：# 后缺少空格，已补全",
            ],
            json!({"table": 0, "bold": 0, "math": 0, "heading": 2, "misc": 0}),
        );
    }

    /// CPython golden. Rule: Py 87 _looks_like_code_indent ordered list (F1)
    #[test]
    fn py_heading_code_indent() {
        assert_case(
            "heading_code_indent",
            "    1. item **x\n    2. second\n\tTabbed * item\n    > quote * star\n    - list * star",
            "    1. item **x**\n    2. second\n\tTabbed * item\n    > quote \\* star\n    - list \\* star",
            &[
                "[加粗] 第 1 行：补全未闭合的 **",
                "[加粗] 第 4 行：转义多余的 *",
                "[加粗] 第 5 行：转义多余的 *",
            ],
            json!({"table": 0, "bold": 3, "math": 0, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: Py 411 _balance_delim '**'
    #[test]
    fn py_bold_unclosed() {
        assert_case(
            "bold_unclosed",
            "This has **unclosed bold text\nAnother line",
            "This has **unclosed bold text**\nAnother line",
            &[
                "[加粗] 第 1 行：补全未闭合的 **",
            ],
            json!({"table": 0, "bold": 1, "math": 0, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: Py 394 lone delimiter both sides blank
    #[test]
    fn py_bold_stray() {
        assert_case(
            "bold_stray",
            "2 * 3 = 6 and 4 * 5 = 20",
            "2 \\* 3 = 6 and 4 \\* 5 = 20",
            &[
                "[加粗] 第 1 行：转义多余的 *",
                "[加粗] 第 1 行：转义多余的 *",
            ],
            json!({"table": 0, "bold": 2, "math": 0, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: Py 434 word kind
    #[test]
    fn py_bold_intraword() {
        assert_case(
            "bold_intraword",
            "foo__bar and snake__case__here",
            "foo__bar and snake__case__here",
            &[],
            json!({"table": 0, "bold": 0, "math": 0, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: Py 496-498 *** then mask_pairs
    #[test]
    fn py_bold_star3() {
        assert_case(
            "bold_star3",
            "***bold italic and **nested bold** and *italic*\n",
            "***bold italic and **nested bold** and *italic***\\*\n",
            &[
                "[加粗] 第 1 行：补全未闭合的 ***",
                "[加粗] 第 1 行：转义多余的 *",
            ],
            json!({"table": 0, "bold": 2, "math": 0, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: mask_pairs vs ** balance (F5)
    #[test]
    fn py_bold_mix_order() {
        assert_case(
            "bold_mix_order",
            "a **b *c** d* e\n",
            "a **b *c** d\\* e\n",
            &[
                "[加粗] 第 1 行：转义多余的 *",
            ],
            json!({"table": 0, "bold": 1, "math": 0, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: Py 498 mask_pairs('**','2')
    #[test]
    fn py_bold_paired_keep() {
        assert_case(
            "bold_paired_keep",
            "**one** and **two** and **three\n",
            "**one** and **two** and **three**\n",
            &[
                "[加粗] 第 1 行：补全未闭合的 **",
            ],
            json!({"table": 0, "bold": 1, "math": 0, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: Py 490 _HR_RE early return
    #[test]
    fn py_bold_hr_skipped() {
        assert_case(
            "bold_hr_skipped",
            "***\n-----\n_____\n",
            "***\n-----\n_____\n",
            &[],
            json!({"table": 0, "bold": 0, "math": 0, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: Py 511 _LIST_ITEM_RE guard
    #[test]
    fn py_bold_list_guard() {
        assert_case(
            "bold_list_guard",
            "* item with *unclosed star\n+ plus *unclosed\n",
            "* item with *unclosed star\n+ plus *unclosed\n",
            &[],
            json!({"table": 0, "bold": 0, "math": 0, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: Py 385 unicodedata P/S/Z fallback (F4)
    #[test]
    fn py_bold_punct_class() {
        assert_case(
            "bold_punct_class",
            "→**arrow**\n©**copyright**\n•**bullet**\n€**euro**\n±**plusminus**\n",
            "→**arrow**\n©**copyright**\n•**bullet**\n€**euro**\n±**plusminus**\n",
            &[],
            json!({"table": 0, "bold": 0, "math": 0, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: Py 377 CJK punct set
    #[test]
    fn py_bold_punct_cjk() {
        assert_case(
            "bold_punct_cjk",
            "（**fullwidth**）\n《**book**》\n—**emdash**\n",
            "（**fullwidth**）\n《**book**》\n—**emdash**\n",
            &[],
            json!({"table": 0, "bold": 0, "math": 0, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: Py 321 insert header separator
    #[test]
    fn py_table_missing_sep() {
        assert_case(
            "table_missing_sep",
            "| Column 1 | Column 2 |\n| Val 1 | Val 2 |",
            "| Column 1 | Column 2 |\n| --- | --- |\n| Val 1 | Val 2 |",
            &[
                "[表格] 第 1 行附近：缺少表头分隔行，已自动补全",
            ],
            json!({"table": 1, "bold": 0, "math": 0, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: Py 326 row_diffs report
    #[test]
    fn py_table_ragged() {
        assert_case(
            "table_ragged",
            "| a | b | c |\n|---|---|---|\n| 1 | 2 |\n| 3 | 4 | 5 | 6 |",
            "| a | b | c |  |\n| --- | --- | --- | --- |\n| 1 | 2 |  |  |\n| 3 | 4 | 5 | 6 |",
            &[
                "[表格] 第 1-4 行：2 行列数不齐，已对齐为 4 列",
            ],
            json!({"table": 1, "bold": 0, "math": 0, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: Py 271 _norm_sep_cell (F2)
    #[test]
    fn py_table_align_cells() {
        assert_case(
            "table_align_cells",
            "| a | b | c | d | e | f |\n|---|-----|:-:|:-|:|:---:|\n| 1 | 2 | 3 | 4 | 5 | 6 |",
            "| a | b | c | d | e | f |\n| --- | --- | --- | --- | --- | --- |\n| --- | ----- | :-: | :- | : | :---: |\n| 1 | 2 | 3 | 4 | 5 | 6 |",
            &[
                "[表格] 第 1 行附近：缺少表头分隔行，已自动补全",
            ],
            json!({"table": 1, "bold": 0, "math": 0, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: Py 267 _escape_cell
    #[test]
    fn py_table_escape_pipe() {
        assert_case(
            "table_escape_pipe",
            "| a | b |\n|---|---|\n| x|y | ok |",
            "| a | b |  |\n| --- | --- | --- |\n| x | y | ok |",
            &[
                "[表格] 第 1-3 行：1 行列数不齐，已对齐为 3 列",
            ],
            json!({"table": 1, "bold": 0, "math": 0, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: Py 363 block[0] not startswith |
    #[test]
    fn py_table_no_leading_pipe() {
        assert_case(
            "table_no_leading_pipe",
            "a | b\n1 | 2",
            "| a | b |\n| --- | --- |\n| 1 | 2 |",
            &[
                "[表格] 第 1 行附近：缺少表头分隔行，已自动补全",
            ],
            json!({"table": 1, "bold": 0, "math": 0, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: Py 280 indent preserved
    #[test]
    fn py_table_indented() {
        assert_case(
            "table_indented",
            "  | a | b |\n  | 1 | 2 |",
            "  | a | b |\n  | --- | --- |\n  | 1 | 2 |",
            &[
                "[表格] 第 1 行附近：缺少表头分隔行，已自动补全",
            ],
            json!({"table": 1, "bold": 0, "math": 0, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: Py 111 mask_code_spans
    #[test]
    fn py_code_inline_protect() {
        assert_case(
            "code_inline_protect",
            "Use `**not bold**` and `` ` `` and `a**b`\n",
            "Use `**not bold**` and `` ` `` and `a**b`\n",
            &[],
            json!({"table": 0, "bold": 0, "math": 0, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: Py 165 mask_all_code
    #[test]
    fn py_code_fence_protect() {
        assert_case(
            "code_fence_protect",
            "```py\n#Title\n| a | b |\n**unclosed\n```\n\nafter **bold\n",
            "```py\n#Title\n| a | b |\n**unclosed\n```\n\nafter **bold**\n",
            &[
                "[加粗] 第 7 行：补全未闭合的 **",
            ],
            json!({"table": 0, "bold": 1, "math": 0, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: Py 553-555 after blank
    #[test]
    fn py_math_display_unclosed() {
        assert_case(
            "math_display_unclosed",
            "$$\nx = y + z\n",
            "$$\nx = y + z\n$$\n\n",
            &[
                "[公式] 补全未闭合的块级公式 $$",
            ],
            json!({"table": 0, "bold": 0, "math": 1, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: Py 559-568 trailing newline (F8)
    #[test]
    fn py_math_display_unclosed_nl() {
        assert_case(
            "math_display_unclosed_nl",
            "$$\nx = y + z\n",
            "$$\nx = y + z\n$$\n\n",
            &[
                "[公式] 补全未闭合的块级公式 $$",
            ],
            json!({"table": 0, "bold": 0, "math": 1, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: Py 538 re.sub collapse (F6)
    #[test]
    fn py_math_display_empty() {
        assert_case(
            "math_display_empty",
            "$$\n$$\n",
            "$$\n",
            &[
                "[公式] 移除空白的 $$ 公式块",
            ],
            json!({"table": 0, "bold": 0, "math": 1, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: Py 565 lines[:1+n] off-by-one (F7)
    #[test]
    fn py_math_display_inline_open() {
        assert_case(
            "math_display_inline_open",
            "inline $$x = 1 tail\nnext line\n\nblank after\n",
            "inline $$x = 1 tail\nnext line\n\n$$\nblank after\n\n",
            &[
                "[公式] 补全未闭合的块级公式 $$",
            ],
            json!({"table": 0, "bold": 0, "math": 1, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: Py 542 even count no-op
    #[test]
    fn py_math_display_two_blocks() {
        assert_case(
            "math_display_two_blocks",
            "$$\na\n$$\n\n$$\nb\n$$\n",
            "$$\na\n$$\nb\n$$\n",
            &[
                "[公式] 移除空白的 $$ 公式块",
            ],
            json!({"table": 0, "bold": 0, "math": 1, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: Py 579 o>c append cl
    #[test]
    fn py_math_paren_open() {
        assert_case(
            "math_paren_open",
            "text \\(a+b and more\n",
            "text \\(a+b and more\\)\n",
            &[
                "[公式] 第 1 行：补全未闭合的 \\( ... \\)",
            ],
            json!({"table": 0, "bold": 0, "math": 1, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: Py 579 \[ ... \]
    #[test]
    fn py_math_bracket_open() {
        assert_case(
            "math_bracket_open",
            "text \\[a+b and more\n",
            "text \\[a+b and more\\]\n",
            &[
                "[公式] 第 1 行：补全未闭合的 \\[ ... \\]",
            ],
            json!({"table": 0, "bold": 0, "math": 1, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: Py 585 s[p+1:] keeps paren (F9)
    #[test]
    fn py_math_paren_stray() {
        assert_case(
            "math_paren_stray",
            "text \\) close only\n",
            "text \\\\) close only\n",
            &[
                "[公式] 第 1 行：转义多余的 \\)",
            ],
            json!({"table": 0, "bold": 0, "math": 1, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: Py 585 (F9)
    #[test]
    fn py_math_bracket_stray() {
        assert_case(
            "math_bracket_stray",
            "text \\] close only\n",
            "text \\\\] close only\n",
            &[
                "[公式] 第 1 行：转义多余的 \\]",
            ],
            json!({"table": 0, "bold": 0, "math": 1, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: Py 612 guard needs \) \] too (F10)
    #[test]
    fn py_math_only_closer_guard() {
        assert_case(
            "math_only_closer_guard",
            "just a \\] here\n",
            "just a \\\\] here\n",
            &[
                "[公式] 第 1 行：转义多余的 \\]",
            ],
            json!({"table": 0, "bold": 0, "math": 1, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: Py 593 rstrip + $
    #[test]
    fn py_math_inline_dollar() {
        assert_case(
            "math_inline_dollar",
            "value $x^2 + 1 = 3\n",
            "value $x^2 + 1 = 3$\n",
            &[
                "[公式] 第 1 行：补全未闭合的行内公式 $",
            ],
            json!({"table": 0, "bold": 0, "math": 1, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: Py 599 _escape_all_unescaped
    #[test]
    fn py_math_currency() {
        assert_case(
            "math_currency",
            "Costs $5 and $6 total\n",
            "Costs $5 and $6 total\n",
            &[],
            json!({"table": 0, "bold": 0, "math": 0, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: Py 596 _escape_at
    #[test]
    fn py_math_dollar_lone() {
        assert_case(
            "math_dollar_lone",
            "a $ b \\(x\\)\n",
            "a \\$ b \\(x\\)\n",
            &[
                "[公式] 第 1 行：转义多余的 $",
            ],
            json!({"table": 0, "bold": 0, "math": 1, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: Py 650 BOM
    #[test]
    fn py_bom_strip() {
        assert_case(
            "bom_strip",
            "﻿#Title\nbody\n",
            "# Title\nbody\n",
            &[
                "[通用] 已去除 UTF-8 BOM",
                "[标题] 第 1 行：# 后缺少空格，已补全",
            ],
            json!({"table": 0, "bold": 0, "math": 0, "heading": 1, "misc": 1}),
        );
    }

    /// CPython golden. Rule: Py 654 CRLF/CR
    #[test]
    fn py_crlf_normalise() {
        assert_case(
            "crlf_normalise",
            "#T\r\nbody **x\rmore\n",
            "# T\nbody **x**\nmore\n",
            &[
                "[标题] 第 1 行：# 后缺少空格，已补全",
                "[加粗] 第 2 行：补全未闭合的 **",
            ],
            json!({"table": 0, "bold": 1, "math": 0, "heading": 1, "misc": 0}),
        );
    }

    /// CPython golden. Rule: phase order tables->headings->emphasis->math
    #[test]
    fn py_combined() {
        assert_case(
            "combined",
            "#Title\n| a | b |\n| 1 | 2 |\n\nText **unclosed and $x^2$\n",
            "# Title\n| a | b |\n| --- | --- |\n| 1 | 2 |\n\nText **unclosed and $x^2$**\n",
            &[
                "[表格] 第 2 行附近：缺少表头分隔行，已自动补全",
                "[标题] 第 1 行：# 后缺少空格，已补全",
                "[加粗] 第 6 行：补全未闭合的 **",
            ],
            json!({"table": 1, "bold": 1, "math": 0, "heading": 1, "misc": 0}),
        );
    }

    /// CPython golden. Rule: Py: nothing to fix
    #[test]
    fn py_noop_clean() {
        assert_case(
            "noop_clean",
            "# Title\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n**bold** and `code`\n",
            "# Title\n\n| a | b |\n| --- | --- |\n| 1 | 2 |\n\n**bold** and `code`\n",
            &[],
            json!({"table": 1, "bold": 0, "math": 0, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: F2: 2-dash widens, 4/5-dash stay verbatim
    #[test]
    fn py_f2_sep_dashes() {
        assert_case(
            "f2_sep_dashes",
            "| a | b | c | d |\n|--|:--:|----|-----|\n| 1 | 2 | 3 | 4 |",
            "| a | b | c | d |\n| --- | :---: | ---- | ----- |\n| 1 | 2 | 3 | 4 |",
            &[],
            json!({"table": 1, "bold": 0, "math": 0, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: F2: nothing to widen must be byte-identical
    #[test]
    fn py_f2_sep_all_long() {
        assert_case(
            "f2_sep_all_long",
            "| a | b |\n|------|--------|\n| 1 | 2 |",
            "| a | b |\n| ------ | -------- |\n| 1 | 2 |",
            &[],
            json!({"table": 1, "bold": 0, "math": 0, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: F2: `:` cell cannot match `-+`
    #[test]
    fn py_f2_sep_one_colon() {
        assert_case(
            "f2_sep_one_colon",
            "| a | b |\n|--|:-|\n| 1 | 2 |",
            "| a | b |\n| --- | --- |\n| -- | :- |\n| 1 | 2 |",
            &[
                "[表格] 第 1 行附近：缺少表头分隔行，已自动补全",
            ],
            json!({"table": 1, "bold": 0, "math": 0, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: F4: U+2192 is Sm, needs the P/S/Z fallback to open
    #[test]
    fn py_f4_arrow_open() {
        assert_case(
            "f4_arrow_open",
            "a→**b\ncost→50%**off\n",
            "a→**b**\ncost→50%**off**\n",
            &[
                "[加粗] 第 1 行：补全未闭合的 **",
                "[加粗] 第 2 行：补全未闭合的 **",
            ],
            json!({"table": 0, "bold": 2, "math": 0, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: F4: symbol boundary decides open vs word
    #[test]
    fn py_f4_symbol_intraword() {
        assert_case(
            "f4_symbol_intraword",
            "x=©**y\n",
            "x=©**y**\n",
            &[
                "[加粗] 第 1 行：补全未闭合的 **",
            ],
            json!({"table": 0, "bold": 1, "math": 0, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: F5: odd unescaped count means mask_pairs bails
    #[test]
    fn py_f5_mask_odd_gate() {
        assert_case(
            "f5_mask_odd_gate",
            "**a** and **b and c**\n",
            "**a** and **b and c**\n",
            &[],
            json!({"table": 0, "bold": 0, "math": 0, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: F5: *** masked before * is balanced
    #[test]
    fn py_f5_mask_triple_then_star() {
        assert_case(
            "f5_mask_triple_then_star",
            "***a** and *b*\n",
            "***a** and *b***\\*\n",
            &[
                "[加粗] 第 1 行：补全未闭合的 ***",
                "[加粗] 第 1 行：转义多余的 *",
            ],
            json!({"table": 0, "bold": 2, "math": 0, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: F5: __ masked with prefix u
    #[test]
    fn py_f5_underscore_pair() {
        assert_case(
            "f5_underscore_pair",
            "__one__ and __two__ and __three\n",
            "__one__ and __two__ and __three__\n",
            &[
                "[加粗] 第 1 行：补全未闭合的 __",
            ],
            json!({"table": 0, "bold": 1, "math": 0, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: F9: both stray closers on one line
    #[test]
    fn py_f9_both_closers() {
        assert_case(
            "f9_both_closers",
            "a \\) b \\] c\n",
            "a \\\\) b \\\\] c\n",
            &[
                "[公式] 第 1 行：转义多余的 \\)",
                "[公式] 第 1 行：转义多余的 \\]",
            ],
            json!({"table": 0, "bold": 0, "math": 2, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: F3: table at end of doc then extra lines
    #[test]
    fn py_f3_stale_bound() {
        assert_case(
            "f3_stale_bound",
            "| a | b |\n| 1 | 2 |\n\n| c | d |\n| 3 | 4 |\n",
            "| a | b |\n| --- | --- |\n| 1 | 2 |\n\n| c | d |\n| --- | --- |\n| 3 | 4 |\n",
            &[
                "[表格] 第 1 行附近：缺少表头分隔行，已自动补全",
                "[表格] 第 5 行附近：缺少表头分隔行，已自动补全",
            ],
            json!({"table": 2, "bold": 0, "math": 0, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: F8: trailing newlines survive the completion
    #[test]
    fn py_f8_two_trailing() {
        assert_case(
            "f8_two_trailing",
            "$$\nx\n$$$\n",
            "$$\nx\n\\$\\$\\$\n",
            &[
                "[公式] 第 3 行：转义疑似货币的 $",
            ],
            json!({"table": 0, "bold": 0, "math": 1, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: Py 413 `if s.strip() == d` branch
    #[test]
    fn py_x_solo() {
        assert_case(
            "x_solo",
            "**\nbody\n",
            "\\*\\*\nbody\n",
            &[
                "[加粗] 第 1 行：转义多余的 **",
            ],
            json!({"table": 0, "bold": 1, "math": 0, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: Py 413 for a lone *
    #[test]
    fn py_star_solo() {
        assert_case(
            "star_solo",
            "*\nbody\n",
            "\\*\nbody\n",
            &[
                "[加粗] 第 1 行：转义多余的 *",
            ],
            json!({"table": 0, "bold": 1, "math": 0, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: Py 490 _HR_RE guards *** on its own line
    #[test]
    fn py_triple_solo() {
        assert_case(
            "triple_solo",
            "***\nbody\n",
            "***\nbody\n",
            &[],
            json!({"table": 0, "bold": 0, "math": 0, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: mask_pairs ph vs mask_code_spans ph must not collide
    #[test]
    fn py_code_and_emphasis() {
        assert_case(
            "code_and_emphasis",
            "a `**x**` b **unclosed and *stray * c\n",
            "a `**x**` b **unclosed and *stray * c**\n",
            &[
                "[加粗] 第 1 行：补全未闭合的 **",
            ],
            json!({"table": 0, "bold": 1, "math": 0, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: fence masks the table entirely
    #[test]
    fn py_table_in_fence() {
        assert_case(
            "table_in_fence",
            "```\n| a | b |\n| 1 |\n```\nreal | table | x\ny | z | w\n",
            "```\n| a | b |\n| 1 |\n```\n| real | table | x |\n| --- | --- | --- |\n| y | z | w |\n",
            &[
                "[表格] 第 5 行附近：缺少表头分隔行，已自动补全",
            ],
            json!({"table": 1, "bold": 0, "math": 0, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: fence masks $$ so display math must not see it
    #[test]
    fn py_math_in_fence() {
        assert_case(
            "math_in_fence",
            "```\n$$\nx\n```\nbody\n",
            "```\n$$\nx\n```\nbody\n",
            &[],
            json!({"table": 0, "bold": 0, "math": 0, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: Py 47 _unescaped_positions skips \$
    #[test]
    fn py_escaped_dollars() {
        assert_case(
            "escaped_dollars",
            "price \\$$ not a delimiter $$x = 1$$ end\n",
            "price \\$\\$ not a delimiter $$x = 1$$ end\n",
            &[
                "[公式] 第 1 行：转义多余的 $",
            ],
            json!({"table": 0, "bold": 0, "math": 1, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: Py 434 word kind with CJK neighbours
    #[test]
    fn py_cjk_intraword() {
        assert_case(
            "cjk_intraword",
            "中文**加粗未闭合\n中文**完成**继续\n",
            "中文\\**加粗未闭合*\n中文\\**完成**继续*\n",
            &[
                "[加粗] 第 1 行：补全未闭合的 *",
                "[加粗] 第 1 行：转义多余的 *",
                "[加粗] 第 2 行：补全未闭合的 *",
                "[加粗] 第 2 行：转义多余的 *",
            ],
            json!({"table": 0, "bold": 4, "math": 0, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: __ between CJK chars
    #[test]
    fn py_cjk_underscore() {
        assert_case(
            "cjk_underscore",
            "变量__name和__other\n",
            "变量__name和__other\n",
            &[],
            json!({"table": 0, "bold": 0, "math": 0, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: pipe inside inline code in a table cell
    #[test]
    fn py_mixed_pipes_code() {
        assert_case(
            "mixed_pipes_code",
            "| a | b |\n|---|---|\n| `x | y` | z |\n",
            "| a | b |\n| --- | --- |\n| `x | y` | z |\n",
            &[],
            json!({"table": 1, "bold": 0, "math": 0, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: phase order: tables run before headings
    #[test]
    fn py_heading_inside_table() {
        assert_case(
            "heading_inside_table",
            "| a | b |\n|#hash | x |\n| 1 | 2 |\n",
            "| a | b |\n| --- | --- |\n| #hash | x |\n| 1 | 2 |\n",
            &[
                "[表格] 第 1 行附近：缺少表头分隔行，已自动补全",
            ],
            json!({"table": 1, "bold": 0, "math": 0, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: emphasis phase before math phase
    #[test]
    fn py_math_after_bold() {
        assert_case(
            "math_after_bold",
            "text **b $x^2$ c\n",
            "text **b $x^2$ c**\n",
            &[
                "[加粗] 第 1 行：补全未闭合的 **",
            ],
            json!({"table": 0, "bold": 1, "math": 0, "heading": 0, "misc": 0}),
        );
    }

    /// CPython golden. Rule: end-to-end whole file (see in.md)
    #[test]
    fn py_full_document() {
        assert_case(
            "full_document",
            "#ReadMD Notes\n\n| Name | Price | Qty |\n|---|-----:|----|\n| Widget | $3 | 12 |\n| Gadget | $4 |\n\nPrefer **unclosed bold and a stray * marker.\nInline code keeps `**stars**` and `| pipes |` untouched.\n\n```python\n#Title\n| a | b |\n**not fixed inside a fence**\n$$\n```\n\n    1. ordered list item with **unclosed\n    indented code-ish line with **stars\n\nThe formula $$x = y + z\nand currency $5 plus $6.\n\nClose: \\) and a lone \\].\n\n---\n\n\\*\\*\\*nested triple and \\*\\*inner\\*\\* and \\*ital\\*\n",
            "# ReadMD Notes\n\n| Name | Price | Qty |\n| --- | -----: | ---- |\n| Widget | \\$3 | 12 |\n| Gadget | \\$4 |  |\n\nPrefer **unclosed bold and a stray * marker.**\nInline code keeps `**stars**` and `| pipes |` untouched.\n\n```python\n#Title\n| a | b |\n**not fixed inside a fence**\n$$\n```\n\n    1. ordered list item with **unclosed**\n    indented code-ish line with **stars\n\nThe formula $$x = y + z\nand currency $5 plus $6.\n\n$$\nClose: \\\\) and a lone \\\\].\n\n---\n\n\\*\\*\\*nested triple and \\*\\*inner\\*\\* and \\*ital\\*\n\n",
            &[
                "[表格] 第 3-6 行：1 行列数不齐，已对齐为 3 列",
                "[标题] 第 1 行：# 后缺少空格，已补全",
                "[加粗] 第 8 行：补全未闭合的 **",
                "[加粗] 第 18 行：补全未闭合的 **",
                "[公式] 补全未闭合的块级公式 $$",
                "[公式] 第 5 行：转义疑似货币的 $",
                "[公式] 第 6 行：转义疑似货币的 $",
                "[公式] 第 25 行：转义多余的 \\)",
                "[公式] 第 25 行：转义多余的 \\]",
            ],
            json!({"table": 1, "bold": 2, "math": 5, "heading": 1, "misc": 0}),
        );
    }

}

// ==========================================================================
// WD3 review items 1/2/3. Every expected value in this block was GENERATED by
// running the real authority, src/readmd_core/readmd_fix.py, under CPython
// 3.11.15 (scratch/rust_parity/wd3/gen.py -> parity_tests.rs). No expected
// string below was typed by hand. The three matrix guards at the bottom of
// this module are the panic belt-and-braces.
// ==========================================================================

#[cfg(test)]
mod wd3_parity_tests {
    use super::*;
    use std::panic::{catch_unwind, AssertUnwindSafe};


    /// Py 413 `if s.strip() == d: return _escape_delim(s, d, 0), [...]` -- the
    /// shortcut. 9 Unicode whitespaces x 5 delimiters: CPython answers all 45;
    /// the port sliced `&s[0 + d.len()..]` by BYTES, so 30 of the 45 land inside
    /// a multi-byte character and panic (see the report for the measured set).
    const BAL45: &[(&str, &str, &str, &[&str])] = &[
        (" *", "*", "\\**", &["转义多余的 *"]),
        (" **", "**", "\\*\\**", &["转义多余的 **"]),
        (" ***", "***", "\\*\\*\\**", &["转义多余的 ***"]),
        (" _", "_", "\\__", &["转义多余的 _"]),
        (" __", "__", "\\_\\__", &["转义多余的 __"]),
        (" *", "*", "\\**", &["转义多余的 *"]),
        (" **", "**", "\\*\\**", &["转义多余的 **"]),
        (" ***", "***", "\\*\\*\\**", &["转义多余的 ***"]),
        (" _", "_", "\\__", &["转义多余的 _"]),
        (" __", "__", "\\_\\__", &["转义多余的 __"]),
        (" *", "*", "\\**", &["转义多余的 *"]),
        (" **", "**", "\\*\\**", &["转义多余的 **"]),
        (" ***", "***", "\\*\\*\\**", &["转义多余的 ***"]),
        (" _", "_", "\\__", &["转义多余的 _"]),
        (" __", "__", "\\_\\__", &["转义多余的 __"]),
        (" *", "*", "\\**", &["转义多余的 *"]),
        (" **", "**", "\\*\\**", &["转义多余的 **"]),
        (" ***", "***", "\\*\\*\\**", &["转义多余的 ***"]),
        (" _", "_", "\\__", &["转义多余的 _"]),
        (" __", "__", "\\_\\__", &["转义多余的 __"]),
        (" *", "*", "\\**", &["转义多余的 *"]),
        (" **", "**", "\\*\\**", &["转义多余的 **"]),
        (" ***", "***", "\\*\\*\\**", &["转义多余的 ***"]),
        (" _", "_", "\\__", &["转义多余的 _"]),
        (" __", "__", "\\_\\__", &["转义多余的 __"]),
        (" *", "*", "\\**", &["转义多余的 *"]),
        (" **", "**", "\\*\\**", &["转义多余的 **"]),
        (" ***", "***", "\\*\\*\\**", &["转义多余的 ***"]),
        (" _", "_", "\\__", &["转义多余的 _"]),
        (" __", "__", "\\_\\__", &["转义多余的 __"]),
        (" *", "*", "\\**", &["转义多余的 *"]),
        (" **", "**", "\\*\\**", &["转义多余的 **"]),
        (" ***", "***", "\\*\\*\\**", &["转义多余的 ***"]),
        (" _", "_", "\\__", &["转义多余的 _"]),
        (" __", "__", "\\_\\__", &["转义多余的 __"]),
        (" *", "*", "\\**", &["转义多余的 *"]),
        (" **", "**", "\\*\\**", &["转义多余的 **"]),
        (" ***", "***", "\\*\\*\\**", &["转义多余的 ***"]),
        (" _", "_", "\\__", &["转义多余的 _"]),
        (" __", "__", "\\_\\__", &["转义多余的 __"]),
        ("　*", "*", "\\**", &["转义多余的 *"]),
        ("　**", "**", "\\*\\**", &["转义多余的 **"]),
        ("　***", "***", "\\*\\*\\**", &["转义多余的 ***"]),
        ("　_", "_", "\\__", &["转义多余的 _"]),
        ("　__", "__", "\\_\\__", &["转义多余的 __"]),
    ];

    #[test]
    fn wd3_balance_delim_ws_matrix_45() {
        for (i, (s, d, want, wlog)) in BAL45.iter().enumerate() {
            let (got, log) = balance_delim(s.to_string(), d, false);
            assert_eq!(&got, want, "balance_delim text, matrix row {} of 45 ({:?}, delim {:?})", i, s, d);
            assert_eq!(log, wlog.to_vec(), "balance_delim log, matrix row {} of 45 ({:?}, delim {:?})", i, s, d);
        }
        assert_eq!(BAL45.len(), 45);
    }

    /// The same 45 combinations driven through the public entry point, i.e.
    /// exactly what `content.rs:314` and `convert.rs:983` hand it. Python answers
    /// 200 + JSON for every one of them; the port aborted the request.
    const FIX45: &[(&str, &str, &[&str], [u64; 5])] = &[
        (" *", "\\**", &["[加粗] 第 1 行：转义多余的 *"], [0, 1, 0, 0, 0]),
        (" **", "\\*\\*\\*", &["[加粗] 第 1 行：转义多余的 **", "[加粗] 第 1 行：转义多余的 *"], [0, 2, 0, 0, 0]),
        (" ***", " ***", &[], [0, 0, 0, 0, 0]),
        (" _", " _", &[], [0, 0, 0, 0, 0]),
        (" __", "\\_\\__", &["[加粗] 第 1 行：转义多余的 __"], [0, 1, 0, 0, 0]),
        (" *", "\\**", &["[加粗] 第 1 行：转义多余的 *"], [0, 1, 0, 0, 0]),
        (" **", "\\*\\*\\*", &["[加粗] 第 1 行：转义多余的 **", "[加粗] 第 1 行：转义多余的 *"], [0, 2, 0, 0, 0]),
        (" ***", " ***", &[], [0, 0, 0, 0, 0]),
        (" _", " _", &[], [0, 0, 0, 0, 0]),
        (" __", "\\_\\__", &["[加粗] 第 1 行：转义多余的 __"], [0, 1, 0, 0, 0]),
        (" *", "\\**", &["[加粗] 第 1 行：转义多余的 *"], [0, 1, 0, 0, 0]),
        (" **", "\\*\\*\\*", &["[加粗] 第 1 行：转义多余的 **", "[加粗] 第 1 行：转义多余的 *"], [0, 2, 0, 0, 0]),
        (" ***", " ***", &[], [0, 0, 0, 0, 0]),
        (" _", " _", &[], [0, 0, 0, 0, 0]),
        (" __", "\\_\\__", &["[加粗] 第 1 行：转义多余的 __"], [0, 1, 0, 0, 0]),
        (" *", "\\**", &["[加粗] 第 1 行：转义多余的 *"], [0, 1, 0, 0, 0]),
        (" **", "\\*\\*\\*", &["[加粗] 第 1 行：转义多余的 **", "[加粗] 第 1 行：转义多余的 *"], [0, 2, 0, 0, 0]),
        (" ***", " ***", &[], [0, 0, 0, 0, 0]),
        (" _", " _", &[], [0, 0, 0, 0, 0]),
        (" __", "\\_\\__", &["[加粗] 第 1 行：转义多余的 __"], [0, 1, 0, 0, 0]),
        (" *", "\\**", &["[加粗] 第 1 行：转义多余的 *"], [0, 1, 0, 0, 0]),
        (" **", "\\*\\*\\*", &["[加粗] 第 1 行：转义多余的 **", "[加粗] 第 1 行：转义多余的 *"], [0, 2, 0, 0, 0]),
        (" ***", " ***", &[], [0, 0, 0, 0, 0]),
        (" _", " _", &[], [0, 0, 0, 0, 0]),
        (" __", "\\_\\__", &["[加粗] 第 1 行：转义多余的 __"], [0, 1, 0, 0, 0]),
        (" *", "\\**", &["[加粗] 第 1 行：转义多余的 *"], [0, 1, 0, 0, 0]),
        (" **", "\\*\\*\\*", &["[加粗] 第 1 行：转义多余的 **", "[加粗] 第 1 行：转义多余的 *"], [0, 2, 0, 0, 0]),
        (" ***", " ***", &[], [0, 0, 0, 0, 0]),
        (" _", " _", &[], [0, 0, 0, 0, 0]),
        (" __", "\\_\\__", &["[加粗] 第 1 行：转义多余的 __"], [0, 1, 0, 0, 0]),
        (" *", "\\**", &["[加粗] 第 1 行：转义多余的 *"], [0, 1, 0, 0, 0]),
        (" **", "\\*\\*\\*", &["[加粗] 第 1 行：转义多余的 **", "[加粗] 第 1 行：转义多余的 *"], [0, 2, 0, 0, 0]),
        (" ***", " ***", &[], [0, 0, 0, 0, 0]),
        (" _", " _", &[], [0, 0, 0, 0, 0]),
        (" __", "\\_\\__", &["[加粗] 第 1 行：转义多余的 __"], [0, 1, 0, 0, 0]),
        (" *", "\\**", &["[加粗] 第 1 行：转义多余的 *"], [0, 1, 0, 0, 0]),
        (" **", "\\*\\*\\*", &["[加粗] 第 1 行：转义多余的 **", "[加粗] 第 1 行：转义多余的 *"], [0, 2, 0, 0, 0]),
        (" ***", " ***", &[], [0, 0, 0, 0, 0]),
        (" _", " _", &[], [0, 0, 0, 0, 0]),
        (" __", "\\_\\__", &["[加粗] 第 1 行：转义多余的 __"], [0, 1, 0, 0, 0]),
        ("　*", "\\**", &["[加粗] 第 1 行：转义多余的 *"], [0, 1, 0, 0, 0]),
        ("　**", "\\*\\*\\*", &["[加粗] 第 1 行：转义多余的 **", "[加粗] 第 1 行：转义多余的 *"], [0, 2, 0, 0, 0]),
        ("　***", "　***", &[], [0, 0, 0, 0, 0]),
        ("　_", "　_", &[], [0, 0, 0, 0, 0]),
        ("　__", "\\_\\__", &["[加粗] 第 1 行：转义多余的 __"], [0, 1, 0, 0, 0]),
    ];

    fn check(no: u64, input: &str, want_text: &str, want_fixes: &[&str], want_stats: [u64; 5]) {
        let res = fix_markdown(input);
        assert_eq!(&res.text, want_text, "text mismatch, case {} input {:?}", no, input);
        let got: Vec<&str> = res.fixes.iter().map(|s| s.as_str()).collect();
        assert_eq!(got, want_fixes.to_vec(), "fixes mismatch, case {} input {:?}", no, input);
        assert_eq!(
            res.stats,
            json!({"table": want_stats[0], "bold": want_stats[1], "math": want_stats[2],
                  "heading": want_stats[3], "misc": want_stats[4]}),
            "stats mismatch, case {} input {:?}", no, input);
    }

    fn run_table(t: &[(&str, &str, &[&str], [u64; 5])]) {
        for (i, (input, text, fixes, stats)) in t.iter().enumerate() {
            check(i as u64, input, text, fixes, *stats);
        }
    }

    #[test]
    fn wd3_fix45_matrix() {
        assert_eq!(FIX45.len(), 45);
        run_table(FIX45);
    }

    /// Rule-by-rule cases for review items 1, 2 and 3, plus 70 non-ASCII fuzz
    /// documents that CPython actually changed. Columns: input, text, fixes,
    /// [table, bold, math, heading, misc].
    const CASES: &[(&str, &str, &[&str], [u64; 5])] = &[
        (" *\nbody\n", "\\**\nbody\n", &["[加粗] 第 1 行：转义多余的 *"], [0, 1, 0, 0, 0]),
        (" **\nbody\n", "\\*\\*\\*\nbody\n", &["[加粗] 第 1 行：转义多余的 **", "[加粗] 第 1 行：转义多余的 *"], [0, 2, 0, 0, 0]),
        (" _\nbody\n", " _\nbody\n", &[], [0, 0, 0, 0, 0]),
        (" __\nbody\n", "\\_\\__\nbody\n", &["[加粗] 第 1 行：转义多余的 __"], [0, 1, 0, 0, 0]),
        (" ***\nbody\n", " ***\nbody\n", &[], [0, 0, 0, 0, 0]),
        (" *\nbody\n", "\\**\nbody\n", &["[加粗] 第 1 行：转义多余的 *"], [0, 1, 0, 0, 0]),
        (" **\nbody\n", "\\*\\*\\*\nbody\n", &["[加粗] 第 1 行：转义多余的 **", "[加粗] 第 1 行：转义多余的 *"], [0, 2, 0, 0, 0]),
        (" _\nbody\n", " _\nbody\n", &[], [0, 0, 0, 0, 0]),
        (" __\nbody\n", "\\_\\__\nbody\n", &["[加粗] 第 1 行：转义多余的 __"], [0, 1, 0, 0, 0]),
        (" ***\nbody\n", " ***\nbody\n", &[], [0, 0, 0, 0, 0]),
        ("　*\nbody\n", "\\**\nbody\n", &["[加粗] 第 1 行：转义多余的 *"], [0, 1, 0, 0, 0]),
        ("　**\nbody\n", "\\*\\*\\*\nbody\n", &["[加粗] 第 1 行：转义多余的 **", "[加粗] 第 1 行：转义多余的 *"], [0, 2, 0, 0, 0]),
        ("　_\nbody\n", "　_\nbody\n", &[], [0, 0, 0, 0, 0]),
        ("　__\nbody\n", "\\_\\__\nbody\n", &["[加粗] 第 1 行：转义多余的 __"], [0, 1, 0, 0, 0]),
        ("　***\nbody\n", "　***\nbody\n", &[], [0, 0, 0, 0, 0]),
        (" *\nbody\n", "\\**\nbody\n", &["[加粗] 第 1 行：转义多余的 *"], [0, 1, 0, 0, 0]),
        (" **\nbody\n", "\\*\\*\\*\nbody\n", &["[加粗] 第 1 行：转义多余的 **", "[加粗] 第 1 行：转义多余的 *"], [0, 2, 0, 0, 0]),
        (" _\nbody\n", " _\nbody\n", &[], [0, 0, 0, 0, 0]),
        (" __\nbody\n", "\\_\\__\nbody\n", &["[加粗] 第 1 行：转义多余的 __"], [0, 1, 0, 0, 0]),
        (" ***\nbody\n", " ***\nbody\n", &[], [0, 0, 0, 0, 0]),
        ("**中文** 和 **游离\n", "**中文** 和 **游离**\n", &["[加粗] 第 1 行：补全未闭合的 **"], [0, 1, 0, 0, 0]),
        ("中文**a**b**c\n", "中文\\**a**b**c*\n", &["[加粗] 第 1 行：补全未闭合的 *", "[加粗] 第 1 行：转义多余的 *"], [0, 2, 0, 0, 0]),
        ("一**二**三**四**五\n", "一\\**二**三**四**五*\n", &["[加粗] 第 1 行：补全未闭合的 *", "[加粗] 第 1 行：转义多余的 *"], [0, 2, 0, 0, 0]),
        ("**一**和**二**和三\n", "**一**和\\**二**和三*\n", &["[加粗] 第 1 行：补全未闭合的 *", "[加粗] 第 1 行：转义多余的 *"], [0, 2, 0, 0, 0]),
        ("***一** 和 *二*\n", "***一** 和 *二***\\*\n", &["[加粗] 第 1 行：补全未闭合的 ***", "[加粗] 第 1 行：转义多余的 *"], [0, 2, 0, 0, 0]),
        ("😀**x\n", "😀**x**\n", &["[加粗] 第 1 行：补全未闭合的 **"], [0, 1, 0, 0, 0]),
        ("😀*😀**😀\n", "😀\\*😀\\*\\*😀\n", &["[加粗] 第 1 行：转义多余的 **", "[加粗] 第 1 行：转义多余的 *"], [0, 2, 0, 0, 0]),
        ("→**x** ← **y\n", "→**x** ← **y**\n", &["[加粗] 第 1 行：补全未闭合的 **"], [0, 1, 0, 0, 0]),
        ("** ** ** \n", "\\*\\* \\*\\* \\*\\* \n", &["[加粗] 第 1 行：转义多余的 **", "[加粗] 第 1 行：转义多余的 **", "[加粗] 第 1 行：转义多余的 **"], [0, 3, 0, 0, 0]),
        ("**　**　**　\n", "\\*\\*　\\*\\*　\\*\\*　\n", &["[加粗] 第 1 行：转义多余的 **", "[加粗] 第 1 行：转义多余的 **", "[加粗] 第 1 行：转义多余的 **"], [0, 3, 0, 0, 0]),
        ("中文**未闭合的加粗\n中文**完成**继续\n", "中文\\**未闭合的加粗*\n中文\\**完成**继续*\n", &["[加粗] 第 1 行：补全未闭合的 *", "[加粗] 第 1 行：转义多余的 *", "[加粗] 第 2 行：补全未闭合的 *", "[加粗] 第 2 行：转义多余的 *"], [0, 4, 0, 0, 0]),
        ("一__二__三__四\n", "一__二__三__四\n", &[], [0, 0, 0, 0, 0]),
        ("中`**x**`一 **未闭合\n", "中`**x**`一 **未闭合**\n", &["[加粗] 第 1 行：补全未闭合的 **"], [0, 1, 0, 0, 0]),
        ("`` ` ** ` `` 和 **未闭合\n", "`` ` ** ` `` 和 **未闭合**\n", &["[加粗] 第 1 行：补全未闭合的 **"], [0, 1, 0, 0, 0]),
        ("a `**x**` b 　**\n", "a `**x**` b 　\\*\\*\n", &["[加粗] 第 1 行：转义多余的 **"], [0, 1, 0, 0, 0]),
        ("中文 $$x = 1\n", "中文 $$x = 1\n\n$$\n", &["[公式] 补全未闭合的块级公式 $$"], [0, 0, 1, 0, 0]),
        (" $$\nx\n", " $$\nx\n$$\n\n", &["[公式] 补全未闭合的块级公式 $$"], [0, 0, 1, 0, 0]),
        ("中文$ \\(x\\)\n", "中文\\$ \\(x\\)\n", &["[公式] 第 1 行：转义多余的 $"], [0, 0, 1, 0, 0]),
        (" $x^2\n", " $x^2$\n", &["[公式] 第 1 行：补全未闭合的行内公式 $"], [0, 0, 1, 0, 0]),
        ("价格￥5 $x^2 和 \\) 个\n", "价格￥5 $x^2 和 \\\\) 个$\n", &["[公式] 第 1 行：转义多余的 \\)", "[公式] 第 1 行：补全未闭合的行内公式 $"], [0, 0, 2, 0, 0]),
        ("cost \\) 中文 \\]  \n", "cost \\\\) 中文 \\\\]  \n", &["[公式] 第 1 行：转义多余的 \\)", "[公式] 第 1 行：转义多余的 \\]"], [0, 0, 2, 0, 0]),
        ("\u{1c}**\nbody\n", "\\*\\*\\*\nbody\n", &["[加粗] 第 1 行：转义多余的 **", "[加粗] 第 1 行：转义多余的 *"], [0, 2, 0, 0, 0]),
        ("**\u{1c}\n", "\\*\\*\u{1c}\n", &["[加粗] 第 1 行：转义多余的 **"], [0, 1, 0, 0, 0]),
        ("\u{1d}*\u{1e}\n", "\u{1d}*\u{1e}\n", &[], [0, 0, 0, 0, 0]),
        ("a **b\u{1d}\n", "a **b**\n", &["[加粗] 第 1 行：补全未闭合的 **"], [0, 1, 0, 0, 0]),
        ("a __b\u{1e}\n", "a __b__\n", &["[加粗] 第 1 行：补全未闭合的 __"], [0, 1, 0, 0, 0]),
        ("\u{1c}#Title\n", "\u{1c}# Title\n", &["[标题] 第 1 行：# 后缺少空格，已补全"], [0, 0, 0, 1, 0]),
        ("#\u{1c}x\n", "#\u{1c}x\n", &[], [0, 0, 0, 0, 0]),
        ("   \u{1c}#T\n", "   \u{1c}# T\n", &["[标题] 第 1 行：# 后缺少空格，已补全"], [0, 0, 0, 1, 0]),
        ("\u{1c}***\u{1d}\n", "\u{1c}***\u{1d}\n", &[], [0, 0, 0, 0, 0]),
        ("\u{1e}___\u{1f}\n", "\u{1e}___\u{1f}\n", &[], [0, 0, 0, 0, 0]),
        ("|\u{1c}--\u{1d}|--|\n| a | b |\n| 1 | 2 |\n", "| --- | --- |\n| a | b |\n| 1 | 2 |\n", &[], [1, 0, 0, 0, 0]),
        ("| a | b |\n|---|\u{1e}--:|\n| 1 | 2 |\n", "| a | b |\n| --- | ---: |\n| 1 | 2 |\n", &[], [1, 0, 0, 0, 0]),
        ("\u{1c}```\ncode\n\u{1d}```\nbody **x\n", "\u{1c}```\ncode\n\u{1d}```\nbody **x**\n", &["[加粗] 第 4 行：补全未闭合的 **"], [0, 1, 0, 0, 0]),
        ("```\ncode\n```\u{1c}\nbody\n", "```\ncode\n```\u{1c}\nbody\n", &[], [0, 0, 0, 0, 0]),
        ("    -\u{1c}x **y\n", "    -\u{1c}x **y**\n", &["[加粗] 第 1 行：补全未闭合的 **"], [0, 1, 0, 0, 0]),
        ("    \u{1c}> q **y\n", "    \u{1c}> q **y**\n", &["[加粗] 第 1 行：补全未闭合的 **"], [0, 1, 0, 0, 0]),
        ("    1\u{1d}. x **y\n", "    1\u{1d}. x **y\n", &[], [0, 0, 0, 0, 0]),
        ("  \u{1c}| a | b |\n  \u{1c}| 1 | 2 |\n", "  \u{1c}| a | b |\n  \u{1c}| --- | --- |\n  \u{1c}| 1 | 2 |\n", &["[表格] 第 1 行附近：缺少表头分隔行，已自动补全"], [1, 0, 0, 0, 0]),
        ("$$\nx = 1\n\u{1c}\nafter\n", "$$\nx = 1\n$$\n\u{1c}\nafter\n\n", &["[公式] 补全未闭合的块级公式 $$"], [0, 0, 1, 0, 0]),
        ("a $$x\u{1d}\n", "a $$x\u{1d}\n\n$$\n", &["[公式] 补全未闭合的块级公式 $$"], [0, 0, 1, 0, 0]),
        ("$$\u{1e}\n", "$$\n$$\n", &["[公式] 补全未闭合的块级公式 $$"], [0, 0, 1, 0, 0]),
        ("中文\\(x\u{1d}\n", "中文\\(x\\)\n", &["[公式] 第 1 行：补全未闭合的 \\( ... \\)"], [0, 0, 1, 0, 0]),
        ("a\u{1c}**b**\n", "a\u{1c}**b**\n", &[], [0, 0, 0, 0, 0]),
        ("\u{1c}\u{1d}*\u{1e}\u{1f}\n", "\u{1c}\u{1d}*\u{1e}\u{1f}\n", &[], [0, 0, 0, 0, 0]),
        ("\u{1c}中文**x\n", "\u{1c}中文\\**x*\n", &["[加粗] 第 1 行：补全未闭合的 *", "[加粗] 第 1 行：转义多余的 *"], [0, 2, 0, 0, 0]),
        ("x=©**y\u{1c}\n", "x=©**y**\n", &["[加粗] 第 1 行：补全未闭合的 **"], [0, 1, 0, 0, 0]),
        ("* item with *unclosed\u{1c}\n", "* item with *unclosed\u{1c}\n", &[], [0, 0, 0, 0, 0]),
        ("\u{1b}**\n", "\u{1b}\\*\\*\n", &["[加粗] 第 1 行：转义多余的 **"], [0, 1, 0, 0, 0]),
        ("\u{1a}**\n", "\u{1a}\\*\\*\n", &["[加粗] 第 1 行：转义多余的 **"], [0, 1, 0, 0, 0]),
        ("`\u{1a}`\u{1c}**\n", "`\u{1a}`\u{1c}\\*\\*\n", &["[加粗] 第 1 行：转义多余的 **"], [0, 1, 0, 0, 0]),
        ("\u{1b}**x\u{1b}\n", "\u{1b}\\**x\u{1b}*\n", &["[加粗] 第 1 行：补全未闭合的 *", "[加粗] 第 1 行：转义多余的 *"], [0, 2, 0, 0, 0]),
        ("→$$　---**___中文", "→$$　---\\*\\*\\_\\__中文\n$$\n", &["[加粗] 第 1 行：转义多余的 **", "[加粗] 第 1 行：转义多余的 __", "[公式] 补全未闭合的块级公式 $$"], [0, 2, 1, 0, 0]),
        ("\\]_©", "\\\\]_©", &["[公式] 第 1 行：转义多余的 \\]"], [0, 0, 1, 0, 0]),
        ("\\(#`x`→\u{1d}___    ", "\\(#`x`→\u{1d}_____\\)", &["[加粗] 第 1 行：补全未闭合的 __", "[公式] 第 1 行：补全未闭合的 \\( ... \\)"], [0, 1, 1, 0, 0]),
        ("\u{1c}　$$_\\(　__", "\u{1c}　$$_\\(　\\_\\_\\)\n$$\n", &["[加粗] 第 1 行：转义多余的 __", "[公式] 补全未闭合的块级公式 $$", "[公式] 第 1 行：补全未闭合的 \\( ... \\)"], [0, 1, 2, 0, 0]),
        ("\\)`x`\u{1c}$\u{1d}", "\\\\)`x`\u{1c}\\$\u{1d}", &["[公式] 第 1 行：转义多余的 \\)", "[公式] 第 1 行：转义多余的 $"], [0, 0, 2, 0, 0]),
        ("\\(中文 \\(\n$", "\\(中文 \\(\\)\n\\$", &["[公式] 第 1 行：补全未闭合的 \\( ... \\)", "[公式] 第 2 行：转义疑似货币的 $"], [0, 0, 2, 0, 0]),
        ("😀***©", "😀\\*\\*\\*©", &["[加粗] 第 1 行：转义多余的 ***"], [0, 1, 0, 0, 0]),
        ("\\)---　```。一_", "\\\\)---　```。一_", &["[公式] 第 1 行：转义多余的 \\)"], [0, 0, 1, 0, 0]),
        ("___\u{1d}---😀 ", "\\_\\__\u{1d}---😀 ", &["[加粗] 第 1 行：转义多余的 __"], [0, 1, 0, 0, 0]),
        ("\u{1c}`x`\\)\n$ ", "\u{1c}`x`\\\\)\n\\$ ", &["[公式] 第 1 行：转义多余的 \\)", "[公式] 第 2 行：转义疑似货币的 $"], [0, 0, 2, 0, 0]),
        ("\n\n\n$\u{1d}", "\n\n\n\\$\u{1d}", &["[公式] 第 4 行：转义疑似货币的 $"], [0, 0, 1, 0, 0]),
        ("\\)\n", "\\\\)\n", &["[公式] 第 1 行：转义多余的 \\)"], [0, 0, 1, 0, 0]),
        (" 中文__", " 中文\\_\\_", &["[加粗] 第 1 行：转义多余的 __"], [0, 1, 0, 0, 0]),
        ("#\\]　 _*　", "# \\\\]　 _\\*　", &["[标题] 第 1 行：# 后缺少空格，已补全", "[加粗] 第 1 行：转义多余的 *", "[公式] 第 1 行：转义多余的 \\]"], [0, 1, 1, 1, 0]),
        (" 😀___", " 😀\\_\\__", &["[加粗] 第 1 行：转义多余的 __"], [0, 1, 0, 0, 0]),
        ("😀中文\u{1c}*", "😀中文\u{1c}\\*", &["[加粗] 第 1 行：转义多余的 *"], [0, 1, 0, 0, 0]),
        ("**\\(", "\\*\\*\\(\\)", &["[加粗] 第 1 行：转义多余的 **", "[公式] 第 1 行：补全未闭合的 \\( ... \\)"], [0, 1, 1, 0, 0]),
        ("*|\\(→", "\\*|\\(→\\)", &["[加粗] 第 1 行：转义多余的 *", "[公式] 第 1 行：补全未闭合的 \\( ... \\)"], [0, 1, 1, 0, 0]),
        ("\u{1c}一__\u{1c} $$$", "\u{1c}一\\_\\_\u{1c} $$$$\n$$\n", &["[加粗] 第 1 行：转义多余的 __", "[公式] 补全未闭合的块级公式 $$", "[公式] 第 1 行：补全未闭合的行内公式 $"], [0, 1, 2, 0, 0]),
        ("$\n\\)→", "\\$\n\\\\)→", &["[公式] 第 1 行：转义疑似货币的 $", "[公式] 第 2 行：转义多余的 \\)"], [0, 0, 2, 0, 0]),
        ("。_|$© ", "。_|$©$", &["[公式] 第 1 行：补全未闭合的行内公式 $"], [0, 0, 1, 0, 0]),
        ("。*___\u{1c}", "。\\*\\_\\__\u{1c}", &["[加粗] 第 1 行：转义多余的 __", "[加粗] 第 1 行：转义多余的 *"], [0, 2, 0, 0, 0]),
        ("___#\\]中文", "\\_\\__#\\\\]中文", &["[加粗] 第 1 行：转义多余的 __", "[公式] 第 1 行：转义多余的 \\]"], [0, 1, 1, 0, 0]),
        ("→　```$", "→　```\\$", &["[公式] 第 1 行：转义疑似货币的 $"], [0, 0, 1, 0, 0]),
        ("---  __\\)    \\]", "---  __\\\\)    \\\\]__", &["[加粗] 第 1 行：补全未闭合的 __", "[公式] 第 1 行：转义多余的 \\)", "[公式] 第 1 行：转义多余的 \\]"], [0, 1, 2, 0, 0]),
        ("`x`\n**", "`x`\n\\*\\*", &["[加粗] 第 2 行：转义多余的 **"], [0, 1, 0, 0, 0]),
        ("\\]___。", "\\\\]\\_\\__。", &["[加粗] 第 1 行：转义多余的 __", "[公式] 第 1 行：转义多余的 \\]"], [0, 1, 1, 0, 0]),
        ("。 \\)__    ", "。 \\\\)\\_\\_    ", &["[加粗] 第 1 行：转义多余的 __", "[公式] 第 1 行：转义多余的 \\)"], [0, 1, 1, 0, 0]),
        ("\\(\u{1d} _", "\\(\u{1d} _\\)", &["[公式] 第 1 行：补全未闭合的 \\( ... \\)"], [0, 0, 1, 0, 0]),
        ("　    😀*", "　    😀\\*", &["[加粗] 第 1 行：转义多余的 *"], [0, 1, 0, 0, 0]),
        ("___---#```$$", "\\_\\__---#```$$\n$$\n", &["[加粗] 第 1 行：转义多余的 __", "[公式] 补全未闭合的块级公式 $$"], [0, 1, 1, 0, 0]),
        ("|||`x`_", "|  |  | `x`_ |\n| --- | --- | --- |", &["[表格] 第 1 行附近：缺少表头分隔行，已自动补全"], [1, 0, 0, 0, 0]),
        ("`x`→\\)\n*_　", "`x`→\\\\)\n\\*_　", &["[加粗] 第 2 行：转义多余的 *", "[公式] 第 1 行：转义多余的 \\)"], [0, 1, 1, 0, 0]),
        ("😀```\\)\\]$一", "😀```\\\\)\\\\]$一$", &["[公式] 第 1 行：转义多余的 \\)", "[公式] 第 1 行：转义多余的 \\]", "[公式] 第 1 行：补全未闭合的行内公式 $"], [0, 0, 3, 0, 0]),
        ("中文`x`$$|", "中文`x`$$|\n$$\n", &["[公式] 补全未闭合的块级公式 $$"], [0, 0, 1, 0, 0]),
        ("\\]$$©©中文*    ", "\\\\]$$©©中文\\*    \n$$\n", &["[加粗] 第 1 行：转义多余的 *", "[公式] 补全未闭合的块级公式 $$", "[公式] 第 1 行：转义多余的 \\]"], [0, 1, 2, 0, 0]),
        ("　😀 *    。---", "　😀 \\*    。---", &["[加粗] 第 1 行：转义多余的 *"], [0, 1, 0, 0, 0]),
        ("**#|\\)", "\\*\\*#|\\\\)", &["[加粗] 第 1 行：转义多余的 **", "[公式] 第 1 行：转义多余的 \\)"], [0, 1, 1, 0, 0]),
        ("    $$。 #　```", "    $$。 #　```\n$$\n", &["[公式] 补全未闭合的块级公式 $$"], [0, 0, 1, 0, 0]),
        ("\\( ©|", "\\( ©|\\)", &["[公式] 第 1 行：补全未闭合的 \\( ... \\)"], [0, 0, 1, 0, 0]),
        ("　\\]©\u{1d}___", "　\\\\]©\u{1d}_____", &["[加粗] 第 1 行：补全未闭合的 __", "[公式] 第 1 行：转义多余的 \\]"], [0, 1, 1, 0, 0]),
        ("\\]___", "\\\\]\\_\\__", &["[加粗] 第 1 行：转义多余的 __", "[公式] 第 1 行：转义多余的 \\]"], [0, 1, 1, 0, 0]),
        ("。***$", "。\\*\\*\\*\\$", &["[加粗] 第 1 行：转义多余的 ***", "[公式] 第 1 行：转义多余的 $"], [0, 1, 1, 0, 0]),
        ("\u{1d}\u{1c}__", "\\_\\___", &["[加粗] 第 1 行：转义多余的 __"], [0, 1, 0, 0, 0]),
        ("\\]。```© ", "\\\\]。```© ", &["[公式] 第 1 行：转义多余的 \\]"], [0, 0, 1, 0, 0]),
        ("---___*😀 ", "---\\_\\__\\*😀 ", &["[加粗] 第 1 行：转义多余的 __", "[加粗] 第 1 行：转义多余的 *"], [0, 2, 0, 0, 0]),
        ("*\\(___。", "\\*\\(\\_\\__。\\)", &["[加粗] 第 1 行：转义多余的 __", "[加粗] 第 1 行：转义多余的 *", "[公式] 第 1 行：补全未闭合的 \\( ... \\)"], [0, 2, 1, 0, 0]),
        ("___\\]　", "\\_\\__\\\\]　", &["[加粗] 第 1 行：转义多余的 __", "[公式] 第 1 行：转义多余的 \\]"], [0, 1, 1, 0, 0]),
        (" ©---\\)\\]", " ©---\\\\)\\\\]", &["[公式] 第 1 行：转义多余的 \\)", "[公式] 第 1 行：转义多余的 \\]"], [0, 0, 2, 0, 0]),
        ("→　*    \\)", "→　\\*    \\\\)", &["[加粗] 第 1 行：转义多余的 *", "[公式] 第 1 行：转义多余的 \\)"], [0, 1, 1, 0, 0]),
        ("$$\u{1c}", "$$\n$$\n", &["[公式] 补全未闭合的块级公式 $$"], [0, 0, 1, 0, 0]),
        ("$$\\( \u{1d}\n\\]", "$$\\(\\)\n\\\\]\n$$\n", &["[公式] 补全未闭合的块级公式 $$", "[公式] 第 1 行：补全未闭合的 \\( ... \\)", "[公式] 第 2 行：转义多余的 \\]"], [0, 0, 3, 0, 0]),
        ("©。#$一|", "©。#\\$一|", &["[公式] 第 1 行：转义疑似货币的 $"], [0, 0, 1, 0, 0]),
        ("______---", "\\_\\_\\_\\_\\_\\_---", &["[加粗] 第 1 行：转义多余的 __", "[加粗] 第 1 行：转义多余的 __", "[加粗] 第 1 行：转义多余的 __"], [0, 3, 0, 0, 0]),
        ("__\u{1d}　__`x`", "\\_\\_\u{1d}　__`x`__", &["[加粗] 第 1 行：补全未闭合的 __", "[加粗] 第 1 行：转义多余的 __"], [0, 2, 0, 0, 0]),
        (" \u{1c}**", "\\*\\*\\*\\*", &["[加粗] 第 1 行：转义多余的 **", "[加粗] 第 1 行：转义多余的 *", "[加粗] 第 1 行：转义多余的 *"], [0, 3, 0, 0, 0]),
        ("\u{1c}$\n→　　$", "\u{1c}\\$\n→　　\\$", &["[公式] 第 1 行：转义疑似货币的 $", "[公式] 第 2 行：转义疑似货币的 $"], [0, 0, 2, 0, 0]),
        ("\n___\\(---中文\\(    ", "\n\\_\\__\\(---中文\\(\\)", &["[加粗] 第 2 行：转义多余的 __", "[公式] 第 2 行：补全未闭合的 \\( ... \\)"], [0, 1, 1, 0, 0]),
        ("©中文__*\u{1c}。", "©中文\\_\\_\\*\u{1c}。", &["[加粗] 第 1 行：转义多余的 __", "[加粗] 第 1 行：转义多余的 *"], [0, 2, 0, 0, 0]),
        ("\\] 中文$", "\\\\] 中文\\$", &["[公式] 第 1 行：转义多余的 \\]", "[公式] 第 1 行：转义多余的 $"], [0, 0, 2, 0, 0]),
        ("_**|**#$$", "_\\*\\*|\\*\\*#$$\n$$\n", &["[加粗] 第 1 行：转义多余的 **", "[加粗] 第 1 行：转义多余的 **", "[公式] 补全未闭合的块级公式 $$"], [0, 2, 1, 0, 0]),
        ("*___", "\\*\\_\\__", &["[加粗] 第 1 行：转义多余的 __", "[加粗] 第 1 行：转义多余的 *"], [0, 2, 0, 0, 0]),
        ("_ →__\\]\u{1d}", "_ →\\_\\_\\\\]\u{1d}", &["[加粗] 第 1 行：转义多余的 __", "[公式] 第 1 行：转义多余的 \\]"], [0, 1, 1, 0, 0]),
        ("**_©", "\\*\\*_©", &["[加粗] 第 1 行：转义多余的 **"], [0, 1, 0, 0, 0]),
        ("_\u{1c}__ ", "_\u{1c}\\_\\_ ", &["[加粗] 第 1 行：转义多余的 __"], [0, 1, 0, 0, 0]),
        ("\u{1c}　　\u{1d}    \\)", "\u{1c}　　\u{1d}    \\\\)", &["[公式] 第 1 行：转义多余的 \\)"], [0, 0, 1, 0, 0]),
        ("#\\)\u{1c}一*\\(", "# \\)\u{1c}一\\*\\(", &["[标题] 第 1 行：# 后缺少空格，已补全", "[加粗] 第 1 行：转义多余的 *"], [0, 1, 0, 1, 0]),
        (" $", " \\$", &["[公式] 第 1 行：转义疑似货币的 $"], [0, 0, 1, 0, 0]),
        ("\n中文 \\(*。**", "\n中文 \\(\\*。\\*\\*\\)", &["[加粗] 第 2 行：转义多余的 **", "[加粗] 第 2 行：转义多余的 *", "[公式] 第 2 行：补全未闭合的 \\( ... \\)"], [0, 2, 1, 0, 0]),
        ("\u{1d}|　", "\u{1d}|  |\n\u{1d}| --- |", &["[表格] 第 1 行附近：缺少表头分隔行，已自动补全", "[表格] 第 1-1 行：1 行列数不齐，已对齐为 1 列"], [1, 0, 0, 0, 0]),
    ];

    #[test]
    fn wd3_rule_cases_and_fuzz_table() {
        assert!(CASES.len() > 120, "table degenerated: {}", CASES.len());
        run_table(CASES);
    }

    /// Belt-and-braces for item 2: enumerate the offending combination space --
    /// 14 whitespace code points (including U+001C..U+001F) x 8 delimiter runs x
    /// 6 multi-byte prefixes x 4 suffixes = 2688 bare-delimiter lines -- and
    /// assert that neither `balance_delim` nor the public entry point panics.
    /// CPython raises on none of them.
    #[test]
    fn wd3_combination_matrix_never_panics() {
        let ws: Vec<char> = vec![' ', '\u{1c}', '\u{1d}', '\u{1e}', '\u{1f}',
                                '\u{a0}', '\u{1680}', '\u{2002}', '\u{2003}',
                                '\u{2009}', '\u{202f}', '\u{205f}', '\u{3000}', '\u{2028}'];
        let delims: Vec<&str> = vec!["*", "**", "***", "****", "_", "__", "___", "$$"];
        let pre: Vec<&str> = vec!["", " ", "\u{a0}", "\u{3000}", "\u{4e2d}\u{6587}", "\u{1f600}"];
        let post: Vec<&str> = vec!["", " ", "\u{3000}", "\u{4e2d}"];
        let mut n = 0usize;
        for w in &ws {
            for d in &delims {
                for p in &pre {
                    for q in &post {
                        let s = format!("{p}{w}{d}{q}");
                        n += 1;
                        let r = catch_unwind(AssertUnwindSafe(|| {
                            let _ = balance_delim(s.clone(), d, false);
                            let _ = balance_delim(s.clone(), d, true);
                            let _ = fix_markdown(&s);
                            let _ = fix_markdown(&format!("{s}\nbody\n"));
                        }));
                        assert!(r.is_ok(), "PANIC on input {:?} (delim {:?}) at matrix #{}", s, d, n);
                    }
                }
            }
        }
        assert_eq!(n, 14 * 8 * 6 * 4);
    }

    /// The same guard aimed straight at the slicing helpers: with 2-, 3- and
    /// 4-byte code points present, every position -- including ones past the end,
    /// where CPython slicing simply clamps -- must be answerable without a panic.
    #[test]
    fn wd3_slicing_helpers_never_panic() {
        let atoms: Vec<&str> = vec!["\u{a0}", "\u{1680}", "\u{2002}", "\u{3000}",
                                    "\u{4e2d}", "\u{1f600}", "*", " ", "\u{1c}",
                                    "a", "**"];
        let mut n = 0usize;
        for k in 1..=4usize {
            for combo in atoms.windows(k) {
                let s = combo.concat();
                for d in ["*", "**", "***", "__", "$$"] {
                    for p in 0..=(s.chars().count() + 3) {
                        n += 1;
                        let r = catch_unwind(AssertUnwindSafe(|| {
                            let _ = escape_delim(&s, d, p);
                            let _ = escape_at(&s, p);
                            let _ = classify_delim(&format!("{s}{d}"), p, d, false);
                            let _ = mask_pairs(format!("{s}{d}"), d, "2");
                            let _ = balance_delim(format!("{s}{d}"), d, false);
                            let _ = unescaped_positions(&s, d);
                        }));
                        assert!(r.is_ok(), "PANIC s={:?} d={:?} p={}", s, d, p);
                    }
                }
            }
        }
        // Recompute the expected combination count from the same tables: the guard
        // is against a degenerate loop (empty windows, dropped position range),
        // not against a guessed magnitude.
        let expected: usize = (1..=4usize)
            .flat_map(|k| atoms.windows(k))
            .map(|combo| 5 * (combo.concat().chars().count() + 4))
            .sum();
        assert_eq!(n, expected, "enumeration degenerated");
        assert!(n >= 1200, "enumeration too small: {}", n);
    }

    /// Item 3 guard: the ported predicate must be EXACTLY
    /// `Unicode White_Space U+001C..U+001F`, and the widened regex classes must
    /// agree with it code point by code point. U+001A/U+001B stay NON-space
    /// because `mask_code_spans` injects U+001A as its placeholder sentinel.
    #[test]
    fn wd3_py_isspace_set_is_exact() {
        let widened = Regex::new(r"[\s\x1c-\x1f]").unwrap();
        for cp in 0u32..0x2100 {
            let ch = match char::from_u32(cp) {
                Some(c) => c,
                None => continue,
            };
            let want = ch.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&ch);
            assert_eq!(py_isspace(ch), want, "py_isspace U+{:04X}", cp);
            assert_eq!(
                widened.is_match(ch.encode_utf8(&mut [0u8; 4])),
                want,
                "the [\\s\\x1c-\\x1f] class disagrees with py_isspace at U+{:04X}", cp);
        }
        assert!(!py_isspace('\u{1a}'));
        assert!(!py_isspace('\u{1b}'));
        assert!(py_isspace('\u{1c}'));
        assert!(py_isspace('\u{1f}'));
        assert!(py_isspace('\u{a0}'));
        assert!(py_isspace('\u{3000}'));
    }
}
