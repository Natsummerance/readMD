//! HTTP/1.1 kernel: hand-rolled server, static asset host, and the `/api/*`
//! contract the shipped ReadMD frontend speaks.

use crate::batch2;
use crate::error::{ApiError, ApiResult};
use crate::{
    ai_providers, content, parity_code, parity_diagram, parity_pets, parity_web, paths,
    skill_import, App,
};
use crate::skill_import::JVal;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU16, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const SERVER_TAG: &str = "readmd-rust";
const MAX_BODY: usize = 32 * 1024 * 1024;
const MAX_HEADER: usize = 64 * 1024;

static BOUND_PORT: AtomicU16 = AtomicU16::new(0);

pub fn bound_port() -> u16 {
    BOUND_PORT.load(Ordering::Relaxed)
}

// ---------------------------------------------------------------- transport

#[derive(Debug)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub query: HashMap<String, String>,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl Request {
    pub fn q(&self, key: &str) -> Option<&str> {
        self.query.get(key).map(|s| s.as_str())
    }

    pub fn header(&self, key: &str) -> Option<&str> {
        self.headers.get(&key.to_ascii_lowercase()).map(|s| s.as_str())
    }

    pub fn wants_keep_alive(&self) -> bool {
        match self.header("connection") {
            Some(v) => !v.to_ascii_lowercase().contains("close"),
            None => !self.is_http10(),
        }
    }

    pub fn is_http10(&self) -> bool {
        self.headers.get("__version__").map(|v| v == "HTTP/1.0").unwrap_or(false)
    }

    pub fn body_text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    /// Parse the body as a JSON object; empty bodies become `{}`.
    pub fn json(&self) -> ApiResult<Value> {
        if self.body.iter().all(|b| b.is_ascii_whitespace()) {
            return Ok(json!({}));
        }
        serde_json::from_slice(&self.body)
            .map_err(|e| ApiError::bad_request("invalid_json").noted("detail", e.to_string()))
    }

    /// Legacy spelling of [`param`]: query string first, then the JSON body.
    pub fn field(&self, key: &str) -> Option<String> {
        param(self, &body_value(self), key)
    }
}

fn value_to_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

// ------------------------------------------------------- parameter layer
//
// `readmd.py` reads request parameters in two disconnected styles: the GET
// branches of `_route()` use `parse_qs` (`qs.get('p', [''])[0]`, plus
// `unquote`), while the POST branches parse the JSON body once and use
// `body.get('path') or body.get('file') or ''`.  A Rust handler that copies
// only one style breaks against the other client, so every P1 route reads
// through this layer instead.
//
// Precedence is query first, body second, and a blank query value counts as
// absent: `parse_qs` defaults to `keep_blank_values=False`, so `?p=` never
// lands in `qs` and the legacy handlers see `''`.

/// One parameter, from the query string or — when absent or blank — the JSON
/// body.  Numbers and booleans are stringified, `null`, arrays and objects map
/// to `None`, matching Python's falsy handling of a missing `body.get(key)`.
pub fn param(req: &Request, body: &Value, key: &str) -> Option<String> {
    if let Some(v) = req.q(key) {
        if !v.is_empty() {
            return Some(v.to_string());
        }
    }
    body.get(key).and_then(value_to_string).filter(|s| !s.is_empty())
}

/// `qs.get(key, [fallback])[0]`: the raw value, or `fallback` when the key is
/// absent or blank.  Differs from [`param`] only in that an existing value is
/// returned verbatim, which is what the legacy `== '1'` comparisons need.
pub fn param_or(req: &Request, body: &Value, key: &str, fallback: &str) -> String {
    param(req, body, key).unwrap_or_else(|| fallback.to_string())
}

/// Python's `a or b or c` chains over either source, e.g.
/// `qs.get('name', [''])[0] or qs.get('filename', [''])[0]`.
pub fn param_first(req: &Request, body: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|k| param(req, body, k))
}

/// `qs.get(key, ['0'])[0] == '1'`, extended to the JSON boolean a body client
/// would send.
pub fn param_flag(req: &Request, body: &Value, key: &str) -> bool {
    if let Some(v) = req.q(key) {
        return matches!(v, "1" | "true" | "yes" | "on");
    }
    match body.get(key) {
        Some(Value::Bool(b)) => *b,
        Some(other) => value_to_string(other)
            .map(|v| matches!(v.as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or(false),
        None => false,
    }
}

/// Whether the client named this key at all, either source.
pub fn param_present(req: &Request, body: &Value, key: &str) -> bool {
    req.q(key).map(|v| !v.is_empty()).unwrap_or(false)
        || body.get(key).and_then(value_to_string).map(|s| !s.is_empty()).unwrap_or(false)
}

/// The body as JSON, or `Value::Null` when absent or unparseable.  Callers that
/// must answer `400 无效请求` use `req.json()` and map the error instead.
pub fn body_value(req: &Request) -> Value {
    req.json().unwrap_or(Value::Null)
}

/// Parsed-body cache for handlers that read several keys from one request.
pub struct Params {
    body: Value,
    invalid: bool,
}

impl Params {
    pub fn new(req: &Request) -> Params {
        let body = req.json();
        let invalid = body.is_err();
        Params { body: body.unwrap_or(Value::Null), invalid }
    }

    /// True when a body was sent but was not valid JSON — the state Python's
    /// `json.loads(...)` raises on.
    pub fn body_invalid(&self) -> bool {
        self.invalid
    }

    /// Raw body value, for list/object fields such as `paths`.
    pub fn body(&self) -> &Value {
        &self.body
    }

    pub fn get(&self, key: &str) -> Option<String> {
        self.body.get(key).and_then(value_to_string)
    }

    pub fn param(&self, req: &Request, key: &str) -> Option<String> {
        param(req, &self.body, key)
    }

    pub fn param_or(&self, req: &Request, key: &str, fallback: &str) -> String {
        param_or(req, &self.body, key, fallback)
    }

    pub fn param_first(&self, req: &Request, keys: &[&str]) -> Option<String> {
        param_first(req, &self.body, keys)
    }

    pub fn flag(&self, req: &Request, key: &str) -> bool {
        param_flag(req, &self.body, key)
    }
}

pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Response {
    pub fn json(value: &Value) -> Response {
        Response::json_status(200, value)
    }

    pub fn json_status(status: u16, value: &Value) -> Response {
        Response {
            status,
            headers: vec![(
                "Content-Type".into(),
                "application/json; charset=utf-8".into(),
            )],
            body: serde_json::to_vec(value).unwrap_or_else(|_| b"{}".to_vec()),
        }
    }

    pub fn json_serde<T: serde::Serialize>(value: &T) -> Response {
        Response::json_serde_status(200, value)
    }

    pub fn json_serde_status<T: serde::Serialize>(status: u16, value: &T) -> Response {
        let body = serde_json::to_vec(value).unwrap_or_else(|_| b"{}".to_vec());
        Response {
            status,
            headers: vec![(
                "Content-Type".into(),
                "application/json; charset=utf-8".into(),
            )],
            body,
        }
    }

    pub fn text(status: u16, text: &str) -> Response {
        Response {
            status,
            headers: vec![(
                "Content-Type".into(),
                "text/plain; charset=utf-8".into(),
            )],
            body: text.as_bytes().to_vec(),
        }
    }

    pub fn bytes(status: u16, ctype: &str, body: Vec<u8>) -> Response {
        Response {
            status,
            headers: vec![(
                "Content-Type".into(),
                format!("{ctype}; charset=utf-8"),
            )],
            body,
        }
    }

    pub fn raw_bytes(status: u16, ctype: &str, body: Vec<u8>) -> Response {
        Response {
            status,
            headers: vec![("Content-Type".into(), ctype.to_string())],
            body,
        }
    }

    pub fn header(mut self, key: &str, value: &str) -> Response {
        self.headers.push((key.to_string(), value.to_string()));
        self
    }
}

/// `urllib.parse.parse_qs(query)` with the default `keep_blank_values=False`:
/// blank values are dropped and the first occurrence of a repeated key wins
/// (handlers read `qs[key][0]`).
fn parse_query(raw: &str) -> HashMap<String, String> {
    let mut map: HashMap<String, String> = HashMap::new();
    for pair in raw.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        let key = percent_encoding::percent_decode_str(k)
            .decode_utf8_lossy()
            .into_owned();
        let value = percent_encoding::percent_decode_str(&v.replace('+', " "))
            .decode_utf8_lossy()
            .into_owned();
        if value.is_empty() {
            continue;
        }
        map.entry(key).or_insert(value);
    }
    map
}

fn read_request(reader: &mut BufReader<TcpStream>) -> std::io::Result<Option<Request>> {
    let mut line = Vec::new();
    let got = reader.read_until(b'\n', &mut line)?;
    if got == 0 {
        return Ok(None);
    }
    let request_line = String::from_utf8_lossy(&line).trim_end_matches(['\r', '\n']).to_string();
    let mut headers: HashMap<String, String> = HashMap::new();
    let mut total = line.len();
    loop {
        let mut h = Vec::new();
        let n = reader.read_until(b'\n', &mut h)?;
        total += n;
        if total > MAX_HEADER {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "headers too large"));
        }
        if n == 0 {
            return Ok(None);
        }
        let text = String::from_utf8_lossy(&h).trim_end_matches(['\r', '\n']).to_string();
        if text.is_empty() {
            break;
        }
        if let Some((k, v)) = text.split_once(':') {
            headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
    }
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("GET").to_ascii_uppercase();
    let target = parts.next().unwrap_or("/");
    let version = parts.next().unwrap_or("HTTP/1.1").to_string();
    headers.insert("__version__".into(), version);
    let (raw_path, raw_query) = target.split_once('?').unwrap_or((target, ""));
    let path = percent_encoding::percent_decode_str(raw_path)
        .decode_utf8_lossy()
        .into_owned();
    let query = parse_query(raw_query);

    let body = read_body(reader, &headers)?;
    Ok(Some(Request {
        method,
        path,
        query,
        headers,
        body,
    }))
}

fn read_body(reader: &mut BufReader<TcpStream>, headers: &HashMap<String, String>) -> std::io::Result<Vec<u8>> {
    let chunked = headers
        .get("transfer-encoding")
        .map(|v| v.to_ascii_lowercase().contains("chunked"))
        .unwrap_or(false);
    if chunked {
        let mut out = Vec::new();
        loop {
            let mut size_line = Vec::new();
            reader.read_until(b'\n', &mut size_line)?;
            let size_text = String::from_utf8_lossy(&size_line)
                .trim_end_matches(['\r', '\n'])
                .split(';')
                .next()
                .unwrap_or("0")
                .trim()
                .to_string();
            let size = usize::from_str_radix(&size_text, 16).unwrap_or(0);
            if size == 0 {
                let mut trailer = Vec::new();
                reader.read_until(b'\n', &mut trailer)?;
                break;
            }
            if out.len() + size > MAX_BODY {
                return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "body too large"));
            }
            let mut chunk = vec![0u8; size + 2];
            reader.read_exact(&mut chunk)?;
            out.extend_from_slice(&chunk[..size]);
        }
        return Ok(out);
    }
    let len: usize = headers
        .get("content-length")
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0);
    if len > MAX_BODY {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "body too large"));
    }
    if len == 0 {
        return Ok(Vec::new());
    }
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf)?;
    Ok(buf)
}

fn write_response(stream: &mut TcpStream, res: &Response, keep_alive: bool, head_only: bool) -> std::io::Result<()> {
    let mut out = Vec::with_capacity(res.body.len() + 256);
    out.extend_from_slice(
        format!(
            "HTTP/1.1 {} {}\r\n",
            res.status,
            reason(res.status)
        )
        .as_bytes(),
    );
    let mut has_len = false;
    for (k, v) in &res.headers {
        if k.eq_ignore_ascii_case("content-length") {
            has_len = true;
            continue;
        }
        out.extend_from_slice(format!("{k}: {v}\r\n").as_bytes());
    }
    if !has_len {
        out.extend_from_slice(format!("Content-Length: {}\r\n", res.body.len()).as_bytes());
    }
    out.extend_from_slice(format!("Connection: {}\r\n", if keep_alive { "keep-alive" } else { "close" }).as_bytes());
    out.extend_from_slice(format!("X-ReadMD-Engine: {SERVER_TAG}/{VERSION}\r\n").as_bytes());
    out.extend_from_slice(b"\r\n");
    if !head_only {
        out.extend_from_slice(&res.body);
    }
    stream.write_all(&out)?;
    stream.flush()
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        304 => "Not Modified",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        409 => "Conflict",
        411 => "Length Required",
        412 => "Precondition Failed",
        413 => "Content Too Large",
        415 => "Unsupported Media Type",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        507 => "Insufficient Storage",
        _ => "Unknown",
    }
}

/// Bind and serve from a background thread; returns the actual port.
pub fn spawn(app: Arc<App>, host: &str, port: u16) -> std::io::Result<u16> {
    let listener = TcpListener::bind(socket_addr(host, port)?)?;
    let actual = listener.local_addr()?.port();
    publish_instance(&app, actual);
    std::thread::Builder::new()
        .name("readmd-accept".into())
        .spawn(move || accept_loop(listener, app))?;
    Ok(actual)
}

/// `readmd.py:6494` — `_write_instance` runs only when the server owns the
/// control port, which is what makes single-instance control and `/api/ping?t=`
/// possible for a second process.
fn publish_instance(app: &Arc<App>, port: u16) {
    BOUND_PORT.store(port, Ordering::Relaxed);
    if port == CONTROL_PORT {
        write_instance(app, port);
    }
}

fn socket_addr(host: &str, port: u16) -> std::io::Result<SocketAddr> {
    let parsed: std::net::SocketAddr = match format!("{host}:{port}").parse() {
        Ok(addr) => addr,
        Err(_) => SocketAddr::from(([127, 0, 0, 1], port)),
    };
    Ok(parsed)
}

pub fn serve_forever(app: Arc<App>, host: &str, port: u16) -> std::io::Result<u16> {
    let listener = TcpListener::bind(socket_addr(host, port)?)?;
    let actual = listener.local_addr()?.port();
    publish_instance(&app, actual);
    accept_loop(listener, app);
    Ok(actual)
}

fn accept_loop(listener: TcpListener, app: Arc<App>) {
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let app = app.clone();
                if std::thread::Builder::new()
                    .name("readmd-conn".into())
                    .spawn(move || handle_connection(stream, app))
                    .is_err()
                {
                    continue;
                }
            }
            Err(_) => continue,
        }
    }
}

fn handle_connection(stream: TcpStream, app: Arc<App>) {
    let _ = stream.set_nodelay(true);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(60)));
    let peer_is_loopback = stream
        .peer_addr()
        .map(|a| a.ip().is_loopback())
        .unwrap_or(false);
    let cloned = match stream.try_clone() {
        Ok(c) => c,
        Err(_) => return,
    };
    let mut reader = BufReader::new(cloned);
    let mut writer = stream;
    loop {
        let req = match read_request(&mut reader) {
            Ok(Some(req)) => req,
            _ => break,
        };
        let mut keep_alive = req.wants_keep_alive() && reader.buffer().len() < MAX_HEADER;
        let head_only = req.method == "HEAD";
        let res = match dispatch_guarded(&app, &req, peer_is_loopback) {
            Ok(res) => res,
            Err(res) => {
                // A handler panicked: its shared state is suspect, so answer and
                // close instead of reusing the connection.
                keep_alive = false;
                res
            }
        };
        if write_response(&mut writer, &res, keep_alive, head_only).is_err() {
            break;
        }
        if !keep_alive {
            break;
        }
    }
}

/// Run [`dispatch`] with panic isolation.
///
/// A panicking handler used to unwind straight out of the connection thread, so
/// the client only ever saw a dropped socket.  Now the panic is logged (route and
/// message only, never the request body) and turned into a structured
/// `500 {ok:false, error_code:"internal_error"}`.  `Err` tells the caller to
/// close the connection after writing the response.
fn dispatch_guarded(app: &Arc<App>, req: &Request, peer_is_loopback: bool) -> Result<Response, Response> {
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        dispatch(app, req, peer_is_loopback)
    }));
    match outcome {
        Ok(res) => Ok(res),
        Err(payload) => {
            let msg = panic_message(&*payload);
            log::error!("handler panicked on {} {}: {}", req.method, req.path, msg);
            Err(internal_error_response())
        }
    }
}

/// Best-effort text of a panic payload (`&str` or `String`).
pub(crate) fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic".to_string()
    }
}

/// The structured response sent in place of a panicked handler's output.
pub(crate) fn internal_error_response() -> Response {
    Response::json_status(
        500,
        &json!({"ok": false, "error_code": "internal_error", "error": "internal error"}),
    )
}

// ------------------------------------------------------------------- routing

type Handler = fn(&Arc<App>, &Request) -> ApiResult<Response>;

fn table() -> &'static HashMap<&'static str, Handler> {
    static TABLE: OnceLock<HashMap<&'static str, Handler>> = OnceLock::new();
    TABLE.get_or_init(|| {
        ROUTES.iter().copied().collect()
    })
}

/// The kernel's own route table.
///
/// Every row here must correspond to a path `readmd.py:Handler._route()`
/// (`readmd.py:1186`-`1378`) recognizes, with the sole exception of the five
/// bridge-private rows marked `KERNEL BRIDGE` below.  Each of those five
/// handlers — [`h_kernel_status`], [`h_rename`], [`h_settings`],
/// [`h_file_save_fixed`] and [`h_export`] — carries its own
/// `KERNEL BRIDGE — not a parity route` doc comment spelling out the `Api`
/// method it stands in for and why it is explicitly **outside** the parity
/// surface.  Wave B removed ten
/// invented rows — `/api/tree`, `/api/render`, `/api/document/create`,
/// `/api/delete`, `/api/search`, `/api/stats`, `/api/wordcount`,
/// `/api/links/extract`, `/api/pin` and the duplicate `/api/settings/get` —
/// because `readmd.py` answers `404 text/plain "not found"` for all of them
/// while the kernel served a `200`; `/api/pin` was additionally a **GET that
/// mutated state**.  Do not re-add them: a route Python 404s is a parity break
/// by definition.  Their handler functions were deliberately left in place as
/// dead code for a later wave to delete.
///
/// Wave E1 removed one more row of exactly that class: `/api/settings/save`.
/// `readmd.py` has no such branch, and no consumer asks for it — `grep -rn
/// settings/save` over `assets/`, `readmd.py` and `src/` is empty, and the
/// injected `main.rs` shim's `save_settings` posts to `/api/settings`, which is
/// already a bridge row.  `h_settings_save` stays behind as dead code on purpose,
/// the same way Wave B left its ten handlers.
pub const ROUTES: &[(&str, Handler)] = &[
    ("/api/ping", h_ping),
    ("/api/kernel/status", h_kernel_status), // KERNEL BRIDGE — non-parity, see its doc comment
    ("/api/file", h_file),
    ("/api/list", h_list),
    ("/raw", h_raw),
    ("/api/save", h_save),
    ("/api/upload", h_upload),
    ("/api/rename", h_rename), // KERNEL BRIDGE — non-parity, see its doc comment
    ("/api/recent/status", h_recent_status),
    ("/api/recent/add", h_recent_add),
    ("/api/recent/remove", h_recent_remove),
    ("/api/recent/clear", h_recent_clear),
    ("/api/links/index", h_links_index),
    ("/api/links/graph", h_links_graph),
    ("/api/links/backlinks", h_links_backlinks),
    ("/api/links/deadlinks", h_links_deadlinks),
    ("/api/settings", h_settings), // KERNEL BRIDGE — non-parity, see its doc comment
    // `/api/settings/save` was removed in Wave E1 (F2): `readmd.py:_route()` has
    // no such branch and no asset, module or shim asks for it, so the row only
    // made the kernel answer `200` where Python answers `404 not found`.  The
    // pywebview `Api.save_settings` it seemed to stand in for
    // (`readmd.py:5792`) is served by the `/api/settings` bridge row above;
    // `h_settings_save` is deliberately left behind as dead code.
    ("/api/style/get", h_style_get),
    ("/api/style/save", h_style_save),
    ("/api/system/language", h_language),
    ("/api/autostart/get", h_autostart_get),
    ("/api/autostart/set", h_autostart_set),
    ("/api/modules", h_modules),
    ("/api/skills", h_skills),
    ("/api/pets", parity_pets::h_pets),
    ("/api/pets/status", parity_pets::h_pets_status),
    ("/api/plugins/list", h_plugins_list),
    ("/api/plugins/toggle", crate::plugin_manager::h_plugins_toggle),
    ("/api/ai/config", h_ai_config),
    ("/api/ai/models", h_ai_models),
    ("/api/ai/chat", h_ai_chat),
    ("/api/ai/history", h_ai_history),
    ("/api/ai/prompts", h_ai_prompts),
    ("/api/image/save", h_image_save),
    ("/api/url", h_url_parity), // `readmd.py:3373` via `parity_web`
    ("/api/web/extract", h_web_extract_parity), // `readmd.py:3393` via `parity_web`
    ("/api/web/cancel", h_web_cancel_parity), // `readmd.py:3481` via `parity_web`
    ("/api/bibtex", h_bibtex),
    ("/api/diagram/capabilities", parity_diagram::h_diagram_capabilities),
    ("/api/control/open", h_control_open),
    ("/api/control/next", h_control_next),
    ("/api/control/pet-batch", h_control_pet_batch),
    ("/api/control/pet-menu", h_control_pet_menu),
    ("/api/import/process", h_import_process),
    ("/api/modules/load", h_modules_load),
    ("/api/upstream-sources", h_upstream_sources),
    ("/api/share/start", h_share_start),
    ("/api/share/status", h_share_status),
    ("/api/share/stop", h_share_stop),
    ("/api/pets/active", parity_pets::h_pet_active),
    ("/api/pets/remove", parity_pets::h_pet_remove),
    ("/api/pets/thumb", parity_pets::h_pet_thumb),
    ("/api/pets/update_status", parity_pets::h_pet_update_status),
    ("/api/pets/uninstall", parity_pets::h_pet_uninstall),
    ("/api/pets/interact", parity_pets::h_pet_interact),
    ("/api/pets/import", parity_pets::h_pet_import),
    ("/api/export", h_export), // KERNEL BRIDGE — non-parity, see its doc comment
    ("/api/task/cancel", h_task_cancel),
    ("/api/export/presets", h_export_presets), // KERNEL BRIDGE — `Api.get/save_export_presets`
    // Wave E1 (F1) first planned to delete this row because `readmd.py:_route()`
    // has no `/api/export` branch.  That premise is false: `main.rs`'s injected
    // pywebview shim implements `export_doc(fmt, payload)` as
    // `fetch('/api/export', ...)` (`main.rs:1746`), `export.js:1095` in
    // `assets/js/features/` calls `py.export_doc(...)` as the **only** branch for
    // every format other than `epub`/`presentation`, and `Api.export_doc` is real
    // (`readmd.py:4873`).  So the row is the same category as the four other
    // bridges — a kernel-only front for an `Api` method — not an invented route,
    // and removing it would 404 PDF/DOCX/HTML/TeX export in the native app.
    
    // Batch 2 routes
    ("/api/update/check", batch2::h_update_check),
    ("/api/update/download", batch2::h_update_download),
    ("/api/update/status", batch2::h_update_status),
    ("/api/update/cancel", batch2::h_update_cancel),
    ("/api/update/apply", batch2::h_update_apply),
    ("/api/diagram/render", parity_diagram::h_diagram_render),
    ("/api/export/epub", batch2::h_export_epub),
    ("/api/plugins/uninstall", crate::plugin_manager::h_plugins_uninstall),
    ("/api/skill-imports", batch2::h_skill_imports_list),
    ("/api/skill-imports/preview", batch2::h_skill_imports_preview),
    ("/api/skill-imports/apply", batch2::h_skill_imports_apply),
    ("/api/convert/collect", batch2::h_convert_collect),
    ("/api/convert/progress", batch2::h_convert_progress),
    ("/api/convert/cancel", batch2::h_convert_cancel),
    
    // Direct endpoints
    ("/api/batch/extract-zip", batch2::h_batch_extract_zip),
    ("/api/pets/install", parity_pets::h_pet_install),
    ("/api/pets/check_update", parity_pets::h_pet_check_update),
    ("/api/pets/apply_update", parity_pets::h_pet_apply_update),
    ("/api/pets/configure", parity_pets::h_pet_configure),
    ("/api/pets/runtime/install", parity_pets::h_pet_runtime_install),
    ("/api/plugins/install", crate::plugin_manager::h_plugins_install),
    ("/api/export/presentation", batch2::h_export_presentation),
    ("/api/code/run", parity_code::h_code_run),
    ("/api/ocr", h_ocr_parity), // `readmd.py:3356` via `parity_web`
    ("/api/transcribe", h_transcribe_parity), // `readmd.py:2052` via `parity_web`
    ("/api/convert", batch2::h_convert),
    ("/api/convert/batch", batch2::h_convert_batch),

    // Native dialog & system bridges
    //
    // Every row in this block is `// KERNEL BRIDGE` because it fronts a pywebview
    // `Api.*` method that `readmd.py:_route()` never exposed over HTTP — so
    // `scratch/rust_parity/route_inventory.py`, which decides bridge-ness with
    // `"KERNEL BRIDGE" in line` on the row itself (`route_inventory.py:48`), must
    // find the literal *on the row line* and not in the block comment above.
    // Wave E left the prose at `PENDING` claiming these ten were "not counted as
    // implemented Python routes"; the inventory disagreed and reported them as
    // RUST-ONLY gaps.  The label is the fix, not a reclassification.
    ("/api/dialog/choose-folder", h_dialog_choose_folder), // KERNEL BRIDGE — no `readmd.py` route; PowerShell `FolderBrowserDialog`
    ("/api/dialog/choose-file", h_dialog_choose_file), // KERNEL BRIDGE — no `readmd.py` route; PowerShell `OpenFileDialog`
    ("/api/dialog/choose-any-file", h_dialog_choose_any_file), // KERNEL BRIDGE — no `readmd.py` route; PowerShell `OpenFileDialog`
    ("/api/dialog/choose-many-files", h_dialog_choose_many_files), // KERNEL BRIDGE — no `readmd.py` route; PowerShell `OpenFileDialog`
    ("/api/dialog/save-file", h_dialog_save_file), // KERNEL BRIDGE — no `readmd.py` route; PowerShell `SaveFileDialog`
    ("/api/dialog/save-as", h_dialog_save_as), // KERNEL BRIDGE — no `readmd.py` route; PowerShell `SaveFileDialog`
    ("/api/file/save-fixed", h_file_save_fixed), // KERNEL BRIDGE — pywebview `Api.save_fixed`, see P3
    ("/api/system/open-path", h_system_open_path), // KERNEL BRIDGE — no `readmd.py` route; shell open
    ("/api/system/reveal-path", h_system_reveal_path), // KERNEL BRIDGE — no `readmd.py` route; shell reveal
    ("/api/system/assoc", h_system_assoc), // KERNEL BRIDGE — no `readmd.py` route; `--assoc` front
    ("/api/clipboard/read", h_clipboard_read), // KERNEL BRIDGE — no `readmd.py` route; clipboard read

];

/// Every path `readmd.py:_route()` recognizes, copied from its dispatch chain
/// (`readmd.py:1186`-`1378`) and recorded in
/// `scratch/rust_parity/contract.json`.  The list is the denominator for
/// `/api/kernel/status`, so a route may only leave it once it is really gone.
pub const LEGACY_ROUTES: &[&str] = &[
    "/api/ai/chat",
    "/api/ai/config",
    "/api/ai/history",
    "/api/ai/models",
    "/api/ai/prompts",
    "/api/autostart/get",
    "/api/autostart/set",
    "/api/batch/extract-zip",
    "/api/bibtex",
    "/api/code/run",
    "/api/control/next",
    "/api/control/open",
    "/api/control/pet-batch",
    "/api/control/pet-menu",
    "/api/convert",
    "/api/convert/batch",
    "/api/convert/cancel",
    "/api/convert/collect",
    "/api/convert/progress",
    "/api/diagram/capabilities",
    "/api/diagram/render",
    "/api/export/epub",
    "/api/export/presentation",
    "/api/file",
    "/api/image/save",
    "/api/import/process",
    "/api/links/backlinks",
    "/api/links/deadlinks",
    "/api/links/graph",
    "/api/links/index",
    "/api/list",
    "/api/modules",
    "/api/modules/load",
    "/api/ocr",
    "/api/pets",
    "/api/pets/active",
    "/api/pets/apply_update",
    "/api/pets/check_update",
    "/api/pets/configure",
    "/api/pets/import",
    "/api/pets/install",
    "/api/pets/interact",
    "/api/pets/remove",
    "/api/pets/runtime/install",
    "/api/pets/status",
    "/api/pets/thumb",
    "/api/pets/uninstall",
    "/api/pets/update_status",
    "/api/ping",
    "/api/plugins/install",
    "/api/plugins/list",
    "/api/plugins/toggle",
    "/api/plugins/uninstall",
    "/api/recent/add",
    "/api/recent/clear",
    "/api/recent/remove",
    "/api/recent/status",
    "/api/save",
    "/api/share/start",
    "/api/share/status",
    "/api/share/stop",
    "/api/skill-imports",
    "/api/skill-imports/apply",
    "/api/skill-imports/preview",
    "/api/skills",
    "/api/style/get",
    "/api/style/save",
    "/api/system/language",
    "/api/transcribe",
    "/api/update/apply",
    "/api/update/cancel",
    "/api/update/check",
    "/api/update/download",
    "/api/update/status",
    "/api/upload",
    "/api/upstream-sources",
    "/api/url",
    "/api/web/cancel",
    "/api/web/extract",
    "/index.html",
    "/",
    "/raw",
];

/// Legacy prefixes matched with `str.startswith` instead of `==` —
/// `readmd.py:1279` branches on `/api/skill-imports/` and `readmd.py:1283` on
/// `/api/upstream-sources/`, and those two are the whole list by construction.
///
/// Wave E1 (F3) made this constant load-bearing.  It used to have zero
/// consumers — a whole-tree grep matched only this declaration — which is
/// exactly the signature of the `KERNEL_ONLY` list Wave B deleted, while the
/// real behaviour was hard-coded ad hoc in `dispatch()`.  `dispatch` now routes
/// every unmatched `/api/...` path through `dynamic_prefix_response`, which
/// iterates this list, and `p1_every_legacy_dynamic_prefix_is_consulted` plus
/// `p1_legacy_dynamic_prefixes_match_the_python_branches` fail if a row stops
/// being answered here or drifts away from `readmd.py`.
///
/// The closing `];` stays on its own line on purpose: `route_inventory.py`
/// slices these consts with `pub const NAME: &\[.*?\n\];`, and a one-line
/// declaration made that match run on and swallow the `PENDING` block below —
/// which is where the bogus `PREFIX mismatch` entries used to come from.
pub const LEGACY_DYNAMIC_PREFIXES: &[&str] = &[
    "/api/skill-imports/",
    "/api/upstream-sources/",
];

// Routes the kernel added for its own host and tests.  `readmd.py` answers
// `404 not found` for every one of them, so they are excluded from parity
// assertions and from the pending count.
//
// **Wave B removed this list.**  It had zero references in `rust/**/*.rs`
// (only its own `pub const` line matched), so it asserted nothing, and its doc
// comment claimed two `p1_*` tests that do not exist.  It also absorbed the
// eleven invented routes this wave deleted, which is how a kernel-only
// surface came to look sanctioned.  The surviving non-parity surface is now
// labelled in place, on the `ROUTES` rows themselves with the comment
// `KERNEL BRIDGE`, and consists of exactly five bridges fronting pywebview
// `Api` methods that have no Python HTTP counterpart:
//
// * `/api/settings`   — `Api.get_settings` (`readmd.py:5030`) and, on `POST`,
//   `Api.save_settings` (`readmd.py:5792`); this is where the injected shim's
//   `save_settings` posts, and why the deleted `/api/settings/save` row was
//   redundant twice over.
// * `/api/file/save-fixed` — `Api.save_fixed` (`readmd.py:5928`)
// * `/api/rename`     — `Api.rename_file`    (`readmd.py:4732`)
// * `/api/export`     — `Api.export_doc`     (`readmd.py:4873`), labelled by
//   Wave E1 after its consumer was found in `main.rs`'s shim.
// * `/api/kernel/status` — diagnostic only; its `implemented`/`pendingCount`
//   are self-declared from `ROUTES` (R1 §334, R6 §C-261) and are
//   `VOLATILE_KEYS`, so they are never parity evidence.
//
// The dialog / system / clipboard rows are bridge plumbing for the injected
// `main.rs` shim and are likewise not counted as implemented Python routes.

/// Legacy surface the kernel cannot serve yet, declared by hand.
///
/// This must never be derived from the route table: an empty table would then
/// make `pendingCount` report a clean port while every route quietly 404s.
/// `p1_pending_surface_is_declared` guards the invariant, and
/// `p1_every_legacy_route_is_either_routed_or_pending` fails when a route falls
/// out of both sets.  It is empty now: `.txt` structuring on `/api/file`, the
/// last entry, is served by `content::describe`.
pub const PENDING: &[&str] = &[];

/// `readmd.py:181` — the single-instance control port.  `instance.json` only
/// exists while the server really owns this port (`readmd.py:6494`).
pub const CONTROL_PORT: u16 = 26891;

pub fn is_api_path(path: &str) -> bool {
    path.starts_with("/api/")
}

/// Run one API call in-process, exactly as a loopback client would see it
/// (same route table, handlers and error shapes).  Used by `readmd --mcp` so
/// the MCP tools and the desktop UI share one implementation.
///
/// `target` is `"/api/…?query"`; `body` is sent as JSON when present.
pub fn call_in_process(app: &Arc<App>, method: &str, target: &str, body: Option<&Value>) -> (u16, Value) {
    let (raw_path, raw_query) = target.split_once('?').unwrap_or((target, ""));
    // With no listener bound (`readmd --mcp`) a port-less loopback Host passes
    // `local_host_authorized`; otherwise it must name the bound port.
    let host = match bound_port() {
        0 => "127.0.0.1".to_string(),
        p => format!("127.0.0.1:{p}"),
    };
    let bytes = body.map(|b| serde_json::to_vec(b).unwrap_or_default()).unwrap_or_default();
    let mut headers = HashMap::from([
        ("host".to_string(), host.clone()),
        ("origin".to_string(), format!("http://{host}")),
        ("x-readmd-app-token".to_string(), app.app_token.clone()),
        ("x-readmd-token".to_string(), app.app_token.clone()),
    ]);
    if !bytes.is_empty() {
        headers.insert("content-type".to_string(), "application/json".to_string());
        headers.insert("content-length".to_string(), bytes.len().to_string());
    }
    let req = Request {
        method: method.to_string(),
        path: raw_path.to_string(),
        query: parse_query(raw_query),
        headers,
        body: bytes,
    };
    let res = match dispatch_guarded(app, &req, true) {
        Ok(r) | Err(r) => r,
    };
    let value = serde_json::from_slice::<Value>(&res.body)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&res.body).into_owned()));
    (res.status, value)
}

fn dispatch(app: &Arc<App>, req: &Request, peer_is_loopback: bool) -> Response {
    if req.method == "OPTIONS" {
        // `readmd.py` defines no `do_OPTIONS` and sends no `Access-Control-*`
        // header anywhere (`Handler._send`, `readmd.py:1397-1412`), so
        // `BaseHTTPRequestHandler` answers the unsupported method with `501`.
        // A wildcard preflight here was not legacy behaviour: it let any web
        // page read the token-free loopback API cross-origin.
        return Response::text(501, "Unsupported method ('OPTIONS')");
    }
    if !is_api_path(&req.path) && req.path != "/raw" {
        return match serve_static(app, req) {
            Ok(res) => res,
            Err(err) => error_response(err),
        };
    }
    if let Err(err) = authorize(app, req, peer_is_loopback) {
        return error_response(err);
    }
    if let Some(stripped) = req.path.strip_suffix('/') {
        // `readmd.py:1273-1284` compares `path == ...` first and only then
        // `path.startswith(...)`, so `/api/skill-imports/` and
        // `/api/upstream-sources/` reach their sub-dispatchers with an empty
        // tail and answer 404 there — they are *not* the list endpoints.
        // Retrying the table on the stripped name would serve the list instead.
        let legacy_prefix_root = LEGACY_DYNAMIC_PREFIXES.contains(&stripped);
        if !stripped.is_empty() && !legacy_prefix_root {
            if let Some(found) = table().get(stripped) {
                return call(app, req, found);
            }
        }
    }
    match table().get(req.path.as_str()) {
        Some(handler) => call(app, req, handler),
        None => {
            if let Some(res) = dynamic_prefix_response(app, req) {
                return res;
            }
            if let Some(feature) = pending_feature(&req.path) {
                return error_response(ApiError::pending(feature));
            }
            error_response(ApiError::plain_text(404, "not found"))
        }
    }
}

/// The `str.startswith` half of `readmd.py:Handler._route()`.
///
/// `readmd.py:1279` and `readmd.py:1283` do not compare `path == ...`; they
/// branch on `path.startswith('/api/skill-imports/')` and
/// `path.startswith('/api/upstream-sources/')` and hand the whole path to a
/// sub-dispatcher.  [`LEGACY_DYNAMIC_PREFIXES`] is the kernel's copy of that
/// pair, and this is the only consumer: iterating it here is what makes the
/// constant load-bearing instead of the decorative `pub const` Wave B deleted
/// `KERNEL_ONLY` for.  `p1_every_legacy_dynamic_prefix_is_consulted` fails if a
/// row is ever added here that dispatch does not actually answer, so a prefix
/// cannot ride along unimplemented and fall through to Python's bare
/// `text/plain` 404 unnoticed.
///
/// An empty tail belongs here too, unlike everywhere else: both sub-dispatchers
/// make their own 404 from it, which is why `dispatch` does not retry the table
/// on the stripped name.
fn dynamic_prefix_response(app: &Arc<App>, req: &Request) -> Option<Response> {
    for prefix in LEGACY_DYNAMIC_PREFIXES {
        let Some(rest) = req.path.strip_prefix(prefix) else {
            continue;
        };
        return match *prefix {
            // `Handler._api_upstream_source_detail` (`readmd.py:2912`), which
            // re-splits `<source_id>/files/<file_id>` itself.
            "/api/upstream-sources/" => match h_upstream_dynamic(app, req) {
                Ok(res) => Some(res),
                Err(err) => Some(error_response(err)),
            },
            // `Handler._api_skill_import_source` (`readmd.py:2855`).
            "/api/skill-imports/" => Some(h_skill_import_source(app, req, rest)),
            _ => None,
        };
    }
    None
}

/// `Handler._api_skill_import_source` (`readmd.py:2855`): the whole `check` /
/// `update` surface for one pinned Skill source.
///
/// Python layers its three failure shapes by *where a statement sits relative
/// to the endpoint's `try:`*, and the order is load-bearing:
///
/// * the 404 for a tail that is not exactly `<id>/<check|update>`, and the 404
///   for an unknown source, are both answered **before** the `POST` check — so a
///   `GET` against a missing source reads as missing, not as the wrong verb;
/// * `find_source` sits **outside** the `try:` (`readmd.py:2864`), so a
///   skills.json whose privacy migration cannot be rewritten raises
///   `config_write_failed` past the endpoint into `Handler.do_POST`'s
///   `except Exception` → `text/plain` 500, not the JSON one;
/// * below the `try:`, `SkillImportError` → `_skill_import_error` →
///   `400 {'ok': False, 'error_code': <code>}` (the message never leaves the
///   process, `readmd.py:2764`) and any other error → the JSON 500.
fn h_skill_import_source(app: &Arc<App>, req: &Request, rest: &str) -> Response {
    let missing = || ApiError::not_found("source_not_found").noted("error", "Skill 来源不存在");
    // `parts = [unquote(p) for p in rest.split('/') if p]`: `read_request` has
    // already percent-decoded the path once, so no second decode belongs here.
    let parts: Vec<&str> = rest.trim_matches('/').split('/').filter(|p| !p.is_empty()).collect();
    if parts.len() != 2 || !matches!(parts[1], "check" | "update") {
        return error_response(missing());
    }
    let (source_id, action) = (parts[0], parts[1]);
    let ctx = skill_import_ctx(app);
    let source = match skill_import::find_source(&ctx, source_id) {
        Ok(Some(source)) => source,
        Ok(None) => return error_response(missing()),
        Err(_) => return error_response(ApiError::plain_text(500, "internal error")),
    };
    if req.method != "POST" {
        return error_response(
            ApiError::new(405, "method_not_allowed").noted("error", "仅支持 POST 请求"),
        );
    }
    match skill_import_source_action(&ctx, req, &source, source_id, action) {
        Ok(value) => skill_import_json(200, &value),
        Err(err) => error_response(err),
    }
}

/// The `try:` body of `_api_skill_import_source`, minus the exception handlers.
fn skill_import_source_action(
    ctx: &skill_import::Ctx,
    req: &Request,
    source: &JVal,
    source_id: &str,
    action: &str,
) -> Result<JVal, ApiError> {
    let body = skill_import_body(req)?;
    // `str(body.get('credential_id') or source.get('credential_id') or '').strip()`
    let credential_id = skill_import::py_strip(
        &body
            .get("credential_id")
            .filter(|value| value.truthy())
            .or_else(|| source.get("credential_id").filter(|value| value.truthy()))
            .map(|value| value.py_str())
            .unwrap_or_default(),
    );
    let preview = skill_import::preview_saved_source(ctx, source, &credential_id)
        .map_err(skill_import_failure)?;
    let changed = skill_import::source_preview_changed(source, &preview);
    let mut out = skill_import::obj(&[
        ("ok", JVal::Bool(true)),
        ("source_id", skill_import::s(source_id)),
        ("changed", JVal::Bool(changed)),
    ]);
    if action == "check" {
        // `'current_commit': source.get('resolved_commit', '')` — a key that is
        // present but null stays null; only an absent key takes the default.
        let commit = source
            .get("resolved_commit")
            .cloned()
            .unwrap_or_else(|| skill_import::s(""));
        out.set_item("current_commit", commit);
        out.set_item("preview", preview);
        return Ok(out);
    }
    if !body.get("confirm").map(|value| value.is_true()).unwrap_or(false) {
        return Err(skill_import_failure(skill_import::SkillImportError::new(
            "confirmation_required",
            "更新 Skill 来源需要明确确认",
        )));
    }
    let selections = match body.get("selections") {
        Some(JVal::List(items)) if !items.is_empty() => items.clone(),
        _ => {
            return Err(skill_import_failure(skill_import::SkillImportError::new(
                "selection_required",
                "更新前请先选择要导入的 Skill",
            )))
        }
    };
    let result =
        skill_import::apply_source_import(ctx, &preview, &selections, &credential_id, true)
            .map_err(skill_import_failure)?;
    // `{**prefix, **result}`: a repeated key keeps the prefix's slot, which is
    // what `dict` display does — and what `set_item` implements.
    if let JVal::Obj(items) = result {
        for (key, value) in items {
            out.set_item(&key, value);
        }
    }
    Ok(out)
}

/// `Handler._skill_import_error` for the codes the module raises, plus the
/// endpoint's `except Exception` for the ones that stand in for a non-SkillError.
fn skill_import_failure(error: skill_import::SkillImportError) -> ApiError {
    if error.code == "internal_error" {
        // `internal()` in the ported module means "an exception Python would not
        // have typed as SkillImportError", i.e. the JSON 500 — and the legacy
        // handler says 检查 for both actions, which is reproduced verbatim.
        return ApiError::new(500, "internal_error")
            .noted("error", "Skill 来源检查失败")
            .noted("detail", error.message);
    }
    ApiError::bad_request(error.code)
}

/// The outside world the Skill-import manager needs, assembled per request.
fn skill_import_ctx(app: &Arc<App>) -> skill_import::Ctx {
    let data_dir = app.paths.data_dir.clone();
    skill_import::Ctx {
        skills_file: data_dir.join("skills.json"),
        fetch: Some(Box::new(skill_import_fetch)),
        credential: Some(Box::new(skill_import_credential)),
        validator: Some(Box::new(skill_import_validate)),
        data_dir,
        ..skill_import::Ctx::new()
    }
}

/// `crypto.load_credential`; `_token` turns any failure into `credential_invalid`.
fn skill_import_credential(credential_id: &str) -> Result<String, String> {
    crate::crypto::load_credential(credential_id).map_err(|error| error.to_string())
}

/// `skills.SkillRegistry([parent]).validate(destination)` is `load_skill`, and
/// `load_runtime_skill` is the kernel's `load_skill`.  Every way it can fail is
/// a `SkillError` in Python — the escape-the-root check is vacuous for a folder
/// against its own parent — so `Err(false)` is never produced.
fn skill_import_validate(folder: &Path) -> Result<(), bool> {
    if load_runtime_skill(folder).is_some() {
        Ok(())
    } else {
        Err(true)
    }
}

/// `urllib.request.build_opener(_SafeRedirectHandler(api)).open(req, timeout=25)`.
///
/// `redirects(0)` on the agent hands the hop policy back to this function, which
/// is the point: `_SafeRedirectHandler.redirect_request` calls `_safe_url()` on
/// every hop **before** urllib opens the next connection, so an untrusted
/// `Location` must never be contacted.  Returning it as the *result* url lets
/// `request()`'s own `_safe_url(response.geturl())` produce the identical
/// `github_redirect_blocked` without a second error channel in [`skill_import::Fetcher`].
///
/// `Fetcher` has no `limit` argument, so the read cap is fixed at
/// `MAX_RESPONSE_BYTES + 1` — which is Python's `response.read(limit + 1)` for
/// every call site, since `request_json` and the archive download both pass
/// `MAX_RESPONSE_BYTES`.
fn skill_import_fetch(
    url: &str,
    token: &str,
    api: bool,
) -> Result<skill_import::FetchResult, skill_import::HttpError> {
    // `HTTPRedirectHandler.max_repeats` / `.max_redirections`.
    const MAX_REPEATS: usize = 4;
    const MAX_REDIRECTS: usize = 10;
    // `req.redirect_dict`: created by the first redirect and only consulted from
    // the second onward, which is why the first hop is never rejected as a repeat.
    let mut visited: Option<Vec<(String, usize)>> = None;
    let mut target = url.to_string();
    loop {
        // Rebuilt per hop: a redirect that crosses into a `NO_PROXY` host has to
        // drop back to a direct connection, same as `http_request`.
        let agent = egress_agent(&target);
        let mut request = agent
            .request("GET", &target)
            .set("Accept", "application/vnd.github+json")
            .set("User-Agent", "ReadMD-Skill-Importer")
            // `timeout=25` is a socket timeout applied to each `open()`, not a
            // deadline shared across hops — so it belongs on the per-hop request.
            .timeout(Duration::from_secs(25));
        if !token.is_empty() {
            request = request.set("Authorization", &format!("Bearer {token}"));
        }
        let response = match request.call() {
            Ok(response) => response,
            Err(ureq::Error::Status(status, response)) => {
                return Err(skill_import::HttpError {
                    status,
                    rate_limit_remaining: response
                        .header("X-RateLimit-Remaining")
                        .map(str::to_string),
                });
            }
            // `URLError` / `TimeoutError`: there is no status line to read.
            Err(_) => return Err(skill_import::HttpError { status: 0, rate_limit_remaining: None }),
        };
        let status = response.status();
        let location = match response.header("Location") {
            Some(value) => value.to_string(),
            None => {
                return Ok(skill_import::FetchResult {
                    body: read_egress_body_bounded(response, skill_import::MAX_RESPONSE_BYTES + 1),
                    final_url: target,
                })
            }
        };
        if !(301..=308).contains(&status) {
            return Ok(skill_import::FetchResult {
                body: read_egress_body_bounded(response, skill_import::MAX_RESPONSE_BYTES + 1),
                final_url: target,
            });
        }
        // Drain the redirect body before moving on, or the pooled connection is
        // leaked instead of reused by the next hop.
        let _ = read_egress_body(response);
        let next = resolve_location(&target, &location).unwrap_or_else(|_| target.clone());
        let mut counts = visited.take().unwrap_or_default();
        if visited.is_some() {
            let seen = counts
                .iter()
                .find(|(url, _)| *url == next)
                .map(|(_, count)| *count)
                .unwrap_or(0);
            if seen >= MAX_REPEATS || counts.len() >= MAX_REDIRECTS {
                // `HTTPError(req.full_url, code, …)` carrying the redirect's own
                // status → `github_http_error（HTTP <status>）`.
                return Err(skill_import::HttpError { status, rate_limit_remaining: None });
            }
        }
        match counts.iter_mut().find(|(url, _)| *url == next) {
            Some(entry) => entry.1 += 1,
            None => counts.push((next.clone(), 1)),
        }
        visited = Some(counts);
        // `_SafeRedirectHandler.redirect_request` → `_safe_url(newurl)` happens
        // before urllib opens the next connection, so an untrusted hop is never
        // contacted; handing it back as `final_url` is what makes `request()`
        // raise the same `github_redirect_blocked`.
        if skill_import::safe_url(&next, api).is_err() {
            return Ok(skill_import::FetchResult { body: Vec::new(), final_url: next });
        }
        target = next;
    }
}

/// [`read_egress_body`] with `response.read(limit + 1)`'s cap: reading past the
/// limit is exactly what the size check in `request()` exists to catch.
fn read_egress_body_bounded(response: ureq::Response, cap: usize) -> Vec<u8> {
    let mut data = Vec::new();
    let _ = response.into_reader().take(cap as u64).read_to_end(&mut data);
    data
}

/// `Handler._skill_import_body` (`readmd.py:2747`) over the bytes `read_body`
/// already buffered for this request.
fn skill_import_body(req: &Request) -> Result<JVal, ApiError> {
    let invalid_size =
        || skill_import_failure(skill_import::SkillImportError::new("request_invalid", "请求体大小无效"));
    // `int(handler.headers.get('Content-Length', 0) or 0)`: an absent *or* empty
    // header is 0, anything `int()` rejects is `request_invalid`.
    let declared = req.header("content-length").unwrap_or("").trim();
    let length = if declared.is_empty() {
        0
    } else {
        match declared.parse::<i64>() {
            Ok(value) => value,
            Err(_) => return Err(invalid_size()),
        }
    };
    if length < 0 || length > 2 * 1024 * 1024 {
        return Err(skill_import_failure(skill_import::SkillImportError::new(
            "request_too_large",
            "请求体超过安全大小限制",
        )));
    }
    if length == 0 {
        return Ok(JVal::Obj(Vec::new()));
    }
    let invalid_body = || {
        skill_import_failure(skill_import::SkillImportError::new(
            "request_invalid",
            "请求体必须是有效 JSON",
        ))
    };
    let text = match std::str::from_utf8(&req.body) {
        Ok(value) => value,
        Err(_) => return Err(invalid_body()),
    };
    match skill_import::json_loads(text) {
        // `if not isinstance(body, dict): request_invalid`
        Ok(value) if value.is_dict() => Ok(value),
        Ok(_) => Err(skill_import_failure(skill_import::SkillImportError::new(
            "request_invalid",
            "请求体必须是 JSON 对象",
        ))),
        Err(_) => Err(invalid_body()),
    }
}

/// `Handler._send_json(status, obj)` without re-encoding through `serde_json`:
/// `skill_import::json_dumps` *is* `json.dumps(obj, ensure_ascii=False)` — same
/// `", "` / `": "` separators and the same float formatting — and it carries the
/// key order the endpoint built instead of sorting it.
fn skill_import_json(status: u16, value: &JVal) -> Response {
    Response {
        status,
        headers: vec![(
            "Content-Type".to_string(),
            "application/json; charset=utf-8".to_string(),
        )],
        body: skill_import::json_dumps(value, None).into_bytes(),
    }
}

fn call(app: &Arc<App>, req: &Request, handler: &Handler) -> Response {
    match handler(app, req) {
        // No `Access-Control-Allow-Origin` on any local API response:
        // `Handler._send` (`readmd.py:1397-1412`) emits
        // Content-Type/Length/Cache-Control/X-Frame-Options only, so a
        // cross-origin page cannot read the loopback answers back.
        Ok(res) => res,
        Err(err) => error_response(err),
    }
}

/// The three bodies `readmd.py` can produce; a text/plain failure keeps its
/// `Content-Type` and raw payload instead of being JSON-wrapped.
fn error_response(err: ApiError) -> Response {
    let res = match &err.shape {
        crate::error::ErrorShape::PlainText { body } => Response::text(err.status, body),
        _ => Response::json_status(err.status, &err.payload()),
    };
    res.header("X-ReadMD-Error", err.code.as_str())
}

impl Response {
    fn empty(status: u16) -> Response {
        Response { status, headers: Vec::new(), body: Vec::new() }
    }
}

/// Which declared pending surface (if any) covers this path.  Entries are
/// either an exact path, a `/{...}` template, or a trailing-slash prefix; a
/// capability marker such as `/api/file.txt-md-structuring` describes a
/// degraded behaviour inside a routed handler and never matches a path.
fn pending_feature(path: &str) -> Option<String> {
    PENDING.iter().find_map(|p| {
        if *p == path {
            return Some((*p).to_string());
        }
        if p.contains('{') {
            let want: Vec<&str> = p.split('/').collect();
            let mine: Vec<&str> = path.split('/').collect();
            if want.len() != mine.len() {
                return None;
            }
            for (w, m) in want.iter().zip(mine.iter()) {
                if w.starts_with('{') {
                    continue;
                }
                if w != m {
                    return None;
                }
            }
            return Some((*p).to_string());
        }
        if p.ends_with('/') && path.starts_with(p) {
            return Some((*p).to_string());
        }
        None
    })
}

// ------------------------------------------------------------- authorization
//
// `do_GET` gates on `_lan_authorized()`; `do_POST`/`do_DELETE` add
// `_post_origin_authorized()` and, for `/api/save`, the `X-ReadMD-App-Token`
// header.  Every refusal is `_send(403, 'text/plain; charset=utf-8',
// b'forbidden')`.

fn authorize(app: &Arc<App>, req: &Request, peer_is_loopback: bool) -> ApiResult<()> {
    let local = local_host_authorized(req);
    if is_api_path(&req.path) && !local {
        return Err(ApiError::plain_text(403, "forbidden"));
    }
    if req.method == "POST" || req.method == "DELETE" {
        if !local || !post_origin_authorized(req) {
            return Err(ApiError::plain_text(403, "forbidden"));
        }
        if req.path == "/api/save" && !app_token_authorized(app, req) {
            return Err(ApiError::plain_text(403, "forbidden"));
        }
    }
    if requires_token() && !env_token_authorized(app, req) {
        return Err(ApiError::plain_text(403, "forbidden"));
    }
    let _ = peer_is_loopback;
    Ok(())
}

/// `Handler._local_host_authorized` (`readmd.py:1146`): the `Host` header must
/// name a loopback name and, when it carries a port, that exact port.  A LAN
/// share runs on its own socket, so `LAN_TOKEN` is never set for this server.
fn local_host_authorized(req: &Request) -> bool {
    let (host, port) = split_authority(req.header("host").unwrap_or(""));
    matches!(host, "127.0.0.1" | "localhost" | "::1")
        && port.map(|p| p == bound_port()).unwrap_or(true)
}

/// `Handler._post_origin_authorized` (`readmd.py:1154`).
fn post_origin_authorized(req: &Request) -> bool {
    match req.header("origin") {
        Some(origin) => {
            let auth = origin_authority(origin);
            let (oh, op) = split_authority(&auth);
            let (hh, hp) = split_authority(req.header("host").unwrap_or(""));
            oh == hh && op == hp
        }
        None => match req.header("sec-fetch-site") {
            Some(v) => v == "none" || v == "same-origin",
            None => true,
        },
    }
}

/// `secrets.compare_digest(supplied, self.server.app_token)` for `/api/save`.
fn app_token_authorized(app: &Arc<App>, req: &Request) -> bool {
    let supplied = req.header("x-readmd-app-token").unwrap_or("");
    !app.app_token.is_empty() && !supplied.is_empty() && supplied == app.app_token.as_str()
}

/// Kernel-only hardening switch (`READMD_REQUIRE_TOKEN=1`); `readmd.py` has no
/// equivalent because the local server is trusted by construction.
fn requires_token() -> bool {
    std::env::var("READMD_REQUIRE_TOKEN")
        .map(|v| matches!(v.trim(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false)
}

/// The token the kernel-only switch accepts is the *running instance's*
/// `server.app_token` (`readmd.py:966`), the same secret `/api/save` compares
/// (`readmd.py:1074-1076`); `READMD_APP_TOKEN` only exists so an operator can
/// pin a fixed value externally.  Reading the env var alone made the switch
/// 403 every request because nothing in the kernel ever writes it.
fn env_token_authorized(app: &Arc<App>, req: &Request) -> bool {
    let pinned = std::env::var("READMD_APP_TOKEN").unwrap_or_default();
    let want: &str = if pinned.trim().is_empty() {
        app.app_token.as_str()
    } else {
        pinned.as_str()
    };
    if want.is_empty() {
        return false;
    }
    let mut supplied = param_or(req, &Value::Null, "t", "");
    if supplied.is_empty() {
        supplied = req.header("x-readmd-token").unwrap_or("").to_string();
    }
    !supplied.is_empty() && supplied == want
}

/// `urlparse('//' + value)` for a `Host` header or an `Origin` URL.
fn split_authority(value: &str) -> (&str, Option<u16>) {
    let value = value
        .trim()
        .trim_start_matches("//")
        .trim_start_matches("http://")
        .trim_start_matches("https://");
    let value = match value.find('/') {
        Some(i) => &value[..i],
        None => value,
    };
    if let Some(rest) = value.strip_prefix('[') {
        if let Some(close) = rest.find(']') {
            let host = &rest[..close];
            let port = rest[close + 1..]
                .strip_prefix(':')
                .and_then(|p| p.parse::<u16>().ok());
            return (host, port);
        }
    }
    match value.rsplit_once(':') {
        Some((h, p)) => (h, p.parse::<u16>().ok()),
        None => (value, None),
    }
}

fn origin_authority(origin: &str) -> String {
    let without_scheme = match origin.find("://") {
        Some(i) => &origin[i + 3..],
        None => origin,
    };
    without_scheme.to_string()
}

// ------------------------------------------------------------ instance.json
//
// `readmd.py` publishes `{port, token, pid, started}` only while it owns
// `CONTROL_PORT`; `_api_ping` and `_api_control_open` re-read the file on every
// request, so an external controller can rotate the token.

pub fn instance_file(app: &Arc<App>) -> PathBuf {
    app.paths.data_dir.join("instance.json")
}

/// `_read_instance().get('token', '')`.
fn instance_token(app: &Arc<App>) -> String {
    std::fs::read_to_string(instance_file(app))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.get("token").and_then(|x| x.as_str()).map(|s| s.to_string()))
        .unwrap_or_default()
}

/// `_write_instance(CONTROL_PORT, secrets.token_urlsafe(16))`.
fn write_instance(app: &Arc<App>, port: u16) {
    let started = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    let value = json!({
        "port": port,
        "token": token_urlsafe(16),
        "pid": std::process::id(),
        "started": started,
    });
    if let Ok(text) = serde_json::to_string(&value) {
        let file = instance_file(app);
        let _ = std::fs::create_dir_all(&app.paths.data_dir);
        let _ = std::fs::write(&file, text);
    }
}

/// `secrets.token_urlsafe(nbytes)`: base64url of `nbytes` random bytes, unpadded.
fn token_urlsafe(nbytes: usize) -> String {
    use base64::Engine;
    let mut buf = vec![0u8; nbytes];
    if getrandom::fill(&mut buf).is_err() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0)
            .to_le_bytes();
        for (i, slot) in buf.iter_mut().enumerate() {
            *slot = nanos[i % nanos.len()] ^ (i as u8 * 31);
        }
    }
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buf)
}

// -------------------------------------------------------------- static host

fn serve_static(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    if req.method != "GET" && req.method != "HEAD" {
        return Err(ApiError::bad_request("method_not_allowed"));
    }
    if req.path == "/" || req.path == "/index.html" || req.path == "/index" {
        return serve_index(app, req);
    }
    if req.path == "/favicon.ico" {
        let candidate = app.paths.assets_dir.join("img/favicon.ico");
        if candidate.is_file() {
            return send_file(req, &candidate, true);
        }
        return Ok(Response::empty(204));
    }
    let root = app
        .paths
        .assets_dir
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| app.paths.assets_dir.clone());
    let root_c = paths::canonical_existing(&root).unwrap_or_else(|_| root.clone());
    let assets_c =
        paths::canonical_existing(&app.paths.assets_dir).unwrap_or_else(|_| app.paths.assets_dir.clone());
    let rel = req.path.trim_start_matches('/');
    let mut candidates = vec![root.join(rel)];
    candidates.push(app.paths.assets_dir.join(rel));
    if let Some(stripped) = rel.strip_prefix("assets/") {
        candidates.insert(1, app.paths.assets_dir.join(stripped));
    }
    for candidate in candidates {
        let Ok(canonical) = paths::canonical_existing(&candidate) else { continue };
        if !canonical.starts_with(&root_c) && !canonical.starts_with(&assets_c) {
            return Err(ApiError::forbidden("path_escape"));
        }
        if canonical.is_dir() {
            let index = canonical.join("index.html");
            if index.is_file() {
                return send_file(req, &index, false);
            }
            continue;
        }
        if canonical.is_file() {
            return send_file(req, &canonical, immutable_hint(&canonical, req.q("v").is_some()));
        }
    }
    Err(ApiError::not_found("asset_missing").noted("path", req.path.clone()))
}

/// Fonts and images never change in place; vendor/dist scripts only count as
/// immutable when the URL carries a `?v=` stamp, so an upgraded bundle is not
/// shadowed by a year-long cache entry of the previous one.
fn immutable_hint(path: &Path, versioned: bool) -> bool {
    let name = path.to_string_lossy().replace('\\', "/");
    (versioned && (name.contains("/vendor/") || name.contains("/dist/"))) || matches!(content::ext_of(path).as_str(), "woff2" | "woff" | "ttf" | "png" | "jpg" | "svg" | "ico" | "gif" | "webp")
}

fn send_file(req: &Request, path: &Path, cache: bool) -> ApiResult<Response> {
    // `_send_file` (`readmd.py:2969`) has no local handler: a failing `os.stat()`
    // escapes to `do_GET`'s blanket `except Exception`
    // (`readmd.py:1059-1066`) -> `500 text/plain "internal error"`.
    let meta = std::fs::metadata(path).map_err(|_| ApiError::plain_text(500, "internal error"))?;
    let etag = format!(
        "\"{:x}-{:x}\"",
        meta.len(),
        content::modified_millis(path)
    );
    if req.header("if-none-match") == Some(etag.as_str()) {
        return Ok(Response::empty(304).header("ETag", &etag));
    }
    let bytes = std::fs::read(path).map_err(|_| ApiError::plain_text(500, "internal error"))?;
    let ctype = mime_of(&content::ext_of(path));
    let mut res = Response::raw_bytes(200, ctype, bytes).header("ETag", &etag);
    if cache {
        res = res.header("Cache-Control", "public, max-age=31536000, immutable");
    } else {
        res = res.header("Cache-Control", "no-cache");
    }
    if ctype == "text/html" || ctype.starts_with("text/") {
        res = res.header("X-Frame-Options", "DENY");
    }
    Ok(res)
}

fn serve_index(app: &Arc<App>, _req: &Request) -> ApiResult<Response> {
    let file = app.paths.assets_dir.join("index.html");
    // (tokens.css and other nested stylesheets are served by `serve_static`.)
    if !file.is_file() {
        // `_send_index` (`readmd.py:1515-1517`).
        return Err(ApiError::plain_text(404, "not found"));
    }
    let mut data = std::fs::read(&file).map_err(|_| ApiError::plain_text(500, "internal error"))?;
    let token = app.app_token.clone();
    data = replace_bytes(&data, b"window.LAN_TOKEN=null;", format!("window.LAN_TOKEN=\"{token}\";").as_bytes());
    data = replace_bytes(
        &data,
        b"<meta name=\"readmd-app-token\" content=\"\">",
        format!("<meta name=\"readmd-app-token\" content=\"{token}\">").as_bytes(),
    );
    // `readmd.py:1527-1529` — the page's own probe flag, which
    // `assets/js/core/state.js:28` and `assets/readmd.boot.js:99` read back as
    // `IS_STARTUP_PROBE`.
    if crate::batch2::startup_probe_enabled() {
        data = replace_bytes(
            &data,
            b"<meta name=\"readmd-startup-probe\" content=\"0\">",
            b"<meta name=\"readmd-startup-probe\" content=\"1\">",
        );
    }
    let marker = b"<head>";
    let injection = format!(
        "<head><script>window.READMD_ENGINE=\"rust\";window.READMD_KERNEL_VERSION=\"{VERSION}\";window.APP_TOKEN=\"{token}\";</script>"
    );
    if let Some(pos) = find_bytes(&data, marker) {
        let mut out = Vec::with_capacity(data.len() + injection.len());
        out.extend_from_slice(&data[..pos]);
        out.extend_from_slice(injection.as_bytes());
        out.extend_from_slice(&data[pos + marker.len()..]);
        data = out;
    }
    Ok(Response::raw_bytes(200, "text/html; charset=utf-8", data)
        .header("Cache-Control", "no-store")
        .header("X-Frame-Options", "DENY"))
}

fn find_bytes(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    (0..=hay.len() - needle.len()).find(|&i| &hay[i..i + needle.len()] == needle)
}

fn replace_bytes(hay: &[u8], needle: &[u8], with: &[u8]) -> Vec<u8> {
    if find_bytes(hay, needle).is_none() {
        return hay.to_vec();
    }
    let mut out = Vec::with_capacity(hay.len());
    let mut i = 0usize;
    while i < hay.len() {
        if i + needle.len() <= hay.len() && &hay[i..i + needle.len()] == needle {
            out.extend_from_slice(with);
            i += needle.len();
        } else {
            out.push(hay[i]);
            i += 1;
        }
    }
    out
}

fn mime_of(ext: &str) -> &'static str {
    match ext {
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" | "mjs" => "text/javascript",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "md" | "markdown" | "txt" => "text/plain",
        "xml" => "application/xml",
        "wasm" => "application/wasm",
        "map" => "application/json",
        _ => "application/octet-stream",
    }
}

// ------------------------------------------------------------------ helpers

/// Path argument for the routes `readmd.py` feeds through `resolve_doc`-style
/// containment checks.  Query and body both count, query first.
fn resolve_arg(app: &Arc<App>, req: &Request, keys: &[&str]) -> ApiResult<PathBuf> {
    let body = body_value(req);
    if let Some(raw) = param_first(req, &body, keys) {
        if !raw.trim().is_empty() {
            // `Error → ApiError` (`lib.rs`) is the ladder that keeps the
            // envelope honest: a containment denial becomes
            // `403 {"ok": false, "error_code": "forbidden"}` and an unresolved
            // path `404 not_found`, with the sentence carrying the absolute path
            // held in `detail`, which `ApiError::payload()` never serializes.
            // The previous `ApiError::internal(format!("serialize_failed: {e}"))`
            // answered 500 and echoed the path inside `error_code` — the
            // `serialize_failed` finding in `scratch/rust_parity/diff-triage.md`.
            return app.paths.resolve_doc(&raw).map_err(ApiError::from);
        }
    }
    Err(ApiError::bad_request("missing_path"))
}

/// The legacy reader/raw/save routes take the path exactly as the client typed
/// it: `_route()` does `unquote(qs.get('p', [''])[0])` and hands that string to
/// `os.path.isfile` / `open()` without resolving it against a document root.
/// Only the empty value counts as missing.
fn raw_path_arg(req: &Request, body: &Value, keys: &[&str]) -> Option<String> {
    param_first(req, body, keys)
        .map(|v| route_unquote(&v))
        .filter(|v| !v.is_empty())
}

/// `os.path.realpath(os.path.normpath(p))` as a comparison key.
fn real_key(path: &Path) -> String {
    paths::canonicalize_or_clean(path).to_string_lossy().replace('\\', "/")
}

/// `Handler._route` decodes the query once with `parse_qs` and then applies a
/// second `unquote()` to the value it hands the handler
/// (`readmd.py:1190`, `:1196`, `:1375`, and `:1217`/`:1220` for OCR/URL), so a
/// `%25` in a `?p=` reaches `os.path.isfile` as `%`.
fn route_unquote(value: &str) -> String {
    percent_encoding::percent_decode_str(value)
        .decode_utf8_lossy()
        .into_owned()
}

/// `readmd.py:957` — `SAVE_EXTENSIONS`.  `/api/save` rejects every other
/// extension even when the path is authorized (`readmd.py:3554-3556`).
const PY_SAVE_EXTENSIONS: &[&str] = &[".md", ".markdown", ".mdown", ".mkd", ".mdx", ".txt"];

/// `os.path.abspath(p)` = `normpath(join(os.getcwd(), p))`.  Kept separate from
/// [`real_key`] because `save_text_atomic` reports the `abspath` while the gate
/// compares the `realpath` (`readmd.py:3540` / `file_writer.py:24`).
fn py_abspath(raw: &str) -> PathBuf {
    let candidate = PathBuf::from(raw.replace('/', std::path::MAIN_SEPARATOR_STR));
    let joined = if candidate.is_absolute() {
        candidate
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(candidate)
    };
    paths::canonicalize_or_clean(&joined)
}

/// `ntpath.dirname(p)` for the *typed* string — the head up to the last
/// separator, with a lone leading separator kept (`'\y'` → `'\'`).
fn py_dirname(p: &str) -> String {
    match p.as_bytes().iter().rposition(|b| *b == b'/' || *b == b'\\') {
        None => String::new(),
        Some(i) => {
            let head = &p[..i];
            if head.is_empty() {
                p[..=i].to_string()
            } else {
                head.to_string()
            }
        }
    }
}

/// `ntpath.basename(p)` — everything after the last separator.
fn py_basename(p: &str) -> String {
    match p.as_bytes().iter().rposition(|b| *b == b'/' || *b == b'\\') {
        None => p.to_string(),
        Some(i) => p[i + 1..].to_string(),
    }
}

/// `ServerHandler.authorized_save_paths` (`readmd.py:967`).
///
/// Python's HTTP layer has **no** allowed-root list: the only containment
/// decision anywhere in `Handler` is this set, which is populated by a prior
/// `GET /api/file` (`readmd.py:3015`) and by the AI-derived-name branch of
/// `_do_save` itself (`readmd.py:3548-3553`).  `/api/list`, `/api/file` and
/// `/raw` consult nothing at all.
fn authorized_save_paths() -> &'static std::sync::Mutex<std::collections::HashSet<String>> {
    static SET: OnceLock<std::sync::Mutex<std::collections::HashSet<String>>> = OnceLock::new();
    SET.get_or_init(|| std::sync::Mutex::new(std::collections::HashSet::new()))
}

fn remember_authorized_save(path: &Path) {
    if let Ok(mut set) = authorized_save_paths().lock() {
        set.insert(real_key(path));
    }
}

fn save_path_authorized(key: &str) -> bool {
    authorized_save_paths()
        .lock()
        .map(|set| set.contains(key))
        .unwrap_or(false)
}

pub fn ok_json(value: Value) -> ApiResult<Response> {
    Ok(Response::json(&value))
}

/// `Handler._send_json(status, obj)` for a success body.
pub fn ok_json_status(status: u16, value: Value) -> ApiResult<Response> {
    Ok(Response::json_status(status, &value))
}

/// The `/api/recent/*` failure envelope.
///
/// `_api_recent_status` / `_api_recent_add` / `_api_recent_clear` /
/// `_api_recent_remove` (`readmd.py:2243`-`2295`) call `_send_json` directly
/// with `{'ok': False, 'code': <name>}` instead of going through
/// `_send_api_error`, so the key really is **`code`** and not `error_code`.
/// Python's inconsistency is copied verbatim rather than "fixed".
fn recent_error<T: Into<String>>(status: u16, code: T) -> ApiResult<Response> {
    ok_json_status(status, json!({ "ok": false, "code": code.into() }))
}

// ----------------------------------------------------------------- handlers

fn h_ping(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    let supplied = req.q("t").unwrap_or("");
    let want = instance_token(app);
    ok_json(json!({ "ok": !supplied.is_empty() && supplied == want }))
}

/// KERNEL BRIDGE — not a parity route.  `readmd.py`'s HTTP dispatcher
/// (`Handler._route`, `readmd.py:1186`-`1378`) has no `/api/kernel/status`:
/// Python answers `404 text/plain "not found"` for that path.  The route exists
/// only so `main.rs` and the harness can introspect which paths the *Rust*
/// kernel advertises; it reports the kernel's own `ROUTES`/`PENDING` tables and
/// runs no Python-visible behaviour.
///
/// Its `implemented` list and `pendingCount` are self-declared from that same
/// table, so they are `VOLATILE_KEYS` in the differential harness and can never
/// count as parity evidence — a route appearing here is precisely what the
/// request would have to prove independently.
fn h_kernel_status(app: &Arc<App>, _req: &Request) -> ApiResult<Response> {
    let mut implemented: Vec<&str> = table().keys().copied().collect();
    implemented.sort_unstable();
    let mut pending = PENDING.to_vec();
    pending.sort_unstable();
    let legacy_total = implemented.len() + pending.len();
    ok_json(json!({
        "ok": true,
        "engine": "rust",
        "version": VERSION,
        "serverTag": SERVER_TAG,
        "routes": {
            "implemented": implemented,
            "pending": pending,
            "implementedCount": implemented.len(),
            "pendingCount": pending.len(),
            "legacyTotal": legacy_total,
        },
        "store": app.store.stats().unwrap_or_else(|e| json!({"error": e.to_string()})),
        "uptimeMs": app.started_at.elapsed().as_millis() as u64,
    }))
}

fn h_file(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    // `readmd.py:1189-1194` — `p` comes from the query only (never the body),
    // and an empty one is `400 text/plain; charset=utf-8` `"missing p"`, which
    // is a raw body, not a JSON envelope.
    let raw = match raw_path_arg(req, &Value::Null, &["p"]) {
        Some(v) => v,
        None => return Err(ApiError::plain_text(400, "missing p")),
    };
    // `Handler._api_file` has **no** allowed-root gate: `readmd.py:3011-3013`
    // tests `os.path.isfile(p)` on the string the client typed and answers
    // `404 {'error': '文件不存在'}` — the `error`-only LegacyError shape, with no
    // `ok` and no `error_code`.  Any exception that escapes `_api_file` reaches
    // `do_GET`'s blanket handler (`readmd.py:1059-1066`), i.e.
    // `500 text/plain; charset=utf-8` `"internal error"`.
    let path = py_abspath(&raw);
    if !path.is_file() {
        return Ok(Response::json_status(
            404,
            &json!({ "error": "文件不存在", "error_code": crate::api_codes::FILE_NOT_FOUND }),
        ));
    }
    // `readmd.py:3015` — reading a document through this route is precisely
    // what authorizes `/api/save` to write it back.
    remember_authorized_save(&path);
    let meta_only = req.q("meta") == Some("1");
    let mut value = content::describe(app, &path, !meta_only)
        .map_err(|_| ApiError::plain_text(500, "internal error"))?;
    if let Some(obj) = value.as_object_mut() {
        // `readmd.py:3029-3031` — `name`/`dir`/`path` are cut out of the string
        // the client sent, not of the resolved path.
        obj.insert("dir".into(), json!(py_dirname(&raw)));
        obj.insert("name".into(), json!(py_basename(&raw)));
        obj.insert("path".into(), json!(raw.as_str()));
        obj.insert("absPath".into(), json!(path.to_string_lossy()));
        obj.insert("mtime".into(), json!(content::modified_millis(&path) as f64 / 1000.0));
        obj.insert("mtimeMs".into(), json!(content::modified_millis(&path)));
        obj.insert("is_code".into(), json!(content::kind_of(&path) == "code"));
        obj.insert("code_lang".into(), json!(content::ext_of(&path)));
        let converted = obj.get("converted").and_then(|v| v.as_bool()).unwrap_or(false);
        obj.insert("is_markdown".into(), json!(converted || content::kind_of(&path) == "markdown"));
        if converted {
            obj.insert("binary".into(), json!(false));
        }
        if !meta_only {
            let content_text = obj.get("content").cloned().unwrap_or(Value::Null);
            obj.insert("text".into(), content_text.clone());
            obj.insert("content".into(), content_text.clone());
            obj.insert("data".into(), content_text);
        }
    }
    if !meta_only {
        let display = app.paths.display_path(&path);
        let title = value.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let _ = app.store.touch_recent(&display, &title);
    }
    ok_json(value)
}

/// `Handler._api_list` (`readmd.py:3095-3112`).
///
/// **There is no allowed-root gate here, in Python or now in Rust.**  `_route()`
/// hands `unquote(qs.get('p', [''])[0])` straight to `os.path.isdir` /
/// `os.walk`, so any directory the process can read is listable — including one
/// outside the workspace.  `/api/save` is the only route in `Handler` with a
/// containment decision (`readmd.py:3554-3556`), and the loopback `Host` check
/// (`readmd.py:1146`) plus `_post_origin_authorized` (`readmd.py:1154`) are what
/// keep that reachable.  The kernel's earlier `check_allowed` hop invented a
/// `403` Python never sends.
///
/// A non-directory is not an error either: `readmd.py:3096-3098` answers
/// `200 {'dir': p, 'files': []}`, and the body is exactly those two keys.
fn h_list(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    let _ = app;
    let raw = raw_path_arg(req, &Value::Null, &["p"]).unwrap_or_default();
    if raw.is_empty() {
        // `os.path.isdir('')` is False, so the empty path is the empty listing —
        // never the process working directory.
        return ok_json(json!({ "dir": raw, "files": [] }));
    }
    let dir = py_abspath(&raw);
    if !dir.is_dir() {
        return ok_json(json!({ "dir": raw, "files": [] }));
    }
    let mut files: Vec<String> = Vec::new();
    let mut stack = vec![(dir.clone(), 0usize)];
    while let Some((current, depth)) = stack.pop() {
        if depth >= 4 || files.len() >= 500 {
            continue;
        }
        let Ok(read) = std::fs::read_dir(&current) else { continue };
        let mut subdirs = Vec::new();
        let mut names: Vec<(String, PathBuf)> = Vec::new();
        for entry in read.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || name.starts_with('_') {
                continue;
            }
            let p = entry.path();
            if p.is_dir() {
                subdirs.push(p);
            } else if content::MD_EXTS.contains(&content::ext_of(&p).as_str()) {
                names.push((name, p));
            }
        }
        names.sort_by(|a, b| a.0.cmp(&b.0));
        for (_, p) in names {
            if files.len() >= 500 {
                break;
            }
            files.push(p.to_string_lossy().into_owned());
        }
        for sd in subdirs {
            stack.push((sd, depth + 1));
        }
    }
    ok_json(json!({ "dir": raw, "files": files }))
}


/// `Handler._send_raw` (`readmd.py:3564-3578`).
///
/// Like `/api/file` and `/api/list` this route has **no** allowed-root gate in
/// Python: `unquote(qs.get('p', [''])[0])` (`readmd.py:1375`) goes straight to
/// `os.path.isfile` and `open()`.  Its two failure bodies are `text/plain`, not
/// JSON — `404` `"not found"` and `500` `"read error"` — and its success headers
/// are Content-Type / Content-Length / `Cache-Control: no-cache` only: no ETag,
/// no conditional 304 and no `X-Frame-Options`, which is why it does not go
/// through [`send_file`] (that one mirrors `_send_file`, `readmd.py:2969-2996`).
fn h_raw(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    let _ = app;
    let raw = raw_path_arg(req, &Value::Null, &["p"]).unwrap_or_default();
    let path = py_abspath(&raw);
    if !raw.is_empty() && path.is_file() {
        match std::fs::read(&path) {
            Ok(bytes) => {
                return Ok(Response::raw_bytes(200, mime_of(&content::ext_of(&path)), bytes)
                    .header("Cache-Control", "no-cache"))
            }
            // `except OSError` around the `open()`/`read()` (`readmd.py:3570-3572`).
            Err(_) => return Err(ApiError::plain_text(500, "read error")),
        }
    }
    Err(ApiError::plain_text(404, "not found"))
}


/// `save_text_atomic` (`src/readmd_core/file_writer.py:17-83`).
///
/// It never raises for ordinary I/O: every failure comes back as
/// `{'ok': False, 'error': <str(exc)>}`, and a stale editor state comes back as
/// the four-key `conflict` dict.  `_do_save` maps those onto 500 / 409.
fn py_save_text_atomic(path: &Path, content: &str, encoding: &str, expected_mtime: Option<f64>) -> Value {
    // Encode first: an unrepresentable character must fail the save *before*
    // any backup or write happens, never be replaced silently.
    let bytes = match crate::text_encoding::encode(content, encoding) {
        Ok(b) => b,
        Err(crate::text_encoding::EncodeError::Unrepresentable { ch, char_index }) => {
            return json!({
                "ok": false,
                "error": format!("当前编码（{encoding}）无法表示字符“{ch}”，可改用 UTF-8 保存"),
                "error_code": "encoding_unrepresentable",
                "encoding": encoding,
                "char": ch.to_string(),
                "offset": char_index,
            });
        }
        Err(crate::text_encoding::EncodeError::Unknown(name)) => {
            return json!({
                "ok": false,
                "error": format!("unknown encoding: {name}"),
                "error_code": "encoding_unknown",
            });
        }
    };
    // `path = os.path.abspath(path)` (`file_writer.py:24`).
    let abspath = paths::canonicalize_or_clean(path);
    // `shutil.copy2(path, path + '.bak')` — string concatenation, so the backup
    // is a sibling named after the *whole* path (`note.md.bak`).
    let backup_path = PathBuf::from(format!("{}.bak", abspath.to_string_lossy()));
    let old_exists = abspath.is_file();
    if expected_mtime.is_some() && !old_exists {
        return json!({
            "ok": false,
            "conflict": true,
            "error": "预期文件已不存在，未重新创建",
            "current_mtime": null,
        });
    }
    let old_mtime = if old_exists {
        Some(content::modified_millis(&abspath) as f64 / 1000.0)
    } else {
        None
    };
    if let (Some(want), Some(have)) = (expected_mtime, old_mtime) {
        // `_same_mtime` is `math.isclose(..., rel_tol=0.0, abs_tol=1e-6)`.
        if (have - want).abs() > 1e-6 {
            return json!({
                "ok": false,
                "conflict": true,
                "error": "文件已在编辑后被其他程序修改",
                "current_mtime": have,
            });
        }
    }
    let mut backup = Value::Null;
    if old_exists && !backup_path.exists() {
        match std::fs::copy(&abspath, &backup_path) {
            Ok(_) => backup = json!(backup_path.to_string_lossy().into_owned()),
            Err(e) => return json!({ "ok": false, "error": e.to_string() }),
        }
    }
    match content::write_bytes_atomic(&abspath, &bytes) {
        Ok(()) => json!({
            "ok": true,
            // `readmd.py:3557-3562` reports `save_text_atomic`'s dict verbatim:
            // exactly `ok`, `path`, `backup`, `mtime`.
            "path": abspath.to_string_lossy().into_owned(),
            "backup": backup,
            "mtime": content::modified_millis(&abspath) as f64 / 1000.0,
        }),
        Err(e) => json!({ "ok": false, "error": e.to_string() }),
    }
}

/// `body.get(k) or fallback`, with Python's truthiness rather than Rust's.
/// `Err(())` means "a truthy non-string", i.e. `os.path.realpath(5)` raises and
/// the exception escapes `_do_save`.
fn py_or_string(body: &Value, keys: &[&str], fallback: &str) -> Result<String, ()> {
    let Some(object) = body.as_object() else {
        // `body.get(...)` on a JSON list/string/number is an `AttributeError`.
        return Err(());
    };
    for key in keys {
        let Some(value) = object.get(*key) else { continue };
        let truthy = match value {
            Value::Null => false,
            Value::Bool(b) => *b,
            Value::Number(n) => n.as_f64().map(|v| v != 0.0).unwrap_or(true),
            Value::String(s) => !s.is_empty(),
            Value::Array(a) => !a.is_empty(),
            Value::Object(m) => !m.is_empty(),
        };
        if !truthy {
            continue;
        }
        match value {
            Value::String(s) => return Ok(s.clone()),
            _ => return Err(()),
        }
    }
    Ok(fallback.to_string())
}

/// `Handler._do_save` (`readmd.py:3529-3562`).
///
/// Three things the previous port got wrong at once: it treated `/api/save` as
/// POST-only, it ran the path through the kernel's invented allowed-root list,
/// and it answered with `content::describe()`'s document payload plus two
/// invented keys (`saved`, `bytes`).  Python answers with `save_text_atomic`'s
/// four-key result dict and the LegacyError envelope (`{'error': ...}`, no `ok`
/// and no `error_code`) on every one of its four early exits.
fn h_save(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    let invalid = || ApiError::legacy_error(400, "无效请求");
    // `n = int(self.headers.get('Content-Length', 0) or 0)` — an absent or blank
    // header is `0`, and `0` makes `_read_request_body_limited` return `b''`,
    // which `json.loads` rejects.  So *any* unusable body is the same 400.
    let header = req.header("content-length").unwrap_or("").trim();
    let declared: i64 = if header.is_empty() {
        0
    } else {
        match header.parse::<i64>() {
            Ok(v) => v,
            Err(_) => return Err(invalid()),
        }
    };
    const SAVE_BODY_LIMIT: i64 = 50 * 1024 * 1024;
    if declared < 0 || declared > SAVE_BODY_LIMIT {
        // `ValueError('request_too_large')` (`readmd.py:1439`).
        return Err(invalid());
    }
    if req.body.len() as i64 != declared {
        // `ValueError('incomplete_request')` (`readmd.py:1449`).
        return Err(invalid());
    }
    let payload: Value = match serde_json::from_slice(if declared == 0 { b"null" } else { &req.body }) {
        Ok(v) => v,
        Err(_) => return Err(invalid()),
    };
    if declared == 0 {
        // `json.loads(b'')` raises `JSONDecodeError`, i.e. the same 400 — the
        // `null` above only exists to keep the `as_object()` probe honest.
        return Err(invalid());
    }
    let internal_error = || ApiError::plain_text(500, "internal error");
    let raw_path = match py_or_string(&payload, &["path", "file"], "") {
        Ok(v) => v,
        Err(_) => return Err(internal_error()),
    };
    let text = py_or_string(&payload, &["content"], "").unwrap_or_default();
    // `enc = body.get('encoding') or 'utf-8'` — the encoding `/api/file`
    // reported; the file is written back in it (see `text_encoding`).
    let enc = py_or_string(&payload, &["encoding"], "utf-8").unwrap_or_else(|_| "utf-8".into());
    if enc.is_empty() {
        return Err(internal_error());
    }
    let expected_mtime = match payload.as_object().and_then(|o| o.get("expected_mtime")) {
        None | Some(Value::Null) => None,
        Some(v) => match v.as_f64() {
            Some(n) => Some(n),
            None => return Ok(Response::json_status(
                500,
                &json!({ "ok": false, "error": "a non-numeric expected_mtime raises in `float()`" }),
            )),
        },
    };
    if raw_path.is_empty() {
        // `readmd.py:3540-3542`.
        return Err(ApiError::legacy_error(400, "缺少文件路径"));
    }
    // `readmd.py:3544` — `os.path.realpath(os.path.normpath(path))`; the kernel
    // applies the second `unquote()` only to the *query* `p`, never to a body
    // field, so this is the raw typed string.
    let safe_path = paths::canonicalize_or_clean(&py_abspath(&raw_path));
    let safe_key = real_key(&safe_path);
    let mut authorized = save_path_authorized(&safe_key);
    // `readmd.py:3545-3553` — a save-authorized origin authorizes an
    // `AI*`-prefixed sibling in the same directory.
    if !authorized {
        if let Ok(origin) = py_or_string(&payload, &["origin_path", "source_file"], "") {
            if !origin.is_empty() {
                let safe_origin = paths::canonicalize_or_clean(&py_abspath(&origin));
                if save_path_authorized(&real_key(&safe_origin))
                    && py_dirname(&safe_origin.to_string_lossy())
                        == py_dirname(&safe_path.to_string_lossy())
                {
                    let base = py_basename(&safe_path.to_string_lossy());
                    let is_derived = base
                        .strip_prefix("AI")
                        .map(|rest| {
                            let digits: String = rest
                                .chars()
                                .take_while(|c| c.is_ascii_digit())
                                .collect();
                            matches!(rest[digits.len()..].chars().next(), Some('-'))
                        })
                        .unwrap_or(false);
                    let wanted = format!(".{}", content::ext_of(&safe_path));
                    if is_derived
                        && PY_SAVE_EXTENSIONS
                            .iter()
                            .any(|e| e.eq_ignore_ascii_case(&wanted.to_ascii_lowercase()))
                    {
                        remember_authorized_save(&safe_path);
                        authorized = true;
                    }
                }
            }
        }
    }
    let wanted = format!(".{}", content::ext_of(&safe_path));
    let ext_ok = PY_SAVE_EXTENSIONS
        .iter()
        .any(|e| e.eq_ignore_ascii_case(&wanted.to_ascii_lowercase()));
    if !authorized || !ext_ok {
        // `readmd.py:3554-3556` — the one containment answer in the legacy HTTP
        // API, in LegacyError shape: `{'error': '文件未被授权保存'}`.
        return Err(ApiError::legacy_error(403, "文件未被授权保存"));
    }
    let result = py_save_text_atomic(&safe_path, &text, &enc, expected_mtime);
    let status = if result.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
        200
    } else if result.get("conflict").and_then(|v| v.as_bool()).unwrap_or(false) {
        409
    } else if result.get("error_code").and_then(|v| v.as_str()) == Some("encoding_unrepresentable") {
        422
    } else {
        500
    };
    if status == 200 {
        // Kernel-side only: refresh the derived store.  `readmd.py` reindexes
        // lazily, and none of these writes reach the response body.
        let _ = content::index(app, &safe_path);
    }
    Ok(Response::json_status(status, &result))
}

/// `Handler._do_upload` (`readmd.py:3495-3527`).
///
/// The whole body — `int(Content-Length)`, `rfile.read`, `makedirs`, the write —
/// sits in one `try`, so every escape is
/// `500 {'error': '上传失败：%s' % e}` (`readmd.py:3526-3527`), in the
/// LegacyError shape with the exception text inside `error`.  Success is
/// `200 {'path': target}` — the single key `readmd.py:3524` sends; the kernel's
/// earlier `ok`/`name`/`size`/`url`/`displayPath`/`relPath` body was invented.
fn h_upload(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    let failed = |e: String| ApiError::legacy_error(500, format!("上传失败：{e}"));
    let ext = req.q("ext").unwrap_or("").to_string();
    // `qs.get('name', [''])[0] or qs.get('filename', [''])[0]` (`readmd.py:1230`).
    let name = {
        let first = req.q("name").unwrap_or("");
        if first.is_empty() {
            req.q("filename").unwrap_or("").to_string()
        } else {
            first.to_string()
        }
    };
    let suffix = |ext: &str| -> String {
        if ext.is_empty() {
            ".bin".to_string()
        } else if ext.starts_with('.') {
            ext.to_string()
        } else {
            format!(".{ext}")
        }
    };
    let dir = app.paths.uploads_dir();
    let bytes = req.body.clone();
    if bytes.is_empty() {
        // `if not data:` (`readmd.py:3500-3502`).
        return Err(ApiError::legacy_error(400, "空文件"));
    }
    let target = if !name.is_empty() {
        // `re.sub(r'[\\/*?:"<>|]', '_', name).strip()` (`readmd.py:3506-3507`).
        let safe_name: String = name
            .chars()
            .map(|c| match c {
                '\\' | '/' | '*' | '?' | '"' | ':' | '<' | '>' | '|' => '_',
                other => other,
            })
            .collect();
        let safe_name = safe_name.trim().to_string();
        let stem = if safe_name.is_empty() {
            format!("document{}", suffix(&ext))
        } else {
            safe_name
        };
        dir.join(stem)
    } else {
        dir.join(format!("{}{}", uuid_hex(req), suffix(&ext)))
    };
    if let Some(parent) = target.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            return Err(failed(e.to_string()));
        }
    }
    if let Err(e) = content::write_bytes_atomic(&target, &bytes) {
        return Err(failed(e.to_string()));
    }
    // `readmd.py:3517-3522` — a stale converted `.md` sibling is dropped so the
    // freshly uploaded bytes are what the reader opens.
    if content::ext_of(&target) != "md" {
        let sibling = target.with_extension("md");
        if sibling.is_file() && sibling != target {
            let _ = std::fs::remove_file(&sibling);
        }
    }
    Ok(Response::json(&json!({ "path": target.to_string_lossy() })))
}

/// `uuid.uuid4().hex` stand-in: the kernel has no uuid crate vendored, and
/// `_do_upload` only needs 32 unpredictable hex characters.
fn uuid_hex(req: &Request) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut seed = (nanos as u64) ^ ((nanos >> 64) as u64) ^ ((req.body.len() as u64) << 32);
    let mut out = String::with_capacity(32);
    for _ in 0..4 {
        // SplitMix64 step, rendered as 8 hex digits per round.
        seed = seed.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = seed;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        out.push_str(&format!("{:016x}", z ^ (z >> 31)));
    }
    out.truncate(32);
    out
}


/// `readmd.py:127` — `ALL_TEXT_EXTS = MD_EXTS + CODE_CONFIG_EXTS`, spelled out
/// with the leading dot exactly the way `Api.rename_file` tests it.
///
/// Deliberately *not* a re-use of [`content::TEXT_EXTS`]: that table is the
/// kernel's syntax-highlighting set and it both over- and under-covers Python's
/// list (it adds `graphql`, `zig`, `svg`, `cmake`… and drops `json5`, `rst`,
/// `adoc`, `bib`, `err`, `out`, `diff`, `patch`, `fish`, `psm1`…), so sharing it
/// would accept renames `Api.rename_file` rejects and reject renames it accepts.
const PY_ALL_TEXT_EXTS: &[&str] = &[
    ".md", ".markdown", ".mdown", ".mkd", ".mdx", ".txt", ".toml", ".yaml", ".yml", ".json",
    ".json5", ".jsonc", ".ini", ".cfg", ".conf", ".config", ".env", ".properties", ".xml",
    ".plist", ".inf", ".bat", ".cmd", ".ps1", ".psm1", ".sh", ".bash", ".zsh", ".fish", ".vbs",
    ".py", ".js", ".mjs", ".cjs", ".ts", ".tsx", ".jsx", ".c", ".cpp", ".h", ".hpp", ".cc",
    ".cxx", ".cs", ".java", ".kt", ".kts", ".rs", ".go", ".rb", ".php", ".swift", ".lua", ".r",
    ".m", ".dart", ".sql", ".dockerfile", ".makefile", ".gradle", ".html", ".htm", ".css",
    ".scss", ".sass", ".less", ".vue", ".svelte", ".log", ".out", ".err", ".diff", ".patch",
    ".gitignore", ".gitattributes", ".editorconfig", ".npmrc", ".rst", ".asciidoc", ".adoc",
    ".bib", ".csv", ".tsv",
];

/// `readmd.py:497-501` — `_WINDOWS_RESERVED_NAMES`, the `COM`/`LPT` families
/// expanded the way the generator expressions expand them.
const PY_WINDOWS_RESERVED_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// `os.path.splitext` on the character array, so the "skip all leading dots"
/// rule of the CPython implementation survives: `.gitignore` has no extension,
/// `..a` has none either, `a.b.` splits as `("a.b", ".")`.
fn py_splitext(path: &str) -> (String, String) {
    let chars: Vec<char> = path.chars().collect();
    let sep_index = chars
        .iter()
        .rposition(|c| *c == '/' || *c == '\\')
        .map(|i| i as isize)
        .unwrap_or(-1);
    let dot_index = chars
        .iter()
        .rposition(|c| *c == '.')
        .map(|i| i as isize)
        .unwrap_or(-1);
    if dot_index > sep_index {
        let mut probe = (sep_index + 1) as usize;
        while (probe as isize) < dot_index {
            if chars[probe] != '.' {
                let split = dot_index as usize;
                let stem: String = chars[..split].iter().collect();
                let ext: String = chars[split..].iter().collect();
                return (stem, ext);
            }
            probe += 1;
        }
    }
    (path.to_string(), String::new())
}

/// `readmd.py:505-518` — `_validate_rename_stem`.  The `Err` carries the
/// `ValueError` text, which is what `Api.rename_file` puts in `error` verbatim.
fn validate_rename_stem(stem: &str, extension: &str) -> Result<String, String> {
    if stem.is_empty() || stem != stem.trim_matches(|c: char| c.is_whitespace()) {
        return Err("文件名不能为空或以空格开头、结尾".to_string());
    }
    if stem.ends_with('.') || stem.chars().any(|c| (c as u32) < 32) {
        return Err("文件名包含无效字符".to_string());
    }
    if "<>:\"/\\|?*".chars().any(|c| stem.contains(c)) {
        return Err("文件名不能包含 < > : \" / \\ | ? *".to_string());
    }
    let head = stem.split('.').next().unwrap_or_default().to_uppercase();
    if PY_WINDOWS_RESERVED_NAMES.contains(&head.as_str()) {
        return Err("该名称是 Windows 系统保留名".to_string());
    }
    if stem.chars().count() + extension.chars().count() > 255 {
        return Err("文件名过长".to_string());
    }
    Ok(stem.to_string())
}

/// `readmd.py:521-523` — `_paths_equal`.  Both operands are already absolute
/// here (`AppPaths::resolve_doc` canonicalises), so only `normcase` is left.
fn paths_equal(left: &Path, right: &Path) -> bool {
    crate::validators::normcase(&left.to_string_lossy())
        == crate::validators::normcase(&right.to_string_lossy())
}

/// `readmd.py:525-532` — `_same_file_target`: case-insensitive equality first,
/// then the real `os.path.samefile` probe for symlinked or duplicated targets.
fn same_file_target(left: &Path, right: &Path) -> bool {
    if paths_equal(left, right) {
        return true;
    }
    if !left.exists() || !right.exists() {
        return false;
    }
    match (std::fs::canonicalize(left), std::fs::canonicalize(right)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

static RENAME_TICK: AtomicU64 = AtomicU64::new(0);

/// `secrets.token_hex(n)` — `2 * n` lowercase hex characters.  Only used for the
/// suffix of the two-step case-only rename, so a monotonic counter mixed into
/// the clock is enough entropy for a name nobody resolves.
fn token_hex(n: usize) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let tick = RENAME_TICK.fetch_add(1, Ordering::Relaxed);
    let mut seed = nanos ^ tick.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (std::process::id() as u64);
    let mut out = String::with_capacity(n * 2);
    for _ in 0..n {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        out.push_str(&format!("{:02x}", (seed & 0xff) as u8));
    }
    out
}

/// `os.path.basename` on an already-resolved path.
fn basename_of(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_string()
}

/// `Api.rename_file` returns a plain dict through pywebview, so every failure is
/// a `200` carrying `{'ok': False, 'code': …, 'error': …}` — the `code`+`error`
/// family, not `_send_api_error`'s `error_code`.
fn rename_failure(code: &str, error: &str) -> ApiResult<Response> {
    Ok(Response::json(&json!({
        "ok": false,
        "code": code,
        "error": error,
    })))
}

/// KERNEL BRIDGE — not a parity route.  `/api/rename` exists only so the
/// pywebview shim in `main.rs` (`rename_file: async function(from, to)`) has
/// something to POST to when the UI calls `py.rename_file(tab.path, newTitle)`;
/// the Python HTTP dispatcher `readmd.py::_route` has no `/api/rename`, so that
/// path answers `404 not found` there.
///
/// Behaviour mirrors `Api.rename_file` (`readmd.py:4732-4824`) rather than a
/// generic "move this file there" endpoint: the second argument is a **bare
/// stem**, `new_path` is `os.path.dirname(old_path) + stem + old_extension`, the
/// stem is screened by `_validate_rename_stem`, a case-only rename goes through
/// a temporary name, the `.bak` sidecar follows, and the result carries Python's
/// `warnings` list.  The destination is never resolved through
/// `AppPaths::resolve_doc`, which is what previously let a bare stem land in the
/// workspace root instead of next to its source.
///
/// Kernel-only deviations, listed so a parity audit can skip them: (1) the
/// source argument goes through `resolve_doc` because the UI holds
/// workspace-relative display paths where Python's `os.path.abspath` suffices;
/// (2) `check_allowed` is a kernel guard with no Python counterpart and answers
/// `403 path_forbidden`; (3) the two-step rename suffix is a counter+xorshift
/// token, not `secrets.token_hex`; (4) the back-reference sync drives the
/// kernel's SQLite index and `settings.json` rather than `RECENT_FILE` /
/// `SETTINGS_FILE` / `HISTORY_FILE` JSON, and `chat_messages` has no `doc`
/// column, so Python's `'AI 历史文档引用未能同步'` branch has nothing to fail
/// against here; (5) the `rename_failed` `error` text is `std::io::Error`'s
/// `Display`, not Python's `[WinError n] …: 'src' -> 'dst'`.
fn h_rename(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    let from_raw = req
        .field("path")
        .or_else(|| req.field("from"))
        .or_else(|| req.q("p").map(|s| s.to_string()))
        .unwrap_or_default();
    // `to` is the spelling `main.rs`'s bridge sends; it still carries a *stem*,
    // exactly like the `new_stem` positional `Api.rename_file` receives.
    let stem_raw = req
        .field("new_stem")
        .or_else(|| req.field("stem"))
        .or_else(|| req.field("to"))
        .unwrap_or_default();

    let old_path = match app.paths.resolve_doc(&from_raw) {
        Ok(p) => p,
        Err(_) => return rename_failure("not_found", "文件不存在或已被移动"),
    };
    if from_raw.is_empty() || !old_path.is_file() {
        return rename_failure("not_found", "文件不存在或已被移动");
    }
    // `os.path.splitext(old_path)[1]` — the extension keeps its original case and
    // is only lower-cased for the membership test.
    let extension = py_splitext(&old_path.to_string_lossy()).1;
    if !PY_ALL_TEXT_EXTS.contains(&extension.to_lowercase().as_str()) {
        return rename_failure("unsupported_type", "只能重命名 Markdown 或文本/代码文件");
    }
    let stem = match validate_rename_stem(&stem_raw, &extension) {
        Ok(s) => s,
        Err(message) => return rename_failure("invalid_name", &message),
    };

    let parent = old_path.parent().unwrap_or_else(|| Path::new("."));
    let new_path = parent.join(format!("{stem}{extension}"));
    if old_path == new_path {
        // Python's no-op branch really does omit `old_path` from the dict.
        return Ok(Response::json(&json!({
            "ok": true,
            "path": old_path.to_string_lossy(),
            "name": basename_of(&old_path),
            "warnings": [],
        })));
    }
    let case_only = same_file_target(&old_path, &new_path);
    if new_path.exists() && !case_only {
        return rename_failure("target_exists", "同目录下已存在同名文件");
    }
    app.paths
        .check_allowed(&new_path)
        .map_err(|_| ApiError::forbidden("path_forbidden"))?;

    let rename_result = if case_only {
        let temp = PathBuf::from(format!(
            "{}.readmd-rename-{}",
            old_path.to_string_lossy(),
            token_hex(6)
        ));
        match std::fs::rename(&old_path, &temp) {
            Ok(()) => std::fs::rename(&temp, &new_path).map_err(|e| {
                let _ = std::fs::rename(&temp, &old_path);
                e
            }),
            Err(e) => Err(e),
        }
    } else {
        std::fs::rename(&old_path, &new_path)
    };
    if let Err(e) = rename_result {
        return rename_failure("rename_failed", &e.to_string());
    }

    let mut warnings: Vec<String> = Vec::new();
    let old_backup = PathBuf::from(format!("{}.bak", old_path.to_string_lossy()));
    let new_backup = PathBuf::from(format!("{}.bak", new_path.to_string_lossy()));
    if old_backup.is_file() {
        let backup_case_only = same_file_target(&old_backup, &new_backup) && old_backup != new_backup;
        if backup_case_only {
            let temp = PathBuf::from(format!(
                "{}.readmd-rename-{}",
                old_backup.to_string_lossy(),
                token_hex(6)
            ));
            let moved = std::fs::rename(&old_backup, &temp).and_then(|()| {
                std::fs::rename(&temp, &new_backup).map_err(|e| {
                    let _ = std::fs::rename(&temp, &old_backup);
                    e
                })
            });
            if moved.is_err() {
                warnings.push("文件已重命名，但备份文件大小写未能同步".to_string());
            }
        } else if new_backup.exists() {
            warnings.push("旧备份未移动：目标备份已存在".to_string());
        } else if std::fs::rename(&old_backup, &new_backup).is_err() {
            warnings.push("文件已重命名，但旧备份未能同步移动".to_string());
        }
    }

    // Back-reference sync: `Api.rename_file`'s passes over the recent list,
    // `settings['last']` and the AI history, against the kernel's own stores.
    let sync_result: Result<(), String> = (|| {
        let from_display = app.paths.display_path(&old_path);
        let to_display = app.paths.display_path(&new_path);
        app.store
            .rename_document(&from_display, &to_display)
            .map_err(|e| e.to_string())?;
        let _ = content::index(app, &new_path);
        let last_still_points_here = app
            .setting("last")
            .as_str()
            .map(PathBuf::from)
            .is_some_and(|last| paths_equal(&last, &old_path));
        if last_still_points_here {
            app.update_settings(&json!({ "last": new_path.to_string_lossy() }));
        }
        Ok(())
    })();
    if sync_result.is_err() {
        warnings.push("文件已重命名，但部分历史记录未能同步".to_string());
    }

    Ok(Response::json(&json!({
        "ok": true,
        "path": new_path.to_string_lossy(),
        "name": basename_of(&new_path),
        "old_path": old_path.to_string_lossy(),
        "warnings": warnings,
    })))
}





fn h_recent_status(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    // `_api_recent_status` (`readmd.py:2243`-`2261`): a bad body is a
    // `ValueError` -> `400 {'ok':False,'code':'invalid_recent_paths'}` and any
    // other failure is the blanket `except Exception` ->
    // `500 {'ok':False,'code':'recent_status_failed'}`.  Note the key is
    // `code`, because this family never calls `_send_api_error`.
    let limit: usize = req.field("limit").and_then(|v| v.parse().ok()).unwrap_or(30).clamp(1, 200);
    let Ok(items) = app.store.recent(limit) else {
        return recent_error(500, "recent_status_failed");
    };
    let mapped: Vec<Value> = items
        .iter()
        .map(|r| {
            let joined = app.paths.workspace.join(&r.path);
            let abs = if joined.exists() { joined } else { PathBuf::from(&r.path) };
            json!({
                "path": r.path,
                "absPath": abs.to_string_lossy(),
                "title": r.title,
                "name": Path::new(&r.path).file_name().and_then(|n| n.to_str()).unwrap_or(""),
                "openedAt": r.opened_at,
                "exists": abs.exists(),
            })
        })
        .collect();
    let paths_only: Vec<&str> = mapped.iter().filter_map(|v| v.get("path").and_then(|p| p.as_str())).collect();
    let _ = paths_only;
    ok_json(json!({
        "ok": true,
        "items": mapped,
        "recent": mapped,
    }))
}

fn h_recent_add(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    // `_api_recent_add` (`readmd.py:2263`-`2276`).  The only validation is
    // `isinstance(path, str) and path and len(path) <=
    // Api.MAX_RECENT_PATH_LENGTH` (`readmd.py:3710`), and Python never resolves
    // the value against a document root, so neither may this handler: an
    // out-of-root path is simply remembered.
    let body = body_value(req);
    let invalid = match body.get("path") {
        Some(Value::String(s)) => s.is_empty() || s.chars().count() > 4096,
        _ => true,
    };
    if invalid {
        return recent_error(400, "invalid_recent_path");
    }
    let raw = body["path"].as_str().unwrap_or_default();
    let display = if Path::new(raw).is_absolute() {
        app.paths.display_path(&paths::canonical_existing(Path::new(raw)).unwrap_or_else(|_| PathBuf::from(raw)))
    } else {
        raw.replace('\\', "/")
    };
    let title = Path::new(raw)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("untitled")
        .to_string();
    if app.store.touch_recent(&display, &title).is_err() {
        // `readmd.py:2275` — blanket `except Exception`.
        return recent_error(500, "recent_add_failed");
    }
    ok_json(json!({ "ok": true }))
}

fn h_recent_remove(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    // `_api_recent_remove` (`readmd.py:2285`-`2297`): `body['path']` only, the
    // same `MAX_RECENT_PATH_LENGTH` gate as add, and the success body is
    // exactly `{'ok': <bool>}`.
    let body = body_value(req);
    let invalid = match body.get("path") {
        Some(Value::String(s)) => s.is_empty() || s.chars().count() > 4096,
        _ => true,
    };
    if invalid {
        return recent_error(400, "invalid_recent_path");
    }
    let raw = body["path"].as_str().unwrap_or_default();
    let display = if Path::new(raw).is_absolute() {
        app.paths.display_path(&paths::canonical_existing(Path::new(raw)).unwrap_or_else(|_| PathBuf::from(raw)))
    } else {
        raw.replace('\\', "/")
    };
    let removed = app.store.remove_recent(&display).is_ok();
    if !removed {
        // `readmd.py:2297` — blanket `except Exception`.
        return recent_error(500, "recent_remove_failed");
    }
    ok_json(json!({ "ok": true }))
}

fn h_recent_clear(app: &Arc<App>, _req: &Request) -> ApiResult<Response> {
    // `_api_recent_clear` (`readmd.py:2277`-`2284`): body is exactly `{'ok':
    // bool(clear_recent())}`.
    if app.store.clear_recent().is_err() {
        return recent_error(500, "recent_clear_failed");
    }
    ok_json(json!({ "ok": true }))
}

/// `readmd.py:2082-2100` — `_api_links_index`.
///
/// * **POST-only.**  `_route()` reaches the handler for any verb and the handler
///   itself answers `405 method_not_allowed` (`readmd.py:2083-2085`), so the gate
///   lives here, not in the dispatcher.
/// * `dir` is `str(body.get('dir', '')).strip()` — an absent key or an explicit
///   `null` is the empty string, and any other JSON type is stringified first.
///   Numbers render the same as CPython (`5` → `"5"`); booleans and containers
///   do not (`True` versus `true`, `['a']` versus `["a"]`), but that difference
///   cannot surface: neither spelling ever names an existing directory, so both
///   roads end on the same `404 dir_not_found`.  Empty-or-not-a-directory is
///   that `404`, and the check runs *before* the indexer is imported.
/// * `force` is `bool(body.get('force', False))`, i.e. **Python truthiness**, not
///   `Option::is_none()`: `''`, `0`, `false`, `null`, `[]` and `{}` are all
///   false, while the string `"false"` is true.  This is deliberately not the
///   `"1"|"true"|"yes"|"on"` numeric flag parser used elsewhere in this file.
/// * success body is exactly `{'ok': True, 'stats': stats}`.  The previous port
///   answered the invented `{ok, indexed, skipped, errors, stats}` shape from
///   `content::reindex_workspace` and a clamped `limit` Python never reads, and
///   `stats` could even be JSON `null`.
fn h_links_index(_app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    if req.method != "POST" {
        return Err(ApiError::new(405, "method_not_allowed"));
    }
    // `_read_json_body(1024 * 1024)` (`readmd.py:1463-1469`): a body of zero
    // announced bytes is `{}`, and a malformed body raises `JSONDecodeError`,
    // which is a `ValueError` and therefore lands on the `400` arm.  The status
    // is kept; the code is the stable `invalid_json` rather than Python's
    // decoder sentence, which would put parser prose on the wire.
    let body = if req.body.is_empty() {
        json!({})
    } else {
        match serde_json::from_slice::<Value>(&req.body) {
            Ok(value) => value,
            Err(_) => return Err(ApiError::bad_request("invalid_json")),
        }
    };
    // `body.get(...)` on a non-mapping is an `AttributeError` -> the blanket
    // `except Exception` -> `500 links_index_failed`.
    let Some(map) = body.as_object() else {
        return Err(ApiError::internal("links_index_failed"));
    };
    let raw = match map.get("dir") {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
    };
    let directory = raw
        .trim_matches(py_isspace as fn(char) -> bool)
        .to_string();
    if directory.is_empty() || !Path::new(&directory).is_dir() {
        return Err(ApiError::not_found("dir_not_found"));
    }
    let force = py_truthy(map.get("force").unwrap_or(&Value::Bool(false)));
    // `link_indexer.get_indexer()` then `indexer.index_directory(dir, force=)`;
    // `None` keeps the singleton on `DATA_DIR/index/link_index.db` exactly like
    // the reference.
    let indexer = match crate::link_indexer::get_indexer(None) {
        Ok(indexer) => indexer,
        Err(_) => return Err(ApiError::internal("links_index_failed")),
    };
    match indexer.index_directory(&directory, force) {
        Ok(stats) => ok_json(json!({ "ok": true, "stats": stats.to_json() })),
        // Python splits this arm into `400 str(ValueError)` and
        // `500 links_index_failed`; the kernel's `index_directory` reports one
        // opaque `String`, so every failure is the 500 and the message stays in
        // diagnostics (`noted("detail")` never reaches the body).
        Err(detail) => Err(ApiError::internal("links_index_failed").noted("detail", detail)),
    }
}

/// Every arm runs inside Python's blanket `except Exception` -> `500
/// links_graph_failed`, and `get_indexer()` failing is part of that same arm.
fn h_links_graph(_app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    // `_api_links_graph` (`readmd.py:2103`-`2117`): reads **only** `dir` and
    // `max_nodes` from the query, and answers the *nested* shape
    // `{'ok': True, 'graph': indexer.get_graph_data(...)}`.  The previous port
    // served `app.store.graph()` -- a `docs`/`links(src,dst)` schema the
    // reference never touches -- flattened `ok` in at the top level, and
    // `assets/js/features/graph.js:243` (`if (!res?.ok || !res.graph) throw`)
    // therefore could not open 知识图谱 at all (R7 §B7).
    let directory = req.q("dir").map(|s| s.trim_matches(py_isspace as fn(char) -> bool).to_string()).unwrap_or_default();
    // `directory or None`: Python truthiness, so a whitespace-only `dir` is
    // *unscoped*, not a scope of `''`.
    let root_dir = if directory.is_empty() { None } else { Some(directory.as_str()) };
    let max_raw = req.q("max_nodes").map(|s| s.trim_matches(py_isspace as fn(char) -> bool).to_string()).unwrap_or_default();
    let max_nodes = match max_raw.replace('_', "").parse::<i64>() {
        Ok(v) => v.max(10).min(2000),
        Err(_) => 500,
    };
    let Ok(indexer) = crate::link_indexer::get_indexer(None) else {
        return Err(ApiError::internal("links_graph_failed"));
    };
    match indexer.get_graph_data(root_dir, max_nodes) {
        Ok(graph) => ok_json(json!({ "ok": true, "graph": graph })),
        Err(_) => Err(ApiError::internal("links_graph_failed")),
    }
}

fn h_links_backlinks(_app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    // `_api_links_backlinks` (`readmd.py:2119`-`2137`).  Python hands the
    // caller's raw string to `indexer.get_backlinks()` and echoes it back
    // **verbatim** in `path`; there is no allowed-root resolution, so an
    // out-of-root path is simply "no backlinks", never a 403.  The response
    // carries `forward_links` too -- `graph.js:265` reads it for the 出站 tab.
    let file_path = req.q("path").map(|s| s.trim_matches(py_isspace as fn(char) -> bool).to_string()).unwrap_or_default();
    if file_path.is_empty() {
        return Err(ApiError::bad_request("missing_path"));
    }
    let Ok(indexer) = crate::link_indexer::get_indexer(None) else {
        return Err(ApiError::internal("links_backlinks_failed"));
    };
    let backlinks = match indexer.get_backlinks(&file_path) {
        Ok(backlinks) => backlinks,
        Err(_) => return Err(ApiError::internal("links_backlinks_failed")),
    };
    let forward_links = match indexer.get_forward_links(&file_path) {
        Ok(forward_links) => forward_links,
        Err(_) => return Err(ApiError::internal("links_backlinks_failed")),
    };
    ok_json(json!({
        "ok": true,
        "path": file_path,
        "backlinks": backlinks,
        "forward_links": forward_links,
    }))
}

fn h_links_deadlinks(_app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    // `_api_links_deadlinks` (`readmd.py:2139`-`2148`): `dir` **is** read, so an
    // empty query is not equivalent to Python's unscoped scan, and the body is
    // exactly `{'ok': True, 'deadlinks': deadlinks}`.  The previous port ignored
    // the request entirely and served `{ok,deadlinks,items,count}` from
    // `app.store.deadlinks()`, whose rows are `{source,target}` instead of
    // Python's `{link_id,source_path,source_title,target_raw,target_clean,
    // alias,line_no}`.
    let directory = req.q("dir").map(|s| s.trim_matches(py_isspace as fn(char) -> bool).to_string()).unwrap_or_default();
    let root_dir = if directory.is_empty() { None } else { Some(directory.as_str()) };
    let Ok(indexer) = crate::link_indexer::get_indexer(None) else {
        return Err(ApiError::internal("links_deadlinks_failed"));
    };
    match indexer.get_deadlinks(root_dir) {
        Ok(deadlinks) => ok_json(json!({ "ok": true, "deadlinks": deadlinks })),
        Err(_) => Err(ApiError::internal("links_deadlinks_failed")),
    }
}



/// KERNEL BRIDGE — not a parity route.  `readmd.py`'s HTTP dispatcher
/// (`readmd.py:1186`-`1378`) has **no** `/api/settings`; this path exists only so
/// the injected `main.rs` shim can reach the pywebview `Api` object, and it must
/// therefore mirror that object, not an HTTP contract.
///
/// `Api.get_settings` (`readmd.py:5030`) is literally
/// `return load_json(SETTINGS_FILE, {})` — the settings map **bare**, with no
/// `ok`/`settings`/`engine` envelope.  `main.rs`'s `get_settings` does
/// `return await res.json()` and `assets/js/core/settings.js` feeds the result
/// straight into `Object.assign(state, s)`, so an envelope made every key
/// invisible: `theme`, `fontSize`, `lineWidth`, `aiPanelWidth`, `autoReload`,
/// `pvLayout`, `pvSync`, `pvSplitX`, `pvSplitY` all fell back to defaults on
/// every launch and `restoreLastFile` (`assets/app.js`) never fired.
///
/// POST keeps the `{ok,settings}` envelope: `Api.save_settings`
/// (`readmd.py:5792`) is a merge whose result no shipped caller reads, and
/// R7 verified the merge semantics clean, so only the read side is bare.
fn h_settings(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    if req.method == "GET" {
        let key = req.q("key").or_else(|| req.q("k"));
        return match key {
            // Kernel extra: no Python `Api` method takes a key.  Kept because
            // it is additive and a single-key probe is useful to the host.
            Some(k) => ok_json(json!({ "ok": true, "key": k, "value": app.setting(k) })),
            None => Ok(Response::json(&app.settings_all())),
        };
    }
    let patch = body_value(req);
    let merged = app.update_settings(&patch);
    ok_json(json!({ "ok": true, "settings": merged }))
}


/// CPython's `str.strip()` whitespace set: the Unicode `White_Space` property
/// plus U+001C..U+001F, which `str.isspace()` also counts and Rust's
/// `char::is_whitespace` does not.
fn py_isspace(c: char) -> bool {
    c.is_whitespace() || matches!(c, '\u{1c}'..='\u{1f}')
}

/// CPython's `bool(<JSON value>)`.  Not `Option::is_none()`, and not the
/// `"1"|"true"|"yes"|"on"` parser the query-string flags in this file use:
/// `''`, `0`, `0.0`, `false`, `null`, `[]` and `{}` are false, and every other
/// value — including the strings `"false"` and `"0"` — is true.
fn py_truthy(value: &Value) -> bool {
    match value {
        Value::Null | Value::Bool(false) => false,
        Value::Bool(true) => true,
        Value::String(s) => !s.is_empty(),
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Value::Array(items) => !items.is_empty(),
        Value::Object(items) => !items.is_empty(),
    }
}

/// `style_injector.find_custom_file(name, workspace_dir=None)`
/// (`style_injector.py:14-40`).  Neither style route ever passes a workspace, so
/// rung 1 is dead and the probe is `<data dir>/<name>` then
/// `~/.readmd/<name>`.  `<data dir>` is `config.DATA_DIR` — i.e.
/// `paths::data_dir()`, which honours `READMD_DATA_DIR` exactly like
/// `style_injector.py:24-27` — and is what `AppPaths::data_dir` carries.
fn custom_style_file(app: &App, name: &str) -> Option<PathBuf> {
    let data_path = app.paths.data_dir.join(name);
    if data_path.is_file() {
        return Some(data_path);
    }
    let home_path = paths::home_dir().join(".readmd").join(name);
    if home_path.is_file() {
        return Some(home_path);
    }
    None
}

/// `style_injector.get_custom_css` / `get_custom_head` (`style_injector.py:43-64`):
/// read the file as UTF-8 with `errors='replace'` and `.strip()` it; a missing
/// file, an unreadable file and a read error all give `''`.
fn custom_style_text(path: Option<&Path>) -> String {
    let Some(path) = path else { return String::new() };
    match std::fs::read(path) {
        Ok(bytes) => String::from_utf8_lossy(&bytes)
            .trim_matches(py_isspace as fn(char) -> bool)
            .to_string(),
        Err(_) => String::new(),
    }
}

/// `readmd.py:2209-2215` — `_api_style_get`.  The body is exactly
/// `{'ok': True, 'data': style_injector.get_custom_styles()}` where `data` is
/// the four-key dict `style_injector.py:93-101` builds: `css`, `head`,
/// `css_path`, `head_path`.  `data` is never `null` and never absent, and each
/// `*_path` is the bare file name or `''` — the absolute path is deliberately
/// kept off the wire.
///
/// Python's `500 style_read_failed` arm is not reproduced because it is not
/// reachable: every fallible step of `get_custom_styles` already swallows its
/// own error into `''` (`style_injector.py:31-32`, `:50-51`, `:61-63`), and the
/// two surviving operations (`os.path.basename`, dict construction) cannot raise.
/// The old port's `{style, css}` body — which no shipped client reads, see
/// `assets/app.js:1309` and `main.rs:3198`, both of which want `res.data.css` —
/// and its `app.setting("style")` backing store are gone: the files are the
/// single source of truth on both sides.
fn h_style_get(app: &Arc<App>, _req: &Request) -> ApiResult<Response> {
    let css_path = custom_style_file(app, "custom.css");
    let head_path = custom_style_file(app, "head.html");
    ok_json(json!({
        "ok": true,
        "data": {
            "css": custom_style_text(css_path.as_deref()),
            "head": custom_style_text(head_path.as_deref()),
            "css_path": css_path.map(|_| "custom.css").unwrap_or_default(),
            "head_path": head_path.map(|_| "head.html").unwrap_or_default(),
        }
    }))
}

/// `readmd.py:2218-2227` — `_api_style_save`.  Inputs are `body.get('css', '')`
/// and `body.get('head', '')`, handed to
/// `style_injector.save_custom_styles(css, head_html)`; the response is
/// **exactly** `{'ok': <bool>}` — no `style`, no `saved`.
///
/// Python falsiness, which is where this route kept breaking:
/// * an absent key is `''` and an explicit `null` is `None`; both are falsy, so
///   `f.write(css or '')` (`style_injector.py:119`) writes an **empty file**.
///   That is not "leave the file alone" — saving clears the slot.
/// * a present, truthy, non-string value (`5`, `true`, `[1]`, `{"a":1}`) reaches
///   `f.write(<non-str>)` → `TypeError`, which `save_custom_styles`'s own blanket
///   `except Exception: return False` swallows.  So the answer is
///   `200 {'ok': false}`, **not** a 500 — and because the truncating
///   `open(..., 'w')` already ran, `custom.css` is left empty while `head.html`
///   is untouched.  That partial write is observable and is reproduced.
/// * the route's `except Exception` → `500 style_save_failed` therefore only
///   fires *before* the injector runs: an unparseable body, or valid JSON that
///   is not a mapping (`body.get` on a list/number/`null` is an
///   `AttributeError`).
/// * `n == 0` short-circuits to `body = {}`, so a bodyless POST is a successful
///   "clear both files", never a parse error.
fn h_style_save(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    let body = if req.body.is_empty() {
        json!({})
    } else {
        match serde_json::from_slice::<Value>(&req.body) {
            Ok(value) => value,
            Err(_) => return Err(ApiError::internal("style_save_failed")),
        }
    };
    let Some(map) = body.as_object() else {
        return Err(ApiError::internal("style_save_failed"));
    };
    let css = map.get("css").cloned().unwrap_or_else(|| json!(""));
    let head = map.get("head").cloned().unwrap_or_else(|| json!(""));
    if std::fs::create_dir_all(&app.paths.data_dir).is_err() {
        return ok_json(json!({ "ok": false }));
    }
    // `style_injector.py:118-121` writes custom.css first and head.html second;
    // the first failure aborts the whole save.
    if !write_style_file(&app.paths.data_dir.join("custom.css"), &css) {
        return ok_json(json!({ "ok": false }));
    }
    if !write_style_file(&app.paths.data_dir.join("head.html"), &head) {
        return ok_json(json!({ "ok": false }));
    }
    ok_json(json!({ "ok": true }))
}

/// `with open(path, 'w', encoding='utf-8') as f: f.write(value or '')`, wrapped
/// by `save_custom_styles`'s `except Exception: return False`.
///
/// The order matters: `open(..., 'w')` truncates the file *before* `f.write()`
/// runs, so a `TypeError` from a truthy non-string still leaves an **empty**
/// file behind.  Creating first and only then resolving the value is what makes
/// that observable difference come out the same way here.
fn write_style_file(path: &Path, value: &Value) -> bool {
    use std::io::Write;
    let mut file = match std::fs::File::create(path) {
        Ok(file) => file,
        Err(_) => return false,
    };
    let text = match value {
        Value::Null | Value::Bool(false) | Value::String(_) => {
            value.as_str().map(str::to_string).unwrap_or_default()
        }
        Value::Bool(true) => return false,
        Value::Number(n) => {
            if n.as_f64().map(|f| f != 0.0).unwrap_or(true) {
                return false; // truthy non-string -> TypeError
            }
            String::new()
        }
        Value::Array(items) => {
            if !items.is_empty() {
                return false;
            }
            String::new()
        }
        Value::Object(items) => {
            if !items.is_empty() {
                return false;
            }
            String::new()
        }
    };
    file.write_all(text.as_bytes()).is_ok()
}

fn h_language(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    if req.method == "GET" {
        return ok_json(json!({ "ok": true, "language": app.setting("language"), "engine": "rust" }));
    }
    let lang = req
        .field("lang")
        .or_else(|| req.field("language"))
        .ok_or_else(|| ApiError::bad_request("missing_language"))?;
    if !matches!(lang.as_str(), "zh-CN" | "zh-TW" | "en" | "ja" | "ko" | "fr" | "de" | "es" | "ru") {
        return Err(ApiError::bad_request("unsupported_language").noted("language", lang));
    }
    app.update_settings(&json!({ "language": lang }));
    ok_json(json!({ "ok": true, "language": lang, "applied": true }))
}

fn h_autostart_get(app: &Arc<App>, _req: &Request) -> ApiResult<Response> {
    let enabled = autostart_state(app)?;
    ok_json(json!({ "ok": true, "enabled": enabled }))
}

fn h_autostart_set(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    let enabled = req
        .json()
        .ok()
        .and_then(|v| v.get("enabled").map(crate::desktop_pet::py_truthy))
        .unwrap_or_else(|| {
            req.field("enabled")
                .map(|v| matches!(v.as_str(), "1" | "true" | "yes" | "on"))
                .unwrap_or(false)
        });
    match set_autostart(app, enabled) {
        Ok(()) => ok_json(json!({ "ok": true, "enabled": enabled })),
        Err(e) => ok_json(json!({ "ok": false, "error": e.to_string() })),
    }
}

fn autostart_state(_app: &Arc<App>) -> ApiResult<bool> {
    #[cfg(windows)]
    {
        Ok(crate::native_system::registry_string_at(
            crate::native_system::HKCU,
            AUTOSTART_SUBKEY,
            "ReadMD",
        )
        .is_some())
    }
    #[cfg(target_os = "linux")]
    {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let autostart_file = PathBuf::from(home).join(".config/autostart/io.github.natsummerance.readmd.desktop");
        Ok(autostart_file.is_file())
    }
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let plist_path = PathBuf::from(home).join("Library/LaunchAgents/io.github.natsummerance.readmd.plist");
        Ok(plist_path.is_file())
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        Ok(false)
    }
}

fn set_autostart(_app: &Arc<App>, enabled: bool) -> ApiResult<()> {
    #[cfg(windows)]
    {
        let exe = std::env::current_exe()
            .unwrap_or_else(|_| PathBuf::from("readmd.exe"))
            .to_string_lossy()
            .to_string();
        let ok = if enabled {
            crate::native_system::win_set_reg_string(
                crate::native_system::HKCU,
                AUTOSTART_SUBKEY,
                "ReadMD",
                &exe,
                false,
            )
        } else {
            crate::native_system::win_delete_reg_value(
                crate::native_system::HKCU,
                AUTOSTART_SUBKEY,
                "ReadMD",
            )
        };
        if !ok {
            return Err(ApiError::internal("autostart_failed"));
        }
        Ok(())
    }
    #[cfg(target_os = "linux")]
    {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let autostart_dir = PathBuf::from(home).join(".config/autostart");
        let autostart_file = autostart_dir.join("io.github.natsummerance.readmd.desktop");
        if enabled {
            let _ = std::fs::create_dir_all(&autostart_dir);
            let _ = std::fs::write(&autostart_file, "[Desktop Entry]\nName=ReadMD\nExec=readmd\nType=Application\n");
        } else {
            let _ = std::fs::remove_file(&autostart_file);
        }
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let plist_dir = PathBuf::from(home).join("Library/LaunchAgents");
        let plist_file = plist_dir.join("io.github.natsummerance.readmd.plist");
        if enabled {
            let _ = std::fs::create_dir_all(&plist_dir);
            let exe = std::env::current_exe()
                .unwrap_or_else(|_| PathBuf::from("readmd"))
                .to_string_lossy()
                .to_string();
            let content = format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n    <key>Label</key>\n    <string>io.github.natsummerance.readmd</string>\n    <key>ProgramArguments</key>\n    <array>\n        <string>{}</string>\n    </array>\n    <key>RunAtLoad</key>\n    <true/>\n</dict>\n</plist>\n",
                exe
            );
            let _ = std::fs::write(&plist_file, content);
        } else {
            let _ = std::fs::remove_file(&plist_file);
        }
        Ok(())
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        Err(ApiError::internal("Unsupported platform"))
    }
}

/// `AUTOSTART_SUBKEY`: `RegOpenKeyExW` takes the root HKEY_CURRENT_USER as a
/// separate argument, and subkey path relative to it.
const AUTOSTART_SUBKEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";

fn h_modules(_app: &Arc<App>, _req: &Request) -> ApiResult<Response> {
    ok_json(json!({
        "ok": true,
        "engine": "rust",
        "version": VERSION,
        "modules": {
            "convert": "ready",
            "ocr": "ready",
            "web": "ready",
            "ai": "ready"
        },
        "win7": false
    }))
}

fn h_skills(app: &Arc<App>, _req: &Request) -> ApiResult<Response> {
    let mut skills: Vec<Value> = Vec::new();
    for dir in skill_dirs(app) {
        collect_skills(&dir, app, &mut skills);
    }
    ok_json(json!({ "ok": true, "skills": skills, "count": skills.len(), "engine": "rust" }))
}

fn skill_dirs(app: &Arc<App>) -> Vec<PathBuf> {
    vec![
        app.paths.data_dir.join("skills"),
        app.paths.workspace.join(".readmd/skills"),
        app.paths.assets_dir.join("skills"),
    ]
}

fn collect_skills(dir: &Path, app: &App, out: &mut Vec<Value>) {
    let Ok(read) = std::fs::read_dir(dir) else { return };
    for entry in read.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let stem = Path::new(&name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        if path.is_dir() {
            let manifest = path.join("SKILL.md");
            let alt = path.join("skill.json");
            let target = if manifest.is_file() { Some(manifest) } else if alt.is_file() { Some(alt) } else { None };
            if let Some(target) = target {
                push_skill(out, app, &stem, &target);
            }
        } else if matches!(content::ext_of(&path).as_str(), "md" | "json" | "yaml" | "yml") && !name.starts_with('.') {
            push_skill(out, app, &stem, &path);
        }
    }
}

fn push_skill(out: &mut Vec<Value>, app: &App, id: &str, file: &Path) {
    if id.is_empty() || out.iter().any(|s| s["id"] == json!(id)) {
        return;
    }
    let description = std::fs::read_to_string(file)
        .ok()
        .and_then(|t| {
            t.lines()
                .find(|l| l.trim_start().starts_with("description:"))
                .map(|l| l.split_once(':').map(|(_, v)| v.trim().to_string()).unwrap_or_default())
        })
        .unwrap_or_default();
    out.push(json!({
        "id": id,
        "name": id,
        "title": id,
        "description": description,
        "file": file.to_string_lossy(),
        "path": app.paths.display_path(file),
        "enabled": true,
        "installed": true,
        "source": "local",
    }));
}



fn h_plugins_list(app: &Arc<App>, _req: &Request) -> ApiResult<Response> {
    ok_json(crate::plugin_manager::load_manifest(app))
}

/// `ai.json` + the credential store (OS manager / encrypted vault): the one
/// place AI connections live, shared with `/api/ai/chat`.
fn ai_settings(app: &App) -> crate::ai::AiSettings {
    crate::ai::AiSettings::new(app.paths.data_dir.join("ai.json"), crate::ai::CredentialStore::os_backend())
}

/// `GET` → `{presets, custom, upstream_catalog, current}` with every provider
/// annotated (`has_key`, `key_source`, never `api_key`); `POST {providers?,
/// current?}` stores keys through the credential store, then answers the same view.
fn h_ai_config(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    let catalog = provider_catalog(app);
    let upstream: Vec<Value> = catalog
        .get("upstream_entries")
        .or_else(|| catalog.get("upstream_catalog"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let settings = ai_settings(app);
    let saving = req.method == "POST";
    if saving {
        if let Err(e) = settings.save_config(&body_value(req)) {
            return Ok(Response::json_status(400, &json!({ "ok": false, "error_code": "ai_config_invalid", "error": e.to_string() })));
        }
    }
    let mut view = settings
        .config_view(preset_provider_records(app), upstream)
        .map_err(|e| ApiError::internal(format!("ai_config_failed: {e}")))?;
    if let Some(obj) = view.as_object_mut() {
        obj.insert("ok".into(), json!(true));
        let presets = obj.get("presets").cloned().unwrap_or_else(|| json!([]));
        obj.insert("providers".into(), presets);
        if saving {
            obj.insert("saved".into(), json!(true));
        }
    }
    ok_json(view)
}

fn ai_config(app: &Arc<App>) -> Value {
    let stored = app.setting("aiConfig");
    let mut base = json!({
        "provider": "openai",
        "baseUrl": "https://api.openai.com/v1",
        "model": "gpt-4o-mini",
        "apiKey": "",
        "temperature": 0.7,
        "maxTokens": 2048,
        "systemPrompt": "You are ReadMD's markdown writing assistant.",
    });
    if let (Some(obj), Some(patch)) = (base.as_object_mut(), stored.as_object()) {
        for (k, v) in patch {
            obj.insert(k.clone(), v.clone());
        }
    }
    base
}


fn h_ai_models(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    let config = ai_config(app);
    let base = config.get("baseUrl").and_then(|v| v.as_str()).unwrap_or("").trim_end_matches('/').to_string();
    let key = config.get("apiKey").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let fallback: Vec<String> = config
        .get("models")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();
    if base.is_empty() || key.is_empty() {
        return ok_json(json!({
            "ok": true,
            "models": fallback,
            "source": "config",
            "message": "Configure baseUrl and apiKey to query the provider model list.",
        }));
    }
    let url = if req.q("full") == Some("1") { format!("{base}/models") } else { format!("{base}/models") };
    match http_get(&url, &[format!("Authorization: Bearer {key}")], 20) {
        Ok((status, bytes)) if (200..300).contains(&status) => {
            let parsed: Value = serde_json::from_slice(&bytes).unwrap_or_else(|_| json!([]));
            let models = parsed
                .get("data")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|m| m.get("id").and_then(|v| v.as_str()).map(String::from))
                        .collect::<Vec<String>>()
                })
                .unwrap_or(fallback);
            ok_json(json!({ "ok": true, "models": models, "source": "provider" }))
        }
        Ok((status, bytes)) => Ok(Response::json_status(
            502,
            &json!({ "ok": false, "error": "provider_error", "status": status, "detail": truncate_text(&String::from_utf8_lossy(&bytes), 400), "models": fallback }),
        )),
        Err(e) => ok_json(json!({ "ok": true, "models": fallback, "source": "config", "warning": e.to_string() })),
    }
}

// ---------------------------------------------------------------------------
// `ai_providers` wiring
//
// `crate::ai_providers` is the port of `src/readmd_modules/ai.py`'s request
// layer: route selection, credential resolution, Skill rendering and the four
// provider transports (`chat/completions`, `completions`, `responses`,
// `v1/messages`).  It is a pure library, so everything it needs from the live
// application arrives through the three traits it is handed.  Those adapters
// live here because this is the file that owns `App`: the persisted settings,
// the pinned provider catalog and the on-disk Skill roots.
//
// Before this section existed, `h_ai_chat` carried its own copy of one of those
// four transports (`{base}/chat/completions` + a `Bearer` header + a hand-rolled
// `choices[0].message.content` read).  That copy could only ever reach an
// OpenAI-shaped endpoint keyed in the never-shipped `aiConfig` blob, so
// `provider`, `credential_id`, `mode`, `endpoint_mode`, `headers` and `skill_id`
// in the request body were dropped on the floor and the ported layer was dead
// weight.  Both problems are gone by construction now.
// ---------------------------------------------------------------------------

/// `assets/providers/provider-catalog.json`, the file `ai.py:27-45` loads into
/// `PRESETS`.  One reader for the settings endpoint and for the runtime lookup,
/// so the two can never disagree about which providers exist.
fn provider_catalog(app: &App) -> Value {
    let path = app.paths.assets_dir.join("providers").join("provider-catalog.json");
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| json!({}))
}

/// `ai.py::PRESETS` as `find_provider` sees them: the **raw** catalog records,
/// each stamped with the `"preset:" + name` id `ai.py:313` synthesises.  These
/// are deliberately not `h_ai_config`'s view objects — that view overwrites
/// `category` and `endpoint_mode` for the settings UI, while
/// `ai_providers::is_local_provider` reads `category` off the record to decide
/// whether a missing key is legal (Ollama and friends).
fn preset_provider_records(app: &App) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    let providers = provider_catalog(app)
        .get("providers")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    for record in providers {
        let name = record.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
        // `ai.py:39` — `isinstance(item, dict) and item.get("name")`.
        if name.is_empty() {
            continue;
        }
        let mut preset = record.clone();
        if let Some(map) = preset.as_object_mut() {
            map.insert("id".to_string(), json!(format!("preset:{}", name)));
        }
        out.push(preset);
    }
    out
}

/// `ai.py::find_provider` (306-314): user connections first, then presets.
fn find_ai_provider(presets: &[Value], custom: &[Value], identifier: &str) -> Value {
    for p in custom {
        let hit = p.get("id").and_then(|v| v.as_str()) == Some(identifier)
            || p.get("name").and_then(|v| v.as_str()) == Some(identifier);
        if hit {
            // `dict(p, custom=True)`
            let mut found = p.clone();
            if let Some(map) = found.as_object_mut() {
                map.insert("custom".to_string(), json!(true));
            }
            return found;
        }
    }
    for p in presets {
        // `"preset:" + p["name"] == identifier or p["name"] == identifier`; the
        // first half is the `id` `preset_provider_records` already stamped on.
        if p.get("name").and_then(|v| v.as_str()) == Some(identifier)
            || p.get("id").and_then(|v| v.as_str()) == Some(identifier)
        {
            return p.clone();
        }
    }
    // `None`: `resolve_chat` turns an unknown non-empty name into `未知提供商`.
    Value::Null
}

/// The settings side of `ai.py` as `ai_providers::ProviderDirectory`.
struct AiProviderDirectory {
    presets: Vec<Value>,
    custom: Vec<Value>,
    current: Value,
}

impl AiProviderDirectory {
    fn new(app: &App) -> AiProviderDirectory {
        let cfg = ai_settings(app).ensure_config().unwrap_or_else(|_| json!({}));
        AiProviderDirectory {
            presets: preset_provider_records(app),
            custom: cfg.get("providers").and_then(|v| v.as_array()).cloned().unwrap_or_default(),
            current: cfg.get("current").cloned().unwrap_or(Value::Null),
        }
    }
}

impl ai_providers::ProviderDirectory for AiProviderDirectory {
    /// `ensure_config().get("current") if isinstance(…, dict) else {}`.
    fn current_config(&self) -> Value {
        if self.current.is_object() { self.current.clone() } else { json!({}) }
    }

    fn find_provider(&self, name: &str) -> Value {
        find_ai_provider(&self.presets, &self.custom, name)
    }

    /// `ai.py::find_provider_by_credential` (317-326) including its handle
    /// validation: anything that is not a `cred:` handle of a sane length never
    /// reaches the credential store, so a client cannot probe it.
    fn find_provider_by_credential(&self, credential_id: &str) -> Option<Value> {
        let cid = credential_id.trim();
        if cid.is_empty() || cid.chars().count() > 128 || !cid.starts_with("cred:") {
            return None;
        }
        self.custom
            .iter()
            .find(|p| p.get("credential_id").and_then(|v| v.as_str()) == Some(cid))
            .cloned()
            .map(|mut p| {
                if let Some(map) = p.as_object_mut() {
                    map.insert("custom".to_string(), json!(true));
                }
                p
            })
    }

    /// `crypto.load_credential` through the kernel's store: the OS credential
    /// manager when there is one, the encrypted vault otherwise.  Unreadable
    /// collapses to `""` exactly like Python's `except` arms.
    fn load_credential(&self, credential_id: &str) -> String {
        crate::ai::CredentialStore::os_backend().load(credential_id)
    }

    /// `crypto.decrypt_api_key` for the retired `enc:`-in-config compatibility
    /// path (`ai.py:333-334`).
    fn decrypt_secret(&self, encoded: &str) -> String {
        crate::crypto::decrypt_api_key(encoded, None).unwrap_or_default()
    }

    /// `os.environ.get(name, "")` — the `env_key` fallback every preset uses.
    fn env_var(&self, name: &str) -> String {
        std::env::var(name).unwrap_or_default()
    }
}

/// One entry of `skills.py::SkillRegistry._skills`: only the fields
/// `SkillRegistry.render` (160-172) actually reads.  `required` is the
/// `required_variables` sidecar value, `None` meaning "the `or ["document"]`
/// default", which `ai_providers::render_skill_template` applies.
struct RuntimeSkill {
    instructions: String,
    required: Option<Value>,
    description: String,
}

struct AiSkillService<'a> {
    app: &'a Arc<App>,
}

const SKILL_MAX_BYTES: u64 = 512 * 1024;
const SKILL_METADATA_MAX_BYTES: u64 = 128 * 1024;
/// `skills.py:19 _ALLOWED_VARIABLES`.
const ALLOWED_SKILL_VARIABLES: [&str; 6] =
    ["context", "document", "language", "output_format", "request", "selection"];

/// `skills.py::default_skill_roots(None)`: builtin, then user, then project.
/// `SkillRegistry.reload` writes one dict in that order, so a **later** root
/// masks an earlier one — the opposite of `h_skills`' first-wins listing
/// dedupe, which is why the runtime index is built here instead of reusing the
/// listing's output.
fn skill_registry_roots(app: &App) -> Vec<PathBuf> {
    vec![
        app.paths.assets_dir.join("skills"),
        app.paths.data_dir.join("skills"),
        app.paths.workspace.join(".readmd/skills"),
    ]
}

/// `skills.py:17 _NAME_RE = ^[a-z0-9][a-z0-9-]{0,63}$`.
fn is_skill_name(name: &str) -> bool {
    let chars: Vec<char> = name.chars().collect();
    let Some(first) = chars.first() else { return false };
    chars.len() <= 64
        && (first.is_ascii_lowercase() || first.is_ascii_digit())
        && chars[1..]
            .iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '-')
}

/// `skills.py::_parse_frontmatter` (48-67).  `None` is every `SkillError` it
/// raises, i.e. "this folder is not a Skill", because `reload` swallows those
/// and skips the folder.
fn parse_skill_frontmatter(text: &str) -> Option<(Value, String)> {
    if !text.starts_with("---") {
        return None; // `SKILL.md must start with YAML frontmatter`
    }
    let mut fields = serde_json::Map::new();
    let mut closed_at: Option<usize> = None;
    let mut offset = 0usize;
    for (index, line) in text.split_inclusive('\n').enumerate() {
        let line_end = offset + line.len();
        let bare = line.trim_end_matches(|c| c == '\n' || c == '\r');
        offset = line_end;
        if index == 0 {
            continue; // the opening delimiter
        }
        // `…\r?\n---…`: the first bare `---` line closes the block, and the body
        // is everything after it — `.strip()`ed, exactly like Python's group (2).
        if bare.trim() == "---" {
            closed_at = Some(line_end);
            break;
        }
        let trimmed = bare.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || !trimmed.contains(':') {
            continue;
        }
        let (key, value) = trimmed.split_once(':').expect("contains(':')");
        fields.insert(key.trim().to_string(), frontmatter_value(value));
    }
    let body_start = closed_at?; // `SKILL.md frontmatter is not closed`
    Some((Value::Object(fields), py_strip_text(&text[body_start.min(text.len())..])))
}

/// One `key: value` line of `_parse_frontmatter` (59-66): quotes stripped, the
/// two boolean spellings lowered, `[a, b]` turned into a list.
fn frontmatter_value(raw: &str) -> Value {
    let value = raw.trim().trim_matches(|c| c == '\'' || c == '"');
    match value.to_lowercase().as_str() {
        "true" => return json!(true),
        "false" => return json!(false),
        _ => {}
    }
    if let Some(inner) = value.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
        return Value::Array(
            inner
                .split(',')
                .map(|item| item.trim().trim_matches(|c| c == '\'' || c == '"'))
                .filter(|item| !item.is_empty())
                .map(|item| Value::String(item.to_string()))
                .collect(),
        );
    }
    json!(value)
}

/// `str.strip()` on whitespace, mirroring `ai_providers::py_strip` for `&str`.
fn py_strip_text(text: &str) -> String {
    ai_providers::py_strip(text)
}

/// `skills.py::_read_metadata` (70-82).  `None` == a `SkillError` == skip the
/// whole Skill, which is how a corrupt sidecar can never smuggle a template in.
fn load_skill_metadata(folder: &Path) -> Option<Value> {
    let sidecar = folder.join("readmd.skill.json");
    let meta = match std::fs::metadata(&sidecar) {
        Ok(meta) => meta,
        Err(_) => return Some(json!({})), // `if not sidecar.exists(): return {}`
    };
    if !meta.is_file() || meta.len() > SKILL_METADATA_MAX_BYTES {
        return None;
    }
    let text = std::fs::read_to_string(&sidecar).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    if !value.is_object() {
        return None;
    }
    Some(value)
}

/// `skills.py::load_skill` (85-112) reduced to what the runtime index needs.
/// Returns the Skill id, the renderable body, and whether it is executable
/// (`skills.py:146-150` keeps a disabled draft out of `_skills`).
fn load_runtime_skill(folder: &Path) -> Option<(String, RuntimeSkill, bool)> {
    let folder_name = folder.file_name()?.to_string_lossy().into_owned();
    if !is_skill_name(&folder_name) {
        return None;
    }
    let manifest = folder.join("SKILL.md");
    let manifest_meta = std::fs::metadata(&manifest).ok()?;
    if !manifest_meta.is_file() || manifest_meta.len() > SKILL_MAX_BYTES {
        return None;
    }
    let text = std::fs::read_to_string(&manifest).ok()?;
    let (frontmatter, instructions) = parse_skill_frontmatter(&text)?;
    // `name = str(frontmatter.get("name") or folder.name).strip()`
    let raw_name = frontmatter.get("name").cloned().unwrap_or(Value::Null);
    let id = ai_providers::py_strip(&if ai_providers::py_truthy(&raw_name) {
        ai_providers::py_str(&raw_name)
    } else {
        folder_name.clone()
    });
    if !is_skill_name(&id) {
        return None;
    }
    // `if not description: raise SkillError("Skill description is required")`
    let description = frontmatter.get("description").cloned().unwrap_or(Value::Null);
    if ai_providers::py_strip(&ai_providers::py_str(&description)).is_empty() {
        return None;
    }
    // `unsupported Skill variables` — the variable scanner is the ported one.
    let unknown = ai_providers::template_variable_names(&instructions)
        .iter()
        .any(|name| !ALLOWED_SKILL_VARIABLES.contains(&name.as_str()));
    if unknown {
        return None;
    }
    let metadata = load_skill_metadata(folder)?;
    let enabled = metadata.get("enabled") != Some(&json!(false));
    Some((
        id,
        RuntimeSkill {
            instructions,
            required: metadata.get("required_variables").cloned(),
            description: ai_providers::py_strip(&ai_providers::py_str(&description)),
        },
        enabled,
    ))
}

/// `SkillRegistry.reload` (127-151) as a one-shot index.  Rebuilt per request
/// like Python rebuilds it per call (`ai.py:455` `_CORE_SERVICE.reload()`), so
/// an edited Skill takes effect without a restart.
fn runtime_skill_index(app: &App) -> HashMap<String, RuntimeSkill> {
    let mut index: HashMap<String, RuntimeSkill> = HashMap::new();
    for root in skill_registry_roots(app) {
        let Ok(read) = std::fs::read_dir(&root) else { continue };
        let mut folders: Vec<PathBuf> = read
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.is_dir()
                    && !path.file_name().map(|n| n.to_string_lossy().starts_with('.')).unwrap_or(true)
            })
            .collect();
        folders.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
        for folder in folders {
            let Some((id, skill, enabled)) = load_runtime_skill(&folder) else { continue };
            if !enabled {
                index.remove(&id);
            } else {
                index.insert(id, skill);
            }
        }
    }
    index
}

/// A Skill as the MCP surface sees it: id, one-line description, raw instructions.
pub struct SkillInfo {
    pub id: String,
    pub description: String,
    pub instructions: String,
}

/// Enabled Skills from every registry root, sorted by id (the same index
/// `/api/ai/chat` renders from).
pub fn skill_list(app: &Arc<App>) -> Vec<SkillInfo> {
    let mut out: Vec<SkillInfo> = runtime_skill_index(app)
        .into_iter()
        .map(|(id, s)| SkillInfo { id, description: s.description, instructions: s.instructions })
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// Render one Skill with the given variables (`SkillRegistry.render`).
pub fn skill_render(app: &Arc<App>, skill_id: &str, variables: &Value) -> Result<String, String> {
    use ai_providers::SkillService;
    let vars = ai_providers::VarMap::from_json(variables).ok_or_else(|| "invalid variables".to_string())?;
    AiSkillService { app }.render_skill(skill_id, &vars).map_err(|e| e.message)
}

impl ai_providers::SkillService for AiSkillService<'_> {
    fn render_skill(
        &self,
        skill_id: &str,
        variables: &ai_providers::VarMap,
    ) -> Result<String, ai_providers::SkillError> {
        // `SkillRegistry.get` strips the id, `render` raises for a miss.
        let id = ai_providers::py_strip(&skill_id.to_string());
        let index = runtime_skill_index(self.app);
        let Some(skill) = index.get(&id) else {
            return Err(ai_providers::SkillError::skill(format!("Skill not found: {}", skill_id)));
        };
        // The rendering itself is the ported `SkillRegistry.render`.
        ai_providers::render_skill_template(&skill.instructions, skill.required.as_ref(), variables)
    }
}

/// `readmd.py:2405-2424`, the non-streaming branch of `_api_ai_chat`: join every
/// `str` item the generator yields and keep the `{'usage': …}` event.  `isinstance`
/// is why a non-string delta is skipped rather than stringified.
fn collapse_chat_stream(
    stream: ai_providers::ChatStream,
) -> Result<(String, Option<Value>), ai_providers::AiError> {
    let mut content = String::new();
    let mut usage: Option<Value> = None;
    for event in stream {
        match event? {
            ai_providers::ChatEvent::Delta(value) => {
                if let Value::String(text) = &value {
                    content.push_str(text);
                }
            }
            ai_providers::ChatEvent::Usage(counts) => {
                if counts.truthy() {
                    usage = Some(counts.to_json());
                }
            }
        }
    }
    Ok((content, usage))
}

/// An `AiError` back into the three failure bodies this handler already answers
/// with, so wiring the module in did not change a single error contract:
///
/// * `HTTP <code>：<body…>` is what `ai.py::_http_json`/`_http_stream` raise on a
///   non-2xx, i.e. the provider answered wrongly — exactly the case that used to
///   be `ai_provider_error` + a `status` extra.
/// * `未配置 API Key（…）` (`ai.py:505`) is the "nothing is configured" case,
///   which has always been `ai_not_configured`.
/// * Everything else (transport failure, unknown provider, Skill rendering,
///   unparseable response) keeps the `ai_request_failed: <text>` shape.
fn ai_chat_failure(err: ai_providers::AiError) -> ApiError {
    if let Some(rest) = err.message.strip_prefix("HTTP ") {
        if let Some((code, body)) = rest.split_once('：') {
            return ApiError::internal("ai_provider_error")
                .noted("status", code)
                .noted("detail", body);
        }
    }
    if err.message.starts_with("未配置 API Key") {
        return ApiError::bad_request("ai_not_configured").noted("detail", err.message);
    }
    ApiError::internal(format!("ai_request_failed: {}", err.message))
}

/// `/api/ai/chat`.  Provider resolution, credential lookup, Skill rendering and
/// every provider transport are `ai_providers` — the port of `ai.py::chat`.
/// What is left here is the HTTP envelope plus the `session`/history state that
/// only the server owns.
fn h_ai_chat(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    ai_chat(app, req, &ai_providers::ureq_transport::UreqTransport::new())
}

/// `h_ai_chat` with its egress injected, so the envelope the frontend gets can
/// be pinned by a test that never opens a socket.
fn ai_chat(app: &Arc<App>, req: &Request, transport: &dyn ai_providers::Transport) -> ApiResult<Response> {
    // `readmd.py:2386-2397` has two arms that share the `invalid_request` code
    // but carry different text: a body that will not JSON-decode, and a decoded
    // body that is not an object.  `body_value` collapses both to `Null`, so the
    // parsed result is read directly to keep the message the UI shows accurate.
    // Without this guard a `null` body would instead surface `resolve_chat`'s
    // re-creation of CPython's `AttributeError` as the `error_code`.
    let payload = match req.json() {
        Ok(value) if value.is_object() => value,
        Ok(_) => {
            return Err(ApiError::bad_request("invalid_request").noted("error", "请求体必须是 JSON 对象"))
        }
        Err(_) => {
            return Err(ApiError::bad_request("invalid_request").noted("error", "请求格式错误"))
        }
    };
    let directory = AiProviderDirectory::new(app);
    let skills = AiSkillService { app };
    // The eager half: validation, credential resolution, `_skill_messages`.
    let route = ai_providers::resolve_chat(&payload, &directory, &skills).map_err(ai_chat_failure)?;
    if route.args.base_url.is_empty() {
        return Err(ApiError::bad_request("ai_not_configured").noted("detail", "请先在设置中配置 AI 服务地址。"));
    }
    // The dispatch half: the transport for the selected `mode`, over the same
    // in-process `ureq` + rustls stack as before — no Python, no subprocess.
    let stream = ai_providers::dispatch_chat(&route, transport).map_err(ai_chat_failure)?;
    let (content, usage) = collapse_chat_stream(stream).map_err(ai_chat_failure)?;
    let model = ai_providers::py_str(&route.args.model);

    let session = payload.get("session").and_then(value_to_string).unwrap_or_else(|| "default".into());
    let user_text = payload
        .get("messages")
        .and_then(|m| m.as_array())
        .and_then(|a| a.iter().rev().find(|m| m.get("role").and_then(|r| r.as_str()) == Some("user")))
        .and_then(|m| m.get("content").and_then(|c| c.as_str()).map(String::from))
        .unwrap_or_default();
    if !user_text.is_empty() {
        let _ = app.store.remember_chat(&session, "user", &user_text, &model);
    }
    let _ = app.store.remember_chat(&session, "assistant", &content, &model);
    ok_json(json!({
        "ok": true,
        "content": content,
        "text": content,
        "response": content,
        "model": model,
        "usage": usage.unwrap_or(Value::Null),
        "session": session,
    }))
}

fn h_ai_history(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    // `_api_ai_history` (`readmd.py:2936`-`2966`) wraps the whole body in one
    // `except Exception -> _send_api_error(500, 'ai_history_failed')`, so every
    // internal failure — even a body that will not JSON-decode — uses that code.
    if req.method == "GET" {
        let session = req.q("session").unwrap_or("default").to_string();
        let limit: usize = req.q("limit").and_then(|v| v.parse().ok()).unwrap_or(200).clamp(1, 2000);
        let Ok(messages) = app.store.chat_history(&session, limit) else {
            return Err(ApiError::internal("ai_history_failed"));
        };
        return ok_json(json!({ "ok": true, "session": session, "messages": messages, "history": messages }));
    }
    let payload = body_value(req);
    let session = payload.get("session").and_then(value_to_string).unwrap_or_else(|| "default".into());
    let role = payload.get("role").and_then(value_to_string).unwrap_or_else(|| "user".into());
    let text = payload.get("content").and_then(value_to_string).unwrap_or_default();
    let model = payload.get("model").and_then(value_to_string).unwrap_or_default();
    if app.store.remember_chat(&session, &role, &text, &model).is_err() {
        return Err(ApiError::internal("ai_history_failed"));
    }
    if payload.get("clear").and_then(|v| v.as_bool()).unwrap_or(false)
        && app.store.clear_chat(&session).is_err()
    {
        return Err(ApiError::internal("ai_history_failed"));
    }
    ok_json(json!({ "ok": true, "session": session }))
}

fn h_ai_prompts(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    if req.method == "GET" {
        let stored = app.setting("aiPrompts");
        let prompts = if stored.is_array() { stored } else { json!(default_prompts()) };
        return ok_json(json!({ "ok": true, "prompts": prompts }));
    }
    let payload = body_value(req);
    let prompts = payload.get("prompts").cloned().unwrap_or(payload);
    app.update_settings(&json!({ "aiPrompts": prompts }));
    ok_json(json!({ "ok": true, "prompts": prompts, "saved": true }))
}

fn default_prompts() -> Vec<Value> {
    [
        ("polish", "润色", "Polish the following markdown, keeping structure and meaning."),
        ("summary", "摘要", "Summarize the document in five bullet points."),
        ("translate", "翻译", "Translate the document into English without losing markdown structure."),
        ("outline", "大纲", "Produce a heading outline for this document."),
        ("title", "标题", "Suggest five concise titles for this document."),
    ]
    .iter()
    .map(|(id, name, text)| json!({ "id": id, "name": name, "prompt": text }))
    .collect()
}

fn h_image_save(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    let payload = body_value(req);
    let encoded = payload
        .get("data")
        .or_else(|| payload.get("base64"))
        .or_else(|| payload.get("dataUrl"))
        .and_then(|v| v.as_str())
        .map(|s| s.rsplit_once(',').map(|(_, b)| b.to_string()).unwrap_or_else(|| s.to_string()))
        .or_else(|| req.q("base64").map(|s| s.to_string()));
    let bytes = match encoded {
        Some(e) => base64_decode(&e).ok_or_else(|| ApiError::bad_request("invalid_base64"))?,
        None => {
            if req.body.is_empty() {
                return Err(ApiError::bad_request("empty_image"));
            }
            req.body.clone()
        }
    };
    let ext = payload.get("format")
        .or_else(|| payload.get("ext"))
        .and_then(value_to_string)
        .unwrap_or_else(|| "png".into());
    let name = payload.get("name").and_then(value_to_string).unwrap_or_else(|| {
        format!("img_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis())
    });
    let dir_val = payload.get("dir").or_else(|| payload.get("path")).and_then(value_to_string);
    let (target, rel) = if let Some(dir_str) = dir_val.filter(|d| !d.trim().is_empty() && std::path::Path::new(d).is_dir()) {
        let images_dir = std::path::PathBuf::from(&dir_str).join("images");
        let _ = std::fs::create_dir_all(&images_dir);
        let safe_name = std::path::Path::new(&name).file_name().and_then(|n| n.to_str()).unwrap_or(&name);
        let clean_stem = safe_name.trim_end_matches(&format!(".{}", ext));
        let filename = format!("{}.{}", clean_stem, ext);
        let target_path = images_dir.join(&filename);
        let rel_path = format!("images/{}", filename);
        (target_path, rel_path)
    } else {
        let uploads = app.paths.uploads_dir();
        let _ = std::fs::create_dir_all(&uploads);
        let target_path = content::unique_path(&uploads, &name, &ext);
        let rel_path = format!("/raw?p={}", percent_encoding::utf8_percent_encode(&target_path.to_string_lossy(), percent_encoding::NON_ALPHANUMERIC));
        (target_path, rel_path)
    };
    // `_api_image_save`'s outer `except Exception` (`readmd.py:2522`-`2524`)
    // answers the *legacy* shape: `{'error': '图片保存失败：%s' % e}`.
    content::write_bytes_atomic(&target, &bytes)
        .map_err(|e| ApiError::legacy_error(500, format!("图片保存失败：{e}")))?;
    ok_json(json!({
        "ok": true,
        "path": target.to_string_lossy(),
        "rel": rel,
        "relPath": rel,
        "size": bytes.len() as i64,
    }))
}

// --------------------------------------------------------- parity_web adapters
//
// `parity_web` assembles every body itself with `Response::json_status` instead
// of returning an [`ApiError`], because `readmd.py` mixes three different
// envelopes across these five routes — `{'error': ...}` alone on the `_api_ocr`
// 404 and the `_api_url` 400, `{'ok': False, 'error_code': ...}` from
// `_send_api_error` and the `_module_ready` gate, and
// `{'ok': False, 'code': ..., 'error': ...}` for the `/api/web/extract` gate.
// The kernel's `error_response()` can only render the second and third shapes
// through `ApiError`, so the ported handlers keep the signature
// `fn(&App, &Request) -> Response` and each route is one hop away here.

/// `Handler._api_ocr` (`readmd.py:3356`): `?p=` is double-unquoted, a missing
/// file answers `404 {'error': '文件不存在'}` and the `ocr` module gate runs
/// before any work.
fn h_ocr_parity(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    Ok(parity_web::h_ocr(app, req))
}

/// `Handler._api_transcribe` (`readmd.py:2052`): POST-only, `path`/`language`/
/// `model` come from the JSON body only (never the query), and the success body
/// is `{'ok': True, 'content', 'path', 'warning'}`.
fn h_transcribe_parity(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    Ok(parity_web::h_transcribe(app, req))
}

/// `Handler._api_url` (`readmd.py:3373`): `?u=` / `?crawl=1`, `400 {'error':
/// '缺少 URL'}` before the `web` module gate.
fn h_url_parity(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    Ok(parity_web::h_url(app, req))
}

/// `Handler._api_web_extract` (`readmd.py:3393`): the WebView/HTTP extractor
/// contract — `409 module_loading` vs `503` on the gate, `render_required`
/// fallbacks, `download_images` localization and the merged `engine_chain`
/// diagnostics.
fn h_web_extract_parity(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    Ok(parity_web::h_web_extract(app, req))
}

/// `Handler._api_web_cancel` (`readmd.py:3481`): gate first, then
/// `web.cancel(body['task_id'] or '')` and `200 {'ok': True}`.
fn h_web_cancel_parity(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    Ok(parity_web::h_web_cancel(app, req))
}



fn h_bibtex(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    let text = match req.field("content") {
        Some(t) => t,
        None => {
            // `_api_bibtex` (`readmd.py:2229`-`2238`) has a single
            // `except Exception -> _send_api_error(500, 'bibtex_failed')`.
            let path = resolve_arg(app, req, &["p", "path", "file"])?;
            content::read_text(&path).map_err(|_| ApiError::internal("bibtex_failed"))?
        }
    };
    let entries = parse_bibtex(&text);
    ok_json(json!({ "ok": true, "entries": entries, "count": entries.len() }))
}


/// `Handler._api_import_process` (`readmd.py:1954-1966`).
///
/// `readmd.py:1958-1960` reads exactly three keys off the JSON body — `content`,
/// `base_dir`, `current_file` — and `readmd.py:1962` passes all three to
/// `import_processor.process_markdown_imports(content, base_dir=…,
/// current_file=…)` with both byte budgets left at their `None` defaults
/// (`src/readmd_modules/import_processor.py:434-440`), which the `None`
/// arguments below reproduce.  Success is `200 {'ok': True, 'content': …}`
/// (`readmd.py:1963`).  The `try` block wraps the body read as well, so a
/// non-UTF-8 body, a `json.loads` failure, a non-object document (Python's
/// `body.get` → `AttributeError`), a non-string `content` (Python's
/// `IMPORT_PATTERN.finditer` → `TypeError`) and every raise out of the processor
/// all collapse into `_send_api_error(500, 'import_process_failed')`
/// (`readmd.py:1964-1966`) — the two-key body
/// `{'ok': False, 'error_code': 'import_process_failed'}`, with no extra key and
/// no exception text on the wire.
fn h_import_process(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    let params = Params::new(req);
    if params.body_invalid() {
        return Err(import_process_failed("body is not valid JSON"));
    }
    let payload = params.body();
    if !payload.is_object() {
        return Err(import_process_failed("body is not a JSON object"));
    }
    // KERNEL BRIDGE residue, preserved verbatim in
    // [`import_process_convert_paths`] rather than deleted: an earlier wave
    // answered a `{'paths': [...]}` / `{'files': [...]}` body with a
    // convert-and-stage contract `readmd.py` does not have.  Neither shipped
    // caller sends it — `assets/js/reader/render.js:854` and
    // `assets/readmd.boot.js:4404` both post
    // `{content, base_dir, current_file}` — so it now only runs for a body that
    // carries none of Python's three keys.
    if payload.get("content").is_none()
        && payload.get("base_dir").is_none()
        && payload.get("current_file").is_none()
        && (payload.get("paths").is_some() || payload.get("files").is_some())
    {
        return import_process_convert_paths(app, payload);
    }
    // `body.get(k, '')`: an absent key is `''`, and `base_dir` / `current_file`
    // only ever meet an `if x:` truthiness test (`import_processor.py:256`,
    // `:270`), so an explicit `null` behaves like `''` there.  `content` is the
    // exception — it goes straight into the regex, so a `null` is a `TypeError`.
    let content = match payload.get("content") {
        None => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(other) => return Err(import_process_failed(&format!("content is {other}"))),
    };
    let base_dir = py_text_field(payload, "base_dir")
        .ok_or_else(|| import_process_failed("base_dir is not text"))?;
    let current_file = py_text_field(payload, "current_file")
        .ok_or_else(|| import_process_failed("current_file is not text"))?;
    let processed = crate::import_processor::process_markdown_imports(
        &content,
        &base_dir,
        Some(current_file.as_str()),
        None,
        None,
    )
    .map_err(|e| import_process_failed(&e.to_string()))?;
    ok_json(json!({ "ok": true, "content": processed }))
}

/// Python's `body.get(key, '')` when the value only meets an `if x:` test:
/// absent or `null` is `''`, a string is itself, anything else would raise.
fn py_text_field(body: &Value, key: &str) -> Option<String> {
    match body.get(key) {
        None | Some(Value::Null) => Some(String::new()),
        Some(Value::String(s)) => Some(s.clone()),
        Some(_) => None,
    }
}

/// `readmd.py:1966` — `Handler._send_api_error(500, 'import_process_failed')`
/// with no `**extra`.  The reason is diagnostics only: `ApiError::noted` routes
/// `detail` off the response body (`lib.rs` `ApiError::noted`).
fn import_process_failed(detail: &str) -> ApiError {
    ApiError::internal("import_process_failed").noted("detail", detail)
}

/// The previous wave's file-import contract for `/api/import/process`: resolve
/// every `paths`/`files` entry, pass readable documents straight through and
/// convert the rest with `crate::convert`.  Not a `readmd.py` behaviour — see
/// the KERNEL BRIDGE note in [`h_import_process`].
fn import_process_convert_paths(app: &Arc<App>, payload: &Value) -> ApiResult<Response> {
    let sources = payload
        .get("paths")
        .or_else(|| payload.get("files"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut converted = Vec::new();
    let mut rejected = Vec::new();
    for item in sources {
        let raw = item.as_str().unwrap_or_default();
        if raw.is_empty() {
            continue;
        }
        match app.paths.resolve_doc(raw) {
            Ok(path) => {
                if content::is_readable(&path) {
                    converted.push(json!({ "path": path.to_string_lossy(), "kind": content::kind_of(&path) }));
                } else {
                    let path_str = path.to_string_lossy();
                    match crate::convert::convert_verbose(&path_str, true) {
                        Ok(res) if res.success => {
                            let out_path = path.with_extension("md");
                            if let Some(c) = res.content {
                                let _ = std::fs::write(&out_path, c);
                            }
                            converted.push(json!({ "path": out_path.to_string_lossy(), "kind": "markdown" }));
                        }
                        Ok(res) => {
                            rejected.push(json!({ "path": raw, "reason": res.error.unwrap_or_else(|| "conversion failed".to_string()) }));
                        }
                        Err(e) => {
                            rejected.push(json!({ "path": raw, "reason": e }));
                        }
                    }
                }
            }
            Err(e) => rejected.push(json!({ "path": raw, "reason": e.to_string() })),
        }
    }
    ok_json(json!({
        "ok": !converted.is_empty(),
        "imported": converted,
        "rejected": rejected,
        "pending": 0,
    }))
}

fn h_control_open(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    let params = Params::new(req);
    if params.body_invalid() {
        return Err(ApiError::legacy_error(400, "\u{65e0}\u{6548}\u{8bf7}\u{6c42}"));
    }
    let want = instance_token(app);
    if params.body().get("token").and_then(|v| v.as_str()) != Some(want.as_str()) {
        return Err(ApiError::legacy_error(403, "forbidden"));
    }
    let file = params.body().get("file").and_then(|v| v.as_str()).unwrap_or("");
    if !file.is_empty() && !std::path::Path::new(file).is_file() {
        return Err(ApiError::legacy_error(404, "\u{6587}\u{4ef6}\u{4e0d}\u{5b58}\u{5728}"));
    }
    app.control
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .open_queue
        .push_back(file.to_string());
    ok_json(json!({ "ok": true }))
}

fn h_control_next(app: &Arc<App>, _req: &Request) -> ApiResult<Response> {
    let act = app.control
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .open_queue
        .pop_front();
    ok_json(json!({ "pending": act.is_some(), "file": act.unwrap_or_default() }))
}

fn h_control_pet_batch(app: &Arc<App>, _req: &Request) -> ApiResult<Response> {
    crate::parity_pets::drain_pet_commands(app);
    let paths = app.control
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .pet_batches
        .pop_front();
    ok_json(json!({ "pending": paths.is_some(), "paths": paths.unwrap_or_default() }))
}

fn h_control_pet_menu(app: &Arc<App>, _req: &Request) -> ApiResult<Response> {
    crate::parity_pets::drain_pet_commands(app);
    let mut queue = app.control.lock().unwrap_or_else(|e| e.into_inner());
    let pending = if queue.pet_menus > 0 {
        queue.pet_menus -= 1;
        true
    } else {
        false
    };
    let action = queue.pet_actions.pop_front();
    ok_json(json!({ "pending": pending || action.is_some(), "action":action }))
}

// ------------------------------------------------------------ http egress

pub fn http_get(url: &str, headers: &[String], timeout: u64) -> Result<(u16, Vec<u8>), crate::Error> {
    http_request("GET", url, headers, None, timeout)
}

pub fn http_json(method: &str, url: &str, auth_header: &str, body: &Value, timeout: u64) -> Result<(u16, Vec<u8>), crate::Error> {
    let payload = serde_json::to_vec(body).unwrap_or_default();
    let mut headers = vec![
        "Content-Type: application/json".to_string(),
        "Accept: application/json".to_string(),
    ];
    if !auth_header.trim().is_empty() {
        headers.push(auth_header.to_string());
    }
    http_request(method, url, &headers, Some(&payload), timeout)
}

/// `curl --location` caps the chain at 50 hops; the loop below makes one
/// request per hop plus the first non-redirect answer, i.e. the same 51.
const MAX_HTTP_REDIRECTS: usize = 50;

/// HTTPS egress runs **in-process** on the TLS stack this crate already links:
/// `ureq` + `rustls` + `ring` + `webpki-roots`, the same combination `ai.rs`,
/// `batch2.rs` and `diagrams.rs` already drive.  It used to spawn the system
/// `curl` under a comment claiming the vendored crate set had no TLS stack; that
/// premise was false (the stack is in `Cargo.lock` and in the binary), and the
/// shell-out additionally spilled every request and response body into
/// world-readable `readmd-http-<nanos>.req` / `.body` files under
/// `std::env::temp_dir()` with a guessable name.  No temp file is created here.
///
/// The observable contract the `curl` marshalling published is preserved,
/// because callers depend on it:
/// * every HTTP status — 4xx and 5xx included — comes back
///   `Ok((status, body))`.  `curl` exited 0 for those too, and `--write-out
///   %{http_code}` reported the final code; `Err` is reserved for transport
///   failure (DNS, refused, TLS, timeout), which is what `code == 0` meant;
/// * `--location`: up to [`MAX_HTTP_REDIRECTS`] hops, the *final* status is the
///   one returned, and a 3xx with no `Location` is returned to the caller;
/// * `--proto http,https`: every redirect target is re-validated, so a
///   `Location: ftp://…` or `file://…` is refused, not followed;
/// * `-X <METHOD>`: `curl` was given `-X` for every non-`GET`, which pins the
///   method and the body across all four redirect kinds.  A plain `GET` keeps
///   curl's default rewrite of 301/302/303 into a bodyless GET;
/// * `--max-time N`: one deadline for the whole operation, so it is split
///   across hops rather than reset per request;
/// * the `Name: value` header list, its CR/LF rejection, the
///   `curl unavailable: …`-style `Error::Msg` shape and the 300-character
///   truncation of the transport error text are unchanged.
///
/// Proxy handling: `curl` read `<scheme>_proxy` / `ALL_PROXY` from the
/// environment and carved hosts back out with `NO_PROXY`.  `ureq` only does the
/// environment half behind its `proxy-from-env` feature, which this crate does
/// **not** build (and cannot ask for — the manifest is frozen), so the same
/// contract is restored explicitly by [`env_proxy_for`] below.  Two
/// `curl` capabilities remain narrower and are recorded rather than hidden:
/// `socks4://` / `socks5://` proxy URLs cannot be spoken (`socks-proxy` is off,
/// so `ureq::Proxy::new` rejects them and the request goes direct), and
/// `NO_PROXY` entries that pin a port are not matched.
pub fn http_request(
    method: &str,
    url: &str,
    headers: &[String],
    body: Option<&[u8]>,
    timeout: u64,
) -> Result<(u16, Vec<u8>), crate::Error> {
    validate_url(url)?;
    let mut named: Vec<(String, String)> = Vec::with_capacity(headers.len());
    for header in headers {
        if header.contains('\n') || header.contains('\r') {
            return Err(crate::Error::Msg("invalid header".into()));
        }
        match header.split_once(':') {
            Some((name, value)) if !name.trim().is_empty() => {
                named.push((name.trim().to_string(), value.trim().to_string()));
            }
            _ => return Err(crate::Error::Msg("invalid header".into())),
        }
    }

    let forced = !method.eq_ignore_ascii_case("GET");
    let deadline = if timeout > 0 {
        Some(std::time::Instant::now() + std::time::Duration::from_secs(timeout))
    } else {
        None
    };
    // `redirects(0)` so this function, not `ureq`, owns the hop policy —
    // `ureq`'s built-in follower turns a 301/302/303 POST into a bodyless GET
    // and refuses to follow a 307/308 that carries a body, neither of which is
    // what `curl -X POST --location` did.
    let mut target = url.to_string();
    let mut call_method = method.to_string();
    let mut payload = body.map(<[u8]>::to_vec);

    for hop in 0..=MAX_HTTP_REDIRECTS {
        let remaining = match deadline {
            None => None,
            Some(at) => match at.checked_duration_since(std::time::Instant::now()) {
                Some(left) => Some(left),
                None => return Err(crate::Error::Msg(truncate_text("request timed out", 300))),
            },
        };
        // Rebuilt per hop: `curl` re-reads the proxy environment for every
        // request it issues, so a redirect that crosses into a `NO_PROXY` host
        // must drop back to a direct connection.
        let agent = egress_agent(&target);
        let mut request = agent.request(&call_method, &target);
        for (name, value) in &named {
            request = request.set(name, value);
        }
        if let Some(left) = remaining {
            request = request.timeout(left);
        }
        let outcome = match payload.as_deref() {
            Some(bytes) => request.send_bytes(bytes),
            None => request.call(),
        };
        let response = match outcome {
            Ok(response) => response,
            Err(ureq::Error::Status(status, response)) => {
                return Ok((status, read_egress_body(response)));
            }
            Err(other) => {
                return Err(crate::Error::Msg(truncate_text(&other.to_string(), 300)));
            }
        };
        let status = response.status();
        let location = response.header("Location").map(str::to_string);
        if !(301..=308).contains(&status) || location.is_none() || hop == MAX_HTTP_REDIRECTS {
            return Ok((status, read_egress_body(response)));
        }
        // Drain the redirect body before moving on: it is what `curl` had
        // already written to `--output`, and dropping the reader would leak the
        // pooled connection instead of letting the next hop reuse it.
        let _ = read_egress_body(response);
        let next = resolve_location(&target, &location.unwrap_or_default())?;
        validate_url(&next).map_err(|_| crate::Error::Msg("redirect to unsupported protocol".into()))?;
        if !forced && matches!(status, 301 | 302 | 303) {
            call_method = "GET".to_string();
            payload = None;
        }
        target = next;
    }
    // Unreachable: the loop always returns inside `MAX_HTTP_REDIRECTS + 1`
    // iterations.  Kept explicit rather than `unreachable!()` so a handler
    // panic can never be the price of a future edit to the loop above.
    Err(crate::Error::Msg(truncate_text("too many redirects", 300)))
}

/// The `ureq` agent for one egress request: no built-in redirect follower (see
/// [`http_request`]) and, when the environment calls for one, `curl`'s proxy.
fn egress_agent(url: &str) -> ureq::Agent {
    let mut builder = ureq::AgentBuilder::new().redirects(0);
    if let Some(proxy) = env_proxy_for(url) {
        builder = builder.proxy(proxy);
    }
    builder.build()
}

/// `curl`'s per-request proxy decision, rebuilt here because `ureq` keeps its
/// own behind the `proxy-from-env` feature this crate does not build.
///
/// Precedence follows `curl`: the scheme-specific variable — lower-case
/// `<scheme>_proxy`, then its upper-case spelling — wins, then `all_proxy` /
/// `ALL_PROXY`.  `curl` does **not** proxy an HTTPS request through a bare
/// `http_proxy`, and neither does this; `ureq`'s built-in scanner does fall back
/// that way, which is one more reason it is not used.
fn env_proxy_for(url: &str) -> Option<ureq::Proxy> {
    let split = url.find("://")?;
    let scheme = url[..split].to_ascii_lowercase();
    let rest = &url[split + 3..];
    let authority_end = rest
        .char_indices()
        .find(|(_, c)| matches!(c, '/' | '?' | '#'))
        .map(|(i, _)| i)
        .unwrap_or(rest.len());
    let host = match rest[..authority_end].rsplit_once('@') {
        Some((_, host)) => host,
        None => &rest[..authority_end],
    };
    if no_proxy_covers(host) {
        return None;
    }
    let upper = scheme.to_ascii_uppercase();
    for name in [
        format!("{scheme}_proxy"),
        format!("{upper}_PROXY"),
        "all_proxy".to_string(),
        "ALL_PROXY".to_string(),
    ] {
        // `var_os`, and a leading `=` rejected: a Windows environment can carry
        // an entry named `=C:` whose *value* (`C:\`) is otherwise picked up as a
        // proxy URL by `std::env::var`.
        let value = match std::env::var_os(&name) {
            Some(value) => value,
            None => continue,
        };
        let value = value.to_string_lossy();
        let value = value.trim();
        if value.is_empty() || value.starts_with('=') {
            continue;
        }
        // A `socks4://` / `socks5://` value fails to parse while `socks-proxy`
        // is off, so the request goes direct.  That is the one `curl` capability
        // the vendored feature set cannot cover; it is stated, not assumed away.
        return ureq::Proxy::new(value).ok();
    }
    None
}

/// `curl`'s `NO_PROXY`: comma- or space-separated, `*` for every host, an
/// optional leading dot ignored, and a match is either the whole host or a
/// dot-aligned suffix — both sides compared lower-cased.  An entry that pins a
/// port is skipped rather than matched loosely, so it can never widen the
/// bypass.
fn no_proxy_covers(host: &str) -> bool {
    let Some(list) = std::env::var_os("NO_PROXY").or_else(|| std::env::var_os("no_proxy")) else {
        return false;
    };
    no_proxy_matches(&list.to_string_lossy(), host)
}

/// The matching rule behind [`no_proxy_covers`], kept apart from the two
/// environment reads so it can be tested without racing the test threads.
fn no_proxy_matches(list: &str, host: &str) -> bool {
    let list = list.to_ascii_lowercase();
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    if host.is_empty() {
        return false;
    }
    for raw in list.split([',', ' ', '\t']) {
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        if raw == "*" {
            return true;
        }
        let pattern = match raw.rsplit_once(':') {
            Some((head, port)) if !head.is_empty() && port.chars().all(|c| c.is_ascii_digit()) => {
                continue;
            }
            _ => raw,
        };
        let pattern = pattern.trim_start_matches('.');
        if pattern.is_empty() {
            continue;
        }
        if host == pattern || host.ends_with(&format!(".{pattern}")) {
            return true;
        }
    }
    false
}

fn read_egress_body(response: ureq::Response) -> Vec<u8> {
    use std::io::Read;
    let mut data = Vec::new();
    let _ = response.into_reader().read_to_end(&mut data);
    data
}

/// Resolve a `Location` against the request URL the way `curl --location` does:
/// absolute, scheme-relative (`//host/path`), root-relative (`/path`) or
/// last-segment-relative.  Anything else is refused rather than guessed.
fn resolve_location(base: &str, location: &str) -> Result<String, crate::Error> {
    let location = location.trim();
    if location.is_empty() {
        return Err(crate::Error::Msg("redirect without location".into()));
    }
    let lower = location.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return Ok(location.to_string());
    }
    let split = base.find("://").ok_or_else(|| crate::Error::Msg("invalid base url".into()))?;
    let (scheme, rest) = (&base[..split], &base[split + 3..]);
    let authority_end = rest
        .char_indices()
        .find(|(_, c)| matches!(c, '/' | '?' | '#'))
        .map(|(i, _)| i)
        .unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    if let Some(rest_of) = location.strip_prefix("//") {
        return Ok(format!("{scheme}://{rest_of}"));
    }
    let path = if location.starts_with('/') {
        location.to_string()
    } else {
        let base_path = &rest[authority_end..];
        let dir = base_path
            .rfind('/')
            .map(|i| &base_path[..=i])
            .unwrap_or("/");
        format!("{dir}{location}")
    };
    Ok(format!("{scheme}://{authority}{path}"))
}

fn validate_url(url: &str) -> Result<(), crate::Error> {
    let trimmed = url.trim();
    if trimmed.is_empty() || trimmed.len() > 4096 {
        return Err(crate::Error::Msg("invalid url".into()));
    }
    if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
        return Err(crate::Error::Msg("url must be http or https".into()));
    }
    if trimmed.chars().any(|c| c.is_control() || c.is_whitespace()) || trimmed.starts_with('-') {
        return Err(crate::Error::Msg("url contains forbidden characters".into()));
    }
    Ok(())
}

// --------------------------------------------------------------- text utils

pub fn urlencode(value: &str) -> String {
    percent_encoding::utf8_percent_encode(
        value,
        percent_encoding::NON_ALPHANUMERIC,
    )
    .to_string()
}

pub fn truncate_text(text: &str, max: usize) -> String {
    let count = text.chars().count();
    if count <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max).collect();
    out.push('…');
    out
}


pub fn parse_bibtex(text: &str) -> Vec<Value> {
    let mut entries = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i] != '@' {
            i += 1;
            continue;
        }
        let start = i + 1;
        let Some(open) = chars[start..].iter().position(|c| *c == '{') else { break };
        let kind: String = chars[start..start + open].iter().collect();
        let mut depth = 1usize;
        let mut j = start + open + 1;
        let body_start = j;
        while j < chars.len() && depth > 0 {
            match chars[j] {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
            if depth == 0 {
                break;
            }
            j += 1;
        }
        let body: String = chars[body_start..j.min(chars.len())].iter().collect();
        i = j + 1;
        let kind = kind.trim().to_ascii_lowercase();
        if kind.is_empty() || kind == "comment" || kind == "preamble" || kind == "string" {
            continue;
        }
        let mut fields = serde_json::Map::new();
        let mut key = String::new();
        for (idx, chunk) in split_fields(&body) {
            if idx == 0 {
                key = chunk.trim().to_string();
                continue;
            }
            if let Some((name, value)) = chunk.split_once('=') {
                fields.insert(name.trim().to_ascii_lowercase(), json!(clean_bibtex_value(value)));
            }
        }
        entries.push(json!({
            "type": kind,
            "key": key,
            "fields": Value::Object(fields),
        }));
    }
    entries
}

fn clean_bibtex_value(raw: &str) -> String {
    let mut v = raw.trim().trim_matches('"').trim().to_string();
    while v.len() >= 2 && v.starts_with('{') && v.ends_with('}') {
        v = v[1..v.len() - 1].trim().to_string();
    }
    v.retain(|c| c != '{' && c != '}');
    v
}

fn split_fields(body: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut depth = 0i32;
    let mut in_string = false;
    for ch in body.chars() {
        match ch {
            '{' => {
                depth += 1;
                current.push(ch);
            }
            '}' => {
                depth -= 1;
                current.push(ch);
            }
            '"' => {
                in_string = !in_string;
                current.push(ch);
            }
            ',' if depth == 0 && !in_string => {
                if !current.trim().is_empty() {
                    out.push((out.len(), current.trim().to_string()));
                }
                current = String::new();
            }
            _ => current.push(ch),
        }
    }
    if !current.trim().is_empty() {
        out.push((out.len(), current.trim().to_string()));
    }
    out
}


const B64_ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64_decode(input: &str) -> Option<Vec<u8>> {
    let mut acc: u32 = 0;
    let mut bits = 0u32;
    let mut out = Vec::new();
    for ch in input.bytes() {
        let ch = match ch {
            b'\n' | b'\r' | b' ' | b'\t' => continue,
            b'=' => break,
            value => value,
        };
        let value = B64_ALPHABET.iter().position(|c| *c == ch)? as u32;
        acc = (acc << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

// ------------------------------------------------------------------ reports

/// Every route the legacy engine exposes, for the parity report.
pub fn known_routes() -> Vec<(&'static str, bool)> {
    let mut out: Vec<(&'static str, bool)> = table().keys().copied().map(|k| (k, true)).collect();
    out.extend(PENDING.iter().map(|k| (*k, false)));
    out.sort_by_key(|(k, _)| *k);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `authorize()` reads `READMD_REQUIRE_TOKEN` from the environment on every
    /// request, so any test that mutates it must exclude every other dispatching
    /// test for its whole call graph.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn env_guard() -> std::sync::MutexGuard<'static, ()> {
        ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn sample_app(tag: &str) -> Arc<App> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("readmd-server-{tag}-{nanos}"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::create_dir_all(dir.join("assets")).unwrap();
        let paths = paths::AppPaths::with_dirs(&dir.join("data"), &dir, &dir.join("assets"));
        let app = App::bootstrap(paths).unwrap();
        std::fs::write(dir.join("assets/index.html"), b"<head><title>t</title></head>").unwrap();
        Arc::new(app)
    }

    fn request(method: &str, target: &str, body: &[u8]) -> Request {
        let (raw_path, raw_query) = target.split_once('?').unwrap_or((target, ""));
        let mut headers = HashMap::from([("host".to_string(), LOOPBACK_HOST.to_string())]);
        // `_read_request_body_limited` (`readmd.py:1428`) sizes the body from the
        // header, so a request carrying bytes it did not announce is not a
        // request a real client can send.
        if !body.is_empty() {
            headers.insert("content-length".to_string(), body.len().to_string());
        }
        Request {
            method: method.to_string(),
            path: raw_path.to_string(),
            query: parse_query(raw_query),
            headers,
            body: body.to_vec(),
        }
    }

    /// Real clients always name the socket they dialed; without it
    /// `readmd.py:1104` refuses every `/api/` call before routing.
    const LOOPBACK_HOST: &str = "127.0.0.1";

    fn app_request(app: &App, method: &str, target: &str, body: &[u8]) -> Request {
        let mut req = request(method, target, body);
        req.headers
            .insert("x-readmd-app-token".to_string(), app.app_token.clone());
        req
    }

    /// `readmd.py:2218-2227` + `style_injector.save_custom_styles`: the save
    /// reads `css`/`head`, writes the two files and answers `{'ok': bool}` and
    /// nothing else; `readmd.py:2209-2215` reads the same two files back under
    /// `data`, which is the only key the shipped clients look at
    /// (`assets/app.js:1309`, `main.rs:3198`).
    #[test]
    fn style_save_round_trips_through_the_custom_files() {
        let _env = env_guard();
        let app = sample_app("style-roundtrip");
        let res = dispatch(
            &app,
            &request("POST", "/api/style/save", br#"{"css":"body{color:red}","head":"<link rel=x>"}"#),
            true,
        );
        assert_eq!(res.status, 200);
        assert_eq!(
            serde_json::from_slice::<Value>(&res.body).unwrap(),
            json!({ "ok": true }),
            "the save body is exactly {{ok}} — no `style`, no `saved`"
        );
        assert!(app.paths.data_dir.join("custom.css").is_file());
        assert!(app.paths.data_dir.join("head.html").is_file());
        let res = dispatch(&app, &request("GET", "/api/style/get", &[]), true);
        assert_eq!(res.status, 200);
        assert_eq!(
            serde_json::from_slice::<Value>(&res.body).unwrap(),
            json!({
                "ok": true,
                "data": {
                    "css": "body{color:red}",
                    "head": "<link rel=x>",
                    "css_path": "custom.css",
                    "head_path": "head.html",
                }
            }),
            "`data` is style_injector.py:93-101's four-key dict, never null"
        );
    }

    /// Python's three distinct states.  An absent key (`''`), an explicit
    /// `null` (`None`) and `''` are all falsy, so `f.write(css or '')` writes an
    /// **empty file** — the file still exists, so `css_path` stays
    /// `"custom.css"`.  The old port read neither key and wrote no file at all.
    #[test]
    fn style_save_clears_a_slot_instead_of_ignoring_it() {
        let _env = env_guard();
        let app = sample_app("style-falsy");
        dispatch(&app, &request("POST", "/api/style/save", br#"{"css":"x","head":"y"}"#), true);
        let res = dispatch(&app, &request("POST", "/api/style/save", br#"{"css":null,"head":""}"#), true);
        assert_eq!(serde_json::from_slice::<Value>(&res.body).unwrap(), json!({ "ok": true }));
        assert_eq!(std::fs::read_to_string(app.paths.data_dir.join("custom.css")).unwrap(), "");
        assert_eq!(std::fs::read(app.paths.data_dir.join("head.html")).unwrap().len(), 0);
        let res = dispatch(&app, &request("GET", "/api/style/get", &[]), true);
        assert_eq!(
            serde_json::from_slice::<Value>(&res.body).unwrap()["data"],
            json!({ "css": "", "head": "", "css_path": "custom.css", "head_path": "head.html" })
        );
        // `n == 0` short-circuits to `body = {}`, so a bodyless POST is a
        // successful "clear both slots", never a parse error.
        let res = dispatch(&app, &request("POST", "/api/style/save", &[]), true);
        assert_eq!(res.status, 200);
        assert_eq!(serde_json::from_slice::<Value>(&res.body).unwrap(), json!({ "ok": true }));
    }

    /// `save_custom_styles` swallows its own `TypeError` into `return False`
    /// (`style_injector.py:123-124`), so a truthy non-string is `200 {'ok':
    /// false}` — *not* a 500 — and because the truncating `open(..., 'w')` has
    /// already run, `custom.css` is emptied while `head.html` keeps its previous
    /// content.  That partial write is observable Python behaviour.
    #[test]
    fn style_save_reports_a_python_type_error_as_200_ok_false() {
        let _env = env_guard();
        let app = sample_app("style-typeerror");
        dispatch(&app, &request("POST", "/api/style/save", br#"{"css":"a{}","head":"keep"}"#), true);
        let res = dispatch(&app, &request("POST", "/api/style/save", br#"{"css":5,"head":"other"}"#), true);
        assert_eq!(res.status, 200);
        assert_eq!(serde_json::from_slice::<Value>(&res.body).unwrap(), json!({ "ok": false }));
        assert_eq!(std::fs::read_to_string(app.paths.data_dir.join("custom.css")).unwrap(), "");
        assert_eq!(std::fs::read_to_string(app.paths.data_dir.join("head.html")).unwrap(), "keep");
    }

    /// The route's `except Exception` → `500 style_save_failed` is only
    /// reachable *before* the injector: a valid-JSON-but-not-a-mapping body
    /// makes `body.get` an `AttributeError`, and an unparseable body raises in
    /// `json.loads`.
    #[test]
    fn style_save_rejects_non_mapping_and_malformed_bodies_with_500() {
        let _env = env_guard();
        let app = sample_app("style-500");
        for body in [b"[1,2]".as_slice(), b"null".as_slice(), b"5".as_slice(), b"not json".as_slice()] {
            let res = dispatch(&app, &request("POST", "/api/style/save", body), true);
            assert_eq!(res.status, 500, "{:?}", String::from_utf8_lossy(body));
            assert_eq!(
                serde_json::from_slice::<Value>(&res.body).unwrap(),
                json!({ "ok": false, "error_code": "style_save_failed" }),
                "the 500 body must not leak decoder text"
            );
        }
        assert!(!app.paths.data_dir.join("custom.css").exists());
    }

    /// `readmd.py:2083-2091` — `_api_links_index` gates its own method (the
    /// dispatcher does not), and `dir` is checked before the indexer is even
    /// imported.
    #[test]
    fn links_index_gates_the_method_and_the_dir_argument() {
        let _env = env_guard();
        let app = sample_app("links-index");
        for method in ["GET", "PUT", "DELETE", "HEAD"] {
            let res = dispatch(&app, &request(method, "/api/links/index", b"{}"), true);
            assert_eq!(res.status, 405, "{method} must be 405 like readmd.py:2084");
            assert_eq!(
                serde_json::from_slice::<Value>(&res.body).unwrap(),
                json!({ "ok": false, "error_code": "method_not_allowed" })
            );
        }
        for body in [
            b"{}".as_slice(),
            br#"{"dir":""}"#.as_slice(),
            br#"{"dir":null}"#.as_slice(),
            br#"{"dir":"   "}"#.as_slice(),
            br#"{"dir":5}"#.as_slice(),
            br#"{"dir":"Z:/definitely/not/a/real/dir"}"#.as_slice(),
        ] {
            let res = dispatch(&app, &request("POST", "/api/links/index", body), true);
            assert_eq!(res.status, 404, "{:?}", String::from_utf8_lossy(body));
            assert_eq!(
                serde_json::from_slice::<Value>(&res.body).unwrap(),
                json!({ "ok": false, "error_code": "dir_not_found" })
            );
        }
        // A malformed body is Python's `JSONDecodeError`, i.e. a `ValueError`
        // and therefore the `400` arm — never the `500`.
        let res = dispatch(&app, &request("POST", "/api/links/index", b"{oops"), true);
        assert_eq!(res.status, 400);
    }

    /// `bool(body.get('force', False))` is CPython truthiness: the string
    /// `"false"` is **true** and `[]`/`{}`/`0`/`''`/`null` are not.  The
    /// `"1"|"true"|"yes"|"on"` parser this file uses for query flags gets both
    /// halves wrong, which is why `force` does not go through it.
    #[test]
    fn python_bool_semantics_hold_for_the_force_key() {
        for falsy in [json!(null), json!(false), json!(0), json!(0.0), json!(""), json!([]), json!({})] {
            assert!(!py_truthy(&falsy), "{falsy} must be falsy");
        }
        for truthy in [
            json!(true),
            json!(1),
            json!(-1),
            json!("false"),
            json!("0"),
            json!(" "),
            json!([0]),
            json!({ "a": 0 }),
        ] {
            assert!(py_truthy(&truthy), "{truthy} must be truthy");
        }
    }

    /// `curl --location` hop resolution, now owned by the kernel: absolute,
    /// scheme-relative, root-relative and last-segment-relative targets, and a
    /// `--proto http,https` refusal for anything else.
    #[test]
    fn egress_resolves_a_location_header_like_curl() {
        assert_eq!(resolve_location("https://a.b/x/y", "https://c.d/z").unwrap(), "https://c.d/z");
        assert_eq!(resolve_location("https://a.b/x/y", "/z").unwrap(), "https://a.b/z");
        assert_eq!(resolve_location("https://a.b/x/y", "//c.d/z").unwrap(), "https://c.d/z");
        assert_eq!(resolve_location("https://a.b/x/y", "z").unwrap(), "https://a.b/x/z");
        assert_eq!(resolve_location("https://a.b", "z").unwrap(), "https://a.b/z");
        assert!(resolve_location("https://a.b/x", "").is_err());
    }

    /// Egress is in process now.  A refused socket is a transport `Err` that
    /// never mentions `curl`, a non-`http(s)` URL is refused before any socket,
    /// and a malformed header line is rejected rather than argv-injected.
    #[test]
    fn egress_reports_transport_failure_without_spawning_curl() {
        // The closed port is only dialled directly when the environment asks
        // for no proxy; a reachable `http_proxy` would answer the same request
        // with its own 502, which is a status and not a transport failure.
        if env_proxy_for("http://127.0.0.1:1/nope").is_none() {
            let err = http_request("GET", "http://127.0.0.1:1/nope", &[], None, 2)
                .expect_err("a closed port is a transport failure");
            let text = err.to_string();
            assert!(!text.contains("curl"), "{text} must not ask the user to install curl");
            assert!(!text.contains("install "), "{text} must not carry installation advice");
        }

        for url in ["ftp://example.invalid/x", "file:///etc/passwd"] {
            let err = http_request("GET", url, &[], None, 1).expect_err("{url} is off-contract");
            assert!(err.to_string().contains("http"), "{err}");
        }
        let err = http_request("GET", "https://example.invalid", &["Bearer no-colon".to_string()], None, 1)
            .expect_err("a header without a name is rejected");
        assert!(err.to_string().contains("invalid header"), "{err}");
        let err = http_request("GET", "https://example.invalid", &["X-a: b\nc: d".to_string()], None, 1)
            .expect_err("CR/LF in a header is rejected");
        assert!(err.to_string().contains("invalid header"), "{err}");
    }

    /// `NO_PROXY`, restored because `ureq`'s own env scan sits behind a cargo
    /// feature this crate does not build.  Without this matcher a `NO_PROXY=*`
    /// box would have every request pushed into a proxy `curl` had skipped.
    #[test]
    fn egress_matches_no_proxy_entries_like_curl() {
        assert!(!no_proxy_matches("", "example.com"));
        assert!(no_proxy_matches("*", "example.com"));
        assert!(no_proxy_matches("example.com", "example.com"));
        assert!(no_proxy_matches("example.com", "api.example.com"));
        assert!(no_proxy_matches(".example.com", "example.com"));
        assert!(no_proxy_matches("EXAMPLE.COM", "example.com"));
        assert!(no_proxy_matches("example.com", "EXAMPLE.com."));
        assert!(no_proxy_matches("a.invalid, example.com", "example.com"));
        assert!(no_proxy_matches("a.invalid example.com", "example.com"));
        assert!(!no_proxy_matches("example.com", "notexample.com"));
        assert!(!no_proxy_matches("example.com", "example.org"));
        // A port-pinned entry never bypasses a bare host.
        assert!(!no_proxy_matches("example.com:8080", "example.com"));
    }

    /// One real request through the new egress path, so "it compiles against
    /// `ureq`" is not the only evidence.  Deliberately `#[ignore]`d: CI runs
    /// offline.  `cargo test --offline -p readmd-kernel --lib egress_live_probe
    /// -- --ignored` is the invocation.  No credential is ever sent — this hits a
    /// public host with no `Authorization` header.
    #[test]
    #[ignore = "needs a live network; run egress_live_probe with --ignored"]
    fn egress_live_probe_reaches_a_public_https_host() {
        let (status, body) = http_request(
            "GET",
            "https://example.com/",
            &["Accept: text/html".to_string()],
            None,
            15,
        )
        .expect("the in-process rustls path must reach a public host without curl");
        assert!((200..500).contains(&status), "unexpected status {status}");
        assert!(!body.is_empty(), "the body must be read, not spilled to a file");
        // A 404/403 is still a *status*, proving `curl`'s exit-0-for-any-HTTP
        // -answer contract survived the move.
        let (missing, _) = http_request("GET", "https://example.com/no-such-page-readmd", &[], None, 15)
            .expect("an HTTP answer is Ok, not Err, whatever the code");
        assert!(missing > 0, "a status code must always be reported");
    }

    /// `readmd.py: Handler._api_ping` returns a single boolean, no `engine` key.
    #[test]
    fn ping_reports_only_the_token_boolean() {
        let _env = env_guard();
        let app = sample_app("ping");
        let res = dispatch(&app, &request("GET", "/api/ping", &[]), true);
        assert_eq!(res.status, 200);
        assert_eq!(serde_json::from_slice::<Value>(&res.body).unwrap(), json!({ "ok": false }));

        // `readmd.py:2242` compares against instance.json, which Python only
        // publishes once the process owns CONTROL_PORT; without it both the
        // unauthenticated and the authenticated case are `false`.
        write_instance(&app, 0);
        let authed = request("GET", &format!("/api/ping?t={}", instance_token(&app)), &[]);
        let res = dispatch(&app, &authed, true);
        assert_eq!(res.status, 200);
        assert_eq!(serde_json::from_slice::<Value>(&res.body).unwrap(), json!({ "ok": true }));

        let wrong = request("GET", "/api/ping?t=not-the-token", &[]);
        let res = dispatch(&app, &wrong, true);
        assert_eq!(serde_json::from_slice::<Value>(&res.body).unwrap(), json!({ "ok": false }));
    }

    /// An unregistered path gets the dispatcher's plain-text 404; a registered
    /// one reaches its handler, which answers `readmd.py:3356`'s 404 JSON.
    #[test]
    fn unknown_route_is_404_and_known_route_reaches_handler() {
        let _env = env_guard();
        let app = sample_app("routes");
        let missing = dispatch(&app, &request("GET", "/api/definitely-not-real", &[]), true);
        assert_eq!(missing.status, 404);
        assert_eq!(String::from_utf8_lossy(&missing.body), "not found");
        let ocr = dispatch(&app, &request("GET", "/api/ocr", &[]), true);
        assert_eq!(ocr.status, 404);
        assert_eq!(
            serde_json::from_slice::<Value>(&ocr.body).unwrap(),
            json!({ "error": "文件不存在", "error_code": "file_not_found" })
        );
    }

    /// Percent-encode a filesystem path for a `?p=` value.  `_route()` decodes
    /// once inside `parse_qs` and applies a second `unquote()`
    /// (`readmd.py:1190`), so single-encoded is what a real client sends.
    fn pquery(p: &Path) -> String {
        percent_encoding::utf8_percent_encode(
            &p.to_string_lossy(),
            percent_encoding::NON_ALPHANUMERIC,
        )
        .to_string()
    }

    /// A fresh directory that is **not** under the sample app's workspace, data
    /// dir or assets dir, i.e. outside every root `AppPaths` would allow.
    fn outside_root(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("readmd-outside-{tag}-{nanos}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// T3: `Handler` has **no** containment gate on the reader routes.
    /// `_api_file` (`readmd.py:3011-3013`) tests `os.path.isfile` on the string
    /// the client typed, `_api_list` (`readmd.py:3095-3112`) walks whatever
    /// `os.path.isdir` accepts and `_send_raw` (`readmd.py:3564-3578`) reads any
    /// file.  The kernel used to answer `403 {'error_code':'forbidden'}` for all
    /// three; it now behaves like Python.
    #[test]
    fn reader_routes_serve_paths_outside_every_allowed_root() {
        let _env = env_guard();
        let app = sample_app("t3-reader");
        let outside = outside_root("reader");
        let doc = outside.join("note.md");
        std::fs::write(&doc, "# Outside\n\n根外正文").unwrap();
        assert!(
            app.paths.check_allowed(&doc).is_err(),
            "the fixture must really be outside the kernel's allowed roots"
        );

        let read = dispatch(&app, &request("GET", &format!("/api/file?p={}", pquery(&doc)), &[]), true);
        assert_eq!(read.status, 200);
        let value: Value = serde_json::from_slice(&read.body).unwrap();
        assert_eq!(value["name"], json!("note.md"));
        assert_eq!(value["path"], json!(doc.to_string_lossy().as_ref()));
        assert_eq!(value["dir"], json!(outside.to_string_lossy().as_ref()));

        let list = dispatch(&app, &request("GET", &format!("/api/list?p={}", pquery(&outside)), &[]), true);
        assert_eq!(list.status, 200);
        let value: Value = serde_json::from_slice(&list.body).unwrap();
        // `readmd.py:3111` — `{'dir': p, 'files': files}`, two keys and no more.
        let keys: Vec<&str> = value.as_object().unwrap().keys().map(|k| k.as_str()).collect();
        assert_eq!(keys, vec!["dir", "files"]);
        assert_eq!(value["dir"], json!(outside.to_string_lossy().as_ref()));
        assert!(value["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f.as_str().map(|s| s.replace('\\', "/")).unwrap_or_default().ends_with("note.md")));

        let raw = dispatch(&app, &request("GET", &format!("/raw?p={}", pquery(&doc)), &[]), true);
        assert_eq!(raw.status, 200);
        assert_eq!(raw.body, std::fs::read(&doc).unwrap());
    }

    /// T3: `/api/save` is the one route in `Handler` with a containment decision
    /// — `authorized_save_paths` (`readmd.py:967`), filled by `/api/file`
    /// (`readmd.py:3015`) and checked at `readmd.py:3554-3556`, which answers
    /// the LegacyError `403 {'error': '文件未被授权保存'}`.
    #[test]
    fn save_requires_a_prior_read_even_for_an_authorized_root() {
        let _env = env_guard();
        let app = sample_app("t3-save");
        let outside = outside_root("save");
        let doc = outside.join("draft.md");
        std::fs::write(&doc, "# Draft\n\n原文").unwrap();
        let body = serde_json::to_vec(&json!({
            "path": doc.to_string_lossy(),
            "content": "# Draft\n\n新正文",
        }))
        .unwrap();

        let refused = dispatch(&app, &app_request(&app, "POST", "/api/save", &body), true);
        assert_eq!(refused.status, 403);
        assert_eq!(
            serde_json::from_slice::<Value>(&refused.body).unwrap(),
            json!({ "error": "文件未被授权保存" })
        );
        assert_eq!(std::fs::read_to_string(&doc).unwrap(), "# Draft\n\n原文");

        // The read is what authorizes the write, exactly as in Python.
        let read = dispatch(&app, &request("GET", &format!("/api/file?p={}", pquery(&doc)), &[]), true);
        assert_eq!(read.status, 200);
        let saved = dispatch(&app, &app_request(&app, "POST", "/api/save", &body), true);
        assert_eq!(saved.status, 200);
        let value: Value = serde_json::from_slice(&saved.body).unwrap();
        // `readmd.py:3557-3562` forwards `save_text_atomic`'s dict verbatim.
        assert_eq!(value["ok"], json!(true));
        assert!(value["path"].as_str().unwrap().replace('\\', "/").ends_with("draft.md"));
        assert!(value.get("backup").is_some());
        assert!(value.get("mtime").is_some());
        assert_eq!(value.as_object().unwrap().len(), 4);
        assert_eq!(std::fs::read_to_string(&doc).unwrap(), "# Draft\n\n新正文");
    }

    /// A GBK file keeps its bytes' encoding through open → edit → save, and a
    /// character GBK cannot hold is refused (422) without touching the file.
    #[test]
    fn save_preserves_the_detected_encoding() {
        let _env = env_guard();
        let app = sample_app("enc-save");
        let outside = outside_root("enc");
        let doc = outside.join("笔记.md");
        std::fs::write(&doc, b"# \xc4\xe3\xba\xc3\n").unwrap(); // "# 你好" in GBK

        let read = dispatch(&app, &request("GET", &format!("/api/file?p={}", pquery(&doc)), &[]), true);
        assert_eq!(read.status, 200);
        let v: Value = serde_json::from_slice(&read.body).unwrap();
        assert_eq!(v["encoding"], json!("gb18030"));
        assert_eq!(v["content"], json!("# 你好\n"));

        let body = serde_json::to_vec(&json!({
            "path": doc.to_string_lossy(), "content": "# 你好世界\n", "encoding": "gb18030",
        }))
        .unwrap();
        let saved = dispatch(&app, &app_request(&app, "POST", "/api/save", &body), true);
        assert_eq!(saved.status, 200);
        assert_eq!(std::fs::read(&doc).unwrap(), b"# \xc4\xe3\xba\xc3\xca\xc0\xbd\xe7\n");

        let before = std::fs::read(&doc).unwrap();
        let body = serde_json::to_vec(&json!({
            "path": doc.to_string_lossy(), "content": "emoji 😀", "encoding": "gbk",
        }))
        .unwrap();
        let refused = dispatch(&app, &app_request(&app, "POST", "/api/save", &body), true);
        assert_eq!(refused.status, 422);
        let r: Value = serde_json::from_slice(&refused.body).unwrap();
        assert_eq!(r["error_code"], json!("encoding_unrepresentable"));
        assert_eq!(r["char"], json!("😀"));
        assert_eq!(std::fs::read(&doc).unwrap(), before);
    }

    /// `.txt` opens structured for reading while the raw text stays editable.
    #[test]
    fn txt_is_structured_on_open() {
        let _env = env_guard();
        let app = sample_app("txt-open");
        let outside = outside_root("txt");
        let doc = outside.join("plain.txt");
        std::fs::write(&doc, "第一章 概述\n\n这是正文。\n\n1. 第一项\n2. 第二项\n").unwrap();
        let read = dispatch(&app, &request("GET", &format!("/api/file?p={}", pquery(&doc)), &[]), true);
        assert_eq!(read.status, 200);
        let v: Value = serde_json::from_slice(&read.body).unwrap();
        assert_eq!(v["structured"], json!(true));
        assert_eq!(v["original"], json!("第一章 概述\n\n这是正文。\n\n1. 第一项\n2. 第二项\n"));
        let (md, _) = crate::convert::txt_to_markdown(v["original"].as_str().unwrap());
        assert_eq!(v["content"], json!(md));
    }

    /// A panicking handler answers `500 internal_error` instead of dropping the
    /// connection.
    #[test]
    fn panicking_handler_becomes_internal_error() {
        let res = internal_error_response();
        assert_eq!(res.status, 500);
        let v: Value = serde_json::from_slice(&res.body).unwrap();
        assert_eq!(v["error_code"], json!("internal_error"));
        let caught = std::panic::catch_unwind(|| -> Response { panic!("boom") });
        let msg = panic_message(&*caught.err().unwrap());
        assert_eq!(msg, "boom");
    }

    /// The reader routes' early exits, each in the envelope `readmd.py` uses.
    #[test]
    fn reader_route_early_exits_use_pythons_shapes() {
        let _env = env_guard();
        let app = sample_app("t3-exits");
        let outside = outside_root("exits");
        let missing = outside.join("nope.md");

        // `readmd.py:1189-1193` — an empty `?p=` is raw text, not JSON.
        let no_p = dispatch(&app, &request("GET", "/api/file", &[]), true);
        assert_eq!(no_p.status, 400);
        assert_eq!(String::from_utf8_lossy(&no_p.body), "missing p");
        assert!(no_p
            .headers
            .iter()
            .any(|(k, v)| k.eq_ignore_ascii_case("content-type") && v.starts_with("text/plain")));

        // `readmd.py:3011-3013` — `404 {'error': '文件不存在'}`, no `ok`.
        let absent = dispatch(&app, &request("GET", &format!("/api/file?p={}", pquery(&missing)), &[]), true);
        assert_eq!(absent.status, 404);
        assert_eq!(
            serde_json::from_slice::<Value>(&absent.body).unwrap(),
            json!({ "error": "文件不存在", "error_code": "file_not_found" })
        );

        // `readmd.py:3096-3098` — a non-directory is an empty listing, not a 4xx,
        // and because `os.path.isdir('')` is False so is an absent `?p=`.
        let not_a_dir = dispatch(&app, &request("GET", &format!("/api/list?p={}", pquery(&missing)), &[]), true);
        assert_eq!(not_a_dir.status, 200);
        assert_eq!(
            serde_json::from_slice::<Value>(&not_a_dir.body).unwrap(),
            json!({ "dir": missing.to_string_lossy(), "files": [] })
        );
        let no_p_list = dispatch(&app, &request("GET", "/api/list", &[]), true);
        assert_eq!(
            serde_json::from_slice::<Value>(&no_p_list.body).unwrap(),
            json!({ "dir": "", "files": [] })
        );

        // `_send_raw` (`readmd.py:3564-3568`) — `404 text/plain "not found"`.
        let raw = dispatch(&app, &request("GET", &format!("/raw?p={}", pquery(&missing)), &[]), true);
        assert_eq!(raw.status, 404);
        assert_eq!(String::from_utf8_lossy(&raw.body), "not found");
    }

    /// A workspace file still round-trips, now in the order Python requires:
    /// read (which authorizes) then save.
    #[test]
    fn file_save_search_roundtrip_over_http() {
        let _env = env_guard();
        let app = sample_app("flow");
        let doc = app.paths.workspace.join("note.md");
        std::fs::write(&doc, "# Note\n\n旧正文").unwrap();
        let query = format!("/api/file?p={}", pquery(&doc));
        assert_eq!(dispatch(&app, &request("GET", &query, &[]), true).status, 200);
        let save_body = serde_json::to_vec(&json!({
            "path": doc.to_string_lossy(),
            "content": "# Note\n\n含中文关键词的正文",
        }))
        .unwrap();
        let save = dispatch(&app, &app_request(&app, "POST", "/api/save", &save_body), true);
        assert_eq!(save.status, 200);
        let read = dispatch(&app, &request("GET", &query, &[]), true);
        let value: Value = serde_json::from_slice(&read.body).unwrap();
        assert_eq!(read.status, 200);
        assert_eq!(value["title"], json!("Note"));
        assert!(value["content"].as_str().unwrap().contains("含中文关键词"));
    }

    /// Wave B P2: the kernel used to serve eleven paths `readmd.py` never
    /// recognizes.  Its dispatcher falls through to
    /// `self._send(404, 'text/plain; charset=utf-8', b'not found')`
    /// (`readmd.py:1377-1378`), so anything here that answers JSON — or that
    /// answers 200 — is a parity break the differential harness reports as
    /// RUST-ONLY.  `/api/pin` was the worst of the set: a **GET** that wrote
    /// pin state.
    #[test]
    fn invented_kernel_routes_answer_pythons_plain_text_404() {
        let _env = env_guard();
        let app = sample_app("invented");
        const REMOVED: &[&str] = &[
            "/api/pin",
            "/api/delete",
            "/api/document/create",
            "/api/search",
            "/api/stats",
            "/api/tree",
            "/api/wordcount",
            "/api/render",
            "/api/readmd_fix",
            "/api/links/extract",
            "/api/settings/get",
        ];
        for path in REMOVED {
            // `is_api_path` gates the fall-through, so both the verb an old
            // client used and a plain GET have to land on the same answer.
            for (method, body) in [("GET", Vec::<u8>::new()), ("POST", b"{}".to_vec())] {
                let res = dispatch(&app, &request(method, path, &body), true);
                assert_eq!(res.status, 404, "{method} {path} must 404 like Python");
                assert_eq!(
                    String::from_utf8_lossy(&res.body),
                    "not found",
                    "{method} {path} must emit Python's bare body, not a JSON envelope"
                );
            }
        }
        // The five surviving bridges must NOT have been caught up in the sweep.
        // `/api/export` is deliberately absent from this GET loop: `h_export`
        // opens a modal `SaveFileDialog` when no `out_path` is supplied, so its
        // liveness is proven dialog-free by
        // `api_export_row_is_a_named_bridge_that_stays_live` instead.
        for path in [
            "/api/settings",
            "/api/rename",
            "/api/file/save-fixed",
            "/api/kernel/status",
        ] {
            let res = dispatch(&app, &request("GET", path, &[]), true);
            assert_ne!(
                String::from_utf8_lossy(&res.body),
                "not found",
                "{path} is a live kernel bridge"
            );
        }
    }

    /// Wave E1 (F2): `/api/settings/save` was the same defect class as Wave B's
    /// ten rows — `readmd.py:_route()` has no such branch, so Python answers
    /// `404 text/plain; charset=utf-8` `not found` (`readmd.py:1377-1378`) while
    /// the kernel served a `200`.  Nothing asked for it either: `grep -rn
    /// settings/save` over `assets/`, `readmd.py` and `src/` is empty, and the
    /// `main.rs` shim's `save_settings` posts to `/api/settings`.  This test is
    /// what stops the row from coming back silently.
    #[test]
    fn deleted_over_surface_rows_answer_pythons_plain_text_404() {
        let _env = env_guard();
        let app = sample_app("we1-deleted");
        assert!(
            !ROUTES.iter().any(|(path, _)| *path == "/api/settings/save"),
            "F2 deleted the /api/settings/save row; re-adding it is a parity break"
        );
        assert!(
            !LEGACY_ROUTES.contains(&"/api/settings/save"),
            "a path Python 404s must not enter the parity denominator either"
        );
        // Both the bare path and the trailing-slash form the dispatcher tolerates
        // must fall all the way through, for either verb.
        for path in ["/api/settings/save", "/api/settings/save/"] {
            for (method, body) in [
                ("GET", Vec::<u8>::new()),
                ("POST", b"{\"theme\":\"dark\"}".to_vec()),
            ] {
                let res = dispatch(&app, &request(method, path, &body), true);
                assert_eq!(res.status, 404, "{method} {path} must 404 like Python");
                assert_eq!(
                    String::from_utf8_lossy(&res.body),
                    "not found",
                    "{method} {path} must emit Python's bare body, not a JSON envelope"
                );
                assert!(
                    res.headers.iter().any(|(k, v)| {
                        k.eq_ignore_ascii_case("content-type")
                            && v == "text/plain; charset=utf-8"
                    }),
                    "{method} {path} must keep Python's text/plain Content-Type, got {:?}",
                    res.headers
                );
            }
        }
    }

    /// Every non-parity row has to carry its label on the table row itself: that
    /// is what `scratch/rust_parity/route_inventory.py` reads to sort a path into
    /// `BRIDGE` instead of `RUST-ONLY`.  Wave E1 (F1) labelled `/api/export`
    /// rather than deleting it, so the annotation is now part of the contract.
    #[test]
    fn every_kernel_bridge_row_is_labelled_on_the_table_row() {
        const SRC: &str = include_str!("server.rs");
        for path in [
            "/api/kernel/status",
            "/api/rename",
            "/api/settings",
            "/api/export",
            "/api/file/save-fixed",
        ] {
            assert!(
                ROUTES.iter().any(|(row, _)| *row == path),
                "{path} must still be routed as a kernel bridge"
            );
            let needle = format!("(\"{path}\",");
            let line = SRC
                .lines()
                .find(|line| line.trim_start().starts_with(needle.as_str()))
                .unwrap_or_else(|| panic!("{path} must have a ROUTES row line"));
            assert!(
                line.contains("KERNEL BRIDGE"),
                "{path} is not a readmd.py route, so its row must be annotated \
                 `KERNEL BRIDGE` on the row line itself, got: {line}"
            );
        }
    }

    /// Wave E1 (F1): `/api/export` looks like an invented row — `readmd.py`
    /// routes only `/api/export/epub` and `/api/export/presentation` — but it is
    /// the kernel's stand-in for pywebview `Api.export_doc` (`readmd.py:4873`),
    /// which `main.rs`'s shim calls through this exact path and which
    /// `assets/js/features/export.js:1095` uses for every format other than epub
    /// and presentation.  Deleting the row would have 404'd those exports.
    /// Asserted with a rejected format so the handler answers without opening a
    /// save dialog.
    #[test]
    fn api_export_row_is_a_named_bridge_that_stays_live() {
        let _env = env_guard();
        let app = sample_app("we1-export");
        let res = dispatch(
            &app,
            &request("POST", "/api/export", b"{\"format\":\"rtf\",\"content\":\"x\"}"),
            true,
        );
        assert_ne!(
            String::from_utf8_lossy(&res.body),
            "not found",
            "/api/export must keep reaching h_export; the bare 404 means the row was \
             deleted and py.export_doc's PDF/DOCX/HTML/TeX path is dead"
        );
        assert_eq!(res.status, 400, "an unsupported format must not be served");
        assert_eq!(
            serde_json::from_slice::<Value>(&res.body).unwrap(),
            json!({ "ok": false, "error_code": "unsupported_export_format" }),
            "the bridge answers with the kernel's error envelope, not Python's \
             {{ok:false, stage:'options'}} — recorded in h_export's doc comment"
        );
    }

    fn export_req(dir: &Path, task_id: &str) -> (Request, PathBuf) {
        let out = dir.join("doc.html");
        let body = json!({
            "format": "html", "content": "# hi", "out_path": out.to_string_lossy(),
            "task_id": task_id
        })
        .to_string();
        (request("POST", "/api/export", body.as_bytes()), out)
    }

    /// C.4.6: with a `task_id` the export is staged as `<name>.readmd-part` and
    /// renamed once; nothing but the target remains afterwards.
    #[test]
    fn task_export_commits_through_one_rename() {
        let app = sample_app("task-export");
        let dir = tempfile::tempdir().unwrap();
        let (req, out) = export_req(dir.path(), "exp-ok-1");
        let res = h_export(&app, &req).unwrap();
        let body: Value = serde_json::from_slice(&res.body).unwrap();
        assert_eq!(body["ok"], true, "{body}");
        assert_eq!(body["path"], out.to_string_lossy().as_ref());
        assert!(out.is_file());
        assert!(!crate::cancel::part_path(&out).exists(), "the staging file must be gone");
        assert_eq!(crate::cancel::cancel("exp-ok-1"), crate::cancel::CancelState::Unknown, "the task deregisters");
    }

    /// A cancel that arrives before the commit point leaves no file at all and
    /// answers `error_code: cancelled`.
    #[test]
    fn task_export_cancelled_before_commit_leaves_nothing() {
        let app = sample_app("task-cancel");
        let dir = tempfile::tempdir().unwrap();
        // Cancel from another thread as soon as the handler registers. Which
        // side wins is timing-dependent, but both outcomes must be clean:
        // either the file was committed, or nothing at all is left.
        let (req, out) = export_req(dir.path(), "exp-cancel-2");
        let stop = std::thread::spawn(|| {
            for _ in 0..2000 {
                if crate::cancel::cancel("exp-cancel-2") != crate::cancel::CancelState::Unknown {
                    break;
                }
                std::thread::yield_now();
            }
        });
        let res = h_export(&app, &req).unwrap();
        stop.join().unwrap();
        let body: Value = serde_json::from_slice(&res.body).unwrap();
        assert!(!crate::cancel::part_path(&out).exists(), "no staging file may survive");
        if body["ok"] == true {
            assert!(out.is_file(), "a finished export keeps its file");
        } else {
            assert_eq!(body["error_code"], "cancelled", "{body}");
            assert!(!out.exists(), "a cancelled export must not leave the target");
        }
    }

    #[test]
    fn task_cancel_route_reports_state() {
        let app = sample_app("task-route");
        let res = dispatch(&app, &request("POST", "/api/task/cancel", br#"{"id":"nobody-home"}"#), true);
        assert_eq!(res.status, 200);
        assert_eq!(serde_json::from_slice::<Value>(&res.body).unwrap(), json!({"ok": true, "state": "unknown"}));
        let bad = dispatch(&app, &request("POST", "/api/task/cancel", br#"{"id":"../x"}"#), true);
        assert_eq!(bad.status, 400);
        let _g = crate::cancel::register("route-live").unwrap();
        let res = dispatch(&app, &request("POST", "/api/task/cancel", br#"{"id":"route-live"}"#), true);
        assert_eq!(serde_json::from_slice::<Value>(&res.body).unwrap()["state"], "cancelling");
    }

    /// Wave E1 (F3): `readmd.py:1279` and `readmd.py:1283` are the only two
    /// `startswith` branches in `_route()`, and they are exactly what
    /// `LEGACY_DYNAMIC_PREFIXES` claims.
    #[test]
    fn p1_legacy_dynamic_prefixes_match_the_python_branches() {
        let mut declared: Vec<&str> = LEGACY_DYNAMIC_PREFIXES.to_vec();
        declared.sort_unstable();
        assert_eq!(
            declared,
            vec!["/api/skill-imports/", "/api/upstream-sources/"],
            "the prefix list must be exactly readmd.py:1279 and readmd.py:1283"
        );
    }

    /// Wave E1 (F3): a `pub const` nobody reads asserts nothing — the reason
    /// Wave B deleted `KERNEL_ONLY`.  Rather than delete it, `dispatch` now
    /// iterates the list through `dynamic_prefix_response`, and this test fails
    /// the moment a row stops being consulted: an unused prefix would fall
    /// through to Python's bare `text/plain` 404, which is indistinguishable
    /// from "no such route" and therefore silently wrong for both rows here.
    #[test]
    fn p1_every_legacy_dynamic_prefix_is_consulted() {
        let _env = env_guard();
        let app = sample_app("we1-prefix");
        assert!(!LEGACY_DYNAMIC_PREFIXES.is_empty());
        for prefix in LEGACY_DYNAMIC_PREFIXES {
            let probe = format!("{prefix}parity-probe");
            let res = dispatch(&app, &request("GET", &probe, &[]), true);
            assert_eq!(res.status, 404, "{probe} must not be served");
            assert_ne!(
                String::from_utf8_lossy(&res.body),
                "not found",
                "{prefix} is declared in LEGACY_DYNAMIC_PREFIXES but dispatch() never \
                 consulted it: {probe} fell through to the generic text/plain 404"
            );
            let body: Value = serde_json::from_slice(&res.body).unwrap_or_else(|err| {
                panic!(
                    "{prefix} must answer the JSON 404 its Python sub-dispatcher answers, \
                     got {} ({err})",
                    String::from_utf8_lossy(&res.body)
                )
            });
            assert_eq!(body["ok"], json!(false), "{probe}");
            assert!(
                body["error_code"].is_string(),
                "{probe} must carry an error_code, got {body}"
            );
        }
        // Control: an undeclared prefix really does get Python's bare 404, so the
        // two assertions above are the prefixes being consulted and not luck.
        let res = dispatch(
            &app,
            &request("GET", "/api/no-such-dynamic-prefix/probe", &[]),
            true,
        );
        assert_eq!(res.status, 404);
        assert_eq!(String::from_utf8_lossy(&res.body), "not found");
    }

    /// Wave E1 (F4) declared `/api/skill-imports/<id>/check|update` `PENDING`
    /// because the Skill-source manager was assumed unported.  It is ported
    /// (`skill_import.rs`), so `h_skill_import_source` serves both actions and
    /// neither template may creep back into the declared-pending list.  Every
    /// sub-path shape still answers the same
    /// `404 {'ok': False, 'error_code': 'source_not_found', ...}`
    /// `readmd.py:2857-2861` does, not the dispatcher's bare text 404.
    #[test]
    fn skill_import_source_subpaths_answer_pythons_404_never_a_pending_501() {
        let _env = env_guard();
        let app = sample_app("we1-skillsrc");
        for action in ["check", "update"] {
            let template = format!("/api/skill-imports/{{source_id}}/{action}");
            assert!(
                !PENDING.iter().any(|p| *p == template),
                "{template} is served by h_skill_import_source; declaring it pending \
                 would understate the port and inflate pendingCount"
            );
            assert_ne!(
                dispatch(&app, &request("POST", &format!("/api/skill-imports/owner--repo/{action}"), b"{}"), true).status,
                501,
                "a declared-pending row must not be what this path answers"
            );
        }
        for path in [
            "/api/skill-imports/owner--repo",
            "/api/skill-imports/owner--repo/check",
            "/api/skill-imports/owner--repo/update",
            "/api/skill-imports/owner--repo/delete",
            "/api/skill-imports/a/b/c",
        ] {
            let res = dispatch(&app, &request("POST", path, b"{}"), true);
            assert_eq!(res.status, 404, "{path}");
            assert_eq!(
                serde_json::from_slice::<Value>(&res.body).unwrap(),
                json!({
                    "ok": false,
                    "error_code": "source_not_found",
                    "error": "Skill 来源不存在"
                }),
                "{path} must answer readmd.py:2857's JSON 404, not a bare text 404"
            );
        }
        // The three exact rows that do exist in `readmd.py` are untouched.
        assert!(
            ROUTES
                .iter()
                .any(|(path, _)| *path == "/api/skill-imports/preview"),
            "the exact skill-import rows are parity routes, not prefix probes"
        );
    }

    /// The gates `readmd.py:2855-2895` runs, in order, against a *registered*
    /// source and without a network: the unknown-source 404 precedes the verb
    /// check, `Content-Length` is sized before the body is parsed, the parse
    /// precedes the preview, and `_skill_import_error` answers a
    /// `SkillImportError` with its code only — the Chinese message never leaves
    /// the process (`readmd.py:2764`).  Key order is asserted from the wire text
    /// because `serde_json` equality ignores it.
    #[test]
    fn skill_import_source_gates_run_in_pythons_order() {
        let _env = env_guard();
        let app = sample_app("we1-srcorder");
        std::fs::write(
            app.paths.data_dir.join("skills.json"),
            br#"{
                "schema_version": 2,
                "sources": [
                    {"source_id": "blank--repo", "source_type": "github", "repository_url": ""},
                    {"source_id": "odd--repo", "source_type": "ftp",
                     "repository_url": "https://github.com/odd/repo"}
                ]
            }"#,
        )
        .unwrap();
        let envelope = |res: &Response| -> (u16, String, Value) {
            let body: Value = serde_json::from_slice(&res.body).unwrap_or_else(|_| {
                panic!("body is not JSON: {}", String::from_utf8_lossy(&res.body))
            });
            let keys = body
                .as_object()
                .map(|map| map.keys().cloned().collect::<Vec<_>>().join(","))
                .unwrap_or_default();
            (res.status, keys, body)
        };
        let post = |source_id: &str, action: &str, body: &[u8]| {
            dispatch(
                &app,
                &request("POST", &format!("/api/skill-imports/{source_id}/{action}"), body),
                true,
            )
        };

        // 404 before the verb check: a `GET` on a source that is not registered
        // reads as missing, not as the wrong method.
        let (status, keys, body) = envelope(&dispatch(
            &app,
            &request("GET", "/api/skill-imports/nope--repo/check", &[]),
            true,
        ));
        assert_eq!((status, keys.as_str()), (404, "ok,error_code,error"));
        assert_eq!(body["error_code"], json!("source_not_found"));

        // The registered source reaches the verb gate, which is where the one
        // message this endpoint does echo lives.
        let (status, keys, body) = envelope(&dispatch(
            &app,
            &request("GET", "/api/skill-imports/blank--repo/update", &[]),
            true,
        ));
        assert_eq!(
            (status, keys.as_str()),
            (405, "ok,error_code,error"),
            "`readmd.py:2869-2872` is a three-key envelope"
        );
        assert_eq!(
            body,
            json!({"ok": false, "error_code": "method_not_allowed", "error": "仅支持 POST 请求"})
        );

        // Body gate: a non-object JSON body is rejected before the source is
        // previewed, and an announced size is what is bounded — never the bytes
        // actually read.
        let (status, keys, body) = envelope(&post("blank--repo", "check", b"[1, 2]"));
        assert_eq!((status, keys.as_str()), (400, "ok,error_code"));
        assert_eq!(body, json!({"ok": false, "error_code": "request_invalid"}));
        for (declared, code) in [
            ("3000000", "request_too_large"),
            ("-1", "request_too_large"),
            ("not-a-number", "request_invalid"),
        ] {
            let mut req = request("POST", "/api/skill-imports/blank--repo/check", b"{}");
            req.headers.insert("content-length".to_string(), declared.to_string());
            let res = dispatch(&app, &req, true);
            let (status, keys, body) = envelope(&res);
            assert_eq!((status, keys.as_str()), (400, "ok,error_code"), "{declared}");
            assert_eq!(body["error_code"], json!(code), "Content-Length: {declared}");
        }

        // Preview gate: both codes below come from the ported module, so the
        // two-key `_skill_import_error` envelope is the proof the module — not a
        // stub — answered.
        let (status, keys, body) = envelope(&post("blank--repo", "check", b"{}"));
        assert_eq!((status, keys.as_str()), (400, "ok,error_code"));
        assert_eq!(
            body,
            json!({"ok": false, "error_code": "source_not_found"}),
            "preview_saved_source's 来源不可用 message must not leave the process"
        );
        let (status, _, body) = envelope(&post("odd--repo", "update", b"{}"));
        assert_eq!(status, 400);
        assert_eq!(body["error_code"], json!("source_type_invalid"));
        // `confirm`/`selections` are checked *after* the preview, so the
        // `update` gate above already proves the ordering; the values themselves
        // are covered by the module's own tests.
    }

    /// The guard `PENDING`'s doc comment promises: the list stays hand-declared
    /// and never describes something the table actually serves.
    #[test]
    fn p1_pending_surface_is_declared() {
        for entry in PENDING {
            assert!(
                !table().contains_key(entry),
                "{entry} is declared PENDING but is a routed row — the count would \
                 understate what is missing"
            );
            assert!(
                entry.starts_with("/api/"),
                "{entry} must name a legacy API surface, not a free-text note"
            );
            assert!(
                *entry != "/api/settings/save",
                "a path Python 404s is not pending work, it is over-surface"
            );
            assert!(
                !entry.starts_with("/api/skill-imports/"),
                "the whole skill-imports surface is served by `h_skill_import_source` \
                 and the batch handlers; {entry} would re-add a retired 501"
            );
        }
    }

    /// The other guard `PENDING`'s doc comment promises: no legacy path may fall
    /// out of both the routed table and the declared pending list.  `/` and
    /// `/index.html` are the two documented exceptions because `readmd.py`
    /// answers them from `serve_static`, not from the route table — asserted
    /// here so the exception cannot quietly absorb a real route.
    #[test]
    fn p1_every_legacy_route_is_either_routed_or_pending() {
        let _env = env_guard();
        let app = sample_app("we1-legacy-cover");
        let mut statics = 0usize;
        for path in LEGACY_ROUTES {
            if *path == "/" || *path == "/index.html" {
                statics += 1;
                continue;
            }
            let routed = table().contains_key(path)
                || LEGACY_DYNAMIC_PREFIXES
                    .iter()
                    .any(|prefix| path.starts_with(*prefix));
            assert!(
                routed || PENDING.iter().any(|p| p == path),
                "{path} is in LEGACY_ROUTES but neither routed nor declared pending"
            );
        }
        assert_eq!(statics, 2, "the static exception list must stay two entries");
        for path in ["/", "/index.html"] {
            let res = dispatch(&app, &request("GET", path, &[]), true);
            assert_eq!(res.status, 200, "{path} must really be served statically");
        }
    }

    #[test]
    fn token_required_mode_rejects_bad_token() {
        let _env = env_guard();
        let previous = std::env::var("READMD_REQUIRE_TOKEN").ok();
        std::env::set_var("READMD_REQUIRE_TOKEN", "1");
        let app = sample_app("token");
        let bad = dispatch(&app, &request("GET", "/api/ping", &[]), false);
        assert_eq!(bad.status, 403);
        match previous {
            Some(v) => std::env::set_var("READMD_REQUIRE_TOKEN", v),
            None => std::env::remove_var("READMD_REQUIRE_TOKEN"),
        }
    }

    #[test]
    fn static_index_receives_token_injection() {
        let _env = env_guard();
        let app = sample_app("index");
        std::fs::write(
            app.paths.assets_dir.join("index.html"),
            b"<head><script>window.LAN_TOKEN=null;</script></head>",
        )
        .unwrap();
        let res = dispatch(&app, &request("GET", "/", &[]), true);
        let html = String::from_utf8_lossy(&res.body).into_owned();
        assert!(html.contains(&format!("window.LAN_TOKEN=\"{}\";", app.app_token)));
        assert!(html.contains("window.READMD_ENGINE=\"rust\""));
    }

    #[test]
    fn path_traversal_in_asset_requests_is_refused() {
        let app = sample_app("traversal");
        let res = dispatch(&app, &request("GET", "/assets/../../../Windows/win.ini", &[]), true);
        assert!(res.status == 403 || res.status == 404);
    }

    #[test]
    fn bibtex_and_base64_helpers_work() {
        let entries = parse_bibtex("@article{smith2020, title = {A {Nested} Title}, year = {2020}}\n");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["key"], json!("smith2020"));
        assert_eq!(entries[0]["fields"]["year"], json!("2020"));
        assert_eq!(base64_decode("aGVsbG8=").unwrap(), b"hello".to_vec());
    }

    #[test]
    fn query_parsing_handles_percent_and_plus() {
        let map = parse_query("p=a%20b.md&q=hi+there");
        assert_eq!(map.get("p").map(|s| s.as_str()), Some("a b.md"));
        assert_eq!(map.get("q").map(|s| s.as_str()), Some("hi there"));
    }

    // -- `ai_providers` wiring ----------------------------------------------
    //
    // `crate::ai_providers` owns everything about *what goes to the provider*
    // and has its own tests for that.  What these pin is the part added in this
    // file: which records the module is shown, how Skill rendering reaches the
    // on-disk registry, and the JSON both sides of the call speak — so the
    // envelope `/api/ai/chat` has always returned cannot drift silently now that
    // it is assembled from a `ChatStream`.

    /// Records every egress call and replays canned provider output, so the
    /// shipped envelope can be pinned without opening a socket.
    struct FakeTransport {
        calls: std::cell::RefCell<Vec<ai_providers::WireRequest>>,
        lines: Vec<String>,
    }

    struct FakeLines {
        lines: std::vec::IntoIter<String>,
    }

    impl ai_providers::LineStream for FakeLines {
        fn next_line(&mut self) -> Result<Option<String>, String> {
            Ok(self.lines.next())
        }
    }

    impl FakeTransport {
        fn new(lines: &[&str]) -> FakeTransport {
            FakeTransport {
                calls: std::cell::RefCell::new(Vec::new()),
                lines: lines.iter().map(|s| s.to_string()).collect(),
            }
        }
        fn recorded(&self) -> Vec<ai_providers::WireRequest> {
            self.calls.borrow().clone()
        }
    }

    impl ai_providers::Transport for FakeTransport {
        fn post_json(
            &self,
            url: &str,
            headers: &ai_providers::Headers,
            body: &str,
        ) -> Result<String, ai_providers::HttpError> {
            self.calls.borrow_mut().push(ai_providers::WireRequest {
                url: url.to_string(),
                headers: headers.clone(),
                body: body.to_string(),
            });
            Ok(self.lines.join(""))
        }
        fn open_stream(
            &self,
            url: &str,
            headers: &ai_providers::Headers,
            body: &str,
        ) -> Result<Box<dyn ai_providers::LineStream>, ai_providers::HttpError> {
            self.calls.borrow_mut().push(ai_providers::WireRequest {
                url: url.to_string(),
                headers: headers.clone(),
                body: body.to_string(),
            });
            Ok(Box::new(FakeLines { lines: self.lines.clone().into_iter() }))
        }
    }

    /// A payload the shipped UI sends once a provider is configured: the
    /// connection fields `resolveSharedAiConnection()` reads out of
    /// `/api/ai/config`.  `base_url`/`api_key` are inline so the test needs
    /// neither the provider catalog nor a stored credential.
    fn chat_body(extra: Value) -> Value {
        let mut body = json!({
            "base_url": "https://provider.test/v1",
            "api_key": "sk-fake",
            "model": "fake-model",
            "messages": [{ "role": "user", "content": "hi" }],
            "session": "pinned",
        });
        if let (Some(map), Some(patch)) = (body.as_object_mut(), extra.as_object()) {
            for (k, v) in patch {
                map.insert(k.clone(), v.clone());
            }
        }
        body
    }

    #[test]
    fn provider_directory_hands_ai_providers_the_records_ai_py_defines() {
        use ai_providers::ProviderDirectory as _;
        let dir = AiProviderDirectory {
            presets: vec![
                json!({ "id": "preset:DeepSeek", "name": "DeepSeek", "base_url": "https://api.deepseek.com/v1" }),
                // A preset whose `category` only the **raw** record carries:
                // `h_ai_config`'s view hard-codes `"preset"` for the settings UI,
                // so showing that view instead would make `is_local_provider`
                // (ai.py:352-365) reject a keyless local server.
                json!({ "id": "preset:Lab", "name": "Lab", "base_url": "https://intranet.test/v1", "category": "local" }),
            ],
            custom: vec![json!({ "id": "conn-1", "name": "Office proxy", "credential_id": "cred:abc" })],
            current: json!({ "provider_id": "conn-1", "model": "m" }),
        };

        // `ai.py::find_provider`: user connections first, stamped `custom`.
        assert_eq!(dir.find_provider("conn-1"), json!({ "id": "conn-1", "name": "Office proxy", "credential_id": "cred:abc", "custom": true }));
        assert_eq!(dir.find_provider("Office proxy")["custom"], json!(true));
        // Presets resolve by bare name *and* by the `preset:` id the UI sends.
        assert_eq!(dir.find_provider("DeepSeek")["base_url"], json!("https://api.deepseek.com/v1"));
        assert_eq!(dir.find_provider("preset:Lab")["name"], json!("Lab"));
        assert!(ai_providers::is_local_provider(&dir.find_provider("Lab")));
        assert!(!ai_providers::is_local_provider(&dir.find_provider("DeepSeek")));
        // Unknown == `None` == `未知提供商`, never an empty-but-truthy record.
        assert_eq!(dir.find_provider("nonsense"), Value::Null);
        assert_eq!(dir.current_config()["provider_id"], json!("conn-1"));
        let broken = AiProviderDirectory { presets: vec![], custom: vec![], current: json!("junk") };
        assert_eq!(broken.current_config(), json!({}));

        // `ai.py::find_provider_by_credential` (317-326) rejects anything that is
        // not a `cred:` handle of a sane length before touching the store, so a
        // client cannot use this route to probe it.
        assert_eq!(dir.find_provider_by_credential("cred:abc").unwrap()["custom"], json!(true));
        assert!(dir.find_provider_by_credential("  cred:abc  ").is_some());
        assert!(dir.find_provider_by_credential("abc").is_none());
        assert!(dir.find_provider_by_credential("").is_none());
        assert!(dir.find_provider_by_credential(&format!("cred:{}", "x".repeat(130))).is_none());
    }

    #[test]
    fn skill_rendering_goes_through_the_on_disk_registry() {
        let app = sample_app("aiskill");
        // `assets/skills` == `skills.py::default_skill_roots()[0]`.
        let folder = app.paths.assets_dir.join("skills").join("tidy");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(
            folder.join("SKILL.md"),
            "---\nname: tidy\ndescription: Tidy a document\n---\n\nTask: {{request}}\nSource: {{document}}\nFormat: {{output_format}}\nLang: {{language}}\n",
        )
        .unwrap();
        let skills = AiSkillService { app: &app };
        let payload = chat_body(json!({
            "skill_id": "tidy",
            "skill_variables": { "document": "DOC" },
        }));

        // `ai.py::_skill_messages` prepends the rendered Skill and drops the
        // payload's own system turns; the variable defaults are the ported ones.
        let messages = ai_providers::skill_messages(&payload, &skills).unwrap();
        assert_eq!(
            Value::Array(messages),
            json!([
                { "role": "system", "content": "Task: \nSource: DOC\nFormat: Markdown\nLang: the document's language" },
                { "role": "user", "content": "hi" },
            ])
        );

        // The `readmd.skill.json` sidecar is honoured, and a registry reload is
        // per request exactly like `ai.py:455`, so an edit needs no restart.
        std::fs::write(folder.join("readmd.skill.json"), br#"{"required_variables": ["request"]}"#).unwrap();
        let err = ai_providers::skill_messages(&payload, &skills).unwrap_err();
        assert_eq!(err.message, "missing required Skill variables: request");

        // `enabled: false` takes the Skill out of `_skills` (`skills.py:146-150`),
        // which the handler reports as the registry's own not-found text.
        std::fs::write(folder.join("readmd.skill.json"), br#"{"enabled": false}"#).unwrap();
        let err = ai_providers::skill_messages(&payload, &skills).unwrap_err();
        assert_eq!(err.message, "Skill not found: tidy");
    }

    #[test]
    fn chat_failures_keep_the_three_error_bodies() {
        // These are exactly the bodies the inline implementation produced, and
        // they are what `error_code` consumers in `assets/js` switch on.
        let provider = ai_chat_failure(ai_providers::AiError::new("HTTP 429：daily usage limit"));
        assert_eq!(provider.status, 500);
        assert_eq!(provider.payload(), json!({ "ok": false, "error_code": "ai_provider_error", "status": "429" }));

        let unconfigured = ai_chat_failure(ai_providers::AiError::new("未配置 API Key（可填入界面，或设置环境变量 OPENAI_API_KEY）"));
        assert_eq!(unconfigured.status, 400);
        assert_eq!(unconfigured.payload(), json!({ "ok": false, "error_code": "ai_not_configured" }));

        let failed = ai_chat_failure(ai_providers::AiError::new("网络错误：getaddrinfo failed"));
        assert_eq!(failed.status, 500);
        assert_eq!(failed.payload(), json!({ "ok": false, "error_code": "ai_request_failed: 网络错误：getaddrinfo failed" }));
    }

    #[test]
    fn non_streaming_assembly_matches_the_python_loop() {
        // `readmd.py:2405-2424`: join the `str` items, keep the `usage` event,
        // and — because of `isinstance(item, str)` — ignore a non-string delta.
        let mut counts = ai_providers::Usage::new();
        counts.push("prompt_tokens", 3);
        counts.push("completion_tokens", 4);
        counts.push("total_tokens", 7);
        let events: Vec<Result<ai_providers::ChatEvent, ai_providers::AiError>> = vec![
            Ok(ai_providers::ChatEvent::Delta(json!("a"))),
            Ok(ai_providers::ChatEvent::Delta(json!(5))),
            Ok(ai_providers::ChatEvent::Delta(Value::Null)),
            Ok(ai_providers::ChatEvent::Delta(json!("b"))),
            Ok(ai_providers::ChatEvent::Usage(counts)),
        ];
        let stream: ai_providers::ChatStream = Box::new(events.into_iter());
        let (content, usage) = collapse_chat_stream(stream).unwrap();
        assert_eq!(content, "ab");
        assert_eq!(usage, Some(json!({ "prompt_tokens": 3, "completion_tokens": 4, "total_tokens": 7 })));

        let empty: ai_providers::ChatStream = Box::new(Vec::new().into_iter());
        let (content, usage) = collapse_chat_stream(empty).unwrap();
        assert_eq!(content, "");
        assert_eq!(usage, None);
    }

    #[test]
    fn chat_answers_json_over_a_streamed_provider_call() {
        // No `stream` key: `ai.py` streams from the provider by default, so the
        // handler has to fold the SSE events back into the JSON the UI reads.
        let app = sample_app("aichat-stream");
        let transport = FakeTransport::new(&[
            "data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}",
            "",
            "data: {\"choices\":[{\"delta\":{\"content\":\" world\"}}],\"usage\":{\"prompt_tokens\":3,\"completion_tokens\":4,\"total_tokens\":7}}",
            "data: [DONE]",
        ]);
        let body = chat_body(json!({})).to_string();
        let res = ai_chat(&app, &request("POST", "/api/ai/chat", body.as_bytes()), &transport).unwrap();

        assert_eq!(res.status, 200);
        let out: Value = serde_json::from_slice(&res.body).unwrap();
        assert_eq!(
            out,
            json!({
                "ok": true,
                "content": "Hello world",
                "text": "Hello world",
                "response": "Hello world",
                "model": "fake-model",
                "usage": { "prompt_tokens": 3, "completion_tokens": 4, "total_tokens": 7 },
                "session": "pinned",
            })
        );
        // No extra or missing keys, i.e. the envelope is byte-for-byte the one
        // the handler assembled by hand before the wiring.
        assert_eq!(out.as_object().unwrap().len(), 7);

        // It really went through the ported transport: `stream_options` and the
        // `temperature` default are emitted only by `chat_openai`, and the body
        // uses CPython's `json.dumps` separators rather than serde's compact ones.
        let calls = transport.recorded();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].url, "https://provider.test/v1/chat/completions");
        assert!(calls[0].body.contains("\"stream\": true"));
        assert!(calls[0].body.contains("\"stream_options\": {\"include_usage\": true}"));
        assert!(calls[0].body.contains("\"temperature\": 0.4"));
        assert!(calls[0].body.contains("{\"role\": \"user\", \"content\": \"hi\"}"));
        assert!(calls[0].headers.iter().any(|(k, v)| k == "Authorization" && v == "Bearer sk-fake"));
        assert!(calls[0].headers.iter().any(|(k, v)| k == "Content-Type" && v == "application/json"));
        assert!(calls[0].headers.iter().any(|(k, _)| k == "User-Agent"));

        // The history write is still the server's own job.
        let history = app.store.chat_history("pinned", 10).unwrap();
        assert_eq!(
            history.iter().map(|m| &m["role"]).collect::<Vec<_>>(),
            vec![&json!("user"), &json!("assistant")]
        );
        assert_eq!(history[1]["content"], json!("Hello world"));
    }

    #[test]
    fn chat_non_streaming_path_keeps_the_same_envelope() {
        // `stream: false` is what `editor.js`, `export.js` and `fixes.js` send,
        // so this is the shape most of the UI actually reads.
        let app = sample_app("aichat-json");
        let transport = FakeTransport::new(&[
            "{\"model\":\"provider-echo\",\"choices\":[{\"message\":{\"content\":\"Plain answer\"}}],\"usage\":{\"prompt_tokens\":1,\"completion_tokens\":2,\"total_tokens\":3}}",
        ]);
        let body = chat_body(json!({ "stream": false })).to_string();
        let res = ai_chat(&app, &request("POST", "/api/ai/chat", body.as_bytes()), &transport).unwrap();
        let out: Value = serde_json::from_slice(&res.body).unwrap();
        assert_eq!(out["content"], json!("Plain answer"));
        assert_eq!(out["text"], json!("Plain answer"));
        assert_eq!(out["usage"], json!({ "prompt_tokens": 1, "completion_tokens": 2, "total_tokens": 3 }));
        // The one value-source change the wiring made: `ai.py` never reads a
        // model back off the response, so `model` is the model that was
        // requested (`provider-echo` above is ignored, as in Python) rather than
        // whatever the provider chose to echo.
        assert_eq!(out["model"], json!("fake-model"));

        let calls = transport.recorded();
        assert_eq!(calls.len(), 1);
        assert!(calls[0].body.contains("\"stream\": false"));
        assert!(!calls[0].body.contains("stream_options"));
    }

    #[test]
    fn chat_rejects_a_body_that_is_not_an_object_like_python_does() {
        let app = sample_app("aichat-bad-body");
        let transport = FakeTransport::new(&["data: [DONE]"]);

        // `Response` is deliberately not `Debug`, so the error is taken through
        // `err()` rather than `unwrap_err()`.
        let err = ai_chat(&app, &request("POST", "/api/ai/chat", b"this is not json"), &transport)
            .err()
            .unwrap();
        assert_eq!(err.status, 400);
        assert_eq!(err.payload(), json!({ "ok": false, "error_code": "invalid_request", "error": "请求格式错误" }));

        let err = ai_chat(&app, &request("POST", "/api/ai/chat", b"[1,2]"), &transport)
            .err()
            .unwrap();
        assert_eq!(err.status, 400);
        assert_eq!(err.payload(), json!({ "ok": false, "error_code": "invalid_request", "error": "请求体必须是 JSON 对象" }));

        // Neither arm may reach the network.
        assert!(transport.recorded().is_empty());
    }
}

// ==================== batch 1: pet, share, upstream, modules ====================

// ===========================================================================
// Ported legacy capabilities, batch 1: pet library lifecycle, LAN share,
// dynamic module gate and the upstream-source catalogue. All of these are
// pure filesystem/JSON work in the legacy engine, so the Rust kernel serves
// them natively instead of returning 501.
// ===========================================================================



#[cfg_attr(not(test), allow(dead_code))]
fn kernel_valid_slug(slug: &str) -> bool {
    let bytes = slug.as_bytes();
    if bytes.is_empty() || bytes.len() > 64 || slug.contains("..") {
        return false;
    }
    if !(bytes[0].is_ascii_alphanumeric()) {
        return false;
    }
    bytes.iter().all(|b| b.is_ascii_alphanumeric() || *b == b'-' || *b == b'_' || *b == b'.')
}

#[cfg_attr(not(test), allow(dead_code))]
fn kernel_copy_tree(src: &Path, dst: &Path) -> std::io::Result<u64> {
    std::fs::create_dir_all(dst)?;
    let mut count = 0u64;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            count += kernel_copy_tree(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
            count += 1;
        }
    }
    Ok(count)
}

#[cfg_attr(not(test), allow(dead_code))]
fn kernel_b64_decode(input: &str) -> Option<Vec<u8>> {
    fn sextet(c: u8) -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some((c - b'A') as u32),
            b'a'..=b'z' => Some((c - b'a' + 26) as u32),
            b'0'..=b'9' => Some((c - b'0' + 52) as u32),
            b'+' | b'-' => Some(62),
            b'/' | b'_' => Some(63),
            _ => None,
        }
    }
    let mut acc: u32 = 0;
    let mut bits: u32 = 0;
    let mut out = Vec::with_capacity(input.len() / 4 * 3 + 3);
    for &c in input.as_bytes() {
        if c == b'=' || c.is_ascii_whitespace() {
            continue;
        }
        acc = (acc << 6) | sextet(c)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1u32 << bits) - 1;
        }
    }
    Some(out)
}

fn kernel_sha256_hex(bytes: &[u8]) -> String {
    let mut h = <sha2::Sha256 as sha2::Digest>::new();
    sha2::Digest::update(&mut h, bytes);
    let digest = sha2::Digest::finalize(h);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}


fn kernel_pet_string(payload: &Value, key: &str) -> String {
    payload.get(key).and_then(|v| v.as_str()).unwrap_or("").trim().to_string()
}




// ------------------------------------------------------------- module gate

const MODULE_PROBES: &[(&str, &str)] = &[
    ("convert", "/api/convert"),
    ("ocr", "/api/ocr"),
    ("web", "/api/web/extract"),
    ("ai", "/api/ai/chat"),
    ("transcribe", "/api/transcribe"),
    ("pdf_editor", "/api/pdf/save"),
];

fn h_modules_load(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    let payload = body_value(req);
    let name = kernel_pet_string(&payload, "name");
    let name = if name.is_empty() {
        req.q("name").unwrap_or("").trim().to_string()
    } else {
        name
    };
    let Some((_, probe)) = MODULE_PROBES.iter().find(|(m, _)| *m == name.as_str()) else {
        return Err(ApiError::bad_request("module_invalid"));
    };
    let _ = app;
    if table().contains_key(*probe) {
        return ok_json(json!({ "ok": true, "name": name, "status": "ready" }));
    }
    if PENDING.iter().any(|p| p == probe) {
        return Ok(Response::json_status(
            503,
            &json!({ "ok": false, "name": name, "status": "unavailable", "error_code": "module_unavailable", "detail": "rust_kernel_pending" }),
        ));
    }
    Ok(Response::json_status(
        202,
        &json!({ "ok": false, "name": name, "status": "loading", "error_code": "module_loading" }),
    ))
}

// ---------------------------------------------------------- upstream sources

fn kernel_upstream_root(app: &Arc<App>) -> PathBuf {
    app.paths.assets_dir.join("upstream")
}

fn kernel_upstream_manifest(app: &Arc<App>) -> Value {
    let file = kernel_upstream_root(app).join("manifest.json");
    match std::fs::read_to_string(&file) {
        Ok(text) => serde_json::from_str(&text).unwrap_or(Value::Null),
        Err(_) => Value::Null,
    }
}

fn kernel_upstream_files(item: &Value) -> Vec<String> {
    let Some(list) = item.get("files").and_then(|v| v.as_array()) else { return Vec::new() };
    list.iter()
        .filter_map(|f| {
            f.as_str()
                .map(|s| s.to_string())
                .or_else(|| f.get("path").and_then(|s| s.as_str()).map(|s| s.to_string()))
        })
        .collect()
}

fn kernel_upstream_sources(app: &Arc<App>) -> Vec<Value> {
    let manifest = kernel_upstream_manifest(app);
    let items: Vec<&Value> = match manifest.get("sources").and_then(|v| v.as_array()) {
        Some(list) => list.iter().collect(),
        None => match manifest.as_array() {
            Some(list) => list.iter().collect(),
            None => Vec::new(),
        },
    };
    let root = kernel_upstream_root(app);
    items
        .iter()
        .map(|item| {
            let id = item
                .get("id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .or_else(|| item.get("name").and_then(|v| v.as_str()).map(|s| content::slug(s)))
                .unwrap_or_default();
            let files = kernel_upstream_files(item);
            let mut total: u64 = 0;
            let mut file_ids: Vec<String> = Vec::new();
            for rel in &files {
                if let Ok(meta) = std::fs::metadata(root.join(rel.replace('/', std::path::MAIN_SEPARATOR_STR))) {
                    total += meta.len();
                }
                file_ids.push(kernel_sha256_hex(rel.as_bytes()).chars().take(24).collect());
            }
            json!({
                "id": id,
                "files": files.len(),
                "bytes": total,
                "manifest": item.get("manifest").cloned().unwrap_or(json!("manifest.json")),
                "license": item.get("license").cloned().unwrap_or(Value::Null),
                "file_ids": file_ids,
                "relative_paths": files,
            })
        })
        .filter(|s| !s["id"].as_str().unwrap_or("").is_empty())
        .collect()
}

fn h_upstream_sources(app: &Arc<App>, _req: &Request) -> ApiResult<Response> {
    ok_json(json!({
        "schema_version": 1,
        "offline": true,
        "sources": kernel_upstream_sources(app),
    }))
}

fn h_upstream_dynamic(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    let rest = req.path.trim_start_matches("/api/upstream-sources/").trim_end_matches('/');
    if rest.is_empty() {
        return Err(ApiError::not_found("upstream_source_not_found"));
    }
    let sources = kernel_upstream_sources(app);
    let root = kernel_upstream_root(app);
    let root_c = paths::canonicalize_or_clean(&root);
    if let Some(pos) = rest.find("/files/") {
        let id = &rest[..pos];
        let want = rest[pos + 7..].trim_start_matches('/');
        let source = sources.iter().find(|s| s["id"].as_str() == Some(id)).ok_or_else(|| {
            ApiError::not_found("upstream_source_not_found").noted("id", id.to_string())
        })?;
        let paths_list = source["relative_paths"].as_array().cloned().unwrap_or_default();
        let files = kernel_upstream_files(&Value::Array(
            paths_list.iter().filter_map(|p| p.as_str()).map(|p| json!(p)).collect(),
        ));
        for rel in &files {
            let fid = kernel_sha256_hex(rel.as_bytes()).chars().take(24).collect::<String>();
            if fid != want {
                continue;
            }
            let abs = root.join(rel.replace('/', std::path::MAIN_SEPARATOR_STR));
            let canonical = paths::canonical_existing(&abs).map_err(|_| {
                ApiError::new(503, "upstream_source_unavailable").noted("path", rel.clone())
            })?;
            if !canonical.starts_with(&root_c) {
                return Err(ApiError::forbidden("path_escape"));
            }
            let bytes = std::fs::read(&canonical).map_err(|_| {
                ApiError::new(503, "upstream_source_unavailable").noted("path", rel.clone())
            })?;
            let ctype = mime_of(&content::ext_of(&canonical));
            let text = if bytes.len() <= 2 * 1024 * 1024 && ctype.starts_with("text/") {
                String::from_utf8_lossy(&bytes).to_string()
            } else {
                String::new()
            };
            return ok_json(json!({
                "id": fid,
                "path": canonical.to_string_lossy(),
                "relative_path": rel,
                "bytes": bytes.len(),
                "sha256": kernel_sha256_hex(&bytes),
                "source_id": id,
                "mime": ctype,
                "content": text,
            }));
        }
        return Err(ApiError::not_found("upstream_source_not_found").noted("file", want.to_string()));
    }
    let source = sources.iter().find(|s| s["id"].as_str() == Some(rest)).ok_or_else(|| {
        ApiError::not_found("upstream_source_not_found").noted("id", rest.to_string())
    })?;
    let rels = source["relative_paths"].as_array().cloned().unwrap_or_default();
    let mut source_files = Vec::new();
    for rel in rels.iter().filter_map(|v| v.as_str()) {
        let abs = root.join(rel.replace('/', std::path::MAIN_SEPARATOR_STR));
        let bytes = std::fs::metadata(&abs).map(|m| m.len()).unwrap_or(0);
        source_files.push(json!({
            "id": kernel_sha256_hex(rel.as_bytes()).chars().take(24).collect::<String>(),
            "relative_path": rel,
            "bytes": bytes,
            "mime": mime_of(&content::ext_of(&abs)),
        }));
    }
    ok_json(json!({
        "schema_version": 1,
        "offline": true,
        "source": source,
        "source_files": source_files,
    }))
}

// -------------------------------------------------------------- LAN sharing

#[derive(Clone, Debug)]
struct ShareSession {
    port: u16,
    token: String,
    _root: PathBuf,
}

static SHARE: OnceLock<std::sync::Mutex<Option<ShareSession>>> = OnceLock::new();
static SHARE_STOP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn kernel_share_state() -> &'static std::sync::Mutex<Option<ShareSession>> {
    SHARE.get_or_init(|| std::sync::Mutex::new(None))
}

fn kernel_share_token(app: &Arc<App>, port: u16) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let seed = format!("{}|{}|{}|{}", app.app_token, port, nanos, std::process::id());
    kernel_sha256_hex(seed.as_bytes()).chars().take(16).collect()
}

fn kernel_lan_ip() -> String {
    if let Ok(sock) = std::net::UdpSocket::bind("0.0.0.0:0") {
        if sock.connect("8.8.8.8:443").is_ok() {
            if let Ok(addr) = sock.local_addr() {
                return addr.ip().to_string();
            }
        }
    }
    "127.0.0.1".to_string()
}

fn kernel_share_url(sess: &ShareSession) -> String {
    format!("http://{}:{}/", kernel_lan_ip(), sess.port)
}

fn kernel_share_query(target: &str, key: &str) -> Option<String> {
    let (_, query) = match target.split_once('?') {
        Some(pair) => pair,
        None => return None,
    };
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        if k == key {
            Some(
                percent_encoding::percent_decode_str(v)
                    .decode_utf8_lossy()
                    .into_owned(),
            )
        } else {
            None
        }
    })
}

fn kernel_share_respond(stream: &mut TcpStream, status: u16, ctype: &str, body: &[u8], head: bool) {
    let text = format!(
        "HTTP/1.1 {status} {}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nAccess-Control-Allow-Origin: *\r\n\r\n",
        reason(status),
        if head { 0 } else { body.len() }
    );
    let _ = stream.write_all(text.as_bytes());
    if !head {
        let _ = stream.write_all(body);
    }
    let _ = stream.flush();
}

fn kernel_serve_share_conn(mut stream: TcpStream, root: &Path, token: &str) {
    let Ok(peer) = stream.try_clone() else { return };
    let _ = peer.set_read_timeout(Some(Duration::from_secs(10)));
    let mut reader = BufReader::new(peer);
    let mut line = String::new();
    if reader.read_line(&mut line).unwrap_or(0) == 0 {
        return;
    }
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("GET").to_string();
    let target = parts.next().unwrap_or("/").to_string();
    if method != "GET" && method != "HEAD" {
        kernel_share_respond(
            &mut stream,
            405,
            "text/plain; charset=utf-8",
            b"method not allowed",
            false,
        );
        return;
    }
    let mut header_token = String::new();
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h).unwrap_or(0) == 0 {
            break;
        }
        let trimmed = h.trim();
        if trimmed.is_empty() {
            break;
        }
        if let Some((k, v)) = trimmed.split_once(':') {
            if k.trim().eq_ignore_ascii_case("x-readmd-token") {
                header_token = v.trim().to_string();
            }
        }
    }
    let given = kernel_share_query(&target, "token").unwrap_or_default();
    if given != token && header_token != token {
        kernel_share_respond(&mut stream, 403, "application/json", br#"{"ok":false,"error":"invalid_token"}"#, false);
        return;
    }
    let path_part = match target.split_once('?') {
        Some((p, _)) => p.to_string(),
        None => target.clone(),
    };
    let decoded = percent_encoding::percent_decode_str(&path_part).decode_utf8_lossy().into_owned();
    let rel = decoded.trim_start_matches('/').replace('/', std::path::MAIN_SEPARATOR_STR);
    let candidate = if rel.is_empty() { root.to_path_buf() } else { root.join(&rel) };
    let canonical = match paths::canonical_existing(&candidate) {
        Ok(c) => c,
        Err(_) => {
            kernel_share_respond(&mut stream, 404, "application/json", br#"{"ok":false,"error":"not_found"}"#, false);
            return;
        }
    };
    let root_c = paths::canonicalize_or_clean(root);
    if !canonical.starts_with(&root_c) {
        kernel_share_respond(&mut stream, 403, "application/json", br#"{"ok":false,"error":"path_escape"}"#, false);
        return;
    }
    if canonical.is_dir() {
        let mut listing = format!("Index of /{}\n", rel);
        if let Ok(entries) = std::fs::read_dir(&canonical) {
            for entry in entries.flatten() {
                let kind = if entry.path().is_dir() { "d" } else { "-" };
                listing.push_str(&format!("{kind} {}\n", entry.file_name().to_string_lossy()));
            }
        }
        kernel_share_respond(&mut stream, 200, "text/plain; charset=utf-8", listing.as_bytes(), method == "HEAD");
        return;
    }
    match std::fs::read(&canonical) {
        Ok(bytes) => {
            let ctype = mime_of(&content::ext_of(&canonical));
            kernel_share_respond(&mut stream, 200, ctype, &bytes, method == "HEAD");
        }
        Err(_) => kernel_share_respond(
            &mut stream,
            500,
            "application/json",
            br#"{"ok":false,"error":"read_failed"}"#,
            false,
        ),
    }
}

fn h_share_start(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    if req.method != "POST" {
        return Err(ApiError::bad_request("method_not_allowed"));
    }
    let payload = body_value(req);
    let current = kernel_pet_string(&payload, "current_file");
    let root = if current.is_empty() {
        app.paths.workspace.clone()
    } else {
        let file = app.paths.resolve_doc(&current).map_err(|_| ApiError::internal("share_start_failed"))?;
        file.parent().map(|p| p.to_path_buf()).unwrap_or(file)
    };
    let Ok(canonical) = paths::canonical_existing(&root) else {
        return Err(ApiError::internal("share_start_failed"));
    };
    if let Some(sess) = share_state_snapshot() {
        return ok_json(json!({
            "ok": true, "running": true, "port": sess.port, "token": sess.token, "url": kernel_share_url(&sess),
        }));
    }
    let listener = match TcpListener::bind(("0.0.0.0", 0)) {
        Ok(l) => l,
        // `readmd.py:1285-1292` — one blanket `except Exception` for the whole
        // route, so even the socket failure answers `share_start_failed`.
        Err(_) => return Err(ApiError::internal("share_start_failed")),
    };
    let Ok(addr) = listener.local_addr() else {
        return Err(ApiError::internal("share_start_failed"));
    };
    let port = addr.port();
    let token = kernel_share_token(app, port);
    let session = ShareSession { port, token: token.clone(), _root: canonical.clone() };
    SHARE_STOP.store(false, Ordering::SeqCst);
    std::thread::spawn(move || {
        for incoming in listener.incoming() {
            if SHARE_STOP.load(Ordering::SeqCst) {
                break;
            }
            if let Ok(stream) = incoming {
                kernel_serve_share_conn(stream, &canonical, &token);
            }
        }
    });
    *kernel_share_state().lock().unwrap_or_else(|e| e.into_inner()) = Some(session.clone());
    ok_json(json!({
        "ok": true, "running": true, "port": session.port, "token": session.token, "url": kernel_share_url(&session),
    }))
}

fn share_state_snapshot() -> Option<ShareSession> {
    kernel_share_state().lock().unwrap_or_else(|e| e.into_inner()).clone()
}

fn h_share_status(_app: &Arc<App>, _req: &Request) -> ApiResult<Response> {
    match share_state_snapshot() {
        Some(sess) => ok_json(json!({
            "ok": true, "running": true, "port": sess.port, "token": sess.token, "url": kernel_share_url(&sess),
        })),
        None => ok_json(json!({ "running": false })),
    }
}

fn h_share_stop(_app: &Arc<App>, _req: &Request) -> ApiResult<Response> {
    let taken = kernel_share_state().lock().unwrap_or_else(|e| e.into_inner()).take();
    SHARE_STOP.store(true, Ordering::SeqCst);
    if let Some(sess) = taken {
        let _ = TcpStream::connect(("127.0.0.1", sess.port));
    }
    ok_json(json!({ "ok": true, "running": false }))
}

pub(crate) fn run_powershell_encoded(script: &str) -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let mut u16_bytes = Vec::with_capacity(script.len() * 2);
        for u in script.encode_utf16() {
            u16_bytes.extend_from_slice(&u.to_le_bytes());
        }
        use base64::Engine;
        let b64 = base64::prelude::BASE64_STANDARD.encode(&u16_bytes);
        let output = std::process::Command::new("powershell")
            .creation_flags(CREATE_NO_WINDOW)
            .args(["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-EncodedCommand", &b64])
            .output()
            .ok()?;
        if output.status.success() {
            let res = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !res.is_empty() {
                return Some(res);
            }
        }
    }
    let _ = script;
    None
}

/// `{ok, path}` for a single-pick dialog.  A cancel keeps `path: null`; a
/// platform with no dialog also says why (`error_code: dialog_unavailable`).
fn dialog_single_json(outcome: crate::native_dialogs::DialogOutcome) -> ApiResult<Response> {
    use crate::native_dialogs::DialogOutcome;
    match outcome {
        DialogOutcome::Unavailable => ok_json(json!({ "ok": true, "path": null, "canceled": true, "error_code": "dialog_unavailable" })),
        other => ok_json(json!({ "ok": true, "path": other.single() })),
    }
}

fn dialog_start_dir(body: &Value) -> String {
    body.get("dir")
        .or_else(|| body.get("initial_dir"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn h_dialog_choose_folder(_app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    use crate::native_dialogs::{run, DialogShape, Request as Dlg};
    let body = body_value(req);
    dialog_single_json(run(&Dlg::new(DialogShape::ChooseFolder).dir(&dialog_start_dir(&body))))
}

fn h_dialog_choose_file(_app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    use crate::native_dialogs::{run, DialogShape, Request as Dlg};
    let body = body_value(req);
    dialog_single_json(run(&Dlg::new(DialogShape::ChooseFile).dir(&dialog_start_dir(&body))))
}

fn h_dialog_choose_any_file(_app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    use crate::native_dialogs::{run, DialogShape, Request as Dlg};
    let body = body_value(req);
    dialog_single_json(run(&Dlg::new(DialogShape::ChooseAnyFile).dir(&dialog_start_dir(&body))))
}

fn h_dialog_choose_many_files(_app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    use crate::native_dialogs::{run, DialogOutcome, DialogShape, Request as Dlg};
    let body = body_value(req);
    match run(&Dlg::new(DialogShape::ChooseManyFiles).dir(&dialog_start_dir(&body))) {
        DialogOutcome::Unavailable => ok_json(json!({ "ok": true, "paths": [], "canceled": true, "error_code": "dialog_unavailable" })),
        other => ok_json(json!({ "ok": true, "paths": other.many() })),
    }
}

fn h_dialog_save_file(_app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    use crate::native_dialogs::{run, DialogShape, Request as Dlg};
    let body = body_value(req);
    let default_name = body.get("name").and_then(|v| v.as_str()).unwrap_or("document.md");
    dialog_single_json(run(&Dlg::new(DialogShape::SaveFile).name(default_name).dir(&dialog_start_dir(&body))))
}

/// `{ok:true}` on success, `{ok:false, error_code}` (`path_not_found` /
/// `open_failed`) otherwise, so the UI can say what went wrong.
fn open_result_json(r: Result<(), &'static str>) -> ApiResult<Response> {
    match r {
        Ok(()) => ok_json(json!({ "ok": true })),
        Err(code) => ok_json(json!({
            "ok": false,
            "error_code": code,
            "error": if code == "path_not_found" { "路径不存在" } else { "无法打开" },
        })),
    }
}

fn h_system_open_path(_app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    let body = body_value(req);
    let path = body.get("path").and_then(|v| v.as_str()).or_else(|| req.q("path")).unwrap_or("");
    // URLs (already vetted by the bridge's `bridgeSafeExternalUrl`) go to the
    // default browser; only http(s)/mailto are accepted here.
    let lower = path.trim().to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("mailto:") {
        if path.chars().any(|c| c.is_control()) || path.len() > 2048 {
            return open_result_json(Err("open_failed"));
        }
        #[cfg(windows)]
        let ok = crate::native_system::shell_open(path);
        #[cfg(target_os = "macos")]
        let ok = std::process::Command::new("open").arg(path).spawn().is_ok();
        #[cfg(all(unix, not(target_os = "macos")))]
        let ok = std::process::Command::new("xdg-open").arg(path).spawn().is_ok();
        return open_result_json(if ok { Ok(()) } else { Err("open_failed") });
    }
    open_result_json(crate::native_dialogs::open_path(path))
}

fn h_system_reveal_path(_app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    let body = body_value(req);
    let path = body.get("path").and_then(|v| v.as_str()).or_else(|| req.q("path")).unwrap_or("");
    open_result_json(crate::native_dialogs::reveal_path(path))
}

/// Save-as: pick a target, copy web-clipped assets next to it, then write the
/// document atomically (with the same `.bak` / encoding rules as `/api/save`).
fn h_dialog_save_as(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    use crate::native_dialogs::{run, DialogOutcome, DialogShape, Request as Dlg};
    let body = body_value(req);
    let mut content = body.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let suggested = body.get("suggested").and_then(|v| v.as_str()).unwrap_or("document.md");
    let encoding = body.get("encoding").and_then(|v| v.as_str()).unwrap_or("utf-8").to_string();

    let target_str = match run(&Dlg::new(DialogShape::SaveAs).name(suggested).dir(&dialog_start_dir(&body))) {
        DialogOutcome::Picked(v) if !v.is_empty() => v[0].clone(),
        DialogOutcome::Unavailable => {
            return ok_json(json!({ "ok": false, "canceled": true, "error_code": "dialog_unavailable" }))
        }
        _ => return ok_json(json!({ "ok": false, "canceled": true })),
    };
    let target_path = PathBuf::from(&target_str);

    let mut warns: Vec<String> = Vec::new();
    if let Some(assets) = body.get("assets").and_then(|v| v.as_array()) {
        if !assets.is_empty() {
            let stem = target_path.file_stem().and_then(|s| s.to_str()).unwrap_or("doc");
            let asset_folder_name = format!("{}.assets", stem);
            if let Some(parent) = target_path.parent() {
                let asset_dir = parent.join(&asset_folder_name);
                if let Err(e) = std::fs::create_dir_all(&asset_dir) {
                    warns.push(format!("无法创建资源目录：{e}"));
                }
                for item in assets {
                    let source = item.get("path").and_then(|v| v.as_str()).unwrap_or("");
                    let raw_name = item.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    // The name is a single path segment; anything else is dropped.
                    let name = Path::new(raw_name).file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if source.is_empty() || name.is_empty() || !Path::new(source).is_file() {
                        if !source.is_empty() {
                            warns.push(format!("资源不存在，已跳过：{source}"));
                        }
                        continue;
                    }
                    let dest = asset_dir.join(name);
                    match std::fs::copy(source, &dest) {
                        Ok(_) => {
                            let rel = format!("{}/{}", asset_folder_name, name);
                            content = content.replace(&source.replace('\\', "/"), &rel);
                            content = content.replace(source, &rel);
                        }
                        Err(e) => warns.push(format!("资源复制失败：{name}（{e}）")),
                    }
                }
            }
        }
    }

    let mut saved = py_save_text_atomic(&target_path, &content, &encoding, None);
    if saved.get("ok") != Some(&Value::Bool(true)) {
        return ok_json(saved);
    }
    let display_title = target_path.file_name().and_then(|n| n.to_str()).unwrap_or(&target_str).to_string();
    let _ = app.store.touch_recent(&target_str, &display_title);
    if let Some(map) = saved.as_object_mut() {
        map.insert("path".to_string(), json!(target_str));
        map.insert("warns".to_string(), json!(warns));
    }
    crate::api_codes::attach_warn_items(&mut saved);
    ok_json(saved)
}

const EXPORT_PRESETS_MAX: usize = 100;

fn export_presets_path(app: &App) -> PathBuf {
    app.paths.data_dir.join("export_presets.json")
}

fn load_export_presets(app: &App) -> (serde_json::Map<String, Value>, Value) {
    let stored: Value = std::fs::read(export_presets_path(app))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_else(|| json!({}));
    let custom = stored.get("custom").and_then(|v| v.as_object()).cloned().unwrap_or_default();
    let last = stored.get("last").cloned().filter(|v| v.is_object()).unwrap_or(Value::Null);
    (custom, last)
}

/// `GET` → `{defaults, presets, custom, last}`; `POST {custom?, last?}` stores
/// either half (each custom preset sanitised, at most 100, built-in names
/// refused with `409 preset_name_conflict`).
fn h_export_presets(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    use crate::export_styles as es;
    let (mut custom, mut last) = load_export_presets(app);
    if req.method == "POST" {
        let body = body_value(req);
        if let Some(c) = body.get("custom") {
            let Some(map) = c.as_object() else {
                return Err(ApiError::bad_request("invalid_presets"));
            };
            let builtin = es::preset_names();
            let mut next = serde_json::Map::new();
            for (name, opts) in map {
                let name = name.trim();
                if name.is_empty() || name.chars().count() > 64 {
                    continue;
                }
                if builtin.contains(&name) {
                    return Ok(Response::json_status(409, &json!({ "ok": false, "error_code": "preset_name_conflict", "name": name })));
                }
                if next.len() >= EXPORT_PRESETS_MAX {
                    break;
                }
                next.insert(name.to_string(), es::sanitize(opts));
            }
            custom = next;
        }
        if let Some(l) = body.get("last") {
            if l.is_object() {
                let fmt = l.get("fmt").and_then(|v| v.as_str()).unwrap_or("pdf");
                let opts = l.get("options").cloned().unwrap_or_else(|| json!({}));
                last = json!({ "fmt": fmt, "options": opts, "preset": l.get("preset").cloned().unwrap_or(Value::Null) });
            }
        }
        let doc = json!({ "custom": custom, "last": last });
        let bytes = serde_json::to_vec_pretty(&doc).unwrap_or_default();
        if let Err(e) = content::write_bytes_atomic(&export_presets_path(app), &bytes) {
            return Err(ApiError::internal(format!("write_failed: {e}")));
        }
        return ok_json(json!({ "ok": true }));
    }
    let mut presets = serde_json::Map::new();
    for n in es::preset_names() {
        presets.insert(n.to_string(), es::preset(n).unwrap_or_else(|| json!({})));
    }
    ok_json(json!({
        "ok": true,
        "defaults": es::default_style(),
        "presets": presets,
        "custom": custom,
        "last": last,
    }))
}

/// KERNEL BRIDGE — not a parity route.  `readmd.py`'s HTTP dispatcher
/// (`Handler._route()`) has **no** `/api/export` branch — it only routes
/// `/api/export/epub` and `/api/export/presentation` — so a differential run of
/// this path against Python is expected to differ and is excluded from parity.
///
/// What stands here is the pywebview method `Api.export_doc(fmt, payload)`
/// (`readmd.py:4873`), which `assets/js/features/export.js:1095` reaches through
/// `py.export_doc(...)` for every format except `epub` and `presentation`.  In
/// the native kernel that call is the injected shim in `main.rs:1746`, which
/// posts `Object.assign({format: fmt}, payload)` — hence the
/// `{content, baseDir, suggestedName, options}` readers below — and the answer
/// keeps `Api.export_doc`'s `{ok, path, size, warns, error, canceled}` shape on
/// the success path.  Two known deviations stay on the bridge side: the format
/// set is the same five `readmd.py:4884-4889` accepts, but an unsupported one
/// answers `400 {"ok": false, "error_code": "unsupported_export_format"}` rather
/// than Python's `{ok: false, stage: "options", error: "不支持的导出格式"}`, and the
/// save dialog is a PowerShell `SaveFileDialog` instead of
/// `window.create_file_dialog`.
fn h_export(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    let body = body_value(req);
    let format = body.get("format").and_then(|v| v.as_str()).unwrap_or("pdf").to_lowercase();
    let content = body.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let base_dir = body.get("baseDir")
        .or_else(|| body.get("base_dir"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let suggested_name = body.get("suggestedName")
        .or_else(|| body.get("suggested_name"))
        .and_then(|v| v.as_str())
        .unwrap_or("export")
        .to_string();
    let options = body.get("options").cloned().unwrap_or_else(|| json!({}));
    let mut out_path = body.get("out_path")
        .or_else(|| body.get("outPath"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if crate::win_dialogs::DialogShape::export_filter_text(&format).is_none() {
        return Err(ApiError::bad_request("unsupported_export_format"));
    }
    // Strip the source extension so `note.md` suggests `note.pdf`, not `note.md.pdf`.
    let stem = {
        let s = suggested_name.trim();
        let s = s.rsplit(['/', '\\']).next().unwrap_or(s);
        let lower = s.to_ascii_lowercase();
        [".markdown", ".md", ".mdown", ".mkd", ".txt"]
            .iter()
            .find(|e| lower.ends_with(*e))
            .map(|e| s[..s.len() - e.len()].to_string())
            .unwrap_or_else(|| s.to_string())
    };
    let stem = if stem.is_empty() { "export".to_string() } else { stem };

    if out_path.is_empty() {
        use crate::native_dialogs::{run, DialogOutcome, DialogShape, Request as Dlg};
        let dlg = Dlg::new(DialogShape::Export).name(&stem).export(&format).dir(&base_dir);
        match run(&dlg) {
            DialogOutcome::Picked(v) if !v.is_empty() => out_path = v[0].clone(),
            DialogOutcome::Unavailable => {
                return ok_json(json!({ "ok": false, "canceled": true, "error_code": "dialog_unavailable" }))
            }
            _ => return ok_json(json!({ "ok": false, "canceled": true })),
        }
    }

    // Optional `task_id`: the export becomes cancellable through
    // `/api/task/cancel` (without it, behaviour is exactly as before).
    let task = body
        .get("task_id")
        .and_then(|v| v.as_str())
        .and_then(crate::cancel::register);
    if let Some(t) = &task {
        if t.is_cancelled() {
            return ok_json(cancelled_body());
        }
    }
    // TeX copies images into `<stem>.assets/` beside the output, so it cannot
    // be staged under another name; its commit point is the start of writing.
    let staged = task.is_some() && !matches!(format.as_str(), "tex" | "latex");
    if let Some(t) = task.as_ref().filter(|_| !staged) {
        if !t.try_commit() {
            return ok_json(cancelled_body());
        }
    }
    let target = out_path.clone();
    let write_to = if staged {
        crate::cancel::part_path(Path::new(&out_path)).to_string_lossy().into_owned()
    } else {
        out_path.clone()
    };
    // Commit a staged export: re-check the flag, then one rename.
    let commit = |res_ok: bool| -> Result<(), Value> {
        let Some(t) = task.as_ref().filter(|_| staged) else { return Ok(()) };
        let part = Path::new(&write_to);
        if !res_ok {
            let _ = std::fs::remove_file(part);
            return Ok(());
        }
        if !t.try_commit() {
            let _ = std::fs::remove_file(part);
            return Err(cancelled_body());
        }
        std::fs::rename(part, &target).map_err(|e| {
            let _ = std::fs::remove_file(part);
            json!({ "ok": false, "error": e.to_string(), "error_code": crate::api_codes::INTERNAL_ERROR })
        })
    };
    let out_path = write_to.clone();

    if format == "presentation" {
        let theme = options.get("theme").and_then(|v| v.as_str()).unwrap_or("black");
        let transition = options.get("transition").and_then(|v| v.as_str()).unwrap_or("slide");
        // Standalone: vendor scripts inlined so the file works offline, anywhere.
        return match crate::mdexport::render_presentation_html(&content, &stem, theme, transition, true, &app.paths.assets_dir) {
            Ok(html) => match content::write_bytes_atomic(Path::new(&out_path), html.as_bytes())
                .map_err(|e| json!({ "ok": false, "error": e.to_string() }))
                .and_then(|()| commit(true))
            {
                Ok(()) => ok_json(json!({
                    "ok": true,
                    "path": target,
                    "size": html.len(),
                    "warns": [],
                    "error": null,
                    "canceled": false
                })),
                Err(body) => {
                    let _ = commit(false);
                    ok_json(body)
                }
            },
            Err(e) => ok_json(json!({ "ok": false, "error": e.to_string(), "error_code": "presentation_export_failed" })),
        };
    }

    match crate::mdexport::export_document(&format, &content, &base_dir, &out_path, &options, &suggested_name, &app.paths.assets_dir) {
        Ok(res) => {
            if let Err(body) = commit(res.ok) {
                return ok_json(body);
            }
            let path = if staged { target.clone() } else { res.path.unwrap_or(out_path) };
            let mut body = json!({
                "ok": res.ok,
                "path": path,
                "size": res.size,
                "warns": res.warns.unwrap_or_default(),
                "error": res.error,
                "canceled": res.canceled.unwrap_or(false)
            });
            crate::api_codes::attach_warn_items(&mut body);
            ok_json(body)
        }
        Err(e) => {
            let _ = commit(false);
            ok_json(json!({
                "ok": false,
                "error": e
            }))
        }
    }
}

/// `/api/export` answer for a task cancelled before its commit point.
fn cancelled_body() -> Value {
    json!({ "ok": false, "canceled": false, "error_code": crate::api_codes::CANCELLED, "error": "已取消" })
}

/// `POST /api/task/cancel {"id": …}` — cancel an export / conversion started
/// with a `task_id`, or a batch conversion job by its job id.
fn h_task_cancel(_app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    let body = body_value(req);
    let id = body.get("id").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    if !crate::cancel::valid_id(&id) {
        return Err(ApiError::bad_request("invalid_request"));
    }
    let mut state = crate::cancel::cancel(&id);
    if state == crate::cancel::CancelState::Unknown {
        state = crate::batch2::cancel_convert_job(&id);
    }
    ok_json(json!({ "ok": true, "state": state.as_str() }))
}

/// KERNEL BRIDGE — not a parity route.  `readmd.py`'s HTTP dispatcher has no
/// `/api/file/save-fixed`, so Python answers `404 text/plain "not found"` for
/// it; the route exists only to stand in for the pywebview method
/// `Api.save_fixed` (`readmd.py:5928`), which the injected `main.rs` shim calls
/// as `py.save_fixed(path, content)`.
///
/// The `.readmd` sidecar naming follows Python: `os.path.splitext(path)` then
/// `base + '.readmd' + (ext or '.md')`, with the empty-extension and dot-file
/// cases landing on the same name the Rust branches produce.
///
/// Known shape deviation left in place (outside the P3 mandate, recorded for a
/// later wave): Python's `Api.save_fixed` returns a **bare path string**, or
/// `None` on failure, while this endpoint answers `{'ok': true, 'path': …}` /
/// `400 missing_path` / `500 write_failed`.  Nothing in `assets/**` reads the
/// return value, so the envelope is currently unconsumed.
fn h_file_save_fixed(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    let body = body_value(req);
    let orig_path = body.get("path").and_then(|v| v.as_str()).unwrap_or("");
    let content = body.get("content").and_then(|v| v.as_str()).unwrap_or("");
    if orig_path.is_empty() {
        return Err(ApiError::bad_request("missing_path"));
    }
    let p = Path::new(orig_path);
    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("document");
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("md");
    let out_file_name = format!("{}.readmd.{}", stem, ext);
    let out_path = p.parent().unwrap_or_else(|| Path::new(".")).join(&out_file_name);
    std::fs::write(&out_path, content.as_bytes()).map_err(|e| ApiError::internal(format!("write_failed: {e}")))?;
    let out_str = out_path.to_string_lossy().to_string();
    let _ = app.store.touch_recent(&out_str, &out_file_name);
    ok_json(json!({ "ok": true, "path": out_str }))
}


fn h_system_assoc(_app: &Arc<App>, _req: &Request) -> ApiResult<Response> {
    #[cfg(target_os = "windows")]
    {
        if let Ok(exe_path) = std::env::current_exe() {
            let exe_str = exe_path.to_string_lossy();
            let cmd = format!("\"{}\" \"%1\"", exe_str);
            for ext in [".md", ".markdown", ".mdown", ".mkd"] {
                crate::native_system::win_set_reg_string(
                    crate::native_system::HKCU,
                    &format!(r"Software\Classes\{}", ext),
                    "",
                    "ReadMD.markdown",
                    false,
                );
            }
            crate::native_system::win_set_reg_string(
                crate::native_system::HKCU,
                r"Software\Classes\ReadMD.markdown",
                "",
                "ReadMD Markdown 阅读器",
                false,
            );
            crate::native_system::win_set_reg_string(
                crate::native_system::HKCU,
                r"Software\Classes\ReadMD.markdown\DefaultIcon",
                "",
                &format!("\"{}\",0", exe_str),
                false,
            );
            crate::native_system::win_set_reg_string(
                crate::native_system::HKCU,
                r"Software\Classes\ReadMD.markdown\shell\open\command",
                "",
                &cmd,
                true,
            );
            crate::native_system::win_set_reg_string(
                crate::native_system::HKCU,
                r"Software\Classes\Applications\ReadMD.exe\shell\open\command",
                "",
                &cmd,
                true,
            );
            return ok_json(json!({ "ok": true }));
        }
    }
    ok_json(json!({ "ok": true }))
}

fn h_clipboard_read(_app: &Arc<App>, _req: &Request) -> ApiResult<Response> {
    #[cfg(target_os = "windows")]
    {
        let script = r#"
            $OutputEncoding = [System.Text.Encoding]::UTF8
            [Console]::OutputEncoding = [System.Text.Encoding]::UTF8
            Add-Type -AssemblyName System.Windows.Forms
            $res = @{ text = ""; html = ""; files = @(); image = ""; image_path = ""; source_type = "empty" }
            if ([System.Windows.Forms.Clipboard]::ContainsFileDropList()) {
                $files = [System.Windows.Forms.Clipboard]::GetFileDropList()
                $list = @()
                foreach ($f in $files) {
                    if (Test-Path $f) { $list += $f }
                }
                if ($list.Count -gt 0) {
                    $res.files = $list
                    $res.source_type = "files"
                }
            } elseif ([System.Windows.Forms.Clipboard]::ContainsImage()) {
                $img = [System.Windows.Forms.Clipboard]::GetImage()
                if ($img -ne $null) {
                    $tmp = [System.IO.Path]::Combine([System.IO.Path]::GetTempPath(), "readmd_clip_" + [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() + ".png")
                    $img.Save($tmp, [System.Drawing.Imaging.ImageFormat]::Png)
                    $img.Dispose()
                    $res.image = $tmp
                    $res.image_path = $tmp
                    $res.source_type = "image"
                }
            } elseif ([System.Windows.Forms.Clipboard]::ContainsText([System.Windows.Forms.TextDataFormat]::Html)) {
                $res.html = [System.Windows.Forms.Clipboard]::GetText([System.Windows.Forms.TextDataFormat]::Html)
                $res.source_type = "html"
                if ([System.Windows.Forms.Clipboard]::ContainsText()) {
                    $res.text = [System.Windows.Forms.Clipboard]::GetText()
                }
            } elseif ([System.Windows.Forms.Clipboard]::ContainsText()) {
                $res.text = [System.Windows.Forms.Clipboard]::GetText()
                $res.source_type = "text"
            }
            $json = $res | ConvertTo-Json -Compress
            [Console]::Out.Write($json)
        "#;
        if let Some(json_str) = run_powershell_encoded(script) {
            if let Ok(val) = serde_json::from_str::<Value>(&json_str) {
                return ok_json(val);
            }
        }
    }
    ok_json(json!({
        "text": "",
        "html": "",
        "files": [],
        "image": "",
        "image_path": "",
        "source_type": "empty",
        "error": "剪贴板为空或不包含支持的内容"
    }))
}

#[cfg(test)]
mod batch1_tests {
    use super::*;

    fn dreq(method: &str, target: &str, body: &[u8]) -> Request {
        let (p, q) = match target.split_once('?') {
            Some(v) => v,
            None => (target, ""),
        };
        Request {
            method: method.to_string(),
            path: p.to_string(),
            query: parse_query(q),
            headers: HashMap::new(),
            body: body.to_vec(),
        }
    }


    #[test]
    fn slug_policy_matches_legacy() {
        assert!(kernel_valid_slug("hermes-cat"));
        assert!(kernel_valid_slug("a1_2.3"));
        assert!(!kernel_valid_slug(""));
        assert!(!kernel_valid_slug("-lead"));
        assert!(!kernel_valid_slug("ok/../etc"));
        assert!(!kernel_valid_slug("spaced out"));
    }

    #[test]
    fn base64_decoder_handles_padding_and_data_url() {
        assert_eq!(kernel_b64_decode("aGVsbG8=").unwrap(), b"hello");
        assert_eq!(kernel_b64_decode("YWJjZA==").unwrap(), b"abcd");
        assert_eq!(kernel_b64_decode("YQ").unwrap(), b"a");
        assert!(kernel_b64_decode("!!!!").is_none());
    }

    #[test]
    fn copy_tree_duplicates_a_pet_folder() {
        let dir = std::env::temp_dir().join(format!("readmd-copy-{}", crate::store::now_millis()));
        let src = dir.join("src");
        std::fs::create_dir_all(src.join("sub")).unwrap();
        std::fs::write(src.join("pet.json"), b"{}").unwrap();
        std::fs::write(src.join("sub/s.png"), b"x").unwrap();
        let dst = dir.join("dst");
        assert_eq!(kernel_copy_tree(&src, &dst).unwrap(), 2);
        assert!(dst.join("pet.json").is_file());
        assert!(dst.join("sub/s.png").is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn upstream_sources_degrade_to_empty_catalogue() {
        let tmp = std::env::temp_dir().join(format!("readmd-ups-{}", crate::store::now_millis()));
        let ws = tmp.join("ws");
        std::fs::create_dir_all(&ws).unwrap();
        std::fs::create_dir_all(tmp.join("assets")).unwrap();
        let app = Arc::new(App::bootstrap(paths::AppPaths::with_dirs(
            &tmp,
            &ws,
            &tmp.join("assets"),
        ))
        .unwrap());
        let res = h_upstream_sources(&app, &dreq("GET", "/api/upstream-sources", &[])).unwrap();
        let body: Value = serde_json::from_slice(&res.body).unwrap();
        assert_eq!(body["schema_version"], 1);
        assert_eq!(body["offline"], true);
        assert!(body["sources"].is_array());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn module_gate_reflects_the_real_route_table() {
        let res = h_modules_load_noapp("web");
        assert_eq!(res.status, 200);
        // All legacy endpoints now have stub handlers returning 501 (pending)
        let res = h_modules_load_noapp("ocr");
        assert_eq!(res.status, 200);
        let res = h_modules_load_noapp("nonsense");
        assert_eq!(res.status, 400);
    }

    fn h_modules_load_noapp(name: &str) -> Response {
        let tmp = std::env::temp_dir().join(format!("readmd-mod-{}", crate::store::now_millis()));
        let ws = tmp.join("ws");
        std::fs::create_dir_all(&ws).unwrap();
        std::fs::create_dir_all(tmp.join("assets")).unwrap();
        let app = Arc::new(
            App::bootstrap(paths::AppPaths::with_dirs(&tmp, &ws, &tmp.join("assets"))).unwrap(),
        );
        let req = dreq("POST", "/api/modules/load", json!({ "name": name }).to_string().as_bytes());
        let out = h_modules_load(&app, &req).unwrap_or_else(|e| Response::json_status(e.status, &e.payload()));
        let _ = std::fs::remove_dir_all(&tmp);
        out
    }

}
