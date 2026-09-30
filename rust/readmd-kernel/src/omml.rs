//! Tiny OMML (Office Math Markup) tree reader plus a MathML writer.
//!
//! `latex2omml::latex_to_omml` is the kernel's single LaTeX math parser; DOCX
//! embeds its output directly.  This module parses that output back into a
//! tree so the other exporters agree with DOCX on every formula: EPUB gets
//! MathML ([`to_mathml`]) and the PDF renderer lays the tree out as vector
//! boxes (`math_layout`).
//!
//! The reader only has to understand what `latex2omml` writes (elements,
//! `m:val` attributes, text, the five XML entities and numeric references),
//! so it is a small hand-rolled scanner rather than a general XML parser.  It
//! is iterative and bounded, never panics, and rejects nesting deeper than
//! [`MAX_DEPTH`].

/// Deepest element nesting accepted; anything deeper is reported as an error
/// so callers can fall back to plain text instead of recursing without bound.
pub const MAX_DEPTH: usize = 200;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Node {
    /// Local name without the namespace prefix (`f`, `sSup`, `r`, `t`, …).
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
    /// Character data directly inside this element (only `m:t` has any).
    pub text: String,
}

impl Node {
    pub fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.name == name)
    }

    /// `m:val` of the named property child (`<m:chr m:val="∑"/>`).
    pub fn prop(&self, pr: &str, key: &str) -> Option<&str> {
        self.child(pr)?.child(key)?.attr("val")
    }

    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    /// Is this a property container (`fPr`, `rPr`, `naryPr`, …)?
    pub fn is_props(&self) -> bool {
        self.name.ends_with("Pr")
    }

    /// Concatenated `m:t` text of a run.
    pub fn run_text(&self) -> String {
        let mut s = String::new();
        for c in &self.children {
            if c.name == "t" {
                s.push_str(&c.text);
            }
        }
        s
    }

    /// Nesting depth of the subtree (a leaf is 1).
    pub fn depth(&self) -> usize {
        let mut max = 0usize;
        let mut stack: Vec<(&Node, usize)> = vec![(self, 1)];
        while let Some((n, d)) = stack.pop() {
            max = max.max(d);
            for c in &n.children {
                stack.push((c, d + 1));
            }
        }
        max
    }
}

fn local_name(raw: &str) -> String {
    match raw.rsplit_once(':') {
        Some((_, n)) => n.to_string(),
        None => raw.to_string(),
    }
}

fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        match tail.find(';') {
            Some(j) if j <= 10 => {
                let ent = &tail[1..j];
                let rep = match ent {
                    "amp" => Some('&'),
                    "lt" => Some('<'),
                    "gt" => Some('>'),
                    "quot" => Some('"'),
                    "apos" => Some('\''),
                    _ => {
                        let num = if let Some(h) = ent.strip_prefix("#x").or_else(|| ent.strip_prefix("#X")) {
                            u32::from_str_radix(h, 16).ok()
                        } else if let Some(d) = ent.strip_prefix('#') {
                            d.parse::<u32>().ok()
                        } else {
                            None
                        };
                        num.and_then(char::from_u32)
                    }
                };
                match rep {
                    Some(c) => {
                        out.push(c);
                        rest = &tail[j + 1..];
                    }
                    None => {
                        out.push('&');
                        rest = &tail[1..];
                    }
                }
            }
            _ => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Parse an OMML fragment into a synthetic root node named `#root`.
pub fn parse(xml: &str) -> Result<Node, String> {
    let mut stack: Vec<Node> = vec![Node { name: "#root".into(), ..Default::default() }];
    let bytes = xml.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'<' {
            let end = match xml[i..].find('>') {
                Some(e) => i + e,
                None => return Err("unterminated tag".into()),
            };
            let inner = &xml[i + 1..end];
            i = end + 1;
            if inner.starts_with('?') || inner.starts_with('!') {
                continue;
            }
            if let Some(close) = inner.strip_prefix('/') {
                let name = local_name(close.trim());
                if stack.len() < 2 {
                    return Err(format!("unbalanced </{name}>"));
                }
                let node = stack.pop().unwrap();
                if node.name != name {
                    return Err(format!("mismatched </{name}> for <{}>", node.name));
                }
                stack.last_mut().unwrap().children.push(node);
                continue;
            }
            let self_close = inner.ends_with('/');
            let body = if self_close { &inner[..inner.len() - 1] } else { inner };
            let body = body.trim();
            let (raw_name, attr_src) = match body.find(|c: char| c.is_whitespace()) {
                Some(k) => (&body[..k], &body[k..]),
                None => (body, ""),
            };
            let mut node = Node { name: local_name(raw_name), ..Default::default() };
            let mut a = attr_src;
            while let Some(eq) = a.find('=') {
                let key = local_name(a[..eq].trim());
                let after = a[eq + 1..].trim_start();
                let quote = match after.chars().next() {
                    Some(q @ ('"' | '\'')) => q,
                    _ => break,
                };
                let val_src = &after[1..];
                let close = match val_src.find(quote) {
                    Some(c) => c,
                    None => break,
                };
                node.attrs.push((key, decode_entities(&val_src[..close])));
                a = &val_src[close + 1..];
            }
            if self_close {
                stack.last_mut().unwrap().children.push(node);
            } else {
                if stack.len() > MAX_DEPTH {
                    return Err("formula nested too deeply".into());
                }
                stack.push(node);
            }
        } else {
            let next = xml[i..].find('<').map(|k| i + k).unwrap_or(bytes.len());
            let text = decode_entities(&xml[i..next]);
            stack.last_mut().unwrap().text.push_str(&text);
            i = next;
        }
    }
    if stack.len() != 1 {
        return Err("unclosed element".into());
    }
    Ok(stack.pop().unwrap())
}

/// Parse LaTeX through the shared converter; `Err` when the OMML is unusable.
pub fn from_latex(latex: &str, display: bool) -> Result<Node, String> {
    let xml = crate::latex2omml::latex_to_omml(latex, display);
    parse(&xml)
}

// ------------------------------------------------------------------ MathML

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            _ => o.push(c),
        }
    }
    o
}

/// Normalise a delimiter as `latex2omml` writes it (`\{`, `\|`, `\langle` …).
pub fn delimiter_char(raw: &str) -> String {
    let t = raw.trim();
    let t = match t {
        "\\{" => "{",
        "\\}" => "}",
        "\\|" | "\\Vert" | "\\lVert" | "\\rVert" | "‖" => "‖",
        "\\vert" | "\\lvert" | "\\rvert" => "|",
        "\\langle" => "⟨",
        "\\rangle" => "⟩",
        "\\lceil" => "⌈",
        "\\rceil" => "⌉",
        "\\lfloor" => "⌊",
        "\\rfloor" => "⌋",
        "." => "",
        other => other.strip_prefix('\\').unwrap_or(other),
    };
    t.to_string()
}

/// Operator class used for spacing (PDF) and `<mo>` classification (MathML).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Ord,
    Bin,
    Rel,
    Punct,
    Open,
    Close,
}

pub fn classify(c: char) -> Class {
    match c {
        '+' | '−' | '-' | '±' | '∓' | '×' | '÷' | '·' | '∗' | '⋆' | '∘' | '∙' | '⊗' | '⊕' | '⊙' | '∩' | '∪'
        | '∖' | '∧' | '∨' => Class::Bin,
        '=' | '<' | '>' | '≤' | '≥' | '≠' | '≈' | '≡' | '∼' | '≃' | '≅' | '∝' | '≪' | '≫' | '≺' | '≻' | '→'
        | '←' | '⇒' | '⇐' | '↔' | '⇔' | '↦' | '↑' | '↓' | '∈' | '∉' | '⊂' | '⊆' | '⊃' | '⊇' | '∣' | '⊥'
        | '∥' | ':' => Class::Rel,
        ',' | ';' => Class::Punct,
        '(' | '[' | '{' | '⟨' | '⌈' | '⌊' => Class::Open,
        ')' | ']' | '}' | '⟩' | '⌉' | '⌋' => Class::Close,
        _ => Class::Ord,
    }
}

/// Run style flags read from `m:rPr`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RunStyle {
    /// `m:nor` — ordinary text (`\text{}`).
    pub normal_text: bool,
    pub bold: bool,
    /// `m:i m:val="off"` — upright letters (`\mathrm{}`).
    pub upright: bool,
    pub double_struck: bool,
    pub script: bool,
}

pub fn run_style(r: &Node) -> RunStyle {
    let mut s = RunStyle::default();
    if let Some(pr) = r.child("rPr") {
        for c in &pr.children {
            match c.name.as_str() {
                "nor" => s.normal_text = true,
                "b" => s.bold = true,
                "i" if c.attr("val") == Some("off") => s.upright = true,
                "scr" => match c.attr("val") {
                    Some("double-struck") => s.double_struck = true,
                    Some("script") => s.script = true,
                    _ => {}
                },
                _ => {}
            }
        }
    }
    s
}

/// A run of several letters (a function name such as `sin`, or an unknown
/// command's name) is set upright, as TeX does for `\sin` / `\operatorname`.
pub fn is_word(text: &str) -> bool {
    text.trim().chars().filter(|c| c.is_alphabetic()).count() > 1
}

/// Convert a parsed OMML root into a MathML `<math>` element.
pub fn to_mathml(root: &Node, display: bool, alt: &str) -> String {
    let mut body = String::new();
    emit_seq(&root.children, &mut body, 0);
    format!(
        "<math xmlns=\"http://www.w3.org/1998/Math/MathML\" display=\"{}\" alttext=\"{}\"><mrow>{}</mrow></math>",
        if display { "block" } else { "inline" },
        esc(alt),
        body
    )
}

fn emit_children(n: &Node, out: &mut String, depth: usize) {
    emit_seq(&n.children, out, depth + 1);
}

fn is_numeric_run(n: &Node) -> bool {
    if n.name != "r" || run_style(n).normal_text {
        return false;
    }
    let t = n.run_text();
    !t.is_empty() && t.chars().all(|c| c.is_ascii_digit() || c == '.')
}

/// Emit siblings, joining consecutive digit runs (`3`, `.`, `1`, `4`) into one `<mn>`.
fn emit_seq(children: &[Node], out: &mut String, depth: usize) {
    let mut i = 0usize;
    while i < children.len() {
        if is_numeric_run(&children[i]) {
            let mut num = String::new();
            let mut j = i;
            while j < children.len() && is_numeric_run(&children[j]) {
                num.push_str(&children[j].run_text());
                j += 1;
            }
            // A lone trailing dot is punctuation, not part of the number.
            let tail_dot = num.ends_with('.') && num.len() > 1;
            let digits = if tail_dot { &num[..num.len() - 1] } else { &num[..] };
            if digits == "." {
                out.push_str("<mo>.</mo>");
            } else {
                out.push_str(&format!("<mn>{}</mn>", esc(digits)));
            }
            if tail_dot {
                out.push_str("<mo>.</mo>");
            }
            i = j;
            continue;
        }
        emit(&children[i], out, depth);
        i += 1;
    }
}

fn mrow(n: Option<&Node>, out: &mut String, depth: usize) {
    out.push_str("<mrow>");
    if let Some(n) = n {
        emit_children(n, out, depth);
    }
    out.push_str("</mrow>");
}

fn run_mathml(r: &Node, out: &mut String) {
    let text = r.run_text();
    let st = run_style(r);
    if text.is_empty() {
        return;
    }
    if st.normal_text {
        out.push_str(&format!("<mtext>{}</mtext>", esc(&text)));
        return;
    }
    let variant = if st.double_struck {
        " mathvariant=\"double-struck\""
    } else if st.script {
        " mathvariant=\"script\""
    } else if st.bold {
        " mathvariant=\"bold\""
    } else if st.upright {
        " mathvariant=\"normal\""
    } else {
        ""
    };
    if is_word(&text) {
        let v = if variant.is_empty() { " mathvariant=\"normal\"" } else { variant };
        out.push_str(&format!("<mi{v}>{}</mi>", esc(text.trim())));
        return;
    }
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c.is_ascii_digit() || (c == '.' && chars.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            let mut j = i;
            while j < chars.len() && (chars[j].is_ascii_digit() || chars[j] == '.') {
                j += 1;
            }
            let num: String = chars[i..j].iter().collect();
            out.push_str(&format!("<mn>{}</mn>", esc(&num)));
            i = j;
            continue;
        }
        if c.is_alphabetic() || matches!(c, '∞' | '∂' | '∇' | 'ℏ' | 'ℓ' | '∅' | '′') {
            out.push_str(&format!("<mi{variant}>{}</mi>", esc(&c.to_string())));
        } else {
            out.push_str(&format!("<mo>{}</mo>", esc(&c.to_string())));
        }
        i += 1;
    }
}

fn emit(n: &Node, out: &mut String, depth: usize) {
    if depth > MAX_DEPTH || n.is_props() {
        return;
    }
    match n.name.as_str() {
        "r" => run_mathml(n, out),
        "f" => {
            let nobar = n.prop("fPr", "type") == Some("noBar");
            out.push_str(if nobar { "<mfrac linethickness=\"0\">" } else { "<mfrac>" });
            mrow(n.child("num"), out, depth);
            mrow(n.child("den"), out, depth);
            out.push_str("</mfrac>");
        }
        "sSup" => {
            out.push_str("<msup>");
            mrow(n.child("e"), out, depth);
            mrow(n.child("sup"), out, depth);
            out.push_str("</msup>");
        }
        "sSub" => {
            out.push_str("<msub>");
            mrow(n.child("e"), out, depth);
            mrow(n.child("sub"), out, depth);
            out.push_str("</msub>");
        }
        "sSubSup" => {
            out.push_str("<msubsup>");
            mrow(n.child("e"), out, depth);
            mrow(n.child("sub"), out, depth);
            mrow(n.child("sup"), out, depth);
            out.push_str("</msubsup>");
        }
        "rad" => {
            let hide = n.prop("radPr", "degHide").is_some();
            let deg = n.child("deg").filter(|d| !d.children.is_empty());
            if hide || deg.is_none() {
                out.push_str("<msqrt>");
                mrow(n.child("e"), out, depth);
                out.push_str("</msqrt>");
            } else {
                out.push_str("<mroot>");
                mrow(n.child("e"), out, depth);
                mrow(deg, out, depth);
                out.push_str("</mroot>");
            }
        }
        "nary" => {
            let chr = n.prop("naryPr", "chr").unwrap_or("∫");
            let und_ovr = n.prop("naryPr", "limLoc") == Some("undOvr");
            let sub = n.child("sub").filter(|s| !s.children.is_empty());
            let sup = n.child("sup").filter(|s| !s.children.is_empty());
            let op = format!("<mo largeop=\"true\" movablelimits=\"true\">{}</mo>", esc(chr));
            let (tag_both, tag_sub, tag_sup) =
                if und_ovr { ("munderover", "munder", "mover") } else { ("msubsup", "msub", "msup") };
            match (sub, sup) {
                (Some(b), Some(p)) => {
                    out.push_str(&format!("<{tag_both}>{op}"));
                    mrow(Some(b), out, depth);
                    mrow(Some(p), out, depth);
                    out.push_str(&format!("</{tag_both}>"));
                }
                (Some(b), None) => {
                    out.push_str(&format!("<{tag_sub}>{op}"));
                    mrow(Some(b), out, depth);
                    out.push_str(&format!("</{tag_sub}>"));
                }
                (None, Some(p)) => {
                    out.push_str(&format!("<{tag_sup}>{op}"));
                    mrow(Some(p), out, depth);
                    out.push_str(&format!("</{tag_sup}>"));
                }
                (None, None) => out.push_str(&op),
            }
            if let Some(e) = n.child("e") {
                emit_children(e, out, depth);
            }
        }
        "d" => {
            let beg = delimiter_char(n.prop("dPr", "begChr").unwrap_or("("));
            let end = delimiter_char(n.prop("dPr", "endChr").unwrap_or(")"));
            out.push_str("<mrow>");
            if !beg.is_empty() {
                out.push_str(&format!("<mo fence=\"true\" stretchy=\"true\">{}</mo>", esc(&beg)));
            }
            for e in n.children.iter().filter(|c| c.name == "e") {
                emit_children(e, out, depth);
            }
            if !end.is_empty() {
                out.push_str(&format!("<mo fence=\"true\" stretchy=\"true\">{}</mo>", esc(&end)));
            }
            out.push_str("</mrow>");
        }
        "m" => {
            out.push_str("<mtable>");
            for row in n.children.iter().filter(|c| c.name == "mr") {
                out.push_str("<mtr>");
                for cell in row.children.iter().filter(|c| c.name == "e") {
                    out.push_str("<mtd>");
                    mrow(Some(cell), out, depth);
                    out.push_str("</mtd>");
                }
                out.push_str("</mtr>");
            }
            out.push_str("</mtable>");
        }
        "eqArr" => {
            out.push_str("<mtable columnalign=\"left\">");
            for row in n.children.iter().filter(|c| c.name == "e") {
                out.push_str("<mtr><mtd>");
                mrow(Some(row), out, depth);
                out.push_str("</mtd></mtr>");
            }
            out.push_str("</mtable>");
        }
        "acc" => {
            let chr = n.prop("accPr", "chr").unwrap_or("^");
            out.push_str("<mover accent=\"true\">");
            mrow(n.child("e"), out, depth);
            out.push_str(&format!("<mo>{}</mo></mover>", esc(chr)));
        }
        "bar" => {
            let bottom = n.prop("barPr", "pos") == Some("bot");
            if bottom {
                out.push_str("<munder>");
                mrow(n.child("e"), out, depth);
                out.push_str("<mo>_</mo></munder>");
            } else {
                out.push_str("<mover>");
                mrow(n.child("e"), out, depth);
                out.push_str("<mo>¯</mo></mover>");
            }
        }
        "limLow" => {
            out.push_str("<munder>");
            mrow(n.child("e"), out, depth);
            mrow(n.child("lim"), out, depth);
            out.push_str("</munder>");
        }
        "limUpp" => {
            out.push_str("<mover>");
            mrow(n.child("e"), out, depth);
            mrow(n.child("lim"), out, depth);
            out.push_str("</mover>");
        }
        "borderBox" => {
            out.push_str("<menclose notation=\"box\">");
            mrow(n.child("e"), out, depth);
            out.push_str("</menclose>");
        }
        // Containers (`oMath`, `oMathPara`, `e`, `num`, …) and anything unknown:
        // keep the content so nothing the author wrote is lost.
        _ => emit_children(n, out, depth),
    }
}

/// LaTeX → MathML through the shared OMML converter.  `None` when the formula
/// cannot be represented (the caller keeps the LaTeX source as text).
pub fn latex_to_mathml(latex: &str, display: bool) -> Option<String> {
    let root = from_latex(latex, display).ok()?;
    Some(to_mathml(&root, display, latex))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_latex2omml_output() {
        let root = from_latex(r"\frac{a}{b} + x^2", false).unwrap();
        let math = root.child("oMath").unwrap();
        assert!(math.child("f").is_some());
        assert!(math.child("sSup").is_some());
    }

    #[test]
    fn entities_and_attributes() {
        let root = parse("<m:r><m:t>a &amp; b &lt; &#x3b1;</m:t></m:r><m:chr m:val=\"&amp;\"/>").unwrap();
        assert_eq!(root.children[0].run_text(), "a & b < α");
        assert_eq!(root.children[1].attr("val"), Some("&"));
    }

    #[test]
    fn rejects_broken_xml() {
        assert!(parse("<m:r><m:t>x</m:r>").is_err());
        assert!(parse("<m:r>").is_err());
        assert!(parse("</m:r>").is_err());
        let deep = "<m:e>".repeat(MAX_DEPTH + 5) + &"</m:e>".repeat(MAX_DEPTH + 5);
        assert!(parse(&deep).is_err());
    }

    #[test]
    fn mathml_covers_common_constructs() {
        let m = latex_to_mathml(r"\sum_{i=1}^{n} \frac{\sqrt{x}}{\alpha_i} \le \left( \begin{matrix} 1 & 2 \\ 3 & 4 \end{matrix} \right)", true).unwrap();
        for tag in ["<munderover>", "<mfrac>", "<msqrt>", "<msub>", "<mtable>", "<mo>≤</mo>", "display=\"block\""] {
            assert!(m.contains(tag), "{tag} missing in {m}");
        }
        let m = latex_to_mathml(r"\sin x + \text{中文} + 3.14", false).unwrap();
        assert!(m.contains("<mi mathvariant=\"normal\">sin</mi>"), "{m}");
        assert!(m.contains("<mtext>中文</mtext>"), "{m}");
        assert!(m.contains("<mn>3.14</mn>"), "{m}");
    }

    #[test]
    fn delimiters_normalise() {
        assert_eq!(delimiter_char("\\{"), "{");
        assert_eq!(delimiter_char("\\|"), "‖");
        assert_eq!(delimiter_char("."), "");
        assert_eq!(delimiter_char("\\langle"), "⟨");
    }

    #[test]
    fn never_panics_on_odd_formulas() {
        for s in ["", "{", "}", "^", "_", "\\frac", "\\sqrt[", "\\left(", "\\begin{matrix}", "a^^b", "\\\\", "&&", "中文"] {
            let _ = latex_to_mathml(s, false);
            let _ = latex_to_mathml(s, true);
        }
    }
}
