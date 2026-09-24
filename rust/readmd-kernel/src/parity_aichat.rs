//! Parity port of `src/readmd_modules/ai_chat_parser.py` (593 lines, 15 defs).
//!
//! This is the AI chat *share-page* HTML -> Markdown extractor that
//! `web.extract_html` tries **first** for every URL extraction
//! (`web.py:529-549`): ChatGPT / Claude / Gemini / DeepSeek / Kimi / ...
//! share links are decoded into a conversation transcript before the
//! trafilatura ladder ever runs.
//!
//! ## Ported deviations (disclosed, same list WD5 committed to)
//!
//! * No `bs4`/`lxml`/`markdownify` in this offline crate; the DOM and Markdown
//!   mirror from [`crate::headless_renderer`] is reused.  Consequences:
//!   `_clean_html_node`'s `str(node)` re-serialisation is not observable (the
//!   cleaned clone goes straight to `markdown_of`), HTML comments are dropped
//!   at scan time instead of at clean time, and markdownify's
//!   `strip=['script','style','button']` is unreachable (already decomposed).
//! * `markdown_of`'s normaliser (`normalize_markdown`) also protects blank
//!   lines inside fenced code blocks and collapses `blank{2,}` runs, where
//!   Python ran `re.sub(r'\n{3,}', '\n\n', ...)` over the raw markdownify
//!   output; deltas are limited to blank lines inside fences and table headers.
//! * JSON goes through the order-preserving [`Pj`] reader (`store::pyjson`)
//!   because `AI_PLATFORMS` iteration and `_find_key_recursive` depend on
//!   Python dict insertion order (`serde_json::Map` here is a `BTreeMap`).
//! * Public entry points never raise for str inputs, so no raised state
//!   escapes this file; Python's `try` blocks are modelled by the private
//!   [`Raised`] marker.

use crate::headless_renderer as hr;
use crate::store::pyjson::{self, Pj};
use std::collections::HashSet;

/// Python exception raised inside a mirrored `try` block.  It always escapes
/// a `fn` whose Python body is not wrapped in `try`, and the caller converts
/// it into that `try`'s `except Exception: pass`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Raised;

pub type PRes<T> = Result<T, Raised>;

// ------------------------------------------------------------- AI_PLATFORMS

/// One record of `ai_chat_parser.AI_PLATFORMS` (`ai_chat_parser.py:25-86`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlatformInfo {
    pub name: &'static str,
    pub icon: &'static str,
    pub role_assistant: &'static str,
    pub hosts: &'static [&'static str],
}

/// `AI_PLATFORMS` — the iteration order is load-bearing for
/// `detect_ai_platform` (first hit wins), so this stays a slice, never a map.
pub const AI_PLATFORMS: &[(&str, PlatformInfo)] = &[
    ("gemini", PlatformInfo {
        name: "Google Gemini",
        icon: "",
        role_assistant: "Gemini",
        hosts: &["share.gemini.google", "gemini.google.com", "bard.google.com"],
    }),
    ("chatgpt", PlatformInfo {
        name: "OpenAI ChatGPT",
        icon: "",
        role_assistant: "ChatGPT",
        hosts: &["chatgpt.com", "chat.openai.com"],
    }),
    ("claude", PlatformInfo {
        name: "Anthropic Claude",
        icon: "",
        role_assistant: "Claude",
        hosts: &["claude.ai"],
    }),
    ("deepseek", PlatformInfo {
        name: "DeepSeek",
        icon: "",
        role_assistant: "DeepSeek",
        hosts: &["chat.deepseek.com", "deepseek.com"],
    }),
    ("kimi", PlatformInfo {
        name: "Kimi AI",
        icon: "",
        role_assistant: "Kimi",
        hosts: &["kimi.moonshot.cn", "kimi.ai"],
    }),
    ("perplexity", PlatformInfo {
        name: "Perplexity AI",
        icon: "",
        role_assistant: "Perplexity",
        hosts: &["perplexity.ai", "www.perplexity.ai"],
    }),
    ("doubao", PlatformInfo {
        name: "豆包 (Doubao)",
        icon: "",
        role_assistant: "豆包",
        hosts: &["doubao.com", "www.doubao.com"],
    }),
    ("tongyi", PlatformInfo {
        name: "通义千问 (Qwen)",
        icon: "",
        role_assistant: "通义千问",
        hosts: &["tongyi.aliyun.com", "qianwen.aliyun.com"],
    }),
    ("yiyan", PlatformInfo {
        name: "文心一言 (ERNIE)",
        icon: "",
        role_assistant: "文心一言",
        hosts: &["yiyan.baidu.com"],
    }),
    ("chatglm", PlatformInfo {
        name: "智谱清言 (GLM)",
        icon: "",
        role_assistant: "智谱清言",
        hosts: &["chatglm.cn"],
    }),
];

// ------------------------------------------------------------ Pj value rules

// Borrowed `'static` defaults: `Vec::new()` and `String::new()` are `const fn`,
// so these can be `static` (a `const` would copy the value into a temporary at
// every `&EMPTY_OBJ` use site and fail the borrow check).
static EMPTY_OBJ: Pj = Pj::Obj(Vec::new());
static EMPTY_ARR: Pj = Pj::Arr(Vec::new());
static EMPTY_STR: Pj = Pj::Str(String::new());
static NULL_VAL: Pj = Pj::Null;

/// `str(value)` for the scalar types; containers get a `repr()` fallback
/// (Python reaches `str(dict)` only through the `str(p['text'])` line, which
/// real share payloads never feed a container; the `repr` shape below is a
/// best-effort mirror for the pathological case).
pub fn py_str(v: &Pj) -> String {
    match v {
        Pj::Str(s) => s.clone(),
        Pj::Bool(true) => "True".to_string(),
        Pj::Bool(false) => "False".to_string(),
        Pj::Null => "None".to_string(),
        Pj::Int(s) => s
            .parse::<i64>()
            .map(|n| n.to_string())
            .unwrap_or_else(|_| normalize_int_literal(s)),
        Pj::Float(f) => pyjson::float_repr(*f),
        other => py_repr(other),
    }
}

fn py_repr(v: &Pj) -> String {
    match v {
        Pj::Str(s) => py_str_repr(s),
        other => py_str(other),
    }
}

/// Python `repr(str)`: single quotes unless the text contains `'` but no `"`.
fn py_str_repr(s: &str) -> String {
    let quote = if s.contains('\'') && !s.contains('"') { '"' } else { '\'' };
    let mut out = String::with_capacity(s.len() + 2);
    out.push(quote);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            q if q == quote => {
                out.push('\\');
                out.push(q);
            }
            other => out.push(other),
        }
    }
    out.push(quote);
    out
}

/// Fallback for integer literals that do not fit `i64`: strip JSON-invalid
/// leading zeros and a `-0` result, the way `str(int(...))` would normalise.
fn normalize_int_literal(s: &str) -> String {
    let (sign, digits) = match s.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", s),
    };
    let trimmed = digits.trim_start_matches('0');
    let trimmed = if trimmed.is_empty() { "0" } else { trimmed };
    if trimmed == "0" {
        return "0".to_string(); // str(-0) == "0"
    }
    format!("{sign}{trimmed}")
}

/// `x.get(key)` on a dict-like; anything that is not an object behaves like
/// Python's `AttributeError` *only where Python would hit it* — the callers
/// that guard with `isinstance` check the shape first.
fn pj_get<'a>(v: &'a Pj, key: &str) -> Option<&'a Pj> {
    match v {
        Pj::Obj(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, val)| val),
        _ => None,
    }
}

fn pj_has(v: &Pj, key: &str) -> bool {
    matches!(v, Pj::Obj(fields) if fields.iter().any(|(k, _)| k == key))
}

/// `x.get(key, {})` immediately followed by another `.get()` in Python: a
/// *missing* key yields the `{}` default, but a present non-dict value makes
/// the next `.get()` raise `AttributeError`.
fn pj_get_dict_field<'a>(v: &'a Pj, key: &str) -> PRes<&'a Pj> {
    match v {
        Pj::Obj(fields) => match fields.iter().find(|(k, _)| k == key) {
            Some((_, val @ Pj::Obj(_))) => Ok(val),
            Some((_, _)) => Err(Raised),
            None => Ok(&EMPTY_OBJ),
        },
        // Python only reaches this line on dicts (guarded by isinstance or
        // by the earlier `.get` chain); non-dicts would have raised already.
        _ => Err(Raised),
    }
}

/// `for x in value` for the shapes `json.loads` can produce.  Iterating a
/// dict yields its keys; iterating a str yields its characters; scalars and
/// booleans raise `TypeError`.
fn pj_iter(v: &Pj) -> PRes<Vec<Pj>> {
    match v {
        Pj::Arr(items) => Ok(items.clone()),
        Pj::Obj(fields) => Ok(fields.iter().map(|(k, _)| Pj::Str(k.clone())).collect()),
        Pj::Str(s) => Ok(s.chars().map(|c| Pj::Str(c.to_string())).collect()),
        _ => Err(Raised),
    }
}

fn eq_str(v: Option<&Pj>, s: &str) -> bool {
    matches!(v, Some(Pj::Str(x)) if x == s)
}

// ---------------------------------------------------------------- urlparse

/// The two `urlparse(url.lower())` fields this module reads: `netloc` and
/// `path`.  Mirrors `urllib.parse.urlsplit` splitting for the shapes a share
/// link takes (scheme + `//authority`), including the quirk that `urlparse`
/// without a scheme leaves `netloc` empty.
struct PyUrl {
    netloc: String,
    path: String,
}

fn py_urlparse(url: &str) -> PyUrl {
    let url = url.trim_matches(|c: char| c == '\t' || c == '\n' || c == '\r' || c == '\u{b}' || c == '\u{c}');
    // Scheme: `url.find(':')` with a valid scheme before it and a letter
    // first (`urlparse` raises for digit-leading schemes; we just treat them
    // as no-scheme, matching what the empty netloc does downstream).
    let mut scheme_len = 0usize; // length of "scheme:" including the colon
    if let Some(idx) = url.find(':') {
        let head = &url[..idx];
        let valid = !head.is_empty()
            && head.chars().next().map(|c| c.is_ascii_alphabetic()).unwrap_or(false)
            && head
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
        if valid {
            scheme_len = idx + 1;
        }
    }
    let rest = &url[scheme_len..];
    if scheme_len > 0 && !rest.starts_with("//") {
        // "scheme:opaque" — netloc stays empty, path is the opaque body
        // truncated at query/fragment.
        let path = truncate_path(opaque_path(rest));
        return PyUrl { netloc: String::new(), path: path.to_string() };
    }
    let body = if scheme_len > 0 {
        &rest[2..]
    } else if let Some(after) = rest.strip_prefix("//") {
        after
    } else if rest.is_empty() {
        ""
    } else {
        url
    };
    // `urlparse` is lenient about "https:/example.com" style bodies: split
    // the authority at the first '/', '?' or '#'.
    let authority_end = body.find(['/', '?', '#']).unwrap_or(body.len());
    let netloc = body[..authority_end].to_string();
    let tail = &body[authority_end..];
    let path = truncate_path(tail);
    PyUrl { netloc, path: path.to_string() }
}

fn opaque_path(rest: &str) -> &str {
    let cut = rest.find(['?', '#']).unwrap_or(rest.len());
    &rest[..cut]
}

fn truncate_path(tail: &str) -> &str {
    let cut = tail.find(['?', '#']).unwrap_or(tail.len());
    &tail[..cut]
}

// ----------------------------------------------------- detect_ai_platform

/// `detect_ai_platform(url)` (`ai_chat_parser.py:89-98`).  Returns the
/// platform id (`"generic"` when unknown) and, when known, the record.
pub fn detect_ai_platform(url: &str) -> (&'static str, Option<&'static PlatformInfo>) {
    if url.is_empty() {
        return ("generic", None);
    }
    let parsed = py_urlparse(&url.to_lowercase());
    let host = match parsed.netloc.find(':') {
        // `netloc.split(':')[0]` — with userinfo present this is the first
        // *userinfo* token segment, not the host (`http://u:p@x` -> `u`).
        Some(idx) => &parsed.netloc[..idx],
        None => parsed.netloc.as_str(),
    };
    for (plat_id, info) in AI_PLATFORMS {
        if info.hosts.iter().any(|h| *h == host || host.ends_with(&format!(".{h}"))) {
            return (plat_id, Some(info));
        }
    }
    ("generic", None)
}

/// `is_ai_chat_url(url)` (`ai_chat_parser.py:101-109`).  No in-repo Python
/// caller (public API only); `'/share/' in path` is subsumed by
/// `'/share' in path` exactly as Python wrote it.
pub fn is_ai_chat_url(url: &str) -> bool {
    if url.is_empty() {
        return false;
    }
    let (plat_id, _) = detect_ai_platform(url);
    if plat_id != "generic" {
        return true;
    }
    let path = py_urlparse(&url.to_lowercase()).path;
    path.contains("/share/") || path.contains("/share") || path.contains("/chat/")
}

// -------------------------------------------------------------- DOM helpers

/// bs4 `soup.find_all(tag_names, pred)` over descendants, document order
/// (element before its own descendants).
fn find_all<'a>(
    root: &'a hr::Element,
    tags: Option<&[&str]>,
    pred: &dyn Fn(&hr::Element) -> bool,
) -> Vec<&'a hr::Element> {
    let mut out: Vec<&'a hr::Element> = Vec::new();
    collect_matches(root, tags, pred, &mut out);
    out
}

fn collect_matches<'a>(
    el: &'a hr::Element,
    tags: Option<&[&str]>,
    pred: &dyn Fn(&hr::Element) -> bool,
    out: &mut Vec<&'a hr::Element>,
) {
    for child in &el.children {
        if let hr::Node::El(inner) = child {
            let name_ok = tags.map(|t| t.contains(&inner.name.as_str())).unwrap_or(true);
            if name_ok && pred(inner) {
                out.push(inner);
            }
            collect_matches(inner, tags, pred, out);
        }
    }
}

/// `node.get('class', [])` — bs4 splits the multi-valued attribute.
fn class_tokens(el: &hr::Element) -> Vec<&str> {
    el.attr("class").map(|v| v.split_whitespace().collect()).unwrap_or_default()
}

/// `' '.join(node.get('class', []))` normalised the way bs4 re-joins it.
fn joined_classes(el: &hr::Element) -> String {
    class_tokens(el).join(" ")
}

/// bs4 matches `class_=re.compile(pattern)` with `pattern.search(token)` per
/// individual class token; every pattern used here is a plain `a|b|c`
/// alternation under `re.I`, i.e. a case-insensitive substring test.
fn class_matches(el: &hr::Element, alts: &[&str]) -> bool {
    class_tokens(el).into_iter().any(|token| {
        let token = token.to_lowercase();
        alts.iter().any(|alt| token.contains(&alt.to_lowercase()))
    })
}

/// bs4 `Tag.string`: exactly one direct child and it is a string.
fn first_child_string(el: &hr::Element) -> Option<&str> {
    if el.children.len() == 1 {
        if let hr::Node::Text(t) = &el.children[0] {
            return Some(t);
        }
    }
    None
}

/// `soup.title.string` with entities decoded (bs4 decodes RCDATA).
fn title_string(doc: &hr::Element) -> Option<String> {
    let node = find_all(doc, Some(&["title"]), &|_| true).into_iter().next()?;
    first_child_string(node).map(hr::decode_entities)
}

/// `script.string or script.text or ''` — script bodies are raw text.
fn script_text(node: &hr::Element) -> String {
    match first_child_string(node) {
        Some(t) => t.to_string(),
        None => raw_text_concat(node),
    }
}

fn raw_text_concat(el: &hr::Element) -> String {
    let mut out = String::new();
    for child in &el.children {
        match child {
            hr::Node::Text(t) => out.push_str(t),
            hr::Node::El(inner) => out.push_str(&raw_text_concat(inner)),
        }
    }
    out
}

// ---------------------------------------------------- _clean_html_node/-md

/// `_clean_html_node` (`ai_chat_parser.py:112-120`).  Returns the cleaned
/// clone; see the module docs for why `str(node)` re-serialisation is not
/// observable through `markdown_of`.
fn clean_html_node(node: &hr::Element) -> hr::Element {
    let mut clone = node.clone();
    // bs4 `node.find_all([...])` + `decompose()` — descendants only.
    hr::filter_out(&mut clone, &["script", "style", "noscript", "svg", "button", "input"]);
    // Comments: dropped at scan time by `hr::parse_html` (disclosed deviation).
    clone
}

/// `_html_to_clean_md` (`ai_chat_parser.py:123-135`).
pub fn html_to_clean_md(html_str: &str) -> String {
    if html_str.is_empty() {
        return String::new();
    }
    let md = hr::markdown_of(&hr::parse_html(html_str));
    py_strip(&collapse_blank_runs(&md))
}

/// `_html_to_clean_md(_clean_html_node(node))` as one step.
fn clean_node_to_md(node: &hr::Element) -> String {
    let md = hr::markdown_of(&clean_html_node(node));
    py_strip(&collapse_blank_runs(&md))
}

/// `re.sub(r'\n{3,}', '\n\n', md)`.
fn collapse_blank_runs(md: &str) -> String {
    let mut out = String::with_capacity(md.len());
    let mut newline_run = 0usize;
    for c in md.chars() {
        if c == '\n' {
            newline_run += 1;
            continue;
        }
        if newline_run > 0 {
            out.push_str(&"\n".repeat(if newline_run >= 3 { 2 } else { newline_run }));
            newline_run = 0;
        }
        out.push(c);
    }
    if newline_run > 0 {
        out.push_str(&"\n".repeat(if newline_run >= 3 { 2 } else { newline_run }));
    }
    out
}

/// Python `str.strip()` (Unicode whitespace, both ends).
fn py_strip(s: &str) -> String {
    s.trim().to_string()
}

// ------------------------------------------------------ _clean_chat_tokens

const ENTITY_ANCHOR: &str = "\u{e200}entity\u{e202}[";
const ENTITY_CLOSER: &str = "]\u{e201}";
const CITE_ANCHOR: &str = "\u{e200}cite\u{e202}";
const URL_ANCHOR: &str = "\u{e200}url\u{e202}";
const PUA_END: &str = "\u{e201}";

/// `_clean_chat_tokens` (`ai_chat_parser.py:138-145`).
pub fn clean_chat_tokens(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    let text = sub_lazy_single_line(text, ENTITY_ANCHOR, ENTITY_CLOSER);
    let text = sub_lazy_single_line(&text, CITE_ANCHOR, PUA_END);
    let text = sub_lazy_single_line(&text, URL_ANCHOR, PUA_END);
    py_strip(&text)
}

/// `re.sub(anchor + r'.*?' + closer, '', text)` with **no** `re.S`: the lazy
/// `.*?` cannot cross a newline.  Leftmost match, shortest tail, non
/// overlapping — exactly the `re.sub` scan.
fn sub_lazy_single_line(text: &str, anchor: &str, closer: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let a: Vec<char> = anchor.chars().collect();
    let c: Vec<char> = closer.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0usize;
    'outer: while i < chars.len() {
        if i + a.len() <= chars.len() && chars[i..i + a.len()] == a[..] {
            let mut j = i + a.len();
            while j + c.len() <= chars.len() {
                if chars[j] == '\n' {
                    break;
                }
                if chars[j..j + c.len()] == c[..] {
                    i = j + c.len();
                    continue 'outer;
                }
                j += 1;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

// -------------------------------------------------------- _detect_node_role

/// `_detect_node_role` (`ai_chat_parser.py:148-176`).
pub fn detect_node_role(node: &hr::Element) -> &'static str {
    let tag_name = node.name.to_lowercase();
    let classes = joined_classes(node).to_lowercase();
    // `node.get('a') or node.get('b') or node.get('c') or ''`: an *empty*
    // attribute value is falsy and falls through to the next candidate.
    let first_nonempty_attr = ["data-message-author-role", "data-role", "data-testid"]
        .iter()
        .map(|k| node.attr(k).unwrap_or(""))
        .find(|v| !v.is_empty())
        .unwrap_or("");
    let role_attr = first_nonempty_attr.to_lowercase();

    if ["user", "human", "prompt"].iter().any(|w| role_attr.contains(w)) {
        return "user";
    }
    if ["assistant", "model", "claude", "bot", "gemini"].iter().any(|w| role_attr.contains(w)) {
        return "assistant";
    }

    if matches!(tag_name.as_str(), "user-query" | "user-message" | "prompt") {
        return "user";
    }
    if matches!(tag_name.as_str(), "model-response" | "claude-message" | "assistant-message") {
        return "assistant";
    }

    if !find_all(node, Some(&["user-query", "div"]), &|el| {
        class_matches(el, &["user-query", "query-content", "user-prompt"])
    })
    .is_empty()
    {
        return "user";
    }
    if !find_all(node, Some(&["model-response", "div"]), &|el| {
        class_matches(el, &["model-response", "response-content"])
    })
    .is_empty()
    {
        return "assistant";
    }

    if ["user-query", "query-content", "user-turn", "user-prompt", "user-message", "human"]
        .iter()
        .any(|w| classes.contains(w))
    {
        return "user";
    }
    if ["model-response", "response-content", "model-turn", "response-container", "assistant"]
        .iter()
        .any(|w| classes.contains(w))
    {
        return "assistant";
    }
    "assistant"
}

// --------------------------------------------- _unflatten_turbo_stream (arena)

/// One arena node.  The slot index stands in for Python's `id()`: memoized
/// containers keep one slot (object identity), which is what lets shared
/// subtrees stay shared and cycles terminate.
#[derive(Debug, Clone)]
pub enum Slot {
    Leaf(Pj),
    Arr(Vec<usize>),
    Obj(Vec<(String, usize)>),
}

/// A resolved value: either an arena node or an inline scalar produced by an
/// iteration that Python would materialise on the fly (dict keys, str chars).
#[derive(Debug, Clone)]
pub enum Found {
    Slot(usize),
    Leaf(Pj),
}

impl Found {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Found::Leaf(Pj::Str(s)) => Some(s.as_str()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TurboArena {
    slots: Vec<Slot>,
    root: usize,
}

/// `_unflatten_turbo_stream` (`ai_chat_parser.py:179-221`).
pub fn unflatten_turbo_stream(arr: &[Pj]) -> TurboArena {
    let mut slots: Vec<Slot> = Vec::new();
    let mut memo: Vec<Option<usize>> = vec![None; arr.len()];
    let root = resolve(arr, &Pj::Int("0".into()), &mut slots, &mut memo);
    TurboArena { slots, root }
}

fn push_slot(slots: &mut Vec<Slot>, s: Slot) -> usize {
    slots.push(s);
    slots.len() - 1
}

/// `resolve(idx)` — `isinstance(idx, int)` with Python's bool-is-int rule;
/// every other JSON type is returned *as is*.
fn resolve(arr: &[Pj], key: &Pj, slots: &mut Vec<Slot>, memo: &mut Vec<Option<usize>>) -> usize {
    match key {
        Pj::Bool(b) => resolve_index(arr, if *b { 1 } else { 0 }, slots, memo),
        Pj::Int(s) => match s.parse::<i64>() {
            Ok(i) => resolve_index(arr, i, slots, memo),
            // An integer that cannot fit i64 is always out of range for any
            // array that could have been parsed into memory: `resolve` -> None.
            Err(_) => push_slot(slots, Slot::Leaf(Pj::Null)),
        },
        // Not an int: Python's `resolve` returns it *as is*.  A leaked raw
        // container still gets its own arena identity so `_find_key_recursive`
        // can walk into it, but its members are never re-indexed: ints nested
        // in a raw list stay raw ints, and a leaked dict keeps literal
        // `_`-prefixed keys and raw values (only indexed values were flat).
        Pj::Arr(items) => {
            let slot = push_slot(slots, Slot::Arr(Vec::new()));
            let children: Vec<usize> = items
                .iter()
                .map(|it| push_slot(slots, Slot::Leaf(it.clone())))
                .collect();
            if let Slot::Arr(target) = &mut slots[slot] {
                *target = children;
            }
            slot
        }
        Pj::Obj(fields) => {
            let slot = push_slot(slots, Slot::Obj(Vec::new()));
            let children: Vec<(String, usize)> = fields
                .iter()
                .map(|(k, v)| (k.clone(), push_slot(slots, Slot::Leaf(v.clone()))))
                .collect();
            if let Slot::Obj(target) = &mut slots[slot] {
                *target = children;
            }
            slot
        }
        other => push_slot(slots, Slot::Leaf(other.clone())),
    }
}

fn resolve_index(
    arr: &[Pj],
    idx: i64,
    slots: &mut Vec<Slot>,
    memo: &mut Vec<Option<usize>>,
) -> usize {
    if idx < 0 || idx as usize >= arr.len() {
        return push_slot(slots, Slot::Leaf(Pj::Null));
    }
    let i = idx as usize;
    if let Some(hit) = memo[i] {
        return hit;
    }
    match &arr[i] {
        Pj::Arr(items) => {
            let slot = push_slot(slots, Slot::Arr(Vec::new()));
            memo[i] = Some(slot);
            let children: Vec<usize> =
                items.iter().map(|it| resolve(arr, it, slots, memo)).collect();
            if let Slot::Arr(target) = &mut slots[slot] {
                *target = children;
            }
            slot
        }
        Pj::Obj(fields) => {
            let slot = push_slot(slots, Slot::Obj(Vec::new()));
            memo[i] = Some(slot);
            let mut out: Vec<(String, usize)> = Vec::new();
            for (k, v) in fields {
                if let Some(rest) = k.strip_prefix('_') {
                    // `int(k[1:])` — ValueError drops the pair silently.
                    if let Some(key_idx) = py_int_str(rest) {
                        // resolve(key_idx) *before* resolve(v), Python order.
                        let name_slot = resolve_index(arr, key_idx, slots, memo);
                        let val_slot = resolve(arr, v, slots, memo);
                        if let Slot::Leaf(Pj::Str(name)) = &slots[name_slot] {
                            obj_insert(&mut out, name.clone(), val_slot);
                        }
                        // key_name not a str (None / int / list): entry dropped.
                    }
                } else {
                    let s = resolve(arr, v, slots, memo);
                    obj_insert(&mut out, k.clone(), s);
                }
            }
            if let Slot::Obj(target) = &mut slots[slot] {
                *target = out;
            }
            slot
        }
        // int / float / bool / str / None: returned as-is (not memoized).
        other => push_slot(slots, Slot::Leaf(other.clone())),
    }
}

/// Python dict assignment: first-occurrence key position, last value.
fn obj_insert(fields: &mut Vec<(String, usize)>, key: String, slot: usize) {
    match fields.iter_mut().find(|(k, _)| *k == key) {
        Some(pos) => pos.1 = slot,
        None => fields.push((key, slot)),
    }
}

/// CPython `int(str)` for the shapes Turbo-Stream keys take: surrounding
/// whitespace, optional sign, ASCII digits with `_` only between digits.
fn py_int_str(s: &str) -> Option<i64> {
    let t = s.trim();
    let (neg, rest) = if let Some(r) = t.strip_prefix('-') {
        (true, r)
    } else if let Some(r) = t.strip_prefix('+') {
        (false, r)
    } else {
        (false, t)
    };
    let chars: Vec<char> = rest.chars().collect();
    if chars.is_empty() {
        return None;
    }
    let mut digits = String::new();
    let mut prev_digit = false;
    for (i, c) in chars.iter().enumerate() {
        match c {
            '_' => {
                if !prev_digit || i + 1 >= chars.len() || !chars[i + 1].is_ascii_digit() {
                    return None;
                }
                prev_digit = false;
            }
            d if d.is_ascii_digit() => {
                digits.push(*d);
                prev_digit = true;
            }
            _ => return None,
        }
    }
    if !prev_digit {
        return None;
    }
    let v: i64 = digits.parse().ok()?;
    Some(if neg { -v } else { v })
}

impl TurboArena {
    pub fn root(&self) -> usize {
        self.root
    }
    pub fn slot(&self, s: usize) -> &Slot {
        &self.slots[s]
    }
    pub fn as_arr(&self, s: usize) -> Option<&[usize]> {
        match &self.slots[s] {
            Slot::Arr(v) => Some(v),
            _ => None,
        }
    }
    pub fn as_obj(&self, s: usize) -> Option<&[(String, usize)]> {
        match &self.slots[s] {
            Slot::Obj(v) => Some(v),
            _ => None,
        }
    }
    pub fn leaf(&self, s: usize) -> Option<&Pj> {
        match &self.slots[s] {
            Slot::Leaf(p) => Some(p),
            _ => None,
        }
    }
    pub fn is_null(&self, s: usize) -> bool {
        matches!(&self.slots[s], Slot::Leaf(Pj::Null))
    }
    /// `bool(value)` for the Python `or` chains and truthiness gates.
    pub fn truthy(&self, s: usize) -> bool {
        match &self.slots[s] {
            Slot::Leaf(p) => p.truthy(),
            Slot::Arr(v) => !v.is_empty(),
            Slot::Obj(v) => !v.is_empty(),
        }
    }
    /// `obj.get(key)` when obj is a dict; `None` = missing key.
    pub fn obj_get(&self, s: usize, key: &str) -> Option<usize> {
        self.as_obj(s)?.iter().find(|(k, _)| k == key).map(|(_, v)| *v)
    }
    pub fn export(&self, s: usize) -> Found {
        match &self.slots[s] {
            Slot::Leaf(p) => Found::Leaf(p.clone()),
            _ => Found::Slot(s),
        }
    }
    /// `str(value)` / `repr(container)` through the arena.
    pub fn py_str(&self, s: usize) -> String {
        self.py_repr(s)
    }
    pub fn py_repr(&self, s: usize) -> String {
        match &self.slots[s] {
            Slot::Leaf(p) => py_str(p),
            Slot::Arr(items) => {
                let inner: Vec<String> = items.iter().map(|i| self.py_repr(*i)).collect();
                format!("[{}]", inner.join(", "))
            }
            Slot::Obj(fields) => {
                let inner: Vec<String> = fields
                    .iter()
                    .map(|(k, v)| format!("{}: {}", py_str_repr(k), self.py_repr(*v)))
                    .collect();
                format!("{{{}}}", inner.join(", "))
            }
        }
    }
    /// `for x in value`: lists yield slots, dicts yield their keys as
    /// strings, strings yield characters; scalars raise `TypeError`.
    pub fn iter_value(&self, s: usize) -> PRes<Vec<Found>> {
        match &self.slots[s] {
            Slot::Arr(items) => Ok(items.iter().map(|i| self.export(*i)).collect()),
            Slot::Obj(fields) => {
                Ok(fields.iter().map(|(k, _)| Found::Leaf(Pj::Str(k.clone()))).collect())
            }
            Slot::Leaf(p) => match p {
                Pj::Str(text) => {
                    Ok(text.chars().map(|c| Found::Leaf(Pj::Str(c.to_string()))).collect())
                }
                _ => Err(Raised),
            },
        }
    }
    pub fn found_truthy(&self, f: &Found) -> bool {
        match f {
            Found::Slot(s) => self.truthy(*s),
            Found::Leaf(p) => p.truthy(),
        }
    }
    pub fn found_str(&self, f: &Found) -> String {
        match f {
            Found::Slot(s) => self.py_str(*s),
            Found::Leaf(p) => py_str(p),
        }
    }

    /// `_find_key_recursive(obj, target_key)` (`ai_chat_parser.py:224-245`).
    /// `visited` is shared across the whole call; a hit whose value is `None`
    /// still aborts that dict's child scan but reads as "not found" upward.
    pub fn find_key(&self, key: &str) -> Option<usize> {
        let mut visited: HashSet<usize> = HashSet::new();
        let found = self.find_in(key, self.root, &mut visited);
        found.filter(|s| !self.is_null(*s))
    }

    fn find_in(&self, key: &str, slot: usize, visited: &mut HashSet<usize>) -> Option<usize> {
        if !visited.insert(slot) {
            return None;
        }
        match &self.slots[slot] {
            Slot::Obj(fields) => {
                if let Some((_, v)) = fields.iter().find(|(k, _)| k == key) {
                    return Some(*v);
                }
                for (_, v) in fields.iter() {
                    if let Some(r) = self.find_in(key, *v, visited) {
                        if !self.is_null(r) {
                            return Some(r);
                        }
                    }
                }
            }
            Slot::Arr(items) => {
                for i in items.iter() {
                    if let Some(r) = self.find_in(key, *i, visited) {
                        if !self.is_null(r) {
                            return Some(r);
                        }
                    }
                }
            }
            Slot::Leaf(_) => {}
        }
        None
    }
}

// ------------------------------------------------------------------ results

/// One `{'role': ..., 'text': ...}` turn as every Python parser emits it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatTurn {
    pub role: String,
    pub text: String,
}

/// The `{'title': ..., 'turns': [...]}` dict the parsers return (or `None`).
#[derive(Debug, Clone)]
pub struct ChatData {
    pub title: Pj,
    pub turns: Vec<ChatTurn>,
}

/// `x or y or z` over borrowed dict values: the first *truthy* one.
fn or_borrowed<'a>(opts: &[Option<&'a Pj>], default: &'a Pj) -> &'a Pj {
    for opt in opts.iter().flatten() {
        if opt.truthy() {
            return opt;
        }
    }
    default
}

// ------------------------------------------------------- parse_chatgpt_json

/// `parse_chatgpt_json(html)` (`ai_chat_parser.py:248-354`).  Each strategy
/// sits in its own `try/except Exception: pass`, so a raise only aborts that
/// strategy.
pub fn parse_chatgpt_json(html: &str) -> Option<ChatData> {
    let doc = hr::parse_html(html);
    let mut title: Pj = Pj::Str("ChatGPT 对话".to_string());
    if let Some(t) = title_string(&doc) {
        let clean_t = py_strip(&t.replace("ChatGPT - ", "").replace(" - ChatGPT", ""));
        if !clean_t.is_empty() {
            title = Pj::Str(clean_t);
        }
    }

    // 策略 1: React Router / Remix Turbo-Stream
    let mut raw_chunks: Vec<String> = Vec::new();
    for s in find_all(&doc, Some(&["script"]), &|_| true) {
        let st = script_text(s);
        if st.contains("streamController.enqueue") {
            for part in st.split("streamController.enqueue(").skip(1) {
                if let Some(idx) = part.rfind(')') {
                    // `part[:idx]` — `)` is ASCII, so the byte offset is a
                    // char boundary by construction.
                    let inner = py_strip(&part[..idx]);
                    if inner.starts_with('"') && inner.ends_with('"') {
                        // `json.loads` of text that starts with `"` can only
                        // produce a str or fail, so `"".join` below can never
                        // raise the way Python's TypeError guard suggests.
                        if let Ok(Pj::Str(chunk)) = pyjson::parse(&inner) {
                            raw_chunks.push(chunk);
                        }
                    }
                }
            }
        }
    }
    if !raw_chunks.is_empty() {
        let full_stream = raw_chunks.concat();
        let line0 = full_stream.split('\n').next().unwrap_or("");
        if let Ok(Some(data)) = chatgpt_strategy1(line0, &mut title) {
            return Some(data);
        }
    }

    // 策略 2: 传统 Next.js __NEXT_DATA__ JSON
    let script_src = find_all(&doc, Some(&["script"]), &|el| {
        el.attr("id") == Some("__NEXT_DATA__")
    })
    .into_iter()
    .next()
    .and_then(|s| first_child_string(s).map(|t| t.to_string()));
    if let Some(src) = script_src {
        if let Ok(Some(data)) = pyjson::parse(&src)
            .map_err(|_| Raised)
            .and_then(|v| chatgpt_strategy2(v, &title))
        {
            return Some(data);
        }
    }
    None
}

/// Everything under the first `try` of `parse_chatgpt_json`, lines 276-306.
/// `soup_title` is the shared local `title` and is updated in place exactly
/// where Python does (`title = stream_title.strip()`), so a later `Raised`
/// still keeps that mutation — matching the swallowed `except`.
fn chatgpt_strategy1(line0: &str, title: &mut Pj) -> PRes<Option<ChatData>> {
    let raw_array = pyjson::parse(line0).map_err(|_| Raised)?;
    let arr = match &raw_array {
        Pj::Arr(items) => items,
        _ => return Ok(None), // isinstance(raw_array, list) gate
    };
    let arena = unflatten_turbo_stream(arr);

    // `find('pageTitle') or find('title')` — a falsy first hit falls through.
    let stream_title: Option<usize> = arena
        .find_key("pageTitle")
        .filter(|s| arena.truthy(*s))
        .or_else(|| arena.find_key("title"));
    if let Some(s) = stream_title {
        if let Some(Pj::Str(st)) = arena.leaf(s) {
            let stripped = py_strip(st);
            if !stripped.is_empty() {
                *title = Pj::Str(stripped);
            }
        }
    }

    let linear = arena.find_key("linear_conversation");
    if let Some(lin) = linear {
        if arena.truthy(lin) && arena.as_arr(lin).is_some() {
            let turns = chatgpt_turns_from_arena(&arena, lin)?;
            if !turns.is_empty() {
                return Ok(Some(ChatData { title: title.clone(), turns }));
            }
        }
    }
    Ok(None)
}

/// Turn extraction over the unflattened graph, lines 286-304.
fn chatgpt_turns_from_arena(arena: &TurboArena, linear_slot: usize) -> PRes<Vec<ChatTurn>> {
    let mut turns: Vec<ChatTurn> = Vec::new();
    for item in arena.iter_value(linear_slot)? {
        // `if isinstance(item, dict)` — scalars and lists are skipped silently.
        let Found::Slot(item_obj) = item else { continue };
        if arena.as_obj(item_obj).is_none() {
            continue;
        }
        // `msg = item.get('message') or item`
        let msg_slot = match arena.obj_get(item_obj, "message") {
            Some(m) if arena.truthy(m) => {
                if arena.as_obj(m).is_none() {
                    return Err(Raised); // msg.get('author') on a non-dict
                }
                m
            }
            _ => item_obj,
        };
        // `author = msg.get('author') or {}` then role only for dicts.
        let author_obj = match arena.obj_get(msg_slot, "author") {
            Some(a) if arena.truthy(a) && arena.as_obj(a).is_some() => Some(a),
            _ => None,
        };
        let role_slot = author_obj.and_then(|a| arena.obj_get(a, "role"));
        let role: Option<&str> = role_slot
            .and_then(|r| arena.leaf(r))
            .and_then(|p| match p {
                Pj::Str(s) => Some(s.as_str()),
                _ => None,
            });
        // `content = msg.get('content') or {}`; parts only when it is a dict.
        let parts_slot = match arena.obj_get(msg_slot, "content") {
            Some(c) if arena.truthy(c) => arena.obj_get(c, "parts").filter(|p| arena.truthy(*p)),
            _ => None,
        };
        let mut text_parts: Vec<String> = Vec::new();
        if let Some(parts) = parts_slot {
            for p in arena.iter_value(parts)? {
                match &p {
                    Found::Slot(s) => match arena.slot(*s) {
                        // `isinstance(p, (str, int, float))` — bool is an int.
                        Slot::Leaf(pj @ (Pj::Str(_) | Pj::Int(_) | Pj::Float(_) | Pj::Bool(_))) => {
                            text_parts.push(clean_chat_tokens(&py_str(pj)));
                        }
                        Slot::Obj(_) => {
                            if let Some(t) = arena.obj_get(*s, "text").filter(|t| arena.truthy(*t))
                            {
                                text_parts.push(clean_chat_tokens(&arena.py_str(t)));
                            }
                        }
                        _ => {}
                    },
                    // Raw values exported from a slot: Python's isinstance
                    // gates decide — scalars via `str(p)`, dicts through
                    // `elif isinstance(p, dict) and p.get('text')`, lists and
                    // None skipped.  Dict keys / string chars are scalars too.
                    Found::Leaf(pj @ (Pj::Str(_) | Pj::Int(_) | Pj::Float(_) | Pj::Bool(_))) => {
                        text_parts.push(clean_chat_tokens(&py_str(pj)));
                    }
                    Found::Leaf(Pj::Obj(fields)) => {
                        if let Some(t) = fields.iter().find(|(k, _)| k == "text").map(|(_, v)| v) {
                            if t.truthy() {
                                text_parts.push(clean_chat_tokens(&py_str(t)));
                            }
                        }
                    }
                    Found::Leaf(_) => {}
                }
            }
        }
        let final_text = py_strip(&text_parts.join("\n\n"));
        if matches!(role, Some("user") | Some("assistant")) && !final_text.is_empty() {
            turns.push(ChatTurn { role: role.unwrap().to_string(), text: final_text });
        }
    }
    Ok(turns)
}

/// Everything under the second `try` of `parse_chatgpt_json`, lines 311-352.
fn chatgpt_strategy2(data: Pj, soup_title: &Pj) -> PRes<Option<ChatData>> {
    let page_props = pj_get_dict_field(&data, "props").and_then(|p| pj_get_dict_field(p, "pageProps"))?;
    let server_resp =
        pj_get_dict_field(page_props, "serverResponse").and_then(|s| pj_get_dict_field(s, "data"))?;
    let title = or_borrowed(
        &[pj_get(page_props, "title"), pj_get(server_resp, "title")],
        soup_title,
    )
    .clone();

    let mut turns: Vec<ChatTurn> = Vec::new();
    let linear_conv = pj_get(server_resp, "linear_conversation");
    if matches!(linear_conv, Some(Pj::Arr(_))) {
        for item in pj_iter(linear_conv.unwrap())? {
            // No isinstance gate here (unlike strategy 1): a non-dict item
            // makes `item.get(...)` raise and aborts the whole strategy.
            let msg = pj_get_dict_field(&item, "message")?;
            let author = pj_get_dict_field(msg, "author")?;
            let role = pj_get(author, "role");
            if !(eq_str(role, "user") || eq_str(role, "assistant")) {
                continue;
            }
            let content_obj = pj_get_dict_field(msg, "content")?;
            let parts = pj_get(content_obj, "parts").unwrap_or(&EMPTY_ARR);
            let text = py_strip(&scalar_parts_joined(parts)?);
            if !text.is_empty() {
                let role_str = if eq_str(role, "user") { "user" } else { "assistant" };
                turns.push(ChatTurn { role: role_str.to_string(), text });
            }
        }
    } else if pj_has(server_resp, "mapping") {
        let mapping = pj_get(server_resp, "mapping").expect("member checked");
        let nodes: &[(String, Pj)] = match mapping {
            Pj::Obj(fields) => fields,
            _ => return Err(Raised), // mapping.values() on a non-dict
        };
        for (_, node) in nodes {
            // Python 336-338 is the *bare* `msg = node.get('message')` /
            // `if not msg: continue` — no `{}` default.  An absent key and a
            // present falsy value (including explicit `null`) both `continue`;
            // only a present truthy non-dict reaches `msg.get(...)` and raises.
            let msg_opt = match node {
                Pj::Obj(_) => match pj_get(node, "message") {
                    None => continue,
                    Some(v) if !v.truthy() => continue,
                    Some(v @ Pj::Obj(_)) => v,
                    Some(_) => return Err(Raised),
                },
                _ => return Err(Raised), // node.get(...) -> AttributeError
            };
            if !msg_opt.truthy() {
                continue;
            }
            let author = pj_get_dict_field(msg_opt, "author")?;
            let role = pj_get(author, "role");
            if !(eq_str(role, "user") || eq_str(role, "assistant")) {
                continue;
            }
            let content_obj = pj_get_dict_field(msg_opt, "content")?;
            let parts = pj_get(content_obj, "parts").unwrap_or(&EMPTY_ARR);
            let text = py_strip(&scalar_parts_joined(parts)?);
            if !text.is_empty() {
                let role_str = if eq_str(role, "user") { "user" } else { "assistant" };
                turns.push(ChatTurn { role: role_str.to_string(), text });
            }
        }
    }
    if !turns.is_empty() {
        return Ok(Some(ChatData { title, turns }));
    }
    Ok(None)
}

/// `'\n\n'.join(_clean_chat_tokens(str(p)) for p in parts if isinstance(p, (str, int, float)))`
/// — bool passes the isinstance test and renders as `True`/`False`.
fn scalar_parts_joined(parts: &Pj) -> PRes<String> {
    let mut tp: Vec<String> = Vec::new();
    for p in pj_iter(parts)? {
        match &p {
            Pj::Str(_) | Pj::Int(_) | Pj::Float(_) | Pj::Bool(_) => {
                tp.push(clean_chat_tokens(&py_str(&p)));
            }
            _ => {}
        }
    }
    Ok(tp.join("\n\n"))
}

// --------------------------------------------------------- parse_claude_json

/// `parse_claude_json(html)` (`ai_chat_parser.py:357-385`).  The whole body
/// past the soup build is one `try/except Exception: pass`.
pub fn parse_claude_json(html: &str) -> Option<ChatData> {
    let doc = hr::parse_html(html);
    let src = find_all(&doc, Some(&["script"]), &|el| el.attr("id") == Some("__NEXT_DATA__"))
        .into_iter()
        .next()
        .and_then(|s| first_child_string(s).map(|t| t.to_string()))?;
    claude_inner(&src).ok().flatten()
}

fn claude_inner(src: &str) -> PRes<Option<ChatData>> {
    let data = pyjson::parse(src).map_err(|_| Raised)?;
    let page_props = pj_get_dict_field(&data, "props").and_then(|p| pj_get_dict_field(p, "pageProps"))?;
    let chat = or_borrowed(
        &[pj_get(page_props, "chat"), pj_get(page_props, "sharedConversation")],
        &EMPTY_OBJ,
    );
    let claude_default_title = Pj::Str("Claude 对话".to_string());
    let title = or_borrowed(
        &[
            pj_get(chat, "name"),
            pj_get(chat, "title"),
            pj_get(page_props, "title"),
        ],
        &claude_default_title,
    )
    .clone();
    let raw_msgs = or_borrowed(
        &[pj_get(chat, "chat_messages"), pj_get(chat, "transcript")],
        &EMPTY_ARR,
    );

    let mut turns: Vec<ChatTurn> = Vec::new();
    for msg in pj_iter(raw_msgs)? {
        // `msg.get(...)` — a non-dict message raises and kills the parse.
        if !matches!(msg, Pj::Obj(_)) {
            return Err(Raised);
        }
        let sender = or_borrowed(&[pj_get(&msg, "sender"), pj_get(&msg, "role")], &NULL_VAL);
        let mut text: Pj = pj_get(&msg, "text")
            .filter(|t| t.truthy())
            .map(|t| t.clone())
            .unwrap_or_else(|| Pj::Str(String::new()));
        if !text.truthy() && pj_has(&msg, "content") {
            let content = pj_get(&msg, "content").expect("member checked");
            for c in pj_iter(content)? {
                if matches!(&c, Pj::Obj(_)) && eq_str(pj_get(&c, "type"), "text") {
                    // `text += c.get('text', '') + '\n'` — str concatenation
                    // raises unless both sides are strings.
                    let piece = match pj_get(&c, "text").unwrap_or(&EMPTY_STR) {
                        Pj::Str(s) => s.clone(),
                        _ => return Err(Raised),
                    };
                    match &mut text {
                        Pj::Str(acc) => acc.push_str(&format!("{piece}\n")),
                        _ => return Err(Raised),
                    }
                }
            }
        }
        // Python evaluates the sender membership first; `text.strip()` is
        // only reached (and can only raise) for a recognised sender.
        let is_user = eq_str(Some(sender), "human") || eq_str(Some(sender), "user");
        let is_assistant = eq_str(Some(sender), "assistant") || eq_str(Some(sender), "claude");
        if is_user || is_assistant {
            let stripped = match &text {
                Pj::Str(s) => py_strip(s),
                _ => return Err(Raised),
            };
            if !stripped.is_empty() {
                turns.push(ChatTurn {
                    role: if is_user { "user" } else { "assistant" }.to_string(),
                    text: clean_chat_tokens(&stripped),
                });
            }
        }
    }
    if !turns.is_empty() {
        return Ok(Some(ChatData { title, turns }));
    }
    Ok(None)
}

// --------------------------------------------------- parse_gemini_dom_or_json

/// `parse_gemini_dom_or_json(html)` (`ai_chat_parser.py:388-477`).  Four
/// strategies; each is guarded by `if not turns:` so an earlier hit short
/// circuits the later ones.  Only strategy 3 sits in a `try` — the DOM
/// strategies cannot raise on `str` inputs here, so the public fn never
/// raises.
pub fn parse_gemini_dom_or_json(html: &str) -> Option<ChatData> {
    let doc = hr::parse_html(html);

    let mut title = String::new();
    if let Some(t) = title_string(&doc) {
        // `soup.title.string.replace(' - Gemini','').replace('Gemini - ','')
        //  .replace('\u200eGemini - ','').strip()` (order preserved).
        title = py_strip(
            &t.replace(" - Gemini", "")
                .replace("Gemini - ", "")
                .replace("\u{200e}Gemini - ", ""),
        );
    }
    // `if not title or title.lower() in ('gemini','live content',
    //  '直接体验 google ai 黑科技')`.
    let lowered = title.to_lowercase();
    if title.is_empty()
        || matches!(lowered.as_str(), "gemini" | "live content" | "直接体验 google ai 黑科技")
    {
        title = "Gemini 对话".to_string();
    }

    let mut turns: Vec<ChatTurn> = Vec::new();

    // 策略 1: 扫描具体对话节点与 Web Components.
    let query_and_response = find_all(&doc, Some(&["user-query", "model-response"]), &|_| true);
    if !query_and_response.is_empty() {
        for node in query_and_response {
            let role = if node.name == "user-query" { "user" } else { "assistant" };
            let md_content = clean_node_to_md(node);
            if !md_content.is_empty() {
                turns.push(ChatTurn { role: role.to_string(), text: clean_chat_tokens(&md_content) });
            }
        }
    }

    // 策略 2: 扫描具体对话外层容器.
    if turns.is_empty() {
        let all_turns = find_all(
            &doc,
            Some(&["div", "article", "section"]),
            &|el| {
                class_matches(
                    el,
                    &[
                        "user-query-container",
                        "response-container",
                        "query-content",
                        "response-content",
                        "user-turn",
                        "model-turn",
                        "chat-turn",
                        "conversation-turn",
                    ],
                )
            },
        );
        for node in all_turns {
            let role = detect_node_role(node);
            let md_content = clean_node_to_md(node);
            // `if md_content and len(md_content) > 1` — `len` is a *char*
            // count on the Python str.
            if !md_content.is_empty() && md_content.chars().count() > 1 {
                // Quirk kept verbatim: the dedupe compares the previous
                // turn's *cleaned* text to this node's *raw* md_content.
                if turns.is_empty() || turns[turns.len() - 1].text != md_content {
                    turns.push(ChatTurn {
                        role: role.to_string(),
                        text: clean_chat_tokens(&md_content),
                    });
                }
            }
        }
    }

    // 策略 3: 解析 Google WIZ_global_data 中的对话字段.
    if turns.is_empty() {
        let mut target_script: Option<String> = None;
        for s in find_all(&doc, Some(&["script"]), &|_| true) {
            let st = script_text(s);
            if st.contains("window.WIZ_global_data") {
                target_script = Some(st);
                break;
            }
        }
        if let Some(target_script) = target_script {
            // `find("{")`/`rfind("}")` — ASCII, so byte offsets are char
            // boundaries; `start != -1 and end != -1` gate.
            let start = target_script.find('{');
            let end = target_script.rfind('}');
            if let (Some(start), Some(end)) = (start, end) {
                let wiz = gemini_wiz_strategy(&target_script[start..=end], &mut title, &mut turns);
                let _ = wiz; // strategy only appends; never returns early
            }
        }
    }

    // 策略 4: 扫描具有 data-message-author-role 标识的元素.
    if turns.is_empty() {
        let nodes = find_all(&doc, Some(&["div", "article", "section"]), &|el| {
            el.attr("data-message-author-role").is_some()
        });
        for node in nodes {
            let role_attr = node.attr("data-message-author-role").unwrap_or("").to_lowercase();
            let md = clean_node_to_md(node);
            if !md.is_empty() {
                let role = if role_attr == "user" { "user" } else { "assistant" };
                turns.push(ChatTurn { role: role.to_string(), text: clean_chat_tokens(&md) });
            }
        }
    }

    if !turns.is_empty() {
        return Some(ChatData { title: Pj::Str(title), turns });
    }
    None
}

/// The `try` body of Gemini strategy 3 (`ai_chat_parser.py:431-465`).  Any
/// JSON/shape error is swallowed (the `except Exception: pass`); a `∞`/`∰`
/// section appends a user+assistant pair.  Returns `Ok(())` on the happy
/// path and `Err(Raised)` to model the swallowed exception.
fn gemini_wiz_strategy(body: &str, title: &mut String, turns: &mut Vec<ChatTurn>) -> PRes<()> {
    let wiz = pyjson::parse(body).map_err(|_| Raised)?;
    let fields = match &wiz {
        Pj::Obj(fields) => fields,
        // `wiz.items()` on a non-dict raises AttributeError -> except pass.
        _ => return Err(Raised),
    };
    for (_k, v) in fields {
        // `isinstance(v, str) and "∞" in v and len(v) > 50`.
        let v = match v {
            Pj::Str(s) => s,
            _ => continue,
        };
        if !v.contains('\u{221e}') || v.chars().count() <= 50 {
            continue;
        }
        // `sections = v.split("∰")`.
        for section in v.split('\u{2230}') {
            let sec = py_strip(section);
            if sec.is_empty() || !sec.contains('\u{221e}') {
                continue;
            }
            let parts: Vec<String> =
                sec.split('\u{221e}').map(py_strip).filter(|p| !p.is_empty()).collect();
            let mut user_parts: Vec<String> = Vec::new();
            let mut image_parts: Vec<String> = Vec::new();
            let mut response_parts: Vec<String> = Vec::new();
            for p in parts {
                if p.starts_with("http://") || p.starts_with("https://") {
                    image_parts.push(format!("![Gemini Image]({p})"));
                } else if response_parts.is_empty() && image_parts.is_empty() {
                    user_parts.push(p);
                } else {
                    response_parts.push(p);
                }
            }
            let user_text = py_strip(&user_parts.join("\n\n"));
            let mut response_text = py_strip(&response_parts.join("\n\n"));
            if !image_parts.is_empty() {
                let head = image_parts.join("\n\n");
                response_text =
                    if response_text.is_empty() { head } else { format!("{head}\n\n{response_text}") };
            }
            if !user_text.is_empty() && !response_text.is_empty() {
                if title.as_str() == "Gemini 对话" {
                    let first_line = py_strip(user_text.split('\n').next().unwrap_or(""));
                    if !first_line.is_empty() && first_line.chars().count() < 60 {
                        *title = first_line;
                    }
                }
                turns.push(ChatTurn { role: "user".to_string(), text: clean_chat_tokens(&user_text) });
                turns.push(ChatTurn {
                    role: "assistant".to_string(),
                    text: clean_chat_tokens(&response_text),
                });
            }
        }
    }
    Ok(())
}

// -------------------------------------------------- parse_generic_ai_chat_dom

/// `parse_generic_ai_chat_dom(html, url='')` (`ai_chat_parser.py:480-509`).
/// `url` is accepted for signature parity but never read in the Python body.
pub fn parse_generic_ai_chat_dom(html: &str, _url: &str) -> Option<ChatData> {
    let doc = hr::parse_html(html);

    let raw_title = match title_string(&doc) {
        Some(t) => py_strip(&t),
        None => "AI 对话记录".to_string(),
    };
    // `re.sub(r'\s*[-_|]\s*(Gemini|ChatGPT|...).*$', '', title, re.I).strip()`.
    // `re.I`; `.` cannot cross `\n` (no re.S); Python `$` also matches just
    // before a single trailing newline — mirrored with `.*\n?\z` (safe: the
    // result is `.strip()`ed, so the at-most-one trailing newline difference
    // from the regex crate's end-of-text `$` is unobservable).
    let title = py_strip(&strip_generic_title_suffix(&raw_title));

    let mut turns: Vec<ChatTurn> = Vec::new();

    // 1. 查找具体角色节点 — bs4 `find_all(callable)` visits every Tag.
    let role_nodes = find_all(&doc, None, &|el| {
        el.attr("data-message-author-role").is_some()
            || el.attr("data-role").is_some()
            || (el.attr("data-testid").is_some()
                && {
                    let v = el.attr("data-testid").unwrap_or("");
                    v.contains("message") || v.contains("turn")
                })
    });
    for node in role_nodes {
        let role = detect_node_role(node);
        let md = clean_node_to_md(node);
        if !md.is_empty() {
            turns.push(ChatTurn { role: role.to_string(), text: clean_chat_tokens(&md) });
        }
    }

    // 2. 查找常见对话类名模式.
    if turns.is_empty() {
        let candidates = find_all(
            &doc,
            Some(&["div", "section", "article"]),
            &|el| {
                class_matches(
                    el,
                    &[
                        "chat-message",
                        "message-item",
                        "conversation-item",
                        "chat-bubble",
                        "dialog-item",
                        "chat-turn",
                    ],
                )
            },
        );
        for c in candidates {
            let role = detect_node_role(c);
            let md = clean_node_to_md(c);
            if !md.is_empty() {
                turns.push(ChatTurn { role: role.to_string(), text: clean_chat_tokens(&md) });
            }
        }
    }

    if !turns.is_empty() {
        return Some(ChatData { title: Pj::Str(title), turns });
    }
    None
}

thread_local! {
    static GENERIC_TITLE_SUFFIX_RE: regex::Regex = regex::Regex::new(
        r"(?i)\s*[-_|]\s*(Gemini|ChatGPT|Claude|DeepSeek|Kimi|Perplexity|豆包|通义千问).*\n?\z"
    )
    .unwrap();
}

/// `re.sub(pattern, '', title, flags=re.I)` — returns the substituted text.
fn strip_generic_title_suffix(title: &str) -> String {
    GENERIC_TITLE_SUFFIX_RE.with(|re| re.replace_all(title, "").into_owned())
}

// ----------------------------------------------------- format_ai_chat_markdown

/// `format_ai_chat_markdown(chat_data, url='')`
/// (`ai_chat_parser.py:512-559`).  Returns `None` when `chat_data` is absent
/// or has no turns.  `assistant_icon` is read by Python but never emitted
/// (all icons are empty), so it is not modelled.
pub fn format_ai_chat_markdown(chat_data: Option<&ChatData>, url: &str) -> Option<String> {
    let chat_data = chat_data.filter(|d| !d.turns.is_empty())?;

    let (_, info) = detect_ai_platform(url);
    let plat_name: &str = info.map(|i| i.name).unwrap_or("AI 对话");
    let assistant_name: &str = info.map(|i| i.role_assistant).unwrap_or("AI 助手");

    // `title = chat_data.get('title') or f"{plat_name} 对话分享"`.
    let title = if chat_data.title.truthy() {
        py_str(&chat_data.title)
    } else {
        format!("{plat_name} 对话分享")
    };
    let turns = &chat_data.turns;

    let mut lines: Vec<String> = Vec::new();
    lines.push(format!("# {title}"));
    lines.push(String::new());
    // U+FF1A full-width colon; two trailing ASCII spaces on this line.
    lines.push(format!("> **平台**：{plat_name}  "));
    if !url.is_empty() {
        lines.push(format!("> **来源链接**：[{url}]({url})  "));
    } else {
        lines.push(String::new());
    }
    lines.push(format!("> **对话轮数**：共 {} 轮交互", turns.len()));
    lines.push(String::new());
    lines.push("---".to_string());
    lines.push(String::new());

    // `lines = [l for l in lines if l != ""]` then `lines.append("")`.
    lines.retain(|l| l != "");
    lines.push(String::new());

    for turn in turns.iter() {
        let role = turn.role.as_str();
        let text = py_strip(&turn.text);
        if text.is_empty() {
            continue;
        }
        if role == "user" {
            lines.push("### 用户 (User)".to_string());
        } else {
            lines.push(py_strip(&format!("### {assistant_name}")));
        }
        lines.push(String::new());
        lines.push(text);
        lines.push(String::new());
        lines.push("---".to_string());
        lines.push(String::new());
    }

    Some(py_strip(&lines.join("\n")) + "\n")
}

// ------------------------------------------------------------ try_parse_ai_chat

/// The `{'ok': True, ...}` envelope `try_parse_ai_chat` returns.
#[derive(Debug, Clone)]
pub struct ChatEnvelope {
    pub title: Pj,
    pub turns_count: usize,
    pub markdown: Option<String>,
    pub platform: &'static str,
}

/// `try_parse_ai_chat(url, html)` (`ai_chat_parser.py:562-593`).  Public
/// entry point: never raises, returns `None` for empty html or when no
/// parser produced any turns.
pub fn try_parse_ai_chat(url: &str, html: &str) -> Option<ChatEnvelope> {
    if html.is_empty() {
        return None;
    }
    let (plat_id, _) = detect_ai_platform(url);

    let chat_data = match plat_id {
        "chatgpt" => parse_chatgpt_json(html).or_else(|| parse_generic_ai_chat_dom(html, url)),
        "claude" => parse_claude_json(html).or_else(|| parse_generic_ai_chat_dom(html, url)),
        "gemini" => {
            parse_gemini_dom_or_json(html).or_else(|| parse_generic_ai_chat_dom(html, url))
        }
        _ => parse_chatgpt_json(html)
            .or_else(|| parse_claude_json(html))
            .or_else(|| parse_gemini_dom_or_json(html))
            .or_else(|| parse_generic_ai_chat_dom(html, url)),
    }?;

    if !chat_data.turns.is_empty() {
        let markdown = format_ai_chat_markdown(Some(&chat_data), url);
        return Some(ChatEnvelope {
            title: chat_data.title.clone(),
            turns_count: chat_data.turns.len(),
            markdown,
            platform: plat_id,
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------ builders

    fn st(x: &str) -> Pj {
        Pj::Str(x.to_string())
    }
    fn ix(x: i64) -> Pj {
        Pj::Int(x.to_string())
    }
    fn obj(fields: Vec<(&str, Pj)>) -> Pj {
        Pj::Obj(fields.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    }
    fn arr(items: Vec<Pj>) -> Pj {
        Pj::Arr(items)
    }
    /// `unflatten_turbo_stream` takes the raw flat array, not a `Pj` wrapper.
    fn flat(items: Vec<Pj>) -> Vec<Pj> {
        items
    }
    fn leaf_text(a: &TurboArena, slot: Option<usize>) -> Option<String> {
        slot.and_then(|s| a.leaf(s)).map(py_str)
    }
    fn next_data(body: &str) -> String {
        format!("<html><body><script id=\"__NEXT_DATA__\">{body}</script></body></html>")
    }

    // -------------------------------------------------- detect_ai_platform

    #[test]
    fn detect_ai_platform_matches_hosts_in_order() {
        assert_eq!(detect_ai_platform("https://chatgpt.com/share/abc").0, "chatgpt");
        assert_eq!(detect_ai_platform("https://chat.openai.com/x").0, "chatgpt");
        assert_eq!(detect_ai_platform("https://x.share.gemini.google/y").0, "gemini");
        assert_eq!(detect_ai_platform("https://CLAUDE.AI/share/z").0, "claude");
        assert_eq!(detect_ai_platform("").0, "generic");
        assert_eq!(detect_ai_platform("https://example.com/share/x").0, "generic");
        assert!(detect_ai_platform("https://claude.ai/x").1.is_some());
        assert!(detect_ai_platform("https://example.com/x").1.is_none());
    }

    #[test]
    fn detect_ai_platform_keeps_urlparse_userinfo_quirk() {
        // `netloc.split(':')[0]` on `u:p@chatgpt.com` yields the *userinfo*
        // token `u`, so the host never reaches the platform table.
        assert_eq!(detect_ai_platform("http://u:p@chatgpt.com/x").0, "generic");
        // A protocol-relative URL still exposes its authority as `netloc`.
        assert_eq!(detect_ai_platform("//claude.ai/share/x").0, "claude");
        // A bare suffix must not match (`notchatgpt.com` != `*.chatgpt.com`).
        assert_eq!(detect_ai_platform("https://notchatgpt.com/x").0, "generic");
    }

    #[test]
    fn is_ai_chat_url_covers_platform_then_paths() {
        assert!(is_ai_chat_url("https://chatgpt.com/x"));
        assert!(is_ai_chat_url("https://example.com/share/x"));
        assert!(is_ai_chat_url("https://example.com/chat/x"));
        assert!(!is_ai_chat_url("https://example.com/blog"));
        assert!(!is_ai_chat_url(""));
    }

    #[test]
    fn urlparse_exposes_netloc_and_path_only() {
        let u = py_urlparse("https://a.b/c/d?x=1#f");
        assert_eq!(u.netloc, "a.b");
        assert_eq!(u.path, "/c/d");
        assert_eq!(py_urlparse("/local/path").netloc, "");
        assert_eq!(py_urlparse("//a.b/p").netloc, "a.b");
    }

    // -------------------------------------------------- clean_chat_tokens

    #[test]
    fn clean_chat_tokens_strips_all_three_pua_forms() {
        assert_eq!(clean_chat_tokens("\u{e200}entity\u{e202}[foo]\u{e201}kept"), "kept");
        assert_eq!(clean_chat_tokens("a\u{e200}cite\u{e202}x\u{e201}b"), "ab");
        assert_eq!(clean_chat_tokens("\u{e200}url\u{e202}y\u{e201}z"), "z");
        assert_eq!(clean_chat_tokens(""), "");
    }

    #[test]
    fn clean_chat_tokens_body_cannot_cross_a_newline() {
        // `re.sub(r'\u200bcite\u2002.*?\u2001', ...)` without `re.S`.
        let text = "\u{e200}cite\u{e202}a\nb\u{e201}c";
        assert_eq!(clean_chat_tokens(text), text);
        // Two same-line tokens are both removed (leftmost, shortest).
        assert_eq!(
            clean_chat_tokens("\u{e200}cite\u{e202}1\u{e201}a\u{e200}cite\u{e202}2\u{e201}b"),
            "ab"
        );
    }

    // --------------------------------------------- unflatten_turbo_stream

    #[test]
    fn unflatten_returns_scalars_as_is() {
        let a = unflatten_turbo_stream(&flat(vec![st("plain")]));
        assert_eq!(a.leaf(a.root()), Some(&st("plain")));
        // `arr[0] == true` is *returned* (it is a value, not an index here).
        let b = unflatten_turbo_stream(&flat(vec![Pj::Bool(true)]));
        assert_eq!(b.leaf(b.root()), Some(&Pj::Bool(true)));
    }

    #[test]
    fn unflatten_bool_is_an_index_and_out_of_range_is_none() {
        // `isinstance(True, int)` is true in Python: true -> 1, false -> 0.
        let t = unflatten_turbo_stream(&flat(vec![obj(vec![("k", Pj::Bool(true))]), st("one")]));
        let root = t.root();
        assert_eq!(leaf_text(&t, t.obj_get(root, "k")), Some("one".to_string()));
        let f = unflatten_turbo_stream(&flat(vec![obj(vec![("k", Pj::Bool(false))]), st("one")]));
        let froot = f.root();
        assert_eq!(f.obj_get(froot, "k"), Some(froot));
        let oob = unflatten_turbo_stream(&flat(vec![obj(vec![("k", ix(9)), ("n", ix(-1))])]));
        let oroot = oob.root();
        assert!(oob.is_null(oob.obj_get(oroot, "k").unwrap()));
        assert!(oob.is_null(oob.obj_get(oroot, "n").unwrap()));
    }

    #[test]
    fn unflatten_cycle_terminates_via_memo() {
        let a = unflatten_turbo_stream(&flat(vec![obj(vec![("child", ix(0))])]));
        let root = a.root();
        assert_eq!(a.obj_get(root, "child"), Some(root));
    }

    #[test]
    fn unflatten_underscore_keys_resolve_names() {
        let ok = unflatten_turbo_stream(&flat(vec![obj(vec![("_1", ix(2))]), st("k"), st("v")]));
        let r = ok.root();
        assert_eq!(leaf_text(&ok, ok.obj_get(r, "k")), Some("v".to_string()));

        // `int('x')` raises ValueError -> the pair is dropped silently.
        let bad = unflatten_turbo_stream(&flat(vec![obj(vec![("_x", ix(1))]), st("a")]));
        assert_eq!(bad.as_obj(bad.root()).map(|f| f.len()), Some(0));

        // A resolved key name that is not a str drops the entry.
        let notstr = unflatten_turbo_stream(&flat(vec![obj(vec![("_1", ix(2))]), ix(5), st("a")]));
        assert_eq!(notstr.as_obj(notstr.root()).map(|f| f.len()), Some(0));

        // Duplicate resolved names: first position, last value.
        let dup = unflatten_turbo_stream(&flat(vec![
            obj(vec![("_1", ix(2)), ("_3", ix(4))]),
            st("k"),
            st("a"),
            st("k"),
            st("b"),
        ]));
        let droot = dup.root();
        let fields = dup.as_obj(droot).unwrap();
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].0, "k");
        assert_eq!(leaf_text(&dup, Some(fields[0].1)), Some("b".to_string()));
    }

    #[test]
    fn unflatten_keeps_shared_subtrees_identical() {
        let a = unflatten_turbo_stream(&flat(vec![
            obj(vec![("a", ix(2)), ("b", ix(2))]),
            Pj::Null,
            arr(vec![ix(3)]),
            st("x"),
        ]));
        let root = a.root();
        assert!(a.obj_get(root, "a").is_some());
        assert_eq!(a.obj_get(root, "a"), a.obj_get(root, "b"));
    }

    // -------------------------------------------------- find_key_recursive

    #[test]
    fn find_key_prefers_the_own_dict_key() {
        let a = unflatten_turbo_stream(&flat(vec![
            obj(vec![("target", ix(1)), ("child", ix(2))]),
            st("root-level"),
            obj(vec![("target", ix(1))]),
        ]));
        assert_eq!(leaf_text(&a, a.find_key("target")), Some("root-level".to_string()));
    }

    #[test]
    fn find_key_null_hit_aborts_that_dict_only() {
        // Root *has* `target` = None -> `return obj[key]` -> None, and the
        // child scan never runs, so the whole lookup reads as "not found".
        let hit = unflatten_turbo_stream(&flat(vec![
            obj(vec![("target", Pj::Null), ("child", ix(1))]),
            obj(vec![("target", st("deep"))]),
        ]));
        assert_eq!(hit.find_key("target"), None);

        // One level down the None is swallowed and the sibling still wins.
        let sibling = unflatten_turbo_stream(&flat(vec![
            obj(vec![("a", ix(1)), ("b", ix(2))]),
            obj(vec![("target", Pj::Null)]),
            obj(vec![("target", st("found"))]),
        ]));
        assert_eq!(
            leaf_text(&sibling, sibling.find_key("target")),
            Some("found".to_string())
        );
    }

    // --------------------------------------------------- parse_chatgpt_json

    const CHATGPT_TURBO_HTML: &str = r###"<html><head><title>x - ChatGPT</title></head><body>
<script>streamController.enqueue("[{\"_1\":2,\"_3\":5},\"pageTitle\",\"Shared Chat\",\"linear_conversation\",null,[6],{\"message\":7},{\"author\":8,\"content\":10},{\"role\":9},\"user\",{\"parts\":11},[12],\"Hello there\"]");</script>
</body></html>"###;

    #[test]
    fn chatgpt_strategy1_turbo_stream_fixture() {
        let data = parse_chatgpt_json(CHATGPT_TURBO_HTML).expect("strategy 1 should parse");
        assert_eq!(py_str(&data.title), "Shared Chat");
        assert_eq!(data.turns.len(), 1);
        assert_eq!(data.turns[0].role, "user");
        assert_eq!(data.turns[0].text, "Hello there");
    }

    #[test]
    fn chatgpt_strategy1_skips_non_dict_items_and_dict_parts_read_text() {
        // `linear = [ "scalar", message ]` -> the scalar is skipped by the
        // `isinstance(item, dict)` gate; parts holds a str *and* a dict whose
        // `text` is used through the `elif isinstance(p, dict) and p.get('text')`
        // branch.  The parts list is reached *by index* (14), like a real
        // TurboStream graph, so Python expands it.  Flat indices: 5=[6,7]
        // 6="scalar" 7={"message":8} 8={"author":9,"content":10} 9={"role":11}
        // 10={"parts":14} 11="assistant" 12="body" 13={"text":" second"}
        // 14=[12,13].
        let html = r###"<html><body><script>streamController.enqueue("[{\"_1\":2,\"_3\":5},\"pageTitle\",\"T\",\"linear_conversation\",null,[6,7],\"scalar\",{\"message\":8},{\"author\":9,\"content\":10},{\"role\":11},{\"parts\":14},\"assistant\",\"body\",{\"text\":\" second\"},[12,13]]");</script></body></html>"###;
        let data = parse_chatgpt_json(html).expect("strategy 1");
        assert_eq!(py_str(&data.title), "T");
        assert_eq!(data.turns.len(), 1);
        assert_eq!(data.turns[0].role, "assistant");
        assert_eq!(data.turns[0].text, "body\n\nsecond");
    }

    #[test]
    fn chatgpt_strategy1_leaked_parts_list_stays_unindexed() {
        // Same graph as above, but with [12,13] nested *raw* inside index 10.
        // Python's `resolve` returns a non-int as-is, so those ints are never
        // re-indexed; the isinstance gate turns them into str(12)/str(13).
        // Verified against the authority: `{'title': 'T', 'turns': [{'role':
        // 'assistant', 'text': '12\\n\\n13'}]}`.
        let html = r###"<html><body><script>streamController.enqueue("[{\"_1\":2,\"_3\":5},\"pageTitle\",\"T\",\"linear_conversation\",null,[6,7],\"scalar\",{\"message\":8},{\"author\":9,\"content\":10},{\"role\":11},{\"parts\":[12,13]},\"assistant\",\"body\",{\"text\":\" second\"}]");</script></body></html>"###;
        let data = parse_chatgpt_json(html).expect("strategy 1 leaked parts");
        assert_eq!(py_str(&data.title), "T");
        assert_eq!(data.turns.len(), 1);
        assert_eq!(data.turns[0].role, "assistant");
        assert_eq!(data.turns[0].text, "12\n\n13");
    }

    #[test]
    fn chatgpt_strategy2_linear_branch_stringifies_scalar_parts() {
        let body = r###"{"props":{"pageProps":{"title":"S2 Title","serverResponse":{"data":{"linear_conversation":[{"message":{"author":{"role":"assistant"},"content":{"parts":["First ",123,"tail"]}}},{"message":{"author":{"role":"weird"},"content":{"parts":["skip"]}}}]}}}}}"###;
        let data = parse_chatgpt_json(&next_data(body)).expect("strategy 2 linear");
        assert_eq!(py_str(&data.title), "S2 Title");
        assert_eq!(data.turns.len(), 1);
        assert_eq!(data.turns[0].role, "assistant");
        // `str(123)` survives the isinstance gate; each part is stripped and
        // the join uses blank lines.
        assert_eq!(data.turns[0].text, "First\n\n123\n\ntail");
    }

    #[test]
    fn chatgpt_strategy2_mapping_branch_walks_values_in_order() {
        let body = r###"{"props":{"pageProps":{"serverResponse":{"data":{"mapping":{"n0":{"message":{"author":{"role":"user"},"content":{"parts":["hello mapping"]}}},"n1":{},"n2":{"message":null}}}}}}}"###;
        let data = parse_chatgpt_json(&next_data(body)).expect("strategy 2 mapping");
        // `page_props.get('title') or server_resp.get('title') or title`
        assert_eq!(py_str(&data.title), "ChatGPT 对话");
        assert_eq!(data.turns.len(), 1);
        assert_eq!(data.turns[0].role, "user");
        assert_eq!(data.turns[0].text, "hello mapping");
    }

    #[test]
    fn chatgpt_strategy2_raises_on_non_dict_item_and_is_swallowed() {
        // `item.get('message', {})` on a str raises AttributeError, which the
        // strategy's `except Exception: pass` swallows: no result at all.
        let body = r###"{"props":{"pageProps":{"serverResponse":{"data":{"linear_conversation":["oops"]}}}}}"###;
        assert!(parse_chatgpt_json(&next_data(body)).is_none());
    }

    // ---------------------------------------------------- parse_claude_json

    #[test]
    fn claude_reads_text_and_content_blocks() {
        let body = r###"{"props":{"pageProps":{"chat":{"name":"My Claude","chat_messages":[{"sender":"human","content":[{"type":"text","text":"Hi\n"},{"type":"tool"}]},{"role":"assistant","text":"Hello!"},{"sender":"other","text":"ignored"}]}}}}"###;
        let data = parse_claude_json(&next_data(body)).expect("claude");
        assert_eq!(py_str(&data.title), "My Claude");
        assert_eq!(data.turns.len(), 2);
        assert_eq!(data.turns[0], ChatTurn { role: "user".into(), text: "Hi".into() });
        assert_eq!(data.turns[1], ChatTurn { role: "assistant".into(), text: "Hello!".into() });
    }

    #[test]
    fn claude_falls_back_to_shared_conversation_and_aborts_on_bad_message() {
        let ok = r###"{"props":{"pageProps":{"sharedConversation":{"transcript":[{"sender":"user","text":"q"},{"sender":"claude","text":"a"}]}}}}"###;
        let data = parse_claude_json(&next_data(ok)).expect("sharedConversation");
        assert_eq!(py_str(&data.title), "Claude 对话");
        assert_eq!(data.turns.len(), 2);
        assert_eq!(data.turns[1].role, "assistant");

        // `msg.get('sender')` on a str raises -> except pass -> None.
        let bad = r###"{"props":{"pageProps":{"chat":{"chat_messages":["oops"]}}}}"###;
        assert!(parse_claude_json(&next_data(bad)).is_none());
    }

    // ------------------------------------------------ parse_gemini_dom_or_json

    #[test]
    fn gemini_strategy1_reads_web_component_nodes() {
        let html = "<html><head><title>Gemini - Trip</title></head><body>\
                    <user-query><p>What?</p></user-query>\
                    <model-response><p>Answer</p></model-response>\
                    </body></html>";
        let data = parse_gemini_dom_or_json(html).expect("gemini s1");
        assert_eq!(py_str(&data.title), "Trip");
        assert_eq!(data.turns.len(), 2);
        assert_eq!(data.turns[0].role, "user");
        assert_eq!(data.turns[0].text, "What?");
        assert_eq!(data.turns[1].role, "assistant");
        assert_eq!(data.turns[1].text, "Answer");
    }

    #[test]
    fn gemini_title_fallback_set_uses_lowercased_title() {
        let html = "<html><head><title>Live Content</title></head><body>\
                    <div class=\"response-container\">Some reply text</div>\
                    </body></html>";
        let data = parse_gemini_dom_or_json(html).expect("gemini s2");
        assert_eq!(py_str(&data.title), "Gemini 对话");
        assert_eq!(data.turns.len(), 1);
        assert_eq!(data.turns[0].role, "assistant");
    }

    #[test]
    fn gemini_strategy2_dedupes_against_raw_md_not_cleaned_text() {
        // Disclosed-behaviour pin: the guard compares the previous turn's
        // *cleaned* text with this node's *raw* md, so two nodes that clean to
        // the same text are NOT deduped.
        let one = "<div class=\"user-query-container\">X\u{e200}cite\u{e202}1\u{e201}</div>";
        let html = format!("<html><body>{one}{one}</body></html>");
        let data = parse_gemini_dom_or_json(&html).expect("gemini s2 dedupe");
        assert_eq!(data.turns.len(), 2);
        assert_eq!(data.turns[0].text, "X");

        // Truly identical nodes *are* dropped (raw == raw).
        let plain = "<div class=\"user-query-container\">Same text</div>";
        let html2 = format!("<html><body>{plain}{plain}</body></html>");
        let data2 = parse_gemini_dom_or_json(&html2).expect("gemini s2 plain");
        assert_eq!(data2.turns.len(), 1);
    }

    #[test]
    fn gemini_strategy3_needs_a_url_boundary_to_split_user_and_response() {
        // Python 444-457: `elif not response_parts and not image_parts` sends
        // *every* leading non-URL part to `user_parts`, so a plain
        // "question ∞ answer" section produces `response_text == ''` and no
        // turn pair at all -- the parse falls through to None, and the
        // first-line title back-fill (which sits inside that same `if`) never
        // runs.
        let q = "What is the tallest building in the world?";
        let a = "The tallest building is the Burj Khalifa.";
        let html = format!(
            "<html><body><script>window.WIZ_global_data = {{\"FbLXxd\":\"{q}\u{221e}{a}\"}};</script></body></html>"
        );
        assert!(parse_gemini_dom_or_json(&html).is_none());
    }

    #[test]
    fn gemini_strategy3_image_parts_prefix_the_response() {
        let q = "Please describe this picture of the harbour at dusk.";
        let img = "https://example.com/a.png";
        let a = "It shows a harbour.";
        let html = format!(
            "<html><body><script>window.WIZ_global_data = {{\"k\":\"{q}\u{221e}{img}\u{221e}{a}\"}};</script></body></html>"
        );
        let data = parse_gemini_dom_or_json(&html).expect("gemini s3 image");
        assert_eq!(data.turns.len(), 2);
        assert_eq!(data.turns[1].text, format!("![Gemini Image]({img})\n\n{a}"));
    }

    #[test]
    fn gemini_strategy3_short_values_and_bad_json_are_ignored() {
        // `len(v) > 50` gate and the `except Exception: pass` around the parse.
        let short = format!(
            "<html><body><script>window.WIZ_global_data = {{\"k\":\"a\u{221e}b\"}};</script></body></html>"
        );
        assert!(parse_gemini_dom_or_json(&short).is_none());
        let broken =
            "<html><body><script>window.WIZ_global_data = {not json};</script></body></html>";
        assert!(parse_gemini_dom_or_json(broken).is_none());
    }

    #[test]
    fn gemini_strategy4_uses_data_message_author_role() {
        let html = "<html><body>\
                    <section data-message-author-role=\"model\"><p>Role text here</p></section>\
                    </body></html>";
        let data = parse_gemini_dom_or_json(html).expect("gemini s4");
        assert_eq!(data.turns.len(), 1);
        // Only `== 'user'` maps to user; everything else is assistant.
        assert_eq!(data.turns[0].role, "assistant");
    }

    // ----------------------------------------------- parse_generic_ai_chat_dom

    #[test]
    fn generic_pass1_reads_role_attributes_and_testids() {
        let html = "<html><head><title>My Chat | ChatGPT</title></head><body>\
                    <div data-message-author-role=\"user\"><p>Question here</p></div>\
                    <span data-testid=\"message-body\"><p>Body text</p></span>\
                    </body></html>";
        let data = parse_generic_ai_chat_dom(html, "").expect("generic pass 1");
        // `re.sub(r'\s*[-_|]\s*(Gemini|ChatGPT|...).*$', '', title, re.I)`
        assert_eq!(py_str(&data.title), "My Chat");
        assert_eq!(data.turns.len(), 2);
        assert_eq!(data.turns[0].role, "user");
        assert_eq!(data.turns[0].text, "Question here");
        assert_eq!(data.turns[1].role, "assistant");
    }

    #[test]
    fn generic_pass2_class_patterns_and_default_title() {
        let html = "<html><body>\
                    <div class=\"chat-bubble\">Bubble text</div>\
                    <article class=\"dialog-item\">Dialog text</article>\
                    </body></html>";
        let data = parse_generic_ai_chat_dom(html, "").expect("generic pass 2");
        assert_eq!(py_str(&data.title), "AI 对话记录");
        assert_eq!(data.turns.len(), 2);
        assert_eq!(data.turns[0].text, "Bubble text");
    }

    #[test]
    fn generic_pass2_is_skipped_when_pass1_matched() {
        let html = "<html><body>\
                    <div data-role=\"user\"><p>Only this</p></div>\
                    <div class=\"chat-bubble\">Ignored bubble</div>\
                    </body></html>";
        let data = parse_generic_ai_chat_dom(html, "").expect("generic precedence");
        assert_eq!(data.turns.len(), 1);
        assert_eq!(data.turns[0].text, "Only this");
    }

    // ------------------------------------------------- format_ai_chat_markdown

    fn sample_chat() -> ChatData {
        ChatData {
            title: st("T"),
            turns: vec![
                ChatTurn { role: "user".into(), text: "Q1".into() },
                ChatTurn { role: "assistant".into(), text: "A1".into() },
            ],
        }
    }

    #[test]
    fn format_ai_chat_markdown_exact_bytes_without_url() {
        let md = format_ai_chat_markdown(Some(&sample_chat()), "").expect("markdown");
        // The header's blank-line placeholders are all dropped by
        // `[l for l in lines if l != ""]`, so title / platform / turn-count /
        // rule lines are contiguous; the platform line keeps two trailing
        // spaces, the turn-count line none.
        assert_eq!(
            md,
            "# T\n> **平台**：AI 对话  \n> **对话轮数**：共 2 轮交互\n---\n\n### 用户 (User)\n\nQ1\n\n---\n\n### AI 助手\n\nA1\n\n---\n"
        );
    }

    #[test]
    fn format_ai_chat_markdown_exact_bytes_with_url_and_platform() {
        let u = "https://chatgpt.com/share/x";
        let md = format_ai_chat_markdown(Some(&sample_chat()), u).expect("markdown");
        assert_eq!(
            md,
            "# T\n> **平台**：OpenAI ChatGPT  \n> **来源链接**：[https://chatgpt.com/share/x](https://chatgpt.com/share/x)  \n> **对话轮数**：共 2 轮交互\n---\n\n### 用户 (User)\n\nQ1\n\n---\n\n### ChatGPT\n\nA1\n\n---\n"
        );
    }

    #[test]
    fn format_ai_chat_markdown_title_fallback_and_empty_turns() {
        let no_title = ChatData { title: st(""), turns: vec![sample_chat().turns[0].clone()] };
        let md = format_ai_chat_markdown(Some(&no_title), "").expect("markdown");
        assert!(md.starts_with("# AI 对话 对话分享\n"));

        let no_turns = ChatData { title: st("T"), turns: vec![] };
        assert!(format_ai_chat_markdown(Some(&no_turns), "").is_none());
        assert!(format_ai_chat_markdown(None, "").is_none());

        // A turn whose text is only whitespace is skipped entirely.
        let blank = ChatData {
            title: st("T"),
            turns: vec![ChatTurn { role: "user".into(), text: "   ".into() }],
        };
        let md = format_ai_chat_markdown(Some(&blank), "").expect("markdown");
        assert!(!md.contains("### 用户 (User)"));
        assert!(md.contains("共 1 轮交互"));
    }

    // -------------------------------------------------------- try_parse_ai_chat

    #[test]
    fn try_parse_requires_html() {
        assert!(try_parse_ai_chat("https://chatgpt.com/share/x", "").is_none());
        assert!(try_parse_ai_chat("", "<html><body>nothing here</body></html>").is_none());
    }

    #[test]
    fn try_parse_dispatches_by_platform() {
        let gpt = try_parse_ai_chat("https://chatgpt.com/share/abc", CHATGPT_TURBO_HTML).unwrap();
        assert_eq!(gpt.platform, "chatgpt");
        assert_eq!(gpt.turns_count, 1);
        assert_eq!(py_str(&gpt.title), "Shared Chat");
        assert!(gpt.markdown.as_deref().unwrap().starts_with("# Shared Chat\n"));

        let claude_html = next_data(
            r###"{"props":{"pageProps":{"chat":{"name":"C","chat_messages":[{"sender":"user","text":"q"}]}}}}"###,
        );
        let c = try_parse_ai_chat("https://claude.ai/share/x", &claude_html).unwrap();
        assert_eq!(c.platform, "claude");
        assert_eq!(c.turns_count, 1);

        let gemini_html = "<html><body><user-query><p>Hi there</p></user-query></body></html>";
        let g = try_parse_ai_chat("https://gemini.google.com/share/x", gemini_html);
        // `gemini.google.com` is a known host, so the gemini parser runs first.
        assert!(g.is_some());
        assert_eq!(g.unwrap().platform, "gemini");
    }

    #[test]
    fn try_parse_generic_url_still_walks_the_full_parser_chain() {
        // The `else` branch tries chatgpt -> claude -> gemini -> generic.
        let gpt = try_parse_ai_chat("https://example.com/share/abc", CHATGPT_TURBO_HTML).unwrap();
        assert_eq!(gpt.platform, "generic");
        assert_eq!(gpt.turns_count, 1);

        let claude_html = next_data(
            r###"{"props":{"pageProps":{"chat":{"chat_messages":[{"sender":"human","text":"only claude shape"}]}}}}"###,
        );
        let c = try_parse_ai_chat("https://example.com/share/abc", &claude_html).unwrap();
        assert_eq!(c.platform, "generic");
        assert_eq!(c.markdown.as_deref().unwrap(), format_ai_chat_markdown(Some(&ChatData {
            title: st("Claude 对话"),
            turns: vec![ChatTurn { role: "user".into(), text: "only claude shape".into() }],
        }), "https://example.com/share/abc").unwrap());

        let generic_html = "<html><body><div data-role=\"user\"><p>Fallback turn</p></div></body></html>";
        let gm = try_parse_ai_chat("https://example.com/share/abc", generic_html).unwrap();
        assert_eq!(gm.platform, "generic");
        assert_eq!(gm.turns_count, 1);
        assert_eq!(py_str(&gm.title), "AI 对话记录");
    }

    // ----------------------------------------------------- disclosed deltas

    #[test]
    fn html_to_clean_md_empty_shortcut_and_blank_run_collapse() {
        assert_eq!(html_to_clean_md(""), "");
        // `re.sub(r'\n{3,}', '\n\n', md)` — three or more newlines become two.
        assert_eq!(collapse_blank_runs("a\n\n\n\nb"), "a\n\nb");
        assert_eq!(collapse_blank_runs("a\n\nb"), "a\n\nb");
        assert_eq!(collapse_blank_runs("a\n\n\n"), "a\n\n");
        assert_eq!(collapse_blank_runs("a\n"), "a\n");
    }

    #[test]
    fn detect_node_role_matches_python_word_lists() {
        let doc = hr::parse_html(
            "<html><body><div class=\"user-turn\"><span>x</span></div>\
             <div class=\"response-container\"><span>y</span></div>\
             <div data-testid=\"assistant-reply\"><span>z</span></div>\
             <div><p>plain</p></div></body></html>",
        );
        let divs = find_all(&doc, Some(&["div"]), &|_| true);
        assert_eq!(divs.len(), 4);
        assert_eq!(detect_node_role(divs[0]), "user");
        assert_eq!(detect_node_role(divs[1]), "assistant");
        assert_eq!(detect_node_role(divs[2]), "assistant");
        assert_eq!(detect_node_role(divs[3]), "assistant");
    }
}
