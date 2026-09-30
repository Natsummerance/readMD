//! Markdown → DOCX (WordprocessingML), built on [`crate::md_ast`].
//!
//! Applies the sanitized export style (`export_styles::sanitize_options`):
//! page size / orientation / margins, body typography, H1–H6, tables (header
//! colours, borders, banding, padding, width, column alignment), code, quotes,
//! links, rules, header / footer with page numbers, optional cover page and
//! TOC field.  Emits real Word numbering for ordered / bullet / nested lists,
//! task checkboxes, hyperlinks (external and to heading bookmarks), footnotes,
//! embedded images and OMML math.

use crate::md_ast::{inline_plain, slugify, Align, Block, Document, Inline};
use serde_json::Value;
use std::collections::HashMap;

/// Image loader: markdown `src` → `(extension, bytes)`, or `None` when the
/// image cannot be embedded (the loader records its own warning).
pub type ImageLoader<'a> = &'a dyn Fn(&str) -> Option<(String, Vec<u8>)>;

/// The files of a `.docx` package, ready to be zipped.
pub struct DocxPackage {
    pub parts: Vec<(String, Vec<u8>)>,
    pub warns: Vec<String>,
}

const W_NS: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const R_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const REL_NS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";

// ------------------------------------------------------------------ helpers

/// XML-escape text and drop characters XML 1.0 forbids (Word refuses the file).
pub fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\t' | '\n' | '\r' => out.push(c),
            c if (c as u32) < 0x20 => {}
            '\u{FFFE}' | '\u{FFFF}' => {}
            c => out.push(c),
        }
    }
    out
}

fn dig<'a>(v: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut cur = v;
    for p in path {
        cur = cur.get(*p)?;
    }
    Some(cur)
}

fn num(v: &Value, path: &[&str], default: f64) -> f64 {
    match dig(v, path) {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(default),
        Some(Value::String(s)) => s.trim().parse().unwrap_or(default),
        _ => default,
    }
}

fn text<'a>(v: &'a Value, path: &[&str], default: &'a str) -> &'a str {
    dig(v, path).and_then(|x| x.as_str()).unwrap_or(default)
}

fn flag(v: &Value, path: &[&str], default: bool) -> bool {
    match dig(v, path) {
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().map(|x| x != 0.0).unwrap_or(default),
        _ => default,
    }
}

/// `#1a2b3c` → `1A2B3C` (invalid → `fallback`).
fn color(v: &Value, path: &[&str], fallback: &str) -> String {
    let raw = text(v, path, fallback).trim().trim_start_matches('#');
    let ok = |s: &str| s.len() == 6 && s.chars().all(|c| c.is_ascii_hexdigit());
    if ok(raw) {
        raw.to_ascii_uppercase()
    } else if raw.len() == 3 && raw.chars().all(|c| c.is_ascii_hexdigit()) {
        raw.chars().flat_map(|c| [c, c]).collect::<String>().to_ascii_uppercase()
    } else {
        fallback.trim_start_matches('#').to_ascii_uppercase()
    }
}

/// Export-style font key → the family name Word knows.
pub fn font_family(key: &str) -> &str {
    match key {
        "MicrosoftYaHei" => "Microsoft YaHei",
        "SimHei" => "SimHei",
        "SimSun" => "SimSun",
        "KaiTi" => "KaiTi",
        "DengXian" => "DengXian",
        "Arial" => "Arial",
        "Consolas" => "Consolas",
        "Courier New" => "Courier New",
        other => other,
    }
}

fn jc(align: &str) -> &'static str {
    match align {
        "center" => "center",
        "right" => "right",
        "justify" => "both",
        _ => "left",
    }
}

/// Points → twentieths of a point (twips).
fn twips(pt: f64) -> i64 {
    (pt * 20.0).round() as i64
}

/// Millimetres → twips.
fn mm_twips(mm: f64) -> i64 {
    (mm * 56.692_913).round() as i64
}

/// Heading bookmark name: Word allows ≤ 40 chars, letters/digits/underscore.
pub fn bookmark_name(slug: &str) -> String {
    let mut out = String::from("_h");
    for c in slug.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if c == '-' || c == '_' {
            out.push('_');
        } else {
            out.push_str(&format!("{:x}", c as u32));
        }
    }
    if out.len() > 40 {
        // keep it unique-ish: prefix + stable hash tail
        let mut h: u32 = 2166136261;
        for b in slug.bytes() {
            h = (h ^ b as u32).wrapping_mul(16777619);
        }
        out.truncate(31);
        out.push_str(&format!("{h:08x}"));
        out.truncate(40);
    }
    out
}

/// Pixel size of PNG / JPEG / GIF / BMP bytes.
pub fn image_size(b: &[u8]) -> Option<(u32, u32)> {
    if b.len() >= 24 && b.starts_with(b"\x89PNG\r\n\x1a\n") {
        let w = u32::from_be_bytes([b[16], b[17], b[18], b[19]]);
        let h = u32::from_be_bytes([b[20], b[21], b[22], b[23]]);
        return Some((w, h));
    }
    if b.len() >= 10 && (b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a")) {
        return Some((u16::from_le_bytes([b[6], b[7]]) as u32, u16::from_le_bytes([b[8], b[9]]) as u32));
    }
    if b.len() >= 26 && b.starts_with(b"BM") {
        let w = i32::from_le_bytes([b[18], b[19], b[20], b[21]]).unsigned_abs();
        let h = i32::from_le_bytes([b[22], b[23], b[24], b[25]]).unsigned_abs();
        return Some((w, h));
    }
    if b.len() >= 4 && b[0] == 0xFF && b[1] == 0xD8 {
        let mut i = 2usize;
        while i + 9 < b.len() {
            if b[i] != 0xFF {
                i += 1;
                continue;
            }
            let marker = b[i + 1];
            if marker == 0xD8 || marker == 0x01 || (0xD0..=0xD7).contains(&marker) || marker == 0xFF {
                i += if marker == 0xFF { 1 } else { 2 };
                continue;
            }
            let len = u16::from_be_bytes([b[i + 2], b[i + 3]]) as usize;
            let is_sof = matches!(marker, 0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF);
            if is_sof {
                let h = u16::from_be_bytes([b[i + 5], b[i + 6]]) as u32;
                let w = u16::from_be_bytes([b[i + 7], b[i + 8]]) as u32;
                return Some((w, h));
            }
            if len < 2 {
                return None;
            }
            i += 2 + len;
        }
    }
    None
}

fn is_cjk(c: char) -> bool {
    crate::latex_writer::is_cjk(c)
}

// ------------------------------------------------------------------ writer

#[derive(Clone, Default)]
struct Fmt {
    bold: bool,
    italic: bool,
    strike: bool,
    code: bool,
    link: bool,
    color: Option<String>,
    size_hp: Option<i64>,
}

struct Rel {
    id: String,
    kind: &'static str,
    target: String,
    external: bool,
}

struct Ctx<'a> {
    style: &'a Value,
    images: ImageLoader<'a>,
    body: String,
    rels: Vec<Rel>,
    media: Vec<(String, Vec<u8>)>,
    image_cache: HashMap<String, Option<(String, u32, u32)>>,
    bullet_num: u32,
    nums: Vec<(u32, u32, u64)>, // (numId, abstractId, start)
    next_num: u32,
    bookmark_id: u32,
    drawing_id: u32,
    footnote_defs: HashMap<String, Vec<Block>>,
    footnotes: Vec<(u32, String)>, // (id, xml of paragraphs)
    footnote_ids: HashMap<String, u32>,
    fn_stack: Vec<String>,
    warns: Vec<String>,
    text_w_emu: i64,
    text_h_emu: i64,
}

impl<'a> Ctx<'a> {
    fn rel(&mut self, kind: &'static str, target: String, external: bool) -> String {
        if let Some(r) = self.rels.iter().find(|r| r.kind == kind && r.target == target) {
            return r.id.clone();
        }
        let id = format!("rId{}", self.rels.len() + 1);
        self.rels.push(Rel { id: id.clone(), kind, target, external });
        id
    }
}

// ------------------------------------------------------------------ inlines

fn run_xml(t: &str, f: &Fmt) -> String {
    if t.is_empty() {
        return String::new();
    }
    let mut rpr = String::new();
    if f.code {
        rpr.push_str("<w:rStyle w:val=\"CodeChar\"/>");
    } else if f.link {
        rpr.push_str("<w:rStyle w:val=\"Hyperlink\"/>");
    }
    if f.bold {
        rpr.push_str("<w:b/><w:bCs/>");
    }
    if f.italic {
        rpr.push_str("<w:i/><w:iCs/>");
    }
    if f.strike {
        rpr.push_str("<w:strike/>");
    }
    if let Some(c) = &f.color {
        rpr.push_str(&format!("<w:color w:val=\"{c}\"/>"));
    }
    if let Some(sz) = f.size_hp {
        rpr.push_str(&format!("<w:sz w:val=\"{sz}\"/><w:szCs w:val=\"{sz}\"/>"));
    }
    let mut out = String::from("<w:r>");
    if !rpr.is_empty() {
        out.push_str("<w:rPr>");
        out.push_str(&rpr);
        out.push_str("</w:rPr>");
    }
    let mut first = true;
    for line in t.split('\n') {
        if !first {
            out.push_str("<w:br/>");
        }
        first = false;
        let mut seg_first = true;
        for seg in line.split('\t') {
            if !seg_first {
                out.push_str("<w:tab/>");
            }
            seg_first = false;
            if !seg.is_empty() {
                out.push_str("<w:t xml:space=\"preserve\">");
                out.push_str(&xml_escape(seg));
                out.push_str("</w:t>");
            }
        }
    }
    out.push_str("</w:r>");
    out
}

fn first_char(v: &[Inline]) -> Option<char> {
    for i in v {
        match i {
            Inline::Text(t) | Inline::Code(t) => return t.chars().next(),
            Inline::Strong(c) | Inline::Emph(c) | Inline::Strike(c) => {
                if let Some(ch) = first_char(c) {
                    return Some(ch);
                }
            }
            Inline::Link { children, .. } => return first_char(children),
            _ => return None,
        }
    }
    None
}

fn last_char(v: &[Inline]) -> Option<char> {
    for i in v.iter().rev() {
        match i {
            Inline::Text(t) | Inline::Code(t) => return t.chars().last(),
            Inline::Strong(c) | Inline::Emph(c) | Inline::Strike(c) => {
                if let Some(ch) = last_char(c) {
                    return Some(ch);
                }
            }
            Inline::Link { children, .. } => return last_char(children),
            _ => return None,
        }
    }
    None
}

impl<'a> Ctx<'a> {
    fn inlines(&mut self, v: &[Inline], f: &Fmt) -> String {
        let mut out = String::new();
        for (idx, i) in v.iter().enumerate() {
            match i {
                Inline::Text(t) => out.push_str(&run_xml(t, f)),
                Inline::Code(t) => {
                    let mut g = f.clone();
                    g.code = true;
                    out.push_str(&run_xml(t, &g));
                }
                Inline::Strong(c) => {
                    let mut g = f.clone();
                    g.bold = true;
                    out.push_str(&self.inlines(c, &g));
                }
                Inline::Emph(c) => {
                    let mut g = f.clone();
                    g.italic = true;
                    out.push_str(&self.inlines(c, &g));
                }
                Inline::Strike(c) => {
                    let mut g = f.clone();
                    g.strike = true;
                    out.push_str(&self.inlines(c, &g));
                }
                Inline::Link { href, children, .. } => {
                    let mut g = f.clone();
                    g.link = true;
                    let inner = self.inlines(children, &g);
                    let inner = if inner.is_empty() { run_xml(href, &g) } else { inner };
                    if let Some(anchor) = href.strip_prefix('#') {
                        let slug = slugify(&percent_encoding::percent_decode_str(anchor).decode_utf8_lossy());
                        out.push_str(&format!(
                            "<w:hyperlink w:anchor=\"{}\" w:history=\"1\">{inner}</w:hyperlink>",
                            bookmark_name(&slug)
                        ));
                    } else if href.is_empty() {
                        out.push_str(&inner);
                    } else {
                        let id = self.rel("hyperlink", xml_escape(href), true);
                        out.push_str(&format!("<w:hyperlink r:id=\"{id}\" w:history=\"1\">{inner}</w:hyperlink>"));
                    }
                }
                Inline::Image { src, alt, .. } => match self.drawing(src, alt, false) {
                    Some(x) => out.push_str(&x),
                    None => {
                        let mut g = f.clone();
                        g.italic = true;
                        out.push_str(&run_xml(&format!("[{}]", if alt.is_empty() { src } else { alt }), &g));
                    }
                },
                Inline::Math(m) => {
                    let m = m.trim();
                    if !m.is_empty() {
                        out.push_str(&crate::latex2omml::latex_to_omml(m, false));
                    }
                }
                Inline::SoftBreak => {
                    // Between two CJK characters a source line break is not a space.
                    let prev = last_char(&v[..idx]);
                    let next = first_char(&v[idx + 1..]);
                    let cjk = matches!((prev, next), (Some(a), Some(b)) if is_cjk(a) && is_cjk(b));
                    if !cjk {
                        out.push_str(&run_xml(" ", f));
                    }
                }
                Inline::HardBreak => out.push_str("<w:r><w:br/></w:r>"),
                Inline::FootnoteRef(label) => out.push_str(&self.footnote_ref(label, f)),
                Inline::Html(_) => {}
            }
        }
        out
    }

    fn footnote_ref(&mut self, label: &str, f: &Fmt) -> String {
        let Some(def) = self.footnote_defs.get(label).cloned() else {
            return run_xml(&format!("[{label}]"), f);
        };
        if self.fn_stack.iter().any(|l| l == label) {
            return String::new();
        }
        self.fn_stack.push(label.to_string());
        let id = self.footnotes.len() as u32 + 1;
        let mut xml = String::new();
        let mut first = true;
        for b in &def {
            // Footnotes hold paragraphs; other block kinds degrade to their text.
            let inner = match b {
                Block::Paragraph(inl) => self.inlines(inl, &Fmt::default()),
                Block::Code { text, .. } => run_xml(text, &Fmt { code: true, ..Fmt::default() }),
                Block::Math(m) => crate::latex2omml::latex_to_omml(m.trim(), false),
                Block::List { items, .. } => {
                    let mut s = String::new();
                    for it in items {
                        for ib in &it.blocks {
                            if let Block::Paragraph(inl) = ib {
                                s.push_str(&run_xml("• ", &Fmt::default()));
                                s.push_str(&self.inlines(inl, &Fmt::default()));
                                s.push_str("<w:r><w:br/></w:r>");
                            }
                        }
                    }
                    s
                }
                _ => continue,
            };
            let mark = if first {
                "<w:r><w:rPr><w:rStyle w:val=\"FootnoteReference\"/></w:rPr><w:footnoteRef/></w:r><w:r><w:t xml:space=\"preserve\"> </w:t></w:r>"
            } else {
                ""
            };
            first = false;
            xml.push_str(&format!("<w:p><w:pPr><w:pStyle w:val=\"FootnoteText\"/></w:pPr>{mark}{inner}</w:p>"));
        }
        if xml.is_empty() {
            xml.push_str("<w:p><w:pPr><w:pStyle w:val=\"FootnoteText\"/></w:pPr><w:r><w:rPr><w:rStyle w:val=\"FootnoteReference\"/></w:rPr><w:footnoteRef/></w:r></w:p>");
        }
        self.fn_stack.pop();
        self.footnotes.push((id, xml));
        self.footnote_ids.insert(label.to_string(), id);
        format!("<w:r><w:rPr><w:rStyle w:val=\"FootnoteReference\"/></w:rPr><w:footnoteReference w:id=\"{id}\"/></w:r>")
    }

    /// `<w:drawing>` for an image, or `None` if it cannot be embedded.
    fn drawing(&mut self, src: &str, alt: &str, block: bool) -> Option<String> {
        let entry = match self.image_cache.get(src) {
            Some(e) => e.clone(),
            None => {
                let loaded = (self.images)(src).and_then(|(ext, bytes)| {
                    let ext = ext.to_ascii_lowercase().replace("jpeg", "jpg");
                    if !matches!(ext.as_str(), "png" | "jpg" | "gif" | "bmp") {
                        self.warns.push(format!("DOCX 不支持该图片格式（{ext}），已跳过：{}", src.chars().take(80).collect::<String>()));
                        return None;
                    }
                    let (w, h) = image_size(&bytes).unwrap_or((600, 400));
                    let name = format!("image{}.{ext}", self.media.len() + 1);
                    self.media.push((name.clone(), bytes));
                    let rid = self.rel("image", format!("media/{name}"), false);
                    Some((rid, w.max(1), h.max(1)))
                });
                self.image_cache.insert(src.to_string(), loaded.clone());
                loaded
            }
        }?;
        let (rid, pw, ph) = entry;
        // 96 dpi → EMU, then fit into the configured share of the text block.
        let mut cx = pw as i64 * 9525;
        let mut cy = ph as i64 * 9525;
        let max_w = (self.text_w_emu as f64 * num(self.style, &["images", "widthPct"], 92.0) / 100.0) as i64;
        let max_h = (self.text_h_emu as f64 * num(self.style, &["images", "maxHeightPct"], 85.0) / 100.0) as i64;
        let _ = block;
        if cx > max_w && max_w > 0 {
            cy = cy * max_w / cx;
            cx = max_w;
        }
        if cy > max_h && max_h > 0 {
            cx = cx * max_h / cy;
            cy = max_h;
        }
        self.drawing_id += 1;
        let id = self.drawing_id;
        let descr = xml_escape(alt);
        Some(format!(
            concat!(
                "<w:r><w:drawing><wp:inline distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\">",
                "<wp:extent cx=\"{cx}\" cy=\"{cy}\"/><wp:docPr id=\"{id}\" name=\"Picture {id}\" descr=\"{descr}\"/>",
                "<wp:cNvGraphicFramePr><a:graphicFrameLocks xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" noChangeAspect=\"1\"/></wp:cNvGraphicFramePr>",
                "<a:graphic xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\">",
                "<a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/picture\">",
                "<pic:pic xmlns:pic=\"http://schemas.openxmlformats.org/drawingml/2006/picture\">",
                "<pic:nvPicPr><pic:cNvPr id=\"{id}\" name=\"Picture {id}\" descr=\"{descr}\"/><pic:cNvPicPr/></pic:nvPicPr>",
                "<pic:blipFill><a:blip r:embed=\"{rid}\"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill>",
                "<pic:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></pic:spPr>",
                "</pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>"
            ),
            cx = cx,
            cy = cy,
            id = id,
            descr = descr,
            rid = rid
        ))
    }
}

// ------------------------------------------------------------------- blocks

#[derive(Clone, Default)]
struct Scope {
    /// List nesting depth (indent level for continuation paragraphs).
    list_depth: usize,
    in_quote: bool,
}

impl<'a> Ctx<'a> {
    fn para(&mut self, ppr: &str, content: &str) {
        self.body.push_str("<w:p>");
        if !ppr.is_empty() {
            self.body.push_str("<w:pPr>");
            self.body.push_str(ppr);
            self.body.push_str("</w:pPr>");
        }
        self.body.push_str(content);
        self.body.push_str("</w:p>");
    }

    fn scope_ppr(&self, sc: &Scope) -> String {
        let mut p = String::new();
        if sc.in_quote {
            p.push_str("<w:pStyle w:val=\"Quote\"/>");
        }
        if sc.list_depth > 0 {
            let left = 720 * sc.list_depth as i64 + if sc.in_quote { 240 } else { 0 };
            p.push_str(&format!("<w:ind w:left=\"{left}\"/>"));
        }
        p
    }

    fn blocks(&mut self, blocks: &[Block], sc: &Scope) {
        for b in blocks {
            self.block(b, sc);
        }
    }

    fn block(&mut self, b: &Block, sc: &Scope) {
        match b {
            Block::Heading { level, id, inlines } => {
                let lvl = (*level).clamp(1, 6);
                self.bookmark_id += 1;
                let bid = self.bookmark_id;
                let runs = self.inlines(inlines, &Fmt::default());
                let content = format!(
                    "<w:bookmarkStart w:id=\"{bid}\" w:name=\"{}\"/>{runs}<w:bookmarkEnd w:id=\"{bid}\"/>",
                    bookmark_name(id)
                );
                self.para(&format!("<w:pStyle w:val=\"Heading{lvl}\"/>"), &content);
            }
            Block::Paragraph(inl) => {
                if let [Inline::Image { src, alt, .. }] = inl.as_slice() {
                    if let Some(d) = self.drawing(src, alt, true) {
                        let mut ppr = self.scope_ppr(sc);
                        ppr.push_str("<w:jc w:val=\"center\"/>");
                        self.para(&ppr, &d);
                        if !alt.trim().is_empty() {
                            let cap = run_xml(alt.trim(), &Fmt::default());
                            self.para("<w:pStyle w:val=\"Caption\"/>", &cap);
                        }
                        return;
                    }
                }
                let runs = self.inlines(inl, &Fmt::default());
                let ppr = self.scope_ppr(sc);
                self.para(&ppr, &runs);
            }
            Block::List { ordered, start, items } => self.list(*ordered, *start, items, sc),
            Block::Quote(inner) => {
                let mut s2 = sc.clone();
                s2.in_quote = true;
                self.blocks(inner, &s2);
            }
            Block::Code { lang, text } => {
                let mut ppr = String::from("<w:pStyle w:val=\"Code\"/>");
                if sc.list_depth > 0 {
                    ppr.push_str(&format!("<w:ind w:left=\"{}\"/>", 720 * sc.list_depth));
                }
                let mut content = String::new();
                if !lang.is_empty() {
                    content.push_str(&format!(
                        "<w:r><w:rPr><w:color w:val=\"8A8F98\"/><w:sz w:val=\"15\"/></w:rPr><w:t xml:space=\"preserve\">{}</w:t></w:r><w:r><w:br/></w:r>",
                        xml_escape(lang)
                    ));
                }
                content.push_str(&highlighted_runs(text, lang));
                self.para(&ppr, &content);
            }
            Block::Math(m) => {
                let m = m.trim();
                if m.is_empty() {
                    return;
                }
                let omml = crate::latex2omml::latex_to_omml(m, true);
                let mut ppr = self.scope_ppr(sc);
                ppr.push_str("<w:jc w:val=\"center\"/>");
                self.para(&ppr, &omml);
            }
            Block::Table { aligns, header, rows } => self.table(aligns, header, rows),
            Block::Hr => {
                let c = color(self.style, &["hr", "color"], "#d8dce2");
                self.para(
                    &format!("<w:pBdr><w:bottom w:val=\"single\" w:sz=\"6\" w:space=\"1\" w:color=\"{c}\"/></w:pBdr><w:spacing w:before=\"120\" w:after=\"120\"/>"),
                    "",
                );
            }
            Block::PageBreak => self.para("", "<w:r><w:br w:type=\"page\"/></w:r>"),
            Block::Html(_) => {}
            Block::FootnoteDef { .. } => {}
        }
    }

    fn numbering_for(&mut self, ordered: bool, start: u64, depth: usize) -> u32 {
        if !ordered {
            return self.bullet_num;
        }
        let id = self.next_num;
        self.next_num += 1;
        self.nums.push((id, depth as u32, start));
        id
    }

    fn list(&mut self, ordered: bool, start: u64, items: &[crate::md_ast::ListItem], sc: &Scope) {
        let depth = sc.list_depth.min(8);
        let num_id = self.numbering_for(ordered, start, depth);
        for it in items {
            let mut first = true;
            for blk in &it.blocks {
                if first {
                    first = false;
                    if let Block::Paragraph(inl) = blk {
                        let runs = self.inlines(inl, &Fmt::default());
                        let mut ppr = String::new();
                        if sc.in_quote {
                            ppr.push_str("<w:pStyle w:val=\"Quote\"/>");
                        } else {
                            ppr.push_str("<w:pStyle w:val=\"ListParagraph\"/>");
                        }
                        let content = match it.task {
                            Some(checked) => {
                                let left = 720 * (depth as i64 + 1);
                                ppr.push_str(&format!("<w:ind w:left=\"{left}\" w:hanging=\"360\"/>"));
                                let box_ = if checked { "\u{2611}" } else { "\u{2610}" };
                                format!(
                                    "<w:r><w:rPr><w:rFonts w:ascii=\"Segoe UI Symbol\" w:hAnsi=\"Segoe UI Symbol\" w:eastAsia=\"Segoe UI Symbol\"/></w:rPr><w:t xml:space=\"preserve\">{box_} </w:t></w:r>{runs}"
                                )
                            }
                            None => {
                                ppr.push_str(&format!("<w:numPr><w:ilvl w:val=\"{depth}\"/><w:numId w:val=\"{num_id}\"/></w:numPr>"));
                                runs
                            }
                        };
                        self.para(&ppr, &content);
                        continue;
                    }
                }
                let s2 = Scope { list_depth: depth + 1, in_quote: sc.in_quote };
                self.block(blk, &s2);
            }
        }
    }

    fn table(&mut self, aligns: &[Align], header: &[Vec<Inline>], rows: &[Vec<Vec<Inline>>]) {
        let st = self.style;
        let ncols = header.len().max(rows.iter().map(|r| r.len()).max().unwrap_or(0)).max(1);
        let border = color(st, &["table", "borderColor"], "#c8cdd4");
        let bw = ((num(st, &["table", "borderWidth"], 0.75) * 8.0).round() as i64).clamp(2, 96);
        let pad = twips(num(st, &["table", "cellPadding"], 6.0));
        let width_pct = num(st, &["table", "widthPct"], 100.0).clamp(10.0, 100.0);
        let head_bg = color(st, &["table", "headerBg"], "#3b6ef5");
        let head_fg = color(st, &["table", "headerColor"], "#ffffff");
        let head_bold = flag(st, &["table", "headerBold"], true);
        let banded = flag(st, &["table", "banded"], true);
        let band = color(st, &["table", "bandColor"], "#f3f5f9");
        let cell_hp = (num(st, &["table", "cellSize"], 10.0) * 2.0).round() as i64;
        let default_align = text(st, &["table", "align"], "left").to_string();
        let grid_w = (self.text_w_emu as f64 / 635.0 * width_pct / 100.0) as i64; // EMU → twips
        let col_w = (grid_w / ncols as i64).max(200);

        let mut x = String::new();
        x.push_str("<w:tbl><w:tblPr>");
        x.push_str(&format!("<w:tblW w:w=\"{}\" w:type=\"pct\"/>", (width_pct * 50.0).round() as i64));
        x.push_str("<w:jc w:val=\"center\"/>");
        let edge = |n: &str| format!("<w:{n} w:val=\"single\" w:sz=\"{bw}\" w:space=\"0\" w:color=\"{border}\"/>");
        x.push_str("<w:tblBorders>");
        for n in ["top", "left", "bottom", "right", "insideH", "insideV"] {
            x.push_str(&edge(n));
        }
        x.push_str("</w:tblBorders>");
        x.push_str("<w:tblLayout w:type=\"autofit\"/>");
        x.push_str(&format!(
            "<w:tblCellMar><w:top w:w=\"{p}\" w:type=\"dxa\"/><w:left w:w=\"{h}\" w:type=\"dxa\"/><w:bottom w:w=\"{p}\" w:type=\"dxa\"/><w:right w:w=\"{h}\" w:type=\"dxa\"/></w:tblCellMar>",
            p = pad / 2,
            h = pad
        ));
        x.push_str("<w:tblLook w:val=\"04A0\" w:firstRow=\"1\" w:lastRow=\"0\" w:firstColumn=\"0\" w:lastColumn=\"0\" w:noHBand=\"0\" w:noVBand=\"1\"/>");
        x.push_str("</w:tblPr><w:tblGrid>");
        for _ in 0..ncols {
            x.push_str(&format!("<w:gridCol w:w=\"{col_w}\"/>"));
        }
        x.push_str("</w:tblGrid>");

        let row_xml = |ctx: &mut Ctx, cells: &[Vec<Inline>], ri: usize, is_head: bool| -> String {
            let mut r = String::from("<w:tr>");
            if is_head {
                r.push_str("<w:trPr><w:tblHeader/><w:cantSplit/></w:trPr>");
            } else {
                r.push_str("<w:trPr><w:cantSplit/></w:trPr>");
            }
            for ci in 0..ncols {
                let align = match aligns.get(ci).copied().unwrap_or(Align::None) {
                    Align::None => default_align.clone(),
                    a => a.as_str().to_string(),
                };
                let fill = if is_head {
                    Some(head_bg.clone())
                } else if banded && ri % 2 == 1 {
                    Some(band.clone())
                } else {
                    None
                };
                let f = Fmt {
                    bold: is_head && head_bold,
                    color: if is_head { Some(head_fg.clone()) } else { None },
                    size_hp: Some(cell_hp),
                    ..Fmt::default()
                };
                let content = cells.get(ci).map(|c| ctx.inlines(c, &f)).unwrap_or_default();
                r.push_str("<w:tc><w:tcPr>");
                r.push_str(&format!("<w:tcW w:w=\"{col_w}\" w:type=\"dxa\"/>"));
                if let Some(fl) = fill {
                    r.push_str(&format!("<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"{fl}\"/>"));
                }
                r.push_str("<w:vAlign w:val=\"center\"/></w:tcPr>");
                r.push_str(&format!(
                    "<w:p><w:pPr><w:pStyle w:val=\"TableText\"/><w:jc w:val=\"{}\"/></w:pPr>{content}</w:p></w:tc>",
                    jc(&align)
                ));
            }
            r.push_str("</w:tr>");
            r
        };
        let h = row_xml(self, header, 0, true);
        x.push_str(&h);
        for (ri, r) in rows.iter().enumerate() {
            let t = row_xml(self, r, ri, false);
            x.push_str(&t);
        }
        x.push_str("</w:tbl>");
        self.body.push_str(&x);
        // Word needs a paragraph between adjacent tables and after a trailing one.
        self.para("<w:spacing w:before=\"0\" w:after=\"0\"/>", "");
    }
}

// --------------------------------------------------------- code highlighting

/// Runs for a code block, coloured by [`crate::code_highlight`].
fn highlighted_runs(code: &str, lang: &str) -> String {
    let mut out = String::new();
    for (kind, tok) in crate::code_highlight::tokenize(code, lang) {
        let f = Fmt { color: crate::code_highlight::color_of(kind).map(|c| c.to_string()), italic: kind == crate::code_highlight::Kind::Comment, ..Fmt::default() };
        out.push_str(&run_xml(tok, &f));
    }
    out
}

// ------------------------------------------------------------------- styles

fn styles_xml(st: &Value) -> String {
    let body_font = font_family(text(st, &["typography", "font"], "MicrosoftYaHei")).to_string();
    let code_font = font_family(text(st, &["code", "font"], "Consolas")).to_string();
    let body_hp = (num(st, &["typography", "size"], 11.0) * 2.0).round() as i64;
    let body_color = color(st, &["typography", "color"], "#262626");
    let line = (num(st, &["typography", "lineHeight"], 1.6) * 240.0).round() as i64;
    let after = twips(num(st, &["typography", "spacing"], 6.0));
    let body_jc = jc(text(st, &["typography", "align"], "left"));
    let code_hp = (num(st, &["code", "size"], 9.5) * 2.0).round() as i64;
    let code_bg = color(st, &["code", "bg"], "#f5f6f8");
    let code_fg = color(st, &["code", "color"], "#2f3b4a");
    let code_bd = color(st, &["code", "borderColor"], "#dfe3e8");
    let code_bw = ((num(st, &["code", "borderWidth"], 0.5) * 8.0).round() as i64).clamp(2, 48);
    let q_bar = color(st, &["quote", "barColor"], "#3b6ef5");
    let q_bg = color(st, &["quote", "bg"], "#f3f6ff");
    let q_fg = color(st, &["quote", "color"], "#4a5568");
    let link = color(st, &["link", "color"], "#2b6cb0");
    let cell_hp = (num(st, &["table", "cellSize"], 10.0) * 2.0).round() as i64;
    let fonts = |f: &str| format!("<w:rFonts w:ascii=\"{f}\" w:hAnsi=\"{f}\" w:eastAsia=\"{f}\" w:cs=\"{f}\"/>");

    let mut s = String::new();
    s.push_str(&format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:styles xmlns:w=\"{W_NS}\">"));
    s.push_str(&format!(
        "<w:docDefaults><w:rPrDefault><w:rPr>{}<w:sz w:val=\"{body_hp}\"/><w:szCs w:val=\"{body_hp}\"/><w:color w:val=\"{body_color}\"/><w:lang w:val=\"en-US\" w:eastAsia=\"zh-CN\"/></w:rPr></w:rPrDefault>\
<w:pPrDefault><w:pPr><w:spacing w:after=\"{after}\" w:line=\"{line}\" w:lineRule=\"auto\"/></w:pPr></w:pPrDefault></w:docDefaults>",
        fonts(&body_font)
    ));
    s.push_str(&format!(
        "<w:style w:type=\"paragraph\" w:default=\"1\" w:styleId=\"Normal\"><w:name w:val=\"Normal\"/><w:qFormat/><w:pPr><w:jc w:val=\"{body_jc}\"/></w:pPr></w:style>"
    ));
    for i in 1..=6 {
        let key = format!("h{i}");
        let h = |k: &str| dig(st, &["headings", key.as_str(), k]);
        let size_d = [20.0, 16.0, 14.0, 12.0, 11.0, 10.5][i - 1];
        let hp = (h("size").and_then(|v| v.as_f64()).unwrap_or(size_d) * 2.0).round() as i64;
        let c = color(st, &["headings", key.as_str(), "color"], "#1a1a1a");
        let bold = h("bold").and_then(|v| v.as_bool()).unwrap_or(true);
        let al = jc(h("align").and_then(|v| v.as_str()).unwrap_or("left"));
        let before = twips(h("before").and_then(|v| v.as_f64()).unwrap_or(12.0));
        let aft = twips(h("after").and_then(|v| v.as_f64()).unwrap_or(6.0));
        let pbb = if h("pageBreakBefore").and_then(|v| v.as_bool()).unwrap_or(false) { "<w:pageBreakBefore/>" } else { "" };
        s.push_str(&format!(
            "<w:style w:type=\"paragraph\" w:styleId=\"Heading{i}\"><w:name w:val=\"heading {i}\"/><w:basedOn w:val=\"Normal\"/><w:next w:val=\"Normal\"/><w:uiPriority w:val=\"9\"/><w:qFormat/>\
<w:pPr><w:keepNext/><w:keepLines/>{pbb}<w:spacing w:before=\"{before}\" w:after=\"{aft}\" w:line=\"276\" w:lineRule=\"auto\"/><w:jc w:val=\"{al}\"/><w:outlineLvl w:val=\"{}\"/></w:pPr>\
<w:rPr>{}<w:sz w:val=\"{hp}\"/><w:szCs w:val=\"{hp}\"/><w:color w:val=\"{c}\"/></w:rPr></w:style>",
            i - 1,
            if bold { "<w:b/><w:bCs/>" } else { "" }
        ));
    }
    s.push_str(&format!(
        "<w:style w:type=\"paragraph\" w:styleId=\"Title\"><w:name w:val=\"Title\"/><w:basedOn w:val=\"Normal\"/><w:qFormat/><w:pPr><w:spacing w:before=\"2400\" w:after=\"240\"/><w:jc w:val=\"center\"/></w:pPr><w:rPr><w:b/><w:sz w:val=\"56\"/><w:color w:val=\"{}\"/></w:rPr></w:style>",
        color(st, &["headings", "h1", "color"], "#1a1a1a")
    ));
    s.push_str("<w:style w:type=\"paragraph\" w:styleId=\"Subtitle\"><w:name w:val=\"Subtitle\"/><w:basedOn w:val=\"Normal\"/><w:qFormat/><w:pPr><w:spacing w:after=\"240\"/><w:jc w:val=\"center\"/></w:pPr><w:rPr><w:sz w:val=\"30\"/><w:color w:val=\"5C6470\"/></w:rPr></w:style>");
    s.push_str(&format!(
        "<w:style w:type=\"paragraph\" w:styleId=\"Code\"><w:name w:val=\"Code\"/><w:basedOn w:val=\"Normal\"/><w:qFormat/>\
<w:pPr><w:pBdr><w:top w:val=\"single\" w:sz=\"{code_bw}\" w:space=\"4\" w:color=\"{code_bd}\"/><w:left w:val=\"single\" w:sz=\"{code_bw}\" w:space=\"4\" w:color=\"{code_bd}\"/><w:bottom w:val=\"single\" w:sz=\"{code_bw}\" w:space=\"4\" w:color=\"{code_bd}\"/><w:right w:val=\"single\" w:sz=\"{code_bw}\" w:space=\"4\" w:color=\"{code_bd}\"/></w:pBdr>\
<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"{code_bg}\"/><w:spacing w:before=\"120\" w:after=\"160\" w:line=\"260\" w:lineRule=\"auto\"/><w:ind w:left=\"120\" w:right=\"120\"/><w:jc w:val=\"left\"/></w:pPr>\
<w:rPr><w:rFonts w:ascii=\"{code_font}\" w:hAnsi=\"{code_font}\" w:eastAsia=\"{body_font}\" w:cs=\"{code_font}\"/><w:noProof/><w:sz w:val=\"{code_hp}\"/><w:szCs w:val=\"{code_hp}\"/><w:color w:val=\"{code_fg}\"/></w:rPr></w:style>"
    ));
    s.push_str(&format!(
        "<w:style w:type=\"character\" w:styleId=\"CodeChar\"><w:name w:val=\"Inline Code\"/><w:rPr><w:rFonts w:ascii=\"{code_font}\" w:hAnsi=\"{code_font}\" w:eastAsia=\"{body_font}\" w:cs=\"{code_font}\"/><w:noProof/><w:color w:val=\"{code_fg}\"/><w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"{code_bg}\"/></w:rPr></w:style>"
    ));
    s.push_str(&format!(
        "<w:style w:type=\"paragraph\" w:styleId=\"Quote\"><w:name w:val=\"Quote\"/><w:basedOn w:val=\"Normal\"/><w:qFormat/>\
<w:pPr><w:pBdr><w:left w:val=\"single\" w:sz=\"24\" w:space=\"8\" w:color=\"{q_bar}\"/></w:pBdr><w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"{q_bg}\"/><w:ind w:left=\"240\" w:right=\"120\"/></w:pPr>\
<w:rPr><w:color w:val=\"{q_fg}\"/></w:rPr></w:style>"
    ));
    s.push_str(&format!(
        "<w:style w:type=\"character\" w:styleId=\"Hyperlink\"><w:name w:val=\"Hyperlink\"/><w:uiPriority w:val=\"99\"/><w:unhideWhenUsed/><w:rPr><w:color w:val=\"{link}\"/><w:u w:val=\"single\"/></w:rPr></w:style>"
    ));
    s.push_str("<w:style w:type=\"paragraph\" w:styleId=\"ListParagraph\"><w:name w:val=\"List Paragraph\"/><w:basedOn w:val=\"Normal\"/><w:qFormat/><w:pPr><w:spacing w:after=\"60\"/><w:contextualSpacing/></w:pPr></w:style>");
    s.push_str(&format!(
        "<w:style w:type=\"paragraph\" w:styleId=\"TableText\"><w:name w:val=\"Table Text\"/><w:basedOn w:val=\"Normal\"/><w:pPr><w:spacing w:before=\"0\" w:after=\"0\" w:line=\"264\" w:lineRule=\"auto\"/></w:pPr><w:rPr><w:sz w:val=\"{cell_hp}\"/><w:szCs w:val=\"{cell_hp}\"/></w:rPr></w:style>"
    ));
    s.push_str("<w:style w:type=\"paragraph\" w:styleId=\"Caption\"><w:name w:val=\"caption\"/><w:basedOn w:val=\"Normal\"/><w:next w:val=\"Normal\"/><w:qFormat/><w:pPr><w:spacing w:before=\"60\" w:after=\"200\"/><w:jc w:val=\"center\"/></w:pPr><w:rPr><w:i/><w:sz w:val=\"18\"/><w:color w:val=\"5C6470\"/></w:rPr></w:style>");
    s.push_str("<w:style w:type=\"paragraph\" w:styleId=\"FootnoteText\"><w:name w:val=\"footnote text\"/><w:basedOn w:val=\"Normal\"/><w:pPr><w:spacing w:after=\"0\" w:line=\"240\" w:lineRule=\"auto\"/></w:pPr><w:rPr><w:sz w:val=\"18\"/><w:szCs w:val=\"18\"/></w:rPr></w:style>");
    s.push_str("<w:style w:type=\"character\" w:styleId=\"FootnoteReference\"><w:name w:val=\"footnote reference\"/><w:rPr><w:vertAlign w:val=\"superscript\"/></w:rPr></w:style>");
    s.push_str("<w:style w:type=\"paragraph\" w:styleId=\"TOCHeading\"><w:name w:val=\"TOC Heading\"/><w:basedOn w:val=\"Heading1\"/><w:next w:val=\"Normal\"/><w:pPr><w:outlineLvl w:val=\"9\"/></w:pPr></w:style>");
    for i in 1..=3 {
        s.push_str(&format!(
            "<w:style w:type=\"paragraph\" w:styleId=\"TOC{i}\"><w:name w:val=\"toc {i}\"/><w:basedOn w:val=\"Normal\"/><w:next w:val=\"Normal\"/><w:uiPriority w:val=\"39\"/><w:pPr><w:spacing w:after=\"60\"/><w:ind w:left=\"{}\"/></w:pPr></w:style>",
            (i - 1) * 240
        ));
    }
    s.push_str("<w:style w:type=\"paragraph\" w:styleId=\"Header\"><w:name w:val=\"header\"/><w:basedOn w:val=\"Normal\"/><w:pPr><w:spacing w:after=\"0\"/></w:pPr><w:rPr><w:sz w:val=\"18\"/><w:color w:val=\"808080\"/></w:rPr></w:style>");
    s.push_str("<w:style w:type=\"paragraph\" w:styleId=\"Footer\"><w:name w:val=\"footer\"/><w:basedOn w:val=\"Normal\"/><w:pPr><w:spacing w:after=\"0\"/></w:pPr><w:rPr><w:sz w:val=\"18\"/><w:color w:val=\"808080\"/></w:rPr></w:style>");
    s.push_str("</w:styles>");
    s
}

fn numbering_xml(nums: &[(u32, u32, u64)], bullet_num: u32) -> String {
    const BULLETS: [&str; 3] = ["\u{2022}", "\u{25E6}", "\u{25AA}"];
    let mut s = format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:numbering xmlns:w=\"{W_NS}\">");
    // abstract 0: bullets
    s.push_str("<w:abstractNum w:abstractNumId=\"0\"><w:multiLevelType w:val=\"hybridMultilevel\"/>");
    for lvl in 0..9 {
        let b = BULLETS[lvl % 3];
        s.push_str(&format!(
            "<w:lvl w:ilvl=\"{lvl}\"><w:start w:val=\"1\"/><w:numFmt w:val=\"bullet\"/><w:lvlText w:val=\"{b}\"/><w:lvlJc w:val=\"left\"/><w:pPr><w:ind w:left=\"{}\" w:hanging=\"360\"/></w:pPr></w:lvl>",
            720 * (lvl + 1)
        ));
    }
    s.push_str("</w:abstractNum>");
    // abstract 1: decimal / lower-letter / lower-roman cycle
    s.push_str("<w:abstractNum w:abstractNumId=\"1\"><w:multiLevelType w:val=\"hybridMultilevel\"/>");
    for lvl in 0..9 {
        let fmt = ["decimal", "lowerLetter", "lowerRoman"][lvl % 3];
        s.push_str(&format!(
            "<w:lvl w:ilvl=\"{lvl}\"><w:start w:val=\"1\"/><w:numFmt w:val=\"{fmt}\"/><w:lvlText w:val=\"%{}.\"/><w:lvlJc w:val=\"left\"/><w:pPr><w:ind w:left=\"{}\" w:hanging=\"360\"/></w:pPr></w:lvl>",
            lvl + 1,
            720 * (lvl + 1)
        ));
    }
    s.push_str("</w:abstractNum>");
    s.push_str(&format!("<w:num w:numId=\"{bullet_num}\"><w:abstractNumId w:val=\"0\"/></w:num>"));
    for (id, lvl, start) in nums {
        s.push_str(&format!(
            "<w:num w:numId=\"{id}\"><w:abstractNumId w:val=\"1\"/><w:lvlOverride w:ilvl=\"{lvl}\"><w:startOverride w:val=\"{start}\"/></w:lvlOverride></w:num>"
        ));
    }
    s.push_str("</w:numbering>");
    s
}

// ------------------------------------------------------------------ package

fn collect_footnote_defs(blocks: &[Block], into: &mut HashMap<String, Vec<Block>>) {
    for b in blocks {
        match b {
            Block::FootnoteDef { label, blocks } => {
                into.insert(label.clone(), blocks.clone());
            }
            Block::Quote(inner) => collect_footnote_defs(inner, into),
            Block::List { items, .. } => {
                for it in items {
                    collect_footnote_defs(&it.blocks, into);
                }
            }
            _ => {}
        }
    }
}

/// Document title used for `core.xml` / the cover page.
pub fn doc_title(doc: &Document, st: &Value, fallback: &str) -> String {
    let t = text(st, &["meta", "title"], "").trim();
    if !t.is_empty() {
        return t.to_string();
    }
    if let Some(t) = doc.meta("title") {
        return t;
    }
    for b in &doc.blocks {
        if let Block::Heading { level: 1, inlines, .. } = b {
            let t = inline_plain(inlines);
            if !t.trim().is_empty() {
                return t.trim().to_string();
            }
        }
    }
    fallback.to_string()
}

fn header_footer(tag: &str, style_id: &str, content: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:{tag} xmlns:w=\"{W_NS}\" xmlns:r=\"{R_NS}\"><w:p><w:pPr><w:pStyle w:val=\"{style_id}\"/>{content}</w:{tag}>"
    )
}

/// Build every part of the `.docx` package.
pub fn build(doc: &Document, style: &Value, source_name: &str, images: ImageLoader) -> DocxPackage {
    let (page_w, page_h) = crate::export_styles::page_dimensions(style);
    let (mt, mr, mb, ml) = (
        num(style, &["page", "marginTop"], 20.0),
        num(style, &["page", "marginRight"], 18.0),
        num(style, &["page", "marginBottom"], 20.0),
        num(style, &["page", "marginLeft"], 18.0),
    );
    let text_w_emu = (((page_w - ml - mr).max(20.0)) * 36000.0) as i64;
    let text_h_emu = (((page_h - mt - mb).max(20.0)) * 36000.0) as i64;

    let mut footnote_defs = HashMap::new();
    collect_footnote_defs(&doc.blocks, &mut footnote_defs);

    let mut ctx = Ctx {
        style,
        images,
        body: String::new(),
        rels: Vec::new(),
        media: Vec::new(),
        image_cache: HashMap::new(),
        bullet_num: 1,
        nums: Vec::new(),
        next_num: 2,
        bookmark_id: 0,
        drawing_id: 0,
        footnote_defs,
        footnotes: Vec::new(),
        footnote_ids: HashMap::new(),
        fn_stack: Vec::new(),
        warns: Vec::new(),
        text_w_emu,
        text_h_emu,
    };
    // Fixed relationships first so their ids are stable.
    ctx.rel("styles", "styles.xml".into(), false);
    ctx.rel("numbering", "numbering.xml".into(), false);
    ctx.rel("settings", "settings.xml".into(), false);
    ctx.rel("footnotes", "footnotes.xml".into(), false);

    let title = doc_title(doc, style, source_name);

    // Cover page.
    let cover = flag(style, &["cover", "enabled"], false);
    if cover {
        let ct = text(style, &["cover", "title"], "").trim();
        let ct = if ct.is_empty() { title.as_str() } else { ct };
        let al = jc(text(style, &["cover", "align"], "center"));
        ctx.para(&format!("<w:pStyle w:val=\"Title\"/><w:jc w:val=\"{al}\"/>"), &run_xml(ct, &Fmt::default()));
        let sub = text(style, &["cover", "subtitle"], "").trim().to_string();
        if !sub.is_empty() {
            ctx.para(&format!("<w:pStyle w:val=\"Subtitle\"/><w:jc w:val=\"{al}\"/>"), &run_xml(&sub, &Fmt::default()));
        }
        let author = text(style, &["meta", "author"], "").trim().to_string();
        if !author.is_empty() {
            ctx.para(&format!("<w:pStyle w:val=\"Subtitle\"/><w:jc w:val=\"{al}\"/>"), &run_xml(&author, &Fmt::default()));
        }
        let date = text(style, &["cover", "date"], "").trim().to_string();
        if !date.is_empty() {
            ctx.para(&format!("<w:pStyle w:val=\"Subtitle\"/><w:jc w:val=\"{al}\"/>"), &run_xml(&date, &Fmt::default()));
        }
        ctx.para("", "<w:r><w:br w:type=\"page\"/></w:r>");
    }

    // Table of contents (a TOC field; Word fills it on open / F9).
    let toc = flag(style, &["toc", "enabled"], false);
    if toc {
        ctx.para("<w:pStyle w:val=\"TOCHeading\"/>", &run_xml("目录", &Fmt::default()));
        ctx.body.push_str(concat!(
            "<w:p><w:r><w:fldChar w:fldCharType=\"begin\" w:dirty=\"true\"/></w:r>",
            "<w:r><w:instrText xml:space=\"preserve\"> TOC \\o \"1-3\" \\h \\z \\u </w:instrText></w:r>",
            "<w:r><w:fldChar w:fldCharType=\"separate\"/></w:r>",
            "<w:r><w:rPr><w:color w:val=\"808080\"/></w:rPr><w:t>打开文档后按 F9（或右键“更新域”）生成目录。</w:t></w:r>",
            "<w:r><w:fldChar w:fldCharType=\"end\"/></w:r></w:p>",
            "<w:p><w:r><w:br w:type=\"page\"/></w:r></w:p>"
        ));
    }

    ctx.blocks(&doc.blocks, &Scope::default());
    if ctx.body.is_empty() {
        ctx.para("", "");
    }

    // Header / footer.
    let header_text = text(style, &["header", "text"], "").trim().to_string();
    let footer_text = text(style, &["footer", "text"], "").trim().to_string();
    let page_numbers = flag(style, &["footer", "pageNumbers"], true);
    let mut parts: Vec<(String, Vec<u8>)> = Vec::new();
    let mut sect_refs = String::new();
    if !header_text.is_empty() {
        let al = jc(text(style, &["header", "align"], "left"));
        let x = header_footer(
            "hdr",
            "Header",
            &format!("<w:jc w:val=\"{al}\"/></w:pPr>{}</w:p>", run_xml(&header_text, &Fmt::default())),
        );
        parts.push(("word/header1.xml".into(), x.into_bytes()));
        let id = ctx.rel("header", "header1.xml".into(), false);
        sect_refs.push_str(&format!("<w:headerReference w:type=\"default\" r:id=\"{id}\"/>"));
    }
    if !footer_text.is_empty() || page_numbers {
        let mut runs = String::new();
        if !footer_text.is_empty() {
            runs.push_str(&run_xml(&footer_text, &Fmt::default()));
            if page_numbers {
                runs.push_str(&run_xml("    ", &Fmt::default()));
            }
        }
        if page_numbers {
            runs.push_str(concat!(
                "<w:r><w:fldChar w:fldCharType=\"begin\"/></w:r>",
                "<w:r><w:instrText xml:space=\"preserve\"> PAGE </w:instrText></w:r>",
                "<w:r><w:fldChar w:fldCharType=\"separate\"/></w:r><w:r><w:t>1</w:t></w:r>",
                "<w:r><w:fldChar w:fldCharType=\"end\"/></w:r>"
            ));
        }
        let x = header_footer("ftr", "Footer", &format!("<w:jc w:val=\"center\"/></w:pPr>{runs}</w:p>"));
        parts.push(("word/footer1.xml".into(), x.into_bytes()));
        let id = ctx.rel("footer", "footer1.xml".into(), false);
        sect_refs.push_str(&format!("<w:footerReference w:type=\"default\" r:id=\"{id}\"/>"));
    }

    let (w_tw, h_tw) = (mm_twips(page_w), mm_twips(page_h));
    let orient = if page_w > page_h { " w:orient=\"landscape\"" } else { "" };
    let sect = format!(
        "<w:sectPr>{sect_refs}<w:pgSz w:w=\"{w_tw}\" w:h=\"{h_tw}\"{orient}/><w:pgMar w:top=\"{}\" w:right=\"{}\" w:bottom=\"{}\" w:left=\"{}\" w:header=\"567\" w:footer=\"567\" w:gutter=\"0\"/>{}<w:docGrid w:linePitch=\"360\"/></w:sectPr>",
        mm_twips(mt),
        mm_twips(mr),
        mm_twips(mb),
        mm_twips(ml),
        if cover { "<w:titlePg/>" } else { "" }
    );
    let document = format!(
        concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n",
            "<w:document xmlns:w=\"{w}\" xmlns:r=\"{r}\" ",
            "xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" ",
            "xmlns:wp=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing\" ",
            "xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" ",
            "xmlns:pic=\"http://schemas.openxmlformats.org/drawingml/2006/picture\">",
            "<w:body>{body}{sect}</w:body></w:document>"
        ),
        w = W_NS,
        r = R_NS,
        body = ctx.body,
        sect = sect
    );

    // Footnotes (separators are mandatory once the part exists).
    let mut fx = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:footnotes xmlns:w=\"{W_NS}\" xmlns:r=\"{R_NS}\" xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\">\
<w:footnote w:type=\"separator\" w:id=\"-1\"><w:p><w:pPr><w:spacing w:after=\"0\" w:line=\"240\" w:lineRule=\"auto\"/></w:pPr><w:r><w:separator/></w:r></w:p></w:footnote>\
<w:footnote w:type=\"continuationSeparator\" w:id=\"0\"><w:p><w:pPr><w:spacing w:after=\"0\" w:line=\"240\" w:lineRule=\"auto\"/></w:pPr><w:r><w:continuationSeparator/></w:r></w:p></w:footnote>"
    );
    for (id, xml) in &ctx.footnotes {
        fx.push_str(&format!("<w:footnote w:id=\"{id}\">{xml}</w:footnote>"));
    }
    fx.push_str("</w:footnotes>");

    let settings = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:settings xmlns:w=\"{W_NS}\">{}<w:defaultTabStop w:val=\"420\"/><w:characterSpacingControl w:val=\"compressPunctuation\"/>\
<w:footnotePr><w:footnote w:id=\"-1\"/><w:footnote w:id=\"0\"/></w:footnotePr>\
<w:compat><w:compatSetting w:name=\"compatibilityMode\" w:uri=\"http://schemas.microsoft.com/office/word\" w:val=\"15\"/></w:compat></w:settings>",
        if toc { "<w:updateFields w:val=\"true\"/>" } else { "" }
    );

    // Relationships.
    let mut rels = format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"{REL_NS}\">");
    for r in &ctx.rels {
        let ty = format!("http://schemas.openxmlformats.org/officeDocument/2006/relationships/{}", r.kind);
        let mode = if r.external { " TargetMode=\"External\"" } else { "" };
        rels.push_str(&format!("<Relationship Id=\"{}\" Type=\"{ty}\" Target=\"{}\"{mode}/>", r.id, r.target));
    }
    rels.push_str("</Relationships>");

    let mut ct = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">");
    ct.push_str("<Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/>");
    for (ext, mime) in [("png", "image/png"), ("jpg", "image/jpeg"), ("gif", "image/gif"), ("bmp", "image/bmp")] {
        ct.push_str(&format!("<Default Extension=\"{ext}\" ContentType=\"{mime}\"/>"));
    }
    let wml = "application/vnd.openxmlformats-officedocument.wordprocessingml";
    for (part, kind) in [
        ("/word/document.xml", "document.main+xml"),
        ("/word/styles.xml", "styles+xml"),
        ("/word/numbering.xml", "numbering+xml"),
        ("/word/settings.xml", "settings+xml"),
        ("/word/footnotes.xml", "footnotes+xml"),
    ] {
        ct.push_str(&format!("<Override PartName=\"{part}\" ContentType=\"{wml}.{kind}\"/>"));
    }
    if parts.iter().any(|(p, _)| p == "word/header1.xml") {
        ct.push_str(&format!("<Override PartName=\"/word/header1.xml\" ContentType=\"{wml}.header+xml\"/>"));
    }
    if parts.iter().any(|(p, _)| p == "word/footer1.xml") {
        ct.push_str(&format!("<Override PartName=\"/word/footer1.xml\" ContentType=\"{wml}.footer+xml\"/>"));
    }
    ct.push_str("<Override PartName=\"/docProps/core.xml\" ContentType=\"application/vnd.openxmlformats-package.core-properties+xml\"/>");
    ct.push_str("<Override PartName=\"/docProps/app.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.extended-properties+xml\"/>");
    ct.push_str("</Types>");

    let root_rels = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"{REL_NS}\">\
<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"word/document.xml\"/>\
<Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties\" Target=\"docProps/core.xml\"/>\
<Relationship Id=\"rId3\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties\" Target=\"docProps/app.xml\"/></Relationships>"
    );
    let now = time::OffsetDateTime::now_utc();
    let stamp = format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        now.year(),
        now.month() as u8,
        now.day(),
        now.hour(),
        now.minute(),
        now.second()
    );
    let core = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:dcterms=\"http://purl.org/dc/terms/\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\
<dc:title>{}</dc:title><dc:subject>{}</dc:subject><dc:creator>{}</dc:creator>\
<dcterms:created xsi:type=\"dcterms:W3CDTF\">{stamp}</dcterms:created><dcterms:modified xsi:type=\"dcterms:W3CDTF\">{stamp}</dcterms:modified></cp:coreProperties>",
        xml_escape(&title),
        xml_escape(text(style, &["meta", "subject"], "")),
        xml_escape(text(style, &["meta", "author"], ""))
    );
    let app = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Properties xmlns=\"http://schemas.openxmlformats.org/officeDocument/2006/extended-properties\"><Application>ReadMD</Application></Properties>";

    let mut all: Vec<(String, Vec<u8>)> = vec![
        ("[Content_Types].xml".into(), ct.into_bytes()),
        ("_rels/.rels".into(), root_rels.into_bytes()),
        ("docProps/core.xml".into(), core.into_bytes()),
        ("docProps/app.xml".into(), app.as_bytes().to_vec()),
        ("word/document.xml".into(), document.into_bytes()),
        ("word/styles.xml".into(), styles_xml(style).into_bytes()),
        ("word/numbering.xml".into(), numbering_xml(&ctx.nums, ctx.bullet_num).into_bytes()),
        ("word/settings.xml".into(), settings.into_bytes()),
        ("word/footnotes.xml".into(), fx.into_bytes()),
        ("word/_rels/document.xml.rels".into(), rels.into_bytes()),
    ];
    all.extend(parts);
    for (name, bytes) in ctx.media {
        all.push((format!("word/media/{name}"), bytes));
    }
    DocxPackage { parts: all, warns: ctx.warns }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::md_ast::parse;

    const RICH: &str = "---\ntitle: 报告\n---\n\n# 第一章\n\n##### 五级\n\n正文 *斜体* **粗体** ~~删除~~ `代码` [外链](https://example.com?a=1&b=2) 与 [内链](#第一章)。\n\n1. 一\n2. 二\n   - 嵌套\n   - [x] 完成\n\n> 引用\n\n| 左 | 中 | 右 |\n|:--|:-:|--:|\n| a | b | c |\n\n```rust\nfn main() {}\n```\n\n$$\\frac{a}{b}$$\n\n行内 $x^2$ 公式[^n]。\n\n![图](a.png)\n\n---\n\n[^n]: 脚注内容\n";

    fn png_1x1() -> Vec<u8> {
        let mut b = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        b.extend_from_slice(&200u32.to_be_bytes());
        b.extend_from_slice(&100u32.to_be_bytes());
        b.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
        b
    }

    fn part<'p>(pkg: &'p DocxPackage, name: &str) -> &'p str {
        let (_, b) = pkg.parts.iter().find(|(n, _)| n == name).unwrap_or_else(|| panic!("missing {name}"));
        std::str::from_utf8(b).unwrap()
    }

    fn build_rich(style: &Value) -> DocxPackage {
        let img = png_1x1();
        let loader = move |src: &str| if src == "a.png" { Some(("png".to_string(), img.clone())) } else { None };
        build(&parse(RICH), style, "doc", &loader)
    }

    #[test]
    fn structure_is_preserved() {
        let st = crate::export_styles::sanitize_options(None);
        let pkg = build_rich(&st);
        let d = part(&pkg, "word/document.xml");
        assert!(d.contains("<w:pStyle w:val=\"Heading1\"/>"));
        assert!(d.contains("<w:pStyle w:val=\"Heading5\"/>"));
        assert!(d.contains("<w:i/>") && d.contains("<w:b/>") && d.contains("<w:strike/>"));
        assert!(d.contains("<w:rStyle w:val=\"CodeChar\"/>"));
        assert!(d.contains("<w:hyperlink r:id="), "external link");
        assert!(d.contains(&format!("w:anchor=\"{}\"", bookmark_name("第一章"))), "internal link");
        assert!(d.contains(&format!("w:name=\"{}\"", bookmark_name("第一章"))), "bookmark");
        assert!(d.matches("<w:numPr>").count() >= 3, "list numbering");
        assert!(d.contains("\u{2611}"), "task checkbox");
        assert!(d.contains("<w:pStyle w:val=\"Quote\"/>"));
        assert!(d.contains("<w:jc w:val=\"left\"/>") && d.contains("<w:jc w:val=\"center\"/>") && d.contains("<w:jc w:val=\"right\"/>"));
        assert!(d.contains("<w:tblHeader/>"));
        assert!(d.contains("<w:pStyle w:val=\"Code\"/>") && d.contains(">rust<"));
        assert!(d.contains("<m:oMathPara") && d.contains("<m:oMath"));
        assert!(d.contains("<w:footnoteReference w:id=\"1\"/>"));
        assert!(d.contains("<w:drawing>"));
        assert!(d.contains("<w:pBdr><w:bottom"), "hr");
        assert!(part(&pkg, "word/footnotes.xml").contains("脚注内容"));
        assert!(part(&pkg, "word/_rels/document.xml.rels").contains("https://example.com?a=1&amp;b=2"));
        assert!(pkg.parts.iter().any(|(n, _)| n == "word/media/image1.png"));
        assert!(part(&pkg, "docProps/core.xml").contains("<dc:title>报告</dc:title>"));
        // ordered list keeps a numbering instance with its start
        assert!(part(&pkg, "word/numbering.xml").contains("<w:startOverride w:val=\"1\"/>"));
    }

    #[test]
    fn style_options_are_applied() {
        let st = crate::export_styles::sanitize_options(Some(&serde_json::json!({
            "page": {"size": "A4", "orientation": "landscape", "marginTop": 25},
            "typography": {"font": "SimSun", "size": 12, "color": "#112233"},
            "headings": {"h1": {"size": 24, "color": "#ff0000", "align": "center"}},
            "table": {"headerBg": "#00ff00"},
            "header": {"text": "页眉"},
            "footer": {"text": "页脚", "pageNumbers": true},
            "cover": {"enabled": true, "subtitle": "副标题"},
            "toc": {"enabled": true},
        })));
        let pkg = build_rich(&st);
        let d = part(&pkg, "word/document.xml");
        assert!(d.contains("w:orient=\"landscape\""));
        assert!(d.contains(&format!("w:top=\"{}\"", mm_twips(25.0))));
        assert!(d.contains("w:fill=\"00FF00\""));
        assert!(d.contains("TOC \\o"));
        assert!(d.contains("<w:pStyle w:val=\"Title\"/>") && d.contains("副标题"));
        assert!(d.contains("<w:titlePg/>"));
        let s = part(&pkg, "word/styles.xml");
        assert!(s.contains("w:ascii=\"SimSun\""));
        assert!(s.contains("<w:sz w:val=\"24\"/>"));
        assert!(s.contains("<w:color w:val=\"112233\"/>"));
        assert!(s.contains("<w:sz w:val=\"48\"/><w:szCs w:val=\"48\"/><w:color w:val=\"FF0000\"/>"));
        assert!(part(&pkg, "word/header1.xml").contains("页眉"));
        let f = part(&pkg, "word/footer1.xml");
        assert!(f.contains("页脚") && f.contains(" PAGE "));
    }

    #[test]
    fn xml_escape_drops_control_chars() {
        assert_eq!(xml_escape("a\u{1}b<&>\"\u{b}"), "ab&lt;&amp;&gt;&quot;");
    }

    #[test]
    fn image_sizes() {
        assert_eq!(image_size(&png_1x1()), Some((200, 100)));
        let gif = b"GIF89a\x10\x00\x20\x00".to_vec();
        assert_eq!(image_size(&gif), Some((16, 32)));
    }

    /// Writes a sample `.docx` for manual inspection when
    /// `READMD_DOCX_SAMPLE=<path>` is set.
    #[test]
    fn write_sample_when_requested() {
        let Ok(path) = std::env::var("READMD_DOCX_SAMPLE") else { return };
        let opts: Value = std::env::var("READMD_DOCX_SAMPLE_OPTS")
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| serde_json::json!({"header": {"text": "ReadMD"}}));
        let st = crate::export_styles::sanitize_options(Some(&opts));
        let pkg = build_rich(&st);
        let entries: Vec<crate::mdexport::ZipEntry> = pkg
            .parts
            .into_iter()
            .map(|(path, data)| crate::mdexport::ZipEntry { path, data, compress: true })
            .collect();
        std::fs::write(path, crate::mdexport::write_zip(&entries).unwrap()).unwrap();
    }
}
