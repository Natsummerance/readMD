//! `src/readmd_core/toc_engine.py` — ReadMD 正文 `[TOC]` 自动内嵌层级目录树生成引擎.
//!
//! Supported syntax (verbatim from the Python module docstring):
//! - `[TOC]`
//! - `[toc]`
//! - `<!-- @import "[TOC]" -->`
//! - with parameters: `[TOC] {depth_from=2 depth_to=4 ordered_list=false ignore=["参考资料"]}`
//!
//! Every branch below is a line-for-line transcription of the Python
//! authority; where the two languages differ, CPython wins.  The divergences
//! that needed explicit repair are recorded at each site.
//!
//! Two contract notes that a caller must not "fix":
//! * the `ignore=[...]` attribute documented above is parsed by *nothing* —
//!   [`process_toc_markers`] only reads `depth_from`, `depth_to` and the two
//!   ordered-list spellings, exactly like `toc_engine.py:116-134`.  The MCP
//!   server (`readmd --mcp`, `mcp.rs`) reaches
//!   [`generate_toc_markdown`] directly when it wants an ignore list.
//! * [`slugify_heading`] never disambiguates duplicate anchors.  Two identical
//!   headings produce two identical `#slug` targets, so this module is *not*
//!   interchangeable with `content::headings()`/`content::slug()`, which
//!   appends `-1` suffixes and maps every separator to `-`.

use lazy_static::lazy_static;
use regex::Regex;

use crate::link_indexer::{py_splitlines, py_strip};

// ---------------------------------------------------------------------------
// compiled patterns
// ---------------------------------------------------------------------------
//
// Python's `\s` for a `str` pattern is `Py_UNICODE_ISSPACE`, i.e. Unicode
// `White_Space` **plus** the four C1 separators `U+001C..U+001F` (verified by
// exhaustive enumeration: 29 code points).  The `regex` crate's `\s` is
// `White_Space` only, so every `\s` inherited from the Python patterns below is
// written as `[\s\x{1c}-\x{1f}]`; a bare `\s` would lose marker and attribute
// matches whose gap holds one of those separators.

lazy_static! {
    /// `toc_engine.TOC_MARKER_PATTERN` (`toc_engine.py:14-17`).
    ///
    /// `re.MULTILINE` becomes `(?m)`.  Both engines treat `\r` as *content*,
    /// which is why a `[TOC]` marker in a CRLF document is deliberately **not**
    /// replaced: the trailing `[ \t]*$` cannot consume the `\r` that sits
    /// before the `\n` the `$` anchors to.
    static ref TOC_MARKER_PATTERN: Regex = Regex::new(
        r#"(?m)^[ \t]*(?:\[(?:TOC|toc)\]|<!--[\s\x{1c}-\x{1f}]*@import[\s\x{1c}-\x{1f}]*["']\[TOC\]["'][\s\x{1c}-\x{1f}]*-->)(?:[\s\x{1c}-\x{1f}]*\{([^}]*)\})?[ \t]*$"#
    ).unwrap();

    /// `toc_engine.HEADING_PATTERN` (`toc_engine.py:19`).
    static ref HEADING_PATTERN: Regex =
        Regex::new(r"^(#{1,6})[ \t]+(.+?)[ \t]*#*[ \t]*$").unwrap();

    /// `re.search(r'depth_from\s*=\s*(\d+)', attr_str)` (`toc_engine.py:123`).
    static ref DEPTH_FROM_RE: Regex =
        Regex::new(r"depth_from[\s\x{1c}-\x{1f}]*=[\s\x{1c}-\x{1f}]*(\d+)").unwrap();

    /// `re.search(r'depth_to\s*=\s*(\d+)', attr_str)` (`toc_engine.py:126`).
    static ref DEPTH_TO_RE: Regex =
        Regex::new(r"depth_to[\s\x{1c}-\x{1f}]*=[\s\x{1c}-\x{1f}]*(\d+)").unwrap();

    /// `re.sub(r'`([^`]+)`', r'\1', heading_text)` (`toc_engine.py:25`).
    static ref INLINE_CODE_RE: Regex = Regex::new(r"`([^`]+)`").unwrap();

    /// `re.sub(r'\[([^\]]+)\]\([^)]+\)', r'\1', text)` (`toc_engine.py:26`).
    static ref LINK_RE: Regex = Regex::new(r"\[([^\]]+)\]\([^)]+\)").unwrap();

    /// `re.sub(r'!\[([^\]]*)\]\([^)]+\)', r'\1', text)` (`toc_engine.py:27`).
    /// Runs *after* [`LINK_RE`], so the ordinary link pass has already eaten
    /// the `[alt](url)` half of most images; the ordering is load-bearing.
    static ref IMAGE_RE: Regex = Regex::new(r"!\[([^\]]*)\]\([^)]+\)").unwrap();

    /// `re.sub(r'<[^>]+>', '', text)` (`toc_engine.py:28`).
    static ref TAG_RE: Regex = Regex::new(r"<[^>]+>").unwrap();

    /// `re.sub(r'[ \t]+', '-', slug)` (`toc_engine.py:33`) — deliberately only
    /// space and tab, so a U+00A0 inside a heading becomes a removed character
    /// rather than a separator.
    static ref SPACE_RUN_RE: Regex = Regex::new(r"[ \t]+").unwrap();

    /// `re.sub(r'[^\w\u4e00-\u9fa5\-]', '', slug)` (`toc_engine.py:35`).
    ///
    /// Python's `\w` on `str` is `Py_UNICODE_ISALNUM` plus `_`, which was
    /// checked code point by code point to be exactly Unicode general category
    /// `L*` ∪ `N*` ∪ `_`; the explicit `\u4e00-\u9fa5` branch adds nothing
    /// because every one of those code points is already `Lo`.  The `regex`
    /// crate's `\w` is **not** that set — it is `Alphabetic ∪ Mn ∪ Nd ∪ Pc`,
    /// so it keeps combining marks (`e\u{301}` would survive) and `Pc`
    /// variants.  `\p{L}\p{N}_` is the faithful translation.
    static ref NON_WORD_RE: Regex = Regex::new(r"[^\p{L}\p{N}_-]").unwrap();

    /// `re.sub(r'[*_~`]', '', title)` (`toc_engine.py:96`).
    static ref EMPHASIS_RE: Regex = Regex::new(r"[*_~`]").unwrap();
}

/// One entry of `extract_headings()`'s `List[Tuple[int, str, str]]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    /// `len(match.group(1))` — the number of leading `#`.
    pub level: i64,
    /// `match.group(2).strip()` (`toc_engine.py:63`).
    pub title: String,
    /// `slugify_heading(title)` (`toc_engine.py:64`).
    pub slug: String,
}

// ---------------------------------------------------------------------------
// python primitives used by the transcription
// ---------------------------------------------------------------------------

/// `stripped[:3]` (`toc_engine.py:49`, `source_map.py:32`) — a **code point**
/// slice, not a byte slice, so a 3-byte fence opener cannot land mid-char.
pub(crate) fn py_first3(s: &str) -> String {
    s.chars().take(3).collect()
}

/// `int(re.search(r'\d+', ...).group(1))`.  CPython integers are unbounded, so
/// a digit run too wide for `i64` saturates instead of failing the match the
/// way `str::parse` would.
fn py_int_unbounded(digits: &str) -> i64 {
    digits.parse::<i64>().unwrap_or(i64::MAX)
}

// ---------------------------------------------------------------------------
// public API
// ---------------------------------------------------------------------------

/// `toc_engine.slugify_heading` (`toc_engine.py:22-36`).
///
/// Generates the GitHub/browser-compatible anchor for a heading.
pub fn slugify_heading(heading_text: &str) -> String {
    // 移除行内代码、图片、链接标记
    let text = INLINE_CODE_RE.replace_all(heading_text, "$1");
    let text = LINK_RE.replace_all(&text, "$1");
    let text = IMAGE_RE.replace_all(&text, "$1");
    let text = TAG_RE.replace_all(&text, "");

    // 转小写并替换空格与标点
    let slug = py_strip(&text).to_lowercase();
    // 替换空格为短横线
    let slug = SPACE_RUN_RE.replace_all(&slug, "-");
    // 移除特殊标点符号（保留字母、数字、中文、下划线、短横线）
    let slug = NON_WORD_RE.replace_all(&slug, "");
    // `return slug or "section"`
    if slug.is_empty() {
        "section".to_string()
    } else {
        slug.into_owned()
    }
}

/// `toc_engine.extract_headings` (`toc_engine.py:39-67`).
///
/// Extracts every effective `(level, title, slug)` triple, skipping the
/// contents of fenced code blocks.
///
/// Two details that make this *not* the same function as `content::headings`:
/// the pattern is matched against the **unstripped** line, so an indented
/// `  # x` is not a heading; and the fence opener captured on entry is
/// re-checked with `curr_fence == fence_char`, so a `~~~` line inside a ```
/// block stays inside it.
pub fn extract_headings(markdown_content: &str) -> Vec<Heading> {
    let mut headings: Vec<Heading> = Vec::new();
    let mut in_code_block = false;
    let mut fence_char = String::new();

    for line in py_splitlines(markdown_content) {
        let stripped = py_strip(line);
        // 检查代码块定界符
        if stripped.starts_with("```") || stripped.starts_with("~~~") {
            let curr_fence = py_first3(stripped);
            if !in_code_block {
                in_code_block = true;
                fence_char = curr_fence;
            } else if curr_fence == fence_char {
                in_code_block = false;
            }
            continue;
        }

        if in_code_block {
            continue;
        }

        if let Some(caps) = HEADING_PATTERN.captures(line) {
            let level = caps.get(1).map(|m| m.as_str()).unwrap_or("").chars().count() as i64;
            let title = py_strip(caps.get(2).map(|m| m.as_str()).unwrap_or("")).to_string();
            let slug = slugify_heading(&title);
            headings.push(Heading { level, title, slug });
        }
    }

    headings
}

/// `toc_engine.generate_toc_markdown` (`toc_engine.py:70-109`).
///
/// Renders the hierarchical Markdown tree for a heading list.
///
/// `ignore_titles` is matched case-insensitively against the *raw* title (not
/// the slug), and the rendered link text has every `*`, `_`, `~` and backtick
/// removed while the anchor keeps them, because the anchor was computed from
/// the unscrubbed title by [`slugify_heading`].
///
/// Panics on a `level` outside `1..=6` exactly where CPython raises
/// `IndexError` from `counters[lvl]` (`toc_engine.py:99`); [`extract_headings`]
/// cannot produce such a level.
pub fn generate_toc_markdown(
    headings: &[Heading],
    depth_from: i64,
    depth_to: i64,
    ordered_list: bool,
    ignore_titles: Option<&[&str]>,
) -> String {
    if headings.is_empty() {
        return String::new();
    }

    let ignore_set: Vec<String> = ignore_titles
        .unwrap_or(&[])
        .iter()
        .map(|t| t.to_lowercase())
        .collect();
    let filtered: Vec<&Heading> = headings
        .iter()
        .filter(|h| {
            depth_from <= h.level
                && h.level <= depth_to
                && !ignore_set.contains(&h.title.to_lowercase())
        })
        .collect();

    if filtered.is_empty() {
        return String::new();
    }

    let min_level = filtered.iter().map(|h| h.level).min().unwrap();
    let mut toc_lines: Vec<String> = Vec::new();

    // 用于多级序号统计 — `counters = [0] * 7`
    let mut counters = vec![0i64; 7];

    for h in filtered {
        let lvl = h.level;
        let indent = "  ".repeat((lvl - min_level) as usize);
        let clean_title = EMPHASIS_RE.replace_all(&h.title, "").to_string();

        let prefix = if ordered_list {
            counters[lvl as usize] += 1;
            // 重置子级别
            for i in (lvl as usize + 1)..7 {
                counters[i] = 0;
            }
            format!("{}. ", counters[lvl as usize])
        } else {
            "- ".to_string()
        };

        toc_lines.push(format!("{}{}[{}](#{})", indent, prefix, clean_title, h.slug));
    }

    toc_lines.join("\n")
}

/// Default `depth_from` of `toc_engine.generate_toc_markdown`.
pub const TOC_DEFAULT_DEPTH_FROM: i64 = 1;
/// Default `depth_to` of `toc_engine.generate_toc_markdown`.
pub const TOC_DEFAULT_DEPTH_TO: i64 = 6;

/// `toc_engine.process_toc_markers` (`toc_engine.py:112-136`).
///
/// Scans the document and replaces every `[TOC]` marker in place with the
/// generated tree.  The heading list is extracted from the **whole** document
/// once, before any substitution, and each marker re-renders that same list
/// with its own attributes.
pub fn process_toc_markers(content: &str) -> String {
    let headings = extract_headings(content);

    TOC_MARKER_PATTERN
        .replace_all(content, |caps: &regex::Captures| {
            let attr_str = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let mut depth_from = TOC_DEFAULT_DEPTH_FROM;
            let mut depth_to = TOC_DEFAULT_DEPTH_TO;
            let mut ordered = false;

            // 简单提取属性
            if let Some(c) = DEPTH_FROM_RE.captures(attr_str) {
                depth_from = py_int_unbounded(&c[1]);
            }
            if let Some(c) = DEPTH_TO_RE.captures(attr_str) {
                depth_to = py_int_unbounded(&c[1]);
            }
            let lowered = attr_str.to_lowercase();
            if lowered.contains("ordered_list=true") || lowered.contains("ordered=true") {
                ordered = true;
            }

            let toc_md =
                generate_toc_markdown(&headings, depth_from, depth_to, ordered, None);
            if toc_md.is_empty() {
                String::new()
            } else {
                format!("\n{}\n", toc_md)
            }
        })
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

const SLUG_CASES: &[(&str, &str, &str)] = &[
    ("01", "1. \u{5FEB}\u{901F}\u{5165}\u{95E8} Quick Start", "1-\u{5FEB}\u{901F}\u{5165}\u{95E8}-quick-start"),
    ("02", "LaTeX \u{516C}\u{5F0F} $E=mc^2$", "latex-\u{516C}\u{5F0F}-emc2"),
    ("03", "`code_snippet()`", "code_snippet"),
    ("04", "Hello, World!", "hello-world"),
    ("05", "  Leading and trailing  ", "leading-and-trailing"),
    ("06", "## Not a heading", "-not-a-heading"),
    ("07", "", "section"),
    ("08", "!!!", "section"),
    ("09", "\u{DC}n\u{EF}c\u{F6}d\u{E9} \u{C0}ccent\u{EB}d", "\u{FC}n\u{EF}c\u{F6}d\u{E9}-\u{E0}ccent\u{EB}d"),
    ("10", "\u{4E2D}\u{6587} \u{6A19}\u{984C} \u{2014} em dash", "\u{4E2D}\u{6587}-\u{6A19}\u{984C}--em-dash"),
    ("11", "[link text](http://example.com)", "link-text"),
    ("12", "![img](a.png)", "img"),
    ("13", "<span>Tagged</span> heading", "tagged-heading"),
    ("14", "a `b` c `d` e", "a-b-c-d-e"),
    ("15", "![x](y) and [z](w)", "x-and-z"),
    ("16", "TAB\there", "tab-here"),
    ("17", "\u{DC}MLAUT", "\u{FC}mlaut"),
    ("18", "\u{130}stanbul", "istanbul"),
    ("19", "STRASSE \u{DF}", "strasse-\u{DF}"),
    ("20", "emoji \u{1F680} heading", "emoji--heading"),
    ("21", "100% done & dusted", "100-done--dusted"),
    ("22", "Nested [a[b]c](url)", "nested-abcurl"),
    ("23", "Under_score and dash-es", "under_score-and-dash-es"),
    ("24", "\u{3000}full-width space\u{3000}", "full-width-space"),
    ("25", "\u{A0}nbsp\u{A0}", "nbsp"),
    ("26", "abc\u{001C}def", "abcdef"),
    ("27", "Multiple   spaces", "multiple-spaces"),
    ("28", "Trailing hash ###", "trailing-hash-"),
    ("29", "C# and F# languages", "c-and-f-languages"),
    ("30", "Section 1.2.3", "section-123"),
    ("31", "<b>bold", "bold"),
    ("32", "[a](b)[c](d)", "ac"),
    ("33", "```  ``", "-"),
    ("34", "\u{41F}\u{440}\u{438}\u{432}\u{435}\u{442} \u{41C}\u{438}\u{440}", "\u{43F}\u{440}\u{438}\u{432}\u{435}\u{442}-\u{43C}\u{438}\u{440}"),
    ("35", "\u{C9}\u{E8}\u{CA}\u{EB}", "\u{E9}\u{E8}\u{EA}\u{EB}"),
    ("36", "a\u{2009}b", "ab"),
    ("37", "\u{2028}sep", "sep"),
    ("38", "[TOC] marker", "toc-marker"),
    ("39", "Keeps-dashes-and_underscores", "keeps-dashes-and_underscores"),
    ("40", "MiXeD CaSe OnLy", "mixed-case-only"),
    ("41", "e\u{301}clair", "eclair"),
    ("42", "zero\u{200D}width", "zerowidth"),
    ("43", "\u{BD} fraction", "\u{BD}-fraction"),
    ("44", "\u{B2}super", "\u{B2}super"),
    ("45", "\u{2167}roman", "\u{2177}roman"),
    ("46", "\u{B7}middle dot", "middle-dot"),
    ("47", "\u{AA}feminine", "\u{AA}feminine"),
    ("48", "\u{3000}\u{3000}", "section"),
    ("49", "\u{AD}soft hyphen", "soft-hyphen"),
    ("50", "\u{660}\u{661} arabic-indic", "\u{660}\u{661}-arabic-indic"),
];

const EXTRACT_CASES: &[(&str, &str, &str)] = &[
    ("01", "\n# \u{771F}\u{5B9E}\u{6807}\u{9898} 1\n\u{6B63}\u{6587}\u{5185}\u{5BB9}\u{3002}\n\n```python\n# \u{8FD9}\u{662F}\u{4EE3}\u{7801}\u{5757}\u{5185}\u{7684}\u{6CE8}\u{91CA}\u{FF0C}\u{4E0D}\u{5E94}\u{63D0}\u{53D6}\u{4E3A}\u{6807}\u{9898}\ndef foo():\n    pass\n```\n\n## \u{771F}\u{5B9E}\u{6807}\u{9898} 2\n~~~markdown\n### \u{4EE3}\u{7801}\u{5757}\u{5185}\u{7684}\u{5047}\u{6807}\u{9898}\n~~~\n### \u{771F}\u{5B9E}\u{6807}\u{9898} 3\n", "[[1, \"\u{771F}\u{5B9E}\u{6807}\u{9898} 1\", \"\u{771F}\u{5B9E}\u{6807}\u{9898}-1\"], [2, \"\u{771F}\u{5B9E}\u{6807}\u{9898} 2\", \"\u{771F}\u{5B9E}\u{6807}\u{9898}-2\"], [3, \"\u{771F}\u{5B9E}\u{6807}\u{9898} 3\", \"\u{771F}\u{5B9E}\u{6807}\u{9898}-3\"]]"),
    ("02", "\n# \u{6587}\u{6863}\u{603B}\u{89C8}\n\n[TOC]\n\n## \u{7B2C}\u{4E00}\u{7AE0} \u{6982}\u{8FF0}\n\u{5185}\u{5BB9} 1\u{3002}\n\n## \u{7B2C}\u{4E8C}\u{7AE0} \u{6DF1}\u{5165}\n\u{5185}\u{5BB9} 2\u{3002}\n", "[[1, \"\u{6587}\u{6863}\u{603B}\u{89C8}\", \"\u{6587}\u{6863}\u{603B}\u{89C8}\"], [2, \"\u{7B2C}\u{4E00}\u{7AE0} \u{6982}\u{8FF0}\", \"\u{7B2C}\u{4E00}\u{7AE0}-\u{6982}\u{8FF0}\"], [2, \"\u{7B2C}\u{4E8C}\u{7AE0} \u{6DF1}\u{5165}\", \"\u{7B2C}\u{4E8C}\u{7AE0}-\u{6DF1}\u{5165}\"]]"),
    ("03", "# a\n## b\n### c\n#### d\n##### e\n###### f\n", "[[1, \"a\", \"a\"], [2, \"b\", \"b\"], [3, \"c\", \"c\"], [4, \"d\", \"d\"], [5, \"e\", \"e\"], [6, \"f\", \"f\"]]"),
    ("04", "####### seven\n", "[]"),
    ("05", "#NoSpace\n#\n# \n#  two spaces\n", "[[1, \"two spaces\", \"two-spaces\"]]"),
    ("06", "# Heading ###\n", "[[1, \"Heading\", \"heading\"]]"),
    ("07", "#\tTab heading\n", "[[1, \"Tab heading\", \"tab-heading\"]]"),
    ("08", "   # indented\n\t# tab indented\n", "[]"),
    ("09", "```\n# in fence\n~~~\n# still fence\n```\n# after\n", "[[1, \"after\", \"after\"]]"),
    ("10", "# unclosed\n```\n# never seen\n", "[[1, \"unclosed\", \"unclosed\"]]"),
    ("11", "# A\r\n## B\r\n", "[[1, \"A\", \"a\"], [2, \"B\", \"b\"]]"),
    ("12", "# A\r## B\r", "[[1, \"A\", \"a\"], [2, \"B\", \"b\"]]"),
    ("13", "# A\n## B", "[[1, \"A\", \"a\"], [2, \"B\", \"b\"]]"),
    ("14", "# trailing   \n## trailing hash #  \t \n", "[[1, \"trailing\", \"trailing\"], [2, \"trailing hash\", \"trailing-hash\"]]"),
    ("15", "### \u{4E2D}\u{6587}\n", "[[3, \"\u{4E2D}\u{6587}\", \"\u{4E2D}\u{6587}\"]]"),
    ("16", "# A\u{2028}# B\u{2029}# C\u{000B}# D\u{000C}# E\n", "[[1, \"A\", \"a\"], [1, \"B\", \"b\"], [1, \"C\", \"c\"], [1, \"D\", \"d\"], [1, \"E\", \"e\"]]"),
    ("17", "# ##\n", "[[1, \"#\", \"section\"]]"),
    ("18", "~~~\n# f\n~~~x\n# g\n", "[[1, \"g\", \"g\"]]"),
    ("19", "text\n", "[]"),
    ("20", "", "[]"),
    ("21", "\n\n\n", "[]"),
    ("22", "####\tt\tt tabbed \t\n", "[[4, \"t\\tt tabbed\", \"t-t-tabbed\"]]"),
    ("23", "# a `b` c\n# <span>x</span> y\n# [l](u) m\n", "[[1, \"a `b` c\", \"a-b-c\"], [1, \"<span>x</span> y\", \"x-y\"], [1, \"[l](u) m\", \"l-m\"]]"),
    ("24", "  \n# lead ws\n", "[[1, \"lead ws\", \"lead-ws\"]]"),
    ("25", "#dup\n#dup\n##dup\n", "[]"),
    ("26", "````` \nfence5\n`````\n# after5\n", "[[1, \"after5\", \"after5\"]]"),
    ("27", "# \u{A0}nbsp heading\n", "[[1, \"nbsp heading\", \"nbsp-heading\"]]"),
    ("28", "# one#\n# two ## #\n", "[[1, \"one\", \"one\"], [1, \"two ##\", \"two-\"]]"),
    ("29", "# L1\n# L1\n## L1\n", "[[1, \"L1\", \"l1\"], [1, \"L1\", \"l1\"], [2, \"L1\", \"l1\"]]"),
    ("30", "#\u{A0}nbsp after hash\n", "[]"),
    ("31", "```\n```\n# A\n", "[[1, \"A\", \"a\"]]"),
    ("32", "# \u{4E2D}\u{6587}#\n", "[[1, \"\u{4E2D}\u{6587}\", \"\u{4E2D}\u{6587}\"]]"),
    ("33", "#\t\tmixed\n", "[[1, \"mixed\", \"mixed\"]]"),
];

const TOC_GEN_CASES: &[(&str, &str, i64, i64, bool, Option<&[&str]>, &str)] = &[
    ("01", "\n# \u{771F}\u{5B9E}\u{6807}\u{9898} 1\n\u{6B63}\u{6587}\u{5185}\u{5BB9}\u{3002}\n\n```python\n# \u{8FD9}\u{662F}\u{4EE3}\u{7801}\u{5757}\u{5185}\u{7684}\u{6CE8}\u{91CA}\u{FF0C}\u{4E0D}\u{5E94}\u{63D0}\u{53D6}\u{4E3A}\u{6807}\u{9898}\ndef foo():\n    pass\n```\n\n## \u{771F}\u{5B9E}\u{6807}\u{9898} 2\n~~~markdown\n### \u{4EE3}\u{7801}\u{5757}\u{5185}\u{7684}\u{5047}\u{6807}\u{9898}\n~~~\n### \u{771F}\u{5B9E}\u{6807}\u{9898} 3\n", 1, 6, false, None, "- [\u{771F}\u{5B9E}\u{6807}\u{9898} 1](#\u{771F}\u{5B9E}\u{6807}\u{9898}-1)\n  - [\u{771F}\u{5B9E}\u{6807}\u{9898} 2](#\u{771F}\u{5B9E}\u{6807}\u{9898}-2)\n    - [\u{771F}\u{5B9E}\u{6807}\u{9898} 3](#\u{771F}\u{5B9E}\u{6807}\u{9898}-3)"),
    ("02", "\n# \u{6587}\u{6863}\u{603B}\u{89C8}\n\n[TOC]\n\n## \u{7B2C}\u{4E00}\u{7AE0} \u{6982}\u{8FF0}\n\u{5185}\u{5BB9} 1\u{3002}\n\n## \u{7B2C}\u{4E8C}\u{7AE0} \u{6DF1}\u{5165}\n\u{5185}\u{5BB9} 2\u{3002}\n", 1, 6, false, None, "- [\u{6587}\u{6863}\u{603B}\u{89C8}](#\u{6587}\u{6863}\u{603B}\u{89C8})\n  - [\u{7B2C}\u{4E00}\u{7AE0} \u{6982}\u{8FF0}](#\u{7B2C}\u{4E00}\u{7AE0}-\u{6982}\u{8FF0})\n  - [\u{7B2C}\u{4E8C}\u{7AE0} \u{6DF1}\u{5165}](#\u{7B2C}\u{4E8C}\u{7AE0}-\u{6DF1}\u{5165})"),
    ("03", "# a\n## b\n### c\n#### d\n", 2, 4, false, None, "- [b](#b)\n  - [c](#c)\n    - [d](#d)"),
    ("04", "# a\n## b\n### c\n", 1, 6, true, None, "1. [a](#a)\n  1. [b](#b)\n    1. [c](#c)"),
    ("05", "# a\n## b\n### c\n## d\n# e\n", 1, 6, true, None, "1. [a](#a)\n  1. [b](#b)\n    1. [c](#c)\n  2. [d](#d)\n2. [e](#e)"),
    ("06", "# \u{7B80}\u{4ECB}\n## \u{53C2}\u{8003}\u{8D44}\u{6599}\n## \u{9644}\u{5F55}\n", 1, 6, false, Some(&["\u{53C2}\u{8003}\u{8D44}\u{6599}"]), "- [\u{7B80}\u{4ECB}](#\u{7B80}\u{4ECB})\n  - [\u{9644}\u{5F55}](#\u{9644}\u{5F55})"),
    ("07", "# a\n## b\n", 1, 6, false, Some(&["A"]), "- [b](#b)"),
    ("08", "# a\n## b\n", 5, 6, false, None, ""),
    ("09", "", 1, 6, false, None, ""),
    ("10", "# only\n", 1, 1, true, None, "1. [only](#only)"),
    ("11", "## starts at 2\n#### deeper\n### mid\n", 1, 6, false, None, "- [starts at 2](#starts-at-2)\n    - [deeper](#deeper)\n  - [mid](#mid)"),
    ("12", "# a `b` c\n# em*pha*sis\n# til~de\n", 1, 6, false, None, "- [a b c](#a-b-c)\n- [emphasis](#emphasis)\n- [tilde](#tilde)"),
    ("13", "# a\n## b\n### c\n", 2, 2, false, None, "- [b](#b)"),
    ("14", "# a\n## b\n", 3, 1, false, None, ""),
    ("15", "# \u{4E2D}\u{6587}\u{6807}\u{9898}\n", 1, 6, false, None, "- [\u{4E2D}\u{6587}\u{6807}\u{9898}](#\u{4E2D}\u{6587}\u{6807}\u{9898})"),
    ("16", "# a\n## b\n### c\n", 1, 6, true, Some(&["B"]), "1. [a](#a)\n    1. [c](#c)"),
    ("17", "# a\n## b\n", 1, 6, false, Some(&["A", "b"]), ""),
    ("18", "# a\n# b\n## c\n# d\n", 1, 6, true, Some(&["b"]), "1. [a](#a)\n  1. [c](#c)\n2. [d](#d)"),
    ("19", "### only deep\n", 1, 2, false, None, ""),
    ("20", "# x\n## y\n", 1, 6, false, Some(&["NOSUCH"]), "- [x](#x)\n  - [y](#y)"),
];

const MARKER_CASES: &[(&str, &str, &str)] = &[
    ("01", "\n# \u{6587}\u{6863}\u{603B}\u{89C8}\n\n[TOC]\n\n## \u{7B2C}\u{4E00}\u{7AE0} \u{6982}\u{8FF0}\n\u{5185}\u{5BB9} 1\u{3002}\n\n## \u{7B2C}\u{4E8C}\u{7AE0} \u{6DF1}\u{5165}\n\u{5185}\u{5BB9} 2\u{3002}\n", "\n# \u{6587}\u{6863}\u{603B}\u{89C8}\n\n\n- [\u{6587}\u{6863}\u{603B}\u{89C8}](#\u{6587}\u{6863}\u{603B}\u{89C8})\n  - [\u{7B2C}\u{4E00}\u{7AE0} \u{6982}\u{8FF0}](#\u{7B2C}\u{4E00}\u{7AE0}-\u{6982}\u{8FF0})\n  - [\u{7B2C}\u{4E8C}\u{7AE0} \u{6DF1}\u{5165}](#\u{7B2C}\u{4E8C}\u{7AE0}-\u{6DF1}\u{5165})\n\n\n## \u{7B2C}\u{4E00}\u{7AE0} \u{6982}\u{8FF0}\n\u{5185}\u{5BB9} 1\u{3002}\n\n## \u{7B2C}\u{4E8C}\u{7AE0} \u{6DF1}\u{5165}\n\u{5185}\u{5BB9} 2\u{3002}\n"),
    ("02", "[TOC]\n# A\n## B\n", "\n- [A](#a)\n  - [B](#b)\n\n# A\n## B\n"),
    ("03", "[toc]\n# A\n## B\n", "\n- [A](#a)\n  - [B](#b)\n\n# A\n## B\n"),
    ("04", "<!-- @import \"[TOC]\" -->\n# A\n", "\n- [A](#a)\n\n# A\n"),
    ("05", "<!-- @import '[TOC]' -->\n# A\n", "\n- [A](#a)\n\n# A\n"),
    ("06", "[TOC] {depth_from=2 depth_to=4 ordered_list=false ignore=[\"\u{53C2}\u{8003}\u{8D44}\u{6599}\"]}\n# A\n## B\n### C\n#### D\n##### E\n", "\n- [B](#b)\n  - [C](#c)\n    - [D](#d)\n\n# A\n## B\n### C\n#### D\n##### E\n"),
    ("07", "[TOC]{depth_from=2}\n# A\n## B\n", "\n- [B](#b)\n\n# A\n## B\n"),
    ("08", "[TOC]\n\ntext\n\n[TOC]\n\n# A\n", "\n- [A](#a)\n\n\ntext\n\n\n- [A](#a)\n\n\n# A\n"),
    ("09", "[TOC] {ordered=true}\n# A\n## B\n", "\n1. [A](#a)\n  1. [B](#b)\n\n# A\n## B\n"),
    ("10", "[TOC] {DEPTH_FROM=2}\n# A\n## B\n", "\n- [A](#a)\n  - [B](#b)\n\n# A\n## B\n"),
    ("11", "[TOC] {Ordered_List=True}\n# A\n## B\n", "\n1. [A](#a)\n  1. [B](#b)\n\n# A\n## B\n"),
    ("12", "   [TOC]\t\n# A\n", "\n- [A](#a)\n\n# A\n"),
    ("13", "[TOC]\r\n# A\r\n", "[TOC]\r\n# A\r\n"),
    ("14", "[TOC]\n\n", "\n\n"),
    ("15", "text [TOC] text\n# A\n", "text [TOC] text\n# A\n"),
    ("16", "[TOC] {depth_to=2}\n# A\n## B\n### C\n", "\n- [A](#a)\n  - [B](#b)\n\n# A\n## B\n### C\n"),
    ("17", "[TOC] {depth_from=1 depth_to=1 ordered=true}\n# A\n# B\n", "\n1. [A](#a)\n2. [B](#b)\n\n# A\n# B\n"),
    ("18", "[TOC] {\n# A\n", "[TOC] {\n# A\n"),
    ("19", "<!--@import\"[TOC]\"-->\n# A\n", "\n- [A](#a)\n\n# A\n"),
    ("20", "[TOC] {depth_from=0 depth_to=9}\n# A\n###### F\n", "\n- [A](#a)\n          - [F](#f)\n\n# A\n###### F\n"),
    ("21", "# pre\n[TOC]\n# post\n", "# pre\n\n- [pre](#pre)\n- [post](#post)\n\n# post\n"),
    ("22", "<!-- @import \"[toc]\" -->\n# A\n", "<!-- @import \"[toc]\" -->\n# A\n"),
    ("23", "[TOC] {  depth_to =  1  }\n# A\n## B\n", "\n- [A](#a)\n\n# A\n## B\n"),
    ("24", "[TOC]\n[TOC]\n# A\n", "\n- [A](#a)\n\n\n- [A](#a)\n\n# A\n"),
    ("25", "\u{4E2D}\u{6587} [TOC] \u{4E2D}\u{6587}\n# A\n", "\u{4E2D}\u{6587} [TOC] \u{4E2D}\u{6587}\n# A\n"),
    ("26", "#### h4\n[TOC] {depth_from=4 depth_to=4}\n", "#### h4\n\n- [h4](#h4)\n\n"),
    ("27", "# A\n## B\n[TOC]", "# A\n## B\n\n- [A](#a)\n  - [B](#b)\n"),
    ("28", "[TOC]\t{depth_to=1}\n# A\n## B\n", "\n- [A](#a)\n\n# A\n## B\n"),
    ("29", "<!-- @import \"[TOC]\" --> {depth_to=1}\n# A\n## B\n", "\n- [A](#a)\n\n# A\n## B\n"),
    ("30", "[TOC] {depth_from=abc depth_to=2}\n# A\n## B\n### C\n", "\n- [A](#a)\n  - [B](#b)\n\n# A\n## B\n### C\n"),
    ("31", "[TOC] {}\n# A\n", "\n- [A](#a)\n\n# A\n"),
    ("32", "\u{3000}[TOC]\n# A\n", "\u{3000}[TOC]\n# A\n"),
    ("33", "[TOC] {ignore=[\"A\"] depth_to=1}\n# A\n## B\n", "\n- [A](#a)\n\n# A\n## B\n"),
    ("34", "[TOC]\n[TOC] {ordered=true}\n# A\n", "\n- [A](#a)\n\n\n1. [A](#a)\n\n# A\n"),
    ("35", "# A\n[TOC]\n[TOC]\n## B\n", "# A\n\n- [A](#a)\n  - [B](#b)\n\n\n- [A](#a)\n  - [B](#b)\n\n## B\n"),
    ("36", "<!--@import\u{001C}\"[TOC]\"-->\n# A\n", "\n- [A](#a)\n\n# A\n"),
    ("37", "[TOC]\u{001C}{depth_to=1}\n# A\n## B\n", "\n- [A](#a)\n\n# A\n## B\n"),
];

    /// `json.dumps(..., ensure_ascii=False)` for one string of a tuple.
    fn py_json_str(s: &str) -> String {
        let mut out = String::from("\"");
        for ch in s.chars() {
            match ch {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
                c => out.push(c),
            }
        }
        out.push('"');
        out
    }

    /// `json.dumps(extract_headings(doc), ensure_ascii=False)`: CPython
    /// serialises each tuple as an array and joins items with `", "`.
    fn dump_headings(hs: &[Heading]) -> String {
        let items: Vec<String> = hs
            .iter()
            .map(|h| format!("[{}, {}, {}]", h.level, py_json_str(&h.title), py_json_str(&h.slug)))
            .collect();
        format!("[{}]", items.join(", "))
    }

    fn check_slug(name: &str) {
        for (n, i, e) in SLUG_CASES {
            if *n == name {
                assert_eq!(&slugify_heading(i), e, "slugify_heading case {}", name);
                return;
            }
        }
        panic!("no slug case {}", name);
    }

    fn check_extract(name: &str) {
        for (n, i, e) in EXTRACT_CASES {
            if *n == name {
                assert_eq!(
                    &dump_headings(&extract_headings(i)),
                    e,
                    "extract_headings case {}",
                    name
                );
                return;
            }
        }
        panic!("no extract case {}", name);
    }

    fn check_gen(name: &str) {
        for row in TOC_GEN_CASES {
            if row.0 == name {
                let hs = extract_headings(row.1);
                assert_eq!(
                    generate_toc_markdown(&hs, row.2, row.3, row.4, row.5),
                    row.6,
                    "generate_toc_markdown case {}",
                    name
                );
                return;
            }
        }
        panic!("no toc_gen case {}", name);
    }

    fn check_marker(name: &str) {
        for (n, i, e) in MARKER_CASES {
            if *n == name {
                assert_eq!(
                    &process_toc_markers(i),
                    e,
                    "process_toc_markers case {}",
                    name
                );
                return;
            }
        }
        panic!("no marker case {}", name);
    }


    // ---------------------------------------------------------------- slugify

    #[test]
    fn slugify_heading_table_matches_cpython() {
        for (name, input, want) in SLUG_CASES {
            assert_eq!(&slugify_heading(input), want, "slugify_heading case {}", name);
        }
    }

    #[test]
    fn slugify_heading_accented_and_cjk_headings() {
        for name in ["01", "02", "09", "10", "17", "34", "35", "45", "50"] {
            check_slug(name);
        }
    }

    #[test]
    fn slugify_heading_inline_code_and_emphasis() {
        for name in ["03", "14", "16", "23", "33", "39"] {
            check_slug(name);
        }
    }

    #[test]
    fn slugify_heading_link_image_and_html_markup_is_stripped() {
        for name in ["11", "12", "13", "15", "22", "31", "32"] {
            check_slug(name);
        }
    }

    #[test]
    fn slugify_heading_unicode_whitespace_and_combining_marks() {
        // U+3000/U+00A0 are stripped as whitespace, U+001C is whitespace to
        // Python but not to Rust's `trim`, and U+0301 is a combining mark that
        // the `regex` crate's own `\w` would wrongly keep.
        for name in ["05", "24", "25", "26", "36", "37", "41", "42", "43", "44", "46", "47", "49"] {
            check_slug(name);
        }
    }

    #[test]
    fn slugify_heading_empty_or_punctuation_only_becomes_section() {
        for name in ["07", "08", "48"] {
            check_slug(name);
        }
    }

    #[test]
    fn slugify_heading_lowercases_like_python_str_lower() {
        for name in ["18", "19", "40"] {
            check_slug(name);
        }
    }

    #[test]
    fn slugify_heading_collapses_nothing_and_keeps_one_dash_per_space() {
        for name in ["21", "27", "28", "29", "30"] {
            check_slug(name);
        }
    }

    // --------------------------------------------------------------- extract

    #[test]
    fn extract_headings_table_matches_cpython() {
        for (name, input, want) in EXTRACT_CASES {
            assert_eq!(
                &dump_headings(&extract_headings(input)),
                want,
                "extract_headings case {}",
                name
            );
        }
    }

    #[test]
    fn extract_headings_all_six_levels_and_seven_hashes_is_not_a_heading() {
        check_extract("03");
        check_extract("04");
    }

    #[test]
    fn extract_headings_duplicate_titles_share_one_slug() {
        // No `-1` disambiguation, which is what separates this authority from
        // `content::headings()`.
        check_extract("25");
        check_extract("29");
    }

    #[test]
    fn extract_headings_with_inline_code_markup_and_cjk() {
        for name in ["15", "23", "27", "32"] {
            check_extract(name);
        }
    }

    #[test]
    fn extract_headings_requires_a_space_or_tab_after_the_hashes() {
        check_extract("05");
        check_extract("30");
        check_extract("33");
    }

    #[test]
    fn extract_headings_strips_trailing_hashes_from_the_title() {
        for name in ["06", "14", "17", "22", "28"] {
            check_extract(name);
        }
    }

    #[test]
    fn extract_headings_ignores_fenced_code_block_contents() {
        for name in ["01", "09", "10", "18", "26", "31"] {
            check_extract(name);
        }
    }

    #[test]
    fn extract_headings_ignores_indented_headings() {
        // The pattern is matched against the raw line, never the stripped one.
        check_extract("08");
        check_extract("24");
    }

    #[test]
    fn extract_headings_over_crlf_lone_cr_and_missing_final_newline() {
        for name in ["11", "12", "13"] {
            check_extract(name);
        }
    }

    #[test]
    fn extract_headings_splits_on_all_eleven_python_line_boundaries() {
        check_extract("16");
    }

    #[test]
    fn extract_headings_on_documents_without_headings() {
        for name in ["19", "20", "21"] {
            check_extract(name);
        }
    }

    #[test]
    fn extract_headings_document_is_a_single_crlf_marker_block() {
        check_extract("02");
    }

    // ---------------------------------------------------------- generate_toc

    #[test]
    fn generate_toc_markdown_table_matches_cpython() {
        for row in TOC_GEN_CASES {
            let hs = extract_headings(row.1);
            assert_eq!(
                generate_toc_markdown(&hs, row.2, row.3, row.4, row.5),
                row.6,
                "generate_toc_markdown case {}",
                row.0
            );
        }
    }

    #[test]
    fn generate_toc_markdown_depth_window_and_relative_indent() {
        for name in ["03", "08", "11", "13", "14", "19", "20"] {
            check_gen(name);
        }
    }

    #[test]
    fn generate_toc_markdown_ordered_counters_reset_children() {
        for name in ["04", "05", "10", "16", "18"] {
            check_gen(name);
        }
    }

    #[test]
    fn generate_toc_markdown_ignore_titles_is_case_insensitive_on_the_title() {
        for name in ["06", "07", "17"] {
            check_gen(name);
        }
    }

    #[test]
    fn generate_toc_markdown_scrubs_emphasis_but_keeps_the_anchor() {
        check_gen("12");
        check_gen("01");
    }

    #[test]
    fn generate_toc_markdown_of_no_headings_is_empty() {
        check_gen("09");
        let hs = extract_headings("no headings here");
        assert_eq!(generate_toc_markdown(&hs, 1, 6, false, None), "");
        assert_eq!(generate_toc_markdown(&[], 1, 6, true, Some(&["a"])), "");
    }

    // ------------------------------------------------------ process_toc_marker

    #[test]
    fn process_toc_markers_table_matches_cpython() {
        for (name, input, want) in MARKER_CASES {
            assert_eq!(&process_toc_markers(input), want, "marker case {}", name);
        }
    }

    #[test]
    fn process_toc_markers_accepts_both_bracket_cases_and_the_import_form() {
        // `[TOC]`, `[toc]`, `<!-- @import "[TOC]" -->`, the single-quote variant,
        // the whitespace-free spelling, and the fact that `[toc]` is *not*
        // accepted inside the @import form.
        for name in ["02", "03", "04", "05", "19", "22"] {
            check_marker(name);
        }
    }

    #[test]
    fn process_toc_markers_with_the_marker_appearing_twice() {
        for name in ["08", "24", "34", "35"] {
            check_marker(name);
        }
    }

    #[test]
    fn process_toc_markers_parses_only_depth_and_ordered_attributes() {
        for name in ["06", "07", "09", "10", "11", "23", "28", "29", "30", "31", "33"] {
            check_marker(name);
        }
    }

    #[test]
    fn process_toc_markers_leaves_a_crlf_document_untouched() {
        check_marker("13");
    }

    #[test]
    fn process_toc_markers_leaves_unmatched_variants_alone() {
        // inline text, an unterminated attribute brace, a U+3000 indent and a
        // surrounding CJK sentence all fail the `^[ \t]* ... [ \t]*$` frame.
        for name in ["15", "18", "25", "32"] {
            check_marker(name);
        }
    }

    #[test]
    fn process_toc_markers_emits_nothing_when_the_document_has_no_headings() {
        check_marker("14");
    }

    #[test]
    fn process_toc_markers_widened_whitespace_still_reaches_the_marker() {
        for name in ["36", "37"] {
            check_marker(name);
        }
    }

    #[test]
    fn process_toc_markers_at_end_of_document_without_a_trailing_newline() {
        check_marker("27");
    }

    #[test]
    fn process_toc_markers_depth_outside_the_real_range_widens_the_tree() {
        check_marker("20");
        check_marker("26");
    }

    #[test]
    fn default_depth_constants_match_the_python_signature() {
        assert_eq!(TOC_DEFAULT_DEPTH_FROM, 1);
        assert_eq!(TOC_DEFAULT_DEPTH_TO, 6);
    }

    #[test]
    fn py_first3_slices_code_points_not_bytes() {
        assert_eq!(py_first3("\u{4e00}\u{4e01}\u{4e02}x"), "\u{4e00}\u{4e01}\u{4e02}");
        assert_eq!(py_first3("````"), "```");
        assert_eq!(py_first3(""), "");
    }
}
