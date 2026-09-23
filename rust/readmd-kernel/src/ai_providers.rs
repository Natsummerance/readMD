//! `ai_providers.rs` — Rust port of the AI provider / transport layer of
//! `src/readmd_modules/ai.py` (branch `main`).
//!
//! This is a **pure library module**: it has no `crate::` references, no global
//! state and no process spawning.  Everything it needs from the application is
//! injected through the [`ProviderDirectory`], [`SkillService`] and
//! [`Transport`] traits, which keeps it unit-testable offline (see `mod tests`)
//! while [`ureq_transport::UreqTransport`] provides the real native-TLS egress
//! used by the shipped binary.
//!
//! ## Provenance map (ai.py line range → Rust symbol)
//!
//! | ai.py | lines | Rust | verdict |
//! |---|---|---|---|
//! | `resolve_key` (used by `chat`) | 328-340 | [`resolve_key`] | faithful (vault/env lookups injected) |
//! | `_is_local_provider` | 352-365 | [`is_local_provider`] | faithful |
//! | `_openai_auth_headers` | 368-369 | [`openai_auth_headers`] | faithful |
//! | `ChatError` | 372-373 | [`AiError`] | faithful (message-only error type) |
//! | `DEFAULT_HTTP_HEADERS` | 376-379 | [`default_http_headers`] | faithful |
//! | `_http_json` | 382-395 | [`http_json`] + [`Transport::post_json`] | faithful; wire `json.dumps` handled by the caller |
//! | `_http_stream` | 398-410 | [`http_stream`] + [`Transport::open_stream`] | faithful |
//! | `_openai_messages` | 413-422 | [`openai_messages`] | faithful (incl. the `insert(0, …)` reversal quirk) |
//! | `_anthropic_messages` | 425-441 | [`anthropic_messages`] | faithful |
//! | `_skill_messages` | 444-471 | [`skill_messages`] | faithful |
//! | `chat` | 477-526 | [`resolve_chat`] + [`dispatch_chat`] + [`chat`] | faithful, eager/lazy split preserved |
//! | `_normalize_base_url` | 527-549 | [`normalize_base_url`] | faithful |
//! | `_endpoint_url` | 552-556 | [`endpoint_url`] | faithful |
//! | `_request_headers` | 559-571 | [`request_headers`] | faithful |
//! | `_openai_usage` | 574-585 | [`openai_usage`] | faithful |
//! | `_chat_openai` | 588-651 | [`chat_openai`] / [`parse_openai_chat`] / [`OpenAiChatMachine`] | faithful |
//! | `_chat_openai_completion` | 654-716 | [`chat_openai_completion`] / [`parse_openai_completion`] / [`OpenAiCompletionMachine`] | faithful |
//! | `_chat_openai_responses` | 719-784 | [`chat_openai_responses`] / [`parse_openai_responses`] / [`OpenAiResponsesMachine`] | faithful |
//! | `_anthropic_usage` | 785-795 | [`anthropic_usage`] | faithful |
//! | `_chat_anthropic` | 798-866 | [`chat_anthropic`] / [`parse_anthropic`] / [`AnthropicMachine`] | faithful |
//! | `SkillRegistry.render` / `.variables` (skills.py 160-172, 35-37) | — | [`render_skill_template`], [`template_variable_names`] | faithful |
//!
//! Deliberately **not** ported here: `list_models` / `_http_get_json`
//! (already available as `ai.rs::AIClient::list_models` and the live
//! `h_ai_models` route), `key_source`, and the provider/config CRUD helpers —
//! those belong to the settings layer, not to the request layer.
//!
//! ## Known, documented divergences from CPython
//!
//! * `str.strip()` also treats `\x1c`-`\x1f` (and `\x85`/`\xa0`-class
//!   separators) as whitespace; [`py_strip`] uses Rust's `char::is_whitespace`
//!   (the `\x1c`-`\x1f` group differs).
//! * Objects parsed by `serde_json` iterate in the order the wire carried
//!   them: `preserve_order` is on, which is CPython's `json.loads` behaviour.
//!   Request bodies, `usage` events and headers additionally go through the
//!   explicit order-preserving helpers in this file.
//! * Python's `float.__str__` spells large values `1e+20`; `serde_json`/ryu
//!   writes `1e20`.
//! * `json.loads` accepts `NaN`/`Infinity`/`-Infinity`; `serde_json` rejects
//!   them, so such an SSE frame is skipped instead of parsed.
//! * Type/`KeyError`/`ValueError`s that CPython raises *uncaught* out of
//!   `chat()` (e.g. `"\n\n" + None` in `_anthropic_messages`) are surfaced as
//!   [`AiError`] carrying the exact CPython message text.

use std::collections::VecDeque;

use serde_json::{Map, Value};

// ---------------------------------------------------------------------------
// errors (ai.py: `ChatError`, 372-373)
// ---------------------------------------------------------------------------

/// `ai.py::ChatError` — a message-only failure.  Also used to carry the text of
/// CPython exceptions that `chat()` lets escape (see the module doc).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiError {
    pub message: String,
}

impl AiError {
    pub fn new(message: impl Into<String>) -> Self {
        AiError { message: message.into() }
    }
    /// `AttributeError: 'x' object has no attribute 'get'`
    fn attribute_error(type_name: &str) -> Self {
        Self::attribute_error_on(type_name, "get")
    }
    fn attribute_error_on(type_name: &str, attribute: &str) -> Self {
        Self::new(format!("'{}' object has no attribute '{}'", type_name, attribute))
    }
    fn key_error(key: &str) -> Self {
        Self::new(format!("KeyError: '{}'", key))
    }
    fn key_error_index(key: i64) -> Self {
        Self::new(format!("KeyError: {}", key))
    }
    fn index_error() -> Self {
        Self::new("IndexError: list index out of range")
    }
    fn not_subscriptable(type_name: &str) -> Self {
        Self::new(format!("TypeError: '{}' object is not subscriptable", type_name))
    }
    fn not_iterable(type_name: &str) -> Self {
        Self::new(format!("TypeError: '{}' object is not iterable", type_name))
    }
    fn value_error(text: &str) -> Self {
        Self::new(format!("invalid literal for int() with base 10: '{}'", text))
    }
    fn concat_error(type_name: &str) -> Self {
        Self::new(format!(
            "TypeError: can only concatenate str (not \"{}\") to str",
            type_name
        ))
    }
    fn unsupported_add(a: &str, b: &str) -> Self {
        Self::new(format!(
            "TypeError: unsupported operand type(s) for +: '{}' and '{}'",
            a, b
        ))
    }
    fn seq_item(index: usize, type_name: &str) -> Self {
        Self::new(format!(
            "TypeError: sequence item {}: expected str instance, {} found",
            index, type_name
        ))
    }
}

impl std::fmt::Display for AiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for AiError {}

/// Distinguishes `except SkillError` (message passthrough) from the generic
/// `except Exception` arm of `_skill_messages` (skills.py 13 / ai.py 466-470).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillErrorKind {
    /// `SkillError` (a `ValueError` subclass raised by `skills.py`).
    Skill,
    /// Any other exception while rendering.
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillError {
    pub kind: SkillErrorKind,
    pub message: String,
}

impl SkillError {
    pub fn skill(message: impl Into<String>) -> Self {
        SkillError { kind: SkillErrorKind::Skill, message: message.into() }
    }
    pub fn other(message: impl Into<String>) -> Self {
        SkillError { kind: SkillErrorKind::Other, message: message.into() }
    }
}

// ---------------------------------------------------------------------------
// CPython-flavoured value helpers
// ---------------------------------------------------------------------------

const EMPTY_STR: &str = "";

/// `bool(x)` for JSON-parsed values.
pub fn py_truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => match n.as_f64() {
            Some(f) => f != 0.0,
            None => true,
        },
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

/// `type(x).__name__` for JSON-parsed values.
pub fn py_type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "NoneType",
        Value::Bool(_) => "bool",
        Value::Number(n) => {
            if n.is_i64() || n.is_u64() {
                "int"
            } else {
                "float"
            }
        }
        Value::String(_) => "str",
        Value::Array(_) => "list",
        Value::Object(_) => "dict",
    }
}

fn null() -> Value {
    Value::Null
}

fn empty_string() -> Value {
    Value::String(String::new())
}

fn str_value(s: &str) -> Value {
    Value::String(s.to_string())
}

/// `dict.get(key)` — `None` both for a missing key and for a non-mapping.
pub fn py_get<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
    match v {
        Value::Object(o) => o.get(key),
        _ => None,
    }
}

/// `v.get(key)` where `v` is required to be a mapping, else `AttributeError`.
fn py_get_checked<'a>(v: &'a Value, key: &str) -> Result<Option<&'a Value>, AiError> {
    match v {
        Value::Object(o) => Ok(o.get(key)),
        other => Err(AiError::attribute_error(py_type_name(other))),
    }
}

/// `x[k]` for a mapping; missing key → `KeyError`.
fn py_subscript_map(v: &Value, key: &str) -> Result<Value, AiError> {
    match v {
        Value::Object(o) => match o.get(key) {
            Some(x) => Ok(x.clone()),
            None => Err(AiError::key_error(key)),
        },
        other => Err(AiError::not_subscriptable(py_type_name(other))),
    }
}

/// `x[0]` for the value of `d.get("choices") or []`.
fn py_index0(v: &Value) -> Result<Value, AiError> {
    match v {
        Value::Array(a) => a.first().cloned().ok_or_else(AiError::index_error),
        Value::String(s) => match s.chars().next() {
            Some(c) => Ok(Value::String(c.to_string())),
            None => Err(AiError::index_error()),
        },
        Value::Object(_) => Err(AiError::key_error_index(0)),
        other => Err(AiError::not_subscriptable(py_type_name(other))),
    }
}

/// `for x in v` — arrays element-wise, strings/objects by their characters/keys.
fn py_iter(v: &Value) -> Result<Vec<Value>, AiError> {
    match v {
        Value::Array(a) => Ok(a.clone()),
        Value::String(s) => Ok(s.chars().map(|c| Value::String(c.to_string())).collect()),
        Value::Object(o) => Ok(o.keys().map(|k| Value::String(k.clone())).collect()),
        other => Err(AiError::not_iterable(py_type_name(other))),
    }
}

/// First truthy candidate, else `fallback` (Python's `a or b or c`).
fn first_truthy<'a>(vals: &[&'a Value], fallback: &'a Value) -> &'a Value {
    for v in vals {
        if py_truthy(v) {
            return v;
        }
    }
    fallback
}

/// `str(x)` for JSON-parsed values.
pub fn py_str(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "None".to_string(),
        Value::Bool(b) => b.to_string_bool(),
        Value::Number(n) => py_num_str(n),
        other => py_repr(other),
    }
}

trait ToStrBool {
    fn to_string_bool(&self) -> String;
}
impl ToStrBool for bool {
    fn to_string_bool(&self) -> String {
        if *self {
            "True".to_string()
        } else {
            "False".to_string()
        }
    }
}

fn py_num_str(n: &serde_json::Number) -> String {
    if let Some(i) = n.as_i64() {
        return i.to_string();
    }
    if let Some(u) = n.as_u64() {
        return u.to_string();
    }
    match n.as_f64() {
        Some(f) => py_float_repr(f),
        None => n.to_string(),
    }
}

fn py_float_repr(f: f64) -> String {
    if f.is_nan() {
        return "nan".to_string();
    }
    if f.is_infinite() {
        return if f > 0.0 { "inf".to_string() } else { "-inf".to_string() };
    }
    let mut s = format!("{}", f);
    if !s.contains('.') && !s.contains('e') && !s.contains('E') {
        s.push_str(".0");
    }
    s
}

/// `repr(x)` for JSON-parsed values.
pub fn py_repr(v: &Value) -> String {
    match v {
        Value::String(s) => py_repr_str(s),
        Value::Null => "None".to_string(),
        Value::Bool(b) => b.to_string_bool(),
        Value::Number(n) => py_num_str(n),
        Value::Array(a) => {
            let parts: Vec<String> = a.iter().map(py_repr).collect();
            format!("[{}]", parts.join(", "))
        }
        Value::Object(o) => {
            let parts: Vec<String> = o.iter().map(|(k, val)| format!("{}: {}", py_repr_str(k), py_repr(val))).collect();
            format!("{{{}}}", parts.join(", "))
        }
    }
}

/// `repr(str)` — CPython prefers `'…'` and switches to `"…"` when the value
/// contains a `'` but no `"`.
pub fn py_repr_str(s: &str) -> String {
    let quote = if s.contains('\'') && !s.contains('"') { '"' } else { '\'' };
    let mut out = String::with_capacity(s.len() + 2);
    out.push(quote);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 0x20 || (c as u32) == 0x7f => {
                out.push_str(&format!("\\x{:02x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

/// `str.strip()` with no argument (see the module doc for the `\x1c` caveat).
pub fn py_strip(s: &str) -> String {
    s.trim().to_string()
}

/// `str.rstrip("/")`.
pub fn py_rstrip_char(s: &str, ch: char) -> String {
    let mut out = s;
    while out.ends_with(ch) {
        out = &out[..out.len() - ch.len_utf8()];
    }
    out.to_string()
}

/// `s[:n]` measured in code points.
pub fn py_slice_prefix(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

fn py_ends_with_ic(haystack: &str, needle: &str) -> bool {
    haystack.to_lowercase().ends_with(needle)
}

/// `str()` of `dict.get(key, "")`-style defaults.
fn chain_str(vals: &[Value], fallback: &str) -> String {
    let refs: Vec<&Value> = vals.iter().collect();
    py_str(first_truthy(&refs, &str_value(fallback)))
}

// ---------------------------------------------------------------------------
// json.dumps with CPython's default formatting
// ---------------------------------------------------------------------------

/// `json.dumps(obj)` — `ensure_ascii=True`, `separators=(', ', ': ')`.
pub fn py_dumps(v: &Value) -> String {
    dumps_json(v, true)
}

/// `json.dumps(obj, ensure_ascii=False)`.
pub fn py_dumps_unicode(v: &Value) -> String {
    dumps_json(v, false)
}

pub fn dumps_json(v: &Value, ascii_only: bool) -> String {
    let mut out = String::new();
    dump_value(v, ascii_only, &mut out);
    out
}

fn dump_value(v: &Value, ascii_only: bool, out: &mut String) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(n) => out.push_str(&py_num_str(n)),
        Value::String(s) => dump_string(s, ascii_only, out),
        Value::Array(a) => {
            out.push('[');
            for (i, item) in a.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                dump_value(item, ascii_only, out);
            }
            out.push(']');
        }
        Value::Object(o) => {
            out.push('{');
            for (i, (k, item)) in o.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                dump_string(k, true, out);
                out.push_str(": ");
                dump_value(item, ascii_only, out);
            }
            out.push('}');
        }
    }
}

/// Serialises an object from already-rendered member strings, so the caller
/// fixes the key order (`model`, `messages`, `stream`, ...) exactly as the
/// Python request body literal does.
pub fn dumps_ordered(pairs: &[(String, String)]) -> String {
    let mut out = String::from("{");
    for (i, (k, v)) in pairs.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        dump_string(k, true, &mut out);
        out.push_str(": ");
        out.push_str(v);
    }
    out.push('}');
    out
}

fn dump_string(s: &str, ascii_only: bool, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c if ascii_only && (c as u32) > 0x7f => {
                let cp = c as u32;
                if cp > 0xffff {
                    let v = cp - 0x10000;
                    out.push_str(&format!("\\u{:04x}\\u{:04x}", 0xd800 + (v >> 10), 0xdc00 + (v & 0x3ff)));
                } else {
                    out.push_str(&format!("\\u{:04x}", cp));
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

// ---------------------------------------------------------------------------
// URL helpers (ai.py 527-556)
// ---------------------------------------------------------------------------

const SUFFIXES_TO_STRIP: [&str; 7] = [
    "/chat/completions",
    "/completions",
    "/responses",
    "/v1/messages",
    "/messages",
    "/v1/models",
    "/models",
];

/// `ai.py::_normalize_base_url` (527-549).  Only the **first** matching suffix
/// is stripped (the loop `break`s), and the comparison is case-insensitive
/// while the kept prefix is not lower-cased.
pub fn normalize_base_url(base_url: &str, endpoint: &str) -> String {
    let mut u = py_rstrip_char(&py_strip(base_url), '/');
    if u.is_empty() {
        return u;
    }
    for s in SUFFIXES_TO_STRIP.iter() {
        if py_ends_with_ic(&u, s) {
            u = py_rstrip_char(&u[..u.len() - s.len()], '/');
            break;
        }
    }
    if endpoint.is_empty() {
        return u;
    }
    let endpoint = endpoint.trim_start_matches('/');
    format!("{}/{}", u, endpoint)
}

/// `ai.py::_endpoint_url` (552-556).  `str(endpoint_mode or "prefix").lower()`
/// is deliberately *not* stripped, matching CPython.
pub fn endpoint_url(base_url: &str, endpoint: &str, endpoint_mode: &str) -> String {
    let mode_value = if endpoint_mode.is_empty() {
        "prefix".to_string()
    } else {
        endpoint_mode.to_string()
    };
    if mode_value.to_lowercase() == "full_url" {
        return py_rstrip_char(&py_strip(base_url), '/');
    }
    normalize_base_url(base_url, endpoint)
}

// ---------------------------------------------------------------------------
// headers (ai.py 368-379, 559-571)
// ---------------------------------------------------------------------------

/// Header list kept ordered so `dict.update()` semantics (and therefore the
/// wire order) survive the port.
pub type Headers = Vec<(String, String)>;

/// `ai.py::DEFAULT_HTTP_HEADERS` (376-379).
pub fn default_http_headers() -> Headers {
    vec![
        (
            "User-Agent".to_string(),
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36 ReadMD-AI/2.3".to_string(),
        ),
        ("Accept".to_string(), "application/json, text/plain, */*".to_string()),
    ]
}

/// `ai.py::_openai_auth_headers` (368-369).
pub fn openai_auth_headers(api_key: &str) -> Headers {
    if api_key.is_empty() {
        Vec::new()
    } else {
        vec![("Authorization".to_string(), format!("Bearer {}", api_key))]
    }
}

/// `ai.py::_request_headers` (559-571).
pub fn request_headers(base: &Headers, custom: Option<&Value>) -> Headers {
    let mut out: Headers = base.clone();
    if let Some(Value::Object(map)) = custom {
        for (k, v) in map.iter() {
            let key = py_strip(k);
            if key.is_empty() {
                continue;
            }
            let lower = key.to_lowercase();
            if lower == "authorization" || lower == "x-api-key" || lower == "cookie" {
                continue;
            }
            let value = py_str(v);
            if value.chars().count() > 2048 || key.chars().count() > 128 || value.chars().any(|c| (c as u32) < 32) {
                continue;
            }
            merge_header(&mut out, &key, &value);
        }
    }
    out
}

fn merge_header(out: &mut Headers, key: &str, value: &str) {
    match out.iter_mut().find(|(k, _)| k == key) {
        Some(slot) => slot.1 = value.to_string(),
        None => out.push((key.to_string(), value.to_string())),
    }
}

/// `req_headers = dict(DEFAULT_HTTP_HEADERS); req_headers.update(headers)`.
fn merge_default_headers(headers: &Headers) -> Headers {
    let mut out = default_http_headers();
    for (k, v) in headers {
        merge_header(&mut out, k, v);
    }
    out
}

// ---------------------------------------------------------------------------
// transport (ai.py 382-410)
// ---------------------------------------------------------------------------

/// The exact tuple `_http_json` / `_http_stream` hands to urllib.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireRequest {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

/// Mirrors urllib's two failure families (`HTTPError` vs `URLError`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpError {
    /// 2xx was not received; `body` is the raw response text.
    Status { code: u16, body: String },
    /// DNS / TLS / connect / reset — the `URLError.reason` text.
    Network(String),
}

/// A byte-line reader over a streaming response.  Each item is one line
/// (Python's `for raw in resp`), already lossily decoded to UTF-8.
pub trait LineStream {
    /// `Ok(None)` == end of stream; `Err(reason)` == mid-stream `URLError`.
    fn next_line(&mut self) -> Result<Option<String>, String>;
}

pub trait Transport {
    /// `_http_json` (382-395).
    fn post_json(&self, url: &str, headers: &Headers, body: &str) -> Result<String, HttpError>;
    /// `_http_stream` (398-410).
    fn open_stream(&self, url: &str, headers: &Headers, body: &str) -> Result<Box<dyn LineStream>, HttpError>;
}

fn http_json(http: &dyn Transport, url: &str, headers: &Headers, body: &str) -> Result<String, AiError> {
    match http.post_json(url, &merge_default_headers(headers), body) {
        Ok(text) => Ok(text),
        Err(HttpError::Status { code, body }) => Err(AiError::new(format!(
            "HTTP {}：{}",
            code,
            py_slice_prefix(&body, 500)
        ))),
        Err(HttpError::Network(reason)) => Err(AiError::new(format!("网络错误：{}", reason))),
    }
}

fn http_stream(
    http: &dyn Transport,
    url: &str,
    headers: &Headers,
    body: &str,
) -> Result<Box<dyn LineStream>, AiError> {
    match http.open_stream(url, &merge_default_headers(headers), body) {
        Ok(stream) => Ok(stream),
        Err(HttpError::Status { code, body }) => Err(AiError::new(format!(
            "HTTP {}：{}",
            code,
            py_slice_prefix(&body, 500)
        ))),
        Err(HttpError::Network(reason)) => Err(AiError::new(format!("网络错误：{}", reason))),
    }
}

/// The real egress used by the binary: `ureq` + rustls, no subprocess.
pub mod ureq_transport {
    use super::{Headers, HttpError, LineStream, Transport};
    use std::io::{BufRead, BufReader, Read};
    use std::time::Duration;

    /// `timeout=240` for `_http_json`, `timeout=300` for `_http_stream`.
    #[derive(Debug, Clone, Copy)]
    pub struct UreqTransport {
        pub json_timeout: Duration,
        pub stream_timeout: Duration,
    }

    impl Default for UreqTransport {
        fn default() -> Self {
            UreqTransport {
                json_timeout: Duration::from_secs(240),
                stream_timeout: Duration::from_secs(300),
            }
        }
    }

    impl UreqTransport {
        pub fn new() -> Self {
            Self::default()
        }

        fn agent(&self, timeout: Duration) -> ureq::Agent {
            ureq::AgentBuilder::new()
                .timeout_read(timeout)
                .timeout_write(timeout)
                .redirects(10)
                .build()
        }
    }

    /// Lossy UTF-8, split on `\n` only — same as iterating a urllib response.
    struct ReaderLines {
        reader: BufReader<Box<dyn Read>>,
    }

    impl LineStream for ReaderLines {
        fn next_line(&mut self) -> Result<Option<String>, String> {
            let mut buf: Vec<u8> = Vec::new();
            match self.reader.read_until(b'\n', &mut buf) {
                Ok(0) => Ok(None),
                Ok(_) => Ok(Some(String::from_utf8_lossy(&buf).into_owned())),
                Err(e) => Err(format!("{}", e)),
            }
        }
    }

    fn read_body(resp: ureq::Response) -> String {
        let mut buf = Vec::new();
        let mut reader = resp.into_reader();
        let _ = reader.read_to_end(&mut buf);
        String::from_utf8_lossy(&buf).into_owned()
    }

    impl Transport for UreqTransport {
        fn post_json(&self, url: &str, headers: &Headers, body: &str) -> Result<String, HttpError> {
            let agent = self.agent(self.json_timeout);
            let mut call = agent.post(url);
            for (k, v) in headers {
                call = call.set(k, v);
            }
            match call.send_string(body) {
                Ok(resp) => Ok(read_body(resp)),
                Err(ureq::Error::Status(code, resp)) => Err(HttpError::Status { code, body: read_body(resp) }),
                Err(e) => Err(HttpError::Network(format!("{}", e))),
            }
        }

        fn open_stream(&self, url: &str, headers: &Headers, body: &str) -> Result<Box<dyn LineStream>, HttpError> {
            let agent = self.agent(self.stream_timeout);
            let mut call = agent.post(url);
            for (k, v) in headers {
                call = call.set(k, v);
            }
            match call.send_string(body) {
                Ok(resp) => {
                    let reader: Box<dyn Read> = Box::new(resp.into_reader());
                    Ok(Box::new(ReaderLines { reader: BufReader::new(reader) }))
                }
                Err(ureq::Error::Status(code, resp)) => Err(HttpError::Status { code, body: read_body(resp) }),
                Err(e) => Err(HttpError::Network(format!("{}", e))),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// message shaping (ai.py 413-441)
// ---------------------------------------------------------------------------

/// One `{"role": …, "content": …}` turn.  Modelled explicitly because
/// `serde_json::Value` objects sort their keys and this dict's order is
/// `role` then `content` on the wire.
#[derive(Debug, Clone, PartialEq)]
pub struct Turn {
    pub role: Value,
    pub content: Value,
}

impl Turn {
    pub fn text(role: &str, content: &str) -> Self {
        Turn { role: Value::String(role.to_string()), content: Value::String(content.to_string()) }
    }

    /// `json.dumps` of this dict, preserving insertion order.
    pub fn dump(&self, ascii_only: bool) -> String {
        dumps_ordered(&[
            ("role".to_string(), dumps_json(&self.role, ascii_only)),
            ("content".to_string(), dumps_json(&self.content, ascii_only)),
        ])
    }

    pub fn to_json(&self) -> Value {
        let mut o = Map::new();
        o.insert("role".to_string(), self.role.clone());
        o.insert("content".to_string(), self.content.clone());
        Value::Object(o)
    }
}

fn dump_turns(turns: &[Turn]) -> String {
    let parts: Vec<String> = turns.iter().map(|t| t.dump(true)).collect();
    format!("[{}]", parts.join(", "))
}

/// `ai.py::_openai_messages` (413-422).
pub fn openai_messages(messages: &[Value]) -> Vec<Turn> {
    let mut out: Vec<Turn> = Vec::new();
    for m in messages {
        let role = py_get(m, "role").cloned().unwrap_or_else(|| str_value("user"));
        // `m.get("content", "")`: a missing key yields "", a present null stays null.
        let content = match m {
            Value::Object(o) => match o.get("content") {
                Some(v) => v.clone(),
                None => empty_string(),
            },
            _ => empty_string(),
        };
        if role == str_value("system") {
            out.insert(0, Turn { role, content });
        } else {
            out.push(Turn { role, content });
        }
    }
    out
}

/// `ai.py::_anthropic_messages` (425-441).  Returns `(system, messages)`.
pub fn anthropic_messages(messages: &[Value]) -> Result<(String, Vec<Turn>), AiError> {
    let mut system: Vec<Value> = Vec::new();
    let mut msgs: Vec<Turn> = Vec::new();
    for m in messages {
        let role = py_get(m, "role").cloned().unwrap_or_else(|| str_value("user"));
        let content = match m {
            Value::Object(o) => match o.get("content") {
                Some(v) => v.clone(),
                None => empty_string(),
            },
            _ => empty_string(),
        };
        if role == str_value("system") {
            system.push(content);
        } else {
            let r = if role == str_value("assistant") { "assistant" } else { "user" };
            let same = !msgs.is_empty() && msgs[msgs.len() - 1].role == str_value(r);
            if same {
                let last = msgs.pop().expect("non-empty");
                // `msgs[-1]["content"] += "\n\n" + content` — the right-hand side
                // is evaluated first, so a non-str `content` errors before the
                // stored value is touched.
                let extra = match &content {
                    Value::String(c) => format!("\n\n{}", c),
                    other => return Err(AiError::concat_error(py_type_name(other))),
                };
                let joined = match &last.content {
                    Value::String(store) => Value::String(format!("{}{}", store, extra)),
                    other => return Err(AiError::unsupported_add(py_type_name(other), "str")),
                };
                msgs.push(Turn { role: Value::String(r.to_string()), content: joined });
            } else {
                msgs.push(Turn { role: Value::String(r.to_string()), content });
            }
        }
    }
    if msgs.is_empty() {
        msgs.push(Turn::text("user", "..."));
    }
    Ok((py_join("\n\n", &system)?, msgs))
}

/// `"sep".join(items)` with CPython's `TypeError` text.
fn py_join(sep: &str, items: &[Value]) -> Result<String, AiError> {
    let mut out = String::new();
    for (i, item) in items.iter().enumerate() {
        match item {
            Value::String(s) => {
                if i > 0 {
                    out.push_str(sep);
                }
                out.push_str(s);
            }
            other => return Err(AiError::seq_item(i, py_type_name(other))),
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Skills (ai.py 444-471, skills.py 35-37 / 160-172)
// ---------------------------------------------------------------------------

/// Ordered variable mapping: `dict(payload["skill_variables"])` plus the
/// `setdefault` chain of `_skill_messages`, whose *insertion order* is asserted
/// by the parity tests.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VarMap {
    pub entries: Vec<(String, Value)>,
}

impl VarMap {
    /// `dict(x)` — `None` when `x` is not a mapping (→ generic render failure).
    pub fn from_json(v: &Value) -> Option<VarMap> {
        match v {
            Value::Object(o) => Some(VarMap { entries: o.iter().map(|(k, val)| (k.clone(), val.clone())).collect() }),
            _ => None,
        }
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    /// `variables[key] = value`
    pub fn assign(&mut self, key: &str, value: Value) {
        match self.entries.iter_mut().find(|(k, _)| k == key) {
            Some(slot) => slot.1 = value,
            None => self.entries.push((key.to_string(), value)),
        }
    }

    /// `variables.setdefault(key, value)`
    pub fn set_default(&mut self, key: &str, value: Value) {
        if self.get(key).is_none() {
            self.entries.push((key.to_string(), value));
        }
    }

    pub fn keys(&self) -> Vec<String> {
        self.entries.iter().map(|(k, _)| k.clone()).collect()
    }

    pub fn to_json(&self) -> Value {
        let mut o = Map::new();
        for (k, v) in &self.entries {
            o.insert(k.clone(), v.clone());
        }
        Value::Object(o)
    }
}

/// `ReadMDCoreService.render_skill` — injected so this module stays free of
/// `crate::` references.
pub trait SkillService {
    fn render_skill(&self, skill_id: &str, variables: &VarMap) -> Result<String, SkillError>;
}

/// `ai.py::_skill_messages` (444-471).
pub fn skill_messages(payload: &Value, skills: &dyn SkillService) -> Result<Vec<Value>, AiError> {
    let raw_skill_id = py_get(payload, "skill_id").cloned().unwrap_or_else(null);
    let skill_id = py_strip(&chain_str(&[raw_skill_id], EMPTY_STR));
    let messages: Vec<Value> = match py_get(payload, "messages") {
        Some(v) if py_truthy(v) => py_iter_or_list(v),
        _ => Vec::new(),
    };
    if skill_id.is_empty() {
        return Ok(messages);
    }
    let rendered = render_variables(&messages, payload, &skill_id, skills);
    match rendered {
        Ok(system) => {
            let mut out = vec![Turn::text("system", &system).to_json()];
            for m in &messages {
                let role = py_get(m, "role").cloned().unwrap_or_else(null);
                if role != str_value("system") {
                    out.push(m.clone());
                }
            }
            Ok(out)
        }
        Err(err) => {
            if err.kind == SkillErrorKind::Skill {
                Err(AiError::new(err.message))
            } else {
                Err(AiError::new("Skill 渲染失败：请检查 Skill 模板或文档内容后重试"))
            }
        }
    }
}

/// The `try:` block of `_skill_messages` (456-465).
fn render_variables(
    messages: &[Value],
    payload: &Value,
    skill_id: &str,
    skills: &dyn SkillService,
) -> Result<String, SkillError> {
    let mut variables = match py_get(payload, "skill_variables") {
        Some(v) if py_truthy(v) => match VarMap::from_json(v) {
            Some(m) => m,
            None => return Err(SkillError::other("dict() argument must be a mapping")),
        },
        _ => VarMap::default(),
    };
    if !py_truthy(&variables.get("document").cloned().unwrap_or_else(null)) {
        let mut users: Vec<Value> = Vec::new();
        for m in messages {
            let role = py_get(m, "role").cloned().unwrap_or_else(null);
            if role == str_value("user") {
                let content = match m {
                    Value::Object(o) => match o.get("content") {
                        Some(v) => v.clone(),
                        None => empty_string(),
                    },
                    // `m.get(...)` on a non-mapping → AttributeError → generic failure
                    _ => return Err(SkillError::other(AiError::attribute_error(py_type_name(m)).message)),
                };
                users.push(content);
            }
        }
        let doc = users.last().cloned().unwrap_or_else(empty_string);
        variables.assign("document", doc);
    }
    variables.set_default("language", str_value("the document's language"));
    variables.set_default("request", empty_string());
    variables.set_default("context", empty_string());
    let doc = variables.get("document").cloned().unwrap_or_else(empty_string);
    variables.set_default("selection", doc);
    variables.set_default("output_format", str_value("Markdown"));
    skills.render_skill(skill_id, &variables)
}

/// `payload["messages"]` shaped by `list(x or [])`.
fn py_iter_or_list(v: &Value) -> Vec<Value> {
    match v {
        Value::Array(a) => a.clone(),
        other => py_iter(other).unwrap_or_default(),
    }
}

/// `Skill.variables` (skills.py 35-37): `sorted(set(_VARIABLE_RE.findall(t)))`.
pub fn template_variable_names(template: &str) -> Vec<String> {
    let mut found = variable_names_unsorted(template);
    found.sort();
    found.dedup();
    found
}

fn variable_names_unsorted(template: &str) -> Vec<String> {
    let re = match regex::Regex::new(r"\{\{\s*([a-zA-Z0-9_-]+)\s*\}\}") {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    re.captures_iter(template)
        .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
        .collect()
}

/// `SkillRegistry.render` (skills.py 160-172) as a pure function, so the
/// `SkillService` bridge and the parity tests share one implementation.
///
/// `required` is `skill.metadata.get("required_variables")` (the sidecar value;
/// front-matter is inert) and `variables` the resolved mapping.
pub fn render_skill_template(
    template: &str,
    required: Option<&Value>,
    variables: &VarMap,
) -> Result<String, SkillError> {
    let names = template_variable_names(template);
    let required_list: Vec<String> = match required {
        Some(v) if py_truthy(v) => match v {
            Value::Array(a) => a.iter().map(py_str).collect(),
            other => vec![py_str(other)],
        },
        _ => vec!["document".to_string()],
    };
    let mut missing: Vec<String> = Vec::new();
    for name in &required_list {
        if !names.iter().any(|n| n == name) {
            continue;
        }
        let raw = variables.get(name).cloned().unwrap_or_else(empty_string);
        if py_strip(&py_str(&raw)).is_empty() {
            missing.push(name.clone());
        }
    }
    if !missing.is_empty() {
        return Err(SkillError::skill(format!(
            "missing required Skill variables: {}",
            missing.join(", ")
        )));
    }
    let mut rendered = template.to_string();
    for name in &names {
        let value = variables.get(name).cloned().unwrap_or_else(empty_string);
        rendered = replace_template(&rendered, name, &py_str(&value));
    }
    Ok(py_strip(&rendered))
}

/// `re.sub(r"\{\{\s*<name>\s*\}\}", lambda m: value, rendered)` — the lambda
/// makes the replacement literal (no `\1` / `$` expansion).
pub fn replace_template(rendered: &str, name: &str, value: &str) -> String {
    let pattern = format!(r"\{{\{{\s*{}\s*\}}\}}", regex::escape(name));
    match regex::Regex::new(&pattern) {
        // `NoExpand` == CPython's `lambda m: value`: no \\1 / $name expansion.
        Ok(re) => re.replace_all(rendered, regex::NoExpand(value)).to_string(),
        Err(_) => rendered.to_string(),
    }
}

// ---------------------------------------------------------------------------
// usage (ai.py 574-585, 785-795)
// ---------------------------------------------------------------------------

/// `usage` dicts keep CPython's insertion order (`prompt_tokens`,
/// `completion_tokens`, `total_tokens`) for the SSE `usage` event.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Usage {
    pub fields: Vec<(String, i64)>,
}

impl Usage {
    pub fn new() -> Self {
        Usage { fields: Vec::new() }
    }
    pub fn push(&mut self, key: &str, value: i64) {
        self.fields.push((key.to_string(), value));
    }
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }
    /// Python truthiness of the dict.
    pub fn truthy(&self) -> bool {
        !self.fields.is_empty()
    }
    pub fn get(&self, key: &str) -> Option<i64> {
        self.fields.iter().find(|(k, _)| k == key).map(|(_, v)| *v)
    }
    pub fn to_json(&self) -> Value {
        let mut o = Map::new();
        for (k, v) in &self.fields {
            o.insert(k.clone(), Value::from(*v));
        }
        Value::Object(o)
    }
    pub fn dump(&self) -> String {
        let pairs: Vec<(String, String)> = self
            .fields
            .iter()
            .map(|(k, v)| (k.clone(), v.to_string()))
            .collect();
        dumps_ordered(&pairs)
    }
    pub fn from_json(v: &Value) -> Option<Usage> {
        let o = v.as_object()?;
        let mut out = Usage::new();
        for key in ["prompt_tokens", "completion_tokens", "total_tokens"] {
            if let Some(n) = o.get(key).and_then(|n| n.as_i64()) {
                out.push(key, n);
            }
        }
        Some(out)
    }
}

/// `ai.py::_openai_usage` (574-585).  Note `input_tokens` / `output_tokens` are
/// intentionally *ignored* by this function.
pub fn openai_usage(d: &Value) -> Result<Option<Usage>, AiError> {
    let u = py_get_checked(d, "usage")?.cloned().unwrap_or_else(null);
    if !py_truthy(&u) {
        return Ok(None);
    }
    let obj = match &u {
        Value::Object(o) => o,
        other => return Err(AiError::attribute_error(py_type_name(other))),
    };
    let mut out = Usage::new();
    for key in ["prompt_tokens", "completion_tokens", "total_tokens"] {
        if let Some(v) = obj.get(key) {
            if !v.is_null() {
                out.push(key, py_int(v)?);
            }
        }
    }
    Ok(if out.is_empty() { None } else { Some(out) })
}

/// `ai.py::_anthropic_usage` (785-795).
pub fn anthropic_usage(u: &Value) -> Result<Option<Usage>, AiError> {
    let obj = match u {
        Value::Object(o) => o,
        other => return Err(AiError::attribute_error(py_type_name(other))),
    };
    let mut out = Usage::new();
    let input = obj.get("input_tokens");
    if let Some(v) = input {
        if !v.is_null() {
            out.push("prompt_tokens", py_int(v)?);
        }
    }
    let output = obj.get("output_tokens");
    if let Some(v) = output {
        if !v.is_null() {
            out.push("completion_tokens", py_int(v)?);
        }
    }
    let p = out.get("prompt_tokens");
    let c = out.get("completion_tokens");
    if let (Some(p), Some(c)) = (p, c) {
        out.push("total_tokens", p + c);
    }
    Ok(if out.is_empty() { None } else { Some(out) })
}

/// `int(x)` for JSON-parsed values (truncates floats toward zero, accepts
/// numeric strings).
pub fn py_int(v: &Value) -> Result<i64, AiError> {
    match v {
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                return Ok(i);
            }
            if let Some(u) = n.as_u64() {
                return Ok(u as i64);
            }
            match n.as_f64() {
                Some(f) if f.is_finite() => Ok(f.trunc() as i64),
                _ => Err(AiError::new(format!(
                    "OverflowError: cannot convert float infinity to integer"
                ))),
            }
        }
        Value::Bool(b) => Ok(if *b { 1 } else { 0 }),
        Value::String(s) => python_int_str(s).ok_or_else(|| AiError::value_error(s)),
        other => Err(AiError::new(format!(
            "TypeError: int() argument must be a string, a bytes-like object or a real number, not '{}'",
            py_type_name(other)
        ))),
    }
}

/// `int(str)` — optional whitespace/sign, decimal digits, `_` between digits.
fn python_int_str(s: &str) -> Option<i64> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    let (neg, digits) = if let Some(rest) = t.strip_prefix('-') {
        (true, rest)
    } else if let Some(rest) = t.strip_prefix('+') {
        (false, rest)
    } else {
        (false, t)
    };
    if digits.is_empty() || digits.starts_with('_') || digits.ends_with('_') || digits.contains("__") {
        return None;
    }
    let mut acc: i64 = 0;
    for ch in digits.chars() {
        if ch == '_' {
            continue;
        }
        if !ch.is_ascii_digit() {
            return None;
        }
        let d = i64::from(ch as u8 - b'0');
        acc = acc.checked_mul(10)?.checked_add(if neg { -d } else { d })?;
    }
    Some(acc)
}

// ---------------------------------------------------------------------------
// events + routing (ai.py 477-526)
// ---------------------------------------------------------------------------

/// What the `chat()` generator yields: `str` deltas and a trailing
/// `{'usage': {...}}` event.  Deltas stay a [`Value`] because Python yields the
/// provider's raw value (a non-string only for malformed upstreams).
#[derive(Debug, Clone, PartialEq)]
pub enum ChatEvent {
    Delta(Value),
    Usage(Usage),
}

pub type ChatStream = Box<dyn Iterator<Item = Result<ChatEvent, AiError>>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteTarget {
    /// `_chat_openai` — `/chat/completions`
    Chat,
    /// `_chat_openai_completion` — `/completions`
    Completion,
    /// `_chat_openai_responses` — `/responses`
    Responses,
    /// `_chat_anthropic` — `/v1/messages`
    Anthropic,
}

/// The argument tuple every `_chat_*` function receives.
#[derive(Debug, Clone, PartialEq)]
pub struct RouteArgs {
    pub base_url: String,
    pub api_key: String,
    pub model: Value,
    pub messages: Vec<Value>,
    pub temperature: Value,
    pub stream: bool,
    pub endpoint_mode: String,
    pub custom_headers: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedRoute {
    pub target: RouteTarget,
    pub args: RouteArgs,
}

/// Everything `chat()` reads out of the persisted configuration.
pub trait ProviderDirectory {
    /// `ensure_config().get("current") if isinstance(..., dict) else {}`
    fn current_config(&self) -> Value;
    /// `find_provider(name)` — `{}`/`None` when unknown.
    fn find_provider(&self, name: &str) -> Value;
    /// `find_provider_by_credential(credential_id)`
    fn find_provider_by_credential(&self, credential_id: &str) -> Option<Value>;
    /// `load_credential(credential_id)`
    fn load_credential(&self, credential_id: &str) -> String;
    /// `decrypt_api_key(p["api_key"])` for the `enc:` compatibility path.
    fn decrypt_secret(&self, encoded: &str) -> String;
    /// `os.environ.get(name, "")`
    fn env_var(&self, name: &str) -> String;
}

/// `ai.py::resolve_key` (328-340).
pub fn resolve_key(p: &Value, dir: &dyn ProviderDirectory) -> Result<String, AiError> {
    let credential_id = py_get_checked(p, "credential_id")?.cloned().unwrap_or_else(null);
    if py_truthy(&credential_id) {
        return Ok(dir.load_credential(&py_str(&credential_id)));
    }
    let api_key = py_get(p, "api_key")
        .cloned()
        .unwrap_or_else(empty_string);
    match &api_key {
        Value::String(s) => {
            if s.starts_with("enc:") {
                return Ok(dir.decrypt_secret(s));
            }
        }
        // `p.get("api_key", "").startswith("enc:")` on a non-str
        other => return Err(AiError::attribute_error_on(py_type_name(other), "startswith")),
    }
    let env = py_get(p, "env_key").cloned().unwrap_or_else(null);
    if py_truthy(&env) {
        let v = dir.env_var(&py_str(&env));
        if !v.is_empty() {
            return Ok(v);
        }
    }
    Ok(String::new())
}

/// `ai.py::_is_local_provider` (352-365).
pub fn is_local_provider(provider: &Value) -> bool {
    if !provider.is_object() {
        return false;
    }
    let category = py_get(provider, "category").cloned().unwrap_or_else(null);
    let category = py_str(&first_truthy(&[&category], &empty_string())).to_lowercase();
    if category == "local" {
        return true;
    }
    let base = py_get(provider, "base_url").cloned().unwrap_or_else(null);
    let base_url = py_str(&first_truthy(&[&base], &empty_string())).to_lowercase();
    ["localhost", "127.0.0.1", "::1"].iter().any(|host| base_url.contains(host))
}

/// `ai.py::chat` — the eager half (validation, credential resolution, Skill
/// rendering, route selection).  HTTP happens in [`dispatch_chat`].
pub fn resolve_chat(
    payload: &Value,
    dir: &dyn ProviderDirectory,
    skills: &dyn SkillService,
) -> Result<ResolvedRoute, AiError> {
    if !payload.is_object() {
        return Err(AiError::attribute_error(py_type_name(payload)));
    }
    // --- provider name -----------------------------------------------------
    // `current` stays empty whenever the payload named a provider, so
    // `current.model` is only consulted on the config-derived path (481-486).
    let provided = py_get(payload, "provider").cloned().unwrap_or_else(null);
    let (current, name) = if py_truthy(&provided) {
        (Value::Object(Map::new()), py_str(&provided))
    } else {
        let mut current = dir.current_config();
        if !current.is_object() {
            current = Value::Object(Map::new());
        }
        let a = py_get(&current, "provider_id").cloned().unwrap_or_else(null);
        let b = py_get(&current, "provider").cloned().unwrap_or_else(null);
        let picked = first_truthy(&[&a, &b], &empty_string()).clone();
        let name = py_strip(&py_str(&picked));
        (current, name)
    };

    // --- provider record ---------------------------------------------------
    let mut prov = if name.is_empty() { Value::Object(Map::new()) } else { dir.find_provider(&name) };
    if !py_truthy(&prov) && !name.is_empty() {
        return Err(AiError::new(format!("未知提供商：{}", name)));
    }
    if !py_truthy(&prov) {
        prov = Value::Object(Map::new());
    }

    // --- base_url / format -------------------------------------------------
    let p_base = py_get(payload, "base_url").cloned().unwrap_or_else(null);
    let v_base = py_get(&prov, "base_url").cloned().unwrap_or_else(null);
    let base_chain = first_truthy(&[&p_base, &v_base], &empty_string()).clone();
    let base_url = py_rstrip_char(&py_str(&base_chain), '/');
    let p_fmt = py_get(payload, "format").cloned().unwrap_or_else(null);
    let v_fmt = py_get(&prov, "format").cloned().unwrap_or_else(null);
    let fmt = chain_str(&[p_fmt, v_fmt], "openai");

    // --- credential --------------------------------------------------------
    let raw_cred = py_get(payload, "credential_id").cloned().unwrap_or_else(null);
    let credential_id = py_strip(&py_str(&first_truthy(&[&raw_cred], &empty_string())));
    if !credential_id.is_empty() {
        let prov_cred = py_get(&prov, "credential_id").cloned().unwrap_or_else(null);
        if py_truthy(&prov_cred) {
            if prov_cred != Value::String(credential_id.clone()) {
                return Err(AiError::new("凭据与提供商不匹配"));
            }
        } else {
            let found = dir.find_provider_by_credential(&credential_id);
            if let Some(f) = found {
                if py_truthy(&f) {
                    prov = f;
                }
            }
        }
    }
    let payload_key = py_get(payload, "api_key").cloned().unwrap_or_else(null);
    let api_key = if py_truthy(&payload_key) {
        py_str(&payload_key)
    } else {
        resolve_key(&prov, dir)?
    };
    if api_key.is_empty() && !is_local_provider(&prov) {
        let env_key = py_get(&prov, "env_key").cloned().unwrap_or_else(null);
        let env_key = chain_str(&[env_key], EMPTY_STR);
        return Err(AiError::new(format!(
            "未配置 API Key（可填入界面，或设置环境变量 {}）",
            env_key
        )));
    }

    // --- model -------------------------------------------------------------
    let m1 = py_get(payload, "model").cloned().unwrap_or_else(null);
    let m2 = py_get(&current, "model").cloned().unwrap_or_else(null);
    let models = py_get(&prov, "models").cloned().unwrap_or_else(null);
    let m3 = models_first(&models)?;
    let model = first_truthy(&[&m1, &m2, &m3, &empty_string()], &empty_string()).clone();

    // --- messages (Skill rendering) ---------------------------------------
    let messages = skill_messages(payload, skills)?;

    // --- sampling / protocol knobs ----------------------------------------
    let temperature = py_get(payload, "temperature").cloned().unwrap_or_else(|| Value::from(0.4));
    let stream = match py_get(payload, "stream") {
        Some(v) => py_truthy(v),
        None => true,
    };
    let p_mode = py_get(payload, "mode").cloned().unwrap_or_else(null);
    let v_mode = py_get(&prov, "mode").cloned().unwrap_or_else(null);
    let mut mode = chain_str(&[p_mode, v_mode], EMPTY_STR);
    mode = mode.trim().to_lowercase();
    if mode.is_empty() {
        mode = if fmt == "anthropic" { "messages".to_string() } else { "auto".to_string() };
    }
    let p_em = py_get(payload, "endpoint_mode").cloned().unwrap_or_else(null);
    let v_em = py_get(&prov, "endpoint_mode").cloned().unwrap_or_else(null);
    let endpoint_mode = chain_str(&[p_em, v_em], "prefix");
    let payload_headers = py_get(payload, "headers").cloned().unwrap_or_else(null);
    let custom_headers = if payload_headers.is_object() {
        payload_headers
    } else {
        py_get(&prov, "headers").cloned().unwrap_or_else(null)
    };

    let target = if mode == "messages" || mode == "anthropic" {
        RouteTarget::Anthropic
    } else if mode == "completion" {
        RouteTarget::Completion
    } else if mode == "responses" {
        RouteTarget::Responses
    } else {
        RouteTarget::Chat
    };

    Ok(ResolvedRoute {
        target,
        args: RouteArgs {
            base_url,
            api_key,
            model,
            messages,
            temperature,
            stream,
            endpoint_mode,
            custom_headers,
        },
    })
}

/// `(provider.get("models") or [""])[0]`
fn models_first(v: &Value) -> Result<Value, AiError> {
    if !py_truthy(v) {
        return Ok(empty_string());
    }
    py_index0(v)
}

/// `ai.py::chat` — the dispatch tail (516-526).
pub fn dispatch_chat(route: &ResolvedRoute, http: &dyn Transport) -> Result<ChatStream, AiError> {
    match route.target {
        RouteTarget::Anthropic => chat_anthropic(&route.args, http),
        RouteTarget::Completion => chat_openai_completion(&route.args, http),
        RouteTarget::Responses => chat_openai_responses(&route.args, http),
        RouteTarget::Chat => chat_openai(&route.args, http),
    }
}

/// `ai.py::chat` (477-526) end to end.
pub fn chat(
    payload: &Value,
    dir: &dyn ProviderDirectory,
    skills: &dyn SkillService,
    http: &dyn Transport,
) -> Result<ChatStream, AiError> {
    let route = resolve_chat(payload, dir, skills)?;
    dispatch_chat(&route, http)
}

// ---------------------------------------------------------------------------
// request bodies / headers per route
// ---------------------------------------------------------------------------

fn openai_headers(api_key: &str) -> Headers {
    let mut h: Headers = vec![("Content-Type".to_string(), "application/json".to_string())];
    for (k, v) in openai_auth_headers(api_key) {
        merge_header(&mut h, &k, &v);
    }
    h
}

fn anthropic_base_headers(api_key: &str) -> Headers {
    let mut h: Headers = vec![("Content-Type".to_string(), "application/json".to_string())];
    if !api_key.is_empty() {
        h.push(("x-api-key".to_string(), api_key.to_string()));
    }
    h.push(("anthropic-version".to_string(), "2023-06-01".to_string()));
    h
}

fn custom_ref(args: &RouteArgs) -> Option<&Value> {
    if args.custom_headers.is_null() {
        None
    } else {
        Some(&args.custom_headers)
    }
}

fn openai_body(args: &RouteArgs, turns: &[Turn], with_stream_options: bool) -> String {
    let mut pairs: Vec<(String, String)> = vec![
        ("model".to_string(), dumps_json(&args.model, true)),
        ("messages".to_string(), dump_turns(turns)),
        ("stream".to_string(), if args.stream { "true".to_string() } else { "false".to_string() }),
        ("temperature".to_string(), dumps_json(&args.temperature, true)),
    ];
    if with_stream_options {
        pairs.push((
            "stream_options".to_string(),
            dumps_ordered(&[("include_usage".to_string(), "true".to_string())]),
        ));
    }
    dumps_ordered(&pairs)
}

fn completion_body(args: &RouteArgs, prompt: &str) -> String {
    dumps_ordered(&[
        ("model".to_string(), dumps_json(&args.model, true)),
        ("prompt".to_string(), dumps_json(&Value::String(prompt.to_string()), true)),
        ("max_tokens".to_string(), "4096".to_string()),
        ("temperature".to_string(), dumps_json(&args.temperature, true)),
        ("stream".to_string(), if args.stream { "true".to_string() } else { "false".to_string() }),
    ])
}

fn responses_body(args: &RouteArgs, turns: &[Turn]) -> String {
    dumps_ordered(&[
        ("model".to_string(), dumps_json(&args.model, true)),
        ("input".to_string(), dump_turns(turns)),
        ("stream".to_string(), if args.stream { "true".to_string() } else { "false".to_string() }),
        ("temperature".to_string(), dumps_json(&args.temperature, true)),
    ])
}

fn anthropic_body(args: &RouteArgs, system: &str, turns: &[Turn]) -> String {
    let mut pairs: Vec<(String, String)> = vec![
        ("model".to_string(), dumps_json(&args.model, true)),
        ("max_tokens".to_string(), "4096".to_string()),
        ("messages".to_string(), dump_turns(turns)),
        ("temperature".to_string(), dumps_json(&args.temperature, true)),
        ("stream".to_string(), if args.stream { "true".to_string() } else { "false".to_string() }),
    ];
    if !system.is_empty() {
        pairs.push(("system".to_string(), dumps_json(&Value::String(system.to_string()), true)));
    }
    dumps_ordered(&pairs)
}

/// `"\n\n".join(m.get("content", "") for m in msgs if m.get("content"))`
fn completion_prompt(turns: &[Turn]) -> Result<String, AiError> {
    let kept: Vec<Value> = turns
        .iter()
        .filter(|t| py_truthy(&t.content))
        .map(|t| t.content.clone())
        .collect();
    py_join("\n\n", &kept)
}

fn box_iter(events: Vec<ChatEvent>) -> ChatStream {
    Box::new(events.into_iter().map(Ok))
}

fn parse_failure(text: &str) -> AiError {
    AiError::new(format!("响应解析失败：{}", py_slice_prefix(text, 300)))
}

// ---------------------------------------------------------------------------
// SSE frame machinery
// ---------------------------------------------------------------------------

enum FrameOut {
    Nothing,
    Delta(Value),
    Stop,
    Fail(AiError),
}

/// One provider's per-frame state machine, i.e. the body of a Python
/// generator's `for raw in resp:` loop.
trait SseMachine {
    fn on_frame(&mut self, frame: &Value) -> FrameOut;
    /// Statements after the loop (`if usage: yield {"usage": usage}`).
    fn tail(&mut self) -> Vec<ChatEvent>;
}

/// Pull-based equivalent of the Python generators: lines are read one frame at
/// a time, so deltas reach the caller as they arrive.
struct SseIter {
    lines: Box<dyn LineStream>,
    machine: Box<dyn SseMachine>,
    queue: VecDeque<ChatEvent>,
    reading: bool,
    tailed: bool,
    poisoned: bool,
}

impl Iterator for SseIter {
    type Item = Result<ChatEvent, AiError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(ev) = self.queue.pop_front() {
            return Some(Ok(ev));
        }
        if self.poisoned {
            return None;
        }
        while self.reading {
            match self.step() {
                Ok(()) => {
                    if let Some(ev) = self.queue.pop_front() {
                        return Some(Ok(ev));
                    }
                }
                Err(e) => {
                    self.poisoned = true;
                    self.reading = false;
                    return Some(Err(e));
                }
            }
        }
        if !self.tailed {
            self.tailed = true;
            for ev in self.machine.tail() {
                self.queue.push_back(ev);
            }
        }
        self.queue.pop_front().map(Ok)
    }
}

impl SseIter {
    fn new(lines: Box<dyn LineStream>, machine: Box<dyn SseMachine>) -> SseIter {
        SseIter { lines, machine, queue: VecDeque::new(), reading: true, tailed: false, poisoned: false }
    }

    /// Reads lines until one frame has been handed to the machine; returning
    /// `Ok(())` with `reading == false` means the loop is over (`[DONE]`, EOF or
    /// a provider-specific `break`).
    fn step(&mut self) -> Result<(), AiError> {
        loop {
            let raw = match self.lines.next_line() {
                Ok(Some(line)) => line,
                Ok(None) => {
                    self.reading = false;
                    return Ok(());
                }
                Err(reason) => return Err(AiError::new(format!("连接中断：{}", reason))),
            };
            if raw.is_empty() {
                continue;
            }
            let line = py_strip(&raw);
            if !line.starts_with("data:") {
                continue;
            }
            let data = py_strip(&line["data:".len()..]);
            if data == "[DONE]" {
                self.reading = false;
                return Ok(());
            }
            let frame: Value = match serde_json::from_str(&data) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if !frame.is_object() {
                return Err(AiError::attribute_error(py_type_name(&frame)));
            }
            return match self.machine.on_frame(&frame) {
                FrameOut::Nothing => Ok(()),
                FrameOut::Stop => {
                    self.reading = false;
                    Ok(())
                }
                FrameOut::Fail(e) => Err(e),
                FrameOut::Delta(v) => {
                    self.queue.push_back(ChatEvent::Delta(v));
                    Ok(())
                }
            };
        }
    }
}

// ---------------------------------------------------------------------------
// _chat_openai — /chat/completions (ai.py 588-651)
// ---------------------------------------------------------------------------

pub fn chat_openai(args: &RouteArgs, http: &dyn Transport) -> Result<ChatStream, AiError> {
    let url = endpoint_url(&args.base_url, "chat/completions", &args.endpoint_mode);
    let turns = openai_messages(&args.messages);
    let body = openai_body(args, &turns, args.stream);
    let headers = request_headers(&openai_headers(&args.api_key), custom_ref(args));
    if !args.stream {
        let text = http_json(http, &url, &headers, &body)?;
        return Ok(box_iter(parse_openai_chat(&text)?));
    }
    let lines = http_stream(http, &url, &headers, &body)?;
    Ok(Box::new(SseIter::new(lines, Box::new(OpenAiChatMachine::default()))))
}

struct OpenAiChatMachine {
    usage: Option<Usage>,
}

impl Default for OpenAiChatMachine {
    fn default() -> Self {
        OpenAiChatMachine { usage: None }
    }
}

impl SseMachine for OpenAiChatMachine {
    fn on_frame(&mut self, d: &Value) -> FrameOut {
        match openai_usage(d) {
            Err(e) => return FrameOut::Fail(e),
            Ok(Some(u)) => self.usage = Some(u),
            Ok(None) => {}
        }
        let choices = match py_get_checked(d, "choices") {
            Ok(v) => v.cloned().unwrap_or_else(null),
            Err(e) => return FrameOut::Fail(e),
        };
        if !py_truthy(&choices) {
            return FrameOut::Nothing;
        }
        let first = match py_index0(&choices) {
            Ok(v) => v,
            Err(e) => return FrameOut::Fail(e),
        };
        let delta = match py_get_checked(&first, "delta") {
            Ok(v) => v.cloned().unwrap_or_else(null),
            Err(e) => return FrameOut::Fail(e),
        };
        let delta = if py_truthy(&delta) { delta } else { Value::Object(Map::new()) };
        let content = match py_get_checked(&delta, "content") {
            Ok(v) => v.cloned().unwrap_or_else(empty_string),
            Err(e) => return FrameOut::Fail(e),
        };
        if py_truthy(&content) {
            FrameOut::Delta(content)
        } else {
            FrameOut::Nothing
        }
    }

    fn tail(&mut self) -> Vec<ChatEvent> {
        match &self.usage {
            Some(u) if u.truthy() => vec![ChatEvent::Usage(u.clone())],
            _ => Vec::new(),
        }
    }
}

/// The `if not stream:` block of `_chat_openai` (596-610).
pub fn parse_openai_chat(text: &str) -> Result<Vec<ChatEvent>, AiError> {
    nonstream(text, |d| {
        let usage = openai_usage(d)?;
        let choices = py_subscript_map(d, "choices")?;
        let first = py_index0(&choices)?;
        let message = py_subscript_map(&first, "message")?;
        let content = py_subscript_map(&message, "content")?;
        let content = first_truthy_owned(content, empty_string());
        Ok((content, usage))
    })
}

fn first_truthy_owned(v: Value, fallback: Value) -> Value {
    if py_truthy(&v) {
        v
    } else {
        fallback
    }
}

/// Shared `try: … except Exception: raise ChatError("响应解析失败：%s" % text[:300])`
/// shape, with the `if content: yield content` / `if usage: yield {"usage": …}`
/// tail.
fn nonstream<F>(text: &str, f: F) -> Result<Vec<ChatEvent>, AiError>
where
    F: FnOnce(&Value) -> Result<(Value, Option<Usage>), AiError>,
{
    let parsed: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return Err(parse_failure(text)),
    };
    match f(&parsed) {
        Ok((content, usage)) => {
            let mut events = Vec::new();
            if py_truthy(&content) {
                events.push(ChatEvent::Delta(content));
            }
            if let Some(u) = usage {
                if u.truthy() {
                    events.push(ChatEvent::Usage(u));
                }
            }
            Ok(events)
        }
        Err(_) => Err(parse_failure(text)),
    }
}

// ---------------------------------------------------------------------------
// _chat_openai_completion — /completions (ai.py 654-716)
// ---------------------------------------------------------------------------

pub fn chat_openai_completion(args: &RouteArgs, http: &dyn Transport) -> Result<ChatStream, AiError> {
    let url = endpoint_url(&args.base_url, "completions", &args.endpoint_mode);
    let turns = openai_messages(&args.messages);
    let prompt_text = completion_prompt(&turns)?;
    let body = completion_body(args, &prompt_text);
    let headers = request_headers(&openai_headers(&args.api_key), custom_ref(args));
    if !args.stream {
        let text = http_json(http, &url, &headers, &body)?;
        return Ok(box_iter(parse_openai_completion(&text)?));
    }
    let lines = http_stream(http, &url, &headers, &body)?;
    Ok(Box::new(SseIter::new(lines, Box::new(OpenAiCompletionMachine::default()))))
}

struct OpenAiCompletionMachine {
    usage: Option<Usage>,
}

impl Default for OpenAiCompletionMachine {
    fn default() -> Self {
        OpenAiCompletionMachine { usage: None }
    }
}

impl SseMachine for OpenAiCompletionMachine {
    fn on_frame(&mut self, d: &Value) -> FrameOut {
        match openai_usage(d) {
            Err(e) => return FrameOut::Fail(e),
            Ok(Some(u)) => self.usage = Some(u),
            Ok(None) => {}
        }
        let choices = match py_get_checked(d, "choices") {
            Ok(v) => v.cloned().unwrap_or_else(null),
            Err(e) => return FrameOut::Fail(e),
        };
        if !py_truthy(&choices) {
            return FrameOut::Nothing;
        }
        let first = match py_index0(&choices) {
            Ok(v) => v,
            Err(e) => return FrameOut::Fail(e),
        };
        // Legacy streams carry `text`; `delta.content` is never read here.
        let delta = match py_get_checked(&first, "delta") {
            Ok(v) => v.cloned().unwrap_or_else(null),
            Err(e) => return FrameOut::Fail(e),
        };
        let delta = if py_truthy(&delta) { delta } else { Value::Object(Map::new()) };
        let piece = match py_get_checked(&delta, "text") {
            Ok(v) => v.cloned().unwrap_or_else(empty_string),
            Err(e) => return FrameOut::Fail(e),
        };
        if py_truthy(&piece) {
            FrameOut::Delta(piece)
        } else {
            FrameOut::Nothing
        }
    }

    fn tail(&mut self) -> Vec<ChatEvent> {
        match &self.usage {
            Some(u) if u.truthy() => vec![ChatEvent::Usage(u.clone())],
            _ => Vec::new(),
        }
    }
}

pub fn parse_openai_completion(text: &str) -> Result<Vec<ChatEvent>, AiError> {
    nonstream(text, |d| {
        let usage = openai_usage(d)?;
        let choices = py_subscript_map(d, "choices")?;
        let first = py_index0(&choices)?;
        let piece = match &first {
            Value::Object(_) => py_get(&first, "text").cloned().unwrap_or_else(empty_string),
            other => return Err(AiError::attribute_error(py_type_name(other))),
        };
        let piece = first_truthy_owned(piece, empty_string());
        Ok((piece, usage))
    })
}

// ---------------------------------------------------------------------------
// _chat_openai_responses — /responses (ai.py 719-784)
// ---------------------------------------------------------------------------

pub fn chat_openai_responses(args: &RouteArgs, http: &dyn Transport) -> Result<ChatStream, AiError> {
    let url = endpoint_url(&args.base_url, "responses", &args.endpoint_mode);
    let mut turns: Vec<Turn> = Vec::new();
    for m in openai_messages(&args.messages) {
        if m.role == str_value("system") {
            turns.insert(0, Turn { role: str_value("system"), content: m.content });
        } else {
            turns.push(m);
        }
    }
    let body = responses_body(args, &turns);
    let headers = request_headers(&openai_headers(&args.api_key), custom_ref(args));
    if !args.stream {
        let text = http_json(http, &url, &headers, &body)?;
        return Ok(box_iter(parse_openai_responses(&text)?));
    }
    let lines = http_stream(http, &url, &headers, &body)?;
    Ok(Box::new(SseIter::new(lines, Box::new(OpenAiResponsesMachine::default()))))
}

struct OpenAiResponsesMachine {
    usage: Option<Usage>,
}

impl Default for OpenAiResponsesMachine {
    fn default() -> Self {
        OpenAiResponsesMachine { usage: None }
    }
}

impl SseMachine for OpenAiResponsesMachine {
    fn on_frame(&mut self, d: &Value) -> FrameOut {
        let kind = match py_get_checked(d, "type") {
            Ok(v) => v.cloned().unwrap_or_else(null),
            Err(e) => return FrameOut::Fail(e),
        };
        if kind == str_value("response.output_text.delta") {
            let dt = match py_get_checked(d, "delta") {
                Ok(v) => v.cloned().unwrap_or_else(null),
                Err(e) => return FrameOut::Fail(e),
            };
            let dt = if py_truthy(&dt) { dt } else { Value::Object(Map::new()) };
            let text = match py_get_checked(&dt, "text") {
                Ok(v) => v.cloned().unwrap_or_else(empty_string),
                Err(e) => return FrameOut::Fail(e),
            };
            return if py_truthy(&text) {
                FrameOut::Delta(text)
            } else {
                FrameOut::Nothing
            };
        }
        if kind == str_value("response.completed") {
            let r2 = match py_get_checked(d, "response") {
                Ok(v) => v.cloned().unwrap_or_else(null),
                Err(e) => return FrameOut::Fail(e),
            };
            let r2 = if py_truthy(&r2) { r2 } else { Value::Object(Map::new()) };
            match openai_usage(&r2) {
                Err(e) => return FrameOut::Fail(e),
                Ok(Some(u)) => self.usage = Some(u),
                Ok(None) => {}
            }
            return FrameOut::Nothing;
        }
        if kind == str_value("error") {
            return FrameOut::Fail(AiError::new(format!(
                "提供商错误：{}",
                py_slice_prefix(&py_dumps_unicode(d), 300)
            )));
        }
        FrameOut::Nothing
    }

    fn tail(&mut self) -> Vec<ChatEvent> {
        match &self.usage {
            Some(u) if u.truthy() => vec![ChatEvent::Usage(u.clone())],
            _ => Vec::new(),
        }
    }
}

pub fn parse_openai_responses(text: &str) -> Result<Vec<ChatEvent>, AiError> {
    nonstream(text, |d| {
        let usage = openai_usage(d)?;
        let text_value = match py_get_checked(d, "output_text") {
            Ok(v) => v.cloned().unwrap_or_else(empty_string),
            Err(e) => return Err(e),
        };
        Ok((first_truthy_owned(text_value, empty_string()), usage))
    })
}

// ---------------------------------------------------------------------------
// _chat_anthropic — /v1/messages (ai.py 798-866)
// ---------------------------------------------------------------------------

pub fn chat_anthropic(args: &RouteArgs, http: &dyn Transport) -> Result<ChatStream, AiError> {
    let url = endpoint_url(&args.base_url, "v1/messages", &args.endpoint_mode);
    let (system, turns) = anthropic_messages(&args.messages)?;
    let body = anthropic_body(args, &system, &turns);
    let headers = request_headers(&anthropic_base_headers(&args.api_key), custom_ref(args));
    if !args.stream {
        let text = http_json(http, &url, &headers, &body)?;
        return Ok(box_iter(parse_anthropic(&text)?));
    }
    let lines = http_stream(http, &url, &headers, &body)?;
    Ok(Box::new(SseIter::new(
        lines,
        Box::new(AnthropicMachine { input_tokens: None, output_tokens: 0 }),
    )))
}

struct AnthropicMachine {
    input_tokens: Option<i64>,
    output_tokens: i64,
}

impl SseMachine for AnthropicMachine {
    fn on_frame(&mut self, d: &Value) -> FrameOut {
        let kind = match py_get_checked(d, "type") {
            Ok(v) => v.cloned().unwrap_or_else(null),
            Err(e) => return FrameOut::Fail(e),
        };
        if kind == str_value("message_start") {
            let message = match py_get_checked(d, "message") {
                Ok(v) => v.cloned().unwrap_or_else(null),
                Err(e) => return FrameOut::Fail(e),
            };
            let message = if py_truthy(&message) { message } else { Value::Object(Map::new()) };
            let u = match py_get_checked(&message, "usage") {
                Ok(v) => v.cloned().unwrap_or_else(null),
                Err(e) => return FrameOut::Fail(e),
            };
            let u = if py_truthy(&u) { u } else { Value::Object(Map::new()) };
            if let Some(v) = py_get(&u, "input_tokens") {
                if !v.is_null() {
                    match py_int(v) {
                        Ok(i) => self.input_tokens = Some(i),
                        Err(e) => return FrameOut::Fail(e),
                    }
                }
            }
            return FrameOut::Nothing;
        }
        if kind == str_value("content_block_delta") {
            let delta = match py_get_checked(d, "delta") {
                Ok(v) => v.cloned().unwrap_or_else(null),
                Err(e) => return FrameOut::Fail(e),
            };
            let delta = if py_truthy(&delta) { delta } else { Value::Object(Map::new()) };
            let dtype = py_get(&delta, "type").cloned().unwrap_or_else(null);
            let text = py_get(&delta, "text").cloned().unwrap_or_else(empty_string);
            if dtype == str_value("text_delta") && py_truthy(&text) {
                return FrameOut::Delta(text);
            }
            return FrameOut::Nothing;
        }
        if kind == str_value("message_delta") {
            let u = match py_get_checked(d, "usage") {
                Ok(v) => v.cloned().unwrap_or_else(null),
                Err(e) => return FrameOut::Fail(e),
            };
            let u = if py_truthy(&u) { u } else { Value::Object(Map::new()) };
            if let Some(v) = py_get(&u, "output_tokens") {
                if !v.is_null() {
                    match py_int(v) {
                        Ok(i) => self.output_tokens = i,
                        Err(e) => return FrameOut::Fail(e),
                    }
                }
            }
            return FrameOut::Nothing;
        }
        if kind == str_value("message_stop") {
            return FrameOut::Stop;
        }
        if kind == str_value("error") {
            return FrameOut::Fail(AiError::new(format!(
                "提供商错误：{}",
                py_slice_prefix(&py_dumps_unicode(d), 300)
            )));
        }
        FrameOut::Nothing
    }

    fn tail(&mut self) -> Vec<ChatEvent> {
        let mut o = Map::new();
        o.insert(
            "input_tokens".to_string(),
            self.input_tokens.map(|v| Value::from(v)).unwrap_or_else(null),
        );
        o.insert("output_tokens".to_string(), Value::from(self.output_tokens));
        match anthropic_usage(&Value::Object(o)) {
            Ok(Some(u)) if u.truthy() => vec![ChatEvent::Usage(u)],
            _ => Vec::new(),
        }
    }
}

pub fn parse_anthropic(text: &str) -> Result<Vec<ChatEvent>, AiError> {
    nonstream(text, |d| {
        let blocks = match py_get(d, "content") {
            Some(v) => v.clone(),
            None => Value::Array(Vec::new()),
        };
        let items = py_iter(&blocks)?;
        let mut parts: Vec<Value> = Vec::new();
        for b in &items {
            let kind = py_get_checked(b, "type")?.cloned().unwrap_or_else(null);
            if kind == str_value("text") {
                parts.push(py_get_checked(b, "text")?.cloned().unwrap_or_else(empty_string));
            }
        }
        let content = py_join("", &parts)?;
        let usage_obj = py_get(d, "usage").cloned().unwrap_or_else(null);
        let usage_obj = if py_truthy(&usage_obj) { usage_obj } else { Value::Object(Map::new()) };
        let usage = anthropic_usage(&usage_obj)?;
        Ok((first_truthy_owned(Value::String(content), empty_string()), usage))
    })
}


// ---------------------------------------------------------------------------
// parity tests — every expectation below was recorded by running the real
// `src/readmd_modules/ai.py` / `skills.py` (see
// scratch/rust_parity/ai_providers_s7/probe.py -> out.json / wire.json)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Mutex;

    /// Records every `(url, headers, body)` tuple handed to the transport.
    pub struct MockTransport {
        pub calls: Mutex<Vec<WireRequest>>,
        pub json_text: String,
        pub stream_lines: Vec<String>,
        pub fail: Option<HttpError>,
    }

    impl MockTransport {
        fn new(json_text: &str, stream_lines: Vec<String>) -> MockTransport {
            MockTransport {
                calls: Mutex::new(Vec::new()),
                json_text: json_text.to_string(),
                stream_lines,
                fail: None,
            }
        }
        fn failing(err: HttpError) -> MockTransport {
            MockTransport { calls: Mutex::new(Vec::new()), json_text: String::new(), stream_lines: Vec::new(), fail: Some(err) }
        }
        fn last(&self) -> WireRequest {
            self.calls.lock().unwrap().last().cloned().expect("no request recorded")
        }
    }

    impl Transport for MockTransport {
        fn post_json(&self, url: &str, headers: &Headers, body: &str) -> Result<String, HttpError> {
            self.calls.lock().unwrap().push(WireRequest { url: url.to_string(), headers: headers.clone(), body: body.to_string() });
            match &self.fail {
                Some(e) => Err(e.clone()),
                None => Ok(self.json_text.clone()),
            }
        }
        fn open_stream(&self, url: &str, headers: &Headers, body: &str) -> Result<Box<dyn LineStream>, HttpError> {
            self.calls.lock().unwrap().push(WireRequest { url: url.to_string(), headers: headers.clone(), body: body.to_string() });
            match &self.fail {
                Some(e) => Err(e.clone()),
                None => Ok(Box::new(VecStream::new(self.stream_lines.clone()))),
            }
        }
    }

    struct VecStream {
        lines: Vec<String>,
        index: usize,
    }

    impl VecStream {
        fn new(lines: Vec<String>) -> VecStream {
            VecStream { lines, index: 0 }
        }
    }

    impl LineStream for VecStream {
        fn next_line(&mut self) -> Result<Option<String>, String> {
            if self.index >= self.lines.len() {
                return Ok(None);
            }
            let line = self.lines[self.index].clone();
            self.index += 1;
            Ok(Some(line))
        }
    }

    /// Minimal stand-in for the persisted provider configuration.
    #[derive(Clone)]
    struct MockDirectory {
        current: Value,
        providers: Vec<(String, Value)>,
        by_credential: Vec<(String, Value)>,
        credentials: Vec<(String, String)>,
        env: Vec<(String, String)>,
    }

    impl MockDirectory {
        fn empty() -> MockDirectory {
            MockDirectory {
                current: json!({}),
                providers: Vec::new(),
                by_credential: Vec::new(),
                credentials: Vec::new(),
                env: Vec::new(),
            }
        }
        fn with_provider(mut self, name: &str, provider: Value) -> MockDirectory {
            self.providers.push((name.to_string(), provider));
            self
        }
    }

    impl ProviderDirectory for MockDirectory {
        fn current_config(&self) -> Value {
            self.current.clone()
        }
        fn find_provider(&self, name: &str) -> Value {
            self.providers
                .iter()
                .find(|(k, _)| k == name)
                .map(|(_, v)| v.clone())
                .unwrap_or(Value::Null)
        }
        fn find_provider_by_credential(&self, credential_id: &str) -> Option<Value> {
            self.by_credential.iter().find(|(k, _)| k == credential_id).map(|(_, v)| v.clone())
        }
        fn load_credential(&self, credential_id: &str) -> String {
            self.credentials.iter().find(|(k, _)| k == credential_id).map(|(_, v)| v.clone()).unwrap_or_default()
        }
        /// stands in for `vault.decrypt_api_key("enc:…")`
        fn decrypt_secret(&self, encoded: &str) -> String {
            encoded.strip_prefix("enc:").unwrap_or(encoded).to_string()
        }
        fn env_var(&self, name: &str) -> String {
            self.env.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone()).unwrap_or_default()
        }
    }

    struct MockSkills {
        rendered: Result<String, SkillError>,
        seen: Mutex<Option<VarMap>>,
    }

    impl MockSkills {
        fn ok(rendered: &str) -> MockSkills {
            MockSkills { rendered: Ok(rendered.to_string()), seen: Mutex::new(None) }
        }
        fn fail(kind: SkillErrorKind, message: &str) -> MockSkills {
            MockSkills { rendered: Err(SkillError { kind, message: message.to_string() }), seen: Mutex::new(None) }
        }
    }

    impl SkillService for MockSkills {
        fn render_skill(&self, skill_id: &str, variables: &VarMap) -> Result<String, SkillError> {
            *self.seen.lock().unwrap() = Some(variables.clone());
            let _ = skill_id;
            self.rendered.clone()
        }
    }

    fn usage(pairs: &[(&str, i64)]) -> Usage {
        let mut u = Usage::new();
        for (k, v) in pairs {
            u.push(k, *v);
        }
        u
    }

    fn deltas(events: &[ChatEvent]) -> Vec<Value> {
        events
            .iter()
            .filter_map(|e| match e {
                ChatEvent::Delta(v) => Some(v.clone()),
                _ => None,
            })
            .collect()
    }

    fn usages(events: &[ChatEvent]) -> Vec<Usage> {
        events
            .iter()
            .filter_map(|e| match e {
                ChatEvent::Usage(u) => Some(u.clone()),
                _ => None,
            })
            .collect()
    }

    fn collect(payload: &Value, dir: &dyn ProviderDirectory, skills: &dyn SkillService, http: &dyn Transport) -> Result<Vec<ChatEvent>, AiError> {
        chat(payload, dir, skills, http)?.collect()
    }

    fn base_messages() -> Vec<Value> {
        vec![
            json!({"role": "system", "content": "sys A"}),
            json!({"role": "user", "content": "hello 你好😀"}),
            json!({"role": "user", "content": "second user"}),
            json!({"role": "assistant", "content": "prior answer"}),
        ]
    }

    fn openai_payload(extra: Value) -> Value {
        let mut p = json!({
            "provider": "openai",
            "model": "gpt-4o",
            "messages": base_messages(),
            "temperature": 0.4,
        });
        if let (Some(obj), Some(given)) = (p.as_object_mut(), extra.as_object()) {
            for (k, v) in given {
                obj.insert(k.clone(), v.clone());
            }
        }
        p
    }

    fn openai_provider() -> Value {
        json!({
            "id": "openai",
            "api_key": "enc:KEY",
            "base_url": "https://api.openai.com/v1",
            "format": "openai",
            "models": ["gpt-4o"],
        })
    }

    fn dir_with(provider: Value) -> MockDirectory {
        let name = provider.get("id").and_then(|v| v.as_str()).unwrap_or("openai").to_string();
        MockDirectory::empty().with_provider(&name, provider)
    }

    fn skills() -> MockSkills {
        MockSkills::ok("RENDERED")
    }

    fn sse(payload: &str) -> String {
        format!("data: {}", payload)
    }

    // -- _normalize_base_url (ai.py 527-549) --------------------------------

    #[test]
    fn normalize_base_url_matches_python() {
        let cases: &[(&str, &str, &str)] = &[
            ("https://api.openai.com/v1", "chat/completions", "https://api.openai.com/v1/chat/completions"),
            ("https://api.openai.com/v1///", "chat/completions", "https://api.openai.com/v1/chat/completions"),
            ("https://api.openai.com/v1/chat/completions", "chat/completions", "https://api.openai.com/v1/chat/completions"),
            ("https://RELAY.example/v1/Chat/Completions", "chat/completions", "https://RELAY.example/v1/chat/completions"),
            // only the FIRST matching suffix is stripped (the loop breaks)
            ("https://x/v1/chat/completions/chat/completions", "chat/completions", "https://x/v1/chat/completions/chat/completions"),
            ("https://x/v1/models", "completions", "https://x/completions"),
            ("", "chat/completions", ""),
            ("   https://x/v1   ", "chat/completions", "https://x/v1/chat/completions"),
            ("https://x/", "responses", "https://x/responses"),
            ("https://x/v1", "", "https://x/v1"),
            ("https://x/v1", "/v1/messages", "https://x/v1/v1/messages"),
        ];
        for (base, endpoint, want) in cases {
            assert_eq!(&normalize_base_url(base, endpoint), want, "normalize_base_url({:?}, {:?})", base, endpoint);
        }
    }

    #[test]
    fn normalize_base_url_empty_endpoint_returns_base() {
        // `_normalize_base_url(base_url, endpoint="")` short-circuits.
        assert_eq!(normalize_base_url("https://x/v1/chat/completions", ""), "https://x/v1");
    }

    // -- _endpoint_url (ai.py 552-556) --------------------------------------

    #[test]
    fn endpoint_url_full_url_mode_returns_the_base_verbatim() {
        assert_eq!(endpoint_url("https://relay.example/api/", "chat/completions", "full_url"), "https://relay.example/api");
        assert_eq!(endpoint_url("  https://relay.example/api  ", "chat/completions", "FULL_URL"), "https://relay.example/api");
        assert_eq!(
            endpoint_url("https://api.openai.com/v1", "chat/completions", ""),
            "https://api.openai.com/v1/chat/completions"
        );
        assert_eq!(endpoint_url("https://x/v1", "v1/messages", "other"), "https://x/v1/v1/messages");
    }

    // -- _request_headers (ai.py 559-571) -----------------------------------

    #[test]
    fn request_headers_filters_unsafe_custom_entries() {
        let base = vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("Authorization".to_string(), "Bearer K".to_string()),
        ];
        let custom = json!({
            "Authorization": "Bearer EVIL",
            "x-api-key": "EVIL",
            "Cookie": "EVIL",
            "X-Trim": "  spaced  ",
            "empty": "",
            "num": 5,
            "unicode": "中",
            "\t tab": "drop",
            "ctrl": "a\u{1}b",
            "big": "y"
        });
        let mut custom = custom.as_object().unwrap().clone();
        custom.insert("k".repeat(129), json!("v"));
        custom.insert("big".to_string(), json!("y".repeat(2049)));
        let custom = Value::Object(custom);
        let out = request_headers(&base, Some(&custom));
        let map: std::collections::BTreeMap<String, String> = out.into_iter().collect();
        let mut want: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
        want.insert("Content-Type".into(), "application/json".into());
        want.insert("Authorization".into(), "Bearer K".into()); // evil override dropped
        want.insert("X-Trim".into(), "  spaced  ".into()); // key stripped but case preserved; value untouched
        want.insert("empty".into(), "".into());
        want.insert("num".into(), "5".into());
        want.insert("tab".into(), "drop".into()); // "\t tab".strip() == "tab"
        want.insert("unicode".into(), "中".into());
        assert_eq!(map, want);
    }

    #[test]
    fn request_headers_ignores_non_mapping_and_keeps_auth_when_nothing_valid() {
        let base = vec![("Content-Type".to_string(), "application/json".to_string())];
        assert_eq!(request_headers(&base, Some(&json!("nope"))), base);
        assert_eq!(request_headers(&base, None), base);
    }

    #[test]
    fn default_headers_are_merged_last_by_http_layer() {
        let h = default_http_headers();
        assert_eq!(h[0].0, "User-Agent");
        assert!(h[0].1.ends_with("ReadMD-AI/2.3"));
        assert_eq!(h[1].1, "application/json, text/plain, */*");
    }

    // -- message shaping ----------------------------------------------------

    #[test]
    fn openai_messages_moves_system_to_front_and_keeps_other_roles() {
        let turns = openai_messages(&base_messages());
        let json_turns: Vec<Value> = turns.iter().map(|t| t.to_json()).collect();
        assert_eq!(json_turns, base_messages());
        // two system messages come out reversed (each is `insert(0, …)`)
        let turns = openai_messages(&[
            json!({"role": "system", "content": "s1"}),
            json!({"role": "user", "content": "u1"}),
            json!({"role": "system", "content": "s2"}),
        ]);
        assert_eq!(
            turns.iter().map(|t| t.to_json()).collect::<Vec<_>>(),
            vec![json!({"role": "system", "content": "s2"}), json!({"role": "system", "content": "s1"}), json!({"role": "user", "content": "u1"})]
        );
    }

    #[test]
    fn openai_messages_defaults_and_null_content() {
        let turns = openai_messages(&[json!({}), json!({"role": "user", "content": null})]);
        assert_eq!(turns[0].role, json!("user"));
        assert_eq!(turns[0].content, json!(""));
        assert_eq!(turns[1].content, Value::Null);
    }

    #[test]
    fn anthropic_messages_merges_same_role_and_joins_system() {
        let (system, msgs) = anthropic_messages(&base_messages()).unwrap();
        assert_eq!(system, "sys A");
        assert_eq!(
            msgs.iter().map(|t| t.to_json()).collect::<Vec<_>>(),
            vec![
                json!({"role": "user", "content": "hello 你好😀\n\nsecond user"}),
                json!({"role": "assistant", "content": "prior answer"}),
            ]
        );
        // multiple system blocks join with "\n\n"; no turns at all → "..."
        let (system, msgs) = anthropic_messages(&[json!({"role": "system", "content": "a"}), json!({"role": "system", "content": "b"})]).unwrap();
        assert_eq!(system, "a\n\nb");
        assert_eq!(msgs[0].to_json(), json!({"role": "user", "content": "..."}));
        // any non-assistant role becomes "user"
        let (_, msgs) = anthropic_messages(&[json!({"role": "tool", "content": "x"}), json!({"role": "user", "content": "y"})]).unwrap();
        assert_eq!(msgs[0].to_json(), json!({"role": "user", "content": "x\n\ny"}));
    }

    #[test]
    fn anthropic_messages_surfaces_cpython_concat_errors() {
        let err = anthropic_messages(&[json!({"role": "user", "content": null}), json!({"role": "user", "content": 5})]).unwrap_err();
        assert_eq!(err.message, "TypeError: can only concatenate str (not \"int\") to str");
    }

    // -- usage extraction ---------------------------------------------------

    #[test]
    fn openai_usage_ignores_anthropic_names_and_stringifies_ints() {
        let cases: &[(Value, Option<Usage>)] = &[
            (json!({"usage": {"prompt_tokens": 9, "completion_tokens": 5, "total_tokens": 14}}), Some(usage(&[("prompt_tokens", 9), ("completion_tokens", 5), ("total_tokens", 14)]))),
            (json!({"usage": {"input_tokens": 7, "output_tokens": 4, "total_tokens": 11}}), Some(usage(&[("total_tokens", 11)]))),
            (json!({"usage": {"total_tokens": "9"}}), Some(usage(&[("total_tokens", 9)]))),
            (json!({"usage": {"completion_tokens": 3}}), Some(usage(&[("completion_tokens", 3)]))),
            (json!({"usage": {}}), None),
            (json!({}), None),
            (json!({"usage": null}), None),
        ];
        for (d, want) in cases {
            assert_eq!(&openai_usage(d).unwrap(), want, "openai_usage({})", d);
        }
        assert_eq!(openai_usage(&json!({"usage": 5})).unwrap_err().message, "'int' object has no attribute 'get'");
    }

    #[test]
    fn anthropic_usage_sums_only_when_both_present() {
        assert_eq!(anthropic_usage(&json!({"input_tokens": 9, "output_tokens": 5})).unwrap(), Some(usage(&[("prompt_tokens", 9), ("completion_tokens", 5), ("total_tokens", 14)])));
        assert_eq!(anthropic_usage(&json!({"output_tokens": 5})).unwrap(), Some(usage(&[("completion_tokens", 5)])));
        assert_eq!(anthropic_usage(&json!({})).unwrap(), None);
        assert_eq!(anthropic_usage(&json!({"input_tokens": 0, "output_tokens": 0})).unwrap(), Some(usage(&[("prompt_tokens", 0), ("completion_tokens", 0), ("total_tokens", 0)])));
        assert_eq!(anthropic_usage(&json!({"input_tokens": null, "output_tokens": 3})).unwrap(), Some(usage(&[("completion_tokens", 3)])));
    }

    #[test]
    fn py_int_follows_python_semantics() {
        assert_eq!(py_int(&json!(7)), Ok(7));
        assert_eq!(py_int(&json!(3.9)), Ok(3));
        assert_eq!(py_int(&json!(-3.9)), Ok(-3));
        assert_eq!(py_int(&json!("9")), Ok(9));
        assert_eq!(py_int(&json!(" -12 ")), Ok(-12));
        assert_eq!(py_int(&json!("1_2")), Ok(12));
        assert_eq!(py_int(&json!(true)), Ok(1));
        assert_eq!(py_int(&json!("abc")).unwrap_err().message, "invalid literal for int() with base 10: 'abc'");
        assert_eq!(py_int(&json!(null)).unwrap_err().message, "TypeError: int() argument must be a string, a bytes-like object or a real number, not 'NoneType'");
    }

    // -- json / str helpers -------------------------------------------------

    #[test]
    fn dumps_uses_python_separators_and_surrogates() {
        // `preserve_order` is on, so a hand-built `Value` keeps the order of its
        // literal exactly like CPython's `json.dumps` keeps a dict's insertion order.
        assert_eq!(py_dumps(&json!({"b": 1, "a": "中😀"})), "{\"b\": 1, \"a\": \"\\u4e2d\\ud83d\\ude00\"}");
        assert_eq!(py_dumps_unicode(&json!({"type": "error", "error": {"message": "nope"}})), "{\"type\": \"error\", \"error\": {\"message\": \"nope\"}}");
        assert_eq!(py_str(&json!("x")), "x");
        assert_eq!(py_str(&json!(null)), "None");
        assert_eq!(py_str(&json!(true)), "True");
        assert_eq!(py_str(&json!(0.4)), "0.4");
        assert_eq!(py_str(&json!(5)), "5");
        assert_eq!(py_str(&json!(["a", 1])), "['a', 1]");
        assert_eq!(py_repr(&json!({"a": null})), "{'a': None}");
    }

    #[test]
    fn ordered_dumper_preserves_body_key_order() {
        let body = dumps_ordered(&[("model".to_string(), "\"m\"".to_string()), ("stream".to_string(), "false".to_string())]);
        assert_eq!(body, "{\"model\": \"m\", \"stream\": false}");
    }

    #[test]
    fn truthiness_matches_python() {
        for falsy in [json!(null), json!(false), json!(0), json!(0.0), json!(""), json!([]), json!({})] {
            assert!(!py_truthy(&falsy), "{} must be falsy", falsy);
        }
        for truthy in [json!(true), json!(1), json!("0"), json!([0]), json!({"a": 1})] {
            assert!(py_truthy(&truthy), "{} must be truthy", truthy);
        }
    }

    // -- template rendering (skills.py 160-172) -----------------------------

    const PROBE_TEMPLATE: &str = "Doc:{{document}}|Sel: {{ selection }} |Doc2:{{document}}|Req:{{request}}|Lang:{{language}}";

    fn vars(pairs: &[(&str, Value)]) -> VarMap {
        VarMap { entries: pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect() }
    }

    #[test]
    fn template_variable_names_are_sorted_and_deduped() {
        assert_eq!(template_variable_names(PROBE_TEMPLATE), vec!["document", "language", "request", "selection"]);
        assert_eq!(template_variable_names("{{a}}{{ a }}{{b}}{{a}}"), vec!["a", "b"]);
        assert_eq!(template_variable_names("{{ nope! }}"), Vec::<String>::new());
    }

    #[test]
    fn render_skill_template_matches_python() {
        let v = vars(&[("document", json!("D\n")), ("request", json!("R")), ("language", json!("en"))]);
        assert_eq!(render_skill_template(PROBE_TEMPLATE, None, &v).unwrap(), "Doc:D\n|Sel:  |Doc2:D\n|Req:R|Lang:en");
        // absent variable → str("") == ""
        let v = vars(&[("document", json!("D\n")), ("request", json!("R"))]);
        assert_eq!(render_skill_template(PROBE_TEMPLATE, None, &v).unwrap(), "Doc:D\n|Sel:  |Doc2:D\n|Req:R|Lang:");
        // replacement text is inserted literally (lambda m: value)
        let v = vars(&[("document", json!("a\\1b\n$c")), ("request", json!("R")), ("language", json!("en"))]);
        assert_eq!(render_skill_template(PROBE_TEMPLATE, None, &v).unwrap(), "Doc:a\\1b\n$c|Sel:  |Doc2:a\\1b\n$c|Req:R|Lang:en");
        // final .strip()
        let v = vars(&[("selection", json!("")), ("request", json!("R")), ("language", json!("en"))]);
        assert_eq!(render_skill_template("Body {{selection}} end", None, &v).unwrap(), "Body  end");
        assert_eq!(render_skill_template("  padded {{document}}  ", None, &vars(&[("document", json!("x"))])).unwrap(), "padded x");
        // str(None) == "None"
        assert_eq!(render_skill_template("V:{{document}}", None, &vars(&[("document", Value::Null)])).unwrap(), "V:None");
    }

    #[test]
    fn required_variables_only_bite_when_referenced() {
        // skills.py:165 — `if name in required and name in skill.variables and not str(...)`
        let required = json!(["document", "context"]);
        let v = vars(&[("request", json!("R"))]);
        let err = render_skill_template("{{document}}|{{request}}", Some(&required), &v).unwrap_err();
        assert_eq!(err.kind, SkillErrorKind::Skill);
        assert_eq!(err.message, "missing required Skill variables: document");
        // `context` is required but never referenced by the template → not missing
        assert_eq!(render_skill_template("Only {{request}} here", Some(&required), &v).unwrap(), "Only R here");
        // default required list is ["document"]
        let err = render_skill_template("{{document}}", None, &v).unwrap_err();
        assert_eq!(err.message, "missing required Skill variables: document");
        assert_eq!(render_skill_template("{{document}}", None, &vars(&[("document", json!("d"))])).unwrap(), "d");
        // a falsy metadata value (`[]`) falls back to the same default
        assert_eq!(render_skill_template("{{document}}", Some(&json!([])), &vars(&[("document", json!("d"))])).unwrap(), "d");
        // whitespace-only input counts as missing (str(...).strip())
        let err = render_skill_template("{{document}}", None, &vars(&[("document", json!("   "))])).unwrap_err();
        assert_eq!(err.message, "missing required Skill variables: document");
    }

    // -- _skill_messages ----------------------------------------------------

    #[test]
    fn skill_messages_without_skill_id_passes_messages_through() {
        for skill_id in [Value::Null, json!(""), json!("   "), json!(0), json!([])] {
            let payload = json!({"skill_id": skill_id, "messages": base_messages()});
            let out = skill_messages(&payload, &skills()).unwrap();
            assert_eq!(out, base_messages(), "skill_id={}", skill_id);
        }
        let out = skill_messages(&json!({"skill_id": "demo"}), &skills()).unwrap();
        assert_eq!(out, vec![json!({"role": "system", "content": "RENDERED"})]);
    }

    #[test]
    fn skill_messages_appends_defaults_in_python_order() {
        let payload = json!({"skill_id": "demo", "messages": base_messages()});
        let svc = MockSkills::ok("RENDERED");
        let out = skill_messages(&payload, &svc).unwrap();
        let seen = svc.seen.lock().unwrap().clone().unwrap();
        assert_eq!(
            seen.keys(),
            vec!["document", "language", "request", "context", "selection", "output_format"]
        );
        assert_eq!(seen.get("document"), Some(&json!("second user"))); // last user message
        assert_eq!(seen.get("language"), Some(&json!("the document's language")));
        assert_eq!(seen.get("request"), Some(&json!("")));
        assert_eq!(seen.get("context"), Some(&json!("")));
        assert_eq!(seen.get("selection"), Some(&json!("second user")));
        assert_eq!(seen.get("output_format"), Some(&json!("Markdown")));
        assert_eq!(out[0], json!({"role": "system", "content": "RENDERED"}));
        // the original system message is replaced, the rest kept verbatim
        assert_eq!(&out[1..], &base_messages()[1..]);
    }

    #[test]
    fn skill_messages_keeps_explicit_variables_first() {
        let payload = json!({"skill_id": "demo", "messages": [], "skill_variables": {"language": "en", "document": "mine"}});
        let svc = MockSkills::ok("R");
        skill_messages(&payload, &svc).unwrap();
        let seen = svc.seen.lock().unwrap().clone().unwrap();
        // `ai.py:456-464` copies the caller's `skill_variables` dict first, so the
        // explicit names keep their wire order and only the setdefault tail is appended.
        assert_eq!(seen.keys(), vec!["language", "document", "request", "context", "selection", "output_format"]);
        assert_eq!(seen.get("document"), Some(&json!("mine")));
        assert_eq!(seen.get("selection"), Some(&json!("mine")));
    }

    #[test]
    fn skill_messages_falsy_document_falls_back_to_last_user_message() {
        let payload = json!({"skill_id": "demo", "messages": [{"role": "user", "content": null}], "skill_variables": {"document": ""}});
        let svc = MockSkills::ok("R");
        skill_messages(&payload, &svc).unwrap();
        let seen = svc.seen.lock().unwrap().clone().unwrap();
        assert_eq!(seen.get("document"), Some(&Value::Null));
        assert_eq!(seen.get("selection"), Some(&Value::Null));
    }

    #[test]
    fn skill_messages_maps_skill_errors_onto_the_two_except_arms() {
        let payload = json!({"skill_id": "demo", "messages": []});
        let err = skill_messages(&payload, &MockSkills::fail(SkillErrorKind::Skill, "Skill not found: demo")).unwrap_err();
        assert_eq!(err.message, "Skill not found: demo");
        let err = skill_messages(&payload, &MockSkills::fail(SkillErrorKind::Other, "boom")).unwrap_err();
        assert_eq!(err.message, "Skill 渲染失败：请检查 Skill 模板或文档内容后重试");
        // a non-mapping skill_variables is the generic arm too
        let payload = json!({"skill_id": "demo", "messages": [], "skill_variables": ["a"]});
        let err = skill_messages(&payload, &skills()).unwrap_err();
        assert_eq!(err.message, "Skill 渲染失败：请检查 Skill 模板或文档内容后重试");
    }

    // -- wire bodies / urls / headers --------------------------------------

    #[test]
    fn chat_route_wire_request_is_byte_identical_to_python() {
        let http = MockTransport::new("{\"choices\": [{\"message\": {\"content\": \"hi\"}}]}", vec![]);
        let payload = openai_payload(json!({"stream": false}));
        collect(&payload, &dir_with(openai_provider()), &skills(), &http).unwrap();
        let call = http.last();
        assert_eq!(call.url, "https://api.openai.com/v1/chat/completions");
        assert_eq!(
            call.body,
            "{\"model\": \"gpt-4o\", \"messages\": [{\"role\": \"system\", \"content\": \"sys A\"}, {\"role\": \"user\", \"content\": \"hello \\u4f60\\u597d\\ud83d\\ude00\"}, {\"role\": \"user\", \"content\": \"second user\"}, {\"role\": \"assistant\", \"content\": \"prior answer\"}], \"stream\": false, \"temperature\": 0.4}"
        );
        assert_eq!(
            call.headers,
            vec![
                ("User-Agent".to_string(), default_http_headers()[0].1.clone()),
                ("Accept".to_string(), "application/json, text/plain, */*".to_string()),
                ("Content-Type".to_string(), "application/json".to_string()),
                ("Authorization".to_string(), "Bearer KEY".to_string()),
            ]
        );
    }

    #[test]
    fn chat_route_stream_body_adds_stream_options() {
        let http = MockTransport::new("", vec![]);
        collect(&openai_payload(json!({"stream": true})), &dir_with(openai_provider()), &skills(), &http).unwrap();
        let call = http.last();
        assert_eq!(call.url, "https://api.openai.com/v1/chat/completions");
        assert!(call.body.ends_with(", \"stream\": true, \"temperature\": 0.4, \"stream_options\": {\"include_usage\": true}}"), "{}", call.body);
    }

    #[test]
    fn completion_route_wire_matches_python() {
        let mut provider = openai_provider();
        provider["mode"] = json!("completion");
        provider["models"] = json!(["m"]);
        let http = MockTransport::new("{\"choices\": [{\"text\": \"flat answer\"}], \"usage\": {\"total_tokens\": 8}}", vec![]);
        let payload = openai_payload(json!({"stream": false}));
        let events = collect(&payload, &dir_with(provider), &skills(), &http).unwrap();
        assert_eq!(http.last().url, "https://api.openai.com/v1/completions");
        assert_eq!(
            http.last().body,
            "{\"model\": \"gpt-4o\", \"prompt\": \"sys A\\n\\nhello \\u4f60\\u597d\\ud83d\\ude00\\n\\nsecond user\\n\\nprior answer\", \"max_tokens\": 4096, \"temperature\": 0.4, \"stream\": false}"
        );
        assert_eq!(deltas(&events), vec![json!("flat answer")]);
        assert_eq!(usages(&events), vec![usage(&[("total_tokens", 8)])]);
    }

    #[test]
    fn responses_route_wire_matches_python() {
        let mut provider = openai_provider();
        provider["mode"] = json!("responses");
        let http = MockTransport::new("{\"output_text\": \"resp text\", \"usage\": {\"total_tokens\": 11}}", vec![]);
        let events = collect(&openai_payload(json!({"stream": false})), &dir_with(provider), &skills(), &http).unwrap();
        assert_eq!(http.last().url, "https://api.openai.com/v1/responses");
        assert_eq!(
            http.last().body,
            "{\"model\": \"gpt-4o\", \"input\": [{\"role\": \"system\", \"content\": \"sys A\"}, {\"role\": \"user\", \"content\": \"hello \\u4f60\\u597d\\ud83d\\ude00\"}, {\"role\": \"user\", \"content\": \"second user\"}, {\"role\": \"assistant\", \"content\": \"prior answer\"}], \"stream\": false, \"temperature\": 0.4}"
        );
        assert_eq!(deltas(&events), vec![json!("resp text")]);
        assert_eq!(usages(&events), vec![usage(&[("total_tokens", 11)])]);
    }

    #[test]
    fn anthropic_route_wire_matches_python() {
        let provider = json!({"id": "claude", "api_key": "enc:KEY", "base_url": "https://api.anthropic.com", "format": "anthropic", "models": ["claude"]});
        let http = MockTransport::new("{\"content\": [{\"type\": \"text\", \"text\": \"Hi there\"}], \"usage\": {\"input_tokens\": 9, \"output_tokens\": 5}}", vec![]);
        let payload = json!({"provider": "claude", "model": "claude", "messages": base_messages(), "temperature": 0.4, "stream": false});
        let events = collect(&payload, &dir_with(provider), &skills(), &http).unwrap();
        assert_eq!(http.last().url, "https://api.anthropic.com/v1/messages");
        assert_eq!(
            http.last().body,
            "{\"model\": \"claude\", \"max_tokens\": 4096, \"messages\": [{\"role\": \"user\", \"content\": \"hello \\u4f60\\u597d\\ud83d\\ude00\\n\\nsecond user\"}, {\"role\": \"assistant\", \"content\": \"prior answer\"}], \"temperature\": 0.4, \"stream\": false, \"system\": \"sys A\"}"
        );
        let call = http.last();
        let names: Vec<&str> = call.headers.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(names, vec!["User-Agent", "Accept", "Content-Type", "x-api-key", "anthropic-version"]);
        assert_eq!(deltas(&events), vec![json!("Hi there")]);
        assert_eq!(usages(&events), vec![usage(&[("prompt_tokens", 9), ("completion_tokens", 5), ("total_tokens", 14)])]);
    }

    #[test]
    fn anthropic_without_key_omits_x_api_key_but_keeps_version() {
        let provider = json!({"id": "local", "category": "local", "base_url": "http://localhost:1234/v1", "format": "anthropic"});
        let http = MockTransport::new("{\"content\": []}", vec![]);
        let payload = json!({"provider": "local", "model": "x", "messages": [{"role": "user", "content": "hi"}], "stream": false});
        let events = collect(&payload, &dir_with(provider), &skills(), &http).unwrap();
        assert_eq!(http.last().url, "http://localhost:1234/v1/v1/messages");
        let call = http.last();
        let names: Vec<&str> = call.headers.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(names, vec!["User-Agent", "Accept", "Content-Type", "anthropic-version"]);
        assert!(events.is_empty());
    }

    // -- non-streaming parse paths -----------------------------------------

    #[test]
    fn parse_failures_use_the_300_char_prefix() {
        let http = MockTransport::new("{\"data\": []}", vec![]);
        let err = collect(&openai_payload(json!({"stream": false})), &dir_with(openai_provider()), &skills(), &http).unwrap_err();
        assert_eq!(err.message, "响应解析失败：{\"data\": []}");
        let http = MockTransport::new("not json", vec![]);
        let err = collect(&openai_payload(json!({"stream": false})), &dir_with(openai_provider()), &skills(), &http).unwrap_err();
        assert_eq!(err.message, "响应解析失败：not json");
    }

    #[test]
    fn null_content_yields_no_events() {
        let http = MockTransport::new("{\"choices\": [{\"message\": {\"content\": null}}]}", vec![]);
        let events = collect(&openai_payload(json!({"stream": false})), &dir_with(openai_provider()), &skills(), &http).unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn anthropic_non_stream_content_must_be_a_list_of_dicts() {
        let http = MockTransport::new("{\"content\": \"not a list\"}", vec![]);
        let provider = json!({"id": "claude", "api_key": "enc:KEY", "base_url": "https://api.anthropic.com", "format": "anthropic"});
        let payload = json!({"provider": "claude", "model": "c", "messages": [{"role": "user", "content": "hi"}], "stream": false});
        let err = collect(&payload, &dir_with(provider), &skills(), &http).unwrap_err();
        assert_eq!(err.message, "响应解析失败：{\"content\": \"not a list\"}");
    }

    #[test]
    fn http_status_and_network_errors_match_python_text() {
        let http = MockTransport::failing(HttpError::Status { code: 429, body: "rate limited".to_string() });
        let err = collect(&openai_payload(json!({"stream": false})), &dir_with(openai_provider()), &skills(), &http).unwrap_err();
        assert_eq!(err.message, "HTTP 429：rate limited");
        let http = MockTransport::failing(HttpError::Network("timed out".to_string()));
        let err = collect(&openai_payload(json!({"stream": true})), &dir_with(openai_provider()), &skills(), &http).unwrap_err();
        assert_eq!(err.message, "网络错误：timed out");
    }

    // -- streaming frame handling ------------------------------------------

    #[test]
    fn chat_stream_frames_follow_the_python_quirks() {
        let lines = vec![
            String::new(),
            "event: foo".to_string(),
            ": comment".to_string(),
            sse("{\"choices\": [{\"delta\": {\"content\": \"He\"}}]}"),
            format!("data:{}", sse("{\"choices\": [{\"delta\": {\"content\": \"Nospace\"}}]}").replacen("data: ", "", 1).as_str()),
            "not-a-frame".to_string(),
            sse("{broken json"),
            sse("{\"choices\": [{\"delta\": {}}]}"),
            sse("{\"choices\": [{\"delta\": {\"content\": \"llo\"}}, {\"delta\": {\"content\": \"IGNORED\"}}]}"),
            sse("{\"usage\": {\"prompt_tokens\": 3, \"completion_tokens\": 2, \"total_tokens\": 5}}"),
            sse("[DONE]"),
            sse("{\"choices\": [{\"delta\": {\"content\": \"AFTER DONE\"}}]}"),
        ];
        let http = MockTransport::new("", lines);
        let events = collect(&openai_payload(json!({"stream": true})), &dir_with(openai_provider()), &skills(), &http).unwrap();
        assert_eq!(deltas(&events), vec![json!("He"), json!("Nospace"), json!("llo")]);
        assert_eq!(usages(&events), vec![usage(&[("prompt_tokens", 3), ("completion_tokens", 2), ("total_tokens", 5)])]);
        assert_eq!(events.len(), 4);
    }

    #[test]
    fn chat_stream_keeps_reading_after_finish_reason() {
        let lines = vec![
            sse("{\"choices\": [{\"delta\": {\"content\": \"a\"}, \"finish_reason\": \"stop\"}]}"),
            sse("{\"usage\": {\"total_tokens\": 7}}"),
            sse("[DONE]"),
        ];
        let http = MockTransport::new("", lines);
        let events = collect(&openai_payload(json!({"stream": true})), &dir_with(openai_provider()), &skills(), &http).unwrap();
        assert_eq!(usages(&events), vec![usage(&[("total_tokens", 7)])]);
    }

    #[test]
    fn completion_stream_reads_delta_text_only() {
        let lines = vec![
            sse("{\"choices\": [{\"delta\": {\"content\": \"WRONG\", \"text\": \"ct1\"}}]}"),
            sse("{\"choices\": [{\"text\": \"WRONG2\"}]}"),
            sse("{\"choices\": [{\"delta\": {\"text\": \"\"}}]}"),
            sse("{\"usage\": {\"total_tokens\": 4}}"),
            sse("[DONE]"),
        ];
        let mut provider = openai_provider();
        provider["mode"] = json!("completion");
        let http = MockTransport::new("", lines);
        let events = collect(&openai_payload(json!({"stream": true})), &dir_with(provider), &skills(), &http).unwrap();
        assert_eq!(deltas(&events), vec![json!("ct1")]);
        assert_eq!(usages(&events), vec![usage(&[("total_tokens", 4)])]);
    }

    #[test]
    fn responses_stream_reads_delta_text_and_completed_usage() {
        let lines = vec![
            sse("{\"type\": \"response.output_text.delta\", \"delta\": {\"text\": \"Hel\"}}"),
            sse("{\"type\": \"response.output_text.delta\", \"delta\": {\"text\": \"\"}}"),
            sse("{\"type\": \"response.created\"}"),
            sse("{\"type\": \"response.completed\", \"response\": {\"usage\": {\"input_tokens\": 4, \"total_tokens\": 11}}}"),
            sse("[DONE]"),
        ];
        let mut provider = openai_provider();
        provider["mode"] = json!("responses");
        let http = MockTransport::new("", lines);
        let events = collect(&openai_payload(json!({"stream": true})), &dir_with(provider), &skills(), &http).unwrap();
        assert_eq!(deltas(&events), vec![json!("Hel")]);
        // `_openai_usage` ignores input_tokens
        assert_eq!(usages(&events), vec![usage(&[("total_tokens", 11)])]);
    }

    #[test]
    fn responses_error_frame_raises_provider_error() {
        let mut provider = openai_provider();
        provider["mode"] = json!("responses");
        let http = MockTransport::new("", vec![sse("{\"type\": \"error\", \"error\": {\"message\": \"nope\"}}"), sse("[DONE]")]);
        let err = collect(&openai_payload(json!({"stream": true})), &dir_with(provider), &skills(), &http).unwrap_err();
        // `preserve_order` keeps the frame's wire order through the parse/serialise
        // round trip, which is CPython's behaviour (`json.loads` then `json.dumps`).
        assert_eq!(err.message, "提供商错误：{\"type\": \"error\", \"error\": {\"message\": \"nope\"}}");
        assert!(err.message.contains(r#""error": {"message": "nope"}"#), "{}", err.message);
    }

    #[test]
    fn anthropic_stream_assembles_usage_and_stops_at_message_stop() {
        let provider = json!({"id": "claude", "api_key": "enc:KEY", "base_url": "https://api.anthropic.com", "format": "anthropic"});
        let lines = vec![
            sse("{\"type\": \"message_start\", \"message\": {\"usage\": {\"input_tokens\": 9}}}"),
            sse("{\"type\": \"content_block_delta\", \"delta\": {\"type\": \"thinking_delta\", \"thinking\": \"hidden\"}}"),
            sse("{\"type\": \"content_block_delta\", \"delta\": {\"type\": \"text_delta\", \"text\": \"Hi\"}}"),
            sse("{\"type\": \"content_block_delta\", \"delta\": {\"type\": \"text_delta\", \"text\": \"\"}}"),
            sse("{\"type\": \"message_delta\", \"usage\": {\"output_tokens\": 5}}"),
            sse("{\"type\": \"message_stop\"}"),
            sse("{\"type\": \"message_delta\", \"usage\": {\"output_tokens\": 99}}"),
        ];
        let payload = json!({"provider": "claude", "model": "c", "messages": [{"role": "user", "content": "hi"}], "stream": true});
        let http = MockTransport::new("", lines);
        let events = collect(&payload, &dir_with(provider), &skills(), &http).unwrap();
        assert_eq!(deltas(&events), vec![json!("Hi")]);
        assert_eq!(usages(&events), vec![usage(&[("prompt_tokens", 9), ("completion_tokens", 5), ("total_tokens", 14)])]);
    }

    #[test]
    fn anthropic_stream_without_message_start_reports_only_output_tokens() {
        let provider = json!({"id": "claude", "api_key": "enc:KEY", "base_url": "https://api.anthropic.com", "format": "anthropic"});
        let lines = vec![sse("{\"type\": \"message_delta\", \"usage\": {\"output_tokens\": 3}}"), sse("{\"type\": \"message_stop\"}")];
        let payload = json!({"provider": "claude", "model": "c", "messages": [{"role": "user", "content": "hi"}], "stream": true});
        let http = MockTransport::new("", lines);
        let events = collect(&payload, &dir_with(provider), &skills(), &http).unwrap();
        assert_eq!(usages(&events), vec![usage(&[("completion_tokens", 3)])]);
    }

    #[test]
    fn mid_stream_network_error_becomes_interrupt_text() {
        struct Broken;
        impl LineStream for Broken {
            fn next_line(&mut self) -> Result<Option<String>, String> {
                Err("connection aborted".to_string())
            }
        }
        struct BrokenTransport;
        impl Transport for BrokenTransport {
            fn post_json(&self, _: &str, _: &Headers, _: &str) -> Result<String, HttpError> {
                Err(HttpError::Network("unused".to_string()))
            }
            fn open_stream(&self, _: &str, _: &Headers, _: &str) -> Result<Box<dyn LineStream>, HttpError> {
                Ok(Box::new(Broken))
            }
        }
        let err = collect(&openai_payload(json!({"stream": true})), &dir_with(openai_provider()), &skills(), &BrokenTransport).unwrap_err();
        assert_eq!(err.message, "连接中断：connection aborted");
    }

    #[test]
    fn stream_rejects_frames_that_are_not_objects() {
        let http = MockTransport::new("", vec![sse("5")]);
        let err = collect(&openai_payload(json!({"stream": true})), &dir_with(openai_provider()), &skills(), &http).unwrap_err();
        assert_eq!(err.message, "'int' object has no attribute 'get'");
    }

    // -- chat() dispatch ----------------------------------------------------

    #[test]
    fn dispatch_rejects_unknown_provider_and_missing_key() {
        let dir = MockDirectory::empty();
        let http = MockTransport::new("{}", vec![]);
        let payload = json!({"provider": "nope", "model": "m", "messages": []});
        let err = collect(&payload, &dir, &skills(), &http).unwrap_err();
        assert_eq!(err.message, "未知提供商：nope");

        let dir = dir_with(json!({"id": "openai", "base_url": "https://api.openai.com/v1"}));
        let err = collect(&openai_payload(json!({})), &dir, &skills(), &http).unwrap_err();
        assert_eq!(err.message, "未配置 API Key（可填入界面，或设置环境变量 ）");

        let dir = dir_with(json!({"id": "openai", "base_url": "https://api.openai.com/v1", "env_key": "OPENAI_API_KEY"}));
        let err = collect(&openai_payload(json!({})), &dir, &skills(), &http).unwrap_err();
        assert_eq!(err.message, "未配置 API Key（可填入界面，或设置环境变量 OPENAI_API_KEY）");
    }

    #[test]
    fn local_providers_need_no_credential() {
        let dir = dir_with(json!({"id": "ollama", "category": "Local", "base_url": "http://localhost:11434/v1", "models": ["llama"]}));
        let http = MockTransport::new("{\"choices\": [{\"message\": {\"content\": \"ok\"}}]}", vec![]);
        let payload = json!({"provider": "ollama", "messages": [{"role": "user", "content": "hi"}], "stream": false});
        let events = collect(&payload, &dir, &skills(), &http).unwrap();
        assert_eq!(deltas(&events), vec![json!("ok")]);
        // no Authorization header is sent for a local provider
        assert!(!http.last().headers.iter().any(|(k, _)| k == "Authorization"));
        assert_eq!(http.last().body, "{\"model\": \"llama\", \"messages\": [{\"role\": \"user\", \"content\": \"hi\"}], \"stream\": false, \"temperature\": 0.4}");

        assert!(is_local_provider(&json!({"category": "local"})));
        assert!(is_local_provider(&json!({"base_url": "http://127.0.0.1:8080"})));
        assert!(is_local_provider(&json!({"base_url": "http://[::1]:8080"})));
        assert!(!is_local_provider(&json!({"base_url": "https://api.openai.com/v1"})));
        assert!(!is_local_provider(&json!("not a dict")));
    }

    #[test]
    fn credential_mismatch_is_eager() {
        let mut dir = dir_with(openai_provider());
        dir.providers[0].1["credential_id"] = json!("cred-other");
        let http = MockTransport::new("{}", vec![]);
        let payload = openai_payload(json!({"credential_id": "cred-1"}));
        let err = collect(&payload, &dir, &skills(), &http).unwrap_err();
        assert_eq!(err.message, "凭据与提供商不匹配");

        // provider without a credential_id adopts the credential's provider,
        // but base_url was already resolved from the name-resolved provider
        // (ai.py 490-500), so only the key/models come from the credential.
        let mut dir = MockDirectory::empty().with_provider("openai", json!({"id": "openai", "base_url": "https://api.openai.com/v1"}));
        dir.current = json!({"provider_id": "openai"});
        dir.by_credential.push(("cred-1".to_string(), json!({"id": "ali", "api_key": "enc:CK", "base_url": "https://ali.example/v1", "models": ["cm"]})));
        let http = MockTransport::new("{\"choices\": [{\"message\": {\"content\": \"y\"}}]}", vec![]);
        let payload = json!({"credential_id": "cred-1", "model": "cm", "messages": [{"role": "user", "content": "hi"}], "stream": false});
        let events = collect(&payload, &dir, &skills(), &http).unwrap();
        assert_eq!(http.last().url, "https://api.openai.com/v1/chat/completions");
        assert!(http.last().headers.contains(&("Authorization".to_string(), "Bearer CK".to_string())));
        assert_eq!(deltas(&events), vec![json!("y")]);

        // ... and with no provider at all the empty base_url survives verbatim.
        let bare = MockDirectory { by_credential: dir.by_credential.clone(), ..MockDirectory::empty() };
        let http = MockTransport::new("{\"choices\": [{\"message\": {\"content\": \"z\"}}]}", vec![]);
        let events = collect(&payload, &bare, &skills(), &http).unwrap();
        assert_eq!(http.last().url, "");
        assert_eq!(deltas(&events), vec![json!("z")]);
    }

    #[test]
    fn resolve_key_prefers_credential_then_enc_then_env() {
        let dir = MockDirectory {
            credentials: vec![("c1".to_string(), "from-credential".to_string())],
            env: vec![("MY_KEY".to_string(), "from-env".to_string())],
            ..MockDirectory::empty()
        };
        assert_eq!(resolve_key(&json!({"credential_id": "c1"}), &dir).unwrap(), "from-credential");
        assert_eq!(resolve_key(&json!({"api_key": "enc:blob"}), &dir).unwrap(), "blob");
        assert_eq!(resolve_key(&json!({"env_key": "MY_KEY"}), &dir).unwrap(), "from-env");
        assert_eq!(resolve_key(&json!({"env_key": "MISSING"}), &dir).unwrap(), "");
        assert_eq!(resolve_key(&json!({"api_key": null}), &dir).unwrap_err().message, "'NoneType' object has no attribute 'startswith'");
        assert_eq!(resolve_key(&json!("nope"), &dir).unwrap_err().message, "'str' object has no attribute 'get'");
    }

    #[test]
    fn mode_and_format_select_the_route() {
        let provider = json!({"id": "p", "api_key": "enc:K", "base_url": "https://x/v1", "format": "openai", "models": ["m"]});
        let cases: &[(&str, Value, RouteTarget)] = &[
            ("payload mode completion", json!({"mode": " COMPLETION "}), RouteTarget::Completion),
            ("payload mode responses", json!({"mode": "responses"}), RouteTarget::Responses),
            ("payload mode messages", json!({"mode": "messages"}), RouteTarget::Anthropic),
            ("payload mode anthropic", json!({"mode": "anthropic"}), RouteTarget::Anthropic),
            ("provider mode via inheritance", json!({}), RouteTarget::Chat),
            ("format anthropic implies messages", json!({"format": "anthropic"}), RouteTarget::Anthropic),
            ("format openai without mode is chat", json!({"format": "openai"}), RouteTarget::Chat),
            ("unknown mode falls through to chat", json!({"mode": "weird"}), RouteTarget::Chat),
        ];
        for (label, extra, want) in cases {
            let mut payload = json!({"provider": "p", "model": "m", "messages": [], "stream": false});
            for (k, v) in extra.as_object().unwrap() {
                payload.as_object_mut().unwrap().insert(k.clone(), v.clone());
            }
            let route = resolve_chat(&payload, &dir_with(provider.clone()), &skills()).unwrap_or_else(|e| panic!("{}: {}", label, e.message));
            assert_eq!(&route.target, want, "{}", label);
        }
    }

    #[test]
    fn stream_and_temperature_defaults_follow_python() {
        let dir = dir_with(openai_provider());
        let base = json!({"provider": "openai", "model": "m", "messages": []});
        let cases: &[(Value, bool)] = &[
            (json!({}), false),
            (json!(null), false),
            (json!(0), false),
            (json!(false), false),
            (json!(""), false),
            (json!(1), true),
            (json!("no"), true),
        ];
        for (value, want) in cases {
            let payload = json!({"provider": "openai", "model": "m", "messages": [], "stream": value});
            let route = resolve_chat(&payload, &dir, &skills()).unwrap();
            assert_eq!(route.args.stream, *want, "stream={}", value);
        }
        let route = resolve_chat(&base, &dir, &skills()).unwrap();
        assert_eq!(route.args.temperature, json!(0.4));
        assert!(route.args.stream, "absent stream key defaults to True");
        let route = resolve_chat(&json!({"provider": "openai", "model": "m", "messages": [], "temperature": null}), &dir, &skills()).unwrap();
        assert_eq!(route.args.temperature, Value::Null);
        let route = resolve_chat(&json!({"provider": "openai", "model": "m", "messages": [], "temperature": 1}), &dir, &skills()).unwrap();
        assert_eq!(route.args.temperature, json!(1));
        assert!(route.args.stream);
    }

    #[test]
    fn model_precedence_payload_current_first_model() {
        let dir = dir_with(json!({"id": "openai", "api_key": "enc:K", "base_url": "https://x/v1", "models": ["first", "second"]}));
        let route = resolve_chat(&json!({"provider": "openai", "model": "picked", "messages": []}), &dir, &skills()).unwrap();
        assert_eq!(route.args.model, json!("picked"));
        // payload provider given → `current` stays {} so current.model is skipped
        let mut dir2 = dir.clone();
        dir2.current = json!({"model": "cfg-model"});
        let route = resolve_chat(&json!({"provider": "openai", "messages": []}), &dir2, &skills()).unwrap();
        assert_eq!(route.args.model, json!("first"));
        // no payload provider → current.model wins over the provider list
        let route = resolve_chat(&json!({"api_key": "K", "messages": []}), &dir2, &skills()).unwrap();
        assert_eq!(route.args.model, json!("cfg-model"));
        // empty models list → ""
        let dir3 = dir_with(json!({"id": "openai", "api_key": "enc:K", "base_url": "https://x/v1", "models": []}));
        let route = resolve_chat(&json!({"provider": "openai", "messages": []}), &dir3, &skills()).unwrap();
        assert_eq!(route.args.model, json!(""));
        // a string "models" is subscripted like a sequence in Python
        let dir4 = dir_with(json!({"id": "openai", "api_key": "enc:K", "base_url": "https://x/v1", "models": "abc"}));
        let route = resolve_chat(&json!({"provider": "openai", "messages": []}), &dir4, &skills()).unwrap();
        assert_eq!(route.args.model, json!("a"));
        // a numeric models is not subscriptable, exactly like CPython
        let dir5 = dir_with(json!({"id": "openai", "api_key": "enc:K", "base_url": "https://x/v1", "models": 5}));
        let err = resolve_chat(&json!({"provider": "openai", "messages": []}), &dir5, &skills()).unwrap_err();
        assert_eq!(err.message, "TypeError: 'int' object is not subscriptable");
    }

    #[test]
    fn provider_name_and_base_url_and_inherited_knobs() {
        let mut dir = MockDirectory::empty();
        dir.current = json!({"provider_id": " openai ", "provider": "fallback"});
        // `name` is only stripped on the config-derived path
        let err = resolve_chat(&json!({"messages": []}), &dir, &skills()).unwrap_err();
        assert_eq!(err.message, "未知提供商：openai");
        dir.current = json!({"provider": "openai"});
        dir.providers.push(("openai".to_string(), json!({
            "id": "openai", "api_key": "enc:K", "base_url": "https://relay.example/v1/",
            "endpoint_mode": "full_url", "headers": {"X-Trace": "1"}, "models": ["m"]
        })));
        let route = resolve_chat(&json!({"messages": [{"role": "user", "content": "hi"}]}), &dir, &skills()).unwrap();
        // payload-less base_url keeps the provider's, rstrip("/") only
        assert_eq!(route.args.base_url, "https://relay.example/v1");
        assert_eq!(route.args.endpoint_mode, "full_url");
        assert_eq!(route.args.custom_headers, json!({"X-Trace": "1"}));
        // a payload base_url overrides the provider's
        let route = resolve_chat(&json!({"base_url": "https://other.example/v1//", "messages": []}), &dir, &skills()).unwrap();
        assert_eq!(route.args.base_url, "https://other.example/v1");
        // payload headers win only when they are a mapping
        let route = resolve_chat(&json!({"headers": "nope", "messages": []}), &dir, &skills()).unwrap();
        assert_eq!(route.args.custom_headers, json!({"X-Trace": "1"}));
        let route = resolve_chat(&json!({"headers": {"X-A": "b"}, "messages": []}), &dir, &skills()).unwrap();
        assert_eq!(route.args.custom_headers, json!({"X-A": "b"}));
    }

    #[test]
    fn eager_errors_happen_before_the_http_call() {
        // no api key and no local provider → transport must never be touched
        let dir = dir_with(json!({"id": "openai", "base_url": "https://api.openai.com/v1"}));
        let http = MockTransport::new("{}", vec![]);
        let err = collect(&openai_payload(json!({})), &dir, &skills(), &http).unwrap_err();
        assert!(err.message.starts_with("未配置 API Key"));
        assert!(http.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn skill_id_reaches_the_request_as_a_system_message() {
        let http = MockTransport::new("{\"choices\": [{\"message\": {\"content\": \"ok\"}}]}", vec![]);
        let payload = json!({
            "provider": "openai", "model": "m", "api_key": "K",
            "skill_id": "demo", "stream": false,
            "messages": [{"role": "system", "content": "stale"}, {"role": "user", "content": "doc body"}]
        });
        let dir = dir_with(json!({"id": "openai", "base_url": "https://x/v1", "models": ["m"]}));
        let events = collect(&payload, &dir, &skills(), &http).unwrap();
        assert_eq!(deltas(&events), vec![json!("ok")]);
        assert_eq!(
            http.last().body,
            "{\"model\": \"m\", \"messages\": [{\"role\": \"system\", \"content\": \"RENDERED\"}, {\"role\": \"user\", \"content\": \"doc body\"}], \"stream\": false, \"temperature\": 0.4}"
        );
    }
}
