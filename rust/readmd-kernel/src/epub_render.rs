//! `src/readmd_modules/mdexport/epub_render.py` — the part of the EPUB 3
//! packaging engine that had no Rust counterpart: the lightweight, stdlib-only
//! Markdown -> XHTML paragraph renderer.
//!
//! The rest of that authority already lives in `mdexport.rs` under the same
//! names — `mdexport.rs:2331 build_epub_css`, `mdexport.rs:2224
//! split_into_chapters`, `mdexport.rs:2689 export_epub` (which the file's own
//! doc comment also credits for the `build_epub` alias, since Python ends with
//! `build_epub = export_epub`).  `mdexport::export_document("epub", ..)` is the
//! dispatcher that stands in for `epub_render.render_epub`.  Only
//! [`_simple_md_to_html`] was genuinely absent, and it is ported here
//! line-for-line from `epub_render.py:23-147`.
//!
//! Escaping is the reason this file exists separately from any Markdown -> HTML
//! helper elsewhere in the kernel: `html.escape(s)` is called with the default
//! `quote=True`, so `"` becomes `&quot;` and `'` becomes `&#x27;`, and it is
//! called **before** the inline `**bold**` / `` `code` `` substitutions.  The
//! consequence is that raw HTML in a paragraph can never reach the output as
//! markup, while emphasis that overlaps a code span is still rewritten inside
//! it — both are pinned by the snapshot tables below.

use lazy_static::lazy_static;
use regex::Regex;

use crate::link_indexer::{py_splitlines, py_strip};
use crate::mdexport::html_escape;

/// `epub_render.EPUB_CSS` (`epub_render.py:23-69`) — the module-level fallback
/// stylesheet.  Kept for surface parity; note that no Python callable reads it
/// (`build_epub_css` re-emits the same rules from the options dict), so it is
/// unreferenced in the authority exactly as it is here.
pub const EPUB_CSS: &str = r#"
@charset "utf-8";
body {
    font-family: -apple-system, BlinkMacSystemFont, "PingFang SC", "Microsoft YaHei", serif;
    margin: 5% 8%;
    line-height: 1.8;
    color: #1a1a1a;
}
h1, h2, h3, h4, h5, h6 {
    font-family: -apple-system, BlinkMacSystemFont, "PingFang SC", "Microsoft YaHei", sans-serif;
    font-weight: 600;
    line-height: 1.4;
    color: #0f172a;
    page-break-after: avoid;
}
h1 { font-size: 1.8em; margin-top: 1.5em; border-bottom: 1px solid #e2e8f0; padding-bottom: 0.3em; }
h2 { font-size: 1.4em; margin-top: 1.3em; }
p { margin: 0.8em 0; text-align: justify; }
pre, code {
    font-family: "Courier New", Courier, monospace;
    background-color: #f1f5f9;
    font-size: 0.9em;
}
pre {
    padding: 12px;
    border-radius: 6px;
    overflow-x: auto;
    border: 1px solid #e2e8f0;
}
blockquote {
    margin: 1em 0;
    padding-left: 1em;
    border-left: 4px solid #3b82f6;
    color: #475569;
}
table {
    width: 100%;
    border-collapse: collapse;
    margin: 1.2em 0;
}
th, td {
    border: 1px solid #cbd5e1;
    padding: 8px 10px;
    text-align: left;
}
th { background-color: #f8fafc; }
"#;

lazy_static! {
    /// `re.sub(r'\*\*(.+?)\*\*', r'<strong>\1</strong>', escaped)`
    /// (`epub_render.py:140`).
    static ref BOLD_RE: Regex = Regex::new(r"\*\*(.+?)\*\*").unwrap();
    /// `re.sub(r'`(.+?)`', r'<code>\1</code>', escaped)` (`epub_render.py:141`).
    static ref CODE_RE: Regex = Regex::new(r"`(.+?)`").unwrap();
}

/// `epub_render._simple_md_to_html` (`epub_render.py:72-147`).
///
/// Lightweight, stdlib-only Markdown -> XHTML paragraph renderer.
///
/// Behaviours carried over verbatim from the Python, each of which a
/// "cleanup" would break:
/// * only ``` opens a code block — `~~~` is an ordinary paragraph
///   (`epub_render.py:84` tests a single literal, unlike `toc_engine.py:48`).
/// * an unclosed fence at EOF swallows its content: `code_lines` is only ever
///   emitted on the *closing* branch.
/// * the heading level comes from `len(line) - len(line.lstrip('#'))`, i.e. from
///   the **unstripped** line, so `  # x` yields `<h0># x</h0>` and
///   `####### x` yields `<h7>`, with no clamp to the HTML `h1..h6` range.
/// * block quotes open and close per line, so consecutive `> ` lines become
///   consecutive `<blockquote><p>` elements.
/// * the `-`/`*` list state is not closed by a fenced code block, and is closed
///   by a blank line, a heading, a quote or a paragraph.
/// * list/quote bodies use `stripped[2:]` while paragraphs use the raw `line`,
///   so paragraph indentation survives inside `<p>`.
pub fn _simple_md_to_html(md_text: &str) -> String {
    let lines = py_splitlines(md_text);
    let mut html_out: Vec<String> = Vec::new();
    let mut in_code = false;
    let mut code_lines: Vec<String> = Vec::new();
    let mut in_list = false;

    for line in &lines {
        let stripped = py_strip(line);

        // 代码块
        if stripped.starts_with("```") {
            if in_code {
                in_code = false;
                let escaped_code = html_escape(&code_lines.join("\n"), true);
                html_out.push(format!("<pre><code>{}</code></pre>", escaped_code));
                code_lines = Vec::new();
            } else {
                in_code = true;
                code_lines = Vec::new();
            }
            continue;
        }

        if in_code {
            code_lines.push(line.to_string());
            continue;
        }

        if stripped.is_empty() {
            if in_list {
                html_out.push("</ul>".to_string());
                in_list = false;
            }
            continue;
        }

        // 标题
        if stripped.starts_with('#') {
            if in_list {
                html_out.push("</ul>".to_string());
                in_list = false;
            }
            let hash_stripped = line.trim_start_matches('#');
            let level = line.chars().count() - hash_stripped.chars().count();
            let title_text = html_escape(py_strip(hash_stripped), true);
            html_out.push(format!("<h{}>{}</h{}>", level, title_text, level));
            continue;
        }

        // 列表
        if stripped.starts_with("- ") || stripped.starts_with("* ") {
            if !in_list {
                html_out.push("<ul>".to_string());
                in_list = true;
            }
            // `stripped[2:]` is a code-point slice, but the two bytes just
            // matched are ASCII `-`/`*` and a space, so the byte slice below
            // always lands on a char boundary.
            let item_text = html_escape(py_strip(&stripped[2..]), true);
            html_out.push(format!("<li>{}</li>", item_text));
            continue;
        }

        // 引用
        if stripped.starts_with("> ") {
            if in_list {
                html_out.push("</ul>".to_string());
                in_list = false;
            }
            let quote_text = html_escape(py_strip(&stripped[2..]), true);
            html_out.push(format!("<blockquote><p>{}</p></blockquote>", quote_text));
            continue;
        }

        // 普通段落
        if in_list {
            html_out.push("</ul>".to_string());
            in_list = false;
        }

        let escaped = html_escape(line, true);
        // 行内粗体与行内代码简单转译
        let escaped = BOLD_RE.replace_all(&escaped, |caps: &regex::Captures| {
            format!("<strong>{}</strong>", &caps[1])
        });
        let escaped = CODE_RE.replace_all(&escaped, |caps: &regex::Captures| {
            format!("<code>{}</code>", &caps[1])
        });
        html_out.push(format!("<p>{}</p>", escaped));
    }

    if in_list {
        html_out.push("</ul>".to_string());
    }

    html_out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

const MD2HTML_CASES: &[(&str, &str, &str)] = &[
    ("01", "# Title\n\nSome text here.\n", "<h1>Title</h1>\n<p>Some text here.</p>"),
    ("02", "<script>alert(1)</script>\n", "<p>&lt;script&gt;alert(1)&lt;/script&gt;</p>"),
    ("03", "He said \"hi\" & it's <b>fine</b>\n", "<p>He said &quot;hi&quot; &amp; it&#x27;s &lt;b&gt;fine&lt;/b&gt;</p>"),
    ("04", "`**a**`\n", "<p><code><strong>a</strong></code></p>"),
    ("05", "**bold** and `code` and **b2**\n", "<p><strong>bold</strong> and <code>code</code> and <strong>b2</strong></p>"),
    ("06", "```\ncode <x> & \"y\"\nmore\n```\n", "<pre><code>code &lt;x&gt; &amp; &quot;y&quot;\nmore</code></pre>"),
    ("07", "- one\n* two\n- three\n", "<ul>\n<li>one</li>\n<li>two</li>\n<li>three</li>\n</ul>"),
    ("08", "> quoted line\n", "<blockquote><p>quoted line</p></blockquote>"),
    ("09", ">no space quote\n", "<p>&gt;no space quote</p>"),
    ("10", "  # indented hash\n", "<h0># indented hash</h0>"),
    ("11", "####### seven\n", "<h7>seven</h7>"),
    ("12", "```\nunclosed\n", ""),
    ("13", "- a\n\npara\n", "<ul>\n<li>a</li>\n</ul>\n<p>para</p>"),
    ("14", "- a\n", "<ul>\n<li>a</li>\n</ul>"),
    ("15", "", ""),
    ("16", "\n\n\n", ""),
    ("17", "~~~\nfence not special\n~~~\n", "<p>~~~</p>\n<p>fence not special</p>\n<p>~~~</p>"),
    ("18", "para\r\n# H\r\n", "<p>para</p>\n<h1>H</h1>"),
    ("19", "   \t leading ws para \t  \n", "<p>   \t leading ws para \t  </p>"),
    ("20", "&amp; already escaped\n", "<p>&amp;amp; already escaped</p>"),
    ("21", "**a** b **c**\n", "<p><strong>a</strong> b <strong>c</strong></p>"),
    ("22", "`a` `b`\n", "<p><code>a</code> <code>b</code></p>"),
    ("23", "- \n", "<p>- </p>"),
    ("24", "text\n- l1\n# h\n> q\n", "<p>text</p>\n<ul>\n<li>l1</li>\n</ul>\n<h1>h</h1>\n<blockquote><p>q</p></blockquote>"),
    ("25", "# \u{4E2D}\u{6587}\u{6807}\u{9898}\n\u{6BB5}\u{843D} **\u{7C97}** `code`\n", "<h1>\u{4E2D}\u{6587}\u{6807}\u{9898}</h1>\n<p>\u{6BB5}\u{843D} <strong>\u{7C97}</strong> <code>code</code></p>"),
    ("26", "a `b ** c` d\n", "<p>a <code>b ** c</code> d</p>"),
    ("27", "`` ` ``\n", "<p><code>` </code> ``</p>"),
    ("28", "#h no space\n", "<h1>h no space</h1>"),
    ("29", "###\n", "<h3></h3>"),
    ("30", "****\n", "<p>****</p>"),
    ("31", "**\n", "<p>**</p>"),
    ("32", "para\n\n\n- l\n", "<p>para</p>\n<ul>\n<li>l</li>\n</ul>"),
    ("33", "x\ry\n", "<p>x</p>\n<p>y</p>"),
    ("34", "'single' \"double\" <>&\n", "<p>&#x27;single&#x27; &quot;double&quot; &lt;&gt;&amp;</p>"),
    ("35", "- a\n```\nx\n```\n", "<ul>\n<li>a</li>\n<pre><code>x</code></pre>\n</ul>"),
    ("36", "- a\n```\nx\n", "<ul>\n<li>a</li>\n</ul>"),
    ("37", "> q\n- l\n", "<blockquote><p>q</p></blockquote>\n<ul>\n<li>l</li>\n</ul>"),
    ("38", "para\n```\nc\n```\n# h\n", "<p>para</p>\n<pre><code>c</code></pre>\n<h1>h</h1>"),
    ("39", "**a `b` c**\n", "<p><strong>a <code>b</code> c</strong></p>"),
    ("40", "`<b>`\n", "<p><code>&lt;b&gt;</code></p>"),
    ("41", "``\n", "<p>``</p>"),
    ("42", "- a\n  - nested\n", "<ul>\n<li>a</li>\n<li>nested</li>\n</ul>"),
    ("43", "\u{A0}\n", ""),
    ("44", "# \u{A0}nbsp heading\n", "<h1>nbsp heading</h1>"),
    ("45", "a**b\nc", "<p>a**b</p>\n<p>c</p>"),
    ("46", "\u{001C}\n", ""),
    ("47", "\u{001C}\u{001D}\u{001E}\u{001F}", ""),
];

const EPUB_CSS_PY: &str = "\n@charset \"utf-8\";\nbody {\n    font-family: -apple-system, BlinkMacSystemFont, \"PingFang SC\", \"Microsoft YaHei\", serif;\n    margin: 5% 8%;\n    line-height: 1.8;\n    color: #1a1a1a;\n}\nh1, h2, h3, h4, h5, h6 {\n    font-family: -apple-system, BlinkMacSystemFont, \"PingFang SC\", \"Microsoft YaHei\", sans-serif;\n    font-weight: 600;\n    line-height: 1.4;\n    color: #0f172a;\n    page-break-after: avoid;\n}\nh1 { font-size: 1.8em; margin-top: 1.5em; border-bottom: 1px solid #e2e8f0; padding-bottom: 0.3em; }\nh2 { font-size: 1.4em; margin-top: 1.3em; }\np { margin: 0.8em 0; text-align: justify; }\npre, code {\n    font-family: \"Courier New\", Courier, monospace;\n    background-color: #f1f5f9;\n    font-size: 0.9em;\n}\npre {\n    padding: 12px;\n    border-radius: 6px;\n    overflow-x: auto;\n    border: 1px solid #e2e8f0;\n}\nblockquote {\n    margin: 1em 0;\n    padding-left: 1em;\n    border-left: 4px solid #3b82f6;\n    color: #475569;\n}\ntable {\n    width: 100%;\n    border-collapse: collapse;\n    margin: 1.2em 0;\n}\nth, td {\n    border: 1px solid #cbd5e1;\n    padding: 8px 10px;\n    text-align: left;\n}\nth { background-color: #f8fafc; }\n";

    fn check_md(name: &str) {
        for (n, i, e) in MD2HTML_CASES {
            if *n == name {
                assert_eq!(&_simple_md_to_html(i), e, "_simple_md_to_html case {}", name);
                return;
            }
        }
        panic!("no md->html case {}", name);
    }


    #[test]
    fn simple_md_to_html_table_matches_cpython() {
        for (name, input, want) in MD2HTML_CASES {
            assert_eq!(&_simple_md_to_html(input), want, "case {}", name);
        }
    }

    #[test]
    fn simple_md_to_html_escapes_every_html_metacharacter() {
        for name in ["02", "03", "34"] {
            check_md(name);
        }
        // CPython-verified (probe5.py): html.escape runs first, so no markup can
        // survive, and it is called with the default quote=True.
        assert_eq!(
            _simple_md_to_html("<img src=x onerror=alert(1)>\n"),
            "<p>&lt;img src=x onerror=alert(1)&gt;</p>");
        assert_eq!(
            _simple_md_to_html("<img src=\"x\" onerror=\"alert('1')\">\n"),
            "<p>&lt;img src=&quot;x&quot; onerror=&quot;alert(&#x27;1&#x27;)&quot;&gt;</p>");
        assert_eq!(
            _simple_md_to_html("A & B < C > D \"E\" 'F'\n"),
            "<p>A &amp; B &lt; C &gt; D &quot;E&quot; &#x27;F&#x27;</p>");
    }

    #[test]
    fn simple_md_to_html_escapes_an_already_escaped_entity_once() {
        check_md("20");
    }

    #[test]
    fn simple_md_to_html_headings_use_the_unstripped_line_for_the_level() {
        for name in ["01", "10", "11", "28", "29", "44"] {
            check_md(name);
        }
    }

    #[test]
    fn simple_md_to_html_lists_close_on_the_right_boundary() {
        for name in ["07", "13", "14", "23", "32", "37", "42"] {
            check_md(name);
        }
    }

    #[test]
    fn simple_md_to_html_list_state_survives_a_fenced_block() {
        // The `<pre>` branch never touches `in_list`, so `</ul>` lands after it.
        for name in ["35", "36"] {
            check_md(name);
        }
    }

    #[test]
    fn simple_md_to_html_quotes_open_and_close_per_line() {
        for name in ["08", "09", "24"] {
            check_md(name);
        }
    }

    #[test]
    fn simple_md_to_html_fences_only_react_to_backticks() {
        for name in ["06", "12", "17", "38"] {
            check_md(name);
        }
    }

    #[test]
    fn simple_md_to_html_emphasis_inside_a_code_span_wins() {
        // `**` is substituted before the backtick pass, so the code span is
        // rewritten even though it should be verbatim.
        for name in ["04", "26"] {
            check_md(name);
        }
    }

    #[test]
    fn simple_md_to_html_code_span_inside_emphasis() {
        for name in ["05", "21", "22", "39"] {
            check_md(name);
        }
    }

    #[test]
    fn simple_md_to_html_unterminated_inline_markup_is_left_alone() {
        for name in ["30", "31", "41", "45"] {
            check_md(name);
        }
    }

    #[test]
    fn simple_md_to_html_on_empty_and_whitespace_only_documents() {
        for name in ["15", "16", "43", "46", "47"] {
            check_md(name);
        }
    }

    #[test]
    fn simple_md_to_html_line_boundaries_and_paragraph_whitespace() {
        for name in ["18", "19", "33"] {
            check_md(name);
        }
    }

    #[test]
    fn simple_md_to_html_cjk_content_is_not_escaped() {
        check_md("25");
        check_md("40");
        check_md("27");
    }

    #[test]
    fn epub_css_constant_matches_the_python_module_string() {
        // Byte-for-byte equality with `epub_render.EPUB_CSS` as CPython reads it.
        assert_eq!(EPUB_CSS, EPUB_CSS_PY);
        assert!(EPUB_CSS.starts_with("\n@charset \"utf-8\";\n"));
        assert!(EPUB_CSS.ends_with("th { background-color: #f8fafc; }\n"));
        assert!(EPUB_CSS.contains(
            "font-family: -apple-system, BlinkMacSystemFont, \"PingFang SC\", \"Microsoft YaHei\", serif;"
        ));
        assert!(EPUB_CSS.contains("border-left: 4px solid #3b82f6;"));
    }
}

// ===========================================================================
// Golden differential test.  `golden_epub.json` is produced by running the
// *Python authority* `src/readmd_modules/mdexport/epub_render.py::_simple_md_to_html`
// offline (see scratch/rust_parity/bibtex_epub_s15/gen_goldens.py).  The
// expected strings here are CPython's answers, never the Rust output, so this
// test cannot go false-green by construction.
// ===========================================================================
#[cfg(test)]
mod golden_tests {
    use super::*;

    const GOLDEN: &str =
        include_str!("../../../scratch/rust_parity/bibtex_epub_s15/golden_epub.json");

    #[test]
    fn simple_md_to_html_matches_cpython_goldens() {
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(GOLDEN).expect("golden_epub.json must be a JSON array");
        assert!(!cases.is_empty(), "golden_epub.json produced zero cases");
        for c in &cases {
            let case = c["case"].as_str().unwrap_or("?").to_string();
            let input = c["input"].as_str().expect("golden input must be a string");
            let want = c["output"].as_str().expect("golden output must be a string");
            let got = _simple_md_to_html(input);
            assert_eq!(
                got, want,
                "CPython golden mismatch for epub case {}\n  input = {:?}\n  want  = {:?}\n  rust  = {:?}",
                case, input, want, got
            );
        }
    }
}
