//! Shared, typed Markdown AST for the document exporters (DOCX, LaTeX, PDF).
//!
//! The older `mdexport::md_parse` is a line-oriented port of the retired Python
//! parser: it drops fence languages, table alignment, task markers, nesting and
//! ordered-list starts.  This module parses with `pulldown-cmark` (GFM tables,
//! strikethrough, task lists, footnotes, `$`/`$$` math, YAML front matter) into
//! an owned tree that every exporter walks, so all formats agree on structure.

use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use serde_json::{json, Value};

/// Column alignment of a GFM table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    None,
    Left,
    Center,
    Right,
}

impl Align {
    pub fn as_str(self) -> &'static str {
        match self {
            Align::None => "",
            Align::Left => "left",
            Align::Center => "center",
            Align::Right => "right",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Inline {
    Text(String),
    Code(String),
    Strong(Vec<Inline>),
    Emph(Vec<Inline>),
    Strike(Vec<Inline>),
    Link { href: String, title: String, children: Vec<Inline> },
    Image { src: String, alt: String, title: String },
    Math(String),
    SoftBreak,
    HardBreak,
    FootnoteRef(String),
    Html(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ListItem {
    /// `Some(checked)` for a GFM task item.
    pub task: Option<bool>,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Heading { level: u8, id: String, inlines: Vec<Inline> },
    Paragraph(Vec<Inline>),
    List { ordered: bool, start: u64, items: Vec<ListItem> },
    Quote(Vec<Block>),
    Code { lang: String, text: String },
    Math(String),
    Table { aligns: Vec<Align>, header: Vec<Vec<Inline>>, rows: Vec<Vec<Vec<Inline>>> },
    Hr,
    PageBreak,
    Html(String),
    FootnoteDef { label: String, blocks: Vec<Block> },
}

/// A parsed document: blocks plus the raw YAML front matter (if any).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Document {
    pub front_matter: Option<String>,
    pub blocks: Vec<Block>,
}

impl Document {
    /// `title:` from the front matter, trimmed of quotes.
    pub fn meta(&self, key: &str) -> Option<String> {
        let fm = self.front_matter.as_deref()?;
        for line in fm.lines() {
            if let Some((k, v)) = line.split_once(':') {
                if k.trim().eq_ignore_ascii_case(key) {
                    let v = v.trim().trim_matches(|c| c == '"' || c == '\'').trim();
                    if !v.is_empty() {
                        return Some(v.to_string());
                    }
                }
            }
        }
        None
    }
}

const PAGEBREAKS: [&str; 3] = ["<!-- pagebreak -->", "<!-- page-break -->", "\\newpage"];

/// Private-use sentinels for a task marker that landed inside a paragraph.
const TASK_ON: &str = "\u{E000}x\u{E000}";
const TASK_OFF: &str = "\u{E000} \u{E000}";

fn options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_MATH
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS
}

/// Parse Markdown source into a [`Document`].
pub fn parse(src: &str) -> Document {
    let src = src.strip_prefix('\u{feff}').unwrap_or(src);
    let events: Vec<Event> = Parser::new_ext(src, options()).collect();
    let mut b = Builder { ev: events, pos: 0, front_matter: None, slugs: Vec::new() };
    let blocks = b.blocks_until(None);
    Document { front_matter: b.front_matter, blocks }
}

struct Builder<'a> {
    ev: Vec<Event<'a>>,
    pos: usize,
    front_matter: Option<String>,
    slugs: Vec<String>,
}

fn heading_level(l: HeadingLevel) -> u8 {
    match l {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn align_of(a: &Alignment) -> Align {
    match a {
        Alignment::None => Align::None,
        Alignment::Left => Align::Left,
        Alignment::Center => Align::Center,
        Alignment::Right => Align::Right,
    }
}

/// GitHub-style heading slug (lowercase, spaces → `-`, punctuation dropped;
/// CJK and other letters kept).
pub fn slugify(text: &str) -> String {
    let mut out = String::new();
    for c in text.trim().chars() {
        if c.is_alphanumeric() || c == '_' || c == '-' {
            out.extend(c.to_lowercase());
        } else if c.is_whitespace() {
            out.push('-');
        }
    }
    out
}

impl<'a> Builder<'a> {
    fn peek(&self) -> Option<&Event<'a>> {
        self.ev.get(self.pos)
    }

    fn next(&mut self) -> Option<Event<'a>> {
        let e = self.ev.get(self.pos).cloned();
        if e.is_some() {
            self.pos += 1;
        }
        e
    }

    fn unique_slug(&mut self, text: &str) -> String {
        let base = slugify(text);
        let base = if base.is_empty() { "section".to_string() } else { base };
        let mut slug = base.clone();
        let mut n = 1;
        while self.slugs.contains(&slug) {
            slug = format!("{base}-{n}");
            n += 1;
        }
        self.slugs.push(slug.clone());
        slug
    }

    /// Collect blocks until the matching `end` tag (consumed) or end of input.
    /// Loose inline events (tight list items) become an implicit paragraph.
    fn blocks_until(&mut self, end: Option<TagEnd>) -> Vec<Block> {
        let mut out = Vec::new();
        let mut loose: Vec<Inline> = Vec::new();
        loop {
            let Some(e) = self.peek().cloned() else { break };
            if let (Event::End(t), Some(want)) = (&e, &end) {
                if t == want {
                    self.pos += 1;
                    break;
                }
            }
            match e {
                Event::Start(tag) if is_block_tag(&tag) => {
                    flush_loose(&mut loose, &mut out);
                    self.pos += 1;
                    if let Some(b) = self.block(tag) {
                        out.push(b);
                    }
                }
                Event::Rule => {
                    flush_loose(&mut loose, &mut out);
                    self.pos += 1;
                    out.push(Block::Hr);
                }
                Event::Html(h) => {
                    flush_loose(&mut loose, &mut out);
                    self.pos += 1;
                    let t = h.trim();
                    if PAGEBREAKS.contains(&t) {
                        out.push(Block::PageBreak);
                    } else if !t.is_empty() {
                        out.push(Block::Html(h.to_string()));
                    }
                }
                Event::DisplayMath(m) if loose.is_empty() => {
                    self.pos += 1;
                    out.push(Block::Math(m.trim().to_string()));
                }
                Event::End(_) => {
                    // Stray end (unbalanced): consume to guarantee progress.
                    self.pos += 1;
                    if end.is_none() {
                        continue;
                    }
                    break;
                }
                _ => {
                    if let Some(i) = self.inline_event() {
                        loose.push(i);
                    }
                }
            }
        }
        flush_loose(&mut loose, &mut out);
        out
    }

    fn block(&mut self, tag: Tag<'a>) -> Option<Block> {
        match tag {
            Tag::Paragraph => {
                // A paragraph holding nothing but one `$$…$$` is a display block.
                let mut j = self.pos;
                let mut maths = Vec::new();
                let mut only_math = true;
                while let Some(e) = self.ev.get(j) {
                    match e {
                        Event::End(TagEnd::Paragraph) => break,
                        Event::DisplayMath(m) => maths.push(m.to_string()),
                        Event::SoftBreak | Event::HardBreak => {}
                        Event::Text(t) if t.trim().is_empty() => {}
                        _ => {
                            only_math = false;
                            break;
                        }
                    }
                    j += 1;
                }
                if only_math && maths.len() == 1 {
                    self.pos = j + 1;
                    return Some(Block::Math(maths.remove(0).trim().to_string()));
                }
                let inl = self.inlines_until(TagEnd::Paragraph);
                Some(paragraph_or_special(inl))
            }
            Tag::Heading { level, id, .. } => {
                let lvl = heading_level(level);
                let inl = self.inlines_until(TagEnd::Heading(level));
                let plain = inline_plain(&inl);
                let id = match id {
                    Some(i) if !i.is_empty() => {
                        self.slugs.push(i.to_string());
                        i.to_string()
                    }
                    _ => self.unique_slug(&plain),
                };
                Some(Block::Heading { level: lvl, id, inlines: inl })
            }
            Tag::BlockQuote(k) => Some(Block::Quote(self.blocks_until(Some(TagEnd::BlockQuote(k))))),
            Tag::CodeBlock(kind) => {
                let lang = match kind {
                    CodeBlockKind::Fenced(info) => info.split_whitespace().next().unwrap_or("").to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                let mut text = String::new();
                while let Some(e) = self.next() {
                    match e {
                        Event::Text(t) => text.push_str(&t),
                        Event::End(TagEnd::CodeBlock) => break,
                        _ => {}
                    }
                }
                if text.ends_with('\n') {
                    text.pop();
                }
                Some(Block::Code { lang, text })
            }
            Tag::HtmlBlock => {
                let mut html = String::new();
                while let Some(e) = self.next() {
                    match e {
                        Event::Html(h) | Event::Text(h) => html.push_str(&h),
                        Event::End(TagEnd::HtmlBlock) => break,
                        _ => {}
                    }
                }
                let t = html.trim();
                if PAGEBREAKS.contains(&t) {
                    Some(Block::PageBreak)
                } else if t.is_empty() {
                    None
                } else {
                    Some(Block::Html(html))
                }
            }
            Tag::List(start) => {
                let ordered = start.is_some();
                let mut items = Vec::new();
                loop {
                    match self.next() {
                        Some(Event::Start(Tag::Item)) => {
                            let mut task = None;
                            if let Some(Event::TaskListMarker(c)) = self.peek() {
                                task = Some(*c);
                                self.pos += 1;
                            }
                            let mut blocks = self.blocks_until(Some(TagEnd::Item));
                            // Loose task items carry the marker inside their first paragraph.
                            if task.is_none() {
                                if let Some(Block::Paragraph(inl)) = blocks.first_mut() {
                                    if let Some(Inline::Text(t)) = inl.first_mut() {
                                        if let Some(rest) = t.strip_prefix(TASK_ON) {
                                            task = Some(true);
                                            *t = rest.to_string();
                                        } else if let Some(rest) = t.strip_prefix(TASK_OFF) {
                                            task = Some(false);
                                            *t = rest.to_string();
                                        }
                                        if t.is_empty() {
                                            inl.remove(0);
                                        }
                                    }
                                }
                            }
                            if blocks.is_empty() {
                                blocks.push(Block::Paragraph(Vec::new()));
                            }
                            items.push(ListItem { task, blocks });
                        }
                        Some(Event::End(TagEnd::List(_))) | None => break,
                        _ => {}
                    }
                }
                Some(Block::List { ordered, start: start.unwrap_or(1), items })
            }
            Tag::Table(aligns) => {
                let aligns: Vec<Align> = aligns.iter().map(align_of).collect();
                let mut header = Vec::new();
                let mut rows = Vec::new();
                let mut cur_row: Vec<Vec<Inline>> = Vec::new();
                let mut in_head = false;
                loop {
                    match self.next() {
                        Some(Event::Start(Tag::TableHead)) => {
                            in_head = true;
                            cur_row.clear();
                        }
                        Some(Event::End(TagEnd::TableHead)) => {
                            header = std::mem::take(&mut cur_row);
                            in_head = false;
                        }
                        Some(Event::Start(Tag::TableRow)) => cur_row.clear(),
                        Some(Event::End(TagEnd::TableRow)) => {
                            if !in_head {
                                rows.push(std::mem::take(&mut cur_row));
                            }
                        }
                        Some(Event::Start(Tag::TableCell)) => {
                            cur_row.push(self.inlines_until(TagEnd::TableCell));
                        }
                        Some(Event::End(TagEnd::Table)) | None => break,
                        _ => {}
                    }
                }
                Some(Block::Table { aligns, header, rows })
            }
            Tag::FootnoteDefinition(label) => {
                let blocks = self.blocks_until(Some(TagEnd::FootnoteDefinition));
                Some(Block::FootnoteDef { label: label.to_string(), blocks })
            }
            Tag::MetadataBlock(kind) => {
                let mut text = String::new();
                while let Some(e) = self.next() {
                    match e {
                        Event::Text(t) => text.push_str(&t),
                        Event::End(TagEnd::MetadataBlock(k)) if k == kind => break,
                        _ => {}
                    }
                }
                if self.front_matter.is_none() {
                    self.front_matter = Some(text);
                }
                None
            }
            // Definition lists are not enabled; treat anything else as a container.
            other => {
                let end = other.to_end();
                let inner = self.blocks_until(Some(end));
                if inner.is_empty() {
                    None
                } else {
                    Some(Block::Quote(inner))
                }
            }
        }
    }

    /// Collect inline nodes until `end` (consumed).
    fn inlines_until(&mut self, end: TagEnd) -> Vec<Inline> {
        let mut out = Vec::new();
        loop {
            match self.peek() {
                None => break,
                Some(Event::End(t)) if *t == end => {
                    self.pos += 1;
                    break;
                }
                Some(Event::End(_)) => {
                    // Unbalanced: let the caller see it.
                    break;
                }
                _ => {
                    if let Some(i) = self.inline_event() {
                        out.push(i);
                    } else if self.pos >= self.ev.len() {
                        break;
                    }
                }
            }
        }
        merge_text(out)
    }

    /// Consume one inline event (or one inline container) and return its node.
    fn inline_event(&mut self) -> Option<Inline> {
        let e = self.next()?;
        Some(match e {
            Event::Text(t) => Inline::Text(t.to_string()),
            Event::Code(t) => Inline::Code(t.to_string()),
            Event::InlineMath(m) => Inline::Math(m.to_string()),
            // Display math inside running text is kept inline (rare; `$$x$$` mid-line).
            Event::DisplayMath(m) => Inline::Math(m.to_string()),
            Event::SoftBreak => Inline::SoftBreak,
            Event::HardBreak => Inline::HardBreak,
            Event::FootnoteReference(l) => Inline::FootnoteRef(l.to_string()),
            Event::InlineHtml(h) | Event::Html(h) => {
                let t = h.trim().to_ascii_lowercase();
                if t == "<br>" || t == "<br/>" || t == "<br />" {
                    Inline::HardBreak
                } else {
                    Inline::Html(h.to_string())
                }
            }
            // Only reached for loose task items; `ListItem` parsing strips it again.
            Event::TaskListMarker(c) => Inline::Text(if c { TASK_ON.into() } else { TASK_OFF.into() }),
            Event::Start(Tag::Emphasis) => Inline::Emph(self.inlines_until(TagEnd::Emphasis)),
            Event::Start(Tag::Strong) => Inline::Strong(self.inlines_until(TagEnd::Strong)),
            Event::Start(Tag::Strikethrough) => Inline::Strike(self.inlines_until(TagEnd::Strikethrough)),
            Event::Start(Tag::Link { dest_url, title, .. }) => {
                let children = self.inlines_until(TagEnd::Link);
                Inline::Link { href: dest_url.to_string(), title: title.to_string(), children }
            }
            Event::Start(Tag::Image { dest_url, title, .. }) => {
                let alt = inline_plain(&self.inlines_until(TagEnd::Image));
                Inline::Image { src: dest_url.to_string(), alt, title: title.to_string() }
            }
            Event::Start(tag) => {
                // Superscript/subscript etc. are not enabled; flatten any other
                // inline container into its children.
                let end = tag.to_end();
                let inner = self.inlines_until(end);
                Inline::Emph(inner)
            }
            Event::Rule => Inline::Text(String::new()),
            Event::End(_) => return None,
        })
    }
}

fn is_block_tag(tag: &Tag) -> bool {
    matches!(
        tag,
        Tag::Paragraph
            | Tag::Heading { .. }
            | Tag::BlockQuote(_)
            | Tag::CodeBlock(_)
            | Tag::HtmlBlock
            | Tag::List(_)
            | Tag::Table(_)
            | Tag::FootnoteDefinition(_)
            | Tag::MetadataBlock(_)
            | Tag::DefinitionList
    )
}

fn flush_loose(loose: &mut Vec<Inline>, out: &mut Vec<Block>) {
    if loose.is_empty() {
        return;
    }
    let inl = merge_text(std::mem::take(loose));
    if inl.iter().all(|i| matches!(i, Inline::SoftBreak | Inline::HardBreak) || matches!(i, Inline::Text(t) if t.trim().is_empty())) {
        return;
    }
    out.push(paragraph_or_special(inl));
}

/// A paragraph that is only `$$…$$` becomes a math block; one that is only a
/// page-break marker becomes a page break.
fn paragraph_or_special(inl: Vec<Inline>) -> Block {
    let meaningful: Vec<&Inline> = inl
        .iter()
        .filter(|i| !matches!(i, Inline::SoftBreak) && !matches!(i, Inline::Text(t) if t.trim().is_empty()))
        .collect();
    if meaningful.len() == 1 {
        if let Inline::Text(t) = meaningful[0] {
            if PAGEBREAKS.contains(&t.trim()) {
                return Block::PageBreak;
            }
        }
        if let Inline::Html(t) = meaningful[0] {
            if PAGEBREAKS.contains(&t.trim()) {
                return Block::PageBreak;
            }
        }
    }
    Block::Paragraph(inl)
}

/// Join adjacent `Text` runs.
fn merge_text(v: Vec<Inline>) -> Vec<Inline> {
    let mut out: Vec<Inline> = Vec::with_capacity(v.len());
    for i in v {
        if let Inline::Text(t) = &i {
            if let Some(Inline::Text(prev)) = out.last_mut() {
                prev.push_str(t);
                continue;
            }
        }
        out.push(i);
    }
    out
}

/// Plain text of an inline list (alt text, slugs, bookmarks).
pub fn inline_plain(v: &[Inline]) -> String {
    let mut s = String::new();
    for i in v {
        match i {
            Inline::Text(t) | Inline::Code(t) | Inline::Math(t) => s.push_str(t),
            Inline::Strong(c) | Inline::Emph(c) | Inline::Strike(c) => s.push_str(&inline_plain(c)),
            Inline::Link { children, .. } => s.push_str(&inline_plain(children)),
            Inline::Image { alt, .. } => s.push_str(alt),
            Inline::SoftBreak | Inline::HardBreak => s.push(' '),
            Inline::FootnoteRef(l) => {
                s.push('[');
                s.push_str(l);
                s.push(']');
            }
            Inline::Html(_) => {}
        }
    }
    s
}

/// Call `f` for every inline node (recursively) in the document.
pub fn walk_inlines<'d>(blocks: &'d [Block], f: &mut dyn FnMut(&'d Inline)) {
    fn walk_list<'d>(v: &'d [Inline], f: &mut dyn FnMut(&'d Inline)) {
        for i in v {
            f(i);
            match i {
                Inline::Strong(c) | Inline::Emph(c) | Inline::Strike(c) => walk_list(c, f),
                Inline::Link { children, .. } => walk_list(children, f),
                _ => {}
            }
        }
    }
    for b in blocks {
        match b {
            Block::Heading { inlines, .. } | Block::Paragraph(inlines) => walk_list(inlines, f),
            Block::List { items, .. } => {
                for it in items {
                    walk_inlines(&it.blocks, f);
                }
            }
            Block::Quote(inner) | Block::FootnoteDef { blocks: inner, .. } => walk_inlines(inner, f),
            Block::Table { header, rows, .. } => {
                for c in header {
                    walk_list(c, f);
                }
                for r in rows {
                    for c in r {
                        walk_list(c, f);
                    }
                }
            }
            _ => {}
        }
    }
}

// ------------------------------------------------------------ render JSON
//
// Projection onto the block/inline JSON shape `pdf_render::render` consumes.

fn inline_json(i: &Inline, out: &mut Vec<Value>) {
    match i {
        Inline::Text(t) => out.push(json!({"t": "text", "v": t})),
        Inline::Code(t) => out.push(json!({"t": "code", "v": t})),
        Inline::Strong(c) => out.push(json!({"t": "bold", "v": inlines_json(c)})),
        Inline::Emph(c) => out.push(json!({"t": "italic", "v": inlines_json(c)})),
        Inline::Strike(c) => out.push(json!({"t": "strike", "v": inlines_json(c)})),
        Inline::Link { href, title, children } => {
            out.push(json!({"t": "link", "text": inlines_json(children), "href": href, "title": title}))
        }
        Inline::Image { src, alt, title } => out.push(json!({"t": "image", "alt": alt, "src": src, "title": title})),
        Inline::Math(m) => out.push(json!({"t": "math", "latex": m, "display": false, "fallback": true})),
        Inline::SoftBreak => out.push(json!({"t": "text", "v": " "})),
        Inline::HardBreak => out.push(json!({"t": "br"})),
        Inline::FootnoteRef(l) => out.push(json!({"t": "text", "v": format!("[{l}]")})),
        Inline::Html(_) => {}
    }
}

pub fn inlines_json(v: &[Inline]) -> Value {
    let mut out = Vec::new();
    for i in v {
        inline_json(i, &mut out);
    }
    Value::Array(out)
}

fn push_blocks_json(blocks: &[Block], level: usize, out: &mut Vec<Value>) {
    for b in blocks {
        match b {
            Block::Heading { level: l, inlines, id } => {
                out.push(json!({"type": "heading", "level": *l as i64, "id": id, "text": inlines_json(inlines)}))
            }
            Block::Paragraph(inl) => out.push(json!({"type": "paragraph", "text": inlines_json(inl)})),
            Block::List { ordered, start, items } => {
                // `pdf_render` draws a flat list of items; nesting is carried as
                // `level` (indent) and every item carries its own number.
                let mut flat = Vec::new();
                let mut tail = Vec::new();
                for (n, it) in items.iter().enumerate() {
                    let mut text = Vec::new();
                    let mut rest: Vec<&Block> = Vec::new();
                    for (bi, blk) in it.blocks.iter().enumerate() {
                        match blk {
                            Block::Paragraph(inl) if bi == 0 => {
                                if let Value::Array(a) = inlines_json(inl) {
                                    text = a;
                                }
                            }
                            other => rest.push(other),
                        }
                    }
                    flat.push(json!({
                        "text": text,
                        "task": it.task.is_some(),
                        "checked": it.task.unwrap_or(false),
                        "ordered": *ordered,
                        "number": *start + n as u64,
                        "level": level,
                    }));
                    for blk in rest {
                        if let Block::List { .. } = blk {
                            // Nested list: flush what we have, then the child.
                            let mut child = Vec::new();
                            push_blocks_json(std::slice::from_ref(blk), level + 1, &mut child);
                            for c in child {
                                if c.get("type").and_then(|t| t.as_str()) == Some("list") {
                                    if let Some(Value::Array(ci)) = c.get("items") {
                                        flat.extend(ci.iter().cloned());
                                    }
                                } else {
                                    tail.push(c);
                                }
                            }
                        } else {
                            let mut child = Vec::new();
                            push_blocks_json(std::slice::from_ref(blk), level + 1, &mut child);
                            // Non-list continuation blocks (code, quote…) break the
                            // flat list; emit the list so far, then them.
                            if !flat.is_empty() {
                                out.push(json!({"type": "list", "items": std::mem::take(&mut flat)}));
                            }
                            out.extend(child);
                        }
                    }
                }
                if !flat.is_empty() {
                    out.push(json!({"type": "list", "items": flat}));
                }
                out.extend(tail);
            }
            Block::Quote(inner) => {
                let mut v = Vec::new();
                push_blocks_json(inner, level, &mut v);
                out.push(json!({"type": "quote", "blocks": v}));
            }
            Block::Code { lang, text } => out.push(json!({"type": "code", "lang": lang, "content": text})),
            Block::Math(m) => out.push(json!({"type": "math", "display": true, "latex": m, "fallback": true})),
            Block::Table { aligns, header, rows } => {
                let al: Vec<Value> = aligns
                    .iter()
                    .map(|a| if *a == Align::None { Value::Null } else { Value::String(a.as_str().into()) })
                    .collect();
                let has_align = aligns.iter().any(|a| *a != Align::None);
                let mut t = json!({
                    "type": "table",
                    "header": header.iter().map(|c| inlines_json(c)).collect::<Vec<_>>(),
                    "rows": rows.iter().map(|r| r.iter().map(|c| inlines_json(c)).collect::<Vec<_>>()).collect::<Vec<_>>(),
                });
                if has_align {
                    t["aligns"] = Value::Array(al);
                }
                out.push(t);
            }
            Block::Hr => out.push(json!({"type": "hr"})),
            Block::PageBreak => out.push(json!({"type": "pagebreak"})),
            Block::Html(_) => {}
            Block::FootnoteDef { label, blocks } => {
                let mut inner = Vec::new();
                push_blocks_json(blocks, level, &mut inner);
                // Rendered as a small paragraph `[label] …`.
                let mut first = true;
                for mut b in inner {
                    if first && b.get("type").and_then(|t| t.as_str()) == Some("paragraph") {
                        if let Some(Value::Array(a)) = b.get_mut("text") {
                            a.insert(0, json!({"t": "text", "v": format!("[{label}] ")}));
                        }
                        first = false;
                    }
                    out.push(b);
                }
            }
        }
    }
}

/// The `pdf_render` block JSON for a parsed document.
pub fn to_render_json(blocks: &[Block]) -> Value {
    let mut out = Vec::new();
    push_blocks_json(blocks, 0, &mut out);
    Value::Array(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Vec<Block> {
        parse(s).blocks
    }

    #[test]
    fn headings_get_unique_slugs() {
        let b = p("# Intro\n\n## Intro\n\n### 中文 标题\n");
        let ids: Vec<&str> = b
            .iter()
            .filter_map(|b| if let Block::Heading { id, .. } = b { Some(id.as_str()) } else { None })
            .collect();
        assert_eq!(ids, vec!["intro", "intro-1", "中文-标题"]);
    }

    #[test]
    fn front_matter_is_stripped() {
        let d = parse("---\ntitle: \"Hello\"\nauthor: Me\n---\n\n# H\n");
        assert_eq!(d.meta("title").as_deref(), Some("Hello"));
        assert_eq!(d.meta("author").as_deref(), Some("Me"));
        assert!(matches!(d.blocks[0], Block::Heading { level: 1, .. }));
    }

    #[test]
    fn nested_ordered_task_lists() {
        let b = p("3. a\n4. b\n   - [x] done\n   - [ ] todo\n");
        match &b[0] {
            Block::List { ordered: true, start: 3, items } => {
                assert_eq!(items.len(), 2);
                match &items[1].blocks[1] {
                    Block::List { ordered: false, items: sub, .. } => {
                        assert_eq!(sub[0].task, Some(true));
                        assert_eq!(sub[1].task, Some(false));
                    }
                    other => panic!("{other:?}"),
                }
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn table_alignment_and_code_lang() {
        let b = p("| a | b | c |\n|:--|:-:|--:|\n| 1 | 2 | 3 |\n\n```rust\nfn x() {}\n```\n");
        match &b[0] {
            Block::Table { aligns, header, rows } => {
                assert_eq!(aligns, &vec![Align::Left, Align::Center, Align::Right]);
                assert_eq!(header.len(), 3);
                assert_eq!(rows.len(), 1);
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(b[1], Block::Code { lang: "rust".into(), text: "fn x() {}".into() });
    }

    #[test]
    fn math_inline_and_display() {
        let b = p("Euler $e^{i\\pi}$ ok\n\n$$\n\\frac{a}{b}\n$$\n");
        match &b[0] {
            Block::Paragraph(inl) => assert!(inl.contains(&Inline::Math("e^{i\\pi}".into()))),
            other => panic!("{other:?}"),
        }
        assert_eq!(b[1], Block::Math("\\frac{a}{b}".into()));
    }

    #[test]
    fn pagebreak_markers() {
        let b = p("a\n\n<!-- pagebreak -->\n\nb\n\n\\newpage\n\nc\n");
        assert_eq!(b.iter().filter(|x| **x == Block::PageBreak).count(), 2);
    }

    #[test]
    fn render_json_carries_lang_aligns_task() {
        let d = parse("```py\nx\n```\n\n| a |\n|--:|\n| 1 |\n\n- [x] t\n");
        let j = to_render_json(&d.blocks);
        assert_eq!(j[0]["lang"], "py");
        assert_eq!(j[1]["aligns"][0], "right");
        assert_eq!(j[2]["items"][0]["task"], true);
        assert_eq!(j[2]["items"][0]["checked"], true);
    }

    #[test]
    fn never_panics_on_odd_input() {
        let samples = [
            "", "*", "**", "***a", "`", "$", "$$", "$$\n", "| a |\n", "- \n-", "> > >", "[x](", "![", "<!--",
            "中文**粗**`码`$x$", "- [ ]", "1.", "```\n", "---\n", "---\ntitle: x", "[^1]\n\n[^1]: n",
        ];
        for s in samples {
            let _ = to_render_json(&parse(s).blocks);
        }
    }
}
