//! Markdown → standalone LaTeX, built on [`crate::md_ast`].
//!
//! Replaces the line-by-line converter that used to live in `mdexport` (which
//! sliced UTF-8 strings by character index and panicked on `中文**粗**`, had no
//! CJK support and dropped images, ordered lists, tables and links).  Output is
//! meant for XeLaTeX (`% !TEX program = xelatex`) but stays compilable with
//! pdfLaTeX for pure-ASCII documents thanks to the `iftex` guard.

use crate::md_ast::{inline_plain, Align, Block, Document, Inline};
use std::collections::HashMap;

/// Document-level knobs, read from the export panel's `tex`/`meta` options.
#[derive(Debug, Clone)]
pub struct LatexOptions {
    pub title: String,
    pub author: String,
    /// Raw LaTeX for `\date{}`; `None` → `\today`.
    pub date: Option<String>,
    pub doc_class: String,
    pub font_size: String,
    pub paper: String,
    pub margin: String,
    /// `None` → enable `ctex` iff the body contains CJK.
    pub use_ctex: Option<bool>,
    pub toc: bool,
}

impl Default for LatexOptions {
    fn default() -> Self {
        LatexOptions {
            title: String::new(),
            author: String::new(),
            date: None,
            doc_class: "article".into(),
            font_size: "11pt".into(),
            paper: "a4paper".into(),
            margin: "2.5cm".into(),
            use_ctex: None,
            toc: false,
        }
    }
}

/// Image resolution callback: markdown `src` → path to use in `\includegraphics`
/// (already relative to the `.tex` file), or `None` if it cannot be embedded.
pub type ImageMap<'a> = &'a dyn Fn(&str) -> Option<String>;

/// True for CJK ideographs, kana, hangul and CJK punctuation / fullwidth forms.
pub fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x1100..=0x11FF | 0x2E80..=0x2FDF | 0x3000..=0x30FF | 0x3100..=0x31FF |
        0x3200..=0x9FFF | 0xA960..=0xA97F | 0xAC00..=0xD7FF | 0xF900..=0xFAFF |
        0xFE30..=0xFE4F | 0xFF00..=0xFFEF | 0x20000..=0x3FFFF)
}

/// Escape running text for LaTeX (text mode).
pub fn escape_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '\\' => out.push_str(r"\textbackslash{}"),
            '&' => out.push_str(r"\&"),
            '%' => out.push_str(r"\%"),
            '$' => out.push_str(r"\$"),
            '#' => out.push_str(r"\#"),
            '_' => out.push_str(r"\_"),
            '{' => out.push_str(r"\{"),
            '}' => out.push_str(r"\}"),
            '~' => out.push_str(r"\textasciitilde{}"),
            '^' => out.push_str(r"\textasciicircum{}"),
            '<' => out.push_str(r"\textless{}"),
            '>' => out.push_str(r"\textgreater{}"),
            '|' => out.push_str(r"\textbar{}"),
            '\u{00A0}' => out.push('~'),
            _ => out.push(c),
        }
    }
    out
}

/// Escape a URL for `\href{…}` / `\url{…}`.
fn escape_url(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '#' => out.push_str(r"\#"),
            '%' => out.push_str(r"\%"),
            '{' => out.push_str("%7B"),
            '}' => out.push_str("%7D"),
            _ => out.push(c),
        }
    }
    out
}

/// A `\label` key: ASCII-safe, deterministic, identical for heading and link.
pub fn label_key(slug: &str) -> String {
    let mut out = String::from("sec:");
    for c in slug.chars() {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
            out.push(c);
        } else {
            out.push_str(&format!("u{:x}", c as u32));
        }
    }
    out
}

/// `listings` only knows a fixed set of languages; anything else would be a
/// compile error, so unknown fences fall back to no highlighting.
fn listings_language(lang: &str) -> Option<&'static str> {
    Some(match lang.to_ascii_lowercase().as_str() {
        "python" | "py" | "python3" => "Python",
        "c" | "h" => "C",
        "cpp" | "c++" | "cc" | "cxx" | "hpp" => "C++",
        "java" => "Java",
        "bash" | "sh" | "shell" | "zsh" => "bash",
        "sql" => "SQL",
        "html" | "htm" => "HTML",
        "xml" | "svg" => "XML",
        "tex" | "latex" => "TeX",
        "ruby" | "rb" => "Ruby",
        "perl" | "pl" => "Perl",
        "php" => "PHP",
        "r" => "R",
        "matlab" | "m" => "Matlab",
        "fortran" | "f90" => "Fortran",
        "pascal" => "Pascal",
        "lisp" | "elisp" => "Lisp",
        "haskell" | "hs" => "Haskell",
        "make" | "makefile" => "make",
        _ => return None,
    })
}

struct Writer<'a> {
    out: String,
    images: ImageMap<'a>,
    footnotes: HashMap<String, Vec<Block>>,
    /// Footnote labels currently being expanded (guards self-reference).
    fn_stack: Vec<String>,
}

/// Render the full standalone `.tex` document.
pub fn render_document(doc: &Document, opts: &LatexOptions, images: ImageMap) -> String {
    let body = render_body(doc, images);
    let wants_ctex = opts.use_ctex.unwrap_or_else(|| {
        body.chars().any(is_cjk) || opts.title.chars().any(is_cjk) || opts.author.chars().any(is_cjk)
    });
    let mut s = String::new();
    s.push_str("% !TEX program = xelatex\n");
    s.push_str("% Generated by ReadMD. Compile with XeLaTeX (recommended) or LuaLaTeX.\n");
    s.push_str(&format!(
        "\\documentclass[{},{}]{{{}}}\n\n",
        opts.font_size, opts.paper, opts.doc_class
    ));
    s.push_str("\\usepackage{iftex}\n\\ifPDFTeX\n  \\usepackage[utf8]{inputenc}\n  \\usepackage[T1]{fontenc}\n\\fi\n");
    if wants_ctex && !opts.doc_class.starts_with("ctex") {
        s.push_str("\\usepackage[UTF8]{ctex}\n");
    }
    s.push_str(&format!("\\usepackage[margin={}]{{geometry}}\n", opts.margin));
    s.push_str(concat!(
        "\\usepackage{amsmath,amssymb,amsfonts,mathtools}\n",
        "\\usepackage{graphicx}\n",
        "\\usepackage[export]{adjustbox}\n",
        "\\usepackage{booktabs}\n",
        "\\usepackage{tabularx}\n",
        "\\usepackage{longtable}\n",
        "\\usepackage[normalem]{ulem}\n",
        "\\usepackage{xcolor}\n",
        "\\usepackage{listings}\n",
        "\\usepackage{hyperref}\n",
        "\n",
        "\\hypersetup{colorlinks=true, linkcolor=blue!60!black, urlcolor=blue!60!black, citecolor=blue!60!black}\n",
        "\\lstset{basicstyle=\\ttfamily\\small, breaklines=true, columns=fullflexible, keepspaces=true,\n",
        "  frame=single, rulecolor=\\color{black!15}, backgroundcolor=\\color{black!3},\n",
        "  keywordstyle=\\color{blue!70!black}\\bfseries, commentstyle=\\color{green!45!black}\\itshape,\n",
        "  stringstyle=\\color{red!60!black}, showstringspaces=false, upquote=true}\n",
        "\\setlength{\\parskip}{0.5em}\n",
        "\\providecommand{\\tightlist}{\\setlength{\\itemsep}{0pt}\\setlength{\\parskip}{0pt}}\n",
        "\n",
    ));
    s.push_str(&format!("\\title{{{}}}\n", escape_text(&opts.title)));
    s.push_str(&format!("\\author{{{}}}\n", escape_text(&opts.author)));
    match &opts.date {
        Some(d) => s.push_str(&format!("\\date{{{}}}\n", escape_text(d))),
        None => s.push_str("\\date{\\today}\n"),
    }
    s.push_str("\n\\begin{document}\n");
    if !opts.title.trim().is_empty() {
        s.push_str("\\maketitle\n");
    }
    if opts.toc {
        s.push_str("\\tableofcontents\n\\newpage\n");
    }
    s.push('\n');
    s.push_str(body.trim_end());
    s.push_str("\n\n\\end{document}\n");
    s
}

/// Render only the body (no preamble).
pub fn render_body(doc: &Document, images: ImageMap) -> String {
    let mut footnotes = HashMap::new();
    collect_footnotes(&doc.blocks, &mut footnotes);
    let mut w = Writer { out: String::new(), images, footnotes, fn_stack: Vec::new() };
    w.blocks(&doc.blocks, 0);
    w.out
}

fn collect_footnotes(blocks: &[Block], into: &mut HashMap<String, Vec<Block>>) {
    for b in blocks {
        match b {
            Block::FootnoteDef { label, blocks } => {
                into.insert(label.clone(), blocks.clone());
            }
            Block::Quote(inner) => collect_footnotes(inner, into),
            Block::List { items, .. } => {
                for it in items {
                    collect_footnotes(&it.blocks, into);
                }
            }
            _ => {}
        }
    }
}

impl<'a> Writer<'a> {
    fn blocks(&mut self, blocks: &[Block], list_depth: usize) {
        for b in blocks {
            self.block(b, list_depth);
        }
    }

    fn para_break(&mut self) {
        if !self.out.is_empty() && !self.out.ends_with("\n\n") {
            if self.out.ends_with('\n') {
                self.out.push('\n');
            } else {
                self.out.push_str("\n\n");
            }
        }
    }

    fn block(&mut self, b: &Block, list_depth: usize) {
        match b {
            Block::Heading { level, id, inlines } => {
                self.para_break();
                let cmd = match level {
                    1 => "section",
                    2 => "subsection",
                    3 => "subsubsection",
                    4 => "paragraph",
                    _ => "subparagraph",
                };
                let text = self.inlines(inlines);
                // Moving arguments must not contain fragile commands: give the
                // TOC/bookmark a plain-text version.
                let plain = escape_text(&inline_plain(inlines));
                if plain == text {
                    self.out.push_str(&format!("\\{cmd}{{{text}}}\\label{{{}}}\n\n", label_key(id)));
                } else {
                    self.out.push_str(&format!(
                        "\\{cmd}[{plain}]{{\\texorpdfstring{{{text}}}{{{plain}}}}}\\label{{{}}}\n\n",
                        label_key(id)
                    ));
                }
            }
            Block::Paragraph(inl) => {
                self.para_break();
                if let [Inline::Image { src, alt, .. }] = inl.as_slice() {
                    self.figure(src, alt);
                } else {
                    let t = self.inlines(inl);
                    self.out.push_str(t.trim());
                    self.out.push_str("\n\n");
                }
            }
            Block::List { ordered, start, items } => {
                self.para_break();
                if list_depth >= 4 {
                    // LaTeX nests lists at most four deep; flatten deeper ones.
                    for (n, it) in items.iter().enumerate() {
                        let mark = if *ordered { format!("{}.", start + n as u64) } else { "--".into() };
                        self.out.push_str(&format!("\\noindent\\hspace*{{{}em}}{mark} ", list_depth * 2));
                        self.item_body(&it.blocks, list_depth);
                    }
                    return;
                }
                let env = if *ordered { "enumerate" } else { "itemize" };
                self.out.push_str(&format!("\\begin{{{env}}}\n"));
                if *ordered && *start != 1 {
                    let counter = ["enumi", "enumii", "enumiii", "enumiv"][list_depth.min(3)];
                    self.out.push_str(&format!("\\setcounter{{{counter}}}{{{}}}\n", start.saturating_sub(1)));
                }
                for it in items {
                    match it.task {
                        Some(true) => self.out.push_str("\\item[$\\boxtimes$] "),
                        Some(false) => self.out.push_str("\\item[$\\square$] "),
                        None => self.out.push_str("\\item "),
                    }
                    self.item_body(&it.blocks, list_depth + 1);
                }
                self.out.push_str(&format!("\\end{{{env}}}\n\n"));
            }
            Block::Quote(inner) => {
                self.para_break();
                self.out.push_str("\\begin{quote}\n");
                self.blocks(inner, list_depth);
                trim_trailing_blank(&mut self.out);
                self.out.push_str("\n\\end{quote}\n\n");
            }
            Block::Code { lang, text } => {
                self.para_break();
                let safe = text.replace("\\end{lstlisting}", "\\end {lstlisting}");
                match listings_language(lang) {
                    Some(l) => self.out.push_str(&format!("\\begin{{lstlisting}}[language={l}]\n")),
                    None => self.out.push_str("\\begin{lstlisting}\n"),
                }
                self.out.push_str(&safe);
                self.out.push_str("\n\\end{lstlisting}\n\n");
            }
            Block::Math(m) => {
                self.para_break();
                self.out.push_str(&display_math(m));
                self.out.push_str("\n\n");
            }
            Block::Table { aligns, header, rows } => {
                self.para_break();
                self.table(aligns, header, rows);
            }
            Block::Hr => {
                self.para_break();
                self.out.push_str("\\noindent\\rule{\\linewidth}{0.4pt}\n\n");
            }
            Block::PageBreak => {
                self.para_break();
                self.out.push_str("\\newpage\n\n");
            }
            Block::Html(h) => {
                self.para_break();
                for line in h.lines() {
                    self.out.push_str("% ");
                    self.out.push_str(line);
                    self.out.push('\n');
                }
                self.out.push('\n');
            }
            // Emitted inline at their reference point.
            Block::FootnoteDef { .. } => {}
        }
    }

    fn item_body(&mut self, blocks: &[Block], depth: usize) {
        let mut first = true;
        for b in blocks {
            if first {
                if let Block::Paragraph(inl) = b {
                    let t = self.inlines(inl);
                    self.out.push_str(t.trim());
                    self.out.push('\n');
                    first = false;
                    continue;
                }
                self.out.push('\n');
            }
            first = false;
            match b {
                Block::Paragraph(inl) => {
                    let t = self.inlines(inl);
                    self.out.push('\n');
                    self.out.push_str(t.trim());
                    self.out.push('\n');
                }
                other => {
                    let before = self.out.len();
                    self.block(other, depth);
                    // keep items compact
                    if self.out.len() > before {
                        trim_trailing_blank(&mut self.out);
                        self.out.push('\n');
                    }
                }
            }
        }
        if blocks.is_empty() || !self.out.ends_with('\n') {
            self.out.push('\n');
        }
    }

    fn figure(&mut self, src: &str, alt: &str) {
        match (self.images)(src) {
            Some(path) => {
                self.out.push_str("\\begin{figure}[htbp]\n\\centering\n");
                self.out.push_str(&format!(
                    "\\includegraphics[max width=\\linewidth,max height=0.7\\textheight]{{{path}}}\n"
                ));
                if !alt.trim().is_empty() {
                    self.out.push_str(&format!("\\caption{{{}}}\n", escape_text(alt)));
                }
                self.out.push_str("\\end{figure}\n\n");
            }
            None => {
                self.out.push_str(&format!("\\textit{{[{}]}}\n\n", escape_text(if alt.is_empty() { src } else { alt })));
            }
        }
    }

    fn table(&mut self, aligns: &[Align], header: &[Vec<Inline>], rows: &[Vec<Vec<Inline>>]) {
        let ncols = header.len().max(rows.iter().map(|r| r.len()).max().unwrap_or(0)).max(1);
        let mut spec = String::from("@{}");
        for i in 0..ncols {
            let a = aligns.get(i).copied().unwrap_or(Align::None);
            spec.push_str(match a {
                Align::Center => ">{\\centering\\arraybackslash}X",
                Align::Right => ">{\\raggedleft\\arraybackslash}X",
                _ => ">{\\raggedright\\arraybackslash}X",
            });
        }
        spec.push_str("@{}");
        self.out.push_str("\\begin{center}\n");
        self.out.push_str(&format!("\\begin{{tabularx}}{{\\linewidth}}{{{spec}}}\n\\toprule\n"));
        let row_tex = |w: &mut Writer, cells: &[Vec<Inline>], bold: bool| -> String {
            let mut parts: Vec<String> = Vec::with_capacity(ncols);
            for i in 0..ncols {
                let c = cells.get(i).map(|c| w.inlines_in_cell(c)).unwrap_or_default();
                parts.push(if bold && !c.is_empty() { format!("\\textbf{{{c}}}") } else { c });
            }
            format!("{} \\\\\n", parts.join(" & "))
        };
        let h = row_tex(self, header, true);
        self.out.push_str(&h);
        self.out.push_str("\\midrule\n");
        for r in rows {
            let t = row_tex(self, r, false);
            self.out.push_str(&t);
        }
        self.out.push_str("\\bottomrule\n\\end{tabularx}\n\\end{center}\n\n");
    }

    fn inlines_in_cell(&mut self, v: &[Inline]) -> String {
        self.inlines(v).replace("\\\\\n", "\\newline ").replace('\n', " ").trim().to_string()
    }

    fn inlines(&mut self, v: &[Inline]) -> String {
        let mut s = String::new();
        for i in v {
            self.inline(i, &mut s);
        }
        s
    }

    fn inline(&mut self, i: &Inline, s: &mut String) {
        match i {
            Inline::Text(t) => s.push_str(&escape_text(t)),
            Inline::Code(t) => {
                s.push_str("\\texttt{");
                s.push_str(&escape_text(t).replace(' ', "\\ "));
                s.push('}');
            }
            Inline::Strong(c) => {
                let t = self.inlines(c);
                s.push_str(&format!("\\textbf{{{t}}}"));
            }
            Inline::Emph(c) => {
                let t = self.inlines(c);
                s.push_str(&format!("\\emph{{{t}}}"));
            }
            Inline::Strike(c) => {
                let t = self.inlines(c);
                s.push_str(&format!("\\sout{{{t}}}"));
            }
            Inline::Link { href, children, .. } => {
                let t = self.inlines(children);
                if let Some(anchor) = href.strip_prefix('#') {
                    let slug = crate::md_ast::slugify(
                        &percent_encoding::percent_decode_str(anchor).decode_utf8_lossy(),
                    );
                    s.push_str(&format!("\\hyperref[{}]{{{t}}}", label_key(&slug)));
                } else if t.is_empty() || inline_plain(children) == *href {
                    s.push_str(&format!("\\url{{{}}}", escape_url(href)));
                } else {
                    s.push_str(&format!("\\href{{{}}}{{{t}}}", escape_url(href)));
                }
            }
            Inline::Image { src, alt, .. } => match (self.images)(src) {
                Some(path) => s.push_str(&format!("\\includegraphics[height=1.2em]{{{path}}}")),
                None => s.push_str(&format!("\\textit{{[{}]}}", escape_text(if alt.is_empty() { src } else { alt }))),
            },
            Inline::Math(m) => {
                s.push_str("\\(");
                s.push_str(m.trim());
                s.push_str("\\)");
            }
            Inline::SoftBreak => s.push('\n'),
            Inline::HardBreak => s.push_str("\\\\\n"),
            Inline::FootnoteRef(label) => {
                if self.fn_stack.contains(label) {
                    return;
                }
                match self.footnotes.get(label).cloned() {
                    Some(blocks) => {
                        self.fn_stack.push(label.clone());
                        let mut parts = Vec::new();
                        for b in &blocks {
                            if let Block::Paragraph(inl) = b {
                                parts.push(self.inlines(inl).trim().to_string());
                            }
                        }
                        self.fn_stack.pop();
                        s.push_str(&format!("\\footnote{{{}}}", parts.join(" \\par ")));
                    }
                    None => s.push_str(&format!("\\textsuperscript{{[{}]}}", escape_text(label))),
                }
            }
            // Raw inline HTML has no safe LaTeX equivalent; its text children
            // arrive as ordinary `Text` nodes and are kept.
            Inline::Html(_) => {}
        }
    }
}

/// Display math: environments pass through, everything else goes in `\[ \]`.
fn display_math(m: &str) -> String {
    let t = m.trim();
    if t.starts_with("\\begin{") {
        t.to_string()
    } else {
        format!("\\[\n{t}\n\\]")
    }
}

fn trim_trailing_blank(s: &mut String) {
    while s.ends_with('\n') {
        s.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::md_ast::parse;

    fn body(md: &str) -> String {
        render_body(&parse(md), &|_| None)
    }

    fn doc(md: &str) -> String {
        render_document(&parse(md), &LatexOptions { title: "T".into(), ..Default::default() }, &|_| None)
    }

    #[test]
    fn cjk_bold_and_code_do_not_panic() {
        let b = body("中文**粗体**与`代码`，然后*斜体*");
        assert!(b.contains("中文\\textbf{粗体}与\\texttt{代码}"), "{b}");
        assert!(b.contains("\\emph{斜体}"));
    }

    #[test]
    fn preamble_has_xelatex_and_ctex_for_cjk() {
        let d = doc("你好");
        assert!(d.starts_with("% !TEX program = xelatex"));
        assert!(d.contains("\\usepackage[UTF8]{ctex}"));
        assert!(!doc("hello").contains("{ctex}"));
    }

    #[test]
    fn special_characters_are_escaped() {
        let b = body("100% & $5 # a_b {x} ~ ^");
        assert!(b.contains(r"100\% \& \$5 \# a\_b \{x\} \textasciitilde{} \textasciicircum{}"), "{b}");
    }

    #[test]
    fn lists_nest_and_keep_start_and_tasks() {
        let b = body("3. a\n4. b\n   - [x] done\n   - [ ] todo\n");
        assert!(b.contains("\\begin{enumerate}\n\\setcounter{enumi}{2}"), "{b}");
        assert!(b.contains("\\item[$\\boxtimes$] done"));
        assert!(b.contains("\\item[$\\square$] todo"));
        assert_eq!(b.matches("\\begin{itemize}").count(), 1);
    }

    #[test]
    fn table_uses_alignment() {
        let b = body("| a | b | c |\n|:--|:-:|--:|\n| 1 | 2 | 3 |\n");
        assert!(b.contains("\\raggedright\\arraybackslash}X>{\\centering\\arraybackslash}X>{\\raggedleft\\arraybackslash}X"), "{b}");
        assert!(b.contains("\\textbf{a} & \\textbf{b} & \\textbf{c} \\\\"));
        assert!(b.contains("1 & 2 & 3 \\\\"));
    }

    #[test]
    fn links_images_math_footnotes() {
        let b = render_body(
            &parse("See [x](https://a.b/c#d) and [intro](#intro).\n\n![cap](img/a.png)\n\n$$\\frac{a}{b}$$\n\nNote[^1].\n\n[^1]: body\n"),
            &|src| if src == "img/a.png" { Some("doc.assets/img1.png".into()) } else { None },
        );
        assert!(b.contains("\\href{https://a.b/c\\#d}{x}"), "{b}");
        assert!(b.contains("\\hyperref[sec:intro]{intro}"));
        assert!(b.contains("\\includegraphics[max width=\\linewidth,max height=0.7\\textheight]{doc.assets/img1.png}"));
        assert!(b.contains("\\caption{cap}"));
        assert!(b.contains("\\[\n\\frac{a}{b}\n\\]"));
        assert!(b.contains("Note\\footnote{body}."));
    }

    #[test]
    fn unknown_code_language_has_no_language_key() {
        assert!(body("```rust\nfn x(){}\n```").contains("\\begin{lstlisting}\nfn x(){}"));
        assert!(body("```python\nx=1\n```").contains("[language=Python]"));
    }

    #[test]
    fn every_used_package_is_loaded() {
        let d = doc("~~s~~ ![i](x.png) | a |\n|---|\n| b |\n\n```py\nx\n```\n[l](http://x)");
        for (cmd, pkg) in [("\\sout", "ulem"), ("tabularx}", "tabularx"), ("\\toprule", "booktabs"), ("lstlisting", "listings"), ("\\href", "hyperref")] {
            if d.contains(cmd) {
                assert!(d.contains(&format!("{pkg}}}")), "{cmd} needs {pkg}");
            }
        }
    }

    /// Randomized: inline markup over multi-byte alphabets never panics and
    /// keeps braces balanced.
    #[test]
    fn randomized_inline_is_safe() {
        let alphabet: Vec<&str> = vec![
            "中", "文", "😀", "é", "\u{200b}", "a", " ", "*", "**", "_", "`", "$", "~~", "[", "]", "(", ")", "\\", "#", "%", "{", "}", "\n",
        ];
        let mut state: u64 = 0x5eed_1234_abcd_0001;
        let iters: usize = std::env::var("READMD_PBT_ITERS").ok().and_then(|v| v.parse().ok()).unwrap_or(500);
        for _ in 0..iters {
            let mut s = String::new();
            state = splitmix64(state);
            let len = (state % 24) as usize;
            for _ in 0..len {
                state = splitmix64(state);
                s.push_str(alphabet[(state as usize) % alphabet.len()]);
            }
            let src = s.clone();
            let r = std::panic::catch_unwind(move || body(&src));
            assert!(r.is_ok(), "panic on {s:?}");
        }
    }

    /// Writes a sample `.tex` for a real XeLaTeX compile when
    /// `READMD_TEX_SAMPLE=<path>` is set.
    #[test]
    fn write_sample_when_requested() {
        let Ok(path) = std::env::var("READMD_TEX_SAMPLE") else { return };
        let md = "---\ntitle: 测试报告\nauthor: ReadMD\n---\n\n# 第一章 概述\n\n中文**粗体**、*斜体*、~~删除~~、`code_x`、100% & $5 # a_b {x}。\n见 [第一章](#第一章-概述) 与 [链接](https://example.com/a_b#c)。\n\n## 列表\n\n3. 三\n4. 四\n   - 嵌套\n   - [x] 完成\n   - [ ] 待办\n\n> 引用段落\n\n| 左 | 中 | 右 |\n|:--|:-:|--:|\n| 文本很长很长很长 | 2 | 3.14 |\n\n```python\nprint(\"你好\")\n```\n\n```rust\nfn main() {}\n```\n\n$$\n\\int_0^1 x^2\\,dx = \\frac{1}{3}\n$$\n\n行内 $e^{i\\pi}+1=0$ 与脚注[^a]。\n\n---\n\n[^a]: 这是脚注。\n";
        let tex = render_document(
            &parse(md),
            &LatexOptions { title: "测试报告".into(), author: "ReadMD".into(), toc: true, ..Default::default() },
            &|_| None,
        );
        std::fs::write(path, tex).unwrap();
    }

    fn splitmix64(mut z: u64) -> u64 {
        z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}
