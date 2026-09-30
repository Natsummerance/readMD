//! EPUB 3 chapter bodies from the shared Markdown AST ([`crate::md_ast`]).
//!
//! Replaces the line parser the EPUB export used to share with the retired
//! Python port: nested and ordered lists, task items, table alignment, fenced
//! code with syntax colour, footnotes as EPUB pop-up notes, MathML formulas
//! (through the same LaTeX parser DOCX uses) and packaged local images.
//! Output is well-formed XHTML: raw HTML from the source is reduced to its
//! text, never copied through.

use crate::md_ast::{inline_plain, Align, Block, Document, Inline};
use std::collections::HashMap;

/// One EPUB chapter.
pub struct Chapter {
    pub title: String,
    pub body: String,
    /// The body contains MathML (the OPF item needs `properties="mathml"`).
    pub has_math: bool,
}

/// Maps a Markdown image `src` to its package-relative href (`images/img_1.png`),
/// or `None` when the image cannot be packaged (the caller records why).
pub type ImageMap<'a> = &'a mut dyn FnMut(&str) -> Option<String>;

pub fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            // Characters XML 1.0 forbids.
            '\u{0}'..='\u{8}' | '\u{b}' | '\u{c}' | '\u{e}'..='\u{1f}' | '\u{fffe}' | '\u{ffff}' => {}
            _ => o.push(c),
        }
    }
    o
}

/// Text content of a raw HTML fragment (tags and comments removed, entities
/// kept as written but re-escaped).
fn html_text(raw: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    let mut rest = raw;
    while !rest.is_empty() {
        if !in_tag {
            if let Some(stripped) = rest.strip_prefix("<!--") {
                rest = match stripped.find("-->") {
                    Some(e) => &stripped[e + 3..],
                    None => "",
                };
                continue;
            }
        }
        let c = rest.chars().next().unwrap();
        rest = &rest[c.len_utf8()..];
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    crate::mdexport::html_unescape(&out)
}

fn xml_id(slug: &str) -> String {
    let mut s = String::from("h-");
    for c in slug.chars() {
        if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
            s.push(c);
        } else {
            s.push('-');
        }
    }
    s
}

fn safe_href(href: &str) -> bool {
    let l = href.trim().to_ascii_lowercase();
    !(l.starts_with("javascript:") || l.starts_with("vbscript:") || l.starts_with("data:"))
}

struct Writer<'a, 'm> {
    images: ImageMap<'m>,
    /// slug → chapter file for cross-chapter `#anchor` links.
    anchors: &'a HashMap<String, String>,
    footnote_defs: &'a HashMap<String, Vec<Block>>,
    /// Footnotes referenced in the current chapter, in order.
    used_notes: Vec<String>,
    has_math: bool,
}

impl<'a, 'm> Writer<'a, 'm> {
    fn inlines(&mut self, v: &[Inline]) -> String {
        let mut out = String::new();
        for (idx, i) in v.iter().enumerate() {
            match i {
                Inline::Text(t) => out.push_str(&esc(t)),
                Inline::Code(t) => out.push_str(&format!("<code>{}</code>", esc(t))),
                Inline::Strong(c) => out.push_str(&format!("<strong>{}</strong>", self.inlines(c))),
                Inline::Emph(c) => out.push_str(&format!("<em>{}</em>", self.inlines(c))),
                Inline::Strike(c) => out.push_str(&format!("<del>{}</del>", self.inlines(c))),
                Inline::Link { href, title, children } => {
                    let inner = self.inlines(children);
                    let inner = if inner.is_empty() { esc(href) } else { inner };
                    if !safe_href(href) || href.is_empty() {
                        out.push_str(&inner);
                        continue;
                    }
                    let target = match href.strip_prefix('#') {
                        Some(anchor) => {
                            let slug = crate::md_ast::slugify(
                                &percent_encoding::percent_decode_str(anchor).decode_utf8_lossy(),
                            );
                            match self.anchors.get(&slug) {
                                Some(file) => format!("{file}#{}", xml_id(&slug)),
                                None => format!("#{}", xml_id(&slug)),
                            }
                        }
                        None => href.clone(),
                    };
                    let title_attr = if title.is_empty() { String::new() } else { format!(" title=\"{}\"", esc(title)) };
                    out.push_str(&format!("<a href=\"{}\"{title_attr}>{inner}</a>", esc(&target)));
                }
                Inline::Image { src, alt, .. } => match (self.images)(src) {
                    Some(href) => out.push_str(&format!("<img src=\"{}\" alt=\"{}\"/>", esc(&href), esc(alt))),
                    None => {
                        if !alt.is_empty() {
                            out.push_str(&format!("<span class=\"img-missing\">[{}]</span>", esc(alt)));
                        }
                    }
                },
                Inline::Math(m) => out.push_str(&self.math(m, false)),
                Inline::SoftBreak => {
                    // A source line break between two CJK characters is not a space.
                    let prev = crate::md_ast::inline_plain(&v[..idx]).chars().last();
                    let next = crate::md_ast::inline_plain(&v[idx + 1..]).chars().next();
                    let cjk = matches!((prev, next), (Some(a), Some(b)) if crate::latex_writer::is_cjk(a) && crate::latex_writer::is_cjk(b));
                    if !cjk {
                        out.push(' ');
                    }
                }
                Inline::HardBreak => out.push_str("<br/>"),
                Inline::FootnoteRef(label) => {
                    if self.footnote_defs.contains_key(label) {
                        let n = match self.used_notes.iter().position(|l| l == label) {
                            Some(p) => p + 1,
                            None => {
                                self.used_notes.push(label.clone());
                                self.used_notes.len()
                            }
                        };
                        out.push_str(&format!(
                            "<a epub:type=\"noteref\" href=\"#fn-{n}\" id=\"fnref-{n}\" class=\"noteref\"><sup>{n}</sup></a>"
                        ));
                    } else {
                        out.push_str(&format!("[{}]", esc(label)));
                    }
                }
                Inline::Html(h) => out.push_str(&esc(&html_text(h))),
            }
        }
        out
    }

    fn math(&mut self, latex: &str, display: bool) -> String {
        let latex = latex.trim();
        if latex.is_empty() {
            return String::new();
        }
        match crate::omml::latex_to_mathml(latex, display) {
            Some(m) => {
                self.has_math = true;
                if display {
                    format!("<div class=\"math-display\">{m}</div>")
                } else {
                    m
                }
            }
            None => {
                let cls = if display { "math-display math-src" } else { "math-src" };
                format!("<span class=\"{cls}\">{}</span>", esc(latex))
            }
        }
    }

    fn code(&self, lang: &str, text: &str) -> String {
        let mut body = String::new();
        for (kind, tok) in crate::code_highlight::tokenize(text, lang) {
            let cls = match kind {
                crate::code_highlight::Kind::Plain => None,
                crate::code_highlight::Kind::Keyword => Some("k"),
                crate::code_highlight::Kind::String => Some("s"),
                crate::code_highlight::Kind::Comment => Some("c"),
                crate::code_highlight::Kind::Number => Some("n"),
                crate::code_highlight::Kind::Type => Some("t"),
            };
            match cls {
                Some(c) => body.push_str(&format!("<span class=\"{c}\">{}</span>", esc(tok))),
                None => body.push_str(&esc(tok)),
            }
        }
        let label = if lang.is_empty() {
            String::new()
        } else {
            format!("<span class=\"code-lang\">{}</span>", esc(lang))
        };
        let cls = if lang.is_empty() { String::new() } else { format!(" class=\"language-{}\"", esc(lang)) };
        format!("<div class=\"code\">{label}<pre><code{cls}>{body}</code></pre></div>")
    }

    fn blocks(&mut self, blocks: &[Block], tight: bool) -> String {
        let mut out = String::new();
        for b in blocks {
            out.push_str(&self.block(b, tight));
            out.push('\n');
        }
        out
    }

    fn block(&mut self, b: &Block, tight: bool) -> String {
        match b {
            Block::Heading { level, id, inlines } => {
                let lv = (*level).clamp(1, 6);
                format!("<h{lv} id=\"{}\">{}</h{lv}>", xml_id(id), self.inlines(inlines))
            }
            Block::Paragraph(inl) => {
                // A paragraph that is only an image becomes a figure.
                let meaningful: Vec<&Inline> = inl
                    .iter()
                    .filter(|i| !matches!(i, Inline::SoftBreak) && !matches!(i, Inline::Text(t) if t.trim().is_empty()))
                    .collect();
                if let [Inline::Image { alt, .. }] = meaningful.as_slice() {
                    let img = self.inlines(inl);
                    if img.starts_with("<img") {
                        let cap = if alt.is_empty() { String::new() } else { format!("<figcaption>{}</figcaption>", esc(alt)) };
                        return format!("<figure>{}{cap}</figure>", img.trim());
                    }
                }
                let body = self.inlines(inl);
                if tight {
                    body
                } else {
                    format!("<p>{body}</p>")
                }
            }
            Block::List { ordered, start, items } => {
                let tag = if *ordered { "ol" } else { "ul" };
                let start_attr = if *ordered && *start != 1 { format!(" start=\"{start}\"") } else { String::new() };
                let has_task = items.iter().any(|i| i.task.is_some());
                let cls = if has_task { " class=\"task-list\"" } else { "" };
                let mut s = format!("<{tag}{start_attr}{cls}>");
                for it in items {
                    let single = it.blocks.len() <= 2 && matches!(it.blocks.first(), Some(Block::Paragraph(_)));
                    let mark = match it.task {
                        Some(true) => "<span class=\"task done\">☑</span> ",
                        Some(false) => "<span class=\"task\">☐</span> ",
                        None => "",
                    };
                    s.push_str(&format!("<li>{mark}{}</li>", self.blocks(&it.blocks, single).trim_end()));
                }
                s.push_str(&format!("</{tag}>"));
                s
            }
            Block::Quote(inner) => format!("<blockquote>{}</blockquote>", self.blocks(inner, false)),
            Block::Code { lang, text } => self.code(lang, text),
            Block::Math(m) => self.math(m, true),
            Block::Table { aligns, header, rows } => {
                let style = |ci: usize| -> String {
                    match aligns.get(ci).copied().unwrap_or(Align::None) {
                        Align::None => String::new(),
                        a => format!(" style=\"text-align:{}\"", a.as_str()),
                    }
                };
                let mut s = String::from("<table><thead><tr>");
                for (ci, c) in header.iter().enumerate() {
                    s.push_str(&format!("<th{}>{}</th>", style(ci), self.inlines(c)));
                }
                s.push_str("</tr></thead><tbody>");
                for r in rows {
                    s.push_str("<tr>");
                    for (ci, c) in r.iter().enumerate() {
                        s.push_str(&format!("<td{}>{}</td>", style(ci), self.inlines(c)));
                    }
                    s.push_str("</tr>");
                }
                s.push_str("</tbody></table>");
                s
            }
            Block::Hr => "<hr/>".to_string(),
            Block::PageBreak => "<div class=\"pagebreak\"></div>".to_string(),
            Block::Html(h) => {
                let t = html_text(h);
                if t.trim().is_empty() {
                    String::new()
                } else {
                    format!("<p>{}</p>", esc(t.trim()))
                }
            }
            // Definitions are emitted as notes at the end of the chapter that cites them.
            Block::FootnoteDef { .. } => String::new(),
        }
    }

    fn notes(&mut self) -> String {
        if self.used_notes.is_empty() {
            return String::new();
        }
        let mut s = String::from("<section class=\"footnotes\" epub:type=\"footnotes\"><hr/>");
        let mut i = 0usize;
        // Notes may cite further notes; the list grows while we walk it.
        while i < self.used_notes.len() {
            let label = self.used_notes[i].clone();
            i += 1;
            let def = self.footnote_defs.get(&label).cloned().unwrap_or_default();
            let body = self.blocks(&def, false);
            s.push_str(&format!(
                "<aside epub:type=\"footnote\" id=\"fn-{i}\"><a href=\"#fnref-{i}\" class=\"fn-back\">{i}.</a> {body}</aside>"
            ));
        }
        s.push_str("</section>");
        s
    }
}

fn collect_defs(blocks: &[Block], into: &mut HashMap<String, Vec<Block>>) {
    for b in blocks {
        match b {
            Block::FootnoteDef { label, blocks } => {
                into.entry(label.clone()).or_insert_with(|| blocks.clone());
            }
            Block::Quote(inner) => collect_defs(inner, into),
            Block::List { items, .. } => {
                for it in items {
                    collect_defs(&it.blocks, into);
                }
            }
            _ => {}
        }
    }
}

/// Split the document into chapters at `h1` (or `h1`/`h2` for `split_level`
/// `"h2"`) and render each one.  Content before the first split heading
/// becomes a "序言" chapter; empty chapters are dropped.
pub fn render_chapters(doc: &Document, split_level: &str, images: ImageMap) -> Vec<Chapter> {
    let max_level: u8 = if split_level == "h2" { 2 } else { 1 };
    let mut groups: Vec<(String, Vec<&Block>)> = Vec::new();
    let mut cur_title = "序言".to_string();
    let mut cur: Vec<&Block> = Vec::new();
    for b in &doc.blocks {
        if let Block::Heading { level, inlines, .. } = b {
            if *level <= max_level {
                if cur.iter().any(|x| !matches!(x, Block::FootnoteDef { .. })) {
                    groups.push((std::mem::take(&mut cur_title), std::mem::take(&mut cur)));
                }
                cur.clear();
                cur_title = inline_plain(inlines).trim().to_string();
            }
        }
        cur.push(b);
    }
    if cur.iter().any(|x| !matches!(x, Block::FootnoteDef { .. })) {
        groups.push((cur_title, cur));
    }

    // Anchor → chapter file, so `[see](#intro)` works across chapters.
    let mut anchors: HashMap<String, String> = HashMap::new();
    for (n, (_, blocks)) in groups.iter().enumerate() {
        let file = format!("chapter_{}.xhtml", n + 1);
        fn walk(b: &Block, file: &str, anchors: &mut HashMap<String, String>) {
            match b {
                Block::Heading { id, .. } => {
                    anchors.entry(id.clone()).or_insert_with(|| file.to_string());
                }
                Block::Quote(inner) => inner.iter().for_each(|x| walk(x, file, anchors)),
                Block::List { items, .. } => {
                    items.iter().flat_map(|i| i.blocks.iter()).for_each(|x| walk(x, file, anchors))
                }
                _ => {}
            }
        }
        for b in blocks {
            walk(b, &file, &mut anchors);
        }
    }
    let mut defs = HashMap::new();
    collect_defs(&doc.blocks, &mut defs);

    let mut out = Vec::new();
    for (title, blocks) in groups {
        let mut w = Writer { images: &mut *images, anchors: &anchors, footnote_defs: &defs, used_notes: Vec::new(), has_math: false };
        let mut body = String::new();
        for b in blocks {
            body.push_str(&w.block(b, false));
            body.push('\n');
        }
        body.push_str(&w.notes());
        let title = if title.is_empty() { "章节".to_string() } else { title };
        out.push(Chapter { title, body, has_math: w.has_math });
    }
    out
}

/// Extra stylesheet rules for the constructs this writer emits.
pub const EXTRA_CSS: &str = r#"
figure { margin: 1.2em 0; text-align: center; }
figure img, p img { max-width: 100%; height: auto; }
figcaption { font-size: 0.85em; color: #64748b; margin-top: 0.4em; }
.code { margin: 1em 0; }
.code-lang { display: block; font-size: 0.72em; color: #94a3b8; text-transform: uppercase; letter-spacing: 0.06em; margin-bottom: 0.2em; font-family: sans-serif; }
pre code { background: none; padding: 0; white-space: pre-wrap; word-wrap: break-word; }
p code, li code, td code { padding: 0.1em 0.3em; border-radius: 3px; }
.k { color: #0033b3; font-weight: bold; }
.s { color: #067d17; }
.c { color: #8c8c8c; font-style: italic; }
.n { color: #1750eb; }
.t { color: #7a3e9d; }
ul.task-list { list-style: none; padding-left: 1.2em; }
.task { font-family: sans-serif; }
.math-display { text-align: center; margin: 1em 0; overflow-x: auto; }
.math-src { font-family: monospace; }
math[display="block"] { display: block; margin: 0.8em auto; }
.noteref { text-decoration: none; }
.footnotes { font-size: 0.88em; color: #475569; margin-top: 2em; }
.footnotes aside { margin: 0.4em 0; }
.fn-back { text-decoration: none; }
.pagebreak { page-break-after: always; break-after: page; }
.img-missing { color: #94a3b8; font-style: italic; }
del { color: #64748b; }
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn render(md: &str) -> Vec<Chapter> {
        let doc = crate::md_ast::parse(md);
        let mut none = |_: &str| -> Option<String> { None };
        render_chapters(&doc, "h1", &mut none)
    }

    #[test]
    fn chapters_split_at_h1_with_preface() {
        let ch = render("intro\n\n# A\n\ntext\n\n## sub\n\n# B\n\nmore\n");
        let titles: Vec<&str> = ch.iter().map(|c| c.title.as_str()).collect();
        assert_eq!(titles, vec!["序言", "A", "B"]);
        assert!(ch[1].body.contains("<h2 id=\"h-sub\">sub</h2>"));
    }

    #[test]
    fn structure_survives() {
        let ch = render("# T\n\n3. a\n4. b\n   - [x] done\n\n| l | r |\n|:--|--:|\n| 1 | 2 |\n\n```rust\nlet x = 1;\n```\n");
        let b = &ch[0].body;
        assert!(b.contains("<ol start=\"3\">"), "{b}");
        assert!(b.contains("class=\"task-list\"") && b.contains("☑"), "{b}");
        assert!(b.contains("style=\"text-align:right\""), "{b}");
        assert!(b.contains("<span class=\"k\">let</span>"), "{b}");
    }

    #[test]
    fn math_becomes_mathml() {
        let ch = render("# M\n\nInline $x^2$.\n\n$$\n\\frac{a}{b}\n$$\n");
        assert!(ch[0].has_math);
        assert!(ch[0].body.contains("<msup>") && ch[0].body.contains("<mfrac>"), "{}", ch[0].body);
    }

    #[test]
    fn footnotes_are_epub_notes() {
        let ch = render("# F\n\nText[^a].\n\n[^a]: The note.\n");
        let b = &ch[0].body;
        assert!(b.contains("epub:type=\"noteref\""), "{b}");
        assert!(b.contains("epub:type=\"footnote\"") && b.contains("The note."), "{b}");
    }

    #[test]
    fn raw_html_is_reduced_to_text_and_links_are_filtered() {
        let ch = render("# H\n\n<div onclick=\"x()\">hi <b>there</b></div>\n\n[x](javascript:alert(1)) <span>s</span>\n");
        let b = &ch[0].body;
        assert!(!b.contains("onclick") && !b.contains("<div onclick"), "{b}");
        assert!(b.contains("hi there"), "{b}");
        assert!(!b.contains("javascript:"), "{b}");
    }

    #[test]
    fn cross_chapter_anchor_links() {
        let ch = render("# One\n\nSee [two](#two).\n\n# Two\n\nhere\n");
        assert!(ch[0].body.contains("href=\"chapter_2.xhtml#h-two\""), "{}", ch[0].body);
    }

    #[test]
    fn images_use_the_package_map() {
        let doc = crate::md_ast::parse("# I\n\n![cap](a.png)\n");
        let mut map = |s: &str| -> Option<String> { (s == "a.png").then(|| "images/img_1.png".to_string()) };
        let ch = render_chapters(&doc, "h1", &mut map);
        assert!(ch[0].body.contains("<figure><img src=\"images/img_1.png\" alt=\"cap\"/><figcaption>cap</figcaption></figure>"), "{}", ch[0].body);
    }
}
