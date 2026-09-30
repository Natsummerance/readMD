//! P10 parity layer: `/api/ocr`, `/api/transcribe`, `/api/url`,
//! `/api/web/extract`, `/api/web/cancel`.
//!
//! Every response body is assembled here with [`Response::json_status`] instead
//! of [`crate::error::ApiError`], because `readmd.py` mixes three different
//! envelopes on these routes:
//!
//! * `{'error': ...}` only — `_api_ocr` 404, `_api_url` 400;
//! * `{'ok': False, 'error_code': ...}` — `_send_api_error`, and the
//!   `_module_ready` gate (`module_loading` 409 / `module_unavailable` 503);
//! * `{'ok': False, 'code': ..., 'error': ..., 'detail': ...}` — `WebError`
//!   dicts and the `/api/web/extract` gate, which keeps the literal
//!   `module_loading` code even on its 503 branch.
//!
//! The kernel's own `error_response()` uses the `error` key, so returning an
//! `ApiError` from these routes would silently change the contract.
//!
//! Network egress goes through `ureq` (already a kernel dependency). The
//! previous implementation shelled out to the system `curl`, which is what made
//! `/api/url` answer 500 where `readmd.py` answers 400/409/200.

use crate::headless_renderer as hr;
use crate::server::{Request, Response};
use crate::{ocr, readmd_fix, transcribe, App};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

// ------------------------------------------------------------- web.py consts

pub const MAX_HTML_BYTES: usize = 50 * 1024 * 1024;
pub const MAX_IMAGE_BYTES: usize = 15 * 1024 * 1024;
pub const MAX_IMAGE_TOTAL: usize = 100 * 1024 * 1024;
pub const MAX_IMAGES: usize = 100;
pub const MAX_REDIRECTS: u32 = 10;
pub const MIN_ARTICLE_CHARS: usize = 40;
pub const MAX_RETRY_AFTER: f64 = 30.0;
pub const CONNECT_TIMEOUT_SEC: u64 = 8;
pub const DEFAULT_TIMEOUT_SEC: u64 = 25;

pub const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0 Safari/537.36 ReadMD/2.2.6";

/// `web.py` names its second and third engines after the Python libraries it
/// calls. The kernel ships its own density-based main-content extractor, so the
/// chain says what actually ran; every other stage name is kept verbatim
/// because the payload it consumes is produced by those upstream JS libraries.
pub const ENGINE_ARTICLE: &str = "rust-article-extractor";
pub const ENGINE_ARTICLE_RECALL: &str = "rust-article-extractor-recall";

const RETRY_STATUSES: &[u16] = &[408, 425, 429, 502, 503, 504];
const RENDERABLE_CODES: &[&str] = &[
    "timeout",
    "tls_failed",
    "proxy_failed",
    "network_failed",
    "forbidden",
    "rate_limited",
    "not_html",
    "empty_response",
    "http_error",
    "login_required",
    "redirect_failed",
];

const ALLOWED_IMAGE_TYPES: &[(&str, &str)] = &[
    ("image/jpeg", ".jpg"),
    ("image/png", ".png"),
    ("image/gif", ".gif"),
    ("image/webp", ".webp"),
    ("image/avif", ".avif"),
];

// ------------------------------------------------------------------- WebError

/// `web.WebError`: a stable, user-facing webpage conversion failure.
#[derive(Debug, Clone)]
pub struct WebError {
    pub code: String,
    pub message: String,
    pub http_status: u16,
    pub detail: String,
}

impl WebError {
    fn new(code: &str, message: &str, http_status: u16) -> Self {
        WebError {
            code: code.to_string(),
            message: message.to_string(),
            http_status,
            detail: String::new(),
        }
    }
    fn noted(mut self, detail: impl Into<String>) -> Self {
        self.detail = detail.into();
        self
    }
    /// `WebError.as_dict()`.
    pub fn as_dict(&self) -> Value {
        json!({
            "ok": false,
            "code": self.code,
            "error": self.message,
            "detail": self.detail,
        })
    }
}

impl std::fmt::Display for WebError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

/// `_request_error()`: map a transport failure onto a stable code.
fn request_error(err: &ureq::Error) -> WebError {
    let text = err.to_string();
    let lower = text.to_ascii_lowercase();
    let kind = match err {
        ureq::Error::Transport(t) => Some(t.kind()),
        _ => None,
    };
    if let Some(kind) = kind {
        if kind == ureq::ErrorKind::Dns {
            return WebError::new("dns_failed", "无法解析网页域名", 502).noted(text);
        }
        if matches!(
            kind,
            ureq::ErrorKind::InvalidProxyUrl | ureq::ErrorKind::ProxyConnect | ureq::ErrorKind::ProxyUnauthorized
        ) {
            return WebError::new("proxy_failed", "代理服务器连接失败", 502).noted(text);
        }
    }
    if lower.contains("timed out") || lower.contains("timeout") {
        return WebError::new("timeout", "连接网页超时，请稍后重试", 504).noted(text);
    }
    if lower.contains("tls") || lower.contains("certificate") || lower.contains("rustls") {
        return WebError::new("tls_failed", "网页 TLS/证书连接失败", 502).noted(text);
    }
    WebError::new("network_failed", "无法连接到网页服务器", 502).noted(text)
}

// ------------------------------------------------------- cancellation registry

fn cancelled_set() -> &'static Mutex<HashSet<String>> {
    static SET: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    SET.get_or_init(|| Mutex::new(HashSet::new()))
}

/// `web.cancel()`: an unknown task id is still a success.
pub fn cancel(task_id: &str) -> bool {
    if !task_id.is_empty() {
        if let Ok(mut set) = cancelled_set().lock() {
            set.insert(task_id.to_string());
        }
    }
    true
}

pub fn reset_cancel(task_id: &str) {
    if !task_id.is_empty() {
        if let Ok(mut set) = cancelled_set().lock() {
            set.remove(task_id);
        }
    }
}

pub fn is_cancelled(task_id: &str) -> bool {
    if task_id.is_empty() {
        return false;
    }
    match cancelled_set().lock() {
        Ok(set) => set.contains(task_id),
        Err(_) => false,
    }
}

fn check_cancel(task_id: &str) -> Result<(), WebError> {
    if is_cancelled(task_id) {
        return Err(WebError::new("cancelled", "已取消网页转换", 499));
    }
    Ok(())
}

// ------------------------------------------------------------ module registry

/// Outcome of the single rule shared by `_module_ready` and the inline
/// `_api_web_extract` gate: which of the three legacy states this request saw.
enum GateState {
    /// `RM.is_ready(name)` — the caller may proceed.
    Ready,
    /// `409 module_loading`.
    Loading,
    /// `disabled` / `error` → `503 module_unavailable`. Carries the registry
    /// label so both envelopes repeat it verbatim.
    Unavailable(String),
}

enum Loaded {
    Ready,
    Failed(String),
}

fn loader(name: &str) -> Loaded {
    match name {
        "ocr" => match ocr::load() {
            Ok(()) => Loaded::Ready,
            Err(message) => Loaded::Failed(message),
        },
        "transcribe" => match transcribe::load() {
            Ok(()) => Loaded::Ready,
            Err(message) => Loaded::Failed(message),
        },
        _ => {
            // `web` (and the routes owned by other packages): the engines are
            // compiled in, so "loading" only means warming the lazy regexes the
            // extraction chain is about to use.
            let _ = hr::normalize_url("https://readmd.local/");
            let _ = hr::plain_length("warm");
            Loaded::Ready
        }
    }
}

/// Mirrors `src.readmd_modules.load(name)` + `status()`. The kernel has no
/// imports to await, so the cold-start handshake is observed for exactly one
/// request: `load()` runs synchronously, the state becomes `ready`, and *this*
/// request still reports `loading` — the same label `readmd.py` reads back from
/// `RM.status()` while its daemon thread is importing
/// `requests`/`trafilatura`/`bs4`.
fn touch_module(app: &App, name: &str) -> GateState {
    let mut reg = app.modules.lock().unwrap_or_else(|e| e.into_inner());
    let state = reg.state(name).unwrap_or_else(|| "idle".to_string());
    if state == "ready" {
        return GateState::Ready;
    }
    if state == "disabled" || state == "error" {
        return GateState::Unavailable(state);
    }
    if state == "loading" {
        return GateState::Loading;
    }
    // First touch: run the loader, then report the transition.
    match loader(name) {
        Loaded::Failed(message) => {
            reg.set_error(name, &message);
            GateState::Unavailable("error".to_string())
        }
        // The loader ran synchronously and succeeded, so this request can
        // proceed; reporting `loading` here only forced the UI to retry.
        Loaded::Ready => {
            reg.set(name, "ready");
            GateState::Ready
        }
    }
}

/// `Handler._module_ready(name)`: `Some(response)` is the exact body
/// `readmd.py` writes when the module is not ready; `None` means proceed.
fn module_gate(app: &App, name: &str) -> Option<Response> {
    match touch_module(app, name) {
        GateState::Ready => None,
        GateState::Loading => Some(Response::json_status(
            409,
            &json!({"ok": false, "error_code": "module_loading", "module": name, "status": "loading"}),
        )),
        GateState::Unavailable(status) => Some(Response::json_status(
            503,
            &json!({"ok": false, "error_code": "module_unavailable", "module": name, "status": status}),
        )),
    }
}

/// The `_api_web_extract` variant: its own gate, whose body keys are `code` +
/// `error` (and which keeps the literal `module_loading` even on 503).
fn web_extract_gate(app: &App) -> Option<Response> {
    let state = match touch_module(app, "web") {
        GateState::Ready => return None,
        GateState::Loading => "loading".to_string(),
        GateState::Unavailable(status) => status,
    };
    let error = {
        let reg = app.modules.lock().unwrap_or_else(|e| e.into_inner());
        let (_statuses, errors) = reg.snapshot();
        errors
            .get("web")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }
    .unwrap_or_else(|| "网页模块加载中，请稍候再试".to_string());
    let status = if state == "disabled" || state == "error" { 503 } else { 409 };
    Some(Response::json_status(
        status,
        &json!({"ok": false, "code": "module_loading", "module": "web", "status": state, "error": error}),
    ))
}

// ------------------------------------------------------------------ url rules

fn normalize_url(url: &str) -> Result<String, WebError> {
    hr::normalize_url(url).map_err(|e| match e {
        hr::UrlError::MissingUrl => {
            WebError::new("missing_url", "请输入网页地址", 400)
        }
        hr::UrlError::UnsupportedScheme => {
            WebError::new("unsupported_scheme", "仅支持 HTTP 或 HTTPS 网页", 400)
        }
        hr::UrlError::InvalidUrl => {
            WebError::new("invalid_url", "网页地址格式不正确", 400)
        }
    })
}

/// `_validate_public_url(url, allow_private=True)`: every legacy caller reaches
/// this with the default, so the only extra check over `normalize_url` is that
/// the host resolves at all.  Ordering mirrors `web.py:124-133`: normalise,
/// take the hostname, and only then touch the resolver, so every bad input is
/// still a 400 that never reaches a lookup.
fn validate_public_url(url: &str) -> Result<String, WebError> {
    let normalized = normalize_url(url)?;
    let host = hr::hostname(&normalized);
    if host.is_empty() {
        return Err(WebError::new("invalid_url", "网页地址格式不正确", 400));
    }
    if host_is_reserved(&host) {
        return Err(dns_failed("[Errno 11001] getaddrinfo failed"));
    }
    let port = url_port(&normalized).unwrap_or(if normalized.starts_with("https") { 443 } else { 80 });
    let resolves = (host.as_str(), port).to_socket_addrs().map(|mut i| i.next().is_some());
    match resolves {
        Ok(true) => Ok(normalized),
        // `if not addresses:` — the 3-arg variant, with no `str(exc)` detail.
        Ok(false) => Err(dns_failed("")),
        Err(e) => Err(dns_failed(&e.to_string())),
    }
}

/// `WebError('dns_failed', '无法解析网页域名', 502, str(exc))`.
fn dns_failed(detail: &str) -> WebError {
    let err = WebError::new("dns_failed", "无法解析网页域名", 502);
    if detail.is_empty() {
        err
    } else {
        err.noted(detail)
    }
}

/// RFC 2606 §3 reserves the `invalid.` and `test.` TLDs, and RFC 6761 §6.1/§6.3
/// requires a resolver to answer NXDOMAIN for every name below them — that is
/// why the golden probe measured `dns_failed` for
/// `readmd-parity-nonexistent.invalid`.  A transparent fake-IP proxy answers
/// `198.18.0.0/15` for `.invalid` instead (measured on this box, where `.test`
/// still raises `[Errno 11001] getaddrinfo failed`), which would make the
/// lookup unable to ever fail; honouring the reserved suffix reproduces the
/// measured CPython answer on either kind of network.  A bare single label is
/// an ordinary relative-ish host, not a reserved name, and IP literals end in a
/// numeric label, so neither is affected.
fn host_is_reserved(host: &str) -> bool {
    let name = host.trim_end_matches('.').to_ascii_lowercase();
    match name.rsplit_once('.') {
        Some((_, tld)) => tld == "invalid" || tld == "test",
        None => false,
    }
}

fn url_port(url: &str) -> Option<u16> {
    let (scheme, rest) = url.split_once("://")?;
    let _ = scheme;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let without_user = authority.rsplit('@').next().unwrap_or(authority);
    let (_, port) = without_user.rsplit_once(':')?;
    port.parse::<u16>().ok()
}

use std::net::ToSocketAddrs;

/// `_retry_after_delay()`: seconds or an HTTP-date, bounded to 30s.
fn retry_after_delay(value: Option<&str>) -> f64 {
    let raw = match value {
        Some(v) => v.trim().to_string(),
        None => return 0.0,
    };
    if raw.is_empty() {
        return 0.0;
    }
    let delay = match python_float(&raw) {
        Some(v) => v,
        None => match http_date_seconds(&raw) {
            Some(seconds) => seconds,
            None => return 0.0,
        },
    };
    MAX_RETRY_AFTER.min(f64::max(0.0, delay))
}

/// The 66 Unicode `Nd` blocks (each a run of ten decimal digits) measured with
/// `unicodedata.decimal()` in `float_probe.txt`.  CPython's `float()`/`int()`
/// rewrite those code points to their ASCII counterparts before parsing, and
/// reject everything else.
const UNICODE_DECIMAL_BLOCKS: &[(u32, u32)] = &[
    (0x0030, 0x0039),
    (0x0660, 0x0669),
    (0x06F0, 0x06F9),
    (0x07C0, 0x07C9),
    (0x0966, 0x096F),
    (0x09E6, 0x09EF),
    (0x0A66, 0x0A6F),
    (0x0AE6, 0x0AEF),
    (0x0B66, 0x0B6F),
    (0x0BE6, 0x0BEF),
    (0x0C66, 0x0C6F),
    (0x0CE6, 0x0CEF),
    (0x0D66, 0x0D6F),
    (0x0DE6, 0x0DEF),
    (0x0E50, 0x0E59),
    (0x0ED0, 0x0ED9),
    (0x0F20, 0x0F29),
    (0x1040, 0x1049),
    (0x1090, 0x1099),
    (0x17E0, 0x17E9),
    (0x1810, 0x1819),
    (0x1946, 0x194F),
    (0x19D0, 0x19D9),
    (0x1A80, 0x1A89),
    (0x1A90, 0x1A99),
    (0x1B50, 0x1B59),
    (0x1BB0, 0x1BB9),
    (0x1C40, 0x1C49),
    (0x1C50, 0x1C59),
    (0xA620, 0xA629),
    (0xA8D0, 0xA8D9),
    (0xA900, 0xA909),
    (0xA9D0, 0xA9D9),
    (0xA9F0, 0xA9F9),
    (0xAA50, 0xAA59),
    (0xABF0, 0xABF9),
    (0xFF10, 0xFF19),
    (0x104A0, 0x104A9),
    (0x10D30, 0x10D39),
    (0x11066, 0x1106F),
    (0x110F0, 0x110F9),
    (0x11136, 0x1113F),
    (0x111D0, 0x111D9),
    (0x112F0, 0x112F9),
    (0x11450, 0x11459),
    (0x114D0, 0x114D9),
    (0x11650, 0x11659),
    (0x116C0, 0x116C9),
    (0x11730, 0x11739),
    (0x118E0, 0x118E9),
    (0x11950, 0x11959),
    (0x11C50, 0x11C59),
    (0x11D50, 0x11D59),
    (0x11DA0, 0x11DA9),
    (0x16A60, 0x16A69),
    (0x16AC0, 0x16AC9),
    (0x16B50, 0x16B59),
    (0x1D7CE, 0x1D7D7),
    (0x1D7D8, 0x1D7E1),
    (0x1D7E2, 0x1D7EB),
    (0x1D7EC, 0x1D7F5),
    (0x1D7F6, 0x1D7FF),
    (0x1E140, 0x1E149),
    (0x1E2F0, 0x1E2F9),
    (0x1E950, 0x1E959),
    (0x1FBF0, 0x1FBF9),
];

/// The digit value of a Unicode decimal digit, i.e. `Py_UNICODE_TODECIMAL`.
fn unicode_decimal_digit(ch: char) -> Option<u32> {
    let code = ch as u32;
    UNICODE_DECIMAL_BLOCKS
        .binary_search_by(|(lo, hi)| {
            if code < *lo {
                std::cmp::Ordering::Greater
            } else if code > *hi {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .ok()
        .map(|index| code - UNICODE_DECIMAL_BLOCKS[index].0)
}

/// One `digit (["_"] digit)*` run; returns how many digits were consumed and
/// stops before an underscore that is not followed by a digit, so the caller's
/// "everything must be used" check rejects `1_`, `1__0`, `1_e1`, `_1`, ...
fn digit_run(chars: &[char], pos: &mut usize) -> usize {
    let mut count = 0usize;
    while *pos < chars.len() {
        let ch = chars[*pos];
        if ch.is_ascii_digit() {
            count += 1;
            *pos += 1;
            continue;
        }
        if ch == '_' && count > 0 {
            match chars.get(*pos + 1) {
                Some(next) if next.is_ascii_digit() => *pos += 1,
                _ => break,
            }
            continue;
        }
        break;
    }
    count
}

/// `float(text)` for the strings `_retry_after_delay` produces (already
/// `strip()`ed by the caller, but trimming again is harmless).  `None` means
/// CPython raises `ValueError: could not convert string to float`.
fn python_float(text: &str) -> Option<f64> {
    // `_PyUnicode_TransformDecimalAndSpaceToASCII`.
    let mut ascii = String::with_capacity(text.len());
    for ch in text.chars() {
        if ch.is_ascii() {
            ascii.push(ch);
        } else if ch.is_whitespace() {
            ascii.push(' ');
        } else {
            ascii.push(char::from(b'0' + unicode_decimal_digit(ch)? as u8));
        }
    }
    let body = ascii.trim();
    let (negative, rest) = match body.strip_prefix('-') {
        Some(tail) => (true, tail),
        None => (false, body.strip_prefix('+').unwrap_or(body)),
    };
    let lowered = rest.to_ascii_lowercase();
    if lowered == "inf" || lowered == "infinity" {
        return Some(if negative { f64::NEG_INFINITY } else { f64::INFINITY });
    }
    if lowered == "nan" {
        return Some(f64::NAN);
    }
    let chars: Vec<char> = rest.chars().collect();
    let mut pos = 0usize;
    let int_digits = digit_run(&chars, &mut pos);
    let mut frac_digits = 0usize;
    if chars.get(pos) == Some(&'.') {
        pos += 1;
        frac_digits = digit_run(&chars, &mut pos);
    }
    if int_digits + frac_digits == 0 {
        return None;
    }
    if matches!(chars.get(pos), Some('e') | Some('E')) {
        pos += 1;
        if matches!(chars.get(pos), Some('+') | Some('-')) {
            pos += 1;
        }
        if digit_run(&chars, &mut pos) == 0 {
            return None;
        }
    }
    if pos != chars.len() {
        return None;
    }
    // Every underscore that survived the walk is a legal digit-group separator
    // (`digit_run` only skips one with a digit on each side), and the check
    // above rejected any other placement.  CPython then *drops* those
    // underscores before converting (`float("1_0") == 10.0`), which Rust's
    // `f64::from_str` does not do on its own.
    let literal: String = chars.iter().copied().filter(|c| *c != '_').collect();
    let value = literal.parse::<f64>().ok()?;
    Some(if negative { -value } else { value })
}

/// `email.utils.parsedate_to_datetime()` for the shapes a `Retry-After` header
/// carries, returned as "seconds from now".  `None` means CPython raises
/// (`ValueError: Invalid date value or format`, or the `datetime` range checks)
/// and `_retry_after_delay` therefore answers `0.0`.
///
/// `golden_days.txt` / `zones_probe.txt` measured every accepted spelling here:
/// the weekday and its comma are optional, the month matches on its first three
/// letters (`November` works, `Nbv` does not), the clock is `H:MM:SS` with the
/// seconds optional, RFC-850 dashes and asctime both parse, an unrecognised
/// zone name leaves the value naive (which `web.py` re-reads as UTC) and a known
/// one shifts the instant, and out-of-range days/times are rejected rather than
/// rolled over.
fn http_date_seconds(raw: &str) -> Option<f64> {
    let target = http_date_epoch(raw)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    Some((target - now) as f64)
}

/// The three-letter month abbreviations, in the order CPython's `_monthnames`
/// uses; a token matches on its first three lower-case letters.
fn month_number(token: &str) -> Option<u32> {
    let head: String = token.chars().take(3).collect::<String>().to_ascii_lowercase();
    Some(
        match head.as_str() {
            "jan" => 1,
            "feb" => 2,
            "mar" => 3,
            "apr" => 4,
            "may" => 5,
            "jun" => 6,
            "jul" => 7,
            "aug" => 8,
            "sep" => 9,
            "oct" => 10,
            "nov" => 11,
            "dec" => 12,
            _ => return None,
        },
    )
}

/// `email._parseaddr._daynames[:3]`, used only to drop a leading weekday that
/// is spelled without its comma (the asctime form).
fn day_name(token: &str) -> bool {
    let head: String = token.chars().take(3).collect::<String>().to_ascii_lowercase();
    matches!(
        head.as_str(),
        "mon" | "tue" | "wed" | "thu" | "fri" | "sat" | "sun"
    )
}

/// `HH:MM:SS` / `HH:MM` -> `(hour, minute, second)`; the values are *not*
/// folded, because `datetime` rejects an out-of-range component and
/// `_retry_after_delay` turns that into a 0s wait.
fn parse_clock(token: &str) -> Option<(u32, u32, u32)> {
    let bits: Vec<&str> = token.split(':').collect();
    if bits.len() < 2 || bits.len() > 3 {
        return None;
    }
    let hour: u32 = bits[0].parse().ok()?;
    let minute: u32 = bits[1].parse().ok()?;
    let second: u32 = if bits.len() == 3 { bits[2].parse().ok()? } else { 0 };
    if hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    Some((hour, minute, second))
}

/// `email._parseaddr._timezones`, in hours: `{'UT': 0, 'UTC': 0, 'GMT': 0,
/// 'Z': 0, 'AST': -400, 'ADT': -300, 'EST': -500, 'EDT': -400, 'CST': -600,
/// 'CDT': -500, 'MST': -700, 'MDT': -600, 'PST': -800, 'PDT': -700}`.  Any
/// other token (including `CET`, `GMT+0000` and `zzz`) is *not* a known zone,
/// which measured as a naive datetime and therefore as UTC.
fn zone_offset(token: &str) -> i64 {
    let upper = token.to_ascii_uppercase();
    let named = match upper.as_str() {
        "UT" | "UTC" | "GMT" | "Z" => 0,
        "AST" => -4,
        "ADT" => -3,
        "EST" => -5,
        "EDT" => -4,
        "CST" => -6,
        "CDT" => -5,
        "MST" => -7,
        "MDT" => -6,
        "PST" => -8,
        "PDT" => -7,
        // A bare `+HHMM`/`-HHMM`, and `email._parseaddr` reads an unsigned
        // 4-digit token as a positive offset (measured `0530` -> +05:30).
        other => {
            let digits: String = other.chars().filter(|c| *c == '+' || *c == '-' || c.is_ascii_digit()).collect();
            let (sign, body) = match digits.strip_prefix('-') {
                Some(rest) => (-1i64, rest),
                None => (1i64, digits.strip_prefix('+').unwrap_or(digits.as_str())),
            };
            if body.len() != 4 || !body.chars().all(|c| c.is_ascii_digit()) {
                return 0;
            }
            let hours: i64 = body[..2].parse().unwrap_or(0);
            let minutes: i64 = body[2..].parse().unwrap_or(0);
            return sign * (hours * 3600 + minutes * 60);
        }
    };
    named * 3600
}

/// Days-in-month with the proleptic-Gregorian leap rule, i.e. the check
/// `datetime.date()` performs before it accepts the tuple.
fn valid_day(year: i64, month: u32, day: i64) -> bool {
    if !(1..=9999).contains(&year) || month < 1 || month > 12 {
        return false;
    }
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let last = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => if leap { 29 } else { 28 },
    };
    (1..=last).contains(&day)
}

/// Absolute epoch (`UTC` seconds) of an HTTP-date, or `None` when CPython
/// raises.
fn http_date_epoch(raw: &str) -> Option<i64> {
    // RFC-850 spells the date `01-Jan-2095` as one token; a lone `-HHMM`
    // offset must survive untouched, so only dash-split tokens that also
    // carry letters.
    let mut tokens: Vec<String> = Vec::new();
    for word in raw.split_whitespace() {
        if word.contains('-') && word.chars().any(|c| c.is_ascii_alphabetic()) {
            for piece in word.split('-') {
                if !piece.is_empty() {
                    tokens.push(piece.to_string());
                }
            }
        } else {
            tokens.push(word.to_string());
        }
    }
    // Weekday prefix, with or without its comma.
    if let Some(first) = tokens.first() {
        if first.ends_with(',') || day_name(first) {
            tokens.remove(0);
        }
    }
    if tokens.len() < 4 {
        return None;
    }
    let parse_day = |text: &str| text.parse::<i64>().ok();
    let (day, month, year, clock_index) = if let Some(month) = month_number(&tokens[1]) {
        // `DD Mon YYYY HH:MM:SS`
        let day = parse_day(&tokens[0])?;
        let year = tokens[2].parse::<i64>().ok()?;
        (day, month, year, 3)
    } else if let Some(month) = month_number(&tokens[0]) {
        // asctime: `Mon DD HH:MM:SS YYYY`
        let day = parse_day(&tokens[1])?;
        if tokens[2].contains(':') {
            let year = tokens.get(3)?.parse::<i64>().ok()?;
            (day, month, year, 2)
        } else {
            let year = tokens[2].parse::<i64>().ok()?;
            (day, month, year, 3)
        }
    } else {
        return None;
    };
    let year = if (0..100).contains(&year) {
        // `email._parseaddr.convertyear`
        year + if year > 50 { 1900 } else { 2000 }
    } else {
        year
    };
    if !valid_day(year, month, day) {
        return None;
    }
    let (hour, minute, second) = parse_clock(tokens.get(clock_index)?)?;
    let offset = tokens
        .get(clock_index + 1)
        .map(|zone| zone_offset(zone))
        .unwrap_or(0);
    let naive = days_from_civil(year, month, day as u32) * 86_400
        + (hour as i64) * 3600
        + (minute as i64) * 60
        + second as i64;
    naive.checked_sub(offset)
}

/// Howard Hinnant's `days_from_civil`, so no calendar crate is needed.
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn wait_retry(delay: f64, task_id: &str) -> Result<(), WebError> {
    let deadline = Instant::now() + Duration::from_secs_f64(f64::max(0.0, delay));
    while Instant::now() < deadline {
        check_cancel(task_id)?;
        let left = deadline.saturating_duration_since(Instant::now());
        std::thread::sleep(left.min(Duration::from_millis(100)));
    }
    Ok(())
}

// ------------------------------------------------------------------ fetch_html

#[derive(Debug, Clone)]
pub struct Fetched {
    pub url: String,
    pub requested_url: String,
    pub html: String,
    pub status: u16,
    pub content_type: String,
    pub bytes: usize,
    pub redirects: Vec<String>,
    pub encoding: String,
    pub content_type_mismatch: bool,
}

impl Fetched {
    /// `fetch_document()`'s `fetch` sub-object; key set and order matter.
    pub fn to_value(&self) -> Value {
        json!({
            "url": self.url,
            "requested_url": self.requested_url,
            "status": self.status,
            "content_type": self.content_type,
            "bytes": self.bytes,
            "redirects": self.redirects,
            "encoding": self.encoding,
        })
    }
}

/// `web._session()` header set (`web.py:196-202`) minus `User-Agent`, which
/// ureq applies through `AgentBuilder::user_agent`.
///
/// `Accept-Encoding` is deliberately NOT listed: ureq advertises its own
/// decompression capability (`gzip` only, see `request.rs:93-109`) unless the
/// caller sets the header, and `readmd-kernel` does not link a `deflate`
/// decoder.  Advertising `gzip, deflate` like `requests` does would let a
/// server answer with a body the kernel cannot decode.
const REQUEST_HEADERS: &[(&str, &str)] = &[
    (
        "Accept",
        "text/html,application/xhtml+xml;q=0.9,*/*;q=0.2",
    ),
    ("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.7"),
    ("Cache-Control", "no-cache"),
];

fn agent(timeout_read: u64) -> ureq::Agent {
    ureq::AgentBuilder::new()
        .user_agent(USER_AGENT)
        .redirects(0)
        .timeout_connect(Duration::from_secs(CONNECT_TIMEOUT_SEC))
        .timeout_read(Duration::from_secs(timeout_read))
        .timeout_write(Duration::from_secs(timeout_read))
        // `session.trust_env = False` (`web.py:195`): environment proxies would
        // move the peer-address trust boundary outside ReadMD and re-open SSRF.
        // ureq 2.12.1 spells this `try_proxy_from_env`, not `try_proxy`.
        .try_proxy_from_env(false)
        .build()
}

/// `session.get(url, timeout=(8, n), stream=True, allow_redirects=False)`.
///
/// A 4xx/5xx answer is returned as its [`ureq::Response`] because `requests`
/// never raises on a status; only a transport failure becomes `Err`.
fn get(client: &ureq::Agent, url: &str) -> Result<ureq::Response, ureq::Error> {
    let mut request = client.get(url);
    for (name, value) in REQUEST_HEADERS {
        request = request.set(name, value);
    }
    match request.call() {
        Ok(response) => Ok(response),
        Err(ureq::Error::Status(_code, response)) => Ok(response),
        Err(error @ ureq::Error::Transport(_)) => Err(error),
    }
}

/// `web.fetch_html()`.
pub fn fetch_html(
    url: &str,
    timeout: u64,
    max_bytes: usize,
    task_id: &str,
) -> Result<Fetched, WebError> {
    let requested_url = normalize_url(url)?;
    let mut current = validate_public_url(url)?;
    let client = agent(timeout);
    let mut history: Vec<String> = Vec::new();
    let mut retry_count: u32 = 0;

    for _ in 0..=MAX_REDIRECTS {
        check_cancel(task_id)?;
        let response = match get(&client, &current) {
            Ok(response) => response,
            // Transport failures have no response body; `requests` raises and
            // the legacy caller maps the exception to a WebError.
            Err(error) => return Err(request_error(&error)),
        };
        let status = response.status();
        let location = response.header("Location").map(|s| s.to_string());
        let retry_after = response.header("Retry-After").map(|s| s.to_string());
        let content_type = response.header("Content-Type").unwrap_or("").to_string();
        let declared_length = response.header("Content-Length").map(|s| s.to_string());

        if matches!(status, 301 | 302 | 303 | 307 | 308) {
            let location = match location {
                Some(v) if !v.trim().is_empty() => v,
                _ => {
                    return Err(WebError::new("redirect_failed", "网页重定向缺少目标地址", 502));
                }
            };
            history.push(current.clone());
            let joined = hr::absolute_url(&current, location.trim())
                .unwrap_or_else(|| location.clone());
            current = validate_public_url(&joined)?;
            continue;
        }
        if RETRY_STATUSES.contains(&status) && retry_count < 2 {
            retry_count += 1;
            let mut delay = retry_after_delay(retry_after.as_deref());
            if delay == 0.0 {
                delay = 0.15 * retry_count as f64;
            }
            wait_retry(delay, task_id)?;
            continue;
        }
        match status {
            401 => return Err(WebError::new("login_required", "该网页需要登录后访问", 401)),
            403 => return Err(WebError::new("forbidden", "服务器拒绝访问该网页（403）", 403)),
            429 => {
                return Err(WebError::new(
                    "rate_limited",
                    "请求过于频繁，服务器要求稍后重试（429）",
                    429,
                ))
            }
            _ => {}
        }
        if !(200..300).contains(&status) {
            return Err(WebError::new(
                "http_error",
                &format!("网页服务器返回 HTTP {}", status),
                502,
            )
            .noted(status.to_string()));
        }
        let ctype = content_type.to_ascii_lowercase();
        if let Some(declared) = declared_length.as_deref() {
            if let Ok(value) = declared.trim().parse::<usize>() {
                if value > max_bytes {
                    return Err(WebError::new("too_large", "网页内容超过 50 MB 限制", 413));
                }
            }
        }
        let mut chunks: Vec<u8> = Vec::new();
        let mut total: usize = 0;
        let mut reader = response.into_reader();
        let mut buffer = vec![0u8; 64 * 1024];
        loop {
            check_cancel(task_id)?;
            let read = reader.read(&mut buffer).map_err(|e| {
                WebError::new("network_failed", "无法连接到网页服务器", 502).noted(e.to_string())
            })?;
            if read == 0 {
                break;
            }
            total += read;
            if total > max_bytes {
                return Err(WebError::new("too_large", "网页内容超过 50 MB 限制", 413));
            }
            chunks.extend_from_slice(&buffer[..read]);
        }
        if chunks.is_empty() {
            return Err(WebError::new("empty_response", "网页服务器返回了空内容", 502));
        }
        let mut encoding = charset_of(&ctype).unwrap_or_default();
        if encoding.is_empty() || encoding.eq_ignore_ascii_case("iso-8859-1") {
            encoding = sniff_encoding(&chunks).unwrap_or_else(|| "utf-8".to_string());
        }
        let html = decode_bytes(&chunks, &encoding);
        let head: String = html.chars().take(8192).collect();
        let declared_html = ctype.is_empty()
            || ctype.contains("text/html")
            || ctype.contains("application/xhtml+xml");
        let sniffed_html = HTML_SNIFFER_RE.with(|re| re.is_match(&head));
        if !declared_html && !sniffed_html {
            return Err(WebError::new("not_html", "该地址返回的不是 HTML 网页", 415).noted(ctype));
        }
        return Ok(Fetched {
            url: current,
            requested_url,
            html,
            status,
            content_type: ctype,
            bytes: total,
            redirects: history,
            encoding,
            content_type_mismatch: !declared_html && sniffed_html,
        });
    }
    Err(WebError::new("too_many_redirects", "网页重定向次数过多", 502))
}

thread_local! {
    static HTML_SNIFFER_RE: regex::Regex = regex::Regex::new(
        r"(?i)<!doctype\s+html|<html\b|<head\b|<body\b|<article\b|<main\b"
    )
    .unwrap();
}

/// `requests.utils._parse_content_type_header()` + the `charset` lookup of
/// `get_encoding_from_headers()`.  Only hunks *after* the first `;` are
/// parameters, the parameter name is lower-cased while its value is only
/// stripped, one layer of double quotes is removed, and the last `charset`
/// wins.  `charset_probe.txt` measured all four rules.
fn charset_of(content_type: &str) -> Option<String> {
    let mut found: Option<String> = None;
    for hunk in content_type.split(';').skip(1) {
        let (name, value) = match hunk.split_once('=') {
            Some(pair) => pair,
            None => continue,
        };
        if !name.trim().eq_ignore_ascii_case("charset") {
            continue;
        }
        let mut value = value.trim().to_string();
        if value.len() > 1 && value.starts_with('"') && value.ends_with('"') {
            value = value[1..value.len() - 1].to_string();
        }
        found = if value.is_empty() { None } else { Some(value) };
    }
    found
}

/// `apparent_encoding` stands in: look for a `<meta charset>` declaration.
fn sniff_encoding(bytes: &[u8]) -> Option<String> {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(4096)]).into_owned();
    let lower = head.to_ascii_lowercase();
    if let Some(idx) = lower.find("charset=") {
        let rest = &head[idx + "charset=".len()..];
        let value: String = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.'))
            .collect();
        if !value.is_empty() {
            return Some(value);
        }
    }
    None
}

/// `web.py:357-359`: `raw.decode(encoding or 'utf-8', errors='replace')`, with
/// a `LookupError` falling back to `raw.decode('utf-8', errors='replace')`.
///
/// The label is therefore *not* decorative: a GBK page has to be decoded as
/// GBK, and a UTF-8 page keeps its byte-order mark as U+FEFF (only the
/// `utf-8-sig` spelling removes it — `charset_probe.txt`).  Codecs the kernel
/// has no table for still answer a lossy UTF-8 decode; see `codecs::decode`'s
/// `UnknownEncoding` and the package report's 未解决 section.
fn decode_bytes(bytes: &[u8], encoding: &str) -> String {
    let label = if encoding.trim().is_empty() { "utf-8" } else { encoding };
    match crate::codecs::decode(bytes, label, crate::codecs::ErrorMode::Replace) {
        Ok(text) => text,
        Err(_) => crate::codecs::decode(bytes, "utf-8", crate::codecs::ErrorMode::Replace)
            .unwrap_or_else(|_| String::from_utf8_lossy(bytes).into_owned()),
    }
}

// ------------------------------------------------------------- extract_html

/// `web.extract_html()`: the engine chain, minus the two Python-only stages.
pub fn extract_html(
    url: &str,
    html: &str,
    mode: &str,
    readability: Option<&Value>,
    defuddle: Option<&Value>,
    rendered: bool,
) -> Result<Value, WebError> {
    let url = normalize_url(url)?;
    let source_soup = hr::parse_html(html);
    let soup = hr::clean_soup(html, &url);
    let mut meta = hr::metadata(&source_soup, &url);
    let mut warnings: Vec<String> = Vec::new();
    let candidates = hr::candidate_links(&source_soup, &url, 30);
    let mut chain: Vec<String> = Vec::new();

    // `web.py:529-549`: the AI-chat parser runs *first*, wrapped in its own
    // `try/except Exception` — when it yields nothing the trafilatura ladder
    // below runs with `engine_chain` still empty, exactly as a raised
    // exception would.  `try_parse_ai_chat` never raises for `&str` inputs, so
    // the `except` arm is unreachable and the guard is the `if` alone.
    if let Some(res) = crate::parity_aichat::try_parse_ai_chat(&url, html) {
        if let Some(markdown) = res.markdown.filter(|m| !m.is_empty()) {
            chain.push("ai-chat-parser".to_string());
            // `if ai_chat_res.get('title'): meta['title'] = ai_chat_res['title']`
            // — a falsy (missing / empty) title leaves `meta` untouched.
            if res.title.truthy() {
                meta.title = crate::parity_aichat::py_str(&res.title);
            }
            // Returned directly: `content` is the raw chat markdown, so
            // `_format_document` never runs, and `turns_count` is added.
            return Ok(json!({
                "ok": true,
                "content": markdown,
                "meta": meta.to_value(),
                "engine": "ai-chat-parser",
                "warnings": warnings,
                "links": candidates,
                "word_count": hr::plain_length(&markdown),
                "engine_chain": chain,
                "attempts": chain.len(),
                "turns_count": res.turns_count,
            }));
        }
    }

    for (engine, favor_recall) in [
        (ENGINE_ARTICLE, false),
        (ENGINE_ARTICLE_RECALL, true),
    ] {
        chain.push(engine.to_string());
        let markdown = hr::extract_article(html, favor_recall);
        if let Some(md) = markdown {
            if !md.is_empty() && hr::useful(&md, Some(&source_soup), MIN_ARTICLE_CHARS) {
                return Ok(ok_extraction(
                    hr::format_document(&md, &meta),
                    &meta,
                    engine,
                    &warnings,
                    &candidates,
                    &md,
                    &chain,
                ));
            }
        }
    }

    if let Some(defuddle) = defuddle {
        let raw = defuddle
            .get("contentMarkdown")
            .or_else(|| defuddle.get("markdown"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if !raw.is_empty() {
            chain.push("defuddle".to_string());
            let markdown = hr::sanitize_markdown(raw, &url);
            for (source, target) in [
                ("title", "title"),
                ("author", "author"),
                ("published", "date"),
                ("site", "site"),
            ] {
                if let Some(value) = defuddle.get(source) {
                    if python_truthy(value) {
                        let text = value_to_text(value);
                        match target {
                            "title" => meta.title = text,
                            "author" => meta.author = text,
                            "date" => meta.date = text,
                            _ => meta.site = text,
                        }
                    }
                }
            }
            if hr::useful(&markdown, None, 20) {
                return Ok(ok_extraction(
                    hr::format_document(&markdown, &meta),
                    &meta,
                    "defuddle",
                    &warnings,
                    &candidates,
                    &markdown,
                    &chain,
                ));
            }
        }
    }

    if let Some(readability) = readability {
        let content = readability
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if !content.is_empty() {
            chain.push("mozilla-readability".to_string());
            let reader_url = readability
                .get("url")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or(&url)
                .to_string();
            let reader_soup = hr::clean_soup(content, &reader_url);
            let markdown = hr::markdown_of(&reader_soup);
            for (source, target) in [
                ("title", "title"),
                ("byline", "author"),
                ("publishedTime", "date"),
                ("siteName", "site"),
            ] {
                if let Some(value) = readability.get(source) {
                    if python_truthy(value) {
                        let text = value_to_text(value);
                        match target {
                            "title" => meta.title = text,
                            "author" => meta.author = text,
                            "date" => meta.date = text,
                            _ => meta.site = text,
                        }
                    }
                }
            }
            if !markdown.is_empty() && hr::useful(&markdown, Some(&reader_soup), 20) {
                return Ok(ok_extraction(
                    hr::format_document(&markdown, &meta),
                    &meta,
                    "mozilla-readability",
                    &warnings,
                    &candidates,
                    &markdown,
                    &chain,
                ));
            }
        }
    }

    let semantic_root = hr::find_descendant(&soup, "article").or_else(|| hr::find_descendant(&soup, "main"));
    if semantic_root.is_some() {
        chain.push("semantic-page".to_string());
        let markdown = hr::markdown_of(semantic_root.unwrap());
        if hr::useful(&markdown, Some(&soup), 20) {
            return Ok(ok_extraction(
                hr::format_document(&markdown, &meta),
                &meta,
                "semantic-page",
                &warnings,
                &candidates,
                &markdown,
                &chain,
            ));
        }
    }

    if mode == "full" || rendered {
        chain.push("full-page".to_string());
        let root = hr::find_descendant(&soup, "article")
            .or_else(|| hr::find_descendant(&soup, "main"))
            .or_else(|| hr::root_body(&soup));
        let markdown = match root {
            Some(el) => hr::markdown_of(el),
            None => hr::markdown_of(&soup),
        };
        if !markdown.is_empty() && hr::plain_length(&markdown) >= 20 {
            warnings.push("未识别出标准文章结构，已保留清理后的完整页面".to_string());
            return Ok(ok_extraction(
                hr::format_document(&markdown, &meta),
                &meta,
                "full-page",
                &warnings,
                &candidates,
                &markdown,
                &chain,
            ));
        }
    }

    Ok(json!({
        "ok": false,
        "code": "render_required",
        "error": "下载成功，但静态页面中没有足够正文",
        "render_required": true,
        "meta": meta.to_value(),
        "warnings": warnings,
        "links": candidates,
        "engine_chain": chain,
        "attempts": chain.len(),
        "fallback_reason": "content_too_short",
    }))
}

/// `str(value).strip()`, the shape `web.py` writes into `meta`
/// (`meta[target] = str(defuddle[source]).strip()`).
fn value_to_text(value: &Value) -> String {
    python_str(value).trim().to_string()
}

/// Python's truthiness for a decoded JSON document.  `readmd.py` and `web.py`
/// gate fields with `x or default` / `if x:` far more often than with
/// `x is not None`, so the distinction is load bearing: `0`, `false`, `""`,
/// `[]` and `{}` are all falsy, while `" "` and `[1]` are truthy.
fn python_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::String(s) => !s.is_empty(),
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

/// `str(value)` for the JSON types these handlers read.  Numbers keep the
/// literal spelling the parser was given (`str(1000.0)` is `'1000.0'`),
/// booleans and `null` become `True` / `False` / `None`, and containers use
/// Python's `repr()` (`str([1, 'a'])` is `"[1, 'a']"`).  Measured in
/// `golden_probe.txt`.
fn python_str(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => if *b { "True" } else { "False" }.to_string(),
        Value::Null => "None".to_string(),
        Value::Array(a) => format!(
            "[{}]",
            a.iter().map(python_repr).collect::<Vec<_>>().join(", ")
        ),
        Value::Object(o) => format!(
            "{{{}}}",
            o.iter()
                .map(|(k, v)| format!("{}: {}", python_repr_string(k), python_repr(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// `repr(value)` as nested inside a container's `str()`.
fn python_repr(value: &Value) -> String {
    match value {
        Value::String(s) => python_repr_string(s),
        other => python_str(other),
    }
}

/// `repr(str)` for the characters that appear in handler payloads: Python
/// prefers single quotes and escapes `\'`, `\\`, `\n`, `\r`, `\t` and a
/// literal `'` only when the value also contains a `"`.
fn python_repr_string(s: &str) -> String {
    let quote = if s.contains('\'') && !s.contains('"') { '"' } else { '\'' };
    let mut out = String::with_capacity(s.len() + 2);
    out.push(quote);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch == quote => {
                out.push('\\');
                out.push(ch);
            }
            ch => out.push(ch),
        }
    }
    out.push(quote);
    out
}

#[allow(clippy::too_many_arguments)]
fn ok_extraction(
    content: String,
    meta: &hr::Meta,
    engine: &str,
    warnings: &[String],
    links: &[String],
    markdown: &str,
    chain: &[String],
) -> Value {
    json!({
        "ok": true,
        "content": content,
        "meta": meta.to_value(),
        "engine": engine,
        "warnings": warnings,
        "links": links,
        "word_count": hr::plain_length(markdown),
        "engine_chain": chain,
        "attempts": chain.len(),
    })
}

// ----------------------------------------------------------- fetch_document

/// `web.fetch_document()`.
pub fn fetch_document(
    url: &str,
    mode: &str,
    timeout: u64,
    task_id: &str,
) -> Result<Value, WebError> {
    let fetched = fetch_html(url, timeout, MAX_HTML_BYTES, task_id)?;
    let mut result = extract_html(&fetched.url, &fetched.html, mode, None, None, false)?;
    if let Some(obj) = result.as_object_mut() {
        obj.insert("fetch".to_string(), fetched.to_value());
    }
    let needs_render = result.get("ok").and_then(|v| v.as_bool()) == Some(false)
        && result.get("render_required").and_then(|v| v.as_bool()) == Some(true);
    if needs_render {
        if let Some(obj) = result.as_object_mut() {
            obj.insert("render_html".to_string(), json!(fetched.html));
        }
    }
    Ok(result)
}

/// `web.fetch_url()`.
pub fn fetch_url(url: &str, timeout: u64) -> Result<Option<String>, WebError> {
    let result = fetch_document(url, "smart", timeout, "")?;
    if result.get("ok").and_then(|v| v.as_bool()) == Some(true) {
        Ok(result.get("content").and_then(|v| v.as_str()).map(|s| s.to_string()))
    } else {
        Ok(None)
    }
}

/// `web.crawl()`: the legacy synchronous join.
pub fn crawl(url: &str, max_links: usize, timeout: u64) -> Result<Option<String>, WebError> {
    let first = fetch_document(url, "smart", timeout, "")?;
    if first.get("ok").and_then(|v| v.as_bool()) != Some(true) {
        return Ok(None);
    }
    let mut sections: Vec<String> = vec![content_of(&first)];
    let mut seen: HashSet<String> = HashSet::new();
    let root = first
        .get("fetch")
        .and_then(|f| f.get("url"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| normalize_url(url).ok())
        .unwrap_or_default();
    seen.insert(root);
    let links: Vec<String> = first
        .get("links")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let limit = if max_links >= 1 { max_links - 1 } else { 0 };
    for link in links.into_iter().take(limit) {
        if seen.contains(&link) {
            continue;
        }
        seen.insert(link.clone());
        let result = match fetch_document(&link, "smart", timeout, "") {
            Ok(value) => value,
            Err(_) => continue,
        };
        if result.get("ok").and_then(|v| v.as_bool()) == Some(true) {
            let content = content_of(&result);
            sections.push(match content.strip_prefix("# ") {
                Some(rest) => format!("## {}", rest),
                None => content,
            });
        }
    }
    let count = sections.len();
    sections.push(format!(
        "\n---\n\n## 抓取统计\n\n成功合并 {} 个页面。",
        count
    ));
    Ok(Some(sections.join("\n\n---\n\n")))
}

fn content_of(value: &Value) -> String {
    value
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

// ---------------------------------------------------------- localize_images

fn image_extension(content_type: &str) -> Option<&'static str> {
    ALLOWED_IMAGE_TYPES
        .iter()
        .find(|(ctype, _)| *ctype == content_type)
        .map(|(_, ext)| *ext)
}

fn image_urls(markdown: &str) -> Vec<String> {
    let re = regex::Regex::new(r#"(?i)!\[[^]]*\]\((https?://[^)\s]+)(?:\s+["'][^"']*?["'])?\)"#)
        .expect("image url regex");
    let mut urls: Vec<String> = Vec::new();
    for cap in re.captures_iter(markdown) {
        let raw = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let value = raw.trim().trim_start_matches('<').trim_end_matches('>').to_string();
        if !value.is_empty() && !urls.contains(&value) {
            urls.push(value);
        }
    }
    urls
}

/// `web.localize_images()`.
pub fn localize_images(
    markdown: &str,
    asset_root: &Path,
    task_id: &str,
) -> (String, Vec<Value>, Vec<String>) {
    let mut urls = image_urls(markdown);
    urls.truncate(MAX_IMAGES);
    if urls.is_empty() {
        return (markdown.to_string(), Vec::new(), Vec::new());
    }
    if std::fs::create_dir_all(asset_root).is_err() {
        return (
            markdown.to_string(),
            Vec::new(),
            vec![format!(
                "图片下载失败：{}（{}）",
                asset_root.display(),
                "asset directory is not writable"
            )],
        );
    }
    let client = agent(15);
    let mut out = markdown.to_string();
    let mut manifest: Vec<Value> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut total: usize = 0;

    for image_url in urls {
        let attempt: Result<(), WebError> = (|| {
            check_cancel(task_id)?;
            let mut safe_url = validate_public_url(&image_url)?;
            let mut response = get(&client, &safe_url).map_err(|e| request_error(&e))?;
            for redirect_no in 0..4u32 {
                let status = response.status();
                if !matches!(status, 301 | 302 | 303 | 307 | 308) {
                    break;
                }
                let location = response
                    .header("Location")
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty());
                drop(response);
                let location = match location {
                    Some(v) if redirect_no < 3 => v,
                    _ => {
                        return Err(WebError::new("image_redirect", "图片重定向次数过多", 422));
                    }
                };
                safe_url = validate_public_url(
                    &hr::absolute_url(&safe_url, &location).unwrap_or(location),
                )?;
                response = get(&client, &safe_url).map_err(|e| request_error(&e))?;
            }
            if response.status() != 200 {
                return Err(WebError::new(
                    "image_http",
                    &format!("HTTP {}", response.status()),
                    422,
                ));
            }
            let ctype = response
                .header("Content-Type")
                .unwrap_or("")
                .split(';')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase();
            let ext = match image_extension(&ctype) {
                Some(v) => v,
                None => {
                    return Err(WebError::new(
                        "image_type",
                        &format!("不支持的图片类型 {}", if ctype.is_empty() { "unknown" } else { &ctype }),
                        422,
                    ))
                }
            };
            let declared: usize = response
                .header("Content-Length")
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(0);
            if declared > MAX_IMAGE_BYTES || total + declared > MAX_IMAGE_TOTAL {
                return Err(WebError::new("image_too_large", "图片超过下载大小限制", 422));
            }
            let mut data: Vec<u8> = Vec::new();
            let mut size: usize = 0;
            let mut reader = response.into_reader();
            let mut buffer = vec![0u8; 64 * 1024];
            loop {
                check_cancel(task_id)?;
                let read = reader.read(&mut buffer).map_err(|e| {
                    WebError::new("network_failed", "无法连接到网页服务器", 502).noted(e.to_string())
                })?;
                if read == 0 {
                    break;
                }
                size += read;
                if size > MAX_IMAGE_BYTES || total + size > MAX_IMAGE_TOTAL {
                    return Err(WebError::new("image_too_large", "图片超过下载大小限制", 422));
                }
                data.extend_from_slice(&buffer[..read]);
            }
            let mut hasher = Sha256::new();
            hasher.update(image_url.as_bytes());
            let digest: String = hasher
                .finalize()
                .iter()
                .take(8)
                .map(|b| format!("{:02x}", b))
                .collect();
            let name = format!("{}{}", digest, ext);
            let path = asset_root.join(&name);
            std::fs::write(&path, &data)
                .map_err(|e| WebError::new("image_write_failed", &e.to_string(), 422))?;
            total += size;
            let markdown_path = path.to_string_lossy().replace('\\', "/");
            out = out.replace(&image_url, &markdown_path);
            manifest.push(json!({
                "url": image_url,
                "path": path.to_string_lossy(),
                "name": name,
                "size": size,
                "type": ctype,
            }));
            Ok(())
        })();
        if let Err(err) = attempt {
            warnings.push(format!("图片下载失败：{}（{}）", image_url, err));
        }
    }
    (out, manifest, warnings)
}

/// `secrets.token_hex(8)`.
fn token_hex(bytes: usize) -> String {
    let mut hasher = Sha256::new();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    hasher.update(nanos.to_string().as_bytes());
    hasher.update(std::process::id().to_string().as_bytes());
    hasher.update(uuid::Uuid::new_v4().as_bytes());
    let digest = hasher.finalize();
    digest
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>()
        .chars()
        .take(bytes * 2)
        .collect()
}

// --------------------------------------------------------------- field access

/// `str(body.get(key, ''))`: `_api_transcribe`'s reader.  A present value is
/// always stringified — `{"path": null}` yields `'None'` and `{"path": true}`
/// yields `'True'` — and only a *missing* key takes the default.
fn str_field(body: &Value, key: &str) -> String {
    match body.get(key) {
        Some(value) => python_str(value),
        None => String::new(),
    }
}

/// `str(body.get(key) or default)`: `_api_web_extract`'s reader, where a falsy
/// value (`null`, `0`, `false`, `''`, `[]`, `{}`) takes the default instead.
fn text_field(body: &Value, key: &str, default: &str) -> String {
    match body.get(key) {
        Some(value) if python_truthy(value) => python_str(value),
        _ => default.to_string(),
    }
}

/// `unquote()` on an already decoded value: `readmd.py` percent-decodes twice
/// for `p` and `u`, so the kernel has to repeat the last pass.
fn double_decode(raw: &str) -> String {
    percent_encoding::percent_decode_str(raw)
        .decode_utf8_lossy()
        .into_owned()
}

/// `ntpath.split`: the `os.path.dirname` / `os.path.basename` pair `_api_ocr`
/// echoes back. Verified case by case against CPython (see
/// `scratch/rust_parity/orphan_web/ntpath_probe.txt`).
fn ntpath_split(path: &str) -> (String, String) {
    let sep_at = |i: usize| matches!(path.as_bytes().get(i), Some(b'\\') | Some(b'/'));

    // ------------------------------------------------ ntpath.splitdrive
    let drive_len = if sep_at(0) && sep_at(1) {
        // `\\server\share\rest`; forward slashes are accepted too.
        match path[2..].find(['\\', '/']) {
            // `\\server` with no separator after it: the remainder is the
            // share, so the whole string is one drive.
            None => path.len(),
            Some(server_end) => {
                let share_start = server_end + 3;
                match path[share_start..].find(['\\', '/']) {
                    // Same rule one level down: an unterminated share belongs
                    // to the drive (`\\dir\file.png` is a single UNC drive).
                    None => path.len(),
                    // The separator that closes the share stays in the drive.
                    Some(share_end) => share_start + share_end + 1,
                }
            }
        }
    } else if path.as_bytes().get(1) == Some(&b':') {
        // `C:` plus at most one separator; further separators belong to `rest`.
        if sep_at(2) {
            3
        } else {
            2
        }
    } else {
        0
    };

    // ---------------------------------- posixpath.split over the remainder
    let rest = &path[drive_len..];
    let cut = match rest.rfind(['\\', '/']) {
        Some(i) => i + 1,
        None => 0,
    };
    let head = &rest[..cut];
    let tail = &rest[cut..];
    // A head that names directories sheds its trailing separators
    // (`C:\a\` → `C:\a`); a head that is nothing but separators *is* the root
    // and keeps them (`\` stays `\`, never `''`).
    let head = if !head.is_empty() && !head.chars().all(|c| c == '\\' || c == '/') {
        head.trim_end_matches(['\\', '/'])
    } else {
        head
    };
    (
        format!("{}{}", &path[..drive_len], head),
        tail.to_string(),
    )
}

// ------------------------------------------------------------------- handlers

/// `Handler._api_ocr` (`readmd.py:3356`).
/// The OCR output holds no recognised text: only the placeholder notes and/or
/// the preserved `![原图](…)` image line.
fn ocr_result_is_empty(text: &str) -> bool {
    text.lines().map(str::trim).filter(|l| !l.is_empty()).all(|l| {
        l.starts_with("![原图](")
            || l == "> （未识别出文字，仅保留原图）"
            || l == ocr::OCR_PDF_EMPTY_PLACEHOLDER
    })
}

pub fn h_ocr(app: &App, req: &Request) -> Response {
    let p = double_decode(req.q("p").unwrap_or(""));
    if !Path::new(&p).is_file() {
        return Response::json_status(404, &json!({"error": "文件不存在", "error_code": crate::api_codes::FILE_NOT_FOUND}));
    }
    if let Some(gate) = module_gate(app, "ocr") {
        return gate;
    }
    match ocr::ocr_any(&p) {
        Ok(text) => {
            let fixed = readmd_fix::fix_markdown(&text);
            let (dir, name) = ntpath_split(&p);
            let mut body = json!({
                "content": fixed.text,
                "fixes": fixed.fixes,
                "name": name,
                "dir": dir,
                "source": "ocr",
                "path": p,
            });
            // Nothing recognised: no file is written, and the UI says so.
            let empty = ocr_result_is_empty(&text);
            if empty {
                body["empty"] = json!(true);
                body["note_code"] = json!("ocr_no_text");
            }
            // `save=1`: write `<src>.md` beside the source, honouring `on_exists`.
            if req.q("save") == Some("1") && !empty {
                let out = std::path::PathBuf::from(crate::convert::md_output_path(&p));
                let mode = req.q("on_exists").unwrap_or("skip");
                let target = match mode {
                    "rename" => crate::batch2::free_output_path(&out),
                    _ => out.clone(),
                };
                let (saved, skipped) = if target.exists() && mode != "overwrite" && mode != "rename" {
                    (false, true)
                } else {
                    (crate::convert::write_md(&target.to_string_lossy(), &fixed.text).is_ok(), false)
                };
                body["out"] = json!(target.to_string_lossy());
                body["saved"] = json!(saved);
                body["skipped"] = json!(skipped);
            }
            Response::json(&body)
        }
        Err(e) => {
            let code = match e.error_code {
                ocr::OcrErrorCode::OcrNoEngine => "ocr_no_engine",
                _ => "ocr_failed",
            };
            Response::json_status(500, &json!({"ok": false, "error_code": code}))
        }
    }
}

/// `Handler._api_transcribe` (`readmd.py:2052`).
pub fn h_transcribe(_app: &App, req: &Request) -> Response {
    if req.method != "POST" {
        return Response::json_status(405, &json!({"ok": false, "error_code": "method_not_allowed"}));
    }
    let body = match parse_json_body(req) {
        Ok(value) => value,
        Err(message) => {
            return Response::json_status(400, &json!({"ok": false, "error_code": message}))
        }
    };
    // `_read_json_body` hands back whatever `json.loads` produced; the first
    // `body.get(...)` then raises `AttributeError` for a non-object document,
    // which the trailing `except Exception` reports as `transcribe_failed`.
    if !body.is_object() {
        return Response::json_status(500, &json!({"ok": false, "error_code": "transcribe_failed"}));
    }
    let file_path = str_field(&body, "path").trim().to_string();
    let language = {
        let raw = str_field(&body, "language").trim().to_string();
        if raw.is_empty() {
            None
        } else {
            Some(raw)
        }
    };
    let model = {
        let raw = if body.get("model").is_some() {
            str_field(&body, "model")
        } else {
            "base".to_string()
        };
        raw.trim().to_string()
    };
    if file_path.is_empty() || !Path::new(&file_path).is_file() {
        return Response::json_status(404, &json!({"ok": false, "error_code": "file_not_found"}));
    }
    if !transcribe::is_supported_media(&file_path) {
        return Response::json_status(
            400,
            &json!({"ok": false, "error_code": "unsupported_media_format"}),
        );
    }
    let (text, err) = transcribe::transcribe_to_md(&file_path, language.as_deref(), &model);
    // `if text:` / `err if err else None`: Python tests truthiness, so an empty
    // string is neither a result nor a warning.
    let text = text.filter(|value| !value.is_empty());
    let err = err.filter(|value| !value.is_empty());
    match (text, err) {
        (Some(content), warning) => Response::json(&json!({
            "ok": true,
            "content": content,
            "path": file_path,
            "warning": warning.map(Value::String).unwrap_or(Value::Null),
        })),
        (None, Some(detail)) => Response::json_status(
            422,
            &json!({"ok": false, "error_code": "transcribe_failed", "error_detail": detail}),
        ),
        (None, None) => Response::json_status(500, &json!({"ok": false, "error_code": "transcribe_empty"})),
    }
}

/// `Handler._api_url` (`readmd.py:3373`).
pub fn h_url(app: &App, req: &Request) -> Response {
    let u = double_decode(req.q("u").unwrap_or(""));
    let crawl = req.q("crawl").unwrap_or("0") == "1";
    if u.is_empty() {
        return Response::json_status(400, &json!({"error": "缺少 URL"}));
    }
    if let Some(gate) = module_gate(app, "web") {
        return gate;
    }
    let text = match fetch_one(&u, crawl) {
        Ok(value) => value,
        Err(_) => {
            return Response::json_status(500, &json!({"ok": false, "error_code": "url_fetch_failed"}))
        }
    };
    // `if not text:` covers both `None` and `''`.
    if text.as_deref().map(|s| s.is_empty()).unwrap_or(true) {
        return Response::json(&json!({
            "content": "",
            "name": u,
            "dir": "",
            "source": "url",
            "note": "未能从该网页提取到正文",
        }));
    }
    let fixed = readmd_fix::fix_markdown(&text.unwrap_or_default());
    Response::json(&json!({
        "content": fixed.text,
        "fixes": fixed.fixes,
        "name": u,
        "dir": "",
        "source": "url",
        "path": u,
    }))
}

fn fetch_one(u: &str, crawl: bool) -> Result<Option<String>, WebError> {
    if crawl {
        crawl_url(u)
    } else {
        fetch_url(u, DEFAULT_TIMEOUT_SEC)
    }
}

fn crawl_url(u: &str) -> Result<Option<String>, WebError> {
    crawl(u, 10, DEFAULT_TIMEOUT_SEC)
}

/// `Handler._api_web_extract` (`readmd.py:3393`).
pub fn h_web_extract(app: &App, req: &Request) -> Response {
    if let Some(gate) = web_extract_gate(app) {
        return gate;
    }
    let length = req.body.len();
    let body: Value = if length == 0 {
        return Response::json_status(
            400,
            &json!({"ok": false, "code": "invalid_request", "error": "请求内容为空"}),
        );
    } else if length > MAX_HTML_BYTES {
        return Response::json_status(
            413,
            &json!({"ok": false, "code": "too_large", "error": "渲染后的网页超过 50 MB 限制"}),
        );
    } else {
        match serde_json::from_slice(&req.body) {
            Ok(value) => value,
            Err(_) => {
                return Response::json_status(
                    400,
                    &json!({"ok": false, "code": "invalid_request", "error": "请求格式错误"}),
                )
            }
        }
    };
    let task_id = text_field(&body, "task_id", "");
    let url = text_field(&body, "url", "");
    let mode = {
        let raw = str_field(&body, "mode");
        if raw == "smart" || raw == "full" {
            raw
        } else {
            "smart".to_string()
        }
    };
    // `rendered_html = body.get('html')` / `if rendered_html is not None:`
    let rendered_html: Option<Value> = body.get("html").filter(|v| !v.is_null()).cloned();
    // A truthy non-string markup reaches `BeautifulSoup`, which raises
    // `TypeError: Incoming markup is of an invalid type: 1. ...`; the trailing
    // `except Exception` in `_api_web_extract` reports it through
    // `_send_api_error(500, 'web_extraction_failed', code='internal_error')`.
    if let Some(value) = &rendered_html {
        if !matches!(value, Value::String(_)) && python_truthy(value) {
            return Response::json_status(
                500,
                &json!({"ok": false, "error_code": "web_extraction_failed", "code": "internal_error"}),
            );
        }
    }
    let outcome: Result<Value, WebError> = (|| match rendered_html {
        Some(value) => {
            let html = match &value {
                Value::String(text) => text.clone(),
                // `html or ''` inside `web.extract_html`.
                _ => String::new(),
            };
            let final_url = text_field(&body, "final_url", &url);
            extract_html(
                &final_url,
                &html,
                &mode,
                body.get("readability").filter(|v| python_truthy(v)),
                body.get("defuddle").filter(|v| python_truthy(v)),
                true,
            )
        }
        None => fetch_document(&url, &mode, DEFAULT_TIMEOUT_SEC, &task_id),
    })();
    let mut result = match outcome {
        Ok(value) => value,
        Err(err) => {
            if RENDERABLE_CODES.contains(&err.code.as_str()) {
                let mut payload = err.as_dict();
                if let Some(obj) = payload.as_object_mut() {
                    obj.insert("render_required".to_string(), json!(true));
                    obj.insert("fallback_reason".to_string(), json!(err.code));
                    obj.insert("engine_chain".to_string(), json!(["http"]));
                    obj.insert("attempts".to_string(), json!(1));
                }
                return Response::json(
                    &payload,
                );
            }
            return Response::json_status(err.http_status, &err.as_dict());
        }
    };

    if let Some(previous) = body.get("diagnostics").filter(|v| v.is_object()) {
        if let Some(prior) = previous.get("engine_chain").and_then(|v| v.as_array()) {
            let mut merged: Vec<Value> = prior.iter().take(12).cloned().collect();
            let current: Vec<Value> = result
                .get("engine_chain")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            merged.extend(current);
            if let Some(obj) = result.as_object_mut() {
                obj.insert("engine_chain".to_string(), json!(merged));
            }
        }
        // `min(99, max(0, int(previous.get('attempts') or 0)) + int(result.get('attempts') or 0))`,
        // where a value `int()` rejects skips the whole assignment.
        let prior_attempts = previous.get("attempts").map(python_int_or_zero);
        let current_attempts = match result.get("attempts") {
            Some(value) => python_int_or_zero(value),
            None => Ok(0),
        };
        let mut merged_attempts = None;
        if let (Some(Ok(prior)), Ok(current)) = (prior_attempts, current_attempts) {
            let total = prior.max(0).saturating_add(current).min(99);
            merged_attempts = Some(total);
        }
        if let Some(total) = merged_attempts {
            if let Some(obj) = result.as_object_mut() {
                obj.insert("attempts".to_string(), json!(total));
            }
        }
        if let Some(reason) = previous.get("fallback_reason").filter(|v| python_truthy(v)) {
            let trimmed: String = python_str(reason).chars().take(80).collect();
            if let Some(obj) = result.as_object_mut() {
                obj.insert("fallback_reason".to_string(), json!(trimmed));
            }
        }
    }

    let ok = result.get("ok").map(python_truthy) == Some(true);
    if ok && body.get("download_images").map(python_truthy) == Some(true) {
        let dir_name = if task_id.is_empty() { token_hex(8) } else { task_id.clone() };
        let asset_dir = app.paths.data_dir.join("web-assets").join(dir_name);
        let content = content_of(&result);
        let (rewritten, assets, image_warnings) = localize_images(&content, &asset_dir, &task_id);
        if let Some(obj) = result.as_object_mut() {
            obj.insert("content".to_string(), json!(rewritten));
            obj.insert("assets".to_string(), Value::Array(assets.clone()));
            let mut warnings: Vec<Value> = obj
                .get("warnings")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            warnings.extend(image_warnings.into_iter().map(Value::String));
            obj.insert("warnings".to_string(), Value::Array(warnings));
            obj.insert(
                "asset_dir".to_string(),
                json!(if assets.is_empty() {
                    String::new()
                } else {
                    asset_dir.to_string_lossy().into_owned()
                }),
            );
        }
    }
    Response::json(&result)
}

/// `int(value or 0)` as used by the diagnostics merge.  `Err(())` mirrors
/// CPython raising `TypeError` / `ValueError`, i.e. the caller's
/// `except (TypeError, ValueError): pass` arm that leaves `attempts` alone.
///
/// Saturating arithmetic stands in for Python's unbounded integers: the only
/// consumer clamps the result with `min(99, ...)`, and the previous
/// `prior + current` overflowed (and panicked a debug test thread) on a body
/// carrying `"attempts": 1e300`.
fn python_int_or_zero(value: &Value) -> Result<i64, ()> {
    if !python_truthy(value) {
        return Ok(0);
    }
    match value {
        Value::Bool(b) => Ok(if *b { 1 } else { 0 }),
        Value::Number(n) => Ok(match n.as_i64() {
            Some(i) => i,
            None => n.as_f64().map(|f| f.clamp(-9.0e18, 9.0e18) as i64).unwrap_or(i64::MAX),
        }),
        Value::String(s) => python_int_text(s),
        // `int([1])` / `int({'a': 1})` -> TypeError.
        _ => Err(()),
    }
}

/// CPython's `int(str)`: optional surrounding whitespace, optional sign, then a
/// single `digit (["_"] digit)*` run in base 10.  Digits may come from any
/// Unicode decimal-digit block (`float_probe.txt`: `int('١٢٣') == 123`,
/// `int('١_٠') == 10`), and a literal wider than `i64` saturates instead of
/// failing, because `_api_web_extract` clamps the sum to `0..=99` afterwards.
fn python_int_text(raw: &str) -> Result<i64, ()> {
    let mut ascii = String::with_capacity(raw.len());
    for ch in raw.chars() {
        if ch.is_ascii() {
            ascii.push(ch);
        } else if ch.is_whitespace() {
            ascii.push(' ');
        } else {
            ascii.push(char::from(b'0' + unicode_decimal_digit(ch).ok_or(())? as u8));
        }
    }
    let body = ascii.trim();
    let (sign, digits) = match body.strip_prefix('-') {
        Some(rest) => (-1i64, rest),
        None => (1i64, body.strip_prefix('+').unwrap_or(body)),
    };
    let chars: Vec<char> = digits.chars().collect();
    let mut pos = 0usize;
    if digit_run(&chars, &mut pos) == 0 || pos != chars.len() {
        return Err(());
    }
    let cleaned: String = chars.iter().filter(|c| **c != '_').collect();
    let significant = cleaned.trim_start_matches('0');
    let magnitude: i64 = if significant.is_empty() {
        0
    } else if significant.len() > 18 {
        i64::MAX
    } else {
        significant.parse().map_err(|_| ())?
    };
    Ok(sign.saturating_mul(magnitude))
}

/// `Handler._api_web_cancel` (`readmd.py:3481`).
pub fn h_web_cancel(app: &App, req: &Request) -> Response {
    if let Some(gate) = module_gate(app, "web") {
        return gate;
    }
    let body: Value = if req.body.is_empty() {
        json!({})
    } else {
        match serde_json::from_slice(&req.body) {
            Ok(value) => value,
            Err(_) => {
                return Response::json_status(
                    500,
                    &json!({"ok": false, "error_code": "web_cancel_failed"}),
                )
            }
        }
    };
    if !body.is_object() {
        return Response::json_status(500, &json!({"ok": false, "error_code": "web_cancel_failed"}));
    }
    let task_id = text_field(&body, "task_id", "");
    cancel(&task_id);
    Response::json(&json!({"ok": true}))
}

/// `_read_json_body(limit=1024 * 1024)`'s Content-Length ceiling.
const MAX_JSON_BODY: usize = 1024 * 1024;

/// Python's `_read_json_body` raises `json.JSONDecodeError` (a `ValueError`),
/// which `_api_transcribe` reports as `400 {'error_code': str(exc)}`. The most
/// common message is reproduced verbatim.
///
/// Only a *decode* failure is reported here: `json.loads` accepts any valid
/// document, so a non-object body has to reach the caller, where Python's
/// `body.get(...)` raises `AttributeError` and the handler answers
/// `500 transcribe_failed` rather than `400`.
fn parse_json_body(req: &Request) -> Result<Value, String> {
    // `n = int(headers.get('Content-Length', 0) or 0)` / `if not n: return {}`.
    if req.body.is_empty() {
        return Ok(json!({}));
    }
    // `_read_request_body_limited` rejects the length before reading a byte.
    if req.body.len() > MAX_JSON_BODY {
        return Err("request_too_large".to_string());
    }
    match serde_json::from_slice::<Value>(&req.body) {
        Ok(value) => Ok(value),
        // `json.loads` accepts the literals CPython's decoder defines only;
        // `NaN`, `Infinity` and `-Infinity` are three of them, and serde
        // rejects all three, so they have to be reported as the *documents*
        // Python builds (a float, hence `AttributeError` -> 500 downstream).
        Err(_) => {
            let trimmed: &[u8] = strip_json_ws(&req.body);
            if matches!(trimmed, b"NaN" | b"Infinity" | b"-Infinity") {
                return Ok(Value::Number(serde_json::Number::from_f64(0.0).expect("0.0")));
            }
            Err(python_json_message(&req.body))
        }
    }
}

/// `json.decoder.WHITESPACE` is exactly `' \t\n\r'` — notably *not* `\x0c` and
/// not any Unicode space (`golden_probe.txt`).
fn is_json_whitespace(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r')
}

fn strip_json_ws(body: &[u8]) -> &[u8] {
    let start = body.iter().position(|b| !is_json_whitespace(*b)).unwrap_or(body.len());
    &body[start..]
}

/// `str(json.JSONDecodeError)`, i.e. `"<msg>: line L column C (char P)"`.
///
/// Positions are *character* offsets into the decoded `str` (`[1, "é", ]`
/// reports char 9 while its UTF-8 encoding is longer), which is why the body is
/// walked with `char_indices`.
fn python_json_message(body: &[u8]) -> String {
    let text = String::from_utf8_lossy(body).into_owned();
    if text.starts_with('\u{feff}') {
        return python_json_message_at("Unexpected UTF-8 BOM (decode using utf-8-sig)", &text, 0);
    }
    let offset = text
        .char_indices()
        .find(|(_, c)| !is_json_whitespace(*c as u32 as u8))
        .map(|(i, _)| i)
        .unwrap_or(0);
    python_json_message_at("Expecting value", &text, offset)
}

fn python_json_message_at(kind: &str, text: &str, offset: usize) -> String {
    let before = &text[..offset];
    let line = before.matches('\n').count() + 1;
    let column = before
        .rsplit('\n')
        .next()
        .map(|s| s.chars().count() + 1)
        .unwrap_or(1);
    format!(
        "{}: line {} column {} (char {})",
        kind, line, column, offset
    )
}

// ------------------------------------------------------------------ internals

/// Kept so `main.rs`/`server.rs` can hand the WebView fallback the same body the
/// legacy renderer produced; the kernel reports it as unavailable.
#[allow(dead_code)]
fn degraded_render(url: &str) -> Value {
    hr::render_url_isolated(url, DEFAULT_TIMEOUT_SEC)
}

#[allow(dead_code)]
fn unused_map() -> Map<String, Value> {
    Map::new()
}

#[allow(dead_code)]
fn unused_buf() -> PathBuf {
    PathBuf::new()
}

// ---------------------------------------------------------------------- tests

#[cfg(test)]
#[allow(dead_code)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Every expected value below is measured, never assumed.  The probe
    /// scripts and their raw output live in
    /// `scratch/rust_parity/parity_web_s1/`:
    ///
    /// * `probe_url.py`   -> `url_probe.txt`   (`web.normalize_url`, `urlparse`)
    /// * `probe_retry.py` -> `retry_probe.txt` (`_retry_after_delay`,
    ///   `parsedate_to_datetime`, `urlsplit().port`)
    /// * `probe_golden.py`-> `golden_probe.txt`(`ntpath.split`, `json.loads`,
    ///   `bytes.decode`, `_image_urls`, `str()`, `float()`)
    /// * `charset_probe.txt` (`requests.utils.get_encoding_from_headers`)
    /// * `misc_probe.txt`   (`unquote`, `parse_qs`, `apparent_encoding`)

    fn scratch_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "readmd-parity-web-{}-{}-{}",
            tag,
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }


    fn request(method: &str, path: &str, query: &[(&str, &str)], body: &[u8]) -> Request {
        Request {
            method: method.to_string(),
            path: path.to_string(),
            query: query
                .iter()
                .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                .collect(),
            headers: HashMap::new(),
            body: body.to_vec(),
        }
    }

    fn json_post(body: &Value) -> Request {
        request("POST", "/api/web/extract", &[], serde_json::to_vec(body).unwrap().as_slice())
    }


    fn answered(res: &Response) -> Value {
        serde_json::from_slice(&res.body).expect("handler answered with JSON")
    }

    fn keys_of(res: &Response) -> Vec<String> {
        answered(res)
            .as_object()
            .expect("object body")
            .keys()
            .cloned()
            .collect()
    }

    fn now_epoch() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    }

    // ------------------------------------------------ web.normalize_url (:301)

    /// `url_probe.txt`: the three rejection codes `normalize_url` can produce.
    #[test]
    fn normalize_url_error_codes_match_python() {
        let missing = ["", "   ", "\t\n"];
        for u in missing {
            let err = normalize_url(u).unwrap_err();
            assert_eq!(err.code, "missing_url", "input {:?}", u);
            assert_eq!(err.message, "请输入网页地址");
            assert_eq!(err.http_status, 400);
        }
        let unsupported = [
            "file:///etc/passwd",
            "gopher://x/",
            "ftp://example.com/",
            "FiLe://X/",
        ];
        for u in unsupported {
            let err = normalize_url(u).unwrap_err();
            assert_eq!(err.code, "unsupported_scheme", "input {:?}", u);
            assert_eq!(err.message, "仅支持 HTTP 或 HTTPS 网页");
            assert_eq!(err.http_status, 400);
        }
        let invalid = [
            "http:///",
            "http:///x",
            "https://",
            "//example.com/",
            "http://@/",
            "javascript:alert(1)",
            "data:text/html,x",
            "http://example.com:65536/",
            "http://example.com:100000/",
            "http://example.com:notaport/",
            "http://example.com:99999999999999999999/",
        ];
        for u in invalid {
            let err = normalize_url(u).unwrap_err();
            assert_eq!(err.code, "invalid_url", "input {:?}", u);
            assert_eq!(err.message, "网页地址格式不正确");
            assert_eq!(err.http_status, 400);
        }
    }

    /// `url_probe.txt`: exact normalised output for the accepted shapes.
    #[test]
    fn normalize_url_output_table() {
        let table: &[(&str, &str)] = &[
            ("example.com", "https://example.com/"),
            ("example.com/a?b=1#frag", "https://example.com/a?b=1"),
            ("http://example.com/", "http://example.com/"),
            ("HTTP://EXAMPLE.COM/", "http://example.com/"),
            ("HtTpS://Example.Com/PaTh", "https://example.com/PaTh"),
            ("mailto:a@b.c", "https://b.c/"),
            ("http://user@127.0.0.1/", "http://127.0.0.1/"),
            ("http://user:pass@127.0.0.1:8080/x", "http://127.0.0.1:8080/x"),
            ("http://[::1]/", "http://[::1]/"),
            ("http://[::1]:8080/", "http://[::1]:8080/"),
            ("http://[fe80::1%25eth0]/", "http://[fe80::1%25eth0]/"),
            ("http://2130706433/", "http://2130706433/"),
            ("http://0x7f000001/", "http://0x7f000001/"),
            ("http://2130706433.evil/", "http://2130706433.evil/"),
            ("http://localhost./", "http://localhost./"),
            ("http://LOCALHOST/", "http://localhost/"),
            ("http://example.com:80/", "http://example.com:80/"),
            ("http://example.com:443/", "http://example.com:443/"),
            ("http://example.com:0/", "http://example.com/"),
            ("http://example.com:/", "http://example.com/"),
            ("https://example.com:443", "https://example.com:443/"),
            ("https://example.com/x?a=1#b", "https://example.com/x?a=1"),
            ("https://example.com/#", "https://example.com/"),
            ("https://example.com/a#b?c", "https://example.com/a"),
            ("http://example.com/a;p=1/q", "http://example.com/a;p=1/q"),
            ("  http://example.com/x  ", "http://example.com/x"),
            ("http://example.com//y", "http://example.com//y"),
            ("http://[::1]", "http://[::1]/"),
            ("http://256.256.256.256/", "http://256.256.256.256/"),
            ("http://1.2.3.4.5/", "http://1.2.3.4.5/"),
            ("http://-example.com/", "http://-example.com/"),
            ("http://example.com./x", "http://example.com./x"),
            ("http://xn--r8jz45g.example/", "http://xn--r8jz45g.example/"),
            ("http://example.com/a@b", "http://example.com/a@b"),
        ];
        for (input, want) in table {
            assert_eq!(normalize_url(input).ok().as_deref(), Some(*want), "input {:?}", input);
        }
    }

    /// Two accepted-by-Python shapes that `hr::normalize_url` rejects outright
    /// (whitespace hosts) or normalises differently (no IDNA).  Pinned as the
    /// kernel answers today; see the report's 未对齐 section - the fix belongs
    /// in `headless_renderer.rs::normalize_url`, not here.
    #[test]
    fn normalize_url_known_divergences_are_pinned() {
        // Python: normalize_url("http://a b/") == "http://a b/"  (url_probe.txt)
        assert_eq!(
            normalize_url("http://a b/").unwrap_err().code,
            "invalid_url"
        );
        // Python: normalize_url("http:// /") == "http:// /"
        assert_eq!(
            normalize_url("http:// /").unwrap_err().code,
            "invalid_url"
        );
        // Python: normalize_url("http://例エ.コム/") ==
        //         "http://xn--icku34g.xn--tckwe/"  (idna-encoded)
        assert_eq!(
            normalize_url("http://\u{4f8b}\u{30a8}.\u{30b3}\u{30e0}/").unwrap(),
            "http://\u{4f8b}\u{30a8}.\u{30b3}\u{30e0}/"
        );
        // Python: normalize_url("http://éxample.com/") ==
        //         "http://xn--xample-9ua.com/"
        assert_eq!(
            normalize_url("http://\u{e9}xample.com/").unwrap(),
            "http://\u{e9}xample.com/"
        );
        // Python: urlparse("http://[::1") raises ValueError("Invalid IPv6 URL")
        // rather than a WebError; the kernel reports invalid_url.
        assert_eq!(
            normalize_url("http://[::1").unwrap_err().code,
            "invalid_url"
        );
    }

    // -------------------------------------- _validate_public_url (:318) / port

    /// The guard only ever *resolves* for these five routes: `readmd.py` calls
    /// `fetch_url` / `crawl` / `fetch_document` / `localize_images` /
    /// `fetch_html`, and every one of them keeps `allow_private=True`
    /// (`src/readmd_modules/web.py:124`).  Measured in `url_probe.txt`: Python
    /// normalises and accepts all of these, so the kernel must too.  This is
    /// the documented SSRF surface, not an accident to be "fixed" here.
    #[test]
    fn validate_public_url_admits_private_literals_like_python() {
        let table: &[(&str, &str)] = &[
            ("http://127.0.0.1/", "http://127.0.0.1/"),
            ("http://127.1.1.1/", "http://127.1.1.1/"),
            ("http://0.0.0.0/", "http://0.0.0.0/"),
            ("http://10.0.0.1/", "http://10.0.0.1/"),
            ("http://172.16.0.1/", "http://172.16.0.1/"),
            ("http://172.31.255.255/", "http://172.31.255.255/"),
            ("http://192.168.1.1/", "http://192.168.1.1/"),
            (
                "http://169.254.169.254/latest/meta-data/",
                "http://169.254.169.254/latest/meta-data/",
            ),
            ("http://[::1]/", "http://[::1]/"),
            ("http://[fe80::1]/", "http://[fe80::1]/"),
            ("http://[::ffff:127.0.0.1]/", "http://[::ffff:127.0.0.1]/"),
            ("http://user@127.0.0.1/", "http://127.0.0.1/"),
            ("http://127.0.0.1:8080/x", "http://127.0.0.1:8080/x"),
            ("127.0.0.1", "https://127.0.0.1/"),
        ];
        for (input, want) in table {
            match validate_public_url(input) {
                Ok(got) => assert_eq!(got, *want, "input {:?}", input),
                Err(err) => panic!(
                    "input {:?} rejected as {} ({}) but Python accepts it",
                    input, err.code, err.detail
                ),
            }
        }
    }

    /// Rejections that happen before any resolver is touched, so they are
    /// deterministic offline.
    #[test]
    fn validate_public_url_rejects_before_dns() {
        for input in ["", "file:///etc/passwd", "http:///", "javascript:alert(1)"] {
            let err = validate_public_url(input).unwrap_err();
            assert!(
                matches!(err.code.as_str(), "missing_url" | "unsupported_scheme" | "invalid_url"),
                "input {:?} -> {}",
                input,
                err.code
            );
            assert_eq!(err.http_status, 400);
        }
    }

    /// `_validate_public_url` answers `dns_failed`/502 when the host will not
    /// resolve; `readmd.py` maps that onto `{'ok': False, 'error_code':
    /// 'url_fetch_failed'}` for `/api/url` but returns the `WebError` dict for
    /// `/api/web/extract`.
    #[test]
    fn validate_public_url_dns_failure_shape() {
        let err = validate_public_url("http://readmd-parity-nonexistent.invalid/");
        match err {
            Ok(v) => panic!("host resolved: {}", v),
            Err(e) => {
                assert_eq!(e.code, "dns_failed");
                assert_eq!(e.message, "无法解析网页域名");
                assert_eq!(e.http_status, 502);
            }
        }
    }

    /// `url_probe.txt` / `retry_probe.txt`: `url_port` must agree with
    /// `urllib.parse.urlsplit(url).port` for the strings this module actually
    /// feeds it (normalised URLs).
    #[test]
    fn url_port_matches_urlsplit_port() {
        let table: &[(&str, Option<u16>)] = &[
            ("http://example.com/", None),
            ("http://example.com:80/", Some(80)),
            ("http://example.com:443/", Some(443)),
            ("https://example.com:443/", Some(443)),
            ("http://example.com:8080/x", Some(8080)),
            ("http://example.com:8080", Some(8080)),
            ("http://example.com:08080/", Some(8080)),
            ("http://example.com:65535/", Some(65535)),
            ("http://[::1]/", None),
            ("http://[::1]:8080/", Some(8080)),
            ("http://[::ffff:127.0.0.1]:9/", Some(9)),
            ("http://127.0.0.1:8080/x", Some(8080)),
            ("http://user:pass@127.0.0.1:8080/x", Some(8080)),
            ("http://example.com:8080/?q=1:2", Some(8080)),
            ("", None),
            ("example.com:8080", None),
        ];
        for (input, want) in table {
            assert_eq!(&url_port(input), want, "input {:?}", input);
        }
        // Python: urlsplit("http://a:65536/").port raises ValueError and
        // urlsplit("http://a:-1/").port raises too, while the kernel's
        // `u16::parse` just answers None.  Unreachable through
        // `validate_public_url` (the URL is normalised first).
        assert_eq!(url_port("http://a:65536/"), None);
        assert_eq!(url_port("http://a:-1/"), None);
        assert_eq!(url_port("http://a:x/"), None);
    }

    // ------------------------------------------------- _retry_after_delay (:345)

    /// `retry_probe.txt`: every delta-seconds spelling CPython's `float()`
    /// accepts (or rejects) and the 30s clamp that follows `max(0.0, delay)`.
    #[test]
    fn retry_after_delay_delta_table() {
        let table: &[(&str, f64)] = &[
            ("", 0.0),
            ("0", 0.0),
            ("1", 1.0),
            ("12", 12.0),
            ("3", 3.0),
            ("29", 29.0),
            ("30", 30.0),
            ("31", 30.0),
            ("100", 30.0),
            ("-1", 0.0),
            ("-0.5", 0.0),
            ("1.5", 1.5),
            (".5", 0.5),
            ("5.", 5.0),
            ("  7  ", 7.0),
            ("1e2", 30.0),
            ("1E-2", 0.01),
            ("+5", 5.0),
            ("inf", 30.0),
            ("-inf", 0.0),
            ("nan", 0.0),
            ("NaN", 0.0),
            ("Infinity", 30.0),
            ("0x10", 0.0),
            ("1,5", 0.0),
            ("5s", 0.0),
            ("abc", 0.0),
            ("\t9\n", 9.0),
            ("1.2.3", 0.0),
            ("true", 0.0),
            ("None", 0.0),
            ("1e999", 30.0),
            ("-1e999", 0.0),
            ("1.5e3", 30.0),
        ];
        for (raw, want) in table {
            assert_eq!(&retry_after_delay(Some(raw)), want, "input {:?}", raw);
        }
        assert_eq!(retry_after_delay(None), 0.0);
        assert_eq!(retry_after_delay(Some("   ")), 0.0);
    }

    /// CPython's `float()` also accepts digit-group underscores and any Unicode
    /// decimal digit (`golden_probe.txt`: `float('1_0') == 10.0`,
    /// `float('٣') == 3.0`, `float('1٠') == 10.0`), and rejects `1__0`, `_1`,
    /// `1_`, `1_.0`, `1e_1`, `½`.
    #[test]
    fn retry_after_delay_matches_python_float_extras() {
        let table: &[(&str, f64)] = &[
            ("1_0", 10.0),
            ("1_000", 30.0),
            ("1_0.0", 10.0),
            ("1_0e1_0", 30.0),
            ("\u{663}", 3.0),
            ("1\u{660}", 10.0),
            ("\u{661}\u{662}\u{663}", 30.0),
            ("1__0", 0.0),
            ("_1", 0.0),
            ("1_", 0.0),
            ("1_.0", 0.0),
            ("1e_1", 0.0),
            ("\u{bd}", 0.0),
            ("+_1", 0.0),
            ("0b1", 0.0),
        ];
        for (raw, want) in table {
            assert_eq!(&retry_after_delay(Some(raw)), want, "input {:?}", raw);
        }
    }

    /// The clamp happens *after* the lower bound, exactly like
    /// `min(MAX_RETRY_AFTER, max(0.0, delay))`.
    #[test]
    fn retry_after_delay_clamp_is_after_the_lower_bound() {
        assert_eq!(retry_after_delay(Some("-1000000")), 0.0);
        assert_eq!(retry_after_delay(Some("1000000")), MAX_RETRY_AFTER);
        assert_eq!(MAX_RETRY_AFTER, 30.0);
        // A date in the far past clamps to 0.0, a date in the far future to 30.
        assert_eq!(
            retry_after_delay(Some("Sun, 06 Nov 1994 08:49:37 GMT")),
            0.0
        );
        assert_eq!(
            retry_after_delay(Some("Sat, 01 Jan 2095 00:00:00 GMT")),
            30.0
        );
    }

    // -------------------------------------------------- http_date_seconds (:364)

    /// `retry_probe.txt`: `email.utils.parsedate_to_datetime` epochs.  The
    /// kernel returns `target - now`, so the derived target is compared with a
    /// two-second slack for the clock read inside the function.
    #[test]
    fn http_date_seconds_matches_parsedate_epochs() {
        let table: &[(&str, f64)] = &[
            ("Sun, 06 Nov 1994 08:49:37 GMT", 784_111_777.0),
            ("Sat, 01 Jan 2095 00:00:00 GMT", 3_944_678_400.0),
            // Non-GMT zone names must shift the instant, not be ignored.
            ("Sat, 01 Jan 2095 00:00:00 EST", 3_944_696_400.0),
            ("Sat, 01 Jan 2095 00:00:00 -0500", 3_944_696_400.0),
            ("Sun, 06 Nov 1994 08:49:37 +1400", 784_061_377.0),
            // Zone-less dates are assumed UTC by web.py, not local time.
            ("Sat, 01 Jan 2095 00:00:00", 3_944_678_400.0),
            ("Fri, 29 Feb 2024 12:00:00 GMT", 1_709_208_000.0),
            ("Tue, 29 Feb 2000 00:00:00 GMT", 951_782_400.0),
            // The weekday name is not validated by CPython.
            ("Xyz, 06 Nov 1994 08:49:37 GMT", 784_111_777.0),
            (", 06 Nov 1994 08:49:37 GMT", 784_111_777.0),
            // A missing weekday is still accepted by parsedate_tz.
            ("06 Nov 1994 08:49:37 GMT", 784_111_777.0),
            ("Sun, 6 Nov 1994 8:49:37 GMT", 784_111_777.0),
            // Long month names are accepted (first three letters).
            ("Sun, 06 November 1994 08:49:37 GMT", 784_111_777.0),
            ("Sun, 06 Nov 1994 08:49:37 GMT extra", 784_111_777.0),
            ("  Sun, 06 Nov 1994 08:49:37 GMT  ", 784_111_777.0),
            ("Sun, 06 Nov 1994 08:49:37 UT", 784_111_777.0),
            ("Sun, 06 Nov 1994 08:49:37 UTC", 784_111_777.0),
            ("Sun, 06 Nov 1994 08:49:37 Z", 784_111_777.0),
        ];
        for (raw, want) in table {
            let before = now_epoch() as f64;
            let delta = http_date_seconds(raw)
                .unwrap_or_else(|| panic!("http_date_seconds({:?}) -> None", raw));
            let derived = before + delta;
            assert!(
                (derived - want).abs() <= 2.0,
                "input {:?} -> target {:?}, CPython parsedate -> {:?}",
                raw,
                derived,
                want
            );
        }
    }

    /// Every month abbreviation resolves to the epoch CPython computes.
    #[test]
    fn http_date_seconds_every_month_name() {
        // Measured in `months_probe.txt`; the earlier guesses were wrong for
        // every month except November.
        let months = [
            ("Jan", 757_846_177.0),
            ("Feb", 760_524_577.0),
            ("Mar", 762_943_777.0),
            ("Apr", 765_622_177.0),
            ("May", 768_214_177.0),
            ("Jun", 770_892_577.0),
            ("Jul", 773_484_577.0),
            ("Aug", 776_162_977.0),
            ("Sep", 778_841_377.0),
            ("Oct", 781_433_377.0),
            ("Nov", 784_111_777.0),
            ("Dec", 786_703_777.0),
        ];
        let base = now_epoch() as f64;
        for (name, want) in months {
            let raw = format!("Sun, 06 {} 1994 08:49:37 GMT", name);
            let delta = http_date_seconds(&raw)
                .unwrap_or_else(|| panic!("{} -> None", name));
            assert!(
                (base + delta - want).abs() <= 2.0,
                "{} -> {:?}",
                name,
                base + delta
            );
            // Case is folded by CPython's month table.
            let upper = format!("sun, 06 {} 1994 08:49:37 gmt", name.to_uppercase());
            assert!(http_date_seconds(&upper).is_some(), "{}", upper);
        }
    }

    /// Dates CPython *rejects* (`ValueError` from `parsedate_to_datetime`, hence
    /// a 0.0 delay) must not produce a usable timestamp either.
    #[test]
    fn http_date_seconds_rejects_invalid_dates() {
        let bad = [
            "Sat, 29 Feb 2023 12:00:00 GMT",
            "Sun, 00 Nov 1994 08:49:37 GMT",
            "Sun, 32 Nov 1994 08:49:37 GMT",
            "Sun, 31 Apr 1994 08:49:37 GMT",
            "Sun, 06 Nov 1994 25:00:00 GMT",
            "Sun, 06 Nov 1994 08:61:00 GMT",
            "Sun, 06 Nov 1994 08:49:60 GMT",
            "Sun, 06 Nbv 1994 08:49:37 GMT",
            "Sun, 06 Xyz 1994 08:49:37 GMT",
            "Sun,06Nov1994 08:49:37 GMT",
            "Sun, 06 Nov -1994 08:49:37 GMT",
            "not a date",
            // "12" is *not* here: parsedate_to_datetime rejects it, but
            // _retry_after_delay already succeeded at float("12") == 12.0, so
            // the delta table pins that value instead.
            "-",
            "",
        ];
        for raw in bad {
            assert!(
                http_date_seconds(raw).is_none(),
                "http_date_seconds({:?}) = {:?}, CPython raises ValueError",
                raw,
                http_date_seconds(raw)
            );
            assert_eq!(retry_after_delay(Some(raw)), 0.0, "input {:?}", raw);
        }
    }

    /// `days_from_civil` against `datetime.date(...).toordinal() - 719163`
    /// (measured in `golden_days.txt`).
    #[test]
    fn days_from_civil_matches_python() {
        let table: &[(i64, u32, u32, i64)] = &[
            (1970, 1, 1, 0),
            (1970, 1, 2, 1),
            (1969, 12, 31, -1),
            (1994, 11, 6, 9075),
            (2000, 2, 29, 11016),
            (2024, 2, 29, 19782),
            (2023, 2, 28, 19416),
            (2100, 3, 1, 47541),
            (1900, 3, 1, -25508),
            (1900, 2, 28, -25509),
            (1, 2, 28, -719104),
            (1582, 10, 15, -141427),
            (1582, 10, 4, -141438),
            (2095, 1, 1, 45656),
            (2095, 12, 31, 46020),
            (1, 1, 1, -719162),
            (2024, 12, 31, 20088),
            (1969, 2, 28, -307),
            (1972, 2, 29, 789),
            (9999, 12, 31, 2932896),
        ];
        for (y, m, d, want) in table {
            assert_eq!(
                &days_from_civil(*y, *m, *d),
                want,
                "days_from_civil({}, {}, {})",
                y,
                m,
                d
            );
        }
    }
}
