//! Port of `src/readmd_modules/mdexport/formula.py` — the LaTeX *normalizer* and
//! the PNG header reader.
//!
//! ## Fidelity contract for [`repair_latex`]
//!
//! `formula.py:46-88` is a byte-level normalizer: whatever it returns is handed to
//! a downstream math renderer, so the Rust port must reproduce Python's output
//! *exactly*, character for character, for every input.  The Python body is four
//! stages and the stage order is load-bearing:
//!
//! 1. `if not latex: return ''` — the empty string is the only falsy `str`, so
//!    `" "` is *not* short-circuited (it goes on to become `''` via `strip()`).
//! 2. `t = latex.strip()` — CPython's whitespace set, **not** Rust's
//!    `char::is_whitespace` alone (see [`crate::pdf_editor::py_strip`], reused
//!    here): `U+001C..U+001F` are whitespace for Python but not for Rust.
//! 3. HTML entity unescape, chained on the *result* of the previous replace:
//!   `&amp;lt;` therefore becomes `&lt;` and then `<`.  `&amp;amp;` becomes
//!    `&amp;` — exactly one pass, never a fixpoint loop.
//! 4. 39 Unicode math symbols -> LaTeX commands, applied in list order.  The
//!    replacement texts are ASCII, so no rule can feed a later rule; the order of
//!    this loop is therefore *not* observable — but the order of stage 3 relative
//!    to stages 2 and 4 *is*, because an entity can unescape into a symbol and a
//!    symbol can introduce the backslash that stage 5 then skips.
//! 5. Brace balancing: a code-point scan where a backslash consumes **two**
//!    positions, so `\{` is inert while `\\{` (escaped backslash, then brace) is
//!    not.  Unbalanced *opens* get `'}' * n` appended; stray *closes* are left
//!    alone.  A trailing lone backslash runs the cursor past the end, which
//!    Python's `while i < len(t)` absorbs and Rust must too.
//!
//! Stage 5 indexes by **code point** (`t[i]`, `i += 1`, `i += 2`).  A Rust
//! `&str` is UTF-8, so the scan runs over `Vec<char>` — slicing by byte offset
//! would both panic off a char boundary and count CJK bytes as if they were
//! braces.  `scratch/docx_render_s1_repair_oracle.py` +
//! `docx_render_s1_repair_golden.json` are the pinned Python snapshots; the
//! `repair_latex_*` tests below replay them.
//!
//! ## What is deliberately *not* wired in
//!
//! In the Python authority `repair_latex` has exactly **one** caller:
//! `render_latex` (`formula.py:95`), i.e. the matplotlib/mathtext PNG lane.  The
//! OMML lane (`latex2omml.py` -> `latex2omml.rs`) never calls it.  Feeding
//! repaired text into [`crate::latex2omml::latex_to_omml`] would therefore make
//! the DOCX/PPTX math output diverge from Python, so this module exports the
//! normalizer but does not splice it into the converter.

/// `formula.py:56-67` — the `unicode_map` list, verbatim and in order.
///
/// Every value keeps Python's trailing space except `\sqrt`, which is written
/// without one because the next token in `sqrt(x)` must glue to the command.
pub const UNICODE_MAP: &[(&str, &str)] = &[
    ("\u{d7}", "\\times "),        // ×
    ("\u{f7}", "\\div "),          // ÷
    ("\u{b1}", "\\pm "),           // ±
    ("\u{2213}", "\\mp "),         // ∓
    ("\u{2264}", "\\le "),         // ≤
    ("\u{2265}", "\\ge "),         // ≥
    ("\u{2260}", "\\ne "),         // ≠
    ("\u{2248}", "\\approx "),     // ≈
    ("\u{2261}", "\\equiv "),      // ≡
    ("\u{221e}", "\\infty "),      // ∞
    ("\u{2211}", "\\sum "),        // ∑
    ("\u{220f}", "\\prod "),       // ∏
    ("\u{222b}", "\\int "),        // ∫
    ("\u{221a}", "\\sqrt"),        // √  (no trailing space)
    ("\u{2208}", "\\in "),         // ∈
    ("\u{2209}", "\\notin "),      // ∉
    ("\u{2282}", "\\subset "),     // ⊂
    ("\u{2286}", "\\subseteq "),   // ⊆
    ("\u{222a}", "\\cup "),        // ∪
    ("\u{2229}", "\\cap "),        // ∩
    ("\u{2200}", "\\forall "),     // ∀
    ("\u{2203}", "\\exists "),     // ∃
    ("\u{2207}", "\\nabla "),      // ∇
    ("\u{2202}", "\\partial "),    // ∂
    ("\u{3b1}", "\\alpha "),       // α
    ("\u{3b2}", "\\beta "),        // β
    ("\u{3b3}", "\\gamma "),       // γ
    ("\u{3b4}", "\\delta "),       // δ
    ("\u{3b5}", "\\varepsilon "),  // ε
    ("\u{3b8}", "\\theta "),       // θ
    ("\u{3bb}", "\\lambda "),      // λ
    ("\u{3bc}", "\\mu "),          // μ
    ("\u{3c0}", "\\pi "),          // π
    ("\u{3c3}", "\\sigma "),       // σ
    ("\u{3c4}", "\\tau "),         // τ
    ("\u{3c6}", "\\varphi "),      // φ
    ("\u{3c9}", "\\omega "),       // ω
    ("\u{394}", "\\Delta "),       // Δ
    ("\u{3a9}", "\\Omega "),       // Ω
];

/// `str.strip()` with no argument: reuses [`crate::pdf_editor::py_strip`], whose
/// set is `char::is_whitespace` plus the C0 separators `U+001C..U+001F` — the
/// same CPython `str.isspace()` model this file previously duplicated.
use crate::pdf_editor::py_strip;

/// `formula.py:46-88` — self-heal a truncated or non-standard LaTeX formula.
///
/// Returns the repaired text; byte-identical to the Python oracle for every
/// input (see the module docs for the four ordered stages).
pub fn repair_latex(latex: &str) -> String {
    // Stage 1 — `if not latex: return ''`.
    if latex.is_empty() {
        return String::new();
    }
    // Stage 2 — `t = latex.strip()`.
    let mut t = py_strip(latex).to_string();

    // Stage 3 — HTML entity recovery, chained on the previous result.
    t = t.replace("&amp;", "&");
    t = t.replace("&lt;", "<");
    t = t.replace("&gt;", ">");
    t = t.replace("&quot;", "\"");

    // Stage 4 — Unicode math symbols -> LaTeX commands, in list order.
    for (u, l) in UNICODE_MAP {
        t = t.replace(u, l);
    }

    // Stage 5 — auto-balance braces.  Code-point scan; a backslash swallows the
    // character after it, so `\{` never counts.  `i` may run one past the end,
    // exactly like Python's `while i < len(t)`.
    let chars: Vec<char> = t.chars().collect();
    let mut open_braces = 0usize;
    let mut i = 0usize;
    while i < chars.len() {
        match chars[i] {
            '\\' => i += 2,
            '{' => {
                open_braces += 1;
                i += 1;
            }
            '}' => {
                if open_braces > 0 {
                    open_braces -= 1;
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    if open_braces > 0 {
        for _ in 0..open_braces {
            t.push('}');
        }
    }
    t
}

/// `formula.py:120-129` — read `(px_w, px_h)` out of a PNG byte stream.
///
/// Works on `&[u8]` only: the signature mirrors `data[:8] != b'\x89PNG\r\n\x1a\n'`
/// and `struct.unpack('>II', data[16:24])`, whose out-of-range slice raises
/// `struct.error` and is swallowed by the `except` — hence "short input => None"
/// instead of a panic.
pub fn png_size(data: &[u8]) -> Option<(u32, u32)> {
    const SIG: &[u8] = b"\x89PNG\r\n\x1a\n";
    if data.len() < 8 || &data[..8] != SIG {
        return None;
    }
    if data.len() < 24 {
        return None;
    }
    let w = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
    let h = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
    Some((w, h))
}

/// `_CJK_RE` (`formula.py:15`) — the CJK / full-width trigger for the mathtext
/// font setup.  Used by [`needs_cjk_font`]; DOCX/PPTX output does not need it
/// (Word resolves fonts itself) but the parity surface and the PDF lane do.
pub fn needs_cjk_font(text: &str) -> bool {
    text.chars().any(|c| {
        ('\u{4e00}'..='\u{9fff}').contains(&c)
            || ('\u{3000}'..='\u{303f}').contains(&c)
            || ('\u{ff00}'..='\u{ffef}').contains(&c)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Replay the pinned Python snapshots (`docx_render_s1_repair_golden.json`,
    /// generated by `scratch/docx_render_s1_repair_oracle.py`).  Kept as a
    /// literal table so the test is hermetic and needs no oracle at build time.
    const GOLDEN: &[(&str, &str)] = &[
        // Auto-generated by scratch/docx_render_s1_golden_splice.py from
        // scratch/docx_render_s1_repair_golden.json: every row is observed CPython
        // output of mdexport/formula.py repair_latex(). Do not edit by hand.
        ("", ""),
        (" ", ""),
        ("\t\n", ""),
        ("\u{a0}", ""),
        ("\u{3000}x\u{3000}", "x"),
        ("\u{85}a", "a"),
        ("\u{2028}b\u{2029}", "b"),
        ("\u{1c}\u{1d}sep\u{1e}\u{1f}", "sep"),
        ("  x  ", "x"),
        ("&amp;", "&"),
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&quot;", "\""),
        ("a &amp; b", "a & b"),
        ("a &lt; b &gt; c", "a < b > c"),
        ("&amp;lt;", "<"),
        ("&amp;amp;", "&amp;"),
        ("&amp;quot;", "\""),
        ("&AMP;", "&AMP;"),
        ("&", "&"),
        ("&am", "&am"),
        ("&amp", "&amp"),
        ("<", "<"),
        (">", ">"),
        ("\"", "\""),
        ("\u{d7}", "\\times "),
        ("\u{f7}", "\\div "),
        ("\u{b1}", "\\pm "),
        ("\u{2213}", "\\mp "),
        ("\u{2264}", "\\le "),
        ("\u{2265}", "\\ge "),
        ("\u{2260}", "\\ne "),
        ("\u{2248}", "\\approx "),
        ("\u{2261}", "\\equiv "),
        ("\u{221e}", "\\infty "),
        ("\u{2211}", "\\sum "),
        ("\u{220f}", "\\prod "),
        ("\u{222b}", "\\int "),
        ("\u{221a}", "\\sqrt"),
        ("\u{2208}", "\\in "),
        ("\u{2209}", "\\notin "),
        ("\u{2282}", "\\subset "),
        ("\u{2286}", "\\subseteq "),
        ("\u{222a}", "\\cup "),
        ("\u{2229}", "\\cap "),
        ("\u{2200}", "\\forall "),
        ("\u{2203}", "\\exists "),
        ("\u{2207}", "\\nabla "),
        ("\u{2202}", "\\partial "),
        ("\u{3b1}", "\\alpha "),
        ("\u{3b2}", "\\beta "),
        ("\u{3b3}", "\\gamma "),
        ("\u{3b4}", "\\delta "),
        ("\u{3b5}", "\\varepsilon "),
        ("\u{3b8}", "\\theta "),
        ("\u{3bb}", "\\lambda "),
        ("\u{3bc}", "\\mu "),
        ("\u{3c0}", "\\pi "),
        ("\u{3c3}", "\\sigma "),
        ("\u{3c4}", "\\tau "),
        ("\u{3c6}", "\\varphi "),
        ("\u{3c9}", "\\omega "),
        ("\u{394}", "\\Delta "),
        ("\u{3a9}", "\\Omega "),
        ("a\u{d7}b\u{d7}c", "a\\times b\\times c"),
        ("\u{221a}{x}", "\\sqrt{x}"),
        ("\u{221a}{x", "\\sqrt{x}"),
        ("\u{2211}_{i=1}^{n}", "\\sum _{i=1}^{n}"),
        ("\u{b1}\u{221e}", "\\pm \\infty "),
        ("\u{2264}\u{2265}\u{2260}", "\\le \\ge \\ne "),
        ("\u{d7} \\times", "\\times  \\times"),
        ("a\u{3bc} b", "a\\mu  b"),
        ("\\mu\u{3bc}", "\\mu\\mu "),
        ("{", "{}"),
        ("{{", "{{}}"),
        ("{a}", "{a}"),
        ("{a", "{a}"),
        ("}", "}"),
        ("}}", "}}"),
        ("a}", "a}"),
        ("\\{", "\\{"),
        ("\\{a", "\\{a"),
        ("\\\\{", "\\\\{}"),
        ("\\\\\\{", "\\\\\\{"),
        ("{{}}", "{{}}"),
        ("{}}{{}", "{}}{{}}"),
        ("{\\frac{1}{2}", "{\\frac{1}{2}}"),
        ("\\left\\{x", "\\left\\{x"),
        ("x\\right\\}", "x\\right\\}"),
        ("\\{a\\}", "\\{a\\}"),
        ("\\{a}", "\\{a}"),
        ("{\\{}", "{\\{}"),
        ("\\{}", "\\{}"),
        ("&amp;\u{d7}{a", "&\\times {a}"),
        ("&lt;{a}", "<{a}"),
        ("\u{221a}{{}", "\\sqrt{{}}"),
        ("\\frac{a}{b}", "\\frac{a}{b}"),
        ("E=mc^{2}", "E=mc^{2}"),
        ("\\alpha+\\beta=\\pi", "\\alpha+\\beta=\\pi"),
        ("\\sqrt{16} = 4", "\\sqrt{16} = 4"),
        ("\\int_{0}^{1} x^{2} dx", "\\int_{0}^{1} x^{2} dx"),
        ("\\text{\u{4e2d}}\\times{a}", "\\text{\u{4e2d}}\\times{a}"),
        ("\u{4e2d}\u{6587}", "\u{4e2d}\u{6587}"),
        ("  \\frac{1}{2  ", "\\frac{1}{2}"),
        ("x }", "x }"),
        ("\\begin{pmatrix}1 & 2\\\\3 & 4\\end{pmatrix}",
            "\\begin{pmatrix}1 & 2\\\\3 & 4\\end{pmatrix}"),
        ("$\\\\alpha$", "$\\\\alpha$"),
        ("\\\\alpha", "\\\\alpha"),
        ("a\\\\", "a\\\\"),
        ("a\\\\{", "a\\\\{}"),
        ("\\{\\}\\{", "\\{\\}\\{"),
        ("\u{222b}_0^\u{221e} e^{-x^2}dx = \\frac{\\sqrt{\\pi}}{2}",
            "\\int _0^\\infty  e^{-x^2}dx = \\frac{\\sqrt{\\pi}}{2}"),
        ("trailing backslash \\", "trailing backslash \\"),
        ("\\text{\\u}", "\\text{\\u}"),
        ("endash\u{2013}", "endash\u{2013}"),
        ("\u{2264}&gt;", "\\le >"),
        ("\u{d7}\u{f7}\u{b1}\u{2213}\u{2264}", "\\times \\div \\pm \\mp \\le "),
    ];

    #[test]
    fn repair_latex_matches_python_golden_table() {
        let mut bad: Vec<String> = Vec::new();
        for (input, want) in GOLDEN {
            let got = repair_latex(input);
            if got != *want {
                bad.push(format!("in={:?} want={:?} got={:?}", input, want, got));
            }
        }
        assert!(bad.is_empty(), "repair_latex diverges:\n{}", bad.join("\n"));
    }

    #[test]
    fn repair_latex_golden_table_covers_every_mapped_symbol() {
        for (u, l) in UNICODE_MAP {
            assert_eq!(&repair_latex(u), l, "rule for {:?}", u);
        }
        assert_eq!(UNICODE_MAP.len(), 39);
    }

    #[test]
    fn repair_latex_entity_pass_is_single_not_fixpoint() {
        // `&amp;amp;` -> `&amp;` only; a fixpoint loop would keep going to `&`.
        assert_eq!(repair_latex("&amp;amp;"), "&amp;");
        // one pass over the chain is enough for the double-escaped forms
        assert_eq!(repair_latex("&amp;lt;"), "<");
        assert_eq!(repair_latex("&amp;gt;"), ">");
        assert_eq!(repair_latex("&amp;quot;"), "\"");
    }

    #[test]
    fn repair_latex_sqrt_is_the_only_rule_without_a_trailing_space() {
        assert_eq!(repair_latex("\u{221a}x"), "\\sqrtx");
        assert_eq!(repair_latex("\u{d7}\u{221a}"), "\\times \\sqrt");
    }

    #[test]
    fn repair_latex_backslash_consumes_two_code_points() {
        // \{ is escaped and must not count as an opener ...
        assert_eq!(repair_latex("\\{a"), "\\{a");
        // ... while \\{ is an escaped backslash plus a real opener.
        assert_eq!(repair_latex("\\\\{"), "\\\\{}");
        // a trailing lone backslash walks the cursor past the end without panic
        assert_eq!(repair_latex("a\\"), "a\\");
        assert_eq!(repair_latex("{a\\"), "{a\\}");
    }

    #[test]
    fn repair_latex_balances_opens_only() {
        assert_eq!(repair_latex("{a"), "{a}");
        assert_eq!(repair_latex("{{a"), "{{a}}");
        assert_eq!(repair_latex("a}"), "a}");
        assert_eq!(repair_latex("}}"), "}}");
        assert_eq!(repair_latex("{a}"), "{a}");
    }

    #[test]
    fn repair_latex_strips_cpython_whitespace_not_rust_trim() {
        // Rust's `trim` also strips U+00A0/U+0085/U+2028/U+3000, so a bare
        // `trim()` would look equivalent here; the C0 separators are where the
        // two disagree, and that is the direction that would silently mis-slice.
        assert_eq!(repair_latex("\u{1c}\u{1d}sep\u{1e}\u{1f}"), "sep");
        assert!(!repair_latex("\u{1c}\u{1d}sep\u{1e}\u{1f}").starts_with('\u{1c}'));
        assert_eq!(repair_latex("\u{3000}x\u{3000}"), "x");
        assert_eq!(repair_latex("\u{a0}"), "");
    }

    #[test]
    fn repair_latex_stage_order_is_observable() {
        // entity -> symbol -> balance: each stage feeds the next one.
        assert_eq!(repair_latex("&amp;\u{d7}{a"), "&\\times {a}");
        // symbol -> balance: the injected `\times ` makes the scanner skip `t`,
        // and the brace is still counted (it is not swallowed by the backslash).
        assert_eq!(repair_latex("\u{d7}{"), "\\times {}");
        // entity -> balance: `<` is not a brace, so nothing is appended.
        assert_eq!(repair_latex("&lt;{a}"), "<{a}");
    }

    #[test]
    fn repair_latex_never_panics_on_mixed_scripts() {
        for s in [
            "\u{4e2d}", "中{", "\u{1f600}", "😀{", "\\√{", "🏳️\u{200d}🌈}", "\u{a0}\u{2028}",
        ] {
            let _ = repair_latex(s);
        }
    }

    #[test]
    fn png_size_reads_ihdr_dimensions() {
        let mut img = vec![0x89u8, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'];
        img.extend_from_slice(&[0, 0, 0, 13, b'I', b'H', b'D', b'R']);
        img.extend_from_slice(&1234u32.to_be_bytes());
        img.extend_from_slice(&4321u32.to_be_bytes());
        img.extend_from_slice(&[8, 2, 0, 0, 0]);
        assert_eq!(png_size(&img), Some((1234, 4321)));
    }

    #[test]
    fn png_size_rejects_non_png_and_truncated_input() {
        assert_eq!(png_size(b""), None);
        assert_eq!(png_size(b"\x89PNG\r\n\x1a\n"), None);
        assert_eq!(png_size(b"GIF89a....12345678"), None);
        let mut short = vec![0x89u8, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'];
        short.extend_from_slice(&[0, 0, 0, 13, b'I', b'H', b'D', b'R', 0, 0]);
        assert_eq!(png_size(&short), None);
    }

    #[test]
    fn needs_cjk_font_matches_the_cjk_regex_ranges() {
        assert!(needs_cjk_font("E=mc^{2} 中"));
        assert!(needs_cjk_font("\u{3000}"));
        assert!(needs_cjk_font("\u{ff21}"));
        assert!(!needs_cjk_font("\\alpha \\text{a}"));
    }
}
