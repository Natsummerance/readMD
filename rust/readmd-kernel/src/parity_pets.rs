//! P9 parity surface: desktop pets, Skills, Skill imports and upstream sources.
//!
//! Every handler here mirrors a `readmd.py` `Handler._api_*` method line for
//! line, including the envelope it emits.  Two envelope families exist and a
//! handler must never mix them up:
//!
//! * `_send_api_error` / `_send_json(…, {'ok': False, 'error_code': …})`
//!   ([`ec`]) — the machine-readable family used by the pet lifecycle routes.
//! * `_send_json(…, {'ok': False, 'code': …})` ([`code_err`]) — the family the
//!   pet runtime API objects return through `_api_pet_interact` and the
//!   installer methods, which the two handlers above forward verbatim.
//!
//! `ApiError` is used only where Python answers through `except Exception` with
//! a 500: `ApiError::internal(code)` renders as `ErrorShape::ApiCode`, whose
//! `payload()` (`src/lib.rs`, `ApiError::payload`) emits
//! `{"ok": false, "error_code": <code>}` at the error's own status — exactly
//! `_send_json(500, {'ok': False, 'error_code': …})`.  Bodies a handler decides
//! for itself (403/404/405, the `code` family) are built here with [`ec`] and
//! [`code_err`] so the status and key stay explicit at the call site.

use crate::error::ApiResult;
use crate::server::{http_get, Request, Response};
use crate::{paths, pet_host, pet_paths, App};
use base64::Engine as _;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// ------------------------------------------------------------------ helpers

const MAX_SPRITESHEET: usize = 20 * 1024 * 1024;
const MAX_PET_META: u64 = 64 * 1024;
const MAX_THUMB_BYTES: usize = 5 * 1024 * 1024;
const MAX_IMPORT_BODY: usize = 24 * 1024 * 1024;
const PET_LIFECYCLE_BODY_LIMIT: usize = 65536;

fn json_at(status: u16, value: Value) -> ApiResult<Response> {
    Ok(Response::json_status(status, &value))
}

/// `{'ok': False, 'error_code': <code>}`.
fn ec(status: u16, code: &str) -> ApiResult<Response> {
    json_at(status, json!({ "ok": false, "error_code": code }))
}

/// `{'ok': False, 'code': <code>}` — the runtime API object family.
fn code_err(status: u16, code: &str) -> ApiResult<Response> {
    json_at(status, json!({ "ok": false, "code": code }))
}

/// `bool(value)` for the JSON types a request body can carry: only `false`,
/// the numeric zeros, `""`, `[]`, `{}` and `null` are falsy in Python.
fn py_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(flag) => *flag,
        Value::String(text) => !text.is_empty(),
        Value::Array(items) => !items.is_empty(),
        Value::Object(map) => !map.is_empty(),
        Value::Number(number) => match number.as_f64() {
            Some(number) => number != 0.0,
            None => true,
        },
    }
}

/// Python's `==` between two decoded JSON values.
///
/// `readmd.py:1767` compares `settings.get('pet_slug')` with the *raw*
/// `body.get('slug')`, so serde's structural equality is almost enough — the
/// one place it is not is `bool`, which Python treats as its `0`/`1` value.
fn py_eq(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Bool(flag), other) | (other, Value::Bool(flag)) if other.is_number() => {
            Value::from(i64::from(*flag)) == *other
        }
        _ => left == right,
    }
}

/// `is_py_space`'s set is Python's `str.strip()` whitespace class narrowed to
/// the code points an ISO-8859-1/UTF-8 header value can actually carry: the
/// ASCII controls CPython's `int()` skips (`' \\t\\n\\x0b\\x0c\\r'`), the
/// `\x1c`-`\x1f` separators, and `\x85`/`\xa0`.  The rest of Unicode's space
/// table is not reproduced; see the report.
fn is_py_space(value: char) -> bool {
    matches!(value, ' ' | '\t' | '\n' | '\u{b}' | '\u{c}' | '\r')
        || ('\u{1c}'..='\u{1f}').contains(&value)
        || value == '\u{85}'
        || value == '\u{a0}'
}

/// `repr(str)` — the form CPython embeds in its `int()` failure message,
/// verified against a real interpreter: `'abc'`, `"a'b"` when the text holds a
/// lone single quote, and `\x08`-style escapes for the control range.
fn py_repr(text: &str) -> String {
    let quote = if text.contains('\'') && !text.contains('"') { '"' } else { '\'' };
    let mut out = String::new();
    out.push(quote);
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            found if found == quote => {
                out.push('\\');
                out.push(found);
            }
            found if (found as u32) < 0x20 || found as u32 == 0x7f => {
                out.push_str(&format!("\\x{:02x}", found as u32));
            }
            found => out.push(found),
        }
    }
    out.push(quote);
    out
}

/// CPython's `int(text)` for the acceptance set a `Content-Length` header can
/// reach it with: surrounding whitespace, one optional sign, ASCII digits and
/// single `_` separators *between* digits.  Overflow saturates, because every
/// caller only compares the value against a byte limit and Python's ints are
/// unbounded — a 40-digit declaration is `'request_too_large'`, never a parse
/// failure.
fn py_int(text: &str) -> Result<i64, String> {
    let failure = || Err(format!("invalid literal for int() with base 10: {}", py_repr(text)));
    let trimmed = text.trim_matches(is_py_space);
    let (negative, digits) = match trimmed.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, trimmed.strip_prefix('+').unwrap_or(trimmed)),
    };
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit() || byte == b'_') {
        return failure();
    }
    // `_` is only legal between two digits: `_1`, `1_` and `1__0` all raise.
    if digits.starts_with('_') || digits.ends_with('_') || digits.contains("__") {
        return failure();
    }
    let compact: String = digits.chars().filter(|ch| *ch != '_').collect();
    let unsigned = compact.trim_start_matches('0');
    // `"000"` strips to nothing but is still the integer 0 — without this arm it
    // would saturate and a legitimately empty body would read as
    // `'request_too_large'`.
    let value = if unsigned.is_empty() {
        0
    } else if unsigned.len() > 18 {
        i64::MAX
    } else {
        unsigned.parse::<i64>().unwrap_or(i64::MAX)
    };
    Ok(if negative { -value } else { value })
}

/// CPython's `json.loads` message for the one failure class that is
/// reproducible without porting its whole scanner: the document has no JSON
/// value at its first non-`WHITESPACE` character, which is what any non-JSON
/// body hits first (`scanner.py`'s `Expecting value`, whose `char` offset is a
/// *character* index into the decoded string).  Everything CPython reaches only
/// *after* dispatching on that first token — the string, array and object
/// scanners, and the `Extra data` tail of a complete leading literal or number
/// (`01`, `1.`, `nulll`) — keeps serde's own text.
fn json_decode_message(raw: &[u8], error: &serde_json::Error) -> String {
    let text = match std::str::from_utf8(raw) {
        Ok(text) => text,
        Err(_) => return error.to_string(),
    };
    let chars: Vec<char> = text.chars().collect();
    let position = chars.iter().position(|ch| !matches!(ch, ' ' | '\t' | '\n' | '\r')).unwrap_or(chars.len());
    // `_json.c`'s `#switch (c)` only dispatches into another message for `{`,
    // `[`, `"`, a *complete* `null`/`true`/`false` and a number that has at
    // least one digit; every other leading byte — including `not-json`, `nul`,
    // `tru`, `-` and `-x` — falls straight through to "Expecting value".
    let dispatched = match chars.get(position) {
        Some('{') | Some('[') | Some('"') | Some('0'..='9') => true,
        Some('n') => chars[position..].starts_with(&['n', 'u', 'l', 'l'][..]),
        Some('t') => chars[position..].starts_with(&['t', 'r', 'u', 'e'][..]),
        Some('f') => chars[position..].starts_with(&['f', 'a', 'l', 's', 'e'][..]),
        Some('-') => matches!(chars.get(position + 1), Some('0'..='9')),
        _ => false,
    };
    if dispatched {
        return error.to_string();
    }
    let line_start = chars[..position].iter().rposition(|ch| *ch == '\n').map(|index| index + 1).unwrap_or(0);
    format!(
        "Expecting value: line {} column {} (char {})",
        1 + chars[..position].iter().filter(|ch| **ch == '\n').count(),
        position - line_start + 1,
        position,
    )
}

/// `_read_json_body(limit)` (`readmd.py:1463-1469`) end to end.
///
/// ```python
/// n = int(self.headers.get('Content-Length', 0) or 0)   # readmd.py:1465
/// if not n:                                             # readmd.py:1466
///     return {}                                         # readmd.py:1467
/// raw = self._read_request_body_limited(n, limit)       # readmd.py:1468
/// return json.loads(raw.decode('utf-8'))                # readmd.py:1469
/// ```
///
/// Three consequences the previous shape missed:
///
/// * the `int()` happens *outside* the bounded helper, so a non-integer
///   header raises CPython's own message instead of `'request_too_large'`;
/// * `if not n` short-circuits **before** the parser, so an empty declared
///   body is not an error at all — the handler runs on `{}` and answers
///   200 from the api method's dict;
/// * the size gate is judged on the declared length
///   (`readmd.py:1445`: `if length < 0 or length > int(limit)`, so a
///   negative declaration is the same `'request_too_large'`), never on the
///   bytes the transport happened to buffer.
enum JsonBody {
    /// `if not n: return {}`.
    NoBody,
    /// The decoded document.  A non-object here is what makes Python's
    /// `body.get(...)` raise `AttributeError`, which each route's own `except`
    /// arm turns into that route's default code.
    Parsed(Value),
    /// A `ValueError` escaped the helper; the payload is `str(exc)` exactly as
    /// `readmd.py:1685` / `readmd.py:1702` forward it.
    Raised(String),
}

fn json_body_of(req: &Request, limit: usize) -> JsonBody {
    let declared = match req.header("content-length") {
        None => return JsonBody::NoBody,
        // `... or 0` turns an empty header value into the integer 0 *before*
        // `int()` ever sees it, so it is a missing body rather than a bad one.
        Some(text) if text.is_empty() => return JsonBody::NoBody,
        Some(text) => match py_int(text) {
            Ok(value) => value,
            Err(message) => return JsonBody::Raised(message),
        },
    };
    if declared == 0 {
        return JsonBody::NoBody;
    }
    if declared < 0 || declared > limit as i64 {
        return JsonBody::Raised("request_too_large".to_string());
    }
    let want = declared as usize;
    if req.body.len() < want {
        return JsonBody::Raised("incomplete_request".to_string());
    }
    match serde_json::from_slice::<Value>(&req.body[..want]) {
        Ok(value) => JsonBody::Parsed(value),
        Err(error) => JsonBody::Raised(json_decode_message(&req.body[..want], &error)),
    }
}

/// The three routes that read their body inline instead of through
/// `_read_json_body` (`_api_pet_import` `readmd.py:1733-1738`,
/// `_api_pet_remove` `readmd.py:1760-1762`, `_api_pet_active`
/// `readmd.py:1780-1782`) share this shape: a bare `int()` on the header whose
/// failure the route's own `except Exception` reports, then `json.loads` only
/// when the declaration is truthy.
fn inline_json_body(req: &Request) -> JsonBody {
    let declared = match req.header("content-length") {
        None => return JsonBody::NoBody,
        Some(text) if text.is_empty() => return JsonBody::NoBody,
        Some(text) => match py_int(text) {
            Ok(value) => value,
            Err(message) => return JsonBody::Raised(message),
        },
    };
    if declared == 0 {
        return JsonBody::NoBody;
    }
    // `self.rfile.read(n)` with a negative `n` reads to end-of-stream rather
    // than raising; the transport has already buffered exactly that much.
    let take = if declared < 0 { req.body.len() } else { declared as usize };
    match serde_json::from_slice::<Value>(&req.body[..take.min(req.body.len())]) {
        Ok(value) => JsonBody::Parsed(value),
        Err(error) => JsonBody::Raised(error.to_string()),
    }
}

/// `str(body.get(key) or '')` — `readmd.py:1696-1697` for interact and the
/// four inline copies in import (`:1739-1745`), remove (`:1765`) and active
/// (`:1785`).
fn body_str(body: &Map<String, Value>, key: &str) -> String {
    match body.get(key) {
        None => String::new(),
        Some(value) => py_or_str(value),
    }
}

/// `str(value or '')`.  Python's `or` collapses *every* falsy value to the
/// empty string — `null`, `false`, `0`, `0.0`, `""`, `[]`, `{}` — and the truthy
/// remainder goes through `str()`, which is not JSON text: `True` becomes
/// `'True'`, a float keeps its mandatory decimal point, and containers fall to
/// their `repr`.
fn py_or_str(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(false) => String::new(),
        Value::String(text) => text.clone(),
        Value::Array(items) if items.is_empty() => String::new(),
        Value::Object(map) if map.is_empty() => String::new(),
        Value::Number(number) => {
            // `0`, `-0.0` and `0.0` are all falsy in Python.
            if number.as_f64() == Some(0.0) || number.to_string() == "0" {
                String::new()
            } else {
                number.to_string()
            }
        }
        other => py_repr_value(other),
    }
}

/// `repr(value)` for the JSON types `str()` cannot handle directly.  serde_json
/// is built without `preserve_order` here, so an object prints its keys sorted
/// rather than in document order — a key *set* is identical, only the order
/// differs, and order is an accepted deviation in this rewrite.
fn py_repr_value(value: &Value) -> String {
    match value {
        Value::Null => "None".to_string(),
        Value::Bool(flag) => if *flag { "True".to_string() } else { "False".to_string() },
        Value::String(text) => py_repr(text),
        Value::Number(number) => number.to_string(),
        Value::Array(items) => {
            let parts: Vec<String> = items.iter().map(py_repr_value).collect();
            format!("[{}]", parts.join(", "))
        }
        Value::Object(map) => {
            let parts: Vec<String> = map
                .iter()
                .map(|(key, item)| format!("{}: {}", py_repr(key), py_repr_value(item)))
                .collect();
            format!("{{{}}}", parts.join(", "))
        }
    }
}

fn wall_now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|delta| delta.as_secs_f64())
        .unwrap_or_default()
}

/// `time.monotonic()` — an arbitrary epoch, like CPython's.
fn mono_now() -> f64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    digest.finalize().iter().map(|byte| format!("{byte:02x}")).collect()
}

fn sha256_file(path: &Path) -> Option<String> {
    let mut file = fs::File::open(path).ok()?;
    let mut digest = Sha256::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let read = file.read(&mut buffer).ok()?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Some(digest.finalize().iter().map(|byte| format!("{byte:02x}")).collect())
}

fn read_bounded(path: &Path, limit: usize) -> Option<Vec<u8>> {
    let mut file = fs::File::open(path).ok()?;
    let mut buffer = Vec::new();
    let mut chunk = vec![0u8; 64 * 1024];
    loop {
        let read = file.read(&mut chunk).ok()?;
        if read == 0 {
            break;
        }
        // Python `stream.read(n)` returns at most n bytes and silently ignores
        // the rest, which is how `/api/pets/thumb` caps a spritesheet.
        if buffer.len() + read > limit {
            let take = limit - buffer.len();
            buffer.extend_from_slice(&chunk[..take]);
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
    }
    Some(buffer)
}

fn file_size(path: &Path) -> u64 {
    fs::metadata(path).map(|meta| meta.len()).unwrap_or(0)
}

fn is_symlink(path: &Path) -> bool {
    fs::symlink_metadata(path).map(|meta| meta.file_type().is_symlink()).unwrap_or(false)
}

/// `os.path.realpath` for paths that exist; falls back to a lexical clean.
fn real_path(path: &Path) -> PathBuf {
    match fs::canonicalize(path) {
        Ok(canonical) => paths::strip_verbatim(canonical),
        Err(_) => paths::canonicalize_or_clean(path),
    }
}

/// `^[a-z0-9][a-z0-9-]{0,62}$` (`store._SLUG`).
fn is_pet_slug(slug: &str) -> bool {
    let bytes = slug.as_bytes();
    if bytes.is_empty() || bytes.len() > 63 {
        return false;
    }
    let first = bytes[0];
    if !(first.is_ascii_lowercase() || first.is_ascii_digit()) {
        return false;
    }
    bytes[1..]
        .iter()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
}

/// `^[a-z0-9][a-z0-9-]{0,63}$` (`skills._NAME_RE` / `skill_import._ID_RE`).
fn is_kebab_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.is_empty() || bytes.len() > 64 {
        return false;
    }
    let first = bytes[0];
    if !(first.is_ascii_lowercase() || first.is_ascii_digit()) {
        return false;
    }
    bytes[1..]
        .iter()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
}

/// `store.slugify`.
fn slugify(value: &str) -> String {
    let lowered = value.trim().to_ascii_lowercase();
    let mut out = String::new();
    let mut pending_dash = false;
    for ch in lowered.chars() {
        if ch.is_ascii_lowercase() || ch.is_ascii_digit() {
            if pending_dash && !out.is_empty() {
                out.push('-');
            }
            pending_dash = false;
            out.push(ch);
        } else {
            pending_dash = true;
        }
    }
    let trimmed: String = out.chars().take(63).collect();
    let trimmed = trimmed.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "pet".to_string()
    } else {
        trimmed
    }
}

/// `skill_import._slug`.
fn import_slug(value: &str) -> String {
    let lowered = value.to_ascii_lowercase();
    let mut out = String::new();
    let mut pending_dash = false;
    for ch in lowered.chars() {
        if ch.is_ascii_lowercase() || ch.is_ascii_digit() {
            if pending_dash && !out.is_empty() {
                out.push('-');
            }
            pending_dash = false;
            out.push(ch);
        } else {
            pending_dash = true;
        }
    }
    let mut out: String = out.chars().take(64).collect();
    while out.ends_with('-') {
        out.pop();
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "imported-skill".to_string()
    } else {
        out
    }
}

// --------------------------------------------------------------- pet store

#[derive(Debug, Clone)]
pub struct InstalledPet {
    pub slug: String,
    pub display_name: String,
    pub description: String,
    pub directory: PathBuf,
    pub spritesheet: PathBuf,
    pub sha256: String,
    pub is_builtin: bool,
}

impl InstalledPet {
    /// `InstalledPet.as_dict`.
    pub fn as_dict(&self) -> Value {
        json!({
            "slug": self.slug,
            "display_name": self.display_name,
            "description": self.description,
            "directory": self.directory.to_string_lossy().into_owned(),
            "spritesheet": self.spritesheet.to_string_lossy().into_owned(),
            "sha256": self.sha256,
            "is_builtin": self.is_builtin,
        })
    }
}

/// `PetStoreError` — carries `.code`, which the handlers forward verbatim.
#[derive(Debug, Clone)]
pub struct StoreError {
    pub code: String,
}

impl StoreError {
    fn new(code: &str) -> StoreError {
        StoreError { code: code.to_string() }
    }
}

/// `BUILTIN_PETS` (declaration order matters: `get_builtin_pets` iterates it).
const BUILTIN_PETS: &[(&str, &str, &str, &str)] = &[
    (
        "mochi",
        "糯米 / Mochi",
        "奶油白的小猫，焦糖色耳尖与尾巴，系一条鼠尾草绿围巾。",
        "mochi-sprite.png",
    ),
    (
        "moss",
        "苔苔 / Moss",
        "圆润的浅绿色史莱姆，头顶两片嫩叶，深墨绿豆豆眼。",
        "moss-sprite.png",
    ),
    (
        "amber",
        "琥珀 / Amber",
        "橘色小狐狸，奶白胸口与尾尖，短腿大尾巴。",
        "amber-sprite.png",
    ),
];

/// `_assets_dir()` — `APP_DIR/assets/pet`.
fn pet_assets(app: &App) -> PathBuf {
    app.paths.assets_dir.join("pet")
}

/// `APP_DIR` — the directory that owns `assets/`.
pub fn app_dir(app: &App) -> PathBuf {
    app.paths
        .assets_dir
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| app.paths.assets_dir.clone())
}

/// The pets library root: Python's `config.DATA_DIR`.
///
/// Every call site below is a `readmd.py` line that passes `DATA_DIR` —
/// `list_pets(DATA_DIR)` (`:1656`), `register_local_pet(DATA_DIR, …)` (`:1744`),
/// `remove_pet(DATA_DIR, …)` (`:1767`), `find_pet(DATA_DIR, …)` (`:1788`),
/// `PetCompanion(DATA_DIR)` (`:3739`) — and `DATA_DIR` comes from
/// `_platform_data_dir` (`config.py:22-46`), which appends `ReadMD`.  The
/// kernel's own `paths::data_dir()` appends a lowercase `readmd` instead; on
/// Windows the two spellings name the same directory, on Linux and macOS they
/// do not, so reading pets through `app.paths.data_dir` alone silently points
/// the kernel at an empty library.
///
/// An `App` built with an explicitly injected tree (`AppPaths::with_dirs`) keeps
/// it — that injection is the Rust equivalent of Python's `READMD_DATA_DIR`
/// isolation override, and must not be replaced by a platform walk.
fn pet_data_dir(app: &App) -> PathBuf {
    if app.paths.data_dir == paths::data_dir() {
        pet_paths::data_dir()
    } else {
        app.paths.data_dir.clone()
    }
}

fn builtin_asset_path(pet_assets: &Path, filename: &str) -> Option<PathBuf> {
    let asset = pet_assets.join(filename);
    if asset.is_file() {
        return Some(asset);
    }
    let cwd_asset = Path::new("assets/pet").join(filename);
    if cwd_asset.is_file() {
        return Some(real_path(&cwd_asset));
    }
    None
}

/// `get_builtin_pets()`.
pub fn builtin_pets(pet_assets: &Path) -> Vec<InstalledPet> {
    let mut res = Vec::new();
    for (slug, display_name, description, filename) in BUILTIN_PETS {
        let Some(path) = builtin_asset_path(pet_assets, filename) else {
            continue;
        };
        res.append(&mut vec![InstalledPet {
            slug: (*slug).to_string(),
            display_name: (*display_name).to_string(),
            description: (*description).to_string(),
            sha256: sha256_file(&path).unwrap_or_default(),
            directory: path.parent().map(Path::to_path_buf).unwrap_or_default(),
            spritesheet: path,
            is_builtin: true,
        }]);
    }
    res
}

/// `get_catalog_pets()` — the shipped 73-pet catalog, empty when absent.
pub fn catalog_pets(pet_assets: &Path) -> Vec<Value> {
    static CACHE: OnceLock<Mutex<Option<Vec<Value>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Ok(guard) = cache.lock() {
        if let Some(items) = &*guard {
            return items.clone();
        }
    }
    let path = pet_assets.join("catalog.json");
    let items = fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default();
    if let Ok(mut guard) = cache.lock() {
        *guard = Some(items.clone());
    }
    items
}

/// `_spritesheet_path`.
fn spritesheet_path(directory: &Path, metadata: &Value) -> Option<PathBuf> {
    let declared = metadata
        .get("spritesheetPath")
        .or_else(|| metadata.get("spritesheet_path"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let mut names: Vec<&str> = Vec::new();
    if !declared.is_empty() {
        names.push(declared);
    }
    let root = real_path(directory);
    for name in names.into_iter().chain(["spritesheet.webp", "spritesheet.png", "sprite.webp", "sprite.png"]) {
        let candidate = directory.join(name);
        let resolved = real_path(&candidate);
        if resolved.parent().map(real_path).as_deref() != Some(root.as_path()) || is_symlink(&candidate) {
            continue;
        }
        let suffix = candidate
            .extension()
            .map(|ext| ext.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        if candidate.is_file() && (suffix == "webp" || suffix == "png") {
            return Some(candidate);
        }
    }
    None
}

/// `list_pets(data_dir, include_builtins)`.
pub fn list_pets(data_dir: &Path, pet_assets: &Path, include_builtins: bool) -> Vec<InstalledPet> {
    let root = data_dir.join("pets");
    let _ = fs::create_dir_all(&root);
    let mut result = Vec::new();
    if include_builtins {
        result.extend(builtin_pets(pet_assets));
    }
    let mut directories: Vec<PathBuf> = match fs::read_dir(&root) {
        Ok(read) => read.flatten().map(|entry| entry.path()).collect(),
        Err(_) => return result,
    };
    directories.sort_by_key(|path| {
        path.file_name()
            .map(|name| name.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default()
    });
    for directory in directories {
        let Some(name) = directory.file_name().map(|name| name.to_string_lossy().into_owned()) else {
            continue;
        };
        // Python sorts by the raw `Path.name`; names are ASCII slugs in
        // practice, and anything else fails `_safe_slug` anyway.
        let Ok(slug) = safe_slug(&name) else {
            continue;
        };
        if !directory.is_dir() || is_symlink(&directory) {
            continue;
        }
        let meta_path = directory.join("pet.json");
        if !meta_path.is_file() || file_size(&meta_path) > MAX_PET_META {
            continue;
        }
        let Ok(text) = fs::read_to_string(&meta_path) else {
            continue;
        };
        let Ok(metadata) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        if !metadata.is_object() {
            continue;
        }
        let Some(sprite) = spritesheet_path(&directory, &metadata) else {
            continue;
        };
        if file_size(&sprite) as usize > MAX_SPRITESHEET {
            continue;
        }
        let display_name = metadata
            .get("displayName")
            .or_else(|| metadata.get("display_name"))
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string())
            .unwrap_or_else(|| slug.clone());
        result.push(InstalledPet {
            sha256: sha256_file(&sprite).unwrap_or_default(),
            description: metadata
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            display_name,
            directory,
            slug,
            spritesheet: sprite,
            is_builtin: false,
        });
    }
    result
}

/// `store._safe_slug`.
fn safe_slug(value: &str) -> Result<String, StoreError> {
    let slug = value.trim().to_ascii_lowercase();
    if !is_pet_slug(&slug) {
        return Err(StoreError::new("pet_slug_invalid"));
    }
    Ok(slug)
}

/// `find_pet`.
pub fn find_pet(data_dir: &Path, pet_assets: &Path, slug: &str) -> Option<InstalledPet> {
    let slug = slug.trim().to_ascii_lowercase();
    if slug.is_empty() {
        return None;
    }
    if let Some(pet) = list_pets(data_dir, pet_assets, true)
        .into_iter()
        .find(|pet| pet.slug == slug)
    {
        return Some(pet);
    }
    for item in catalog_pets(pet_assets) {
        if item.get("slug").and_then(Value::as_str) != Some(slug.as_str()) {
            continue;
        }
        let Some(sprite_path) = resolve_catalog_pet_path(pet_assets, &slug) else {
            continue;
        };
        if !sprite_path.is_file() {
            continue;
        }
        return Some(InstalledPet {
            slug: slug.clone(),
            display_name: ["zh_name", "en_name"]
                .iter()
                .find_map(|key| item.get(*key).and_then(Value::as_str).filter(|v| !v.is_empty()))
                .map(|value| value.to_string())
                .unwrap_or_else(|| slug.clone()),
            description: item
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            sha256: sha256_file(&sprite_path).unwrap_or_default(),
            directory: sprite_path.parent().map(Path::to_path_buf).unwrap_or_default(),
            spritesheet: sprite_path,
            is_builtin: false,
        });
    }
    None
}

/// `resolve_catalog_pet_path`.
fn resolve_catalog_pet_path(pet_assets: &Path, slug: &str) -> Option<PathBuf> {
    for candidate in [
        pet_assets.join(slug).join("spritesheet.webp"),
        pet_assets.join(slug).join("spritesheet.png"),
        pet_assets.join(format!("{slug}-sprite.png")),
        pet_assets.join("thumbs").join(format!("{slug}.png")),
    ] {
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// `remove_pet`.
pub fn remove_pet(data_dir: &Path, slug: &str) -> Result<(), StoreError> {
    let slug = safe_slug(slug)?;
    if BUILTIN_PETS.iter().any(|(name, ..)| *name == slug) {
        return Err(StoreError::new("pet_cannot_delete_builtin"));
    }
    let root = data_dir.join("pets");
    let _ = fs::create_dir_all(&root);
    let root_real = real_path(&root);
    let directory = root.join(&slug);
    let resolved = real_path(&directory);
    let escaped = resolved.parent().map(|parent| parent != root_real.as_path()).unwrap_or(true);
    if escaped || !directory.is_dir() || is_symlink(&directory) || !directory.join("pet.json").is_file() {
        return Err(StoreError::new("pet_not_found"));
    }
    match fs::remove_dir_all(&resolved) {
        Ok(_) => Ok(()),
        Err(_) => Err(StoreError::new("pet_not_found")),
    }
}

/// `register_local_pet`, minus `normalize_and_segment_spritesheet` (a PIL
/// dependency the Rust kernel must not imitate silently).
pub fn register_local_pet(
    data_dir: &Path,
    raw_slug: &str,
    spritesheet: &[u8],
    display_name: &str,
    description: &str,
    replace: bool,
) -> Result<InstalledPet, StoreError> {
    let raw = raw_slug.trim();
    if raw.is_empty() || raw.contains('/') || raw.contains('\\') || raw == "." || raw == ".." {
        return Err(StoreError::new("pet_slug_invalid"));
    }
    let slug = slugify(raw);
    if !is_pet_slug(&slug) {
        return Err(StoreError::new("pet_slug_invalid"));
    }
    if spritesheet.is_empty() {
        return Err(StoreError::new("pet_spritesheet_missing"));
    }
    if spritesheet.len() > MAX_SPRITESHEET {
        return Err(StoreError::new("pet_spritesheet_too_large"));
    }
    let is_png = spritesheet.starts_with(b"\x89PNG\r\n\x1a\n");
    let is_webp = spritesheet.len() >= 12
        && &spritesheet[0..4] == b"RIFF"
        && &spritesheet[8..12] == b"WEBP";
    if !is_png && !is_webp {
        return Err(StoreError::new("pet_spritesheet_format_invalid"));
    }
    let root = data_dir.join("pets");
    let _ = fs::create_dir_all(&root);
    let directory = root.join(&slug);
    let exists = directory.exists();
    if is_symlink(&directory) || (exists && !directory.is_dir()) {
        return Err(StoreError::new("pet_destination_invalid"));
    }
    if exists && !replace {
        return Err(StoreError::new("pet_already_exists"));
    }
    fs::create_dir_all(&directory).map_err(|_| StoreError::new("pet_destination_invalid"))?;
    let sprite_name = if is_png { "spritesheet.png" } else { "spritesheet.webp" };
    atomic_write(&directory.join(sprite_name), spritesheet)?;
    let metadata = json!({
        "id": slug,
        "displayName": clip(display_name, 200).unwrap_or_else(|| slug.clone()),
        "description": clip(description, 2000).unwrap_or_default(),
        "spritesheetPath": sprite_name,
        "createdBy": "readmd-local-import",
    });
    // `json.dumps(..., ensure_ascii=False, indent=2)`
    let text = serde_json::to_string_pretty(&metadata).map_err(|_| StoreError::new("pet_import_invalid"))?;
    atomic_write(&directory.join("pet.json"), text.as_bytes())?;
    list_pets(data_dir, &PathBuf::new(), false)
        .into_iter()
        .find(|pet| pet.slug == slug)
        .ok_or_else(|| StoreError::new("pet_import_invalid"))
}

/// Python `str(value or default)[:limit]` — character counted, like `str` slices.
fn clip(value: &str, limit: usize) -> Option<String> {
    let text = if value.is_empty() { None } else { Some(value) };
    Some(
        text.map(|value| value.chars().take(limit).collect::<String>())
            .unwrap_or_default(),
    )
}

fn atomic_write(path: &Path, data: &[u8]) -> Result<(), StoreError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|_| StoreError::new("pet_destination_invalid"))?;
    let tmp = parent.join(format!(
        ".{}.{}",
        path.file_name().and_then(|name| name.to_str()).unwrap_or("part"),
        std::process::id()
    ));
    fs::write(&tmp, data).map_err(|_| StoreError::new("pet_destination_invalid"))?;
    let renamed = fs::rename(&tmp, path).or_else(|_| {
        let copied = fs::copy(&tmp, path).map(|_| ());
        copied
    });
    let _ = fs::remove_file(&tmp);
    renamed.map_err(|_| StoreError::new("pet_destination_invalid"))
}

// ------------------------------------------------------------ pet controller

/// `PetController`: renderer-independent task state.
#[derive(Debug, Default, Clone)]
pub struct PetController {
    enabled: bool,
    reduced_motion: bool,
    fullscreen: bool,
    quiet: bool,
    tasks: BTreeSet<String>,
    feedback: Option<String>,
    feedback_until: f64,
    updated_at: f64,
    revision: u64,
}

const MAX_TASKS: usize = 256;

impl PetController {
    fn fresh() -> PetController {
        PetController {
            updated_at: mono_now(),
            ..Default::default()
        }
    }

    fn touch(&mut self) {
        self.updated_at = mono_now();
        self.revision += 1;
    }

    fn set_flag(&mut self, key: FlagField, value: bool) -> Value {
        let current = match key {
            FlagField::Enabled => &mut self.enabled,
            FlagField::ReducedMotion => &mut self.reduced_motion,
            FlagField::Fullscreen => &mut self.fullscreen,
            FlagField::Quiet => &mut self.quiet,
        };
        if *current != value {
            *current = value;
            self.touch();
        }
        self.snapshot()
    }

    pub fn enable(&mut self) -> Value {
        self.set_flag(FlagField::Enabled, true)
    }
    pub fn disable(&mut self) -> Value {
        self.set_flag(FlagField::Enabled, false)
    }
    pub fn set_reduced_motion(&mut self, value: bool) -> Value {
        self.set_flag(FlagField::ReducedMotion, value)
    }
    pub fn set_quiet(&mut self, value: bool) -> Value {
        self.set_flag(FlagField::Quiet, value)
    }
    pub fn set_fullscreen(&mut self, value: bool) -> Value {
        self.set_flag(FlagField::Fullscreen, value)
    }

    /// `handle_event(event, task_id)`.
    pub fn handle_event(&mut self, event: &str, task_id: Option<&str>) -> Result<Value, String> {
        let state = match event {
            "work_started" => "busy",
            "work_succeeded" => "success",
            "work_failed" => "error",
            "work_cancelled" => "idle",
            "idle" => "idle",
            _ => return Err("unknown_pet_event".to_string()),
        };
        let _ = state;
        let key = task_id.unwrap_or("__legacy__").to_string();
        if key.is_empty() || key.chars().count() > 256 {
            return Err("invalid_pet_task_id".to_string());
        }
        let before_tasks = self.tasks.clone();
        let before_feedback = self.feedback.clone();
        let before_until = self.feedback_until;
        let moment = mono_now();
        if event == "work_started" {
            if !self.tasks.contains(&key) && self.tasks.len() >= MAX_TASKS {
                return Err("pet_task_capacity".to_string());
            }
            self.tasks.insert(key);
        } else if event == "idle" {
            self.tasks.remove("__legacy__");
            self.feedback = None;
            self.feedback_until = 0.0;
        } else {
            if task_id.is_some() && !self.tasks.contains(&key) {
                return Ok(self.snapshot());
            }
            self.tasks.remove(&key);
            if event == "work_failed" {
                self.feedback = Some("error".to_string());
                self.feedback_until = moment + 5.0;
            } else if event == "work_succeeded"
                && !(self.feedback.as_deref() == Some("error") && moment < self.feedback_until)
            {
                self.feedback = Some("success".to_string());
                self.feedback_until = moment + 3.0;
            }
        }
        if before_tasks != self.tasks
            || before_feedback != self.feedback
            || before_until != self.feedback_until
        {
            self.touch();
        }
        Ok(self.snapshot())
    }

    /// `snapshot()`.
    pub fn snapshot(&mut self) -> Value {
        let moment = mono_now();
        if self.feedback.is_some() && moment >= self.feedback_until {
            self.feedback = None;
            self.feedback_until = 0.0;
            self.touch();
        }
        let visible = self.enabled && !self.fullscreen;
        let activity = if self.feedback.as_deref() == Some("error") {
            "error"
        } else if !self.tasks.is_empty() {
            "busy"
        } else {
            match &self.feedback {
                Some(feedback) => feedback.as_str(),
                None => "idle",
            }
        };
        let animation_enabled = visible && !self.reduced_motion;
        let fps_cap: i64 = if !animation_enabled {
            0
        } else if activity != "idle" {
            30
        } else {
            6
        };
        json!({
            "enabled": self.enabled,
            "visible": visible,
            "state": if visible { activity } else { "hidden" },
            "activity_state": activity,
            "active_tasks": self.tasks.len(),
            "quiet": self.quiet,
            "animation_enabled": animation_enabled,
            "fps_cap": fps_cap,
            "updated_at": self.updated_at,
            "revision": self.revision,
        })
    }
}

#[derive(Clone, Copy)]
enum FlagField {
    Enabled,
    ReducedMotion,
    Fullscreen,
    Quiet,
}

// -------------------------------------------------------------- companion

/// `PetCompanion`: the offline companionship profile store.
pub struct PetCompanion {
    path: PathBuf,
    profiles: Map<String, Value>,
}

const COOLDOWNS: [(&str, f64); 5] = [("pet", 2.0), ("feed", 30.0), ("play", 20.0), ("rest", 0.0), ("wake", 0.0)];
const ACTIONS: [&str; 5] = ["pet", "feed", "play", "rest", "wake"];
const MAX_PROFILES: usize = 256;

/// The two failure families `_api_pet_interact` (`readmd.py:1701-1705`) keeps
/// apart.  A bare `Err(String)` could only ever speak the first one, which is
/// why the route's 500 arm was unreachable in the port.
pub enum CompanionError {
    /// `companion.py`'s `ValueError`s — `'invalid_pet_action'` (`:81`),
    /// `'invalid_pet_character'` (`:41`) and `'pet_character_capacity'`
    /// (`:64`) — whose `str(e)` the route forwards verbatim:
    /// `400 {'ok': False, 'code': str(e)}`.
    ValueError(String),
    /// Any other exception escaping `interact_pet`.  In Python that is
    /// `PetCompanion._save`'s `mkdir` / `write_text` / `os.replace`
    /// (`companion.py:110-118`), all unguarded `OSError`s, reaching
    /// `readmd.py:1703-1705`'s `500 {'ok': False, 'code': 'pet_interact_failed'}`.
    Unexpected,
}

impl PetCompanion {
    /// `PetCompanion.__init__` — a corrupt or oversized file is ignored.
    pub fn load(data_dir: &Path) -> PetCompanion {
        let path = data_dir.join("pet").join("companion.json");
        let mut profiles = Map::new();
        if file_size(&path) > 0 && file_size(&path) <= 1024 * 1024 {
            if let Ok(text) = fs::read_to_string(&path) {
                if let Ok(Value::Object(raw)) = serde_json::from_str::<Value>(&text) {
                    if raw.get("version").and_then(Value::as_i64) == Some(1) {
                        if let Some(Value::Object(items)) = raw.get("profiles") {
                            for (key, value) in items.iter().take(256) {
                                profiles.insert(key.clone(), value.clone());
                            }
                        }
                    }
                }
            }
        }
        PetCompanion { path, profiles }
    }

    fn number(value: Option<&Value>, default: f64, low: f64, high: f64) -> f64 {
        let parsed = match value {
            Some(Value::Number(number)) => number.as_f64(),
            Some(Value::String(text)) => text.parse::<f64>().ok(),
            Some(Value::Bool(flag)) => Some(if *flag { 1.0 } else { 0.0 }),
            _ => None,
        };
        match parsed {
            Some(number) if number.is_finite() => number.max(low).min(high),
            _ => default,
        }
    }

    fn profile(&mut self, character: &str) -> Result<Value, CompanionError> {
        let count = character.chars().count();
        if character.is_empty() || count > 128 {
            return Err(CompanionError::ValueError("invalid_pet_character".to_string()));
        }
        let now = wall_now();
        let raw = self
            .profiles
            .get(character)
            .cloned()
            .unwrap_or(Value::Null);
        let raw = match raw {
            Value::Object(map) => map,
            _ => Map::new(),
        };
        let mut profile = Map::new();
        profile.insert("energy".into(), json!(Self::number(raw.get("energy"), 80.0, 0.0, 100.0)));
        profile.insert("mood".into(), json!(Self::number(raw.get("mood"), 75.0, 0.0, 100.0)));
        profile.insert(
            "affection".into(),
            json!(Self::number(raw.get("affection"), 0.0, 0.0, 100.0)),
        );
        profile.insert(
            "xp".into(),
            json!(Self::number(raw.get("xp"), 0.0, 0.0, 1_000_000.0) as i64),
        );
        profile.insert(
            "resting".into(),
            json!(raw.get("resting").and_then(Value::as_bool) == Some(true)),
        );
        profile.insert(
            "updated_at".into(),
            json!(Self::number(raw.get("updated_at"), now, 0.0, now.max(0.0))),
        );
        profile.insert(
            "last_actions".into(),
            match raw.get("last_actions") {
                Some(value @ Value::Object(_)) => value.clone(),
                _ => Value::Object(Map::new()),
            },
        );
        profile.insert(
            "revision".into(),
            json!(Self::number(raw.get("revision"), 0.0, 0.0, 1_000_000_000.0) as i64),
        );
        profile.insert(
            "last_action".into(),
            json!(raw.get("last_action").and_then(Value::as_str).unwrap_or("")),
        );
        let mut profile = Value::Object(profile);
        let updated_at = profile["updated_at"].as_f64().unwrap_or(now);
        let elapsed = (now - updated_at).clamp(0.0, 86400.0);
        if elapsed >= 60.0 {
            if profile["resting"].as_bool().unwrap_or(false) {
                let energy = profile["energy"].as_f64().unwrap_or(0.0);
                profile["energy"] = json!((energy + elapsed / 60.0 * 2.0).min(100.0));
            }
            profile["updated_at"] = json!(now);
        }
        if !self.profiles.contains_key(character) && self.profiles.len() >= MAX_PROFILES {
            return Err(CompanionError::ValueError("pet_character_capacity".to_string()));
        }
        self.profiles.insert(character.to_string(), profile.clone());
        Ok(profile)
    }

    /// `snapshot(character)`.
    pub fn snapshot(&mut self, character: &str) -> Result<Value, CompanionError> {
        let mut profile = self.profile(character)?;
        let now = wall_now();
        let xp = profile["xp"].as_i64().unwrap_or(0);
        profile["level"] = json!(1 + xp / 50);
        profile["character"] = json!(character);
        profile["energy"] = json!(round_half_even(profile["energy"].as_f64().unwrap_or(0.0)));
        profile["mood"] = json!(round_half_even(profile["mood"].as_f64().unwrap_or(0.0)));
        let mut cooldowns = Map::new();
        for (action, seconds) in COOLDOWNS {
            let last = profile["last_actions"]
                .get(action)
                .map(|value| Self::number(Some(value), 0.0, 0.0, now))
                .unwrap_or(0.0);
            let wait = (last + seconds - now).max(0.0).ceil();
            cooldowns.insert(action.to_string(), json!(wait as i64));
        }
        profile["cooldowns"] = Value::Object(cooldowns);
        Ok(profile)
    }

    /// `interact(character, action)`.
    pub fn interact(&mut self, character: &str, action: &str) -> Result<Value, CompanionError> {
        if !ACTIONS.contains(&action) {
            return Err(CompanionError::ValueError("invalid_pet_action".to_string()));
        }
        // `interact` reads the cooldown through `snapshot`, which may accrue
        // idle energy first, then re-reads the (now updated) profile.
        let wait = self
            .snapshot(character)?
            .get("cooldowns")
            .and_then(|value| value.get(action))
            .and_then(Value::as_i64)
            .unwrap_or(0);
        let mut profile = self
            .profiles
            .get(character)
            .cloned()
            .unwrap_or_else(|| json!({}));
        if wait != 0 {
            let companion = self.snapshot(character)?;
            return Ok(json!({
                "ok": false,
                "code": "pet_action_cooldown",
                "retry_after": wait,
                "companion": companion,
            }));
        }
        let energy = profile["energy"].as_f64().unwrap_or(0.0);
        if action == "play" && energy < 10.0 {
            let companion = self.snapshot(character)?;
            return Ok(json!({ "ok": false, "code": "pet_needs_rest", "companion": companion }));
        }
        let mood = profile["mood"].as_f64().unwrap_or(0.0);
        match action {
            "feed" => {
                profile["energy"] = json!((energy + 15.0).min(100.0));
                profile["mood"] = json!((mood + 4.0).min(100.0));
            }
            "play" => {
                profile["energy"] = json!(energy - 10.0);
                profile["mood"] = json!((mood + 12.0).min(100.0));
                profile["resting"] = json!(false);
            }
            "pet" => {
                profile["mood"] = json!((mood + 6.0).min(100.0));
            }
            _ => {
                profile["resting"] = json!(action == "rest");
            }
        }
        if matches!(action, "pet" | "feed" | "play") {
            let affection = profile["affection"].as_f64().unwrap_or(0.0);
            let xp = profile["xp"].as_i64().unwrap_or(0);
            profile["affection"] = json!((affection + 1.0).min(100.0));
            profile["xp"] = json!((xp + if action == "play" { 10 } else { 3 }).min(1_000_000));
        }
        profile["revision"] = json!(profile["revision"].as_i64().unwrap_or(0) + 1);
        profile["last_action"] = json!(action);
        let now = wall_now();
        if let Some(map) = profile["last_actions"].as_object_mut() {
            map.insert(action.to_string(), json!(now));
        }
        profile["updated_at"] = json!(now);
        self.profiles.insert(character.to_string(), profile);
        if let Err(error) = self.save() {
            // Python lets `_save`'s OSError escape `interact` untouched, so it
            // lands in `readmd.py:1703-1705`, not in the `ValueError` arm.
            let _ = error;
            return Err(CompanionError::Unexpected);
        }
        let companion = self.snapshot(character)?;
        Ok(json!({ "ok": true, "companion": companion }))
    }

    /// `PetCompanion._save` (`companion.py:110-118`).  All three of its file
    /// operations are unguarded in Python — `mkdir(parents=True,
    /// exist_ok=True)` still raises `FileExistsError` when the parent exists as
    /// a *file*, and `write_text`/`os.replace` raise their own `OSError`s — so
    /// a failure here is exactly the "not a ValueError" case that reaches
    /// `readmd.py:1703-1705`.  Python has no copy-on-rename-failure fallback
    /// either, which is why the previous `fs::copy` retry is gone.
    fn save(&self) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let payload = json!({ "version": 1, "profiles": self.profiles });
        let text = serde_json::to_string(&payload).map_err(|error| {
            // `json.dumps(..., allow_nan=False)` would raise `ValueError` here;
            // no profile field can hold a non-finite number (`Self::number`
            // rejects those), so this stays an IO-flavoured failure.
            std::io::Error::new(std::io::ErrorKind::InvalidInput, error.to_string())
        })?;
        let tmp = self.path.with_extension("tmp");
        fs::write(&tmp, text.as_bytes())?;
        let replaced = fs::rename(&tmp, &self.path);
        // `finally: temporary.unlink(missing_ok=True)` — a no-op after a
        // successful replace, which has already moved the file.
        let _ = fs::remove_file(&tmp);
        replaced
    }
}

/// CPython's `round()` is banker's rounding; the companion only ever rounds
/// energies in `[0, 100]`, so match the halves-to-even rule explicitly.
fn round_half_even(value: f64) -> f64 {
    let floor = value.floor();
    let delta = value - floor;
    if delta > 0.5 {
        floor + 1.0
    } else if delta < 0.5 {
        floor
    } else if ((floor as i64) % 2).abs() == 1 {
        floor + 1.0
    } else {
        floor
    }
}

// ---------------------------------------------------- runtime installer tree

/// `RustPetRuntimeInstaller` / `HermesPetPluginInstaller` targets.
#[derive(Debug, Clone)]
pub struct RuntimeTree {
    pub root: PathBuf,
    pub target: PathBuf,
    pub kind: RuntimeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeKind {
    Rust,
    Electron,
}

impl RuntimeTree {
    fn install_root(app: &App) -> PathBuf {
        // `get_default_pet_install_root()` == `<APP_DIR>/plugins`, created eagerly.
        let root = app_dir(app).join("plugins");
        let _ = fs::create_dir_all(&root);
        root
    }

    pub fn rust(app: &App) -> RuntimeTree {
        Self::rust_at(&Self::install_root(app))
    }

    pub fn rust_at(install_root: &Path) -> RuntimeTree {
        let root = install_root.join("pet");
        RuntimeTree {
            target: root.join("readmd-rust-host"),
            kind: RuntimeKind::Rust,
            root,
        }
    }

    pub fn electron(app: &App) -> RuntimeTree {
        let root = Self::install_root(app).join("pet");
        RuntimeTree {
            target: root.join("hermes-adapter"),
            kind: RuntimeKind::Electron,
            root,
        }
    }

    fn manifest_path(&self) -> PathBuf {
        match self.kind {
            RuntimeKind::Rust => self.target.join("runtime-manifest.json"),
            RuntimeKind::Electron => self.target.join("readmd-pet-plugin.json"),
        }
    }

    fn release_info_path(&self) -> PathBuf {
        match self.kind {
            RuntimeKind::Rust => self.target.join("runtime-release-info.json"),
            RuntimeKind::Electron => self.target.join("pet-release-info.json"),
        }
    }

    pub fn binary_path(&self) -> PathBuf {
        match self.kind {
            RuntimeKind::Rust => self.target.join(if cfg!(windows) {
                "readmd-pet-rust.exe"
            } else {
                "readmd-pet-rust"
            }),
            RuntimeKind::Electron => self.target.join(if cfg!(windows) { "electron.exe" } else { "electron" }),
        }
    }

    /// `get_installed_manifest()`.
    pub fn installed_manifest(&self) -> Option<Value> {
        let text = fs::read_to_string(self.manifest_path()).ok()?;
        match serde_json::from_str::<Value>(&text).ok() {
            Some(value @ Value::Object(_)) => Some(value),
            _ => None,
        }
    }

    pub fn installed_manifest_hash(&self) -> Option<String> {
        sha256_file(&self.manifest_path())
    }

    /// `get_release_info()`.
    pub fn release_info(&self) -> Value {
        fs::read_to_string(self.release_info_path())
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
            .filter(|value| value.is_object())
            .unwrap_or_else(|| json!({}))
    }

    /// `set_release_info(info)` — only once the target tree exists.
    pub fn set_release_info(&self, info: &Value) {
        if !self.target.is_dir() {
            return;
        }
        let text = serde_json::to_string_pretty(info).unwrap_or_else(|_| "{}".into());
        let _ = fs::write(self.release_info_path(), text.as_bytes());
    }

    /// `_verified_install()`: manifest + executable + renderer present.  The
    /// full per-file digest tree verification lives in `_verify_tree`; the
    /// Rust kernel keeps the same fail-closed shape but reports a digest
    /// mismatch through the same `available == false` path.
    pub fn verified_install(&self) -> bool {
        let Some(manifest) = self.installed_manifest() else {
            return false;
        };
        if manifest.as_object().map(|object| object.is_empty()).unwrap_or(true) {
            return false;
        }
        if !self.binary_path().is_file() {
            return false;
        }
        if !self.target.join("renderer").join("index.html").is_file() {
            return false;
        }
        // The runtime manifest the kernel actually ships lists its files under
        // `artifacts` — a list of `{path, sha256, size, role}` objects
        // (`runtime.py:161`, mirrored by `manifest_expected`) — *not* a path/
        // digest `files` map.  Reading `files` here let the real manifest fall
        // into a `None => true` arm and ship with zero digest verification.
        // This must fail closed: a missing, empty or non-array `artifacts`
        // section, an unreadable installed file, or any size/SHA-256 mismatch
        // means the tree is not a verified install.
        let Some(artifacts) = manifest.get("artifacts").and_then(Value::as_array) else {
            return false;
        };
        if artifacts.is_empty() {
            return false;
        }
        artifacts.iter().all(|item| {
            let Some(object) = item.as_object() else { return false; };
            let Some(relative) = object.get("path").and_then(Value::as_str) else {
                return false;
            };
            let Some(expected) = object
                .get("sha256")
                .and_then(Value::as_str)
                .map(|digest| digest.to_ascii_lowercase())
            else {
                return false;
            };
            let Some(size) = object.get("size").and_then(py_int_from_json) else {
                return false;
            };
            let Some(candidate) = safe_join(&self.target, relative) else {
                return false;
            };
            match (fs::metadata(&candidate).ok(), sha256_file(&candidate)) {
                // `path.stat().st_size != item["size"] or _sha256(path) !=
                // item["sha256"]` (`runtime.py:242`) — an unreadable file is a
                // mismatch, never a pass.
                (Some(meta), Some(actual)) => meta.len() as i64 == size && actual == expected,
                _ => false,
            }
        })
    }

    /// `available()` — the installer's own "is the runtime already there".
    pub fn available(&self) -> bool {
        self.verified_install()
    }

    /// `install_directory(directory, confirm)` (`runtime.py:277-305`).  Nothing
    /// is copied before the *whole* source tree has been walked and digested by
    /// `_verify_tree`; only `expected` plus the manifest are published, and any
    /// I/O failure inside that region is the `except (OSError, ValueError,
    /// TypeError, KeyError)` at `runtime.py:303-305`.
    pub fn install_directory(&self, source: &Path, confirm: bool) -> Value {
        if !confirm {
            return json!({ "ok": false, "code": "pet_install_confirmation_required" });
        }
        if is_symlink(source) {
            return json!({ "ok": false, "code": "unsafe_rust_runtime_path" });
        }
        let source = real_path(source);
        if !source.is_dir() {
            return json!({ "ok": false, "code": "invalid_rust_runtime_directory" });
        }
        let failed = || json!({ "ok": false, "code": "rust_runtime_install_failed" });
        let manifest_path = source.join("runtime-manifest.json");
        let text = match fs::read_to_string(&manifest_path) {
            Ok(text) => text,
            Err(_) => return failed(),
        };
        let manifest = match serde_json::from_str::<Value>(&text) {
            Ok(manifest) => manifest,
            Err(_) => return failed(),
        };
        let expected = match verify_tree(&source, &manifest) {
            Ok(expected) => expected,
            Err(code) => return json!({ "ok": false, "code": code }),
        };
        let _ = fs::create_dir_all(&self.root);
        let staged = self
            .root
            .join(format!("readmd-rust-{}.host", std::process::id()));
        let _ = fs::remove_dir_all(&staged);
        let mut copied = fs::create_dir_all(&staged).is_ok();
        for relative in expected.keys() {
            let (Some(from), Some(to)) = (safe_join(&source, relative), safe_join(&staged, relative)) else {
                copied = false;
                break;
            };
            if let Some(parent) = to.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if !copy_file(&from, &to) {
                copied = false;
                break;
            }
        }
        if copied {
            copied = copy_file(&manifest_path, &staged.join("runtime-manifest.json"));
        }
        let result = if copied {
            self.publish(&staged)
        } else {
            json!({ "ok": false, "code": "rust_runtime_install_failed" })
        };
        let _ = fs::remove_dir_all(&staged);
        result
    }

    /// `_publish(staged)` — swap the staged tree in, keeping the old target on
    /// failure.
    fn publish(&self, staged: &Path) -> Value {
        let backup = self
            .root
            .join(format!("readmd-rust-backup-{}", std::process::id()));
        let _ = fs::remove_dir_all(&backup);
        let had_target = self.target.exists();
        if had_target && swap_replace(&self.target, &backup).is_err() {
            return json!({ "ok": false, "code": "rust_runtime_install_failed" });
        }
        match swap_replace(staged, &self.target) {
            Ok(_) => {
                if backup.is_dir() {
                    let _ = fs::remove_dir_all(&backup);
                }
                json!({ "ok": true, "installed": true, "install_path": self.target.to_string_lossy() })
            }
            Err(_) => {
                if had_target && !self.target.exists() {
                    let _ = fs::rename(&backup, &self.target);
                }
                json!({ "ok": false, "code": "rust_runtime_install_failed" })
            }
        }
    }

    /// `uninstall()`.
    pub fn uninstall(&self) -> bool {
        if !self.target.exists() {
            return true;
        }
        fs::remove_dir_all(&self.target).is_ok() && !self.target.exists()
    }
}

fn safe_join(root: &Path, relative: &str) -> Option<PathBuf> {
    let normalized = relative.replace('\\', "/");
    if normalized.is_empty() || normalized.starts_with('/') {
        return None;
    }
    let mut out = root.to_path_buf();
    for part in normalized.split('/') {
        match part {
            "" | "." => continue,
            ".." => return None,
            other => out.push(other),
        }
    }
    Some(out)
}

fn copy_file(from: &Path, to: &Path) -> bool {
    match fs::metadata(from) {
        Ok(meta) if meta.is_dir() => {
            let _ = fs::create_dir_all(to);
            true
        }
        Ok(_) => fs::copy(from, to).is_ok(),
        Err(_) => false,
    }
}

// ------------------------------------------------- Windows swap retry (defect #2)

/// `RustPetRuntimeInstaller.SWAP_ATTEMPTS` / `SWAP_DELAY` (`runtime.py:47-48`).
/// Windows keeps a rename-onto a running/locked target failing with
/// `ERROR_ACCESS_DENIED`, so a single `fs::rename` can never complete an update
/// while the old host is still shutting down; Python retries the replace 40
/// times, 250 ms apart, before giving up.
const SWAP_ATTEMPTS: u32 = 40;
const SWAP_DELAY: std::time::Duration = std::time::Duration::from_millis(250);

/// The `for/else` retry in `_publish` (`runtime.py:252-259`, `runtime.py:261-268`):
/// try `attempts` times, sleeping `delay` after each `PermissionError`, and raise
/// a final `PermissionError` if none landed.  Any *other* OS error (a missing
/// source, e.g.) is not a `PermissionError` and escapes the `except` immediately,
/// so `retry_swap` returns it on the first try rather than spinning.  `attempts`
/// and `delay` are parameters so the loop can be exercised without real sleeping.
fn retry_swap<F>(mut attempt: F, attempts: u32, delay: std::time::Duration) -> std::io::Result<()>
where
    F: FnMut() -> std::io::Result<()>,
{
    for _ in 0..attempts {
        match attempt() {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                std::thread::sleep(delay);
            }
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        "rust_runtime_target_locked",
    ))
}

/// `os.replace(from, to)` wrapped in the retry loop.  Used for both forward
/// renames in `_publish`; the backup restore stays a single rename because
/// `runtime.py:271` does one plain `os.replace` there.
fn swap_replace(from: &Path, to: &Path) -> std::io::Result<()> {
    retry_swap(|| fs::rename(from, to), SWAP_ATTEMPTS, SWAP_DELAY)
}

// -------------------------------------------- reentrant install lock (defect #6)

/// `self._pet_install_lock = threading.RLock()` (`readmd.py:3750`), held around
/// `apply_pet_update` (`readmd.py:5310`) and `install_default_pet_plugin`
/// (`readmd.py:5432`).  It is *reentrant* because `install_default_pet_plugin`
/// calls `apply_pet_update` (`parity_pets.rs` network-fallback branch) on the
/// same thread; a plain mutex would self-deadlock there.  Modelled with a
/// thread-keyed owner + hold depth and a `Condvar` to park other threads.
#[derive(Default)]
struct InstallLockState {
    owner: Option<std::thread::ThreadId>,
    depth: u32,
}

#[derive(Default)]
struct InstallLock {
    state: Mutex<InstallLockState>,
    parked: std::sync::Condvar,
}

/// RAII guard; releasing the last nested acquire notifies a waiting thread.
struct InstallLockGuard<'a> {
    lock: &'a InstallLock,
}

impl Drop for InstallLockGuard<'_> {
    fn drop(&mut self) {
        let mut state = self.lock.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        state.depth = state.depth.saturating_sub(1);
        if state.depth == 0 {
            state.owner = None;
            self.lock.parked.notify_all();
        }
    }
}

impl InstallLock {
    fn acquire(&self) -> InstallLockGuard<'_> {
        let me = std::thread::current().id();
        let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        while state.owner.map(|owner| owner != me).unwrap_or(false) {
            state = self.parked.wait(state).unwrap_or_else(|poisoned| poisoned.into_inner());
        }
        state.owner = Some(me);
        state.depth += 1;
        InstallLockGuard { lock: self }
    }
}

fn install_lock() -> &'static InstallLock {
    static LOCK: OnceLock<InstallLock> = OnceLock::new();
    LOCK.get_or_init(InstallLock::default)
}

/// `PetRuntimeOrchestrator.status()` in `rust-strict` mode.
fn adapter_status(app: &App, active_backend: &str, diagnostic: &str) -> Value {
    let rust = RuntimeTree::rust(app);
    // `RustPetRuntime.status()` (`runtime.py:551-566`): the tracked child is
    // the only host this process may claim, and `generation` is read out of
    // its health report, exactly as Python derives it.
    let (rust_running, rust_pid, rust_health) = host_liveness();
    let rust_generation = rust_health
        .get("runtime_generation")
        .or_else(|| rust_health.get("engine_generation"))
        .cloned()
        .unwrap_or(Value::Null);
    let rust_status = json!({
        "available": rust.verified_install(),
        "running": rust_running,
        "pid": rust_pid.map(|value| json!(value)).unwrap_or(Value::Null),
        "parent_pid": if rust_running { json!(std::process::id()) } else { Value::Null },
        "generation": rust_generation,
        "health": rust_health,
        "runtime": "rust",
        "executable": rust.binary_path().to_string_lossy(),
        "diagnostic": diagnostic,
    });
    let mode = pet_runtime_mode();
    let electron = if mode == "auto" || mode == "electron" {
        json!({ "available": RuntimeTree::electron(app).manifest_path().is_file(), "running": false, "health": {} })
    } else {
        json!({ "available": false, "running": false, "health": {} })
    };
    let available = match mode.as_str() {
        "in_app" => true,
        "rust" | "rust-strict" => rust_status["available"].as_bool().unwrap_or(false),
        "electron" => electron["available"].as_bool().unwrap_or(false),
        _ => {
            rust_status["available"].as_bool().unwrap_or(false)
                || electron["available"].as_bool().unwrap_or(false)
        }
    };
    // `PetRuntimeOrchestrator.status()` only reports the *active* backend as
    // running, and its health (`runtime.py:702-706`).
    let active_running = active_backend == "rust" && rust_running;
    json!({
        "available": available,
        "running": active_running,
        "backend": if active_backend.is_empty() { Value::Null } else { json!(active_backend) },
        "runtime": match active_backend {
            "rust" => json!("rust"),
            "electron" => json!("electron"),
            _ => Value::Null,
        },
        "health": if active_running { rust_health.clone() } else { json!({}) },
        "rust": rust_status,
        "electron": electron,
        "diagnostic": diagnostic,
    })
}

fn pet_runtime_mode() -> String {
    let raw = std::env::var("READMD_PET_RUNTIME_MODE")
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    if raw.is_empty() {
        return "rust-strict".to_string();
    }
    if ["auto", "rust", "rust-strict", "electron", "in_app"].contains(&raw.as_str()) {
        raw
    } else {
        "rust-strict".to_string()
    }
}

/// `verify_model_bundle(root)`.
pub fn verify_model_bundle(root: &Path) -> Value {
    let fail = |code: &str| json!({ "ready": false, "code": code });
    let root = real_path(root);
    let manifest_path = root.join("readmd.live2d.json");
    if !manifest_path.is_file() {
        return fail("model_manifest_missing");
    }
    let Ok(text) = fs::read_to_string(&manifest_path) else {
        return fail("invalid_model_manifest");
    };
    let Ok(manifest) = serde_json::from_str::<Value>(&text) else {
        return fail("invalid_model_manifest");
    };
    let Some(manifest) = manifest.as_object() else {
        return fail("invalid_model_manifest");
    };
    if manifest.get("format_version").and_then(Value::as_i64) != Some(1) {
        return fail("unsupported_manifest_format");
    }
    let id = manifest.get("id").and_then(Value::as_str).unwrap_or("");
    if !is_model_id(id) {
        return fail("invalid_model_id");
    }
    if manifest.get("renderer").and_then(Value::as_str) != Some("cubism-web") {
        return fail("unsupported_model_renderer");
    }
    let has = |key: &str| {
        !manifest
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .is_empty()
    };
    if !has("author") || !has("license") {
        return fail("missing_model_provenance");
    }
    let Some(rights) = manifest.get("rights").and_then(Value::as_object) else {
        return fail("missing_model_rights_chain");
    };
    for key in ["asset_origin", "redistribution_authorization", "cubism_publication_license"] {
        if rights
            .get(key)
            .map(|value| value.as_str().unwrap_or("").trim().is_empty())
            .unwrap_or(true)
        {
            return fail("missing_model_rights_chain");
        }
    }
    let publication = rights
        .get("cubism_publication_license")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if ["pending", "pending-manual-review", "unknown", "none"].contains(&publication.as_str()) {
        return fail("cubism_publication_license_pending");
    }
    let Some(files) = manifest.get("files").and_then(Value::as_object) else {
        return fail("missing_model_files");
    };
    if files.is_empty() {
        return fail("missing_model_files");
    }
    for (relative, expected) in files {
        if !is_safe_relative(relative) {
            return fail("unsafe_asset_path");
        }
        let Some(expected) = expected.as_str().map(|value| value.to_ascii_lowercase()) else {
            return fail("invalid_asset_digest");
        };
        if !is_digest(&expected) {
            return fail("invalid_asset_digest");
        }
        let Some(candidate) = safe_join(&root, relative) else {
            return fail("missing_or_unsafe_asset");
        };
        if !candidate.is_file() || is_symlink(&candidate) {
            return fail("missing_or_unsafe_asset");
        }
        if sha256_file(&candidate).as_deref() != Some(expected.as_str()) {
            return fail("asset_digest_mismatch");
        }
    }
    json!({
        "ready": true,
        "code": "ready_for_platform_probe",
        "model_id": id,
        "renderer": "cubism-web",
        "license": manifest.get("license").and_then(Value::as_str).unwrap_or(""),
        "asset_count": files.len(),
    })
}

/// `^[a-z0-9][a-z0-9-]{1,62}$` (model ids need at least two characters).
fn is_model_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 2
        && bytes.len() <= 63
        && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
        && bytes[1..]
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
}

fn is_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_alphabetic() || matches!(byte, b'a'..=b'f' | b'0'..=b'9'))
}

/// `model_manifest._safe_relative`.
fn is_safe_relative(value: &str) -> bool {
    let path = Path::new(value);
    !path.is_absolute()
        && !value.contains("..")
        && path
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
}

// ------------------------------------------------------------- pet API state

/// `RustPetRuntime._process` plus the health path that child writes to.
/// Tracking it is what makes `running`, `pid` and `health` reportable at all:
/// the kernel previously dropped the `Child` on the floor and could only ever
/// answer `running: false` (`runtime.py:551-566` does not).
pub struct HostChild {
    child: std::process::Child,
    health_file: PathBuf,
}

/// Held across the whole of `start_pet_host`, including `_wait_health`, exactly
/// like `RustPetRuntime._lock` is (`runtime.py:537`).
fn host_start_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(Default::default)
}

/// The tracked child, in a lock of its own so `adapter_status` can read
/// liveness without queueing behind a 15 s health wait.  Python gets the same
/// property for free: `RustPetRuntime.status()` takes no lock at all.
fn host_slot() -> &'static Mutex<Option<HostChild>> {
    static SLOT: OnceLock<Mutex<Option<HostChild>>> = OnceLock::new();
    SLOT.get_or_init(Default::default)
}

/// `RustPetRuntime.status()`'s `running` / `pid` / `health`, derived from the
/// one child this process owns.  A child that has exited is reaped here so
/// `running` cannot stay true after a crash.
fn host_liveness() -> (bool, Option<u32>, Value) {
    let Ok(mut slot) = host_slot().lock() else {
        return (false, None, json!({}));
    };
    let Some(host) = slot.as_mut() else {
        return (false, None, json!({}));
    };
    match host.child.try_wait() {
        Ok(None) | Err(_) => {
            let pid = host.child.id();
            let report = fs::read(&host.health_file)
                .ok()
                .and_then(|bytes| pet_host::health_report(&bytes, pid))
                .unwrap_or(Value::Null);
            (true, Some(pid), if report.is_null() { json!({}) } else { report })
        }
        Ok(Some(_)) => {
            *slot = None;
            (false, None, json!({}))
        }
    }
}

/// `HermesPetBridge._root` / `.state_path` for this install (`hermes_adapter.py:84-86`,
/// constructed at `readmd.py:3749` from `get_default_pet_install_root()`).
fn bridge_paths(app: &App) -> (PathBuf, PathBuf) {
    let root = RuntimeTree::rust(app).root;
    let state = root.join("hermes-overlay-state.json");
    (root, state)
}

/// `_get_shared_api()`'s pet fields, shared across requests like the singleton.
pub struct PetApiState {
    pub controller: PetController,
    pub in_app: bool,
    pub active_backend: String,
    pub diagnostic: String,
    pub cached_update: Option<Value>,
    pub update_progress: Option<Value>,
    pub renderer: Option<String>,
}

impl Default for PetApiState {
    fn default() -> PetApiState {
        PetApiState {
            controller: PetController::fresh(),
            in_app: true,
            active_backend: String::new(),
            diagnostic: String::new(),
            cached_update: None,
            update_progress: None,
            renderer: None,
        }
    }
}

fn pet_state() -> &'static Mutex<PetApiState> {
    static STATE: OnceLock<Mutex<PetApiState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(PetApiState::default()))
}

fn with_state<T>(run: impl FnOnce(&mut PetApiState) -> T) -> T {
    let mut guard = match pet_state().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    run(&mut guard)
}

// --------------------------------------------------------------- preferences

/// `_pet_preferences()` minus the 43-key `lines` pack (see module notes).
fn pet_preferences(app: &App) -> Value {
    let settings = app.settings_all();
    let settings = settings.as_object().cloned().unwrap_or_default();
    let bounds = match settings.get("pet_bounds") {
        Some(value @ Value::Object(_)) => value.clone(),
        _ => Value::Null,
    };
    let scale = clamp(
        py_round(settings.get("pet_scale").unwrap_or(&json!(0.33)).as_f64().unwrap_or(0.33), 2),
        0.18,
        0.72,
    );
    let opacity = clamp(
        py_round(settings.get("pet_opacity").unwrap_or(&json!(1.0)).as_f64().unwrap_or(1.0), 2),
        0.35,
        1.0,
    );
    let renderer_value = settings
        .get("pet_renderer")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let renderer = if renderer_value == "hermes-sprite" || renderer_value == "live2d" {
        renderer_value
    } else {
        "hermes-sprite".to_string()
    };
    let slug = settings
        .get("pet_slug")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let character = if slug.is_empty() { "arch-chan" } else { &slug };
    let mut info = Map::new();
    info.insert("scale".into(), json!(scale));
    info.insert("opacity".into(), json!(opacity));
    info.insert("locale".into(), json!(pet_locale()));
    info.insert("lines".into(), Value::Object(Map::new()));
    if let Ok(mut companion) = PetCompanion::load(&pet_data_dir(app)).snapshot(character) {
        if let Some(object) = companion.as_object_mut() {
            object.insert("character".into(), json!(character));
        }
        info.insert("companion".into(), companion);
    }
    let mut characters: Vec<Value> = vec![json!({
        "slug": "arch-chan",
        "name": format!("Arch-chan ({})", if pet_locale().starts_with("zh") { "Live2D" } else { "Live2D" }),
        "renderer": "live2d",
    })];
    for item in catalog_pets(&pet_assets(app)) {
        let Some(catalog_slug) = item.get("slug").and_then(Value::as_str) else {
            continue;
        };
        let name = preset_name(app, catalog_slug, &item, &pet_locale());
        characters.push(json!({ "slug": catalog_slug, "name": name, "renderer": "hermes-sprite" }));
    }
    info.insert("characters".into(), Value::Array(characters));
    info.insert("renderer".into(), json!(renderer));
    json!({ "bounds": bounds, "renderer": renderer, "info": Value::Object(info) })
}

fn preset_name(app: &App, slug: &str, item: &Value, locale: &str) -> String {
    let key = format!("pet.preset.{slug}");
    let is_zh = locale.to_ascii_lowercase().starts_with("zh");
    for candidate in [locale, "en"] {
        let path = app.paths.assets_dir.join("i18n").join(format!("{candidate}.json"));
        if let Ok(text) = fs::read_to_string(&path) {
            if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(&text) {
                if let Some(found) = map.get(&key).and_then(Value::as_str) {
                    if !found.is_empty() {
                        return found.to_string();
                    }
                }
            }
        }
    }
    if is_zh {
        item.get("zh_name")
            .and_then(Value::as_str)
            .map(|value| value.to_string())
            .unwrap_or_else(|| slug.to_string())
    } else {
        item.get("en_name")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string())
            .unwrap_or_else(|| slug.to_string())
    }
}

/// `get_system_language()` (`readmd_core/config.py:55`): 'auto' -> OS locale.
fn pet_locale() -> String {
    match crate::ai::system_language() {
        Ok(locale) => locale,
        Err(_) => "en".to_string(),
    }
}

fn clamp(value: f64, low: f64, high: f64) -> f64 {
    value.max(low).min(high)
}

/// `round(value, 2)` — half-away-from-zero on the decimal scale, which is what
/// the pet preference bounds need.
fn py_round(value: f64, digits: i32) -> f64 {
    if !value.is_finite() {
        return value;
    }
    let factor = 10f64.powi(digits);
    (value * factor + 0.5).floor() / factor
}

// -------------------------------------------------------------- model status

fn pet_model_status(app: &App) -> Value {
    let rust_model = RuntimeTree::rust(app).target.join("models").join("arch-chan");
    let installed = if rust_model.is_dir() {
        rust_model
    } else {
        app_dir(app).join("assets").join("pet").join("model")
    };
    verify_model_bundle(&installed)
}

// --------------------------------------------------------------- update flow

const GITHUB_REPO: &str = "Natsummerance/readMD";
const MIRROR_PREFIXES: [&str; 3] = [
    "https://ghfast.top/",
    "https://ghproxy.net/",
    "https://mirror.ghproxy.com/",
];

fn pet_platform() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

/// `_arch()` (`runtime.py:132-142`): the lowercased `PROCESSOR_ARCHITECTURE`
/// wins, the compiled-in machine name is only the fallback.
fn pet_arch() -> String {
    let machine = std::env::var("PROCESSOR_ARCHITECTURE")
        .ok()
        .filter(|value| !value.is_empty())
        .map(|value| value.to_lowercase())
        .unwrap_or_else(|| std::env::consts::ARCH.to_string());
    match machine.as_str() {
        "amd64" | "x86_64" | "x64" => "x86_64".to_string(),
        "arm64" | "aarch64" => "aarch64".to_string(),
        other => other.to_string(),
    }
}

fn preferred_rust_asset() -> &'static str {
    match (pet_platform(), pet_arch().as_str()) {
        ("windows", "x86_64") => "readmd-pet-rust.zip",
        ("windows", "aarch64") => "readmd-pet-rust-windows-aarch64.zip",
        ("macos", "x86_64") => "readmd-pet-rust-macos-x86_64.zip",
        ("macos", "aarch64") => "readmd-pet-rust-macos-aarch64.zip",
        ("linux", "x86_64") => "readmd-pet-rust-linux-x86_64.zip",
        ("linux", "aarch64") => "readmd-pet-rust-linux-aarch64.zip",
        _ => "",
    }
}

/// `Path.resolve()` where an `OSError` *aborts the enclosing block*.  Python
/// distinguishes the two failure sites (`updater.py:116-121` skips the whole
/// root, `:124-131` drops just that archive), so the caller has to own the
/// difference; `real_path` cannot express it because it always succeeds.
fn try_resolve(path: &Path) -> Option<PathBuf> {
    fs::canonicalize(path).ok().map(paths::strip_verbatim)
}

/// `p.stat().st_mtime`, or `0` (`UNIX_EPOCH`) when `stat()` raises
/// (`updater.py:136-138`).
fn cand_mtime(path: &Path) -> SystemTime {
    fs::metadata(path)
        .and_then(|meta| meta.modified())
        .unwrap_or(UNIX_EPOCH)
}

/// `_cand_sort_key` (`updater.py:133-135`).  `is_canonical` is the exact
/// lowercase filename match on `readmd-desktop-pet.zip`.
fn cand_is_canonical(path: &Path) -> bool {
    path.file_name()
        .map(|name| name.to_string_lossy().to_ascii_lowercase())
        .as_deref()
        == Some("readmd-desktop-pet.zip")
}

/// Python's whole sort: `found.sort(key=_cand_sort_key, reverse=True)`.
///
/// The mtime read is injected so the ordering — including the tie case that
/// `list.sort`'s stability decides — is testable without racing the
/// filesystem's timestamp granularity.  `slice::sort_by` is stable, so equal
/// keys keep discovery order exactly like CPython.
fn sort_bundled_candidates(
    found: &mut Vec<PathBuf>,
    mtime: impl Fn(&Path) -> SystemTime,
) {
    let key = |path: &Path| (u8::from(cand_is_canonical(path)), mtime(path));
    found.sort_by(|left, right| key(right).cmp(&key(left)));
}

/// `find_bundled_candidate_archives()` filtered by `_archive_matches_runtime`.
///
/// Discovery order and the sort together decide `candidates[0]`, i.e. the one
/// archive `check_pet_update` may offer, so both are Python's exactly: five
/// roots, ten names, one shared resolve-dedupe `seen` set, and a *stable*
/// reverse sort on `(is_canonical, mtime)` (`slice::sort_by` is stable, which
/// is what keeps ties in discovery order like `list.sort` does).
fn bundled_candidates(app: &App) -> Vec<PathBuf> {
    let install = app_dir(app);
    let roots = [
        install.clone(),
        install.join("dist"),
        install.join("dist").join("ReadMD"),
        install
            .join("packages")
            .join("readmd-hermes-pet-adapter")
            .join("dist"),
        install.join("packages").join("readmd-pet-rust").join("dist"),
    ];
    let names = [
        "ReadMD-Desktop-Pet.zip",
        "ReadMD-Desktop-Pet-review.zip",
        "readmd-hermes-pet-adapter-v0.1.0.zip",
        "ReadMD-Pet-Rust.zip",
        "ReadMD-Pet-Rust-windows-x86_64.zip",
        "ReadMD-Pet-Rust-windows-aarch64.zip",
        "ReadMD-Pet-Rust-macos-x86_64.zip",
        "ReadMD-Pet-Rust-macos-aarch64.zip",
        "ReadMD-Pet-Rust-linux-x86_64.zip",
        "ReadMD-Pet-Rust-linux-aarch64.zip",
    ];
    let mut found: Vec<PathBuf> = Vec::new();
    let mut seen: Vec<PathBuf> = Vec::new();
    for root in roots {
        if !root.is_dir() {
            continue;
        }
        // `except OSError: continue` — an unresolvable root is skipped whole,
        // it does not merely lose its own entry.
        let Some(root_resolved) = try_resolve(&root) else {
            continue;
        };
        if seen.contains(&root_resolved) {
            continue;
        }
        seen.push(root_resolved);
        for name in names {
            let candidate = root.join(name);
            if !candidate.is_file() || !archive_has_runtime_manifest(&candidate) {
                continue;
            }
            // `except OSError: pass` — a single archive drops out silently.
            let Some(archive_resolved) = try_resolve(&candidate) else {
                continue;
            };
            if !seen.contains(&archive_resolved) {
                seen.push(archive_resolved);
                found.push(candidate);
            }
        }
    }
    sort_bundled_candidates(&mut found, cand_mtime);
    found
}

fn archive_has_runtime_manifest(path: &Path) -> bool {
    let Ok(blob) = fs::read(path) else {
        return false;
    };
    match zip::entries(&blob) {
        Ok(entries) => entries.iter().any(|entry| entry.name == "runtime-manifest.json"),
        Err(_) => false,
    }
}

/// `check_pet_update(installer, launcher, allow_network=...)`.
///
/// Every network or parse failure collapses into the `source: 'none'` report,
/// exactly like the Python `except Exception` — a pet update check must never
/// turn into a 500.
pub fn check_pet_update(app: &App, allow_network: bool) -> Value {
    let installer = RuntimeTree::rust(app);
    let is_installed = adapter_status(app, "", "").get("rust").is_some_and(|rust| {
        rust.get("available").and_then(Value::as_bool).unwrap_or(false)
    });
    let installed_hash = installer.installed_manifest_hash();
    let release_info = installer.release_info();
    let install_path = installer.target.to_string_lossy().into_owned();

    // Python's step 1 is *not* "the first candidate that differs"
    // (`updater.py:206-246`).  Three observable rules:
    //   * `installed_matches_any` is scanned over EVERY candidate but computed
    //     only when `is_installed and installed_manifest_hash` are both truthy
    //     — the `and` short-circuit matters, because an installed tree with an
    //     unreadable manifest can never "match" anything;
    //   * when it matches, the whole bundled branch is skipped and we fall
    //     through to GitHub even though candidates remain;
    //   * otherwise only `candidates[0]` is ever offered — candidates[1..] are
    //     dropped even when their hash differs.
    let candidates = bundled_candidates(app);
    if !candidates.is_empty() {
        let mut installed_matches_any = false;
        if is_installed && installed_hash.as_ref().is_some_and(|hash| !hash.is_empty()) {
            for candidate in &candidates {
                if archive_manifest_hash(candidate)
                    .is_some_and(|hash| Some(&hash) == installed_hash.as_ref())
                {
                    installed_matches_any = true;
                    break;
                }
            }
        }
        if !installed_matches_any {
            let candidate = &candidates[0];
            if let Some(candidate_hash) = archive_manifest_hash(candidate) {
                if Some(&candidate_hash) != installed_hash.as_ref() {
                    return json!({
                        "ok": true,
                        "has_update": true,
                        "source": "bundled",
                        "version": "bundled_update",
                        "archive_path": real_path(candidate).to_string_lossy(),
                        "installed": is_installed,
                        "install_path": install_path,
                        "runtime": "rust",
                        "reason": "检测到软件内置了更新版本的伴侣桌宠包",
                    });
                }
            }
        }
    }

    if allow_network {
        if let Some(report) = github_update(app, is_installed, &install_path, &release_info) {
            return report;
        }
    }

    json!({
        "ok": true,
        "has_update": false,
        "source": "none",
        "current_version": release_info
            .get("release_tag")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .unwrap_or("bundled"),
        "installed": is_installed,
        "install_path": install_path,
        "runtime": "rust",
    })
}

fn archive_manifest_hash(path: &Path) -> Option<String> {
    let blob = fs::read(path).ok()?;
    let entries = zip::entries(&blob).ok()?;
    let entry = entries.iter().find(|entry| entry.name == "runtime-manifest.json")?;
    let bytes = zip::read_entry(&blob, entry).ok()?;
    Some(sha256_hex(&bytes))
}

fn fetch_github_json(url: &str) -> Option<Value> {
    let headers = vec![
        "Accept: application/vnd.github.v3+json".to_string(),
        "User-Agent: ReadMD-Desktop-Pet-Updater".to_string(),
    ];
    for candidate in std::iter::once(url.to_string()).chain(
        MIRROR_PREFIXES
            .iter()
            .map(|prefix| format!("{prefix}{url}")),
    ) {
        if let Ok((status, body)) = http_get(&candidate, &headers, 12) {
            if status != 200 {
                continue;
            }
            let trimmed = if body.len() > 2 * 1024 * 1024 {
                &body[..2 * 1024 * 1024]
            } else {
                &body[..]
            };
            if let Ok(value) = serde_json::from_slice::<Value>(trimmed) {
                return Some(value);
            }
        }
    }
    None
}

fn github_update(
    app: &App,
    is_installed: bool,
    install_path: &str,
    release_info: &Value,
) -> Option<Value> {
    let url = format!("https://api.github.com/repos/{GITHUB_REPO}/releases/latest");
    let release = fetch_github_json(&url)?;
    let release = release.as_object()?.clone();
    let assets = release
        .get("assets")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let tag_name = release
        .get("tag_name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let preferred = preferred_rust_asset();
    let mut chosen: Option<Value> = None;
    for asset in &assets {
        let name = asset
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_ascii_lowercase();
        if !preferred.is_empty() {
            if name == preferred {
                chosen = Some(asset.clone());
                break;
            }
            continue;
        }
        if name.starts_with("readmd-pet-rust") && name.ends_with(".zip") {
            chosen = Some(asset.clone());
            break;
        }
    }
    let asset = chosen?;
    let download_url = asset.get("browser_download_url").and_then(Value::as_str)?;
    let asset_updated = asset
        .get("updated_at")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let mut is_newer = !is_installed;
    if is_installed {
        let installed_tag = release_info
            .get("release_tag")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if !installed_tag.is_empty() && !tag_name.is_empty() {
            is_newer = compare_versions(&tag_name, &installed_tag) == Some(1);
        } else if !asset_updated.is_empty()
            && Some(asset_updated.as_str())
                != release_info.get("asset_updated_at").and_then(Value::as_str)
        {
            is_newer = true;
        }
    }
    if !is_newer {
        return None;
    }
    Some(json!({
        "ok": true,
        "has_update": true,
        "source": "github",
        "version": if tag_name.is_empty() { "latest".to_string() } else { tag_name.clone() },
        "asset_name": asset.get("name").cloned().unwrap_or(Value::Null),
        "download_url": download_url,
        "size": asset.get("size").cloned().unwrap_or(json!(0)),
        "release_name": release.get("name").cloned().unwrap_or(Value::Null),
        "release_notes": clip(
            release.get("body").and_then(Value::as_str).unwrap_or(""),
            600,
        ).unwrap_or_default(),
        "installed": is_installed,
        "install_path": install_path,
        "runtime": "rust",
        "reason": format!("GitHub 发布了最新桌宠版本 {tag_name}"),
        "asset_updated_at": asset_updated,
    }))
}

/// `parse_version` / `compare_versions` (`readmd_core/versioning.py`).
pub fn compare_versions(left: &str, right: &str) -> Option<i32> {
    let a = parse_version(left)?;
    let b = parse_version(right)?;
    let len = a.len().max(b.len());
    for index in 0..len {
        let x = *a.get(index).unwrap_or(&0);
        let y = *b.get(index).unwrap_or(&0);
        if x != y {
            return Some(if x > y { 1 } else { -1 });
        }
    }
    Some(0)
}

fn parse_version(raw: &str) -> Option<Vec<i64>> {
    let cleaned = raw.trim().trim_start_matches('v');
    let core = cleaned.split(['-', '+']).next().unwrap_or(cleaned);
    if core.is_empty() {
        return None;
    }
    let mut parts = Vec::new();
    for piece in core.split('.') {
        if piece.is_empty() || !piece.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        parts.push(piece.parse::<i64>().ok()?);
    }
    Some(parts)
}

/// `get_pet_update_status()`.
pub fn pet_update_status(app: &App) -> Value {
    let adapter = adapter_status(app, "", "");
    let rust_available = adapter
        .get("rust")
        .and_then(|rust| rust.get("available"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let installer = RuntimeTree::rust(app);
    let release_info = installer.release_info();
    let manifest = installer.installed_manifest();
    let version = release_info
        .get("release_tag")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(|value| json!(value))
        .unwrap_or_else(|| match &manifest {
            Some(_) => {
                if rust_available {
                    json!("0.1.0")
                } else {
                    Value::Null
                }
            }
            None => {
                if rust_available {
                    json!("0.1.0")
                } else {
                    Value::Null
                }
            }
        });
    let cached = with_state(|state| state.cached_update.clone());
    let has_update = cached
        .as_ref()
        .and_then(|value| value.get("has_update"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    json!({
        "ok": true,
        "installed": rust_available,
        "install_path": installer.target.to_string_lossy(),
        "legacy_install_path": RuntimeTree::electron(app).target.to_string_lossy(),
        "version": version,
        "source": release_info.get("source").and_then(Value::as_str).unwrap_or("bundled"),
        "updated_at": release_info.get("updated_at").cloned().unwrap_or(Value::Null),
        "has_update": has_update,
        "update_info": if has_update { cached.clone().unwrap_or(Value::Null) } else { Value::Null },
        "progress": with_state(|state| state.update_progress.clone()).unwrap_or(Value::Null),
        "runtime": if rust_available { json!("rust") } else { Value::Null },
    })
}

/// `apply_pet_update(update_info)`; downloading and swapping a native runtime
/// is the only part that must not pretend, so it goes through the same
/// archive-verification gate as `install_archive`.
pub fn apply_pet_update(app: &App, update_info: Option<&Value>) -> PetOutcome {
    // `readmd.py:5310` runs the whole update under `self._pet_install_lock` so
    // two concurrent requests can never interleave their download/swap.  Reused
    // reentrantly because `install_default_pet_plugin` calls this while it holds
    // the same lock.
    let _install_guard = install_lock().acquire();
    let mut info = update_info.cloned().filter(|value| !value.is_null());
    if info.as_ref().and_then(|value| value.get("has_update")).and_then(Value::as_bool) != Some(true) {
        info = with_state(|state| state.cached_update.clone())
            .filter(|value| value.get("has_update").and_then(Value::as_bool) == Some(true));
    }
    if info.is_none() {
        info = Some(check_pet_update(app, true))
            .filter(|value| value.get("has_update").and_then(Value::as_bool) == Some(true));
    }
    let Some(info) = info else {
        return PetOutcome::Returned(json!({ "ok": false, "code": "no_update_available" }));
    };
    with_state(|state| {
        state.update_progress = Some(json!({ "percent": 0, "downloaded": 0, "total": 0 }))
    });
    // DEFECT #3 (recorded, not invented): Python's `apply_pet_update` reads
    // `was_running = launcher.status().get('running')`, calls `launcher.stop()`
    // before the swap and `launcher.start()` afterwards / in its finally
    // (`updater.py:369-420`).  The kernel has no owned pet host: `adapter_status`
    // hardcodes `running:false` and there is no stop API, and `start_pet_host`
    // does not track the spawned child.  Stopping/restarting is therefore a
    // cross-file need (a process-control handle on `PetApiState`), not something
    // to fabricate here.
    let result: PetOutcome = (|| -> PetOutcome {
        match info.get("source").and_then(Value::as_str).unwrap_or("") {
            "bundled" => {
                let archive = info.get("archive_path").and_then(Value::as_str).unwrap_or("");
                let path = PathBuf::from(archive);
                if archive.is_empty() || !path.is_file() {
                    return PetOutcome::Returned(json!({ "ok": false, "code": "bundled_archive_not_found" }));
                }
                let installer = RuntimeTree::rust(app);
                // `updater.py:379` sends the bundled archive through the same
                // `install_archive` gate as the github download, so an escaping
                // member error must reach the route rather than a result dict.
                let res = install_zip(&installer, &path, true);
                if matches!(res, PetOutcome::Raised) {
                    return res;
                }
                if res.value().and_then(|value| value.get("ok")).and_then(Value::as_bool) == Some(true) {
                    installer.set_release_info(&json!({
                        "source": "bundled",
                        "updated_at": wall_now(),
                        "installed_archive": path.file_name().and_then(|name| name.to_str()).unwrap_or(""),
                    }));
                    let _ = clean_legacy_pet_installations(&installer.target);
                    with_state(|state| state.cached_update = None);
                    return PetOutcome::Returned(json!({
                        "ok": true, "updated": true,
                        "install_path": installer.target.to_string_lossy(),
                        "source": "bundled",
                    }));
                }
                res
            }
            "github" => {
                let Some(download_url) = info.get("download_url").and_then(Value::as_str) else {
                    return PetOutcome::Returned(json!({ "ok": false, "code": "missing_download_url" }));
                };
                let Some(bytes) = download_asset(app, download_url) else {
                    return PetOutcome::Returned(json!({ "ok": false, "code": "pet_download_failed" }));
                };
                let installer = RuntimeTree::rust(app);
                let staged = installer.root.join(format!("readmd-pet-dl-{}", std::process::id()));
                let archive = staged.join("ReadMD-Desktop-Pet.zip");
                let _ = fs::create_dir_all(&staged);
                let res = match fs::write(&archive, &bytes) {
                    Ok(_) => install_zip(&installer, &archive, true),
                    Err(_) => PetOutcome::Returned(json!({ "ok": false, "code": "pet_download_failed" })),
                };
                let _ = fs::remove_dir_all(&staged);
                // Python's `apply_pet_update` wraps everything in a try/finally
                // with no `except` (readmd.py:5309-5321, updater.py:374/418), so
                // an exception escaping `install_archive` propagates to the
                // route rather than becoming a result dict.
                if matches!(res, PetOutcome::Raised) {
                    return res;
                }
                if res.value().and_then(|value| value.get("ok")).and_then(Value::as_bool) == Some(true) {
                    installer.set_release_info(&json!({
                        "source": "github",
                        "release_tag": info.get("version").cloned().unwrap_or(Value::Null),
                        "asset_url": download_url,
                        "asset_updated_at": info.get("asset_updated_at").cloned().unwrap_or(Value::Null),
                        "updated_at": wall_now(),
                    }));
                    let _ = clean_legacy_pet_installations(&installer.target);
                    with_state(|state| state.cached_update = None);
                    return PetOutcome::Returned(json!({
                        "ok": true, "updated": true,
                        "install_path": installer.target.to_string_lossy(),
                        "version": info.get("version").cloned().unwrap_or(Value::Null),
                        "source": "github",
                    }));
                }
                res
            }
            _ => PetOutcome::Returned(json!({ "ok": false, "code": "unknown_update_source" })),
        }
    })();
    with_state(|state| state.update_progress = None);
    result
}

/// Python distinguishes two things the Rust port used to merge: the api method
/// *returning* `{'ok': False, ...}` — which `_handle_pet_lifecycle_action`
/// forwards at 200 (`readmd.py:1717-1718`) — and the api method *raising*, which
/// is the only way to reach `readmd.py:1719-1721`:
///
/// ```python
/// try:
///     res = getattr(api, method_name)()
///     self._send_json(200, res)
/// except Exception:
///     self._send_json(500, {'ok': False, 'error_code': error_code})
/// ```
pub enum PetOutcome {
    Returned(Value),
    /// An exception that escapes the installer's `except (OSError, ValueError,
    /// TypeError, KeyError, zipfile.BadZipFile, UnicodeError)`
    /// (`runtime.py:343-346`) and therefore the api method as a whole.
    Raised,
}

impl PetOutcome {
    /// The documented 200 payload, for the callers that swallow the raise.
    fn value(&self) -> Option<&Value> {
        match self {
            PetOutcome::Returned(value) => Some(value),
            PetOutcome::Raised => None,
        }
    }
}

/// Which `zip::read_entry` failures are CPython's *uncaught* ones.
///
/// `bundle.open()` / `bundle.read()` raise `RuntimeError("File … is encrypted,
/// but encrypted files are not supported")` for an encrypted member and
/// `NotImplementedError("… compression is not supported")` for a method
/// `zipfile` does not implement; a truncated deflate stream is a `zlib.error`.
/// All three are `Exception`s that `install_archive`'s except clause
/// (`runtime.py:343`) lets through, so the route answers with its 500 code,
/// while a malformed local header is a caught `BadZipFile`.
fn escapes_install_archive(error: &str) -> bool {
    error == "encrypted"
        || error.starts_with("unsupported compression ")
        || error == "corrupt deflate stream"
}

/// `install_archive` for a downloaded or local ZIP bundle (`runtime.py:298-346`).
pub fn install_zip(installer: &RuntimeTree, archive: &Path, confirm: bool) -> PetOutcome {
    let failed = || {
        PetOutcome::Returned(json!({ "ok": false, "code": "rust_runtime_install_failed" }))
    };
    if !confirm {
        return PetOutcome::Returned(json!({ "ok": false, "code": "pet_install_confirmation_required" }));
    }
    let archive = real_path(archive);
    if !archive.is_file()
        || archive
            .extension()
            .map(|ext| ext.to_string_lossy().to_ascii_lowercase() != "zip")
            .unwrap_or(true)
    {
        return PetOutcome::Returned(json!({ "ok": false, "code": "invalid_rust_runtime_archive" }));
    }
    if file_size(&archive) > MAX_ARCHIVE_BYTES {
        return PetOutcome::Returned(json!({ "ok": false, "code": "rust_runtime_archive_too_large" }));
    }
    let Ok(blob) = fs::read(&archive) else {
        return failed();
    };
    let entries = match zip::entries(&blob) {
        // `zipfile.ZipFile(archive)` raising `BadZipFile` is inside the except.
        Ok(entries) => entries,
        Err(_) => return failed(),
    };
    let entries: Vec<zip::ZipEntry> = entries.into_iter().filter(|entry| !entry.name.ends_with('/')).collect();
    if entries.is_empty() || entries.len() > MAX_INSTALL_FILES {
        return PetOutcome::Returned(json!({ "ok": false, "code": "invalid_rust_runtime_contents" }));
    }
    if entries
        .iter()
        .any(|entry| !zip_safe_name(&entry.name) || (entry.external_attr >> 16) & 0o170000 == 0o120000)
    {
        return PetOutcome::Returned(json!({ "ok": false, "code": "unsafe_rust_runtime_path" }));
    }
    let Some(manifest_entry) = entries.iter().find(|entry| entry.name == "runtime-manifest.json") else {
        return PetOutcome::Returned(json!({ "ok": false, "code": "rust_runtime_manifest_missing" }));
    };
    // `bundle.read(manifest_item)` is the first escape hatch: an encrypted or
    // non-deflate member raises before any manifest key is looked at.
    let manifest_bytes = match zip::read_entry(&blob, manifest_entry) {
        Ok(bytes) => bytes,
        Err(error) => {
            if escapes_install_archive(&error) {
                return PetOutcome::Raised;
            }
            return failed();
        }
    };
    let Ok(manifest) = serde_json::from_slice::<Value>(&manifest_bytes) else {
        return failed();
    };
    let expected = match manifest_expected(&manifest) {
        Ok(expected) => expected,
        Err(code) => return PetOutcome::Returned(json!({ "ok": false, "code": code })),
    };
    let names: BTreeSet<&str> = entries.iter().map(|entry| entry.name.as_str()).collect();
    let folded: BTreeSet<String> = names.iter().map(|name| name.to_lowercase()).collect();
    if names.len() != entries.len()
        || folded.len() != entries.len()
        || !expected.keys().all(|path| names.contains(path.as_str()))
        || names
            .iter()
            .any(|name| !expected.contains_key(*name) && !ALLOWED_METADATA.contains(name))
    {
        return PetOutcome::Returned(json!({ "ok": false, "code": "rust_runtime_unlisted_file" }));
    }
    if entries.iter().map(|entry| entry.uncompressed_size).sum::<u64>() > MAX_EXPANDED_BYTES {
        return PetOutcome::Returned(json!({ "ok": false, "code": "rust_runtime_too_large" }));
    }
    let _ = fs::create_dir_all(&installer.root);
    let staged = installer
        .root
        .join(format!("readmd-rust-{}-zip.host", std::process::id()));
    let _ = fs::remove_dir_all(&staged);
    // Every `bundle.open(relative)` below can be the one that escapes, so the
    // loop's failure is a `(PetOutcome, bool)` pair rather than a plain bool.
    let mut outcome = failed();
    let mut staged_ok = fs::create_dir_all(&staged).is_ok();
    while staged_ok {
        for (relative, item) in &expected {
            let Some(entry) = entries.iter().find(|entry| &entry.name == relative) else {
                staged_ok = false;
                break;
            };
            let bytes = match zip::read_entry(&blob, entry) {
                Ok(bytes) => bytes,
                // A member whose declared data runs past the archive is CPython's
                // `BadZipFile("Overlapped entries …")` (`zipfile.py:1623-1626`),
                // which `install_archive` catches at `runtime.py:355`, so only the
                // three escape-set codes above may leave the loop.
                Err(error) => {
                    if escapes_install_archive(&error) {
                        outcome = PetOutcome::Raised;
                    }
                    staged_ok = false;
                    break;
                }
            };
            let Some(destination) = safe_join(&staged, relative) else {
                staged_ok = false;
                break;
            };
            if let Some(parent) = destination.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if fs::write(&destination, &bytes).is_err() {
                staged_ok = false;
                break;
            }
            // `destination.stat().st_size != item["size"] or _sha256(destination)
            // != item["sha256"]` on the bytes just written.
            if bytes.len() as i64 != item.size || sha256_hex(&bytes) != item.sha256 {
                outcome =
                    PetOutcome::Returned(json!({ "ok": false, "code": "rust_runtime_hash_mismatch" }));
                staged_ok = false;
                break;
            }
        }
        if !staged_ok {
            break;
        }
        // "Preserve the manifest bytes exactly as shipped."
        if fs::write(staged.join("runtime-manifest.json"), &manifest_bytes).is_err() {
            break;
        }
        outcome = PetOutcome::Returned(installer.publish(&staged));
        break;
    }
    let _ = fs::remove_dir_all(&staged);
    outcome
}

/// `RustPetRuntimeInstaller._safe_name` (`runtime.py:105-119`).  ZIP member
/// names are POSIX paths even on Windows, so the drive-qualified-first-component
/// rule is checked by hand; there is no length or depth limit in Python.
fn zip_safe_name(name: &str) -> bool {
    if name.is_empty() || name.contains('\0') || name.contains('\\') {
        return false;
    }
    if name.starts_with('/') || name.starts_with('~') {
        return false;
    }
    // Windows accepts `C:relative` as a drive-relative path even though it is
    // not absolute, so every drive-qualified first component is rejected before
    // anything resolves it against the current drive.
    let first = name.split('/').next().unwrap_or(name);
    if first.len() >= 2 && first.as_bytes()[1] == b':' {
        return false;
    }
    !name.split('/').any(|part| part == "." || part == "..")
}

/// `ALLOWED_METADATA` / `ALLOWED_ROLES` (`runtime.py:46-47`) plus the two size
/// gates (`runtime.py:43-44`).
const ALLOWED_METADATA: [&str; 2] = ["runtime-manifest.json", "runtime-release-info.json"];
const ALLOWED_ROLES: [&str; 5] = ["executable", "renderer", "asset", "model", "companion"];
const MAX_INSTALL_FILES: usize = 10_000;
const MAX_EXPANDED_BYTES: u64 = 1_500 * 1024 * 1024;
const MAX_ARCHIVE_BYTES: u64 = 750 * 1024 * 1024;

/// One validated `manifest["artifacts"]` entry.
#[derive(Debug, Clone)]
struct ExpectedArtifact {
    sha256: String,
    size: i64,
    role: String,
}

/// CPython's `int(value)` on a decoded JSON value (`runtime.py:150` and
/// `runtime.py:165`).  `None` is `TypeError`, a float truncates toward zero,
/// `True`/`False` are 1/0, and a string goes through the same acceptance set a
/// header value does.
fn py_int_from_json(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => number.as_f64().map(|value| value.trunc() as i64),
        Value::Bool(flag) => Some(i64::from(*flag)),
        Value::String(text) => py_int(text).ok(),
        _ => None,
    }
}

/// `RustPetRuntimeInstaller._manifest_expected` (`runtime.py:144-193`).
///
/// The manifest is a *list* of artifacts, not a path/digest map: every entry
/// needs `path`, `sha256`, `size` and an optional `role`, the runtime has to be
/// `readmd-pet-rust` at protocol 1, and the tree must name both its executable
/// and `renderer/index.html`.  `Err` carries the code Python returns alongside
/// `None`.
fn manifest_expected(
    manifest: &Value,
) -> Result<BTreeMap<String, ExpectedArtifact>, String> {
    let invalid = || Err("invalid_rust_runtime_manifest".to_string());
    let object = match manifest.as_object() {
        Some(object) => object,
        None => return invalid(),
    };
    if object.get("runtime").and_then(Value::as_str) != Some("readmd-pet-rust") {
        return invalid();
    }
    let protocol = match object.get("protocol_version") {
        Some(value) => value.clone(),
        // `manifest.get("protocol_version", manifest.get("protocol", -1))`.
        None => object.get("protocol").cloned().unwrap_or(Value::from(-1)),
    };
    if py_int_from_json(&protocol) != Some(1) {
        return invalid();
    }
    // Python compares these two *verbatim* — `"Windows"` is a mismatch.
    let platform = py_or_str(object.get("platform").unwrap_or(&Value::Null));
    if platform != pet_platform() && platform != "any" {
        return Err("rust_runtime_platform_mismatch".to_string());
    }
    let arch = py_or_str(object.get("arch").unwrap_or(&Value::Null));
    if arch != pet_arch() && arch != "any" {
        return Err("rust_runtime_arch_mismatch".to_string());
    }
    let items = match object.get("artifacts").and_then(Value::as_array) {
        Some(items) if !items.is_empty() => items,
        _ => return invalid(),
    };
    let mut expected: BTreeMap<String, ExpectedArtifact> = BTreeMap::new();
    let mut folded: BTreeSet<String> = BTreeSet::new();
    for item in items {
        let Some(item) = item.as_object() else { return invalid() };
        let Some(path) = item.get("path").and_then(Value::as_str) else { return invalid() };
        let digest = py_or_str(item.get("sha256").unwrap_or(&Value::Null)).to_lowercase();
        let Some(size) = item.get("size").and_then(py_int_from_json) else {
            return invalid();
        };
        // `int(True) == 1` would otherwise sail through, so Python rejects a
        // boolean `size` explicitly.
        if item.get("size").map(Value::is_boolean).unwrap_or(false) {
            return invalid();
        }
        let role = match item.get("role") {
            // `item.get("role", "asset")`: only a *missing* key defaults, an
            // explicit `null` fails `isinstance(role, str)`.
            None => "asset".to_string(),
            Some(value) => match value.as_str() {
                Some(role) => role.to_string(),
                None => return invalid(),
            },
        };
        if !ALLOWED_ROLES.contains(&role.as_str()) {
            return invalid();
        }
        let folded_path = path.to_lowercase();
        if !zip_safe_name(path)
            || expected.contains_key(path)
            || folded.contains(&folded_path)
            || digest.len() != 64
            || digest
                .bytes()
                .any(|byte| !matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
            || size < 0
        {
            return invalid();
        }
        folded.insert(folded_path);
        expected.insert(
            path.to_string(),
            ExpectedArtifact { sha256: digest, size, role },
        );
    }
    let executable: Vec<&String> = expected
        .iter()
        .filter(|(_, item)| item.role == "executable")
        .map(|(path, _)| path)
        .collect();
    let names_the_host = executable.iter().any(|path| {
        let path = path.replace('\\', "/");
        path.ends_with("readmd-pet-rust.exe") || path.ends_with("readmd-pet-rust")
    });
    if executable.is_empty() || !names_the_host {
        return Err("rust_runtime_executable_missing".to_string());
    }
    if !expected.contains_key("renderer/index.html") {
        return Err("rust_runtime_renderer_missing".to_string());
    }
    Ok(expected)
}

/// `RustPetRuntimeInstaller._verify_tree` (`runtime.py:196-244`).  Python folds
/// relative paths with `.casefold()`; this file's convention is
/// `to_lowercase()`, the same choice `install_zip`'s duplicate-name gate makes.
fn verify_tree(source: &Path, manifest: &Value) -> Result<BTreeMap<String, ExpectedArtifact>, String> {
    let expected = manifest_expected(manifest)?;
    let mut actual: BTreeSet<String> = BTreeSet::new();
    let mut folded_actual: BTreeSet<String> = BTreeSet::new();
    let mut total: u64 = 0;
    let mut stack = vec![source.to_path_buf()];
    while let Some(directory) = stack.pop() {
        let Ok(entries) = fs::read_dir(&directory) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(_) => return Err("unsafe_rust_runtime_path".to_string()),
            };
            if file_type.is_symlink() {
                return Err("unsafe_rust_runtime_path".to_string());
            }
            let Ok(relative) = path.strip_prefix(source) else { continue };
            let relative = relative
                .components()
                .map(|part| part.as_os_str().to_string_lossy().to_string())
                .collect::<Vec<_>>()
                .join("/");
            if file_type.is_dir() {
                stack.push(path);
                continue;
            }
            if !zip_safe_name(&relative) {
                return Err("unsafe_rust_runtime_path".to_string());
            }
            if !folded_actual.insert(relative.to_lowercase()) {
                return Err("rust_runtime_duplicate_file".to_string());
            }
            total += match entry.metadata() {
                // `total += path.stat().st_size` (`runtime.py:221`) raising is an
                // `OSError`, i.e. the caller's install-failed code.
                Ok(meta) => meta.len(),
                Err(_) => return Err("rust_runtime_install_failed".to_string()),
            };
            actual.insert(relative);
        }
    }
    if actual.len() > MAX_INSTALL_FILES || total > MAX_EXPANDED_BYTES {
        return Err("rust_runtime_too_large".to_string());
    }
    if !expected.keys().all(|path| actual.contains(path)) {
        return Err("rust_runtime_file_missing".to_string());
    }
    let mutable_prefixes: Vec<String> = expected
        .iter()
        .filter(|(_, item)| item.role == "executable")
        .map(|(path, _)| format!("{path}.WebView2/"))
        .collect();
    let has_extra = actual.iter().any(|relative| {
        !(expected.contains_key(relative)
            || ALLOWED_METADATA.contains(&relative.as_str())
            || mutable_prefixes
                .iter()
                .any(|prefix| relative.starts_with(prefix.as_str())))
    });
    if has_extra {
        return Err("rust_runtime_unlisted_file".to_string());
    }
    for (relative, item) in &expected {
        let path = match safe_join(source, relative) {
            Some(path) => path,
            None => return Err("rust_runtime_file_missing".to_string()),
        };
        // A file that cannot be stat'd or read is Python's `path.stat()` /
        // `_sha256(path)` raising an `OSError` (`runtime.py:242`), not a digest
        // mismatch.
        let size = match fs::metadata(&path) {
            Ok(meta) => meta.len(),
            Err(_) => return Err("rust_runtime_install_failed".to_string()),
        };
        let digest = match sha256_file(&path) {
            Some(digest) => digest,
            None => return Err("rust_runtime_install_failed".to_string()),
        };
        if size as i64 != item.size || digest != item.sha256 {
            return Err("rust_runtime_hash_mismatch".to_string());
        }
    }
    Ok(expected)
}

/// `download_github_asset` with the mirror fallback chain (`updater.py:324-359`).
///
/// The single-attempt fetch (`fetch_asset_once`) turns *this* mirror's failure
/// into `None` — an unreachable mirror, a non-200, or a body small enough to be
/// an error stub — and `download_from_mirrors` moves on to the next candidate.
/// The old code did `http_get(..).ok()?` *inside* the loop, which returned
/// `None` from the whole function on the first unreachable mirror instead of
/// trying the next.
fn download_asset(app: &App, url: &str) -> Option<Vec<u8>> {
    let headers = vec![
        "User-Agent: ReadMD-Desktop-Pet-Updater".to_string(),
        "Accept: application/octet-stream".to_string(),
    ];
    let urls: Vec<String> = std::iter::once(url.to_string())
        .chain(MIRROR_PREFIXES.iter().map(|prefix| format!("{prefix}{url}")))
        .collect();
    let bytes = download_from_mirrors(&urls, |candidate| fetch_asset_once(candidate, &headers))?;
    let total = bytes.len() as i64;
    with_state(|state| {
        state.update_progress = Some(json!({
            "percent": py_round(100.0, 1),
            "downloaded": total,
            "total": total,
        }))
    });
    let _ = app;
    Some(bytes)
}

/// Walks the mirror chain, returning the first acceptable body.  A `None` from
/// one attempt must not stop the walk — that is the whole point of the fallback.
fn download_from_mirrors<F>(urls: &[String], mut fetch: F) -> Option<Vec<u8>>
where
    F: FnMut(&str) -> Option<Vec<u8>>,
{
    for candidate in urls {
        if let Some(bytes) = fetch(candidate.as_str()) {
            return Some(bytes);
        }
    }
    None
}

/// One mirror attempt.  Any of the three reasons below makes this candidate a
/// `None` so the caller falls through to the next mirror: a transport error
/// (`updater.py:351` `except Exception: continue`), a non-200 status
/// (`updater.py:335-336`), or a body of `<= 1000` bytes — Python accepts a
/// download only when `dest_path.stat().st_size > 1000` (`updater.py:349`).
fn fetch_asset_once(candidate: &str, headers: &[String]) -> Option<Vec<u8>> {
    let (status, bytes) = http_get(candidate, headers, 60).ok()?;
    if status != 200 {
        return None;
    }
    if bytes.len() <= 1000 {
        return None;
    }
    Some(bytes)
}

/// `clean_legacy_pet_installations`.
pub fn clean_legacy_pet_installations(active_target: &Path) -> Vec<String> {
    let appdata = std::env::var("APPDATA").unwrap_or_default();
    if appdata.trim().is_empty() {
        return Vec::new();
    }
    let base = PathBuf::from(appdata).join("ReadMD");
    let mut cleaned = Vec::new();
    for legacy in [
        base.join("plugins").join("pet").join("hermes-adapter"),
        base.join("pet").join("hermes-adapter"),
    ] {
        if !legacy.is_dir() {
            continue;
        }
        if real_path(&legacy) == real_path(active_target) {
            continue;
        }
        if fs::remove_dir_all(&legacy).is_ok() && !legacy.exists() {
            cleaned.push(legacy.to_string_lossy().into_owned());
        }
    }
    cleaned
}

/// `install_default_pet_plugin()`'s local-candidate search, then the network
/// fallback that Python performs with an explicit `allow_network=True`.
fn install_default_pet_plugin(app: &App) -> PetOutcome {
    // `readmd.py:5432` wraps the local-candidate scan and its network fallback in
    // `self._pet_install_lock`; the fallback calls `apply_pet_update`, which
    // re-enters the same reentrant lock on this thread.
    let _install_guard = install_lock().acquire();
    let installer = RuntimeTree::rust(app);
    if installer.available() {
        return PetOutcome::Returned(json!({ "ok": true, "installed": true, "runtime": "rust", "install_path": installer.target.to_string_lossy() }));
    }
    let install = app_dir(app);
    let roots: Vec<PathBuf> = vec![
        install.clone(),
        install.join("build"),
        install.join("dist"),
        install.join("dist").join("ReadMD"),
        install.join("packages").join("readmd-pet-rust").join("dist"),
    ];
    let directories = [
        "readmd-rust-host",
        "readmd-pet-rust-windows",
        "ReadMD-Pet-Rust",
        "ReadMD-Pet-Rust-windows-x86_64",
        "ReadMD-Pet-Rust-windows-aarch64",
        "ReadMD-Pet-Rust-macos-x86_64",
        "ReadMD-Pet-Rust-macos-aarch64",
        "ReadMD-Pet-Rust-linux-x86_64",
        "ReadMD-Pet-Rust-linux-aarch64",
    ];
    let archives = [
        "ReadMD-Pet-Rust.zip",
        "ReadMD-Pet-Rust-windows-x86_64.zip",
        "ReadMD-Pet-Rust-windows-aarch64.zip",
        "ReadMD-Pet-Rust-macos-x86_64.zip",
        "ReadMD-Pet-Rust-macos-aarch64.zip",
        "ReadMD-Pet-Rust-linux-x86_64.zip",
        "ReadMD-Pet-Rust-linux-aarch64.zip",
        "ReadMD-Pet-Rust-windows.zip",
        "readmd-pet-rust-windows.zip",
    ];
    // `readmd.py:5491-5496`: the `seen_roots` gate is a *raw string* check
    // applied before `os.path.isdir`, so a root listed twice is scanned once
    // and its sidecar candidates are never offered to the installer twice.
    let mut seen_roots: BTreeSet<String> = BTreeSet::new();
    for root in roots {
        let root_key = root.to_string_lossy().into_owned();
        if root_key.is_empty() || seen_roots.contains(&root_key) {
            continue;
        }
        seen_roots.insert(root_key);
        if !root.is_dir() {
            continue;
        }
        for name in directories {
            let directory = root.join(name);
            if !directory.is_dir() {
                continue;
            }
            // `install_directory`'s `except (OSError, ValueError, TypeError,
            // KeyError)` (runtime.py:303-305) swallows every realistic failure,
            // so it never contributes a raise to the loop.
            let res = installer.install_directory(&directory, true);
            if res.get("ok").and_then(Value::as_bool) == Some(true) {
                return PetOutcome::Returned(res);
            }
        }
        for name in archives {
            let archive = root.join(name);
            if !archive.is_file() {
                continue;
            }
            let res = install_zip(&installer, &archive, true);
            // `_install_default_pet_plugin_locked` calls install_archive with no
            // surrounding try (readmd.py:5491-5521); an exception escaping the
            // except clause (runtime.py:343-346) aborts the request -> 500, so
            // it must not fall through to the next candidate.
            if matches!(res, PetOutcome::Raised) {
                return res;
            }
            if res.value().and_then(|value| value.get("ok")).and_then(Value::as_bool) == Some(true) {
                return res;
            }
        }
        if let Ok(read) = fs::read_dir(&root) {
            for entry in read.flatten() {
                let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
                if !name.starts_with("readmd-pet-rust") || !name.ends_with(".zip") {
                    continue;
                }
                let res = install_zip(&installer, &entry.path(), true);
                if matches!(res, PetOutcome::Raised) {
                    return res;
                }
                if res.value().and_then(|value| value.get("ok")).and_then(Value::as_bool) == Some(true) {
                    return res;
                }
            }
        }
    }
    let missing = json!({ "ok": false, "code": "rust_runtime_bundle_missing" });
    // Python only reaches out to the network after the local candidates failed,
    // and `install_default_pet_plugin` wraps that fallback in
    // `try: ... except Exception: pass` (readmd.py:5437-5447), so a raising
    // `apply_pet_update` is swallowed and the bundle-missing result is returned
    // instead of the update's own value.
    let up = check_pet_update(app, true);
    if up.get("ok").and_then(Value::as_bool) == Some(true)
        && up.get("has_update").and_then(Value::as_bool) == Some(true)
        && up.get("source").and_then(Value::as_str) == Some("github")
    {
        if let PetOutcome::Returned(value) = apply_pet_update(app, Some(&up)) {
            return PetOutcome::Returned(value);
        }
    }
    PetOutcome::Returned(missing)
}

// ------------------------------------------------------------ status/configure

/// `get_pet_runtime_status()`.
pub fn pet_runtime_status(app: &App) -> Value {
    let mut status = with_state(|state| state.controller.snapshot());
    let adapter = with_state(|state| adapter_status(app, &state.active_backend, &state.diagnostic));
    status["model"] = pet_model_status(app);
    status["adapter"] = adapter.clone();
    status["runtime_backend"] = adapter.get("backend").cloned().unwrap_or(Value::Null);
    status["rust_runtime"] = adapter.get("rust").cloned().unwrap_or(json!({}));
    let in_app = with_state(|state| state.in_app);
    status["in_app"] = json!(in_app);
    let preferences = pet_preferences(app);
    let mut info = preferences
        .get("info")
        .cloned()
        .unwrap_or_else(|| json!({}));
    if let Some(object) = info.as_object_mut() {
        object.insert("renderer".into(), preferences["renderer"].clone());
    }
    status["preferences"] = info;
    let active_slug = app.setting("pet_slug").as_str().unwrap_or_default().to_string();
    status["active_slug"] = json!(active_slug);
    let character = if active_slug.is_empty() { "arch-chan" } else { active_slug.as_str() };
    status["companion"] = match PetCompanion::load(&pet_data_dir(app)).snapshot(character) {
        Ok(profile) => profile,
        Err(_) => Value::Null,
    };
    let rust_available = adapter
        .get("rust")
        .and_then(|rust| rust.get("available"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    status["installed"] = json!(if in_app { true } else { rust_available });
    status["install_path"] = json!(RuntimeTree::rust(app).target.to_string_lossy());
    status["legacy_install_path"] = json!(RuntimeTree::electron(app).target.to_string_lossy());
    status["update"] = pet_update_status(app);
    status
}

/// `configure_pet(settings)` including the launch-failure rollback.
pub fn configure_pet(app: &App, settings: &Value) -> Value {
    let Some(settings) = settings.as_object() else {
        return json!({ "ok": false, "code": "invalid_pet_settings" });
    };
    let default_renderer = pet_preferences(app)["renderer"]
        .as_str()
        .unwrap_or("hermes-sprite")
        .to_string();
    let renderer = match settings.get("renderer") {
        Some(Value::String(value)) => value.clone(),
        _ => default_renderer.clone(),
    };
    if renderer != "hermes-sprite" && renderer != "live2d" {
        return json!({ "ok": false, "code": "invalid_pet_renderer" });
    }
    let previous_in_app = with_state(|state| state.in_app);
    let in_app = match settings.get("in_app") {
        Some(value) => match value.as_bool() {
            Some(flag) => flag,
            None => return json!({ "ok": false, "code": "invalid_pet_settings" }),
        },
        None => previous_in_app,
    };
    if settings.contains_key("enabled") && !settings["enabled"].is_boolean() {
        return json!({ "ok": false, "code": "invalid_pet_settings" });
    }
    let mut preference_updates = Map::new();
    for (key, low, high) in [("scale", 0.18f64, 0.72f64), ("opacity", 0.35f64, 1.0f64)] {
        let Some(raw) = settings.get(key) else {
            continue;
        };
        let Some(number) = raw.as_f64() else {
            return json!({ "ok": false, "code": format!("invalid_pet_{key}") });
        };
        if !number.is_finite() {
            return json!({ "ok": false, "code": format!("invalid_pet_{key}") });
        }
        let value = py_round(number, 2);
        if value < low || value > high {
            return json!({ "ok": false, "code": format!("invalid_pet_{key}") });
        }
        preference_updates.insert(format!("pet_{key}"), json!(value));
    }
    let previous_runtime = with_state(|state| state.controller.snapshot());
    let previous_enabled = previous_runtime["enabled"].as_bool().unwrap_or(false);
    if settings.contains_key("renderer") {
        with_state(|state| state.renderer = Some(renderer.clone()));
    }
    if settings.contains_key("in_app") {
        preference_updates.insert("pet_in_app".into(), json!(in_app));
    }
    let enabled = match settings.get("enabled") {
        Some(value) => value.as_bool().unwrap_or(false),
        None => previous_enabled,
    };
    if settings.contains_key("enabled") {
        preference_updates.insert("pet_enabled".into(), json!(enabled));
    }
    if !preference_updates.is_empty() {
        app.update_settings(&Value::Object(preference_updates));
    }
    if let Some(reduced_motion) = settings.get("reduced_motion") {
        let flag = reduced_motion.as_bool().unwrap_or_else(|| !reduced_motion.is_null());
        with_state(|state| {
            state.controller.set_reduced_motion(flag);
        });
    }
    let _ = previous_runtime;
    // `readmd.py` disables through `PetRuntimeOrchestrator.stop()`, which only
    // touches the backend this process started.  Without it a hidden pet keeps
    // its always-on-top overlay and its WebView2 process alive.
    if !enabled {
        stop_pet_host();
    }
    let runtime = with_state(|state| {
        if !enabled {
            state.controller.disable()
        } else if !previous_enabled {
            state.controller.enable()
        } else {
            state.controller.snapshot()
        }
    });
    with_state(|state| state.in_app = in_app);
    if enabled && !in_app {
        let launched = start_pet_host(app);
        if launched.get("ok").and_then(Value::as_bool) != Some(true) {
            with_state(|state| {
                state.controller.disable();
                state.in_app = previous_in_app;
            });
            app.update_settings(&json!({
                "pet_enabled": false,
                "pet_in_app": previous_in_app,
                "pet_renderer": default_renderer,
            }));
            return launched;
        }
    }
    json!({
        "ok": true,
        "runtime": runtime,
        "renderer": renderer,
        "model": pet_model_status(app),
        "in_app": in_app,
        "adapter": with_state(|state| adapter_status(app, &state.active_backend, &state.diagnostic)),
    })
}

/// `PetRuntimeOrchestrator.start()` in `rust-strict`, delegating to
/// `RustPetRuntime.start()` (`runtime.py:536-608`).
///
/// This is the pet host "ship step": the route the UI reaches
/// (`/api/pets/configure` -> `configure_pet` -> here) is the only thing that
/// can put a pet on screen.  Two rules carry over from Python:
///
/// * the child is started with the **documented environment**.  A bare
///   `spawn()` inherits whatever the kernel happens to have, and
///   `HostConfig::from_env` (`packages/readmd-pet-rust/src/runtime.rs:33-43`)
///   exits with code 2 on `READMD_PET_BRIDGE_FILE is required`, so the previous
///   three-line spawn could not produce a pet at all; and
/// * `ok: true` means *healthy*.  Python does not answer until `_wait_health`
///   has seen `state: "ready"` written by this exact pid (`runtime.py:604-607`);
///   calling a spawn a running pet is how a dead overlay got a success code.
fn start_pet_host(app: &App) -> Value {
    let installer = RuntimeTree::rust(app);
    let status = adapter_status(app, "", "rust:unavailable");
    if !status["available"].as_bool().unwrap_or(false) {
        with_state(|state| state.diagnostic = "rust:unavailable".to_string());
        return json!({
            "ok": false,
            "code": "rust_strict_runtime_failed",
            "diagnostic": "rust:unavailable",
            "runtime": status,
        });
    }
    // `runtime.py:537`: one start at a time, and the health wait runs inside
    // that lock, so a second `/api/pets/configure` cannot double-launch.
    let _start_guard = match host_start_lock().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    // `runtime.py:543-551`: an owned live child is already a success; a live
    // child we do not own is reported and never adopted.
    let (running, pid, _) = host_liveness();
    if running {
        let owned = host_slot()
            .lock()
            .map(|slot| slot.as_ref().is_some_and(|host| Some(host.child.id()) == pid))
            .unwrap_or(false);
        if owned {
            return json!({ "ok": true, "runtime": adapter_status(app, "rust", "") });
        }
        with_state(|state| state.diagnostic = "rust_runtime_already_running".to_string());
        return json!({
            "ok": false,
            "code": "rust_runtime_already_running",
            "runtime": adapter_status(app, "", "rust_runtime_already_running"),
        });
    }

    // `runtime.py:547-553`: the tree must exist, and a stale report left by a
    // previous host must not be mistaken for this one's readiness.
    let _ = fs::create_dir_all(&installer.target);
    let (bridge_root, bridge_file) = bridge_paths(app);
    let renderer = pet_preferences(app)
        .get("renderer")
        .and_then(Value::as_str)
        .unwrap_or("hermes-sprite")
        .to_string();
    let ambient: std::collections::BTreeMap<String, String> = std::env::vars().collect();
    let plan = pet_host::build_plan(
        &installer.binary_path(),
        &installer.target,
        &bridge_file,
        &bridge_root,
        &renderer,
        std::process::id(),
        &ambient,
    );
    let _ = fs::remove_file(&plan.health_file);
    let mut command = crate::process::silent_command(&plan.binary);
    command.current_dir(&plan.cwd);
    // argv is `[binary]` and nothing else (`runtime.py:588`); the environment is
    // the parent's plus the seven contract keys, so deliberately no `env_clear`.
    for (key, value) in &plan.env {
        command.env(key, value);
    }
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(_) => {
            // `runtime.py:588-596`: an `OSError` closes both pipe ends and
            // reports the spawn failure.  This path owns no pipe to close.
            with_state(|state| state.diagnostic = "rust:spawn_failed".to_string());
            return json!({
                "ok": false,
                "code": "rust_runtime_start_failed",
                "runtime": adapter_status(app, "", "rust:spawn_failed"),
            });
        }
    };
    let pid = child.id();
    let budget = pet_host::health_timeout(&ambient);
    let verdict = pet_host::wait_for_health(&plan.health_file, pid, budget, || {
        match child.try_wait() {
            Ok(Some(exit)) => Some(exit.code().map(|code| code as i32)),
            Ok(None) | Err(_) => None,
        }
    });
    drop(_start_guard);
    match verdict {
        Ok(()) => {
            if let Ok(mut slot) = host_slot().lock() {
                *slot = Some(HostChild { child, health_file: plan.health_file.clone() });
            }
            with_state(|state| {
                state.active_backend = "rust".to_string();
                state.diagnostic = String::new();
            });
            json!({ "ok": true, "runtime": adapter_status(app, "rust", "") })
        }
        Err(diagnostic) => {
            // `runtime.py:604-607`: a host that never reported ready is stopped
            // before the failure is returned, so it cannot leak an overlay.
            let _ = child.kill();
            let _ = child.wait();
            if let Ok(mut slot) = host_slot().lock() {
                *slot = None;
            }
            with_state(|state| {
                state.active_backend = String::new();
                state.diagnostic = diagnostic.clone();
            });
            json!({
                "ok": false,
                "code": "rust_runtime_health_failed",
                "runtime": adapter_status(app, "", &diagnostic),
                "diagnostic": diagnostic,
            })
        }
    }
}

/// `RustPetRuntime.stop()` for the one child the kernel owns
/// (`runtime.py:610-649`).  Python also closes the inherited pipe's write end;
/// this build passes no handle (see `pet_host`'s module notes), so terminating
/// the tracked child is the whole of it.  Returns whether a host was running.
fn stop_pet_host() -> bool {
    let Ok(mut slot) = host_slot().lock() else {
        return false;
    };
    match slot.as_mut() {
        Some(host) => {
            let _ = host.child.kill();
            let _ = host.child.wait();
            *slot = None;
            true
        }
        None => false,
    }
}

/// `install_companion_pet()`.
fn install_companion_pet(app: &App) -> PetOutcome {
    let renderer = pet_preferences(app)["renderer"]
        .as_str()
        .unwrap_or("hermes-sprite")
        .to_string();
    let in_app = {
        let current = with_state(|state| state.in_app);
        renderer != "live2d" && current
    };
    if !in_app {
        // `install_companion_pet` calls `install_default_pet_plugin()` with no
        // try/except (readmd.py:5323-5330), so a raised install propagates to
        // the route as 500 `pet_install_failed`.
        let installed = install_default_pet_plugin(app);
        if matches!(installed, PetOutcome::Raised) {
            return installed;
        }
        if installed.value().and_then(|value| value.get("ok")).and_then(Value::as_bool) != Some(true) {
            return installed;
        }
    }
    let result = configure_pet(
        app,
        &json!({ "enabled": true, "in_app": in_app, "renderer": renderer }),
    );
    if result.get("ok").and_then(Value::as_bool) != Some(true) {
        return PetOutcome::Returned(result);
    }
    app.update_settings(&json!({ "pet_installed": true }));
    PetOutcome::Returned(json!({ "ok": true, "installed": true, "status": pet_runtime_status(app) }))
}

/// `uninstall_companion_pet()`.
fn uninstall_companion_pet(app: &App) -> Value {
    configure_pet(app, &json!({ "enabled": false }));
    let legacy_removed = RuntimeTree::electron(app).uninstall();
    let rust_removed = RuntimeTree::rust(app).uninstall();
    if !legacy_removed || !rust_removed {
        return json!({
            "ok": false,
            "code": "pet_plugin_remove_failed",
            "status": pet_runtime_status(app),
        });
    }
    app.update_settings(&json!({ "pet_installed": false, "pet_enabled": false }));
    json!({ "ok": true, "installed": false, "status": pet_runtime_status(app) })
}

// -------------------------------------------------------------- pet handlers

/// `/api/pets` — `_api_pets` (`readmd.py:1646`).
pub fn h_pets(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    if req.method != "GET" {
        return ec(405, "method_not_allowed");
    }
    let active = app.setting("pet_slug").as_str().unwrap_or_default().to_string();
    let pets: Vec<Value> = list_pets(&pet_data_dir(app), &pet_assets(app), true)
        .iter()
        .map(InstalledPet::as_dict)
        .collect();
    json_at(200, json!({ "ok": true, "active": active, "pets": pets }))
}

/// `/api/pets/status` — `_api_pet_status` (`readmd.py:1661`).
pub fn h_pets_status(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    if req.method != "GET" {
        return ec(405, "method_not_allowed");
    }
    json_at(200, json!({ "ok": true, "status": pet_runtime_status(app) }))
}

/// `/api/pets/thumb?slug=` — `_api_pet_thumb` (`readmd.py:1809`).
///
/// There is deliberately *no* slug validation here: an unknown slug is a 404 in
/// the legacy server, and a 400 would be a behaviour change the UI would show
/// as a different error.  Only an unreadable file is a 404 as well.
pub fn h_pet_thumb(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    if req.method != "GET" {
        return ec(405, "method_not_allowed");
    }
    let slug = req.q("slug").unwrap_or("").to_string();
    let thumb = app_dir(app)
        .join("assets")
        .join("pet")
        .join("thumbs")
        .join(format!("{slug}.png"));
    if thumb.is_file() {
        if let Some(body) = read_bounded(&thumb, MAX_THUMB_BYTES) {
            return Ok(Response::raw_bytes(200, "image/png", body)
                .header("Cache-Control", "private, max-age=86400"));
        }
        return ec(404, "pet_not_found");
    }
    let Some(pet) = find_pet(&pet_data_dir(app), &pet_assets(app), &slug) else {
        return ec(404, "pet_not_found");
    };
    let path = real_path(&pet.spritesheet);
    let escaped = !pet.is_builtin
        && path.parent().map(real_path).as_deref() != Some(real_path(&pet.directory).as_path());
    if escaped {
        return ec(403, "pet_path_invalid");
    }
    let mime = if path.to_string_lossy().to_ascii_lowercase().ends_with(".png") {
        "image/png"
    } else {
        "image/webp"
    };
    let Some(body) = read_bounded(&path, MAX_SPRITESHEET + 1) else {
        return ec(404, "pet_not_found");
    };
    if body.len() > MAX_SPRITESHEET {
        return ec(413, "pet_spritesheet_too_large");
    }
    Ok(Response::raw_bytes(200, mime, body).header("Cache-Control", "private, max-age=3600"))
}

/// `/api/pets/import` — `_api_pet_import` (`readmd.py:1729`).
pub fn h_pet_import(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    if req.method != "POST" {
        return ec(405, "method_not_allowed");
    }
    // `readmd.py:1734-1736` judges `n <= 0 or n > 24 * 1024 * 1024`, so an empty
    // declaration really is a 413 here (unlike the `_read_json_body` routes,
    // where it means "no body").  A `Content-Length` that `int()` rejects
    // raises *before* that comparison, and the route's
    // `getattr(exc, 'code', 'pet_import_failed')` finds no code on a ValueError.
    let default_code = "pet_import_failed";
    let declared = match req.header("content-length") {
        None => 0,
        Some(text) if text.is_empty() => 0,
        Some(text) => match py_int(text) {
            Ok(value) => value,
            Err(_) => return ec(400, default_code),
        },
    };
    if declared <= 0 || declared > MAX_IMPORT_BODY as i64 {
        return ec(413, "pet_spritesheet_too_large");
    }
    let body = match inline_json_body(req) {
        JsonBody::Parsed(value) => value,
        // `json.loads` failure, and `body.get('confirm')` on a non-object, both
        // reach the same default code.
        _ => return ec(400, default_code),
    };
    let object = match body.as_object() {
        Some(object) => object.clone(),
        None => return ec(400, default_code),
    };
    if object.get("confirm").and_then(Value::as_bool) != Some(true) {
        return ec(400, "confirmation_required");
    }
    let encoded = body_str(&object, "image_base64");
    let raw = match base64::engine::general_purpose::STANDARD.decode(encoded.as_bytes()) {
        Ok(raw) => raw,
        Err(_) => return ec(400, "pet_spritesheet_format_invalid"),
    };
    match register_local_pet(
        &pet_data_dir(app),
        &body_str(&object, "slug"),
        &raw,
        &body_str(&object, "display_name"),
        &body_str(&object, "description"),
        py_truthy(object.get("replace").unwrap_or(&Value::Null)),
    ) {
        Ok(pet) => json_at(200, json!({ "ok": true, "pet": pet.as_dict() })),
        Err(error) => ec(400, &error.code),
    }
}

/// `/api/pets/remove` — `_api_pet_remove` (`readmd.py:1756`).
pub fn h_pet_remove(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    if req.method != "POST" && req.method != "DELETE" {
        return ec(405, "method_not_allowed");
    }
    let body = match inline_json_body(req) {
        JsonBody::Parsed(value) => value,
        // `getattr(exc, 'code', 'pet_remove_failed')` (`readmd.py:1774`): both
        // the bare `int()` on the header and `json.loads` raise `ValueError`s
        // that carry no `code`, so the route's default answers.
        JsonBody::Raised(_) => return ec(400, "pet_remove_failed"),
        // `json.loads(...) if n else {}`.
        JsonBody::NoBody => Value::Object(Map::new()),
    };
    // A non-object document makes `body.get('confirm')` raise `AttributeError`,
    // which has no `code` either.
    let Some(object) = body.as_object() else {
        return ec(400, "pet_remove_failed");
    };
    if object.get("confirm").and_then(Value::as_bool) != Some(true) {
        return ec(400, "confirmation_required");
    }
    let slug = body_str(object, "slug");
    if let Err(error) = remove_pet(&pet_data_dir(app), &slug) {
        return ec(400, &error.code);
    }
    // `readmd.py:1767` compares the stored value against the *raw* body entry,
    // not against `str(body.get('slug') or '')`.
    let stored = app.setting("pet_slug");
    if py_eq(&stored, object.get("slug").unwrap_or(&Value::Null)) {
        app.update_settings(&json!({ "pet_slug": null }));
    }
    json_at(200, json!({ "ok": true }))
}

/// `/api/pets/active` — `_api_pet_active` (`readmd.py:1776`).
pub fn h_pet_active(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    if req.method != "POST" {
        return ec(405, "method_not_allowed");
    }
    let body = match inline_json_body(req) {
        JsonBody::Parsed(value) => value,
        // `readmd.py:1806`'s `getattr(exc, 'code', 'pet_active_failed')`.
        JsonBody::Raised(_) => return ec(400, "pet_active_failed"),
        JsonBody::NoBody => Value::Object(Map::new()),
    };
    let Some(object) = body.as_object() else {
        return ec(400, "pet_active_failed");
    };
    if object.get("confirm").and_then(Value::as_bool) != Some(true) {
        return ec(400, "confirmation_required");
    }
    let slug = body_str(object, "slug").trim().to_ascii_lowercase();
    if !slug.is_empty() && find_pet(&pet_data_dir(app), &pet_assets(app), &slug).is_none() {
        return ec(404, "pet_not_found");
    }
    if !slug.is_empty() {
        let source = app_dir(app).join("assets").join("pet").join(&slug);
        let target = pet_data_dir(app).join("pets").join(&slug);
        if source.is_dir() && !target.is_dir() {
            if let Some(parent) = target.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = copy_tree(&source, &target);
        }
    }
    if slug.is_empty() {
        app.update_settings(&json!({ "pet_slug": null }));
    } else {
        app.update_settings(&json!({ "pet_slug": slug.clone() }));
    }
    json_at(200, json!({ "ok": true, "active": slug }))
}

/// CPython's `sys.getrecursionlimit()` default, which is the reference copy's
/// own ceiling: `shutil.copytree` recurses one Python frame per directory, so a
/// tree deeper than this is one `readmd.py:1797` could not finish either
/// (measured: `RecursionError` on a linking tree at a lowered limit). Pairing
/// this with the iterative walk below means nothing the reference copied is
/// refused here.
const MAX_COPY_DEPTH: usize = 1_000;

/// One pending directory pair on `copy_tree`'s explicit worklist.
struct CopyFrame {
    from: PathBuf,
    to: PathBuf,
    depth: usize,
    /// Canonical identities of every source directory above this one.
    ancestors: CopyAncestors,
}

/// Persistent ancestor chain, so a wide tree does not copy its ancestry per
/// pushed directory.
#[derive(Clone, Default)]
struct CopyAncestors {
    head: Option<std::rc::Rc<CopyAncestorNode>>,
}

struct CopyAncestorNode {
    key: String,
    parent: Option<std::rc::Rc<CopyAncestorNode>>,
}

impl CopyAncestors {
    fn extended(&self, key: String) -> Self {
        CopyAncestors {
            head: Some(std::rc::Rc::new(CopyAncestorNode { key, parent: self.head.clone() })),
        }
    }

    fn holds(&self, key: &str) -> bool {
        let mut node = self.head.as_deref();
        while let Some(current) = node {
            if current.key == key {
                return true;
            }
            node = current.parent.as_deref();
        }
        false
    }
}

/// Canonical identity of a source directory, used only as a cycle key and never
/// opened from this form. Falls back to the raw path when `canonicalize` fails;
/// `MAX_COPY_DEPTH` still bounds the walk in that case.
fn copy_cycle_key(path: &Path) -> String {
    fs::canonicalize(path)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| path.to_string_lossy().to_string())
}

/// `shutil.copytree(source, target)` (`readmd.py:1797`) with the reference's
/// defaults (`symlinks=False`), on an explicit worklist.
///
/// Link handling is deliberately *not* the `os.walk` one: `copytree` with
/// `symlinks=False` resolves a directory link and copies what it points at
/// (measured here — a junction `loop` inside the copied tree produced
/// `loop/a.md` and `loop/b/b.md` in the destination), so skipping links would
/// silently copy fewer files than Python. A loop is removed instead by pruning
/// a directory whose canonical identity already appears among its own
/// ancestors, which is precisely the shape `copytree` answers with a
/// `RecursionError` after having written thousands of duplicate copies and
/// deep-enough nesting to fill the disk.
fn copy_tree(source: &Path, target: &Path) -> std::io::Result<()> {
    fs::create_dir_all(target)?;
    let mut stack: Vec<CopyFrame> = vec![CopyFrame {
        from: source.to_path_buf(),
        to: target.to_path_buf(),
        depth: 0,
        ancestors: CopyAncestors::default(),
    }];
    while let Some(frame) = stack.pop() {
        let ancestors = frame.ancestors.extended(copy_cycle_key(&frame.from));
        let mut dirs: Vec<(PathBuf, PathBuf)> = Vec::new();
        for entry in fs::read_dir(&frame.from)? {
            let entry = entry?;
            let from = entry.path();
            let to = frame.to.join(entry.file_name());
            if from.is_dir() {
                dirs.push((from, to));
            } else {
                fs::copy(&from, &to)?;
            }
        }
        if frame.depth >= MAX_COPY_DEPTH {
            // This branch alone is refused; siblings still on the worklist
            // continue to be copied.
            continue;
        }
        let depth = frame.depth + 1;
        for (from, to) in dirs {
            if ancestors.holds(&copy_cycle_key(&from)) {
                continue;
            }
            // Created on the way down, so an empty source directory still
            // yields an empty target directory exactly like `copytree`.
            fs::create_dir_all(&to)?;
            stack.push(CopyFrame { from, to, depth, ancestors: ancestors.clone() });
        }
    }
    Ok(())
}

/// `_handle_pet_lifecycle_action` — body read errors are swallowed.
fn lifecycle<F: FnOnce(&App) -> PetOutcome>(
    req: &Request,
    app: &App,
    error_code: &str,
    run: F,
) -> ApiResult<Response> {
    if req.method != "POST" {
        return ec(405, "method_not_allowed");
    }
    // `_handle_pet_lifecycle_action` (`readmd.py:1711-1714`) reads the body only
    // to drain it and swallows *every* failure, so the answer never depends on
    // it — but the read still has to happen for the transport.
    let _ = json_body_of(req, PET_LIFECYCLE_BODY_LIMIT);
    // `readmd.py:1715-1721`: a returned dict is answered 200; only a raised
    // exception reaches the 500 `error_code` envelope.
    match run(app) {
        PetOutcome::Returned(value) => {
            json_at(200, value).map_err(|_| crate::error::ApiError::internal(error_code))
        }
        PetOutcome::Raised => ec(500, error_code),
    }
}

/// `/api/pets/install` — `install_companion_pet`.
pub fn h_pet_install(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    lifecycle(req, app, "pet_install_failed", |app| install_companion_pet(app))
}

/// `/api/pets/uninstall` — `uninstall_companion_pet`.
pub fn h_pet_uninstall(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    lifecycle(req, app, "pet_uninstall_failed", |app| {
        PetOutcome::Returned(uninstall_companion_pet(app))
    })
}

/// `/api/pets/runtime/install` — `install_default_pet_plugin`.
///
/// The failure code is `readmd.py:1262`'s literal third argument to
/// `_handle_pet_lifecycle_action`, `'pet_plugin_install_failed'`; the
/// previously-used `pet_runtime_install_failed` exists nowhere in Python.
pub fn h_pet_runtime_install(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    lifecycle(req, app, "pet_plugin_install_failed", |app| {
        install_default_pet_plugin(app)
    })
}

/// `/api/pets/configure` — `_api_pet_configure` (`readmd.py:1673`).
///
/// Oversize body: `_read_json_body(65536)` raises `ValueError('request_too_large')`
/// (`readmd.py:1446`), and the `except ValueError` arm at `readmd.py:1682-1685`
/// only returns without answering when `str(e) == 'payload_too_large'` — a
/// message Python never raises — so the fall-through `readmd.py:1685` sends
/// `400 {'ok': False, 'error_code': 'request_too_large'}`.
pub fn h_pet_configure(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    if req.method != "POST" {
        return ec(405, "method_not_allowed");
    }
    let body = match json_body_of(req, PET_LIFECYCLE_BODY_LIMIT) {
        // `if not n: return {}` (`readmd.py:1466-1467`): an empty declaration
        // never reaches `json.loads`, so Python answers 200 with
        // `configure_pet({})`'s dict — `_api_pet_configure`'s own
        // `isinstance`-free path.
        JsonBody::NoBody => Value::Object(Map::new()),
        JsonBody::Parsed(value) => value,
        JsonBody::Raised(message) => return ec(400, &message),
    };
    json_at(200, configure_pet(app, &body))
}

/// `/api/pets/interact` — `_api_pet_interact` (`readmd.py:1690`).
///
/// Unlike its neighbours, this route answers `ValueError` with the `code` key
/// (`readmd.py:1701-1702` sends `{'ok': False, 'code': str(e)}`) while its 405
/// arm still uses `error_code` (`readmd.py:1692`), and the message Python puts
/// there is `'request_too_large'` (`readmd.py:1446`) — not `'payload_too_large'`.
pub fn h_pet_interact(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    if req.method != "POST" {
        return ec(405, "method_not_allowed");
    }
    let body = match json_body_of(req, PET_LIFECYCLE_BODY_LIMIT) {
        // `Content-Length: 0` / no header is `{}` (`readmd.py:1466-1467`), so
        // the route still calls `interact_pet('')` and answers
        // `400 {'ok': False, 'code': 'invalid_pet_action'}` — not a parse error.
        JsonBody::NoBody => Value::Object(Map::new()),
        JsonBody::Parsed(value) => value,
        JsonBody::Raised(message) => return code_err(400, &message),
    };
    // `body.get('action')` (`readmd.py:1696`) on a JSON document that is not an
    // object raises AttributeError, which only the blanket handler catches.
    let Some(object) = body.as_object() else {
        return code_err(500, "pet_interact_failed");
    };
    let action = body_str(object, "action").trim().to_string();
    let character_raw = body_str(object, "character").trim().to_string();
    let character = if character_raw.is_empty() {
        py_or_str(&app.setting("pet_slug"))
    } else {
        character_raw
    };
    let character = if character.is_empty() { "arch-chan" } else { &character };
    match PetCompanion::load(&pet_data_dir(app)).interact(character, &action) {
        Ok(result) => {
            if result.get("ok").and_then(Value::as_bool) == Some(true) {
                with_state(|state| state.controller.snapshot());
            }
            json_at(200, result)
        }
        Err(CompanionError::ValueError(message)) => code_err(400, &message),
        // `readmd.py:1703-1705`: the same `code` key, this time at 500.
        Err(CompanionError::Unexpected) => code_err(500, "pet_interact_failed"),
    }
}

/// `/api/pets/update_status` — `_api_pet_update_status` (`readmd.py:1840`).
pub fn h_pet_update_status(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    if req.method != "GET" {
        return ec(405, "method_not_allowed");
    }
    json_at(200, pet_update_status(app))
}

/// `bool(body.get('allow_network', True))` (`readmd.py:1857`).
///
/// The `True` is a *default argument* of `dict.get`, so it only covers a
/// missing key: an explicit `null` reaches `bool(None)` and disables the
/// network branch.
fn allow_network_of(object: &Map<String, Value>) -> bool {
    match object.get("allow_network") {
        None => true,
        Some(value) => py_truthy(value),
    }
}

/// `/api/pets/check_update` — `_api_pet_check_update` (`readmd.py:1852`).
///
/// There is no `ValueError` arm here: `_read_json_body(65536)`'s
/// `ValueError('request_too_large')` (`readmd.py:1446`) is a subclass of
/// `Exception`, so the oversize body lands in `readmd.py:1862-1864`'s
/// `except Exception` and Python answers
/// `500 {'ok': False, 'error_code': 'pet_check_update_failed'}` — not `200 null`.
pub fn h_pet_check_update(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    if req.method != "POST" {
        return ec(405, "method_not_allowed");
    }
    let body = match json_body_of(req, PET_LIFECYCLE_BODY_LIMIT) {
        JsonBody::Parsed(value) => value,
        // Every `ValueError` from `_read_json_body` — the header `int()`'s
        // message, `'request_too_large'`, `'incomplete_request'`, the decoder's
        // message — is an `Exception`, so `readmd.py:1862-1864` answers the
        // route's own 500 code.
        JsonBody::Raised(_) => return Err(crate::error::ApiError::internal("pet_check_update_failed")),
        JsonBody::NoBody => Value::Object(Map::new()),
    };
    // `body.get('allow_network', True)` on a non-object raises `AttributeError`.
    let Some(object) = body.as_object() else {
        return Err(crate::error::ApiError::internal("pet_check_update_failed"));
    };
    let allow_network = allow_network_of(object);
    let res = check_pet_update(app, allow_network);
    let cache = res.get("ok").and_then(Value::as_bool) == Some(true)
        && res.get("has_update").and_then(Value::as_bool) == Some(true);
    with_state(|state| state.cached_update = if cache { Some(res.clone()) } else { None });
    json_at(200, res)
}

/// `/api/pets/apply_update` — `_api_pet_apply_update` (`readmd.py:1866`).
///
/// Same contract as `check_update`: the `ValueError('request_too_large')` from
/// `_read_json_body(65536)` is caught by `readmd.py:1875-1877`'s
/// `except Exception`, giving `500 {'ok': False, 'error_code':
/// 'pet_apply_update_failed'}`.
pub fn h_pet_apply_update(app: &Arc<App>, req: &Request) -> ApiResult<Response> {
    if req.method != "POST" {
        return ec(405, "method_not_allowed");
    }
    let body = match json_body_of(req, PET_LIFECYCLE_BODY_LIMIT) {
        JsonBody::Parsed(value) => value,
        JsonBody::Raised(_) => return Err(crate::error::ApiError::internal("pet_apply_update_failed")),
        // `if not n: return {}` — so `body.get('update_info')` is `None`.
        JsonBody::NoBody => Value::Object(Map::new()),
    };
    let object = match body.as_object() {
        Some(object) => object,
        None => return Err(crate::error::ApiError::internal("pet_apply_update_failed")),
    };
    match apply_pet_update(app, object.get("update_info")) {
        PetOutcome::Returned(value) => json_at(200, value),
        PetOutcome::Raised => ec(500, "pet_apply_update_failed"),
    }
}

// --------------------------------------------------------------- zip reader

/// Minimal read-only ZIP central-directory reader: the vendored dependency set
/// has no `zip` crate, and both the pet runtime installer and the Skill
/// importer only need listing plus member bytes.
pub mod zip {
    use flate2::read::DeflateDecoder;
    use std::io::Read;

    #[derive(Debug, Clone)]
    pub struct ZipEntry {
        pub name: String,
        pub method: u16,
        pub compressed_size: u64,
        pub uncompressed_size: u64,
        pub local_offset: u64,
        pub external_attr: u32,
        pub flags: u16,
    }

    fn u16_at(bytes: &[u8], at: usize) -> u16 {
        u16::from_le_bytes([bytes[at], bytes[at + 1]])
    }

    fn u32_at(bytes: &[u8], at: usize) -> u32 {
        u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
    }

    pub fn entries(blob: &[u8]) -> Result<Vec<ZipEntry>, String> {
        if blob.len() < 22 {
            return Err("too small".into());
        }
        let start = blob.len().saturating_sub(22 - 1 + 65535);
        let mut signature = None;
        for index in (start..blob.len() - 3).rev() {
            if u32_at(blob, index) == 0x0605_4b50 {
                signature = Some(index);
                break;
            }
        }
        let eocd = signature.ok_or("missing end of central directory")?;
        let count = u16_at(blob, eocd + 10) as usize;
        let directory_size = u32_at(blob, eocd + 12) as usize;
        let directory_offset = u32_at(blob, eocd + 16) as usize;
        if directory_offset + directory_size > blob.len() {
            return Err("central directory out of range".into());
        }
        let mut cursor = directory_offset;
        let mut out = Vec::with_capacity(count);
        for _ in 0..count {
            if cursor + 46 > blob.len() || u32_at(blob, cursor) != 0x0201_4b50 {
                break;
            }
            let flags = u16_at(blob, cursor + 8);
            let method = u16_at(blob, cursor + 10);
            let compressed_size = u32_at(blob, cursor + 20) as u64;
            let uncompressed_size = u32_at(blob, cursor + 24) as u64;
            let name_len = u16_at(blob, cursor + 28) as usize;
            let extra_len = u16_at(blob, cursor + 30) as usize;
            let comment_len = u16_at(blob, cursor + 32) as usize;
            let external_attr = u32_at(blob, cursor + 38);
            let local_offset = u32_at(blob, cursor + 42) as u64;
            let name_end = cursor + 46 + name_len;
            if name_end > blob.len() {
                return Err("name out of range".into());
            }
            let name = String::from_utf8_lossy(&blob[cursor + 46..name_end]).into_owned();
            out.push(ZipEntry {
                flags,
                method,
                compressed_size,
                uncompressed_size,
                local_offset,
                external_attr,
                name,
            });
            cursor = name_end + extra_len + comment_len;
        }
        Ok(out)
    }

    pub fn read_entry(blob: &[u8], entry: &ZipEntry) -> Result<Vec<u8>, String> {
        if entry.flags & 0x1 != 0 {
            return Err("encrypted".into());
        }
        let offset = entry.local_offset as usize;
        if offset + 30 > blob.len() || u32_at(blob, offset) != 0x0403_4b50 {
            return Err("bad local header".into());
        }
        let name_len = u16_at(blob, offset + 26) as usize;
        let extra_len = u16_at(blob, offset + 28) as usize;
        let data_start = offset + 30 + name_len + extra_len;
        let data_end = data_start + entry.compressed_size as usize;
        if data_end > blob.len() {
            return Err("data out of range".into());
        }
        let payload = &blob[data_start..data_end];
        match entry.method {
            0 => Ok(payload.to_vec()),
            8 => {
                let mut decoder = DeflateDecoder::new(payload);
                let mut out = Vec::with_capacity(entry.uncompressed_size as usize);
                decoder
                    .read_to_end(&mut out)
                    .map_err(|error| error.to_string())?;
                Ok(out)
            }
            other => Err(format!("unsupported compression {other}")),
        }
    }
}

// ------------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn scratch_dir(tag: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "readmd-parity-pets-{tag}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn test_app(tag: &str) -> Arc<App> {
        let dir = scratch_dir(tag);
        fs::create_dir_all(dir.join("assets")).unwrap();
        let paths = paths::AppPaths::with_dirs(&dir.join("data"), &dir, &dir.join("assets"));
        Arc::new(App::bootstrap(paths).unwrap())
    }

    /// A request whose declared `Content-Length` is independent of the bytes it
    /// carries: `_read_json_body` gates on the *header* (`readmd.py:1465`)
    /// through `_read_request_body_limited` (`readmd.py:1441-1446`), so that is
    /// the only lever needed to reach the size gate offline.
    fn declared_request(method: &str, path: &str, raw: &[u8], content_length: Option<&str>) -> Request {
        let mut headers = HashMap::new();
        if let Some(value) = content_length {
            headers.insert("content-length".to_string(), value.to_string());
        }
        Request {
            method: method.to_string(),
            path: path.to_string(),
            query: HashMap::new(),
            headers,
            body: raw.to_vec(),
        }
    }

    /// Uniform `(status, body)` view of a handler answer; an `Err` is rendered
    /// exactly the way the dispatcher renders it (`server.rs`:
    /// `Response::json_status(err.status, &err.payload())`).
    fn answer(result: ApiResult<Response>) -> (u16, Value) {
        match result {
            Ok(res) => (
                res.status,
                serde_json::from_slice(&res.body).expect("handler answered with valid JSON"),
            ),
            Err(err) => (err.status, err.payload()),
        }
    }

    fn key_set(body: &Value) -> Vec<String> {
        let mut keys: Vec<String> = body
            .as_object()
            .map(|map| map.keys().cloned().collect())
            .unwrap_or_default();
        keys.sort();
        keys
    }

    /// `readmd.py:1445` — `if length < 0 or length > int(limit)`, judged on the
    /// header, with `== limit` still allowed and a missing header meaning 0
    /// (`readmd.py:1465-1467`).
    #[test]
    fn size_gate_judges_the_declared_length_including_negative() {
        let gate = |length: Option<&str>| -> String {
            match json_body_of(
                &declared_request("POST", "/api/pets/configure", b"{}", length),
                PET_LIFECYCLE_BODY_LIMIT,
            ) {
                JsonBody::NoBody => "no-body".to_string(),
                JsonBody::Parsed(_) => "parsed".to_string(),
                JsonBody::Raised(message) => message,
            }
        };
        assert_eq!(gate(None), "no-body", "no Content-Length means an empty body");
        assert_eq!(gate(Some("0")), "no-body");
        assert_eq!(gate(Some("2")), "parsed");
        assert_eq!(
            gate(Some("65536")),
            "incomplete_request",
            "Python rejects `> limit`, not `>= limit`"
        );
        assert_eq!(gate(Some("65537")), "request_too_large");
        assert_eq!(gate(Some("-1")), "request_too_large", "`length < 0` is the same ValueError");
        assert_eq!(
            gate(Some("abc")),
            "invalid literal for int() with base 10: 'abc'"
        );
    }

    /// `readmd.py:1682-1685`: configure's `except ValueError` only stays silent
    /// for the never-raised `'payload_too_large'`, so the real answer is
    /// `400 {'ok': False, 'error_code': 'request_too_large'}`.
    #[test]
    fn configure_oversize_body_is_400_request_too_large() {
        let app = test_app("configure-too-large");
        let req = declared_request("POST", "/api/pets/configure", b"{}", Some("70000"));
        let (status, body) = answer(h_pet_configure(&app, &req));
        assert_eq!(status, 400);
        assert_eq!(key_set(&body), ["error_code", "ok"]);
        assert_eq!(body["error_code"], json!("request_too_large"));
        assert_eq!(body["ok"], json!(false));
    }

    /// A negative declaration used to parse as "no Content-Length" through
    /// `parse::<usize>()`; Python raises for it exactly like an oversized body.
    #[test]
    fn configure_negative_content_length_is_not_silently_accepted() {
        let app = test_app("configure-negative");
        let req = declared_request("POST", "/api/pets/configure", b"{}", Some("-1"));
        let (status, body) = answer(h_pet_configure(&app, &req));
        assert_eq!((status, body["error_code"].as_str()), (400, Some("request_too_large")));
    }

    /// `readmd.py:1701-1702` — interact answers `ValueError` with the `code`
    /// key, while its 405 arm (`readmd.py:1692`) keeps `error_code`.  The two
    /// arms of one route must not be conflated.
    #[test]
    fn interact_oversize_body_uses_the_code_key_not_error_code() {
        let app = test_app("interact-too-large");
        let req = declared_request("POST", "/api/pets/interact", b"{}", Some("70000"));
        let (status, body) = answer(h_pet_interact(&app, &req));
        assert_eq!(status, 400);
        assert_eq!(key_set(&body), ["code", "ok"]);
        assert_eq!(body["code"], json!("request_too_large"));
        assert!(body.get("error_code").is_none());

        let wrong_method = declared_request("GET", "/api/pets/interact", b"", Some("0"));
        let (status, body) = answer(h_pet_interact(&app, &wrong_method));
        assert_eq!(status, 405);
        assert_eq!(key_set(&body), ["error_code", "ok"]);
        assert_eq!(body["error_code"], json!("method_not_allowed"));
    }

    /// Both update routes have no `ValueError` arm, so the oversize
    /// `ValueError` reaches their blanket `except Exception`:
    /// `readmd.py:1862-1864` and `readmd.py:1875-1877`.
    #[test]
    fn update_routes_oversize_body_is_500_with_their_own_error_code() {
        let app = test_app("update-too-large");
        let cases = [
            ("/api/pets/check_update", "pet_check_update_failed"),
            ("/api/pets/apply_update", "pet_apply_update_failed"),
        ];
        for (path, code) in cases {
            let req = declared_request("POST", path, b"{}", Some("70000"));
            let (status, body) = answer(if path.ends_with("check_update") {
                h_pet_check_update(&app, &req)
            } else {
                h_pet_apply_update(&app, &req)
            });
            assert_eq!(status, 500, "{path} must not answer 200 for an oversize body");
            assert_eq!(key_set(&body), ["error_code", "ok"]);
            assert_eq!(body["error_code"], json!(code));
            assert_eq!(body["ok"], json!(false));
        }
    }

    /// `_handle_pet_lifecycle_action` (`readmd.py:1715-1721`) forwards the dict
    /// the api method returned at 200 and reserves `error_code` for a raised
    /// exception.  The Rust pet actions return `Value`, so this helper can only
    /// ever reach the 200 path — recorded so the dead `map_err` arm stays
    /// visible to the wave that gives those actions a failure channel.
    #[test]
    fn lifecycle_forwards_a_failed_action_dict_at_200() {
        let app = test_app("lifecycle");
        let req = declared_request("POST", "/api/pets/runtime/install", b"", Some("0"));
        let (status, body) = answer(lifecycle(&req, &app, "pet_plugin_install_failed", |_| {
            PetOutcome::Returned(json!({ "ok": false, "code": "rust_runtime_install_failed" }))
        }));
        assert_eq!(status, 200);
        assert_eq!(key_set(&body), ["code", "ok"]);
        assert_eq!(body["code"], json!("rust_runtime_install_failed"));

        let wrong_method = declared_request("GET", "/api/pets/runtime/install", b"", Some("0"));
        let (status, body) = answer(lifecycle(&wrong_method, &app, "pet_plugin_install_failed", |_| {
            PetOutcome::Returned(json!({ "ok": true }))
        }));
        assert_eq!((status, body["error_code"].as_str()), (405, Some("method_not_allowed")));
    }

    /// A request carrying `raw` and a `Content-Length` that matches it, i.e. the
    /// document Python's `json.loads` actually sees.
    fn json_request(method: &str, path: &str, raw: &str) -> Request {
        declared_request(method, path, raw.as_bytes(), Some(&raw.len().to_string()))
    }

    /// `readmd.py:1466-1467` — `if not n: return {}` short-circuits *before*
    /// `json.loads`, so an empty declaration is not a parse failure: the route
    /// runs `configure_pet({})` and answers 200 with its dict.  The previous
    /// `serde_json::from_slice(&[])` shape answered `400` here.
    #[test]
    fn configure_with_an_empty_declaration_answers_200_from_configure_pet_of_empty_dict() {
        let app = test_app("configure-empty-body");
        for length in [None, Some("0"), Some("00")] {
            let req = declared_request("POST", "/api/pets/configure", b"", length);
            let (status, body) = answer(h_pet_configure(&app, &req));
            assert_eq!(status, 200, "`if not n` never reaches the parser");
            assert_eq!(body["ok"], json!(true));
            assert_eq!(
                key_set(&body),
                ["adapter", "in_app", "model", "ok", "renderer", "runtime"]
            );
        }
    }

    /// The *present*-but-undecodable arm: `json.loads` raises `JSONDecodeError`,
    /// which is a `ValueError` subclass, and `readmd.py:1685` forwards `str(e)`
    /// as the `error_code`.
    #[test]
    fn configure_with_a_malformed_body_answers_the_decoder_message() {
        let app = test_app("configure-malformed");
        let (status, body) = answer(h_pet_configure(
            &app,
            &json_request("POST", "/api/pets/configure", "not-json"),
        ));
        assert_eq!(status, 400);
        assert_eq!(key_set(&body), ["error_code", "ok"]);
        assert_eq!(
            body["error_code"],
            json!("Expecting value: line 1 column 1 (char 0)")
        );

        let (status, body) = answer(h_pet_configure(
            &app,
            &json_request("POST", "/api/pets/configure", "  {\"a\":"),
        ));
        assert_eq!(status, 400);
        assert_eq!(body["ok"], json!(false));
    }

    /// `configure_pet` is the one pet route that `isinstance`-checks its body
    /// (`readmd.py:5523-5525`), so a JSON array is a 200 with the `code` key —
    /// not the `AttributeError`-turned-500 its neighbours get.
    #[test]
    fn configure_with_a_non_object_document_is_invalid_pet_settings_at_200() {
        let app = test_app("configure-non-object");
        let (status, body) = answer(h_pet_configure(
            &app,
            &json_request("POST", "/api/pets/configure", "[1, 2]"),
        ));
        assert_eq!(status, 200);
        assert_eq!(key_set(&body), ["code", "ok"]);
        assert_eq!(body["code"], json!("invalid_pet_settings"));
    }

    /// interact's empty-body arm is `{}` → `str(None or '').strip()` → `''` →
    /// `companion.py:74`'s `ValueError('invalid_pet_action')` → the `code`-keyed
    /// 400, *not* a parse error and not `payload_too_large`.
    #[test]
    fn interact_with_an_empty_declaration_is_invalid_pet_action() {
        let app = test_app("interact-empty-body");
        for length in [None, Some("0")] {
            let req = declared_request("POST", "/api/pets/interact", b"", length);
            let (status, body) = answer(h_pet_interact(&app, &req));
            assert_eq!(status, 400);
            assert_eq!(key_set(&body), ["code", "ok"]);
            assert_eq!(body["code"], json!("invalid_pet_action"));
        }
    }

    /// `body.get('action')` (`readmd.py:1696`) on a non-object document raises
    /// `AttributeError`; only the blanket arm sees it.
    #[test]
    fn interact_with_a_non_object_document_is_500_pet_interact_failed() {
        let app = test_app("interact-non-object");
        for raw in ["[1]", "\"text\"", "7"] {
            let req = json_request("POST", "/api/pets/interact", raw);
            let (status, body) = answer(h_pet_interact(&app, &req));
            assert_eq!((status, body["code"].as_str()), (500, Some("pet_interact_failed")), "{raw}");
            assert_eq!(key_set(&body), ["code", "ok"]);
            assert!(body.get("error_code").is_none());
        }
    }

    /// `PetCompanion._save` (`companion.py:110`) calls
    /// `mkdir(parents=True, exist_ok=True)`, which raises `FileExistsError`
    /// (an `OSError`) when `<data_dir>/pet` is a *file*.  `interact_pet`
    /// (`readmd.py:5247-5254`) has no guard for it, so the blanket arm answers
    /// `500 {'ok': False, 'code': 'pet_interact_failed'}` — the arm that had no
    /// channel at all while `save()` swallowed its errors.
    #[test]
    fn interact_is_500_when_the_companion_state_cannot_be_written() {
        let app = test_app("interact-save-blocked");
        fs::create_dir_all(&pet_data_dir(&app)).unwrap();
        fs::write(pet_data_dir(&app).join("pet"), b"not a directory").unwrap();
        let req = json_request("POST", "/api/pets/interact", r#"{"action":"pet"}"#);
        let (status, body) = answer(h_pet_interact(&app, &req));
        assert_eq!(status, 500);
        assert_eq!(key_set(&body), ["code", "ok"]);
        assert_eq!(body["code"], json!("pet_interact_failed"));
    }

    /// Item 8: `str(body.get(k) or '')` is not `json.get(k).as_str()`.  Falsy
    /// JSON collapses to `''`, `True` becomes `'True'`, a float keeps its
    /// mandatory decimal point and containers fall to their `repr`.
    #[test]
    fn interact_stringifies_non_string_body_values_like_python() {
        let app = test_app("interact-stringify");
        let cases: [(&str, &str); 6] = [
            ("true", "True"),
            ("3.0", "3.0"),
            ("12", "12"),
            ("{\"a\": 1}", "{'a': 1}"),
            ("[1, 2]", "[1, 2]"),
            ("false", "arch-chan"),
        ];
        for (raw_value, expected) in cases {
            let raw = format!("{{\"action\":\"pet\",\"character\":{raw_value}}}");
            let (status, body) = answer(h_pet_interact(&app, &json_request("POST", "/api/pets/interact", &raw)));
            assert_eq!(status, 200, "{raw_value} must reach a profile");
            assert_eq!(
                body["companion"]["character"],
                json!(expected),
                "`str({raw_value} or '')` in Python"
            );
        }
    }

    /// The three routes that read their body inline report *their own* default
    /// code for every failure, because `getattr(exc, 'code', default)` finds no
    /// `code` attribute on a `ValueError` or an `AttributeError`.
    #[test]
    fn inline_body_readers_answer_their_route_default_code() {
        let app = test_app("inline-default-codes");
        let cases = [
            ("/api/pets/import", "pet_import_failed", (413, "pet_spritesheet_too_large"), h_pet_import as fn(&Arc<App>, &Request) -> ApiResult<Response>),
            ("/api/pets/remove", "pet_remove_failed", (400, "confirmation_required"), h_pet_remove),
            ("/api/pets/active", "pet_active_failed", (400, "confirmation_required"), h_pet_active),
        ];
        for (path, code, empty, handler) in cases {
            for raw in ["not-json", "[1]", "\"text\""] {
                let (status, body) = answer(handler(&app, &json_request("POST", path, raw)));
                assert_eq!((status, body["error_code"].as_str()), (400, Some(code)), "{path} {raw}");
            }
            // A non-integer declaration fails at the bare `int()`, before the
            // parser, and lands in the same default.
            let bad_length = declared_request("POST", path, b"{}", Some("abc"));
            let (status, body) = answer(handler(&app, &bad_length));
            assert_eq!((status, body["error_code"].as_str()), (400, Some(code)), "{path} length");
            // An empty declaration is `{}` for remove/active — whose missing
            // 'confirm' is the *other* 400, proving the parse short-circuited —
            // while import gates `if n <= 0` before parsing (`readmd.py:1735`).
            let (status, body) = answer(handler(&app, &declared_request("POST", path, b"", Some("0"))));
            assert_eq!(
                (status, body["error_code"].as_str()),
                (empty.0, Some(empty.1)),
                "{path} empty"
            );
        }
    }

    /// CPython's `int()` acceptance set for a header value (`readmd.py:1465`),
    /// verified against a real interpreter for every case below.
    ///
    /// One accepted deviation: `int('٣') == 3` in CPython, which also takes
    /// Unicode decimal digits.  An HTTP header is decoded as ISO-8859-1, where
    /// no such digit exists, so `py_int`'s ASCII-only digit class cannot be
    /// reached with one.
    #[test]
    fn py_int_matches_cpython_and_its_failure_message() {
        let ok: [(&str, i64); 11] = [
            ("0", 0),
            ("000", 0),
            ("-0", 0),
            ("  42  ", 42),
            ("+7", 7),
            ("1_000", 1000),
            ("1_0", 10),
            ("1_2_3", 123),
            ("0_0", 0),
            ("\u{b}12\u{c}", 12),
            ("9223372036854775807", i64::MAX),
        ];
        for (text, expected) in ok {
            assert_eq!(py_int(text), Ok(expected), "{text:?}");
        }
        // Overflow saturates: every caller only compares against a byte limit.
        assert_eq!(py_int("9".repeat(40).as_str()), Ok(i64::MAX));
        for rejected in ["", " ", "abc", "_1", "1_", "1__0", "0x10", "1.0", "-", "5 5"] {
            assert!(py_int(rejected).is_err(), "{rejected:?} must raise");
        }
        assert_eq!(
            py_int("abc"),
            Err("invalid literal for int() with base 10: 'abc'".to_string())
        );
        assert_eq!(
            py_int(""),
            Err("invalid literal for int() with base 10: ''".to_string())
        );
        assert_eq!(
            py_int("a'b"),
            Err("invalid literal for int() with base 10: \"a'b\"".to_string())
        );
        assert_eq!(
            py_int("\u{1b}"),
            Err("invalid literal for int() with base 10: '\\x1b'".to_string())
        );
    }

    /// `bool(body.get('allow_network', True))`: the `True` belongs to `dict.get`,
    /// not to `bool`, so a present `null` disables the network branch while an
    /// absent key keeps it.
    #[test]
    fn allow_network_only_defaults_when_the_key_is_absent() {
        let parse = |raw: &str| -> bool {
            let value: Value = serde_json::from_str(raw).unwrap();
            allow_network_of(value.as_object().unwrap())
        };
        assert!(parse("{}"), "`body.get('allow_network', True)` defaults to True");
        assert!(parse(r#"{"allow_network": true}"#));
        assert!(parse(r#"{"allow_network": "off"}"#), "a non-empty string is truthy");
        assert!(!parse(r#"{"allow_network": null}"#));
        assert!(!parse(r#"{"allow_network": false}"#));
        assert!(!parse(r#"{"allow_network": 0}"#));
        assert!(!parse(r#"{"allow_network": ""}"#));
        assert!(!parse(r#"{"allow_network": []}"#));
    }

    // ------------------------------------------------------ installer fixtures

    /// The three members a real `install_archive`/`_verify_tree` accepts: an
    /// `artifacts` manifest carrying `sha256`/`size`, its executable and
    /// `renderer/index.html` (`runtime.py:161-192`).
    fn bundle_parts() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let mut exe = vec![b'M', b'Z', 0x90, 0x00];
        exe.extend((0u8..=255).cycle().take(600));
        let html = b"<!doctype html><title>pet</title>".to_vec();
        let manifest = json!({
            "runtime": "readmd-pet-rust",
            "protocol_version": 1,
            "platform": "any",
            "arch": "any",
            "artifacts": [
                {
                    "path": "renderer/index.html",
                    "sha256": sha256_hex(&html),
                    "size": html.len(),
                    "role": "renderer",
                },
                {
                    "path": "readmd-pet-rust.exe",
                    "sha256": sha256_hex(&exe),
                    "size": exe.len(),
                    "role": "executable",
                },
            ],
        });
        (serde_json::to_vec(&manifest).expect("manifest bytes"), exe, html)
    }

    /// A stored-member ZIP from the crate's own writer (`mdexport::write_zip`).
    fn zip_of(members: &[(&str, &[u8])]) -> Vec<u8> {
        let entries: Vec<crate::mdexport::ZipEntry> = members
            .iter()
            .map(|(path, data)| crate::mdexport::ZipEntry {
                path: (*path).to_string(),
                data: data.to_vec(),
                compress: false,
            })
            .collect();
        crate::mdexport::write_zip(&entries).expect("zip writer")
    }

    fn bundle_zip() -> Vec<u8> {
        let (manifest, exe, html) = bundle_parts();
        zip_of(&[
            ("runtime-manifest.json", &manifest),
            ("renderer/index.html", &html),
            ("readmd-pet-rust.exe", &exe),
        ])
    }

    /// Offset of `name`'s central-directory record.
    fn central_record(blob: &[u8], name: &str) -> usize {
        (0..blob.len().saturating_sub(46))
            .find(|&at| {
                blob[at..at + 4] == *b"PK\x01\x02"
                    && blob.len() >= at + 46 + name.len()
                    && &blob[at + 46..at + 46 + name.len()] == name.as_bytes()
            })
            .expect("central directory record")
    }

    /// Re-stamp one member's general-purpose flags and compression method.
    /// Those fields sit at `+6/+8` in a local header and `+8/+10` in its
    /// central record, so both have to move for the fixture to stay consistent.
    fn restamp_member(blob: &mut Vec<u8>, name: &str, flags: u16, method: u16) {
        let local = zip::entries(blob)
            .expect("entries")
            .into_iter()
            .find(|entry| entry.name == name)
            .expect("member")
            .local_offset as usize;
        let central = central_record(blob, name);
        for (at, u16_at) in [
            (local + 6, flags),
            (local + 8, method),
            (central + 8, flags),
            (central + 10, method),
        ] {
            blob[at..at + 2].copy_from_slice(&u16_at.to_le_bytes());
        }
    }

    /// `zipfile`'s `RuntimeError` for an encrypted member: on the host (read
    /// inside `install_archive`'s staging loop, `runtime.py:349`) and on the
    /// manifest (read at `runtime.py:328`).
    fn encrypted_zip(host: bool) -> Vec<u8> {
        let mut blob = bundle_zip();
        restamp_member(
            &mut blob,
            if host { "readmd-pet-rust.exe" } else { "runtime-manifest.json" },
            1,
            0,
        );
        blob
    }

    /// `zipfile`'s `NotImplementedError` for a method it does not implement.
    fn unsupported_method_zip() -> Vec<u8> {
        let mut blob = bundle_zip();
        restamp_member(&mut blob, "readmd-pet-rust.exe", 0, 99);
        blob
    }

    /// The shape a truncated archive leaves: the central directory promises more
    /// data for the member than the file holds.
    fn oversized_member_zip() -> Vec<u8> {
        let mut blob = bundle_zip();
        let central = central_record(&blob, "readmd-pet-rust.exe");
        let declared = (blob.len() as u32) + 4096;
        blob[central + 20..central + 24].copy_from_slice(&declared.to_le_bytes());
        blob
    }

    /// Write `blob` under a name the installer's extension gate accepts.
    fn archive_file(tag: &str, blob: &[u8]) -> PathBuf {
        let path = scratch_dir(tag).join("ReadMD-Pet-Rust.zip");
        fs::write(&path, blob).expect("write archive");
        path
    }

    /// The bundle laid out as a directory tree for `install_directory`.
    fn bundle_tree(tag: &str) -> PathBuf {
        let dir = scratch_dir(tag);
        let (manifest, exe, html) = bundle_parts();
        fs::create_dir_all(dir.join("renderer")).unwrap();
        fs::write(dir.join("runtime-manifest.json"), &manifest).unwrap();
        fs::write(dir.join("readmd-pet-rust.exe"), &exe).unwrap();
        fs::write(dir.join("renderer").join("index.html"), &html).unwrap();
        dir
    }

    /// `{'update_info': …}` straight through to `apply_pet_update`, so the
    /// bundled branch runs without any network candidate being reachable.
    fn update_request(archive: &Path) -> Request {
        let body = json!({
            "update_info": {
                "has_update": true,
                "source": "bundled",
                "archive_path": archive.to_string_lossy(),
            },
        });
        let raw = serde_json::to_string(&body).expect("update_info body");
        json_request("POST", "/api/pets/apply_update", &raw)
    }

    // ------------------------------------------------------ the raised channel

    /// `runtime.py:349` raising `RuntimeError` for an encrypted member is not in
    /// `install_archive`'s except tuple (`runtime.py:355`), so the api method
    /// raises and `install_zip` must report `Raised`, never a result dict.
    #[test]
    fn install_zip_raises_for_an_encrypted_member() {
        for (tag, host) in [("host", true), ("manifest", false)] {
            let app = test_app(&format!("zip-encrypted-{tag}"));
            let installer = RuntimeTree::rust(&app);
            let archive = archive_file(&format!("zip-encrypted-{tag}"), &encrypted_zip(host));
            assert!(
                matches!(install_zip(&installer, &archive, true), PetOutcome::Raised),
                "encrypted {tag} member must escape install_archive"
            );
            assert!(!installer.target.exists(), "nothing may be published");
        }
    }

    /// The second member of the escape set: an unimplemented compression method
    /// is a `NotImplementedError`, caught by neither `runtime.py:355` nor any
    /// route handler.
    #[test]
    fn install_zip_raises_for_an_unsupported_compression_member() {
        let app = test_app("zip-method");
        let installer = RuntimeTree::rust(&app);
        let archive = archive_file("zip-method", &unsupported_method_zip());
        assert!(matches!(
            install_zip(&installer, &archive, true),
            PetOutcome::Raised
        ));
        assert!(!installer.target.exists());
    }

    /// A member that overruns the archive is *not* a short read on CPython 3.11:
    /// `zipfile.py:1623-1625` turns it into `BadZipFile("Overlapped entries …")`,
    /// which `runtime.py:355` catches, so the reference answer stays
    /// `{'ok': False, 'code': 'rust_runtime_install_failed'}`.  Confirmed by
    /// running the real `install_archive` against this fixture shape.
    #[test]
    fn install_zip_keeps_an_overrunning_member_as_a_handled_install_failure() {
        let app = test_app("zip-oversized-member");
        let installer = RuntimeTree::rust(&app);
        let archive = archive_file("zip-oversized-member", &oversized_member_zip());
        let outcome = install_zip(&installer, &archive, true);
        let value = outcome.value().expect("a caught failure stays a result dict");
        assert_eq!(value["code"], json!("rust_runtime_install_failed"));
        assert!(!installer.target.exists());
    }

    /// `readmd.py:1719-1721`: `except Exception` answers
    /// `500 {'ok': False, 'error_code': …}`.  This 500 arm of `lifecycle` had no
    /// test at all, which is how the bundled-branch defect survived a green
    /// suite.
    #[test]
    fn lifecycle_maps_a_raised_outcome_to_the_routes_error_code() {
        let app = test_app("lifecycle-raised");
        let req = declared_request("POST", "/api/pets/runtime/install", b"", Some("0"));
        let (status, body) = answer(lifecycle(&req, &app, "pet_plugin_install_failed", |_| {
            PetOutcome::Raised
        }));
        assert_eq!(status, 500);
        assert_eq!(key_set(&body), ["error_code", "ok"]);
        assert_eq!(body["error_code"], json!("pet_plugin_install_failed"));
        assert!(body.get("code").is_none(), "the 500 family has no `code` key");
    }

    /// The same arm end-to-end: `/api/pets/runtime/install` finds a bundled
    /// `ReadMD-Pet-Rust.zip` whose host member is encrypted, and the raise
    /// escapes the candidate loop (`readmd.py:5491-5521`) as a 500.
    #[test]
    fn runtime_install_route_answers_500_when_a_bundled_candidate_raises() {
        let app = test_app("runtime-install-raised");
        fs::write(
            app_dir(&app).join("ReadMD-Pet-Rust.zip"),
            &encrypted_zip(true),
        )
        .expect("stage the bundled candidate");
        let req = declared_request("POST", "/api/pets/runtime/install", b"", Some("0"));
        let (status, body) = answer(h_pet_runtime_install(&app, &req));
        assert_eq!(status, 500);
        assert_eq!(key_set(&body), ["error_code", "ok"]);
        assert_eq!(body["error_code"], json!("pet_plugin_install_failed"));
    }

    /// `updater.py:379` sends the bundled archive through the same
    /// `install_archive` gate, and `apply_pet_update` has a `try:`/`finally:`
    /// with no `except` (`readmd.py:5309-5321`), so the raise must reach
    /// `readmd.py:1875-1877`.
    #[test]
    fn apply_update_route_answers_500_when_the_bundled_archive_raises() {
        let app = test_app("apply-update-raised");
        let archive = archive_file("apply-update-raised", &encrypted_zip(true));
        let (status, body) = answer(h_pet_apply_update(&app, &update_request(&archive)));
        assert_eq!(status, 500);
        assert_eq!(key_set(&body), ["error_code", "ok"]);
        assert_eq!(body["error_code"], json!("pet_apply_update_failed"));
    }

    /// The other half of `updater.py:379-389`: a valid bundled archive really
    /// installs.  Routing this through `install_directory` instead answered
    /// `invalid_rust_runtime_directory` at 200.
    #[test]
    fn apply_update_route_installs_a_bundled_archive() {
        let app = test_app("apply-update-bundled");
        let installer = RuntimeTree::rust(&app);
        let target = installer.target.to_string_lossy().into_owned();
        let archive = archive_file("apply-update-bundled", &bundle_zip());
        let (status, body) = answer(h_pet_apply_update(&app, &update_request(&archive)));
        assert_eq!(status, 200);
        assert_eq!(body["ok"], json!(true), "{body}");
        assert_eq!(body["updated"], json!(true));
        assert_eq!(body["source"], json!("bundled"));
        assert_eq!(body["install_path"], json!(target));
        assert!(installer.target.join("readmd-pet-rust.exe").is_file());
        assert!(installer.target.join("renderer").join("index.html").is_file());
        assert!(installer.target.join("runtime-manifest.json").is_file());
    }

    // ------------------------------------------------------ install_directory

    /// `install_directory` copies nothing until `_verify_tree` has walked the
    /// tree and digested every expected file.
    #[test]
    fn install_directory_publishes_a_verified_tree() {
        let app = test_app("install-directory-ok");
        let installer = RuntimeTree::rust(&app);
        let source = bundle_tree("install-directory-ok");
        let result = installer.install_directory(&source, true);
        assert_eq!(result["ok"], json!(true), "{result}");
        assert_eq!(result["installed"], json!(true));
        assert!(installer.target.join("readmd-pet-rust.exe").is_file());
        assert!(installer.target.join("renderer").join("index.html").is_file());
        assert!(installer.target.join("runtime-manifest.json").is_file());
    }

    /// The manifest is an `artifacts` list; a path/digest `files` object is
    /// `invalid_rust_runtime_manifest` (`runtime.py:161-163`), not a list of
    /// files to copy without verification.
    #[test]
    fn install_directory_rejects_a_files_shaped_manifest() {
        let app = test_app("install-directory-files");
        let installer = RuntimeTree::rust(&app);
        let source = bundle_tree("install-directory-files");
        let files = json!({
            "runtime": "readmd-pet-rust",
            "protocol_version": 1,
            "platform": "any",
            "arch": "any",
            "files": { "readmd-pet-rust.exe": "0".repeat(64) },
        });
        fs::write(
            source.join("runtime-manifest.json"),
            serde_json::to_vec(&files).unwrap(),
        )
        .unwrap();
        let result = installer.install_directory(&source, true);
        assert_eq!(result["code"], json!("invalid_rust_runtime_manifest"));
        assert!(!installer.target.exists());
    }

    /// `runtime.py:240-243` — the gate that catches "a corrupt or tampered
    /// executable before it can ever be spawned" (`runtime.py:236-237`).
    #[test]
    fn install_directory_reports_a_tampered_executable_as_hash_mismatch() {
        let app = test_app("install-directory-tampered");
        let installer = RuntimeTree::rust(&app);
        let source = bundle_tree("install-directory-tampered");
        let (_, mut exe, _) = bundle_parts();
        exe[0] = b'X';
        fs::write(source.join("readmd-pet-rust.exe"), &exe).unwrap();
        let result = installer.install_directory(&source, true);
        assert_eq!(result["code"], json!("rust_runtime_hash_mismatch"));
        assert!(!installer.target.exists());
    }

    /// Every shipped file has to be represented by the manifest
    /// (`runtime.py:231-239`).
    #[test]
    fn install_directory_reports_an_unlisted_extra_file() {
        let app = test_app("install-directory-extra");
        let installer = RuntimeTree::rust(&app);
        let source = bundle_tree("install-directory-extra");
        fs::write(source.join("evil.dll"), b"junk").unwrap();
        let result = installer.install_directory(&source, true);
        assert_eq!(result["code"], json!("rust_runtime_unlisted_file"));
        assert!(!installer.target.exists());
    }

    /// `runtime.py:224-225` — a missing expected file is reported before any
    /// digest is computed.
    #[test]
    fn install_directory_reports_a_missing_expected_file() {
        let app = test_app("install-directory-missing");
        let installer = RuntimeTree::rust(&app);
        let source = bundle_tree("install-directory-missing");
        fs::remove_file(source.join("readmd-pet-rust.exe")).unwrap();
        let result = installer.install_directory(&source, true);
        assert_eq!(result["code"], json!("rust_runtime_file_missing"));
        assert!(!installer.target.exists());
    }

    /// `runtime.py:226-234` exempts the executable's mutable
    /// `<path>.WebView2/` tree from the extras gate — and only from that gate,
    /// since publishing still walks `expected`.
    #[test]
    fn install_directory_allows_the_mutable_webview_tree_below_the_executable() {
        let app = test_app("install-directory-webview");
        let installer = RuntimeTree::rust(&app);
        let source = bundle_tree("install-directory-webview");
        let mutable = source.join("readmd-pet-rust.exe.WebView2").join("Default");
        fs::create_dir_all(&mutable).unwrap();
        fs::write(mutable.join("DIPS"), b"sqlite").unwrap();
        let result = installer.install_directory(&source, true);
        assert_eq!(result["ok"], json!(true), "{result}");
        assert!(!installer.target.join("readmd-pet-rust.exe.WebView2").exists());
    }

    // ------------------------------------------------- defect #1: digest gate

    /// The shipped manifest lists its files under `artifacts`, so a tree whose
    /// executable has been swapped after install — same length, different
    /// content — must NOT verify.  The old `verified_install` digested
    /// `manifest["files"]`, found nothing, hit the `None => true` arm and
    /// reported the tampered tree as a good install.
    #[test]
    fn verified_install_rejects_a_tampered_installed_artifact() {
        let app = test_app("verified-install-tamper");
        let installer = RuntimeTree::rust(&app);
        let source = bundle_tree("verified-install-tamper");
        assert_eq!(installer.install_directory(&source, true)["ok"], json!(true));
        assert!(installer.verified_install(), "a clean install must verify");
        let exe = installer.target.join("readmd-pet-rust.exe");
        let mut bytes = fs::read(&exe).unwrap();
        bytes[4] ^= 0xFF; // keep the length identical; only a content digest sees this
        fs::write(&exe, &bytes).unwrap();
        assert!(
            !installer.verified_install(),
            "a tampered artifact must fail verification, not fall through to `None => true`"
        );
        assert!(!installer.available());
    }

    /// A manifest with no `artifacts` section is not a verified install.  This is
    /// the exact arm the old code returned `true` from.
    #[test]
    fn verified_install_requires_an_artifacts_section() {
        let app = test_app("verified-install-noartifacts");
        let installer = RuntimeTree::rust(&app);
        let source = bundle_tree("verified-install-noartifacts");
        assert_eq!(installer.install_directory(&source, true)["ok"], json!(true));
        let bare = json!({
            "runtime": "readmd-pet-rust",
            "protocol_version": 1,
            "platform": "any",
            "arch": "any",
        });
        fs::write(
            installer.target.join("runtime-manifest.json"),
            serde_json::to_vec(&bare).unwrap(),
        )
        .unwrap();
        assert!(
            !installer.verified_install(),
            "missing `artifacts` must fail closed (the old `None => true` arm said yes)"
        );
    }

    // --------------------------------------------- defect #2: Windows swap retry

    /// `SWAP_ATTEMPTS` / `SWAP_DELAY` must equal Python's 40 / 0.25 s.
    #[test]
    fn swap_retry_matches_the_python_attempt_count_and_interval() {
        assert_eq!(SWAP_ATTEMPTS, 40, "runtime.py:47 SWAP_ATTEMPTS");
        assert_eq!(SWAP_DELAY, std::time::Duration::from_millis(250), "runtime.py:48 SWAP_DELAY");
    }

    /// A locked target that keeps failing is retried exactly `SWAP_ATTEMPTS`
    /// times and then fails; the old code gave up after one rename.
    #[test]
    fn swap_retry_gives_up_after_the_attempt_count_on_a_still_locked_target() {
        let mut calls = 0u32;
        let result = retry_swap(
            || {
                calls += 1;
                Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "locked",
                ))
            },
            SWAP_ATTEMPTS,
            std::time::Duration::ZERO,
        );
        assert!(result.is_err(), "a permanently locked swap must fail");
        assert_eq!(calls, SWAP_ATTEMPTS, "must retry the Python number of times");
    }

    /// A target that frees up mid-retry succeeds on the later attempt.
    #[test]
    fn swap_retry_succeeds_once_the_locked_target_frees_up() {
        let mut calls = 0u32;
        let result = retry_swap(
            || {
                calls += 1;
                if calls < 4 {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::PermissionDenied,
                        "locked",
                    ))
                } else {
                    Ok(())
                }
            },
            SWAP_ATTEMPTS,
            std::time::Duration::ZERO,
        );
        assert!(result.is_ok(), "the retry must land once the lock clears");
        assert_eq!(calls, 4);
    }

    /// A non-`PermissionError` (e.g. a missing source) is not a lock: it escapes
    /// immediately rather than spinning 40 times, matching Python's narrower
    /// `except PermissionError`.
    #[test]
    fn swap_retry_does_not_spin_on_non_permission_errors() {
        let mut calls = 0u32;
        let result = retry_swap(
            || {
                calls += 1;
                Err(std::io::Error::new(std::io::ErrorKind::NotFound, "missing"))
            },
            SWAP_ATTEMPTS,
            std::time::Duration::ZERO,
        );
        assert!(result.is_err());
        assert_eq!(calls, 1, "an unrelated OS error must return on the first try");
    }

    // ------------------------------------------- defect #4: mirror fallback loop

    /// A candidate whose fetch yields `None` (unreachable / non-200 / stub body)
    /// must not abort the whole chain — the old `.ok()?` did exactly that.
    #[test]
    fn download_from_mirrors_tries_the_next_candidate_after_a_failure() {
        let urls = vec!["direct".to_string(), "m1".to_string(), "m2".to_string()];
        let mut tried = Vec::new();
        let got = download_from_mirrors(&urls, |candidate| {
            tried.push(candidate.to_string());
            if candidate == "m2" {
                Some(vec![0xAB; 2048])
            } else {
                None
            }
        });
        assert_eq!(tried, vec!["direct", "m1", "m2"], "every mirror up to the hit is tried");
        assert_eq!(got, Some(vec![0xAB; 2048]));
    }

    /// If no mirror is acceptable the chain is exhausted and reports `None`.
    #[test]
    fn download_from_mirrors_returns_none_only_after_all_fail() {
        let urls = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let mut tried = 0;
        let got = download_from_mirrors(&urls, |_| {
            tried += 1;
            None
        });
        assert!(got.is_none());
        assert_eq!(tried, 3, "all candidates must have been attempted");
    }

    // ------------------------------- defect #5: acceptance = declared size + SHA-256

    /// The install compares the real downloaded length against the manifest's
    /// declared `size` even when the digest is otherwise well-formed — proving
    /// acceptance is not a hash-only or length-only/CRC shortcut.
    #[test]
    fn install_zip_enforces_the_declared_size_independently_of_the_digest() {
        let (manifest, exe, html) = bundle_parts();
        let mut value: Value = serde_json::from_slice(&manifest).unwrap();
        value["artifacts"][0]["size"] = json!(1); // right sha256, wrong declared size
        let manifest = serde_json::to_vec(&value).unwrap();
        let blob = zip_of(&[
            ("runtime-manifest.json", &manifest),
            ("renderer/index.html", &html),
            ("readmd-pet-rust.exe", &exe),
        ]);
        let app = test_app("zip-size-mismatch");
        let installer = RuntimeTree::rust(&app);
        let archive = archive_file("zip-size-mismatch", &blob);
        let outcome = install_zip(&installer, &archive, true);
        let value = outcome.value().expect("a caught mismatch stays a result dict");
        assert_eq!(value["code"], json!("rust_runtime_hash_mismatch"));
        assert!(!installer.target.exists());
    }

    /// A length-preserving content change is caught, so the digest is a real
    /// content hash (SHA-256 via `sha256_hex`) and not a size/CRC shortcut.
    #[test]
    fn install_zip_catches_a_length_preserving_content_change() {
        let (manifest, mut exe, html) = bundle_parts();
        exe[4] ^= 0xFF; // manifest still carries the original SHA-256 and size
        let blob = zip_of(&[
            ("runtime-manifest.json", &manifest),
            ("renderer/index.html", &html),
            ("readmd-pet-rust.exe", &exe),
        ]);
        let app = test_app("zip-content-mismatch");
        let installer = RuntimeTree::rust(&app);
        let archive = archive_file("zip-content-mismatch", &blob);
        let outcome = install_zip(&installer, &archive, true);
        let value = outcome.value().expect("a caught mismatch stays a result dict");
        assert_eq!(value["code"], json!("rust_runtime_hash_mismatch"));
        assert!(!installer.target.exists());
    }

    // -------------------------------------------------- wave E: bundled scan

    /// A one-member archive carrying `runtime-manifest.json`, built with the
    /// crate's own stored-member writer.  Its digest follows `manifest_text`,
    /// so E1's "does this candidate match the install" comparisons cannot come
    /// out equal by accident.
    fn pet_zip(path: &Path, manifest_text: &str) {
        let bytes = zip_of(&[("runtime-manifest.json", manifest_text.as_bytes())]);
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn bundled_candidates_require_the_runtime_manifest() {
        let app = test_app("we-filter");
        let root = app_dir(&app);
        pet_zip(&root.join("ReadMD-Pet-Rust.zip"), "{}");
        fs::write(root.join("ReadMD-Desktop-Pet.zip"), b"not a zip").unwrap();
        let found = bundled_candidates(&app);
        assert_eq!(found.len(), 1);
        assert_eq!(
            found[0].file_name().unwrap().to_string_lossy(),
            "ReadMD-Pet-Rust.zip"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn bundled_candidates_sort_canonical_then_newest_first() {
        // E2 — `found.sort(key=_cand_sort_key, reverse=True)` (`updater.py:141`).
        let app = test_app("we-sort");
        let root = app_dir(&app);
        // The canonical name goes in last on purpose: discovery order must not
        // decide `candidates[0]`, the sort key must.
        pet_zip(&root.join("ReadMD-Pet-Rust.zip"), "{\"a\":1}");
        touch_mtimes(&root.join("ReadMD-Pet-Rust.zip"), Duration::from_secs(3600));
        pet_zip(&root.join("ReadMD-Desktop-Pet.zip"), "{\"b\":2}");
        pet_zip(&root.join("ReadMD-Pet-Rust-linux-x86_64.zip"), "{\"c\":3}");
        let found = bundled_candidates(&app);
        assert_eq!(
            found.iter().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect::<Vec<_>>(),
            vec![
                "ReadMD-Desktop-Pet.zip",
                "ReadMD-Pet-Rust-linux-x86_64.zip",
                "ReadMD-Pet-Rust.zip",
            ],
            "canonical name first, then newest mtime"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn bundled_candidates_prefer_the_newest_of_two_plain_names() {
        let app = test_app("we-sort2");
        let root = app_dir(&app);
        pet_zip(&root.join("ReadMD-Pet-Rust.zip"), "{\"a\":1}");
        pet_zip(&root.join("ReadMD-Pet-Rust-linux-x86_64.zip"), "{\"b\":2}");
        pet_zip(&root.join("ReadMD-Pet-Rust-macos-x86_64.zip"), "{\"c\":3}");
        touch_mtimes(&root.join("ReadMD-Pet-Rust.zip"), Duration::from_secs(7200));
        touch_mtimes(&root.join("ReadMD-Pet-Rust-linux-x86_64.zip"), Duration::from_secs(60));
        touch_mtimes(&root.join("ReadMD-Pet-Rust-macos-x86_64.zip"), Duration::from_secs(120));
        let found = bundled_candidates(&app);
        // `touch_mtimes` writes `mtime = now - age`, so the *smallest* age is
        // the *newest* file: linux (60s) > macos (120s) > Pet-Rust (7200s).
        // None of these three names is canonical — `_cand_sort_key` only calls
        // `readmd-desktop-pet.zip` canonical (`updater.py:133-135`) — so the
        // key degenerates to `mtime` and `reverse=True` puts the newest first.
        // Measured live in `scratch/rust_parity/pets_probe1.py` CASE A, which
        // returns `['ReadMD-Pet-Rust-linux-x86_64.zip',
        //            'ReadMD-Pet-Rust-macos-x86_64.zip',
        //            'ReadMD-Pet-Rust.zip']`.
        // `linux-x86_64` also sits LAST in `candidate_names`, so expecting it
        // still proves the sort — not discovery order — decides `found[0]`.
        assert_eq!(
            found[0].file_name().unwrap().to_string_lossy(),
            "ReadMD-Pet-Rust-linux-x86_64.zip",
            "of two non-canonical names the newer mtime wins, whatever the list order says"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn the_candidate_sort_is_stable_so_discovery_order_breaks_ties() {
        // `list.sort` is stable, so an exact key tie keeps discovery order;
        // `names` order therefore still matters and must not be reshuffled.
        let app = test_app("we-stable");
        let root = app_dir(&app);
        pet_zip(&root.join("ReadMD-Pet-Rust.zip"), "{\"a\":1}");
        pet_zip(&root.join("ReadMD-Pet-Rust-linux-x86_64.zip"), "{\"b\":2}");
        let same = fs::metadata(root.join("ReadMD-Pet-Rust.zip"))
            .unwrap()
            .modified()
            .unwrap();
        for name in ["ReadMD-Pet-Rust.zip", "ReadMD-Pet-Rust-linux-x86_64.zip"] {
            touch_mtimes_to(&root.join(name), same);
        }
        let found = bundled_candidates(&app);
        assert_eq!(
            found[0].file_name().unwrap().to_string_lossy(),
            "ReadMD-Pet-Rust.zip",
            "equal keys keep the discovery order of `names`"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn a_candidate_reachable_through_two_roots_is_offered_once() {
        // E3 — one `seen` set shared by roots and archives (`updater.py:116-131`).
        let app = test_app("we-dedupe");
        let root = app_dir(&app);
        let real = root.join("packages").join("readmd-pet-rust").join("dist");
        fs::create_dir_all(&real).unwrap();
        pet_zip(&real.join("ReadMD-Pet-Rust.zip"), "{\"a\":1}");
        let alias = root.join("alias");
        #[cfg(windows)]
        let linked = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&alias)
            .arg(&real)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false);
        #[cfg(not(windows))]
        let linked = std::os::unix::fs::symlink(&real, &alias).is_ok();
        if !linked {
            // No link privilege on this host: pin the ordinary same-root case
            // instead of silently passing.
            let found = bundled_candidates(&app);
            assert_eq!(found.len(), 1);
            let _ = fs::remove_dir_all(root);
            return;
        }
        let roots = [
            alias.clone(),
            real.clone(),
            root.join("packages").join("readmd-hermes-pet-adapter").join("dist"),
            alias.clone(),
        ];
        let mut seen: Vec<PathBuf> = Vec::new();
        let mut counted: Vec<PathBuf> = Vec::new();
        for candidate in roots {
            let Some(resolved) = try_resolve(&candidate) else {
                continue;
            };
            if seen.contains(&resolved) {
                continue;
            }
            seen.push(resolved);
            let archive = candidate.join("ReadMD-Pet-Rust.zip");
            if archive.is_file() && archive_has_runtime_manifest(&archive) {
                let Some(resolved_archive) = try_resolve(&archive) else {
                    continue;
                };
                if !seen.contains(&resolved_archive) {
                    seen.push(resolved_archive);
                    counted.push(archive);
                }
            }
        }
        assert_eq!(
            counted.len(),
            1,
            "the same directory behind two roots must yield one candidate"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn every_python_root_and_name_pair_is_probed() {
        // E4 + E5 — five roots x ten names (`updater.py:92-110`).
        let app = test_app("we-grid");
        let root = app_dir(&app);
        for segment in ["dist", "dist/ReadMD", "packages/readmd-hermes-pet-adapter/dist", "packages/readmd-pet-rust/dist"] {
            fs::create_dir_all(root.join(segment)).unwrap();
        }
        let roots: Vec<PathBuf> = vec![
            root.clone(),
            root.join("dist"),
            root.join("dist").join("ReadMD"),
            root.join("packages").join("readmd-hermes-pet-adapter").join("dist"),
            root.join("packages").join("readmd-pet-rust").join("dist"),
        ];
        for (index, candidate) in roots.iter().enumerate() {
            pet_zip(&candidate.join("ReadMD-Pet-Rust.zip"), &format!("{{\"root\":{index}}}"));
        }
        assert_eq!(bundled_candidates(&app).len(), 5);
        for candidate in &roots {
            let _ = fs::remove_file(candidate.join("ReadMD-Pet-Rust.zip"));
        }
        let names = [
            "ReadMD-Desktop-Pet.zip",
            "ReadMD-Desktop-Pet-review.zip",
            "readmd-hermes-pet-adapter-v0.1.0.zip",
            "ReadMD-Pet-Rust.zip",
            "ReadMD-Pet-Rust-windows-x86_64.zip",
            "ReadMD-Pet-Rust-windows-aarch64.zip",
            "ReadMD-Pet-Rust-macos-x86_64.zip",
            "ReadMD-Pet-Rust-macos-aarch64.zip",
            "ReadMD-Pet-Rust-linux-x86_64.zip",
            "ReadMD-Pet-Rust-linux-aarch64.zip",
        ];
        for name in names {
            pet_zip(&root.join(name), &format!("{{\"name\":\"{name}\"}}"));
        }
        assert_eq!(bundled_candidates(&app).len(), names.len());
        let _ = fs::remove_dir_all(root);
    }

    /// `os.utime(path, (t, t))` — Win32 `SetFileTime` through the stable
    /// `File::set_times`.  A whole `Duration` is subtracted because
    /// `SystemTime - Duration` is what picks the unit.
    fn touch_mtimes(path: &Path, age: Duration) {
        touch_mtimes_to(path, SystemTime::now() - age);
    }

    /// Both files share one `mtime`, so the tuple keys tie exactly.
    fn touch_mtimes_to(path: &Path, when: SystemTime) {
        let handle = fs::OpenOptions::new().write(true).open(path).unwrap();
        handle
            .set_times(fs::FileTimes::new().set_accessed(when).set_modified(when))
            .unwrap();
    }

    #[test]
    fn a_candidate_that_matches_the_installed_hash_never_reports_an_update() {
        // E1, case 1: the installed hash equals `candidates[2]`'s.  Python
        // computes `installed_matches_any` over EVERY candidate, so the whole
        // bundled branch is skipped and nothing is reported.
        let app = test_app("we-e1-match");
        let installer = RuntimeTree::rust_at(&app_dir(&app).join("plugins"));
        fs::create_dir_all(&installer.target).unwrap();
        fs::write(installer.target.join("readmd-pet-rust.exe"), b"pe").unwrap();
        fs::write(installer.target.join("runtime-manifest.json"), b"{\"installed\":1}").unwrap();
        let root = app_dir(&app);
        pet_zip(&root.join("ReadMD-Pet-Rust-macos-x86_64.zip"), "{\"cand\":0}");
        pet_zip(&root.join("ReadMD-Pet-Rust-linux-x86_64.zip"), "{\"cand\":1}");
        pet_zip(&root.join("ReadMD-Pet-Rust.zip"), "{\"installed\":1}");
        let report = check_pet_update(&app, false);
        assert_eq!(report["has_update"], json!(false), "{report}");
        assert_eq!(report["source"], json!("none"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn only_the_first_candidate_can_ever_be_offered() {
        // E1, case 2: `candidates[0]` differs from the install while a *later*
        // candidate matches it.  `installed_matches_any` scans EVERY candidate
        // (`updater.py:222-229`), so the whole bundled branch is skipped and
        // nothing is offered; the old Rust loop looked only at
        // `candidates[0]` and returned it.
        //
        // The fixture must therefore be a tree `_verify_tree` accepts.  The
        // guard is `if is_installed and installed_manifest_hash:` and
        // `is_installed` is `launcher.status()["available"]` ==
        // `_verified_install()` (`runtime.py:428-442`), so a bare
        // `{"installed":1}` manifest — no `artifacts`, so not a verified
        // install — left `is_installed` False, skipped the scan entirely and
        // never reached the rule this test is named for.  Measured live in
        // `scratch/rust_parity/pets_probe1.py` CASE B (unverified fixture:
        // Python answers `source: bundled` with the linux archive, exactly as
        // the kernel did) and `scratch/rust_parity/pets_probe2.py` CASE G1
        // (verified fixture: Python answers `source: none`, `has_update:
        // false`, `installed: true`).
        let app = test_app("we-e1-first");
        let installer = RuntimeTree::rust_at(&app_dir(&app).join("plugins"));
        let (manifest, exe, html) = bundle_parts();
        fs::create_dir_all(installer.target.join("renderer")).unwrap();
        fs::write(installer.target.join("runtime-manifest.json"), &manifest).unwrap();
        fs::write(installer.target.join("readmd-pet-rust.exe"), &exe).unwrap();
        fs::write(
            installer.target.join("renderer").join("index.html"),
            &html,
        )
        .unwrap();
        assert!(
            installer.verified_install(),
            "the guard needs a genuinely verified install"
        );
        let root = app_dir(&app);
        // Byte-identical to the installed manifest, so its digest matches.
        let same = String::from_utf8(manifest.clone()).expect("utf-8 manifest");
        pet_zip(&root.join("ReadMD-Pet-Rust.zip"), &same);
        pet_zip(&root.join("ReadMD-Pet-Rust-linux-x86_64.zip"), "{\"other\":2}");
        // Newest mtime first, so the *differing* archive is the only one
        // `candidates[0]`-only code could ever offer.
        touch_mtimes(&root.join("ReadMD-Pet-Rust-linux-x86_64.zip"), Duration::from_secs(10));
        touch_mtimes(&root.join("ReadMD-Pet-Rust.zip"), Duration::from_secs(600));
        let candidates = bundled_candidates(&app);
        assert_eq!(
            candidates[0].file_name().unwrap().to_string_lossy(),
            "ReadMD-Pet-Rust-linux-x86_64.zip",
            "the offered candidate must be the one whose hash differs"
        );
        assert_eq!(candidates.len(), 2);
        let report = check_pet_update(&app, false);
        assert_eq!(report["installed"], json!(true), "{report}");
        assert_eq!(report["source"], json!("none"), "{report}");
        assert_eq!(report["has_update"], json!(false));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn an_uninstalled_host_offers_the_first_candidate_even_at_the_installed_hash() {
        // E1, case 3: the `is_installed and installed_manifest_hash`
        // short-circuit.  With nothing installed the match scan never runs, so
        // the only test left is `cand_hash != installed_hash`.
        let app = test_app("we-e1-fresh");
        let root = app_dir(&app);
        pet_zip(&root.join("ReadMD-Pet-Rust.zip"), "{\"fresh\":1}");
        let report = check_pet_update(&app, false);
        assert_eq!(report["source"], json!("bundled"), "{report}");
        assert_eq!(report["has_update"], json!(true));
        assert_eq!(report["installed"], json!(false));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn legacy_cleanup_never_reaches_beyond_appdata() {
        // E6's hard constraint, pinned: `updater.py:43` reads %APPDATA% only,
        // so a host without it cleans nothing.  The XDG/home walk belongs to
        // `config._platform_data_dir`, not to this function.
        if std::env::var_os("APPDATA").is_some() {
            // Cannot observe the miss without clobbering the process-wide
            // environment, so the Windows branch is checked for shape instead.
            let target = std::env::temp_dir().join("readmd-we-legacy-absent");
            let cleaned = clean_legacy_pet_installations(&target);
            assert!(cleaned.is_empty() || cleaned.iter().all(|path| path.contains("ReadMD")));
            return;
        }
        let target = std::env::temp_dir().join("readmd-we-legacy-absent");
        assert!(clean_legacy_pet_installations(&target).is_empty());
    }

    // ---------------------------------------------------- defect #6: install lock

    /// `threading.RLock` parity: the same thread may re-enter.  A plain mutex
    /// would hang here, because `install_default_pet_plugin` calls
    /// `apply_pet_update` while it already holds the lock.
    #[test]
    fn install_lock_is_reentrant_on_one_thread() {
        let lock = InstallLock::default();
        let _outer = lock.acquire();
        let _inner = lock.acquire();
        let _innermost = lock.acquire();
        drop(_innermost);
        drop(_inner);
        drop(_outer);
    }

    /// Exclusion across threads: a second thread waits until the holder drops the
    /// last guard, so two concurrent updates cannot interleave their swaps.
    #[test]
    fn install_lock_excludes_another_thread_until_released() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let lock = Arc::new(InstallLock::default());
        let acquired = Arc::new(AtomicBool::new(false));
        let guard = lock.acquire();
        let lock2 = Arc::clone(&lock);
        let acquired2 = Arc::clone(&acquired);
        let child = std::thread::spawn(move || {
            let _held = lock2.acquire();
            acquired2.store(true, Ordering::SeqCst);
        });
        std::thread::sleep(std::time::Duration::from_millis(120));
        assert!(
            !acquired.load(Ordering::SeqCst),
            "the second thread must block while the lock is held"
        );
        drop(guard);
        child.join().expect("the waiting thread runs once the holder releases");
        assert!(acquired.load(Ordering::SeqCst));
    }
}

#[cfg(test)]
mod copy_tree_link_tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "readmd-copytree-{tag}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn touch(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "x").unwrap();
    }

    /// `cmd /c mklink /J`: a junction is creatable without elevation on this
    /// box, a directory symlink is not (measured `WinError 1314`).
    fn make_junction(link: &Path, target: &Path) -> bool {
        std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false)
    }

    /// Iterative census of a copied tree: `(files, deepest directory level)`.
    fn census(dir: &Path) -> (usize, usize) {
        let mut files = 0;
        let mut deepest = 0;
        let mut stack: Vec<(PathBuf, usize)> = vec![(dir.to_path_buf(), 0)];
        while let Some((current, depth)) = stack.pop() {
            deepest = deepest.max(depth);
            let read = match fs::read_dir(&current) {
                Ok(r) => r,
                Err(_) => continue,
            };
            for entry in read.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push((path, depth + 1));
                } else {
                    files += 1;
                }
            }
        }
        (files, deepest)
    }

    fn drop_links(links: &[&Path]) {
        for link in links {
            let _ = fs::remove_dir(link);
            let _ = fs::remove_file(link);
        }
    }

    #[test]
    fn copy_tree_refuses_a_link_loop_back_to_its_ancestor() {
        let root = scratch("cycle");
        let source = root.join("pet");
        touch(&source.join("pet.json"));
        touch(&source.join("sub").join("frame.png"));
        let loop_link = source.join("loop");
        if !make_junction(&loop_link, &source) {
            eprintln!("skipping: could not create a junction here");
            let _ = fs::remove_dir_all(&root);
            return;
        }
        // Before the fix this call recursed through `loop` into its own source,
        // writing thousands of duplicate copies and the same number of nested
        // directories until the path wall or the stack stopped it.
        let target = root.join("installed");
        copy_tree(&source, &target).expect("copy succeeds");
        let (files, deepest) = census(&target);
        assert_eq!(files, 2, "every real file exactly once: {files}");
        assert!(deepest <= 2, "the loop is pruned, not nested: {deepest}");
        assert!(target.join("pet.json").is_file());
        assert!(target.join("sub").join("frame.png").is_file());
        assert!(!target.join("loop").exists(), "a pruned cycle is not published");
        drop_links(&[&loop_link]);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn copy_tree_refuses_a_mutual_two_directory_cycle() {
        let root = scratch("mutual");
        let source = root.join("pet");
        let left = source.join("left");
        let right = source.join("right");
        touch(&left.join("l.txt"));
        touch(&right.join("r.txt"));
        let a_to_b = left.join("there");
        let b_to_a = right.join("back");
        if !make_junction(&a_to_b, &right) || !make_junction(&b_to_a, &left) {
            eprintln!("skipping: could not create the junction pair here");
            let _ = fs::remove_dir_all(&root);
            return;
        }
        let target = root.join("installed");
        copy_tree(&source, &target).expect("copy succeeds");
        let (files, _) = census(&target);
        // `left/there` is a fresh directory for the ancestor chain, so its
        // contents are copied once; the link back into `left` is what must not
        // be entered again.
        assert_eq!(files, 4, "each file reachable without looping: {files}");
        drop_links(&[&a_to_b, &b_to_a]);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn copy_tree_still_publishes_empty_directories_and_deep_packages() {
        let root = scratch("shape");
        let source = root.join("pet");
        fs::create_dir_all(source.join("frames").join("idle")).unwrap();
        let mut deep = source.join("pkg");
        for _ in 0..40 {
            deep = deep.join("p");
        }
        touch(&deep.join("runtime.exe"));
        let target = root.join("installed");
        copy_tree(&source, &target).expect("copy succeeds");
        assert!(target.join("frames").join("idle").is_dir(), "empty dir kept");
        assert!(target.join("pkg").exists());
        let (files, _) = census(&target);
        assert_eq!(files, 1);
        assert!(MAX_COPY_DEPTH >= 1_000, "cap stays at CPython's recursionlimit");
        let _ = fs::remove_dir_all(&root);
    }
}
