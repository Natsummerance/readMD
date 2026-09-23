//! Pure-Rust HTML engine: tolerant parser, sanitizer, Markdown serializer,
//! main-content extractor and URL helpers.
//!
//! This module replaces the legacy out-of-process renderer that shelled out to
//! the Python WebView helper (`python -m src.readmd_modules.headless_renderer`).
//! The Rust kernel must not depend on Python or on any external binary, so
//! everything below is hand-rolled on `std` + `regex`.
//!
//! Rules mirrored from `src/readmd_modules/web.py`:
//! * `_clean_soup` — `BLOCKED_TAGS` decompose, `on*`/`style`/`srcdoc` stripping,
//!   lazy `img` promotion, `href`/`src`/`poster` absolutization with an
//!   http(s)-only scheme filter.
//! * `normalize_url` — scheme padding/whitelist, host and port validation,
//!   path defaulting, fragment dropped.
//! * `_metadata`, `_candidate_links`, `_plain_length`, `_useful`,
//!   `_format_document`, `_sanitize_markdown`.
//! * `markdownify(heading_style='ATX', bullets='-')` style serialization.
//! * `trafilatura.extract` main-content selection, implemented as a
//!   link-penalised text-density scorer over the parsed tree.

use regex::Regex;
use serde_json::{json, Value};

// --------------------------------------------------------------- node model

/// `web.BLOCKED_TAGS`
pub const BLOCKED_TAGS: &[&str] = &["script", "style", "form", "iframe", "object", "embed", "canvas", "svg"];

const VOID_TAGS: &[&str] = &[
    "area", "base", "basefont", "br", "col", "embed", "frame", "hr", "img", "input", "isindex",
    "link", "meta", "param", "source", "track", "wbr",
];

const BLOCK_TAGS: &[&str] = &[
    "address", "article", "aside", "blockquote", "center", "dd", "details", "dialog", "dir",
    "div", "dl", "dt", "fieldset", "figcaption", "figure", "footer", "form", "frame", "frameset",
    "h1", "h2", "h3", "h4", "h5", "h6", "header", "hgroup", "hr", "li", "main", "menu", "nav",
    "noscript", "ol", "p", "plaintext", "pre", "script", "section", "style", "summary", "table",
    "tbody", "td", "tfoot", "th", "thead", "tr", "ul", "br", "body", "html", "head", "option",
];

/// Elements that end a currently open `<p>`.
const P_CLOSERS: &[&str] = &[
    "address", "article", "aside", "blockquote", "details", "div", "dl", "fieldset", "figcaption",
    "figure", "footer", "form", "h1", "h2", "h3", "h4", "h5", "h6", "header", "hgroup", "hr",
    "main", "menu", "nav", "ol", "p", "pre", "section", "table", "ul", "li", "dd", "dt", "tbody",
    "td", "tfoot", "th", "thead", "tr", "option", "summary", "style", "script", "link", "meta",
];

/// Inline elements that may legally stay inside an open `<p>`.
const P_KEEPERS: &[&str] = &[
    "span", "a", "img", "br", "b", "i", "em", "strong", "code", "small", "u", "sub", "sup", "wbr",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    El(Element),
    Text(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Element {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
}

impl Element {
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }

    pub fn remove_attr(&mut self, name: &str) {
        self.attrs.retain(|(k, _)| k != name);
    }

    pub fn set_attr(&mut self, name: &str, value: &str) {
        match self.attrs.iter_mut().find(|(k, _)| k == name) {
            Some(slot) => slot.1 = value.to_string(),
            None => self.attrs.push((name.to_string(), value.to_string())),
        }
    }

    /// `rel` holds a token list; bs4 matched `'canonical' in value`.
    fn rel_contains(&self, token: &str) -> bool {
        self.attrs
            .iter()
            .any(|(k, v)| k == "rel" && v.split_whitespace().any(|t| t.eq_ignore_ascii_case(token)))
    }

    pub fn is_doc(&self) -> bool {
        self.name == "#doc"
    }
}

fn is_void(name: &str) -> bool {
    VOID_TAGS.contains(&name)
}

fn is_block(name: &str) -> bool {
    BLOCK_TAGS.contains(&name)
}

fn is_raw_text(name: &str) -> bool {
    matches!(name, "script" | "style" | "textarea" | "title" | "xmp" | "plaintext" | "noframes")
}

// ------------------------------------------------------------------- regexes

macro_rules! static_re {
    ($pat:expr) => {{
        static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
        RE.get_or_init(|| Regex::new($pat).expect("static pattern must compile"))
    }};
}

fn image_link_re() -> &'static Regex {
    static_re!(r"!\[[^]]*\]\([^)]*\)")
}

fn any_link_re() -> &'static Regex {
    static_re!(r"\[([^]]+)\]\([^)]*\)")
}

fn md_noise_re() -> &'static Regex {
    static_re!(r"[`*_>#|-]+")
}

/// CPython `\s+`, i.e. `[\s\x1c-\x1f]+`.
///
/// Ported from two authorities, both of which use a `re` `\s` class:
/// `web.py:407` `len(re.sub(r'\s+', '', text))` (`_plain_length`) and
/// `web.py:470-471` `re.sub(r'\s+', ' ', …).casefold()` (`_format_document`);
/// `collapse_ws` documents itself as `re.sub(r'\s+', ' ', text.strip())`.
/// Measured on this box (CPython 3.11.15) over every one of the 1,114,112
/// code points `re` can match: `re.match(r'\s', ch)` is true exactly when
/// `ch.isspace()` — so `\s` includes the four C0 file/group/record/unit
/// separators U+001C..U+001F, which Rust's `\s` (Unicode `White_Space`)
/// leaves out.  With a plain `\s+` a title containing `a\x1cb` collapsed to
/// `a\x1cb` instead of `a b`, and `_plain_length` counted the separator as a
/// character, moving `word_count` and the `>= 20` usefulness gate
/// (`web.py:615`).  Same widening as `bad_scheme_link_re` below.
fn ws_re() -> &'static Regex {
    static_re!(r"[\s\x1c-\x1f]+")
}

fn link_ext_skip_re() -> &'static Regex {
    static_re!(r"\.(pdf|zip|rar|7z|png|jpe?g|gif|webp|avif|mp4|mp3|docx?|xlsx?|pptx?)(\?|$)")
}

fn any_tag_re() -> &'static Regex {
    static_re!(r"<[^>]+>")
}

/// CPython's `\s` also covers U+001C..U+001F, which Rust's `\s` does not. These
/// classes mirror `web.py:501-512`; narrowing them would let `](\x1cjavascript:`
/// through the scheme blocklist and leave `<\x1cscript>` unstripped.
fn bad_scheme_link_re() -> &'static Regex {
    static_re!(r"(\]\()[\s\x1c-\x1f]*(?:javascript|data|file):[^)]*(\))")
}

fn link_shape_re() -> &'static Regex {
    static_re!(r"(!?\[[^\]]*\]\()([^\s\x1c-\x1f)]+)([^)]*\))")
}

/// Python used one backreference (`\1`) here; the `regex` crate has none, so the
/// five tag names get their own lazy pattern.
fn embedded_block_res() -> &'static [Regex] {
    static RES: std::sync::OnceLock<Vec<Regex>> = std::sync::OnceLock::new();
    RES.get_or_init(|| {
        ["script", "style", "iframe", "object", "embed"]
            .iter()
            .filter_map(|tag| {
                Regex::new(&format!(
                    r"(?is)<[\s\x1c-\x1f]*{}\b[^>]*>.*?<[\s\x1c-\x1f]*/[\s\x1c-\x1f]*{}[\s\x1c-\x1f]*>",
                    tag, tag
                ))
                .ok()
            })
            .collect()
    })
}

// ------------------------------------------------------------------ scanner

struct Scanner<'a> {
    src: &'a [u8],
    pos: usize,
}

impl<'a> Scanner<'a> {
    fn new(src: &'a str) -> Scanner<'a> {
        Scanner { src: src.as_bytes(), pos: 0 }
    }

    fn eof(&self) -> bool {
        self.pos >= self.src.len()
    }

    fn byte(&self, offset: usize) -> Option<u8> {
        self.src.get(self.pos + offset).copied()
    }

    fn starts_ci(&self, needle: &str) -> bool {
        let n = needle.as_bytes();
        self.src.len().saturating_sub(self.pos) >= n.len()
            && self.src[self.pos..self.pos + n.len()].eq_ignore_ascii_case(n)
    }

    fn take_while<F: FnMut(u8) -> bool>(&mut self, mut f: F) -> &'a str {
        let start = self.pos;
        while self.pos < self.src.len() && f(self.src[self.pos]) {
            self.pos += 1;
        }
        self.slice_from(start)
    }

    fn skip_to(&mut self, stop: u8) -> &'a str {
        let start = self.pos;
        while self.pos < self.src.len() && self.src[self.pos] != stop {
            self.pos += 1;
        }
        self.slice_from(start)
    }

    fn skip_ws(&mut self) {
        self.take_while(|c| c.is_ascii_whitespace());
    }

    /// Cut points are ASCII bytes, so slices always land on char boundaries.
    fn slice_from(&self, from: usize) -> &'a str {
        std::str::from_utf8(&self.src[from..self.pos]).unwrap_or("")
    }

    fn text_from(&self, from: usize) -> String {
        String::from_utf8_lossy(&self.src[from..self.pos]).into_owned()
    }
}

fn find_ci(hay: &[u8], needle: &str) -> Option<usize> {
    let n = needle.as_bytes();
    if n.is_empty() || hay.len() < n.len() {
        return None;
    }
    (0..=hay.len() - n.len()).find(|&i| hay[i..i + n.len()].eq_ignore_ascii_case(n))
}

fn tag_name_byte(c: u8) -> bool {
    !matches!(c, b'>' | b'/' | b'<') && !c.is_ascii_whitespace()
}

fn attr_name_byte(c: u8) -> bool {
    !matches!(c, b'>' | b'/' | b'=' | b'<' | b'"' | b'\'') && !c.is_ascii_whitespace()
}

fn norm(raw: &str) -> String {
    raw.trim().to_ascii_lowercase()
}

/// Parse HTML into a detached root element named `#doc`. Never panics and never
/// fails: unterminated tags, stray `<`, mismatched closes and bogus attributes
/// are tolerated the way `lxml` tolerates them.
pub fn parse_html(html: &str) -> Element {
    let mut root = Element { name: "#doc".into(), attrs: Vec::new(), children: Vec::new() };
    let mut stack: Vec<Element> = Vec::new();
    let mut sc = Scanner::new(html);

    while !sc.eof() {
        if sc.byte(0) != Some(b'<') {
            let start = sc.pos;
            sc.take_while(|c| c != b'<');
            if sc.pos == start {
                sc.pos += 1;
            }
            push_text(&mut stack, &mut root, sc.text_from(start));
            continue;
        }

        if sc.starts_ci("<!--") {
            sc.pos += 4;
            match find_ci(&sc.src[sc.pos..], "-->") {
                Some(off) => sc.pos += off + 3,
                None => sc.pos = sc.src.len(),
            }
            continue;
        }
        if sc.starts_ci("<!") || sc.starts_ci("<?") {
            sc.skip_to(b'>');
            if !sc.eof() {
                sc.pos += 1;
            }
            continue;
        }
        if sc.starts_ci("</") {
            sc.pos += 2;
            let name = norm(sc.take_while(tag_name_byte));
            sc.skip_to(b'>');
            if !sc.eof() {
                sc.pos += 1;
            }
            if !name.is_empty() {
                if let Some(idx) = stack.iter().rposition(|el| el.name == name) {
                    flush(&mut stack, &mut root, idx + 1);
                    let el = stack.pop().expect("index checked above");
                    flush_to_parent(&mut stack, &mut root, el);
                }
            }
            continue;
        }

        let save = sc.pos;
        sc.pos += 1;
        let raw_name = sc.take_while(tag_name_byte);
        let name = norm(raw_name);
        if raw_name.is_empty() || !raw_name.as_bytes()[0].is_ascii_alphabetic() {
            sc.pos = save;
            sc.pos += 1;
            push_text(&mut stack, &mut root, "<".to_string());
            continue;
        }

        let mut attrs: Vec<(String, String)> = Vec::new();
        let mut self_closing = false;
        loop {
            sc.skip_ws();
            if sc.eof() {
                break;
            }
            match sc.byte(0) {
                Some(b'>') => {
                    sc.pos += 1;
                    break;
                }
                Some(b'/') if sc.byte(1) == Some(b'>') => {
                    self_closing = true;
                    sc.pos += 2;
                    break;
                }
                Some(b'/') => {
                    sc.pos += 1;
                    continue;
                }
                _ => {}
            }
            let key_start = sc.pos;
            let key = norm(sc.take_while(attr_name_byte));
            if key.is_empty() {
                // `norm` trims Unicode whitespace (U+00A0, U+0085, U+2028,
                // U+3000) that the ASCII-only byte predicate accepts as name
                // bytes, so an empty attribute name does not imply the scanner
                // stalled.  Only step forward when it truly made no progress,
                // otherwise `pos` can run past `src.len()` and the slicer dies.
                if sc.pos == key_start {
                    sc.pos += 1;
                }
                continue;
            }
            sc.skip_ws();
            let mut value = String::new();
            if sc.byte(0) == Some(b'=') {
                sc.pos += 1;
                sc.skip_ws();
                match sc.byte(0) {
                    Some(q @ (b'"' | b'\'')) => {
                        sc.pos += 1;
                        let start = sc.pos;
                        while !sc.eof() && sc.byte(0) != Some(q) {
                            sc.pos += 1;
                        }
                        value = sc.text_from(start);
                        if !sc.eof() {
                            sc.pos += 1;
                        }
                    }
                    _ => {
                        let start = sc.pos;
                        sc.take_while(|c| !c.is_ascii_whitespace() && c != b'>' && c != b'/');
                        value = sc.text_from(start);
                    }
                }
            }
            if !attrs.iter().any(|(k, _)| *k == key) {
                attrs.push((key, decode_entities(&value)));
            }
        }

        if is_raw_text(&name) {
            let start = sc.pos;
            let close = format!("</{}", name);
            match find_ci(&sc.src[start..], &close) {
                Some(off) => sc.pos = start + off,
                None => sc.pos = sc.src.len(),
            }
            let body = sc.text_from(start);
            let children = if body.is_empty() { Vec::new() } else { vec![Node::Text(body)] };
            flush_to_parent(&mut stack, &mut root, Element { name, attrs, children });
            continue;
        }

        if (is_block(&name) || P_CLOSERS.contains(&name.as_str()))
            && !P_KEEPERS.contains(&name.as_str())
        {
            if let Some(idx) = stack.iter().rposition(|el| el.name == "p") {
                flush(&mut stack, &mut root, idx);
            }
        }
        match name.as_str() {
            "li" => close_if_top(&mut stack, &mut root, "li"),
            "td" | "th" => {
                close_if_top(&mut stack, &mut root, "td");
                close_if_top(&mut stack, &mut root, "th");
            }
            "tr" => close_if_top(&mut stack, &mut root, "tr"),
            "option" => close_if_top(&mut stack, &mut root, "option"),
            _ => {}
        }

        let el = Element { name, attrs, children: Vec::new() };
        if is_void(&el.name) || self_closing {
            flush_to_parent(&mut stack, &mut root, el);
        } else {
            stack.push(el);
        }
    }
    flush(&mut stack, &mut root, 0);
    root
}

fn flush(stack: &mut Vec<Element>, root: &mut Element, upto: usize) {
    while stack.len() > upto {
        let el = stack.pop().expect("length checked");
        flush_to_parent(stack, root, el);
    }
}

fn flush_to_parent(stack: &mut Vec<Element>, root: &mut Element, el: Element) {
    match stack.last_mut() {
        Some(parent) => parent.children.push(Node::El(el)),
        None => root.children.push(Node::El(el)),
    }
}

fn close_if_top(stack: &mut Vec<Element>, root: &mut Element, name: &str) {
    if stack.last().map(|el| el.name == name).unwrap_or(false) {
        let el = stack.pop().expect("checked");
        flush_to_parent(stack, root, el);
    }
}

fn push_text(stack: &mut Vec<Element>, root: &mut Element, text: String) {
    if text.is_empty() {
        return;
    }
    let children = match stack.last_mut() {
        Some(parent) => &mut parent.children,
        None => &mut root.children,
    };
    match children.last_mut() {
        Some(Node::Text(prev)) => prev.push_str(&text),
        _ => children.push(Node::Text(text)),
    }
}

// ---------------------------------------------------------------- traversal

/// bs4 `soup.find(tag)`: first descendant with that tag, document order.
pub fn find_descendant<'a>(el: &'a Element, tag: &str) -> Option<&'a Element> {
    for child in &el.children {
        if let Node::El(inner) = child {
            if inner.name == tag {
                return Some(inner);
            }
            if let Some(hit) = find_descendant(inner, tag) {
                return Some(hit);
            }
        }
    }
    None
}

pub fn has_descendant(el: &Element, tags: &[&str]) -> bool {
    for child in &el.children {
        if let Node::El(inner) = child {
            if tags.contains(&inner.name.as_str()) || has_descendant(inner, tags) {
                return true;
            }
        }
    }
    false
}

/// Pre-order visit of `el` and every descendant.
pub fn walk_el<'a, F: FnMut(&'a Element)>(el: &'a Element, f: &mut F) {
    f(el);
    for child in &el.children {
        if let Node::El(inner) = child {
            walk_el(inner, f);
        }
    }
}

pub fn walk_mut<F>(el: &mut Element, f: &mut F)
where
    F: for<'b> FnMut(&'b mut Element),
{
    for child in &mut el.children {
        if let Node::El(inner) = child {
            walk_mut(inner, f);
        }
    }
    f(el);
}

/// Remove every subtree whose root tag is in `tags` (bs4 `decompose`).
pub fn filter_out(root: &mut Element, tags: &[&str]) {
    fn prune(nodes: &mut Vec<Node>, tags: &[&str]) {
        nodes.retain(|n| match n {
            Node::El(el) => !tags.contains(&el.name.as_str()),
            Node::Text(_) => true,
        });
        for node in nodes.iter_mut() {
            if let Node::El(el) = node {
                prune(&mut el.children, tags);
            }
        }
    }
    prune(&mut root.children, tags);
}

// -------------------------------------------------------------- entity decode

const NAMED_ENTITIES: &[(&str, &str)] = &[
    ("amp", "&"),
    ("lt", "<"),
    ("gt", ">"),
    ("quot", "\""),
    ("apos", "'"),
    ("nbsp", "\u{a0}"),
    ("ensp", "\u{2002}"),
    ("emsp", "\u{2003}"),
    ("thinsp", "\u{2009}"),
    ("copy", "©"),
    ("reg", "®"),
    ("trade", "™"),
    ("hellip", "…"),
    ("mdash", "—"),
    ("ndash", "–"),
    ("minus", "−"),
    ("laquo", "«"),
    ("raquo", "»"),
    ("lsquo", "\u{2018}"),
    ("rsquo", "\u{2019}"),
    ("ldquo", "\u{201c}"),
    ("rdquo", "\u{201d}"),
    ("middot", "·"),
    ("bull", "•"),
    ("deg", "°"),
    ("plusmn", "±"),
    ("times", "×"),
    ("divide", "÷"),
    ("sect", "§"),
    ("para", "¶"),
    ("dagger", "†"),
    ("permil", "‰"),
    ("prime", "′"),
    ("Prime", "″"),
    ("larr", "←"),
    ("rarr", "→"),
    ("harr", "↔"),
    ("zwj", "\u{200d}"),
    ("zwnj", "\u{200c}"),
    ("shy", "\u{ad}"),
    ("eacute", "é"),
    ("egrave", "è"),
    ("auml", "ä"),
    ("ouml", "ö"),
    ("uuml", "ü"),
    ("szlig", "ß"),
    ("Auml", "Ä"),
    ("Ouml", "Ö"),
    ("Uuml", "Ü"),
];

fn named_entity(body: &str) -> Option<&'static str> {
    NAMED_ENTITIES.iter().find(|(k, _)| *k == body).map(|(_, v)| *v)
}

/// Decode a numeric reference starting at `start` (past the `#`).
/// Returns the text and the index just after it (including an optional `;`).
fn numeric_entity_at(chars: &[char], start: usize) -> Option<(String, usize)> {
    let (radix, skip) = match chars.get(start) {
        Some('x' | 'X') => (16, 1),
        _ => (10, 0),
    };
    let mut digits = String::new();
    let mut i = start + skip;
    while i < chars.len() {
        match chars[i].to_digit(radix) {
            Some(_) => {
                digits.push(chars[i]);
                i += 1;
            }
            None => break,
        }
    }
    if digits.is_empty() {
        return None;
    }
    let code = u32::from_str_radix(&digits, radix).ok()?;
    let text = match char::from_u32(code) {
        Some(c) => c.to_string(),
        None => "\u{fffd}".to_string(),
    };
    if chars.get(i) == Some(&';') {
        i += 1;
    }
    Some((text, i))
}

/// Decode HTML entities the way a browser does; unknown references stay literal.
pub fn decode_entities(text: &str) -> String {
    if !text.contains('&') {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i] != '&' {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        if chars.get(i + 1) == Some(&'#') {
            if let Some((decoded, next)) = numeric_entity_at(&chars, i + 2) {
                out.push_str(&decoded);
                i = next;
                continue;
            }
            out.push('&');
            i += 1;
            continue;
        }
        let mut body = String::new();
        let mut j = i + 1;
        while j < chars.len() && body.chars().count() < 12 && chars[j].is_ascii_alphanumeric() {
            body.push(chars[j]);
            j += 1;
        }
        let mut hit: Option<(usize, &'static str)> = None;
        for len in (1..=body.chars().count()).rev() {
            let probe: String = body.chars().take(len).collect();
            if let Some(value) = named_entity(&probe) {
                hit = Some((len, value));
                break;
            }
        }
        match hit {
            Some((len, value)) => {
                out.push_str(value);
                i += 1 + len;
                if chars.get(i) == Some(&';') {
                    i += 1;
                }
            }
            None => {
                out.push('&');
                i += 1;
            }
        }
    }
    out
}

/// Python `str.isspace()`: Unicode `White_Space` plus the four C0 separators
/// U+001C..U+001F, which Rust's `char::is_whitespace` leaves out.  This is the
/// same class `re`'s `\s` matches (see `ws_re`).
fn py_isspace(ch: char) -> bool {
    ch.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&ch)
}

/// Python `str.strip()` — `trim()` alone would keep `\x1c`..`\x1f` at the edges.
fn py_trim(text: &str) -> &str {
    text.trim_matches(py_isspace as fn(char) -> bool)
}

/// `re.sub(r'\s+', ' ', text.strip())`
pub fn collapse_ws(text: &str) -> String {
    ws_re().replace_all(py_trim(text), " ").into_owned()
}

// ------------------------------------------------------- _clean_soup mirror

/// Mirrors `web._clean_soup`.
pub fn clean_soup(html: &str, base_url: &str) -> Element {
    let mut root = parse_html(html);
    walk_mut(&mut root, &mut |el| {
        el.attrs.retain(|(k, _)| {
            let low = k.to_ascii_lowercase();
            !(low.starts_with("on") || low == "style" || low == "srcdoc")
        });
        if el.name == "img" && el.attr("src").map(|v| v.trim().is_empty()).unwrap_or(true) {
            let lazy = el
                .attr("data-src")
                .or_else(|| el.attr("data-original"))
                .unwrap_or("")
                .trim()
                .to_string();
            if !lazy.is_empty() {
                el.set_attr("src", &lazy);
            }
        }
        let mut removals: Vec<String> = Vec::new();
        for attr in ["href", "src", "poster"] {
            let value = match el.attr(attr) {
                Some(v) if !v.trim().is_empty() => v.trim().to_string(),
                _ => continue,
            };
            match absolute_url(base_url, &value) {
                Some(absolute) if is_http(&absolute) => el.set_attr(attr, &absolute),
                _ => removals.push(attr.to_string()),
            }
        }
        for attr in removals {
            el.remove_attr(&attr);
        }
    });
    filter_out(&mut root, BLOCKED_TAGS);
    root
}

pub fn is_http(url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UrlError {
    MissingUrl,
    UnsupportedScheme,
    InvalidUrl,
}

/// Mirrors `web.normalize_url`: pad a missing scheme, whitelist http(s),
/// validate host and port, default the path to `/`, drop the fragment.
pub fn normalize_url(url: &str) -> Result<String, UrlError> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err(UrlError::MissingUrl);
    }
    let with_scheme = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("https://{}", trimmed)
    };
    let (scheme, rest) = with_scheme.split_once("://").ok_or(UrlError::InvalidUrl)?;
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return Err(UrlError::UnsupportedScheme);
    }
    let (authority, tail) = match rest.find(['/', '?', '#']) {
        Some(idx) => (&rest[..idx], &rest[idx..]),
        None => (rest, ""),
    };
    let hostport = authority.rsplit('@').next().unwrap_or("");
    if hostport.is_empty() {
        return Err(UrlError::InvalidUrl);
    }
    // `urlsplit._hostinfo` maps an *empty* port segment to None, so both
    // "http://example.com:/" and "http://[::1]:/" carry no port at all and the
    // rebuilt netloc drops the colon; only a non-empty, unparsable segment is
    // the ValueError that `normalize_url` turns into `invalid_url`.
    let (host, port) = if let Some(inner) = hostport.strip_prefix('[') {
        let close = inner.find(']').ok_or(UrlError::InvalidUrl)?;
        let after = &inner[close + 1..];
        let port = match after.strip_prefix(':') {
            Some(p) if !p.is_empty() => Some(p.to_string()),
            Some(_) => None,
            None => None,
        };
        (inner[..close].to_string(), port)
    } else {
        match hostport.split_once(':') {
            Some((h, p)) => (h.to_string(), if p.is_empty() { None } else { Some(p.to_string()) }),
            None => (hostport.to_string(), None),
        }
    };
    // web.normalize_url uses parsed.hostname, which is lowercased (and idna-encoded);
    // an ASCII host only needs the lowercasing to match Python.
    let host = host.to_ascii_lowercase();
    if host.is_empty() || host.chars().any(|c| c.is_whitespace()) {
        return Err(UrlError::InvalidUrl);
    }
    let mut netloc = if host.contains(':') { format!("[{}]", host) } else { host };
    if let Some(raw) = port {
        // Mirrors Python `port = parsed.port` (ValueError -> invalid_url) then
        // `if port: netloc += ':' + str(port)`. parsed.port returns the explicit
        // integer even for a scheme default (80/443), so those ARE kept; only a
        // missing or falsy (0) port is dropped. Valid range: 0 <= port <= 65535.
        let parsed: u32 = raw.parse().map_err(|_| UrlError::InvalidUrl)?;
        if parsed > 65535 {
            return Err(UrlError::InvalidUrl);
        }
        if parsed != 0 {
            netloc.push(':');
            netloc.push_str(&parsed.to_string());
        }
    }
    let body = match tail.split_once('#') {
        Some((before, _)) => before.to_string(),
        None => tail.to_string(),
    };
    let (mut path, query) = match body.split_once('?') {
        Some((p, q)) => (p.to_string(), format!("?{}", q)),
        None => (body, String::new()),
    };
    if path.is_empty() {
        path = "/".to_string();
    }
    Ok(format!("{}://{}{}{}", scheme, netloc, path, query))
}

pub fn normalize_url_opt(url: &str) -> Option<String> {
    normalize_url(url).ok()
}

/// `urllib.parse.urljoin` for the shapes the sanitizer and crawler see.
pub fn absolute_url(base: &str, target: &str) -> Option<String> {
    let target = target.trim();
    if target.is_empty() {
        return None;
    }
    if has_scheme(target) {
        return Some(scrub(target));
    }
    let base = split_url(base)?;
    if let Some(after) = target.strip_prefix("//") {
        return Some(scrub(&format!("{}://{}", base.0, after)));
    }
    if target.starts_with('/') {
        return Some(scrub(&format!("{}://{}{}", base.0, base.1, target)));
    }
    if target.starts_with('?') {
        let bare = base.2.split_once('?').map(|(p, _)| p).unwrap_or(&base.2);
        return Some(scrub(&format!("{}://{}{}{}", base.0, base.1, bare, target)));
    }
    if target.starts_with('#') {
        // urljoin: a fragment-only reference keeps the base path+query and replaces
        // the fragment; an empty `#` drops the fragment entirely (no trailing '#').
        let base_no_frag = base.2.split_once('#').map(|(p, _)| p).unwrap_or(&base.2);
        let frag = if target == "#" { "" } else { target };
        return Some(scrub(&format!(
            "{}://{}{}{}",
            base.0, base.1, base_no_frag, frag
        )));
    }
    let dir = match base.2.rfind('/') {
        Some(idx) => &base.2[..idx + 1],
        None => "/",
    };
    Some(scrub(&format!(
        "{}://{}{}",
        base.0,
        base.1,
        normalize_path(&format!("{}{}", dir, target))
    )))
}

fn scrub(url: &str) -> String {
    url.chars().filter(|c| !c.is_whitespace() && !matches!(c, '<' | '>')).collect()
}

fn has_scheme(url: &str) -> bool {
    match url.find(':') {
        Some(idx) if idx > 0 => {
            let scheme = &url[..idx];
            !scheme.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(true)
                && scheme
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        }
        _ => false,
    }
}

fn normalize_path(path: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "." => {}
            ".." => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    let mut joined = out.join("/");
    if path.ends_with('/') && !joined.ends_with('/') {
        joined.push('/');
    }
    if !joined.starts_with('/') {
        joined.insert(0, '/');
    }
    joined
}

/// (scheme, authority, path-with-query)
pub fn split_url(url: &str) -> Option<(String, String, String)> {
    let (scheme, rest) = url.split_once("://")?;
    let (authority, tail) = match rest.find('/') {
        Some(idx) => (&rest[..idx], &rest[idx..]),
        None => (rest, "/"),
    };
    let (path, query) = match tail.split_once('?') {
        Some((p, q)) => (p.to_string(), format!("?{}", q)),
        None => (tail.to_string(), String::new()),
    };
    let path = if path.is_empty() { "/".to_string() } else { path };
    Some((
        scheme.to_ascii_lowercase(),
        authority.to_string(),
        format!("{}{}", path, query),
    ))
}

pub fn hostname(url: &str) -> String {
    split_url(url)
        .map(|(_, authority, _)| {
            let host = authority.rsplit('@').next().unwrap_or("");
            match host.strip_prefix('[') {
                Some(inner) => inner.split(']').next().unwrap_or("").to_ascii_lowercase(),
                None => host.split(':').next().unwrap_or("").to_ascii_lowercase(),
            }
        })
        .unwrap_or_default()
}

fn strip_www(host: &str) -> String {
    host.strip_prefix("www.").unwrap_or(host).to_string()
}

// --------------------------------------------------------------- _metadata

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Meta {
    pub title: String,
    pub author: String,
    pub date: String,
    pub site: String,
    pub canonical_url: String,
}

impl Meta {
    pub fn to_value(&self) -> Value {
        json!({
            "title": self.title,
            "author": self.author,
            "date": self.date,
            "site": self.site,
            "canonical_url": self.canonical_url,
        })
    }
}

fn meta_content(doc: &Element, names: &[&str]) -> String {
    for name in names {
        let mut found = String::new();
        walk_el(doc, &mut |el| {
            if !found.is_empty() || el.name != "meta" {
                return;
            }
            let hit = el
                .attr("property")
                .or_else(|| el.attr("name"))
                .map(|v| v.eq_ignore_ascii_case(name))
                .unwrap_or(false);
            if hit {
                if let Some(content) = el.attr("content") {
                    if !content.trim().is_empty() {
                        found = content.trim().to_string();
                    }
                }
            }
        });
        if !found.is_empty() {
            return found;
        }
    }
    String::new()
}

/// Mirrors `web._metadata`.
pub fn metadata(doc: &Element, url: &str) -> Meta {
    let mut title = meta_content(doc, &["og:title", "twitter:title"]);
    if title.is_empty() {
        if let Some(node) = find_descendant(doc, "title") {
            title = collapse_ws(&element_text(node));
        }
    }
    let mut canonical_href: Option<String> = None;
    walk_el(doc, &mut |el| {
        if canonical_href.is_some() || el.name != "link" || !el.rel_contains("canonical") {
            return;
        }
        if let Some(href) = el.attr("href") {
            if !href.trim().is_empty() {
                canonical_href = Some(href.trim().to_string());
            }
        }
    });
    let canonical_url = match canonical_href {
        Some(href) => absolute_url(url, &href).unwrap_or_else(|| url.to_string()),
        None => url.to_string(),
    };
    let og_site = meta_content(doc, &["og:site_name"]);
    let site = if og_site.is_empty() { hostname(url) } else { og_site };
    Meta {
        title: if title.is_empty() {
            url.to_string()
        } else {
            title.chars().take(300).collect()
        },
        author: meta_content(doc, &["author", "article:author"]),
        date: meta_content(doc, &["article:published_time", "date", "datePublished"]),
        site,
        canonical_url,
    }
}

// ------------------------------------------------------- _candidate_links

/// Mirrors `web._candidate_links`.
pub fn candidate_links(doc: &Element, base_url: &str, limit: usize) -> Vec<String> {
    let base_host = strip_www(&hostname(base_url));
    let base_clean = canonical_link(base_url);
    let mut anchors: Vec<String> = Vec::new();
    walk_el(doc, &mut |el| {
        if el.name == "a" {
            if let Some(href) = el.attr("href") {
                if !href.trim().is_empty() {
                    anchors.push(href.trim().to_string());
                }
            }
        }
    });
    let mut out: Vec<String> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for href in anchors {
        let joined = match absolute_url(base_url, &href) {
            Some(j) => j,
            None => continue,
        };
        let full = match normalize_url(&joined) {
            Ok(f) => f,
            Err(_) => continue,
        };
        if strip_www(&hostname(&full)) != base_host {
            continue;
        }
        let clean = canonical_link(&full);
        if seen.iter().any(|s| s == &clean) || clean == base_clean {
            continue;
        }
        if link_ext_skip_re().is_match(&clean) {
            continue;
        }
        seen.push(clean.clone());
        out.push(clean);
        if out.len() >= limit {
            break;
        }
    }
    out
}

/// `urlunparse((scheme, netloc, path or '/', params, query, ''))`
fn canonical_link(url: &str) -> String {
    match split_url(url) {
        Some((scheme, netloc, tail)) => {
            let (path, query) = match tail.split_once('?') {
                Some((p, q)) => (p.to_string(), format!("?{}", q)),
                None => (tail, String::new()),
            };
            let (path, params) = match path.split_once(';') {
                Some((p, rest)) => (p.to_string(), format!(";{}", rest)),
                None => (path, String::new()),
            };
            let path = if path.is_empty() { "/".to_string() } else { path };
            format!("{}://{}{}{}{}", scheme, netloc, path, params, query)
        }
        None => url.to_string(),
    }
}

// --------------------------------------------------- length / usefulness / md

/// Mirrors `web._plain_length`: Markdown stripped, whitespace ignored, counted
/// in code points (Python's `len(str)`).
pub fn plain_length(markdown: &str) -> usize {
    let text = image_link_re().replace_all(markdown, " ").into_owned();
    let text = any_link_re().replace_all(&text, "$1").into_owned();
    let text = md_noise_re().replace_all(&text, " ").into_owned();
    ws_re().replace_all(&text, "").chars().count()
}

/// Mirrors `web._useful`.
pub fn useful(markdown: &str, soup: Option<&Element>, minimum: usize) -> bool {
    let length = plain_length(markdown);
    if length >= minimum {
        return true;
    }
    if length < 20 || soup.is_none() {
        return false;
    }
    let root = find_descendant(soup.unwrap(), "article").or_else(|| find_descendant(soup.unwrap(), "main"));
    match root {
        Some(el) => has_descendant(el, &["p", "pre", "table", "ul", "ol", "blockquote"]),
        None => false,
    }
}

// ------------------------------------------------------- _sanitize_markdown

/// Mirrors `web._sanitize_markdown`.
pub fn sanitize_markdown(markdown: &str, base_url: &str) -> String {
    let mut value = markdown.to_string();
    for re in embedded_block_res() {
        value = re.replace_all(&value, "").into_owned();
    }
    value = any_tag_re().replace_all(&value, "").into_owned();
    value = bad_scheme_link_re().replace_all(&value, "${1}#${2}").into_owned();
    let mut out = String::with_capacity(value.len());
    let mut last = 0usize;
    for caps in link_shape_re().captures_iter(&value) {
        let whole = match caps.get(0) {
            Some(m) => m,
            None => continue,
        };
        out.push_str(&value[last..whole.start()]);
        last = whole.end();
        let prefix = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        let target = caps.get(2).map(|m| m.as_str()).unwrap_or("");
        let suffix = caps.get(3).map(|m| m.as_str()).unwrap_or("");
        let raw = target.trim().trim_start_matches('<').trim_end_matches('>');
        let absolute = if base_url.is_empty() {
            raw.to_string()
        } else {
            absolute_url(base_url, raw).unwrap_or_else(|| raw.to_string())
        };
        out.push_str(prefix);
        out.push_str(if is_http(&absolute) { &absolute } else { "#" });
        out.push_str(suffix);
    }
    out.push_str(&value[last..]);
    out.trim().to_string()
}

// -------------------------------------------------------- _format_document

/// Mirrors `web._format_document`.
pub fn format_document(markdown: &str, meta: &Meta) -> String {
    let title = {
        // `web.py:466` — `(title or canonical or '网页').strip()`, and that value
        // is what `:471` collapses and what `# {title}` is emitted with.
        let t = py_trim(&meta.title);
        if !t.is_empty() {
            t.to_string()
        } else {
            let c = py_trim(&meta.canonical_url);
            if !c.is_empty() {
                c.to_string()
            } else {
                "网页".to_string()
            }
        }
    };
    let mut body = py_trim(&sanitize_markdown(markdown, &meta.canonical_url)).to_string();
    let mut lines: Vec<&str> = body.split('\n').collect();
    if lines.first().map(|l| l.starts_with("# ")).unwrap_or(false) {
        let first = lines[0];
        let extracted = collapse_ws(&first[2..]).to_lowercase();
        // `web.py:466` builds `title` with `.strip()` and `:471` collapses it with
        // a `re` `\s+` — both ends need the Python whitespace class.
        let document_title = ws_re().replace_all(py_trim(&title), " ").into_owned().to_lowercase();
        if extracted == document_title {
            lines.remove(0);
            body = lines.join("\n").trim_start().to_string();
        }
    }
    let mut out: Vec<String> = vec![
        format!("# {}", title),
        String::new(),
        format!("> 来源：{}", meta.canonical_url),
    ];
    let mut extra: Vec<String> = Vec::new();
    if !meta.author.is_empty() {
        extra.push(format!("作者：{}", meta.author));
    }
    if !meta.date.is_empty() {
        extra.push(format!("发布时间：{}", meta.date));
    }
    if !meta.site.is_empty() {
        extra.push(format!("站点：{}", meta.site));
    }
    if !extra.is_empty() {
        out.push(format!("> {}", extra.join(" · ")));
    }
    out.push(String::new());
    out.push(body);
    let mut joined = out.join("\n").trim().to_string();
    joined.push('\n');
    joined
}

// ---------------------------------------------------- html -> markdown mirror

/// `markdownify(html, heading_style='ATX', bullets='-', strip=(script, style,
/// form, iframe), table_infer_header=True).strip()`
pub fn html_to_markdown(html: &str) -> String {
    markdown_of(&parse_html(html))
}

/// Serialize an element's children to Markdown.
pub fn markdown_of(root: &Element) -> String {
    let mut buf = String::new();
    for child in &root.children {
        render(child, &mut buf);
    }
    normalize_markdown(&buf)
}

fn render(node: &Node, buf: &mut String) {
    match node {
        Node::Text(t) => {
            let collapsed = collapse_inline(&decode_entities(t));
            if !collapsed.is_empty() {
                buf.push_str(&collapsed);
            }
        }
        Node::El(el) => render_el(el, buf),
    }
}

#[allow(clippy::too_many_lines)]
fn render_el(el: &Element, buf: &mut String) {
    let name = el.name.as_str();
    if matches!(
        name,
        "script" | "style" | "noscript" | "template" | "head" | "meta" | "link" | "title"
            | "form" | "iframe" | "option" | "select" | "button" | "input" | "textarea" | "svg" | "canvas"
            | "object" | "embed"
    ) {
        return;
    }
    match name {
        "br" => buf.push('\n'),
        "hr" => buf.push_str("\n\n---\n\n"),
        "img" => {
            let src = el.attr("src").unwrap_or("");
            if src.is_empty() {
                return;
            }
            let alt = el.attr("alt").unwrap_or("");
            match el.attr("title") {
                Some(t) if !t.is_empty() => {
                    buf.push_str(&format!("![{}]({} \"{}\")", alt, md_url(src), t))
                }
                _ => buf.push_str(&format!("![{}]({})", alt, md_url(src))),
            }
        }
        "a" => {
            let href = el.attr("href").unwrap_or("");
            let label = collapse_ws(&element_text(el));
            if href.is_empty() {
                if !label.is_empty() {
                    buf.push_str(&label);
                }
                return;
            }
            let label = if label.is_empty() { href.to_string() } else { label };
            buf.push_str(&format!("[{}]({})", label, md_url(href)));
        }
        "strong" | "b" => push_wrapped(buf, el, "**", "**"),
        "em" | "i" | "cite" | "var" | "dfn" => push_wrapped(buf, el, "_", "_"),
        "del" | "s" | "strike" => push_wrapped(buf, el, "~~", "~~"),
        "code" | "kbd" | "samp" | "tt" => {
            let inner = element_text(el).trim().to_string();
            if inner.is_empty() {
                return;
            }
            let fence = if inner.contains("``") {
                "```"
            } else if inner.contains('`') {
                "``"
            } else {
                "`"
            };
            let pad = if inner.starts_with('`') || inner.ends_with('`') { " " } else { "" };
            buf.push_str(&format!("{fence}{pad}{inner}{pad}{fence}"));
        }
        "pre" => {
            let lang = el
                .attr("class")
                .and_then(|c| {
                    c.split_whitespace().find_map(|t| {
                        t.strip_prefix("language-")
                            .or_else(|| t.strip_prefix("lang-"))
                            .or_else(|| t.strip_prefix("highlight-"))
                    })
                })
                .unwrap_or("")
                .to_string();
            let text = element_text(el).trim_end_matches('\n').to_string();
            if text.trim().is_empty() {
                return;
            }
            buf.push_str(&format!("\n\n```{}\n{}\n```\n\n", lang, text));
        }
        "blockquote" => {
            let inner = normalize_markdown(&render_children(el));
            if inner.trim().is_empty() {
                return;
            }
            buf.push('\n');
            for line in inner.trim().split('\n') {
                if line.trim().is_empty() {
                    buf.push_str(">\n");
                } else {
                    buf.push_str("> ");
                    buf.push_str(line.trim());
                    buf.push('\n');
                }
            }
            buf.push('\n');
        }
        "ul" | "ol" => render_list(el, buf, name == "ol"),
        "table" => render_table(el, buf),
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
            let level = (name.as_bytes()[1] - b'0') as usize;
            let text = collapse_ws(&element_text(el));
            if text.is_empty() {
                return;
            }
            buf.push('\n');
            for _ in 0..level {
                buf.push('#');
            }
            buf.push(' ');
            buf.push_str(&text);
            buf.push_str("\n\n");
        }
        "p" | "div" | "section" | "article" | "main" | "header" | "footer" | "aside" | "figure"
        | "figcaption" | "details" | "summary" | "nav" | "fieldset" | "dl" | "center" | "tr"
        | "tbody" | "thead" | "tfoot" | "body" | "html" | "#doc" => {
            let has_block_child = el.children.iter().any(|c| match c {
                Node::El(inner) => inner.name != "br" && is_block(&inner.name),
                Node::Text(_) => false,
            });
            let inner = render_children(el);
            if has_block_child {
                buf.push_str(&inner);
            } else {
                let trimmed = collapse_ws(&inner);
                if !trimmed.is_empty() {
                    buf.push('\n');
                    buf.push_str(&trimmed);
                    buf.push_str("\n\n");
                }
            }
        }
        _ => {
            let inner = render_children(el);
            buf.push_str(&inner);
        }
    }
}

fn push_wrapped(buf: &mut String, el: &Element, open: &str, close: &str) {
    let inner = collapse_ws(&element_text(el));
    if inner.is_empty() {
        return;
    }
    buf.push_str(open);
    buf.push_str(&inner);
    buf.push_str(close);
}

fn render_children(el: &Element) -> String {
    let mut buf = String::new();
    for child in &el.children {
        render(child, &mut buf);
    }
    buf
}

fn render_list(el: &Element, buf: &mut String, ordered: bool) {
    let start = if ordered {
        el.attr("start").and_then(|v| v.trim().parse::<usize>().ok()).unwrap_or(1)
    } else {
        1
    };
    let mut index = start;
    buf.push('\n');
    for child in &el.children {
        if let Node::El(li) = child {
            if li.name != "li" {
                render(child, buf);
                continue;
            }
            let marker = if ordered { format!("{}. ", index) } else { "- ".to_string() };
            index += 1;
            let inner = normalize_markdown(&render_children(li));
            if inner.trim().is_empty() {
                buf.push_str(&marker);
                buf.push('\n');
                continue;
            }
            let mut lines = inner.trim().split('\n');
            if let Some(first) = lines.next() {
                buf.push_str(&marker);
                buf.push_str(first.trim());
                buf.push('\n');
            }
            for rest in lines {
                if rest.trim().is_empty() {
                    buf.push('\n');
                } else {
                    buf.push_str("  ");
                    buf.push_str(rest.trim());
                    buf.push('\n');
                }
            }
        }
    }
    buf.push('\n');
}

fn render_table(el: &Element, buf: &mut String) {
    let mut rows: Vec<Vec<String>> = Vec::new();
    collect_rows(el, &mut rows);
    if rows.is_empty() {
        return;
    }
    let width = rows.iter().map(|r| r.len()).max().unwrap_or(0).max(1);
    buf.push('\n');
    for (idx, row) in rows.iter().enumerate() {
        buf.push_str("| ");
        for col in 0..width {
            let cell = row.get(col).map(|s| s.as_str()).unwrap_or("");
            buf.push_str(&cell.replace('|', "\\|"));
            buf.push_str(" | ");
        }
        buf.push('\n');
        if idx == 0 {
            buf.push('|');
            for _ in 0..width {
                buf.push_str(" --- |");
            }
            buf.push('\n');
        }
    }
    buf.push('\n');
}

fn collect_rows(el: &Element, rows: &mut Vec<Vec<String>>) {
    for child in &el.children {
        if let Node::El(inner) = child {
            match inner.name.as_str() {
                "tr" => rows.push(row_cells(inner)),
                "thead" | "tbody" | "tfoot" => collect_rows(inner, rows),
                _ => {}
            }
        }
    }
}

fn row_cells(tr: &Element) -> Vec<String> {
    let mut cells = Vec::new();
    for child in &tr.children {
        if let Node::El(cell) = child {
            if matches!(cell.name.as_str(), "td" | "th") {
                cells.push(collapse_ws(&element_text(cell)));
            }
        }
    }
    cells
}

fn md_url(url: &str) -> String {
    let mut out = String::with_capacity(url.len());
    for c in url.chars() {
        match c {
            ' ' => out.push_str("%20"),
            '(' => out.push_str("%28"),
            ')' => out.push_str("%29"),
            '<' => out.push_str("%3C"),
            '>' => out.push_str("%3E"),
            '"' => out.push_str("%22"),
            other => out.push(other),
        }
    }
    out
}

/// Whitespace collapsing for text nodes: runs become a single space, and a run
/// that contains a newline no longer sticks words together.
fn collapse_inline(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut pending = false;
    for c in text.chars() {
        if c.is_whitespace() {
            if !out.is_empty() {
                pending = true;
            }
            continue;
        }
        if pending {
            out.push(' ');
            pending = false;
        }
        out.push(c);
    }
    if pending && !out.is_empty() {
        out.push(' ');
    }
    out
}

/// markdownify post-processing: trim ends and collapse blank-line runs.
pub fn normalize_markdown(raw: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut blank = 0usize;
    let mut in_fence = false;
    for line in raw.split('\n') {
        let line = line.trim_end();
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            out.push(line.to_string());
            blank = 0;
            continue;
        }
        if line.trim().is_empty() && !in_fence {
            blank += 1;
            if blank <= 1 {
                out.push(String::new());
            }
            continue;
        }
        blank = 0;
        out.push(line.to_string());
    }
    out.join("\n").trim().to_string()
}

/// Text of a subtree, entities decoded, `script`/`style` skipped.
pub fn element_text(el: &Element) -> String {
    let mut out = String::new();
    collect_text_el(el, &mut out);
    out
}

fn collect_text_el(el: &Element, out: &mut String) {
    if matches!(el.name.as_str(), "script" | "style" | "noscript" | "template") {
        return;
    }
    let block = is_block(el.name.as_str());
    if block {
        out.push(' ');
    }
    for child in &el.children {
        match child {
            Node::Text(t) => out.push_str(&decode_entities(t)),
            Node::El(inner) => collect_text_el(inner, out),
        }
    }
    if matches!(el.name.as_str(), "br" | "img" | "hr") {
        out.push(' ');
    }
    if block {
        out.push(' ');
    }
}

pub fn root_body(doc: &Element) -> Option<&Element> {
    find_descendant(doc, "body")
}

// ------------------------------------------------------ article extraction

/// trafilatura-style main-content selection: score every candidate container by
/// how much *non-link* text it holds, then serialize the winner. `favor_recall`
/// relaxes pruning so more containers compete (trafilatura's second pass).
pub fn extract_article(html: &str, favor_recall: bool) -> Option<String> {
    let doc = parse_html(html);
    let mut best: Option<(f64, usize, Element)> = None;
    consider(&doc, favor_recall, &mut best, 0);
    let (_, _, chosen) = best?;
    let markdown = markdown_of(&chosen);
    if plain_length(&markdown) < 20 {
        return None;
    }
    Some(markdown)
}

fn consider(el: &Element, favor_recall: bool, best: &mut Option<(f64, usize, Element)>, depth: usize) {
    if depth > 40 {
        return;
    }
    if !favor_recall
        && matches!(el.name.as_str(), "nav" | "aside" | "footer" | "header" | "noscript" | "template")
    {
        return;
    }
    if matches!(
        el.name.as_str(),
        "div" | "article" | "main" | "section" | "td" | "body" | "#doc" | "li"
    ) {
        let text_len = plain_length(&element_text(el)) as f64;
        let mut link_len = 0f64;
        let mut blocks = 0f64;
        walk_el(el, &mut |inner| match inner.name.as_str() {
            "a" => link_len += plain_length(&element_text(inner)) as f64,
            "p" => {
                if plain_length(&element_text(inner)) >= 40 {
                    blocks += 1.0;
                }
            }
            "pre" | "table" | "ul" | "ol" | "blockquote" | "h2" | "h3" => blocks += 1.0,
            _ => {}
        });
        let score = (text_len - link_len.min(text_len)) + 25.0 * blocks;
        if score >= 80.0 {
            // On an exact tie the DEEPER (more specific) container wins: an ancestor
            // that only adds pure navigation scores identically (its extra text is all
            // link text, cancelled by `text_len - link_len`), so keeping the deeper node
            // drops the nav/menu wrapper, matching trafilatura's article selection.
            let incumbent_wins = matches!(
                best.as_ref(),
                Some((prev, prev_depth, _))
                    if *prev > score || (*prev == score && *prev_depth >= depth)
            );
            if !incumbent_wins {
                *best = Some((score, depth, el.clone()));
            }
        }
    }
    for child in &el.children {
        if let Node::El(inner) = child {
            consider(inner, favor_recall, best, depth + 1);
        }
    }
}

// -------------------------------------------------- legacy renderer contract

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderErrorCode {
    RenderDependencyMissing,
    RenderTimeout,
    RenderProcessFailed,
    RenderInvalidResponse,
}

#[derive(Debug, Clone)]
pub struct RenderError {
    pub code: String,
    pub error_code: RenderErrorCode,
}

/// Headless "render" of a URL. The Rust kernel ships no system WebView bridge,
/// so this reports the same degraded shape the legacy renderer returned when
/// `pywebview` was missing — the difference being that no subprocess is spawned.
pub fn render_url_isolated(url: &str, _timeout_sec: u64) -> Value {
    json!({
        "ok": false,
        "url": url,
        "error": "webview renderer is not available in the rust kernel",
        "turns": [],
        "turns_count": 0,
    })
}

pub fn render_url(url: &str, timeout_sec: u64) -> Result<Value, RenderError> {
    let value = render_url_isolated(url, timeout_sec);
    Err(RenderError {
        code: value
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("render_unavailable")
            .to_string(),
        error_code: RenderErrorCode::RenderDependencyMissing,
    })
}

/// `str(element)` — hands a sanitized subtree to another pipeline stage.
pub fn to_html(node: &Node) -> String {
    let mut out = String::new();
    write_node(node, &mut out);
    out
}

fn write_node(node: &Node, out: &mut String) {
    match node {
        Node::Text(t) => out.push_str(t),
        Node::El(el) => write_el(el, out),
    }
}

fn write_el(el: &Element, out: &mut String) {
    if el.is_doc() {
        for child in &el.children {
            write_node(child, out);
        }
        return;
    }
    out.push('<');
    out.push_str(&el.name);
    for (k, v) in &el.attrs {
        out.push(' ');
        out.push_str(k);
        out.push_str("=\"");
        out.push_str(&v.replace('"', "&quot;"));
        out.push('"');
    }
    out.push('>');
    if is_void(&el.name) {
        return;
    }
    for child in &el.children {
        write_node(child, out);
    }
    out.push_str("</");
    out.push_str(&el.name);
    out.push('>');
}

// --------------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_and_mismatched_tags() {
        let doc = parse_html("<div><p>hello <b>bold</b><p>second</div>");
        let text = element_text(&doc);
        assert!(text.contains("hello"), "{text}");
        assert!(text.contains("bold"), "{text}");
        assert!(text.contains("second"), "{text}");
    }

    #[test]
    fn unclosed_structures_still_yield_content() {
        let doc = parse_html("<ul><li>one<li>two</ul><table><tr><td>a<td>b</tr></table>");
        let md = markdown_of(&doc);
        assert!(md.contains("- one"), "{md}");
        assert!(md.contains("- two"), "{md}");
        assert!(md.contains("| a | b |"), "{md}");
    }

    #[test]
    fn decodes_named_and_numeric_entities() {
        assert_eq!(decode_entities("a &amp; b"), "a & b");
        assert_eq!(decode_entities("&#65;&#x42;"), "AB");
        // Panics if any row drifts from CPython `html.unescape`
        // (src/readmd_modules/convert.py:290), which never yields U+0020 here.
        assert_eq!(decode_entities("&nbsp;x"), "\u{a0}x");
        assert_eq!(decode_entities("&ensp;&emsp;&thinsp;&shy;"), "\u{2002}\u{2003}\u{2009}\u{ad}");
        assert_eq!(decode_entities("&unknownthing;"), "&unknownthing;");
        assert_eq!(decode_entities("5 &lt; 6 and a&amp;#b"), "5 < 6 and a&#b");
        assert_eq!(decode_entities("&#xZZ;"), "&#xZZ;");
    }

    #[test]
    fn clean_soup_strips_events_promotes_lazy_and_drops_js_links() {
        let cleaned = clean_soup(
            "<a href=\"javascript:alert(1)\">bad</a><a href=\"/ok\">good</a>\
             <img data-src=\"pic.png\" onload=\"x()\" style=\"a\"><script>evil()</script>",
            "https://example.com/docs/page.html",
        );
        let html = to_html(&Node::El(cleaned));
        assert!(!html.contains("javascript:"), "{html}");
        assert!(html.contains("https://example.com/ok"), "{html}");
        assert!(html.contains("https://example.com/docs/pic.png"), "{html}");
        assert!(!html.contains("evil"), "{html}");
        assert!(!html.contains("onload"), "{html}");
        assert!(!html.contains("style="), "{html}");
    }

    #[test]
    fn plain_length_counts_non_space_chars() {
        assert_eq!(plain_length("a b [c](d)"), 3);
        assert_eq!(plain_length("![img](u) text"), 4);
    }

    #[test]
    fn metadata_reads_og_canonical_and_host() {
        let doc = parse_html(
            "<html><head><title>fallback</title>\
             <meta property=\"og:title\" content=\"Real Title\">\
             <link rel=\"canonical\" href=\"/a/b\">\
             <meta name=\"author\" content=\"Jane\"></head><body>x</body></html>",
        );
        let meta = metadata(&doc, "https://example.com/post");
        assert_eq!(meta.title, "Real Title");
        assert_eq!(meta.canonical_url, "https://example.com/a/b");
        assert_eq!(meta.author, "Jane");
        assert_eq!(meta.site, "example.com");
    }

    #[test]
    fn metadata_title_falls_back_to_the_url() {
        let doc = parse_html("<p>no title here</p>");
        let meta = metadata(&doc, "https://example.com/x");
        assert_eq!(meta.title, "https://example.com/x");
    }

    #[test]
    fn candidate_links_are_same_site_and_filtered() {
        let doc = parse_html(
            "<a href=\"/a.pdf\">pdf</a><a href=\"/posts/1\">one</a>\
             <a href=\"https://other.com/x\">off</a><a href=\"/posts/1\">dup</a>\
             <a href=\"/posts/2\">two</a>",
        );
        let links = candidate_links(&doc, "https://www.example.com/posts/0", 30);
        // Python keeps the netloc verbatim (`www.` included); removeprefix('www.') is only
        // for the same-site check, never the emitted link (web.py:445).
        assert_eq!(
            links,
            vec![
                "https://www.example.com/posts/1",
                "https://www.example.com/posts/2",
            ]
        );
    }

    #[test]
    fn candidate_links_respect_the_limit() {
        let mut html = String::new();
        for i in 0..40 {
            html.push_str(&format!("<a href=\"/p/{i}\">{i}</a>"));
        }
        let doc = parse_html(&html);
        assert_eq!(candidate_links(&doc, "https://example.com/", 10).len(), 10);
    }

    #[test]
    fn sanitize_markdown_strips_tags_schemes_and_absolutizes() {
        let out = sanitize_markdown("[x](javascript:1) [y](/z) <b>bold</b>", "https://example.com/a/b");
        // javascript: -> `](#)` first, then the absolute_link sub urljoin's the bare `#`
        // against the base (empty fragment dropped) -> the base URL (web.py:505-512).
        assert_eq!(out, "[x](https://example.com/a/b) [y](https://example.com/z) bold");
        let dropped = sanitize_markdown("<script>evil()</script>keep", "https://example.com");
        assert_eq!(dropped, "keep");
    }

    #[test]
    fn format_document_prefixes_title_and_source() {
        let meta = Meta {
            title: "Doc".into(),
            author: "Jane".into(),
            date: String::new(),
            site: "example.com".into(),
            canonical_url: "https://example.com/a".into(),
        };
        let out = format_document("# Doc\n\nbody text", &meta);
        assert_eq!(out, "# Doc\n\n> 来源：https://example.com/a\n> 作者：Jane · 站点：example.com\n\nbody text\n");
    }

    #[test]
    fn format_document_keeps_a_differing_heading() {
        let meta = Meta {
            title: "Site".into(),
            author: String::new(),
            date: String::new(),
            site: String::new(),
            canonical_url: "https://example.com/a".into(),
        };
        let out = format_document("# Article\n\ntext", &meta);
        assert!(out.starts_with("# Site\n"), "{out}");
        assert!(out.contains("# Article"), "{out}");
    }

    #[test]
    fn html_to_markdown_covers_common_constructs() {
        let md = html_to_markdown(
            "<h2>Head</h2><p>Some <strong>bold</strong> and <em>it</em>.</p>\
             <ul><li>one</li><li>two</li></ul>\
             <table><tr><th>A</th><th>B</th></tr><tr><td>1</td><td>2</td></tr></table>\
             <blockquote>quoted</blockquote><pre><code>println!()</code></pre>\
             <a href=\"https://x.example/p\">link</a><img src=\"i.png\" alt=\"i\">",
        );
        assert!(md.contains("## Head"), "{md}");
        assert!(md.contains("**bold**"), "{md}");
        assert!(md.contains("- one"), "{md}");
        assert!(md.contains("| A | B |"), "{md}");
        assert!(md.contains("> quoted"), "{md}");
        assert!(md.contains("```"), "{md}");
        assert!(md.contains("[link](https://x.example/p)"), "{md}");
        assert!(md.contains("![i](i.png)") || md.contains("![i]("), "{md}");
    }

    #[test]
    fn article_extraction_prefers_the_text_dense_container() {
        let filler = "This paragraph carries enough words to win the density scoring against navigation blocks. ";
        let html = format!(
            "<body><nav><a href=\"/1\">menu one</a><a href=\"/2\">menu two</a></nav>\
             <div id=\"content\"><p>{f}</p><p>{f}{f}</p></div></body>",
            f = filler
        );
        let out = extract_article(&html, false).expect("article found");
        assert!(out.contains("density scoring"), "{out}");
        assert!(!out.contains("menu one"), "{out}");
    }

    #[test]
    fn useful_requires_length_or_a_semantic_root() {
        assert!(useful(&"字".repeat(40), None, 40));
        assert!(!useful("short", None, 40));
        let doc = parse_html("<article><p>medium length body text here for the semantic check</p></article>");
        assert!(useful(&"词".repeat(25), Some(&doc), 40));
        let bare = parse_html("<article></article>");
        assert!(!useful(&"词".repeat(25), Some(&bare), 40));
    }

    #[test]
    fn urljoin_helper_handles_all_shapes() {
        let base = "https://example.com/a/b/c.html?x=1";
        assert_eq!(absolute_url(base, "/z").unwrap(), "https://example.com/z");
        assert_eq!(absolute_url(base, "../z").unwrap(), "https://example.com/a/z");
        assert_eq!(absolute_url(base, "z").unwrap(), "https://example.com/a/b/z");
        assert_eq!(absolute_url(base, "//cdn.example.com/x").unwrap(), "https://cdn.example.com/x");
        assert_eq!(absolute_url(base, "https://other.com/x").unwrap(), "https://other.com/x");
    }

    #[test]
    fn normalize_url_mirrors_python_rules() {
        assert_eq!(normalize_url("example.com/a").unwrap(), "https://example.com/a");
        assert_eq!(normalize_url("http://example.com").unwrap(), "http://example.com/");
        assert_eq!(normalize_url("http://example.com:8080").unwrap(), "http://example.com:8080/");
        assert_eq!(normalize_url("HTTP://Example.COM/a#frag").unwrap(), "http://example.com/a");
        assert_eq!(normalize_url("http://example.com:80/").unwrap(), "http://example.com:80/");
        assert_eq!(normalize_url("ftp://example.com/a"), Err(UrlError::UnsupportedScheme));
        assert_eq!(normalize_url("   "), Err(UrlError::MissingUrl));
        assert_eq!(normalize_url("http:///nohost"), Err(UrlError::InvalidUrl));
        assert_eq!(normalize_url("http://example.com:notaport/x"), Err(UrlError::InvalidUrl));
    }

    #[test]
    fn renderer_reports_no_python_or_subprocess_dependency() {
        let value = render_url_isolated("https://example.com", 5);
        assert_eq!(value["ok"], json!(false));
        assert!(value["error"].as_str().unwrap().contains("rust kernel"));
        assert_eq!(value["turns_count"], json!(0));
    }

    // ---------------------------------------------- Item 2: CPython whitespace
    //
    // Measured on this box (CPython 3.11.15) over all 1,114,112 code points `re`
    // can match: `re.match(r'\s', ch)` is true exactly when `ch.isspace()`.  Rust's
    // `\s` is Unicode `White_Space`, which drops the four C0 separators
    // U+001C..U+001F that Python treats as whitespace, so `ws_re` — and the
    // `.strip()` it is paired with — had to be widened.  See the `ws_re` doc.

    #[test]
    fn c0_separators_are_whitespace_where_python_says_so() {
        for ch in ['\u{1c}', '\u{1d}', '\u{1e}', '\u{1f}'] {
            assert!(py_isspace(ch), "{:?}", ch);
            // `re.sub(r'\s+', ' ', text.strip())` — web.py:470-471 / collapse_ws
            assert_eq!(collapse_ws(&format!("a{}b", ch)), "a b", "{:?}", ch);
            assert_eq!(collapse_ws(&format!("{}ab{}  cd", ch, ch)), "ab cd", "{:?}", ch);
            // `len(re.sub(r'\s+', '', text))` — web.py:407, i.e. `word_count` and
            // the `>= 20` gate in `_useful`; the separator is not a character.
            assert_eq!(plain_length(&format!("a{}b", ch)), 2, "{:?}", ch);
            assert_eq!(plain_length(&format!("a{}b", ch)), plain_length("a b"));
        }
        // The class must not be *wider* than Python's, either.
        assert!(!py_isspace('\u{7f}'), "DEL is not whitespace");
        assert!(!py_isspace('\u{200b}'), "ZERO WIDTH SPACE is not whitespace");
        assert_eq!(plain_length("a\u{200b}b"), 3);
        assert_eq!(collapse_ws("a\u{200b}b"), "a\u{200b}b");
        // …and the ones both sides agree on stay whitespace.
        assert!(py_isspace('\u{a0}') && py_isspace('\u{85}'));
        assert_eq!(plain_length("a\u{a0}\u{85}b"), 2);
        assert_eq!(collapse_ws("a\u{a0}\u{85}b"), "a b");
    }

    /// `web.py:466-472`: the title is `.strip()`ed and both sides of the H1
    /// comparison go through `re.sub(r'\s+', ' ', …)`, so a heading padded with C0
    /// separators is the *same* title and has to be dropped.
    #[test]
    fn format_document_strips_and_collapses_cpython_whitespace() {
        let meta = Meta {
            title: "\u{1c} Doc \u{1d}".into(),
            author: String::new(),
            date: String::new(),
            site: String::new(),
            canonical_url: "https://example.com/a".into(),
        };
        let out = format_document("#  Doc\u{1e} \n\nbody", &meta);
        assert_eq!(out, "# Doc\n\n> 来源：https://example.com/a\n\nbody\n");
        // A genuinely different heading still survives, C0 separator and all.
        let out = format_document("# Other\u{1c}thing\n\nbody", &meta);
        assert_eq!(out.matches("# ").count(), 2, "{out}");
        assert!(out.contains("Other\u{1c}thing"), "{out}");
    }

    // --------------------------------------------- Item 2: UTF-8 slice safety
    //
    /// Every slice in this module cuts at an ASCII cut point, at a boundary the
    /// byte scanner has already stepped over `len_utf8()` past, or through
    /// `from_utf8_lossy` / an explicit `is_empty()` guard, and the file contains no
    /// `unsafe` at all.  This drives the whole public surface with random
    /// multi-byte / C0 / bidi / astral input so a future byte-offset slice panics
    /// here rather than in a 1 MiB overlay thread.
    #[test]
    fn hostile_multibyte_input_never_panics_a_slicer() {
        let atoms: &[&str] = &[
            "\u{1c}", "\u{1d}", "\u{1e}", "\u{1f}", "\u{7f}", "\u{ad}", "\u{200b}",
            "\u{feff}", "\u{e000}", "中", "文", "é", "😀", "Ω", "\u{a0}", "\u{85}",
            "<", ">", "&", "\"", "'", "=", "/", "\\", "%", "$", "#", "[", "]", "(", ")",
            "https://", "javascript:", "data:text/html,", "file://", "mailto:", "\n", "\t",
            "\r\n", "  ", "a", "b", "1", "`", "*", "_", "|", "-", "!", ":", "?",
        ];
        let mut state = 0x9E37_79B9_7F4A_7C15u64;
        for case in 0..1500u32 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let mut s = String::new();
            for k in 0..(case % 26) {
                s.push_str(atoms[((state >> (k % 32)) as usize ^ (k as usize * 7)) % atoms.len()]);
            }
            let doc = parse_html(&s);
            let cleaned = clean_soup(&s, &s);
            let meta = metadata(&doc, &s);
            let _ = metadata(&cleaned, &s);
            let _ = candidate_links(&cleaned, &s, 5);
            let _ = plain_length(&s);
            let _ = collapse_ws(&s);
            let _ = decode_entities(&s);
            let _ = sanitize_markdown(&s, &s);
            let _ = format_document(&s, &meta);
            let _ = html_to_markdown(&s);
            let _ = markdown_of(&doc);
            let _ = normalize_markdown(&s);
            let _ = element_text(&doc);
            let _ = root_body(&doc);
            let _ = find_descendant(&doc, "a");
            let _ = has_descendant(&doc, &["p"]);
            let _ = extract_article(&s, case % 2 == 0);
            let _ = useful(&s, Some(&doc), 20);
            let _ = normalize_url(&s);
            let _ = normalize_url_opt(&s);
            let _ = absolute_url(&s, &s);
            let _ = is_http(&s);
            let _ = split_url(&s);
            let _ = hostname(&s);
            let _ = to_html(&Node::El(doc.clone()));
            let _ = meta.to_value();
        }
    }
}
