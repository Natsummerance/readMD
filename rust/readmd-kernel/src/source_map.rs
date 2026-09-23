//! `src/readmd_core/source_map.py` — ReadMD AST 源码行号映射器 (Source Map Line Injector).
//!
//! Injects `data-source-line="N"` provenance for Markdown block elements:
//! headings (`<h1>`-`<h6>`), paragraphs (`<p>`), code blocks (`<pre>`),
//! blockquotes (`<blockquote>`), tables (`<table>`) and lists (`<ul>`, `<ol>`),
//! so the reader/editor can apply the front-end's linear interpolation between
//! anchors and stop collapsed formulas and long tables from drifting the scroll.
//!
//! Ported line for line from `source_map.py:1-70`.  Two behaviours of the
//! Python authority are bugs that callers depend on and are therefore
//! reproduced rather than repaired:
//!
//! * [`annotate_markdown_source_lines`] **drops** a fence line that opens with
//!   the other fence character while already inside a block.  `source_map.py:31-40`
//!   consumes the line in the `startswith` branch, matches neither
//!   `if not in_code` nor `elif curr_fence == fence_char`, and reaches `continue`
//!   without appending anything — so ``~~~`` inside a ``` block vanishes from
//!   the output while the line numbering of everything after it still advances.
//! * The two annotation branches at `source_map.py:47-52` are byte-identical, so
//!   `|`, `>`, `- `, `* ` and any other non-blank, non-comment line all get a
//!   marker; the distinction is dead but preserved.

use lazy_static::lazy_static;
use regex::Regex;

use crate::link_indexer::{py_splitlines, py_strip};
use crate::toc_engine::py_first3;

lazy_static! {
    /// `source_map.inject_source_line_attributes_to_html`'s pattern
    /// (`source_map.py:59-62`).
    ///
    /// `re.MULTILINE` is a no-op here (the pattern holds no `^`/`$`).  Each of
    /// the three `\s*` gaps is widened to Python's `\s` = `White_Space` plus
    /// `U+001C..U+001F`, and `[^>]*` still crosses newlines, which is why a
    /// custom element name such as `<my-tag>` is matched as the tag `my` with
    /// `-tag` left over in the attribute tail.
    static ref SOURCE_LINE_TO_ATTR: Regex = Regex::new(
        r#"<!--[\s\x{1c}-\x{1f}]*data-source-line="(\d+)"[\s\x{1c}-\x{1f}]*-->[\s\x{1c}-\x{1f}]*<([a-zA-Z0-9]+)([^>]*)>"#
    )
    .unwrap();
}

/// `source_map.annotate_markdown_source_lines` (`source_map.py:20-54`).
///
/// Writes `<!-- data-source-line="N" -->` above every block-level element of
/// the Markdown source.  `N` counts *source* lines as `enumerate(lines, 1)`
/// does, i.e. it is the index into `str.splitlines()` of the **input**, not a
/// position in the returned text — the dropped cross-fence line and every
/// `\r`/`\r\n`/`\v`/`\f`/`\x1c-`\x1e`/`\u{85}`/`\u{2028}`/`\u{2029}` boundary
/// therefore still burn a number.
pub fn annotate_markdown_source_lines(markdown_content: &str) -> String {
    let lines = py_splitlines(markdown_content);
    let mut annotated: Vec<String> = Vec::new();
    let mut in_code = false;
    let mut fence_char = String::new();

    for (line_idx, line) in lines.iter().enumerate() {
        // `enumerate(lines, start=1)`
        let line_idx = line_idx + 1;
        let stripped = py_strip(line);

        // 处理代码块定界符
        if stripped.starts_with("```") || stripped.starts_with("~~~") {
            let curr_fence = py_first3(stripped);
            if !in_code {
                in_code = true;
                fence_char = curr_fence;
                annotated.push(format!("<!-- data-source-line=\"{}\" -->\n{}", line_idx, line));
            } else if curr_fence == fence_char {
                in_code = false;
                annotated.push(line.to_string());
            }
            // No third branch: a mismatched fence opener while `in_code` is
            // dropped, exactly as in Python.
            continue;
        }

        if in_code {
            annotated.push(line.to_string());
            continue;
        }

        // 针对普通块级元素注入行号标记
        let is_block = stripped.starts_with('#')
            || stripped.starts_with('|')
            || stripped.starts_with('>')
            || stripped.starts_with("- ")
            || stripped.starts_with("* ");
        if is_block || (stripped != "" && !stripped.starts_with("<!--")) {
            annotated.push(format!("<!-- data-source-line=\"{}\" -->\n{}", line_idx, line));
        } else {
            annotated.push(line.to_string());
        }
    }

    annotated.join("\n")
}

/// `source_map.inject_source_line_attributes_to_html` (`source_map.py:57-70`).
///
/// Promotes each `<!-- data-source-line="N" -->` comment into a
/// `data-source-line="N"` attribute on the tag that follows it.  The comment
/// must be followed by `<tag>` with only whitespace between; anything else
/// (including a zero-width space, which neither engine treats as whitespace)
/// is left untouched.  A tag that already carries the attribute ends up with
/// two of them, because the Python replacement re-emits `rest_attrs` verbatim.
pub fn inject_source_line_attributes_to_html(html_content: &str) -> String {
    SOURCE_LINE_TO_ATTR
        .replace_all(html_content, |caps: &regex::Captures| {
            let line_no = &caps[1];
            let tag_name = &caps[2];
            let rest_attrs = caps
                .get(3)
                .map(|m| m.as_str())
                .unwrap_or("");
            format!("<{} data-source-line=\"{}\"{}>", tag_name, line_no, rest_attrs)
        })
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

const SM_CASES: &[(&str, &str, &str)] = &[
    ("01", "# \u{6807}\u{9898} (Line 1)\n\n\u{6BB5}\u{843D}\u{6587}\u{5B57} (Line 3)\n\n```python\ncode (Line 6)\n```", "<!-- data-source-line=\"1\" -->\n# \u{6807}\u{9898} (Line 1)\n\n<!-- data-source-line=\"3\" -->\n\u{6BB5}\u{843D}\u{6587}\u{5B57} (Line 3)\n\n<!-- data-source-line=\"5\" -->\n```python\ncode (Line 6)\n```"),
    ("02", "", ""),
    ("03", "\n\n\n", "\n\n"),
    ("04", "# A\r\n\r\npara\r\n", "<!-- data-source-line=\"1\" -->\n# A\n\n<!-- data-source-line=\"3\" -->\npara"),
    ("05", "# A\rpara\r", "<!-- data-source-line=\"1\" -->\n# A\n<!-- data-source-line=\"2\" -->\npara"),
    ("06", "# A\npara", "<!-- data-source-line=\"1\" -->\n# A\n<!-- data-source-line=\"2\" -->\npara"),
    ("07", "```\n~~~\n```\nafter\n", "<!-- data-source-line=\"1\" -->\n```\n```\n<!-- data-source-line=\"4\" -->\nafter"),
    ("08", "```\n# in\n", "<!-- data-source-line=\"1\" -->\n```\n# in"),
    ("09", "<!-- already a comment -->\npara\n", "<!-- already a comment -->\n<!-- data-source-line=\"2\" -->\npara"),
    ("10", "| a | b |\n> quote\n- item\n* star\n", "<!-- data-source-line=\"1\" -->\n| a | b |\n<!-- data-source-line=\"2\" -->\n> quote\n<!-- data-source-line=\"3\" -->\n- item\n<!-- data-source-line=\"4\" -->\n* star"),
    ("11", "# A\u{2028}# B\u{2029}para\u{000B}q\u{000C}r\n", "<!-- data-source-line=\"1\" -->\n# A\n<!-- data-source-line=\"2\" -->\n# B\n<!-- data-source-line=\"3\" -->\npara\n<!-- data-source-line=\"4\" -->\nq\n<!-- data-source-line=\"5\" -->\nr"),
    ("12", "  # indented hash\n", "<!-- data-source-line=\"1\" -->\n  # indented hash"),
    ("13", "~~~\nbody\n~~~\ntail\n", "<!-- data-source-line=\"1\" -->\n~~~\nbody\n~~~\n<!-- data-source-line=\"4\" -->\ntail"),
    ("14", "\n# after blank\n", "\n<!-- data-source-line=\"2\" -->\n# after blank"),
    ("15", "para\n\n# h\n", "<!-- data-source-line=\"1\" -->\npara\n\n<!-- data-source-line=\"3\" -->\n# h"),
    ("16", "# \u{4E2D}\u{6587}\n", "<!-- data-source-line=\"1\" -->\n# \u{4E2D}\u{6587}"),
    ("17", "  \n\t\n", "  \n\t"),
    ("18", "```x\ny\n```z\n", "<!-- data-source-line=\"1\" -->\n```x\ny\n```z"),
    ("19", "- \n- a\n", "<!-- data-source-line=\"1\" -->\n- \n<!-- data-source-line=\"2\" -->\n- a"),
    ("20", "text with\r\nnewlines\nlast", "<!-- data-source-line=\"1\" -->\ntext with\n<!-- data-source-line=\"2\" -->\nnewlines\n<!-- data-source-line=\"3\" -->\nlast"),
    ("21", "- a\n```\nx\n~~~\ny\n```\n- b\n", "<!-- data-source-line=\"1\" -->\n- a\n<!-- data-source-line=\"2\" -->\n```\nx\ny\n```\n<!-- data-source-line=\"7\" -->\n- b"),
    ("22", "<!-- data-source-line=\"5\" -->\npara\n", "<!-- data-source-line=\"5\" -->\n<!-- data-source-line=\"2\" -->\npara"),
    ("23", "\u{A0}\n# A\n", "\u{A0}\n<!-- data-source-line=\"2\" -->\n# A"),
    ("24", "# A\n\u{2028}\n", "<!-- data-source-line=\"1\" -->\n# A\n\n"),
    ("25", "````\nx\n````\n", "<!-- data-source-line=\"1\" -->\n````\nx\n````"),
    ("26", "```  \nx\n```  \n", "<!-- data-source-line=\"1\" -->\n```  \nx\n```  "),
    ("27", "\u{A0}", "\u{A0}"),
    ("28", "> q\n\n\n# h\n", "<!-- data-source-line=\"1\" -->\n> q\n\n\n<!-- data-source-line=\"4\" -->\n# h"),
    ("29", "\u{001C}\n# A\n", "\n\n<!-- data-source-line=\"3\" -->\n# A"),
    ("30", "# A\u{001C}# B\u{001D}para\u{001F}", "<!-- data-source-line=\"1\" -->\n# A\n<!-- data-source-line=\"2\" -->\n# B\n<!-- data-source-line=\"3\" -->\npara\u{001F}"),
];

const SM_HTML_CASES: &[(&str, &str, &str)] = &[
    ("01", "<!-- data-source-line=\"10\" -->\n<h2 class=\"section\">\u{6807}\u{9898}</h2>\n<!-- data-source-line=\"15\" -->\n<p>\u{5185}\u{5BB9}</p>", "<h2 data-source-line=\"10\" class=\"section\">\u{6807}\u{9898}</h2>\n<p data-source-line=\"15\">\u{5185}\u{5BB9}</p>"),
    ("02", "<!--data-source-line=\"3\"--><p>x</p>", "<p data-source-line=\"3\">x</p>"),
    ("03", "<!-- data-source-line=\"1\" -->   <div id=\"a\">y</div>", "<div data-source-line=\"1\" id=\"a\">y</div>"),
    ("04", "<!-- data-source-line=\"1\" -->\nnothing here", "<!-- data-source-line=\"1\" -->\nnothing here"),
    ("05", "<!-- data-source-line=\"1\" -->\n<p>a</p><!-- data-source-line=\"2\" -->\n<p>b</p>", "<p data-source-line=\"1\">a</p><p data-source-line=\"2\">b</p>"),
    ("06", "<!-- data-source-line=\"007\" --><br/>", "<br data-source-line=\"007\"/>"),
    ("07", "<!-- data-source-line=\"1\" --><p title=\"a>b\">x</p>", "<p data-source-line=\"1\" title=\"a>b\">x</p>"),
    ("08", "<!-- data-source-line=\"x\" --><p>y</p>", "<!-- data-source-line=\"x\" --><p>y</p>"),
    ("09", "<!-- data-source-line=\"1\" --><PRE class=\"c\">t</pre>", "<PRE data-source-line=\"1\" class=\"c\">t</pre>"),
    ("10", "<!-- data-source-line=\"1\" -->\n\n\n<h1>z</h1>", "<h1 data-source-line=\"1\">z</h1>"),
    ("11", "prefix <!-- data-source-line=\"1\" --><h1>z</h1> suffix", "prefix <h1 data-source-line=\"1\">z</h1> suffix"),
    ("12", "<!-- data-source-line=\"1\" --><h1>a</h1><!-- data-source-line=\"2\" --><h2>b</h2>", "<h1 data-source-line=\"1\">a</h1><h2 data-source-line=\"2\">b</h2>"),
    ("13", "", ""),
    ("14", "<!-- data-source-line=\"1\" -->\u{4E2D}<h1>x</h1>", "<!-- data-source-line=\"1\" -->\u{4E2D}<h1>x</h1>"),
    ("15", "<!-- data-source-line=\"12\" --><h3 data-source-line=\"9\">dup</h3>", "<h3 data-source-line=\"12\" data-source-line=\"9\">dup</h3>"),
    ("16", "<!-- data-source-line=\"1\" -->\u{001C}<p>x</p>", "<p data-source-line=\"1\">x</p>"),
    ("17", "<!-- data-source-line=\"1\" -->\u{200B}<p>x</p>", "<!-- data-source-line=\"1\" -->\u{200B}<p>x</p>"),
    ("18", "<!-- data-source-line=\"1\" -->\u{85}<p>x</p>", "<p data-source-line=\"1\">x</p>"),
    ("19", "<!-- data-source-line=\"1\" --><my-tag>x</my-tag>", "<my data-source-line=\"1\"-tag>x</my-tag>"),
    ("20", "<!--data-source-line=\"9\"-->\u{000B}\u{000C}<div>y</div>", "<div data-source-line=\"9\">y</div>"),
    ("21", "<!-- data-source-line=\"1\" -->\u{3000}<p>x</p>", "<p data-source-line=\"1\">x</p>"),
];

    fn check_sm(name: &str) {
        for (n, i, e) in SM_CASES {
            if *n == name {
                assert_eq!(
                    &annotate_markdown_source_lines(i),
                    e,
                    "annotate_markdown_source_lines case {}",
                    name
                );
                return;
            }
        }
        panic!("no annotate case {}", name);
    }

    fn check_smh(name: &str) {
        for (n, i, e) in SM_HTML_CASES {
            if *n == name {
                assert_eq!(
                    &inject_source_line_attributes_to_html(i),
                    e,
                    "inject_source_line_attributes_to_html case {}",
                    name
                );
                return;
            }
        }
        panic!("no inject case {}", name);
    }


    #[test]
    fn annotate_markdown_source_lines_table_matches_cpython() {
        for (name, input, want) in SM_CASES {
            assert_eq!(
                &annotate_markdown_source_lines(input),
                want,
                "annotate case {}",
                name
            );
        }
    }

    #[test]
    fn inject_source_line_attributes_to_html_table_matches_cpython() {
        for (name, input, want) in SM_HTML_CASES {
            assert_eq!(
                &inject_source_line_attributes_to_html(input),
                want,
                "inject case {}",
                name
            );
        }
    }

    #[test]
    fn annotate_numbers_lines_one_based_like_enumerate() {
        check_sm("01");
        check_sm("10");
        check_sm("28");
    }

    #[test]
    fn annotate_empty_document_yields_empty_output() {
        check_sm("02");
    }

    #[test]
    fn annotate_document_of_only_blank_lines_keeps_them_unannotated() {
        for name in ["03", "14", "17", "27"] {
            check_sm(name);
        }
    }

    #[test]
    fn annotate_normalises_crlf_lone_cr_and_missing_final_newline() {
        // The terminator information is lost with the split while every line
        // keeps its own source index.
        for name in ["04", "05", "06", "20"] {
            check_sm(name);
        }
    }

    #[test]
    fn annotate_counts_all_eleven_python_line_boundaries() {
        for name in ["11", "24", "29", "30"] {
            check_sm(name);
        }
    }

    #[test]
    fn annotate_drops_a_cross_fence_opener_but_still_advances_the_number() {
        for name in ["07", "13", "21"] {
            check_sm(name);
        }
    }

    #[test]
    fn annotate_code_block_bodies_never_get_a_marker() {
        for name in ["08", "18", "25", "26"] {
            check_sm(name);
        }
    }

    #[test]
    fn annotate_skips_pre_existing_comment_lines() {
        check_sm("09");
        check_sm("22");
    }

    #[test]
    fn annotate_indented_heading_and_bullet_forms() {
        for name in ["12", "15", "16", "19"] {
            check_sm(name);
        }
    }

    #[test]
    fn annotate_is_not_idempotent_a_second_pass_renumbers_every_line() {
        // Verified against CPython (probe5.py).  The `startswith('<!--')` guard
        // stops the previous comments from being annotated again, but `line_idx`
        // still counts them, so a second pass shifts every number it emits.
        let once = annotate_markdown_source_lines("para\n");
        assert_eq!(once, "<!-- data-source-line=\"1\" -->\npara");
        assert_eq!(
            annotate_markdown_source_lines(&once),
            "<!-- data-source-line=\"1\" -->\n<!-- data-source-line=\"2\" -->\npara");
        let two = annotate_markdown_source_lines("a\nb\n");
        assert_eq!(
            two,
            "<!-- data-source-line=\"1\" -->\na\n<!-- data-source-line=\"2\" -->\nb");
        assert_eq!(
            annotate_markdown_source_lines(&two),
            "<!-- data-source-line=\"1\" -->\n<!-- data-source-line=\"2\" -->\na\n\
             <!-- data-source-line=\"2\" -->\n<!-- data-source-line=\"4\" -->\nb");
    }

    #[test]
    fn inject_promotes_the_number_without_reading_it() {
        check_smh("06");
        check_smh("15");
    }

    #[test]
    fn inject_leaves_a_comment_that_is_not_followed_by_a_tag() {
        for name in ["04", "14", "17"] {
            check_smh(name);
        }
    }

    #[test]
    fn inject_widened_whitespace_spans_c1_controls_and_newlines() {
        for name in ["10", "16", "18", "20", "21"] {
            check_smh(name);
        }
    }

    #[test]
    fn inject_truncates_a_custom_element_at_the_hyphen() {
        check_smh("19");
    }

    #[test]
    fn inject_on_empty_input_is_empty() {
        check_smh("13");
    }

    #[test]
    fn inject_preserves_surrounding_markup_and_case() {
        for name in ["01", "05", "09", "11", "12"] {
            check_smh(name);
        }
    }

    #[test]
    fn annotation_and_promotion_are_inverse_halves_of_one_contract() {
        let annotated = annotate_markdown_source_lines("# \u{6807}\u{9898}\n");
        assert_eq!(annotated, "<!-- data-source-line=\"1\" -->\n# \u{6807}\u{9898}");
        let rendered = "<!-- data-source-line=\"1\" -->\n<h1>\u{6807}\u{9898}</h1>";
        assert_eq!(
            inject_source_line_attributes_to_html(rendered),
            "<h1 data-source-line=\"1\">\u{6807}\u{9898}</h1>"
        );
    }
}
