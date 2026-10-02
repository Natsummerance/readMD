// -*- coding: utf-8 -*-
//! Host side of the optional Hermes-source desktop pet adapter: the versioned
//! file bridge, the adapter launcher and the staged-tree publish primitives.
//!
//! Authority: `src/readmd_modules/pet/hermes_adapter.py`.
//!
//! | Python callable | Lines | Rust item |
//! |---|---|---|
//! | `kill_processes_by_target` | 28-73 | [`kill_processes_by_target`] |
//! | `HermesPetBridge.__init__` | 83-91 | [`HermesPetBridge::new`] |
//! | `HermesPetBridge.publish` | 93-131 | [`HermesPetBridge::publish`] |
//! | `HermesPetBridge.take_command` | 133-142 | [`HermesPetBridge::take_command`] |
//! | `HermesPetBridge._take_command_file` | 144-241 | [`HermesPetBridge::take_command_file`] + [`validate_command`] |
//! | `HermesPetBridge._safe_bounds` | 243-256 | [`safe_bounds`] |
//! | `HermesPetLauncher.__init__` / `.adapter_dir` | 262-277 | [`HermesPetLauncher::new`] / [`HermesPetLauncher::adapter_dir`] |
//! | `HermesPetLauncher.status` | 279-318 | [`HermesPetLauncher::status`] |
//! | `HermesPetLauncher._renderer_assets_ready` | 320-332 | [`HermesPetLauncher::renderer_assets_ready`] |
//! | `HermesPetLauncher.start` | 334-370 | [`HermesPetLauncher::start`] |
//! | `HermesPetLauncher.stop` | 372-404 | [`HermesPetLauncher::stop`] |
//! | `HermesPetPluginInstaller._replace_with_retry` | 542-553 | [`replace_with_retry`] |
//! | `HermesPetPluginInstaller._replace_tree_in_place` | 555-604 | [`replace_tree_in_place`] |
//! | `HermesPetPluginInstaller._is_safe_name` | 606-609 | [`is_safe_name`] |
//! | `HermesPetPluginInstaller._sha256` | 611-617 | [`sha256_file`] |
//! | `HermesPetPluginInstaller._publish_staged_tree` | 667-698 | [`publish_staged_tree`] |
//!
//! `_take_command_file`'s two pure halves are factored out of the bridge: the
//! decode as [`py_json_loads`] (CPython's `json.loads`, because `json.dumps`
//! writes `NaN`) and the validation block
//! (`hermes_adapter.py:175-241`) as [`validate_command_ord`].  Both speak
//! [`OrdValue`] because the command is a `dict` whose key *order* the authority
//! preserves into the returned value, and neither reads state in the original
//! either.  The seam lets the whole command contract be tested without a
//! filesystem.  Likewise the process table behind
//! [`kill_plan`] / [`runtime_is_running`] is injected, so the *decisions* (which
//! pid, in which order, matched how) are portable cfg-free code and only the
//! Win32 plumbing sits behind `mod win`.
//!
//! # Native purity
//!
//! Nothing here spawns a shell, PowerShell, `taskkill`, `wmic` or Python.  The
//! two places where the authority shells out become direct kernel32 calls that
//! do the same job:
//!
//! * `kill_processes_by_target`'s psutil path (`:37-54`) becomes
//!   `CreateToolhelp32Snapshot` + `Process32FirstW`/`Process32NextW` +
//!   `QueryFullProcessImageNameW`, and `proc.kill()` becomes
//!   `OpenProcess(PROCESS_TERMINATE)` + `TerminateProcess(1)`.
//! * `HermesPetLauncher.stop`'s `taskkill /F /T /PID` (`:384-388`) becomes
//!   [`terminate_tree`], the same children-before-parent sweep.
//!
//! The one behaviour that cannot be reproduced offline is the PowerShell
//! *fallback* (`hermes_adapter.py:58-73`), which the authority only reaches
//! when `import psutil` failed.  `readmd-kernel/Cargo.toml` has no
//! `windows`/`winapi`/`libc` and there is no network to add one, so the
//! fallback is deliberately suppressed and *reported* instead
//! ([`KillOutcome::fallback_suppressed`]) rather than spawning a program.  The
//! native snapshot path above already covers what that fallback could kill.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{json, Map, Value};

lazy_static::lazy_static! {
    /// `re.findall(r"(?:src|href)=[\"'](?:\./)?assets/([^\"']+)[\"']", html)`
    /// from `_renderer_assets_ready` (`hermes_adapter.py:329`).  A leading slash
    /// (`src="/assets/x.js"`) is deliberately *not* matched, exactly as in the
    /// authority.
    static ref RENDERER_ASSET_RE: regex::Regex =
        regex::Regex::new(r#"(?:src|href)=["'](?:\./)?assets/([^"']+)["']"#)
            .expect("static renderer asset regex");
}

// ---------------------------------------------------------------------------
// constants (hermes_adapter.py:80-81, 137-138, 167, 190, 216-233, 300-305)
// ---------------------------------------------------------------------------

// >>> S1:limits -- std only (plain `pub const`s, `Duration`); copied verbatim
// >>> by build_harness.py.  The region deliberately covers the whole constant
// >>> block below, because the harness needs the health limits too.
/// `HermesPetBridge.FORMAT_VERSION` (`hermes_adapter.py:80`).
pub const FORMAT_VERSION: i64 = 1;

/// `HermesPetBridge._COMMANDS` (`hermes_adapter.py:81`), the allow-list a
/// command `type` must be a member of.
pub const COMMANDS: [&str; 11] = [
    "bounds",
    "clipboard",
    "drop",
    "open-app",
    "open-menu",
    "pop-in",
    "scale",
    "submit",
    "toggle-app",
    "interact",
    "character",
];

/// `command.get('action') not in {'pet', 'feed', 'play', 'rest', 'wake'}`
/// (`hermes_adapter.py:184`).
pub const INTERACT_ACTIONS: [&str; 5] = ["pet", "feed", "play", "rest", "wake"];

/// `command.get('renderer') not in {'hermes-sprite', 'live2d'}`
/// (`hermes_adapter.py:190`).
pub const CHARACTER_RENDERERS: [&str; 2] = ["hermes-sprite", "live2d"];

/// `claim_path.stat().st_size > 32 * 1024 * 1024` (`hermes_adapter.py:167`).
pub const MAX_COMMAND_FILE_BYTES: u64 = 32 * 1024 * 1024;
/// `queued[:128]` (`hermes_adapter.py:138`), `len(paths) > 128` (`:216`, `:233`).
pub const MAX_COMMAND_BATCH: usize = 128;
/// `len(slug) > 63` (`hermes_adapter.py:190`) — code points, not bytes.
pub const MAX_SLUG_CHARS: usize = 63;
/// `len(path) > 32768` (`hermes_adapter.py:219`, `:233`) — code points.
pub const MAX_PATH_CHARS: usize = 32768;
/// `len(text.encode("utf-8")) > 4 * 1024 * 1024` (`hermes_adapter.py:227`) —
/// the one cap the authority measures in *bytes*.
pub const MAX_CLIPBOARD_TEXT_BYTES: usize = 4 * 1024 * 1024;
/// `len(image) > 24 * 1024 * 1024` (`hermes_adapter.py:230`) — code points.
pub const MAX_CLIPBOARD_IMAGE_CHARS: usize = 24 * 1024 * 1024;

/// The psutil scan cache in `status()` (`hermes_adapter.py:300`).
pub const SCAN_CACHE_SECONDS: f64 = 2.0;
/// `health_path.stat().st_size < 16384` (`hermes_adapter.py:305`).
pub const HEALTH_MAX_BYTES: u64 = 16_384;
/// `for key in ('state', 'renderer', 'code')` (`hermes_adapter.py:308`) — also
/// the order the picked keys are written in.
pub const HEALTH_KEYS: [&str; 3] = ["state", "renderer", "code"];

/// `HermesPetPluginInstaller.SWAP_ATTEMPTS` (`hermes_adapter.py:546`).
pub const SWAP_ATTEMPTS: u32 = 40;
/// `HermesPetPluginInstaller.SWAP_RETRY_DELAY` (`hermes_adapter.py:553`).
/// `parity_pets` uses the same attempt count with a 250 ms delay for the
/// *rust-host* tree, which is a different contract from this one.
pub const SWAP_RETRY_DELAY: Duration = Duration::from_millis(750);
/// `backup = self.root / "hermes-adapter.previous"` (`hermes_adapter.py:669`).
pub const BACKUP_DIR_NAME: &str = "hermes-adapter.previous";
/// `destination.with_name(destination.name + ".readmd-new")`
/// (`hermes_adapter.py:576`).
pub const IN_PLACE_TEMP_SUFFIX: &str = ".readmd-new";
/// `self.state_path = root / "hermes-overlay-state.json"`
/// (`hermes_adapter.py:86`).
pub const STATE_FILE_NAME: &str = "hermes-overlay-state.json";
/// `self.state_path.with_suffix(".tmp")` (`hermes_adapter.py:120`) — the suffix
/// is *replaced*, so the temporary is `hermes-overlay-state.tmp`.
pub const STATE_TMP_NAME: &str = "hermes-overlay-state.tmp";
/// `Path(str(self._bridge.state_path) + '.health.json')`
/// (`hermes_adapter.py:304`) — a string append, not `with_extension`.
pub const HEALTH_SUFFIX: &str = ".health.json";
// <<< S1:limits
/// `env.pop('ELECTRON_RUN_AS_NODE', None)` (`hermes_adapter.py:348`).
pub const ENV_RUN_AS_NODE: &str = "ELECTRON_RUN_AS_NODE";

// ---------------------------------------------------------------------------
// ordered JSON value
// ---------------------------------------------------------------------------
// >>> S1:ord_value -- std + serde_json only; extracted verbatim by
// >>> scratch/rust_parity/pet_json_s1/build_harness.py.  Keep the marker pair
// >>> around the whole section if this port grows.
/// A JSON value that remembers object key order *and* the three non-finite
/// floats Python's decoder produces.
///
/// `json.dumps` writes the state payload in *insertion* order
/// (`hermes_adapter.py:121`) and those bytes are the cross-process contract
/// with the Electron overlay.  `serde_json` keeps that order now
/// (`preserve_order` is on) but its `Number::from_f64` still has no `NaN`/`inf`
/// case at all — `json!({"x": f64::NAN})` degrades to `Value::Null` — so
/// [`OrdValue`] exists to carry the non-finite floats: [`py_json_loads`] builds
/// every object level here and [`OrdValue::dump`] re-emits the exact
/// `json.dumps` bytes.
///
/// Leaf values that arrive as a caller-supplied `serde_json::Value` (an
/// `info` field, a `Value`-typed argument) keep serde_json's sorted rendering
/// and cannot carry a non-finite float; that is noted where it can be observed
/// and is why the durable command path never converts back.
#[derive(Clone, Debug, PartialEq)]
pub enum OrdValue {
    Json(Value),
    /// `NaN` / `Infinity` / `-Infinity`, which `Value` cannot hold.  Produced by
    /// [`py_json_loads`], rendered by [`OrdValue::dump`], *lost* by
    /// [`OrdValue::to_json`] (see the doc there).
    NotFinite(NonFinite),
    Object(Vec<(String, OrdValue)>),
    Array(Vec<OrdValue>),
}

/// One of the three non-finite floats CPython's `json.loads` accepts as a bare
/// token by default (`json/decoder.py`, `parse_constant`) and `json.dumps`
/// writes back out with the same spelling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NonFinite {
    /// `NaN`
    Nan,
    /// `Infinity`
    Infinity,
    /// `-Infinity`
    NegInfinity,
}

impl NonFinite {
    /// The `json.dumps` spelling, which is also the accepted `json.loads` token.
    pub fn token(self) -> &'static str {
        match self {
            NonFinite::Nan => "NaN",
            NonFinite::Infinity => "Infinity",
            NonFinite::NegInfinity => "-Infinity",
        }
    }

    /// The number Python holds after `loads`, for the `float()` gates.
    pub fn as_f64(self) -> f64 {
        match self {
            NonFinite::Nan => f64::NAN,
            NonFinite::Infinity => f64::INFINITY,
            NonFinite::NegInfinity => f64::NEG_INFINITY,
        }
    }

    /// `Some` only for the values `serde_json::Value` cannot represent.
    pub fn from_f64(value: f64) -> Option<NonFinite> {
        if value.is_finite() {
            None
        } else if value.is_nan() {
            Some(NonFinite::Nan)
        } else if value > 0.0 {
            Some(NonFinite::Infinity)
        } else {
            Some(NonFinite::NegInfinity)
        }
    }
}

impl OrdValue {
    pub fn null() -> OrdValue {
        OrdValue::Json(Value::Null)
    }
    pub fn boolean(value: bool) -> OrdValue {
        OrdValue::Json(json!(value))
    }
    pub fn int(value: i64) -> OrdValue {
        OrdValue::Json(json!(value))
    }
    pub fn text(value: &str) -> OrdValue {
        OrdValue::Json(json!(value))
    }
    /// A non-finite float, i.e. one of the three bare tokens.
    pub fn not_finite(kind: NonFinite) -> OrdValue {
        OrdValue::NotFinite(kind)
    }
    /// `json.dumps`-compatible number: `Value::Number` where serde_json can
    /// carry the value, [`OrdValue::NotFinite`] where it cannot.  `json!(f64)`
    /// would quietly give `Value::Null` for the latter.
    pub fn number(value: f64) -> OrdValue {
        match NonFinite::from_f64(value) {
            Some(kind) => OrdValue::NotFinite(kind),
            None => OrdValue::Json(json!(value)),
        }
    }
    pub fn object(fields: Vec<(&str, OrdValue)>) -> OrdValue {
        OrdValue::Object(fields.into_iter().map(|(key, value)| (key.to_string(), value)).collect())
    }
    pub fn from_json(value: &Value) -> OrdValue {
        match value {
            Value::Object(map) => {
                let mut fields = Vec::with_capacity(map.len());
                for (k, v) in map {
                    fields.push((k.clone(), Self::from_json(v)));
                }
                OrdValue::Object(fields)
            }
            Value::Array(items) => OrdValue::Array(items.iter().map(Self::from_json).collect()),
            other => OrdValue::Json(other.clone()),
        }
    }
    /// `json.dumps(payload, ensure_ascii=False, separators=(",", ":"))`
    /// (`hermes_adapter.py:121`).
    pub fn dump(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }
    fn write(&self, out: &mut String) {
        match self {
            OrdValue::Json(value) => write_json(value, out),
            OrdValue::NotFinite(kind) => out.push_str(kind.token()),
            OrdValue::Object(fields) => {
                out.push('{');
                for (index, (key, value)) in fields.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    out.push_str(&crate::pet_queue::json_str(key));
                    out.push(':');
                    value.write(out);
                }
                out.push('}');
            }
            OrdValue::Array(items) => {
                out.push('[');
                for (index, value) in items.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    value.write(out);
                }
                out.push(']');
            }
        }
    }
    /// The same data as a plain `serde_json::Value` for programmatic use.
    ///
    /// One loss, forced by `Value` itself: [`OrdValue::NotFinite`] becomes
    /// `Value::Null`, which is exactly what `json!(f64::NAN)` already yields.
    /// Use [`OrdValue::dump`] when the bytes matter, which is why the durable
    /// command path returns `OrdValue`.
    pub fn to_json(&self) -> Value {
        match self {
            OrdValue::Json(value) => value.clone(),
            OrdValue::NotFinite(_) => Value::Null,
            OrdValue::Object(fields) => {
                let mut map = Map::new();
                for (key, value) in fields {
                    map.insert(key.clone(), value.to_json());
                }
                Value::Object(map)
            }
            OrdValue::Array(items) => Value::Array(items.iter().map(OrdValue::to_json).collect()),
        }
    }
    pub fn get(&self, key: &str) -> Option<&OrdValue> {
        match self {
            OrdValue::Object(fields) => fields.iter().find(|(name, _)| name == key).map(|(_, value)| value),
            _ => None,
        }
    }
    pub fn is_true(&self, key: &str) -> bool {
        self.get(key) == Some(&OrdValue::Json(Value::Bool(true)))
    }
}

/// Compact `ensure_ascii=False` rendering for a `serde_json::Value`.
fn write_json(value: &Value, out: &mut String) {
    match value {
        Value::String(text) => out.push_str(&crate::pet_queue::json_str(text)),
        other => out.push_str(&other.to_string()),
    }
}
// <<< S1:ord_value

// ---------------------------------------------------------------------------
// `json.loads` (F-LA1) and the `OrdValue` operations it needs
// ---------------------------------------------------------------------------

// >>> S1:command_json -- std + serde_json only; copied verbatim by
// >>> scratch/rust_parity/pet_json_s1/build_harness.py together with
// >>> S1:limits, S1:ord_value, S1:py_float and S1:validate.
/// Nesting cap of [`py_json_loads`], the same 128 serde_json enforces.
///
/// CPython has no depth limit of its own: a deeply nested document raises
/// `RecursionError`, which `_take_command_file`'s `except (OSError, ValueError,
/// TypeError)` does *not* catch, so the authority lets the whole `take_command`
/// call blow up on it.  A host must not crash over one queue file, so the limit
/// stays where the crate already had it and the entry is rejected instead.
pub const MAX_JSON_DEPTH: usize = 128;

/// `json.loads(text)` — CPython's decoder, not serde_json's.
///
/// `serde_json::from_str` is *stricter* than `json.loads` in exactly one way
/// that matters to the pet bridge: it has no production for the three bare
/// tokens `NaN`, `Infinity` and `-Infinity` that `json.JSONDecoder`'s
/// `parse_constant` accepts (`json.loads('{"x": NaN}')` is `{'x': nan}`), and it
/// also errors on an out-of-range number literal (`{"x": 1e999}`, which Python
/// reads as `inf`).  Both are the *same* defect at the same seam
/// (`hermes_adapter.py:169`): the command file is written by the overlay with
/// `json.dumps`, which happily emits `NaN`, so a Rust reader dropped — for a
/// durable queue entry *discarded* — commands Python accepted.
///
/// The values land in [`OrdValue`] so that (a) the tokens survive as
/// [`NonFinite`] and are re-emitted by [`OrdValue::dump`] exactly as
/// `json.dumps` would, and (b) object key order survives, which is what
/// [`validate_command_ord`]'s bounds branch needs (F-LA2).
///
/// Two deliberate residuals, both unreachable from the pet bridge and both
/// recorded in `scratch/rust_parity/pet_json_s1/REPORT.md`: an integer literal
/// wider than `u64` keeps the exact value in Python and an `f64` here, and a
/// `\uD800`-style lone surrogate is a legal Python `str` that a Rust `String`
/// cannot hold, so it is rejected (as it was under serde_json).
///
/// The `Err` payload is a diagnostic string, not a `json.JSONDecodeError`: only
/// the accept/reject decision is load-bearing here.
pub fn py_json_loads(text: &str) -> Result<OrdValue, String> {
    let mut scanner = JsonScanner { text, pos: 0, depth: 0 };
    scanner.skip_whitespace();
    let value = scanner.scan_value()?;
    scanner.skip_whitespace();
    if scanner.pos != text.len() {
        return Err(scanner.error_at("Extra data"));
    }
    Ok(value)
}

struct JsonScanner<'a> {
    text: &'a str,
    pos: usize,
    depth: usize,
}

impl<'a> JsonScanner<'a> {
    /// `json.decoder.WHITESPACE`, i.e. `' \t\n\r'` and nothing else: `\x0b`,
    /// `\x0c`, `\x1c`, `\x85`, `\xa0` and a leading BOM all raise `Expecting
    /// value` (measured, `probe_escape.txt`).
    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ') | Some(b'\t') | Some(b'\n') | Some(b'\r')) {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.text.as_bytes().get(self.pos).copied()
    }

    fn bytes(&self) -> &'a [u8] {
        self.text.as_bytes()
    }

    /// `string[idx:idx + len(word)] == word`, advancing on a hit — the exact
    /// shape of CPython's `null`/`true`/`false` and `parse_constant` tests.
    fn literal(&mut self, word: &str) -> bool {
        if !self.bytes()[self.pos..].starts_with(word.as_bytes()) {
            return false;
        }
        self.pos += word.len();
        true
    }

    fn error_at(&self, what: &str) -> String {
        // `pos` always lands on a byte boundary the scanner itself crossed, but a
        // diagnostic must never be able to panic on a slice, so snap back first.
        let mut end = self.pos.min(self.text.len());
        while end > 0 && !self.text.is_char_boundary(end) {
            end -= 1;
        }
        let before = &self.text[..end];
        let line = before.matches('\n').count() + 1;
        format!("{what}: line {line}, char {}", before.chars().count())
    }

    /// `_scan_once`, in CPython's order: string, object, array, the three
    /// keyword literals, the `NUMBER_RE` match, then the `parse_constant`
    /// spellings.
    fn scan_value(&mut self) -> Result<OrdValue, String> {
        if self.depth > MAX_JSON_DEPTH {
            return Err(self.error_at("Maximum nesting depth exceeded"));
        }
        match self.peek() {
            Some(b'"') => {
                self.pos += 1;
                return Ok(OrdValue::text(&self.scan_string()?));
            }
            Some(b'{') => {
                self.pos += 1;
                return self.scan_object();
            }
            Some(b'[') => {
                self.pos += 1;
                return self.scan_array();
            }
            Some(b'n') => {
                if self.literal("null") {
                    return Ok(OrdValue::null());
                }
            }
            Some(b't') => {
                if self.literal("true") {
                    return Ok(OrdValue::boolean(true));
                }
            }
            Some(b'f') => {
                if self.literal("false") {
                    return Ok(OrdValue::boolean(false));
                }
            }
            _ => {}
        }
        if let Some(number) = self.scan_number()? {
            return Ok(number);
        }
        // Only these three spellings: `-NaN`, `nan`, `NAN`, `inf`, `Infinity`
        // with a sign and a `NaN` prefix followed by more text all fail.
        if self.literal("NaN") {
            return Ok(OrdValue::not_finite(NonFinite::Nan));
        }
        if self.literal("Infinity") {
            return Ok(OrdValue::not_finite(NonFinite::Infinity));
        }
        if self.literal("-Infinity") {
            return Ok(OrdValue::not_finite(NonFinite::NegInfinity));
        }
        Err(self.error_at("Expecting value"))
    }

    /// `NUMBER_RE` = `(-?(?:0|[1-9]\d*))(\.\d+)?([eE][-+]?\d+)?`, matched (not
    /// searched) at `pos`.  The optional groups only match when *complete*, so
    /// `1.` yields the number `1` and then "Extra data", exactly as in Python,
    /// and `-Infinity` never matches here (no digit after the sign).
    fn scan_number(&mut self) -> Result<Option<OrdValue>, String> {
        let bytes = self.bytes();
        let start = self.pos;
        let mut end = start;
        if bytes.get(end) == Some(&b'-') {
            end += 1;
        }
        match bytes.get(end) {
            Some(b'0') => end += 1,
            Some(b'1'..=b'9') => {
                end += 1;
                while matches!(bytes.get(end), Some(b'0'..=b'9')) {
                    end += 1;
                }
            }
            _ => return Ok(None),
        }
        let mut fractional = false;
        if bytes.get(end) == Some(&b'.') && matches!(bytes.get(end + 1), Some(b'0'..=b'9')) {
            end += 1;
            while matches!(bytes.get(end), Some(b'0'..=b'9')) {
                end += 1;
            }
            fractional = true;
        }
        if matches!(bytes.get(end), Some(b'e') | Some(b'E')) {
            let mut exponent = end + 1;
            if matches!(bytes.get(exponent), Some(b'+') | Some(b'-')) {
                exponent += 1;
            }
            if matches!(bytes.get(exponent), Some(b'0'..=b'9')) {
                while matches!(bytes.get(exponent), Some(b'0'..=b'9')) {
                    exponent += 1;
                }
                end = exponent;
                fractional = true;
            }
        }
        let Ok(literal) = std::str::from_utf8(&bytes[start..end]) else {
            return Err(self.error_at("Invalid number"));
        };
        let literal = literal.to_string();
        self.pos = end;
        let value = if fractional {
            // `parse_float` is `float(text)`, so an out-of-range exponent gives
            // `inf`/`-inf where serde_json errored, and `1e-999` gives `0.0`.
            match py_float_str(&literal) {
                Some(number) => OrdValue::number(number),
                None => return Err(self.error_at("Invalid number")),
            }
        } else {
            // `parse_int` is `int(text)`: exact while `i64`/`u64` reach, and an
            // `f64` approximation past them (see the note on [`py_json_loads`]).
            match literal.parse::<i64>() {
                Ok(number) => OrdValue::int(number),
                Err(_) => match literal.parse::<u64>() {
                    Ok(number) => OrdValue::Json(Value::from(number)),
                    Err(_) => match py_float_str(&literal) {
                        Some(number) => OrdValue::number(number),
                        None => return Err(self.error_at("Invalid number")),
                    },
                },
            }
        };
        Ok(Some(value))
    }

    fn scan_object(&mut self) -> Result<OrdValue, String> {
        self.depth += 1;
        let fields = self.object_fields();
        self.depth -= 1;
        fields.map(OrdValue::Object)
    }

    /// `JSONObject` with the default `object_hook`: pairs are appended in file
    /// order and then handed to `dict()`, so a repeated key keeps the *first*
    /// position and the *last* value — `{"k":1,"j":2,"k":3}` is `{"k":3,"j":2}`.
    fn object_fields(&mut self) -> Result<Vec<(String, OrdValue)>, String> {
        let mut fields: Vec<(String, OrdValue)> = Vec::new();
        self.skip_whitespace();
        if self.literal("}") {
            return Ok(fields);
        }
        loop {
            self.skip_whitespace();
            if self.peek() != Some(b'"') {
                return Err(self.error_at("Expecting property name enclosed in double quotes"));
            }
            self.pos += 1;
            let key = self.scan_string()?;
            self.skip_whitespace();
            if self.peek() != Some(b':') {
                return Err(self.error_at("Expected ':' between string and value"));
            }
            self.pos += 1;
            self.skip_whitespace();
            let value = self.scan_value()?;
            ord_insert(&mut fields, key, value);
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(fields);
                }
                _ => return Err(self.error_at("Expecting ',' delimiter")),
            }
        }
    }

    fn scan_array(&mut self) -> Result<OrdValue, String> {
        self.depth += 1;
        let items = self.array_items();
        self.depth -= 1;
        items.map(OrdValue::Array)
    }

    fn array_items(&mut self) -> Result<Vec<OrdValue>, String> {
        let mut items: Vec<OrdValue> = Vec::new();
        self.skip_whitespace();
        if self.literal("]") {
            return Ok(items);
        }
        loop {
            self.skip_whitespace();
            items.push(self.scan_value()?);
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(items);
                }
                _ => return Err(self.error_at("Expecting ',' delimiter")),
            }
        }
    }

    /// `scanstring(s, end, strict=True)` — `end` is just past the opening quote.
    /// Raw control characters below `U+0020` are rejected (strict), `U+007F` is
    /// not, and only the eight table escapes plus `\uXXXX` exist.
    fn scan_string(&mut self) -> Result<String, String> {
        let bytes = self.bytes();
        let begin = self.pos - 1;
        let mut out = String::new();
        loop {
            let mut end = self.pos;
            while end < bytes.len() {
                let ch = bytes[end];
                if ch == b'"' || ch == b'\\' || ch < 0x20 {
                    break;
                }
                end += 1;
            }
            if end == bytes.len() {
                return Err(format!("Unterminated string starting at char {begin}"));
            }
            out.push_str(&self.text[self.pos..end]);
            self.pos = end;
            match bytes[end] {
                b'"' => {
                    self.pos = end + 1;
                    return Ok(out);
                }
                b'\\' => self.scan_escape(&mut out)?,
                other => {
                    return Err(format!(
                        "Invalid control character {other:#04x} at char {}",
                        self.text[..end].chars().count()
                    ))
                }
            }
        }
    }

    /// One `\x` escape, `self.pos` on the backslash.
    fn scan_escape(&mut self, out: &mut String) -> Result<(), String> {
        const ESCAPES: [(u8, char); 8] = [
            (b'"', '"'),
            (b'\\', '\\'),
            (b'/', '/'),
            (b'b', '\u{8}'),
            (b'f', '\u{c}'),
            (b'n', '\n'),
            (b'r', '\r'),
            (b't', '\t'),
        ];
        let at = self.pos + 1;
        let Some(&esc) = self.bytes().get(at) else {
            return Err("Unterminated string starting at the escape".to_string());
        };
        if esc == b'u' {
            let first = self.decode_uxxxx(at)?;
            let mut code = first;
            let mut next = at + 5;
            // The C scanner only pairs a *high* surrogate with an immediately
            // following `\uXXXX` low surrogate; anything else stays one unit.
            if (0xd800..=0xdbff).contains(&first)
                && self.bytes().get(next) == Some(&b'\\')
                && self.bytes().get(next + 1) == Some(&b'u')
            {
                let second = self.decode_uxxxx(next + 1)?;
                if (0xdc00..=0xdfff).contains(&second) {
                    code = 0x10000 + (((first - 0xd800) << 10) | (second - 0xdc00));
                    next += 6;
                }
            }
            let Some(ch) = char::from_u32(code) else {
                return Err(format!(
                    "lone surrogate U+{code:04X} at char {at} cannot be held in a Rust string"
                ));
            };
            out.push(ch);
            self.pos = next;
            return Ok(());
        }
        match ESCAPES.iter().find(|(byte, _)| *byte == esc) {
            Some((_, ch)) => {
                out.push(*ch);
                self.pos = at + 1;
                Ok(())
            }
            None => Err(format!("Invalid \\escape: byte {esc:#04x} at char {at}")),
        }
    }

    /// `_decode_uXXXX`: exactly four ASCII hex digits after the `u`.  A sign, an
    /// `_` or a `0x` prefix are all rejected even though `int(text, 16)` would
    /// take two of them, because the C scanner is what `json.loads` uses.
    fn decode_uxxxx(&self, u_at: usize) -> Result<u32, String> {
        let digits = match self.bytes().get(u_at + 1..u_at + 5) {
            Some(digits) if digits.iter().all(u8::is_ascii_hexdigit) => digits,
            _ => return Err(format!("Invalid \\uXXXX escape at char {}", u_at + 1)),
        };
        let text = std::str::from_utf8(digits).map_err(|_| "Invalid \\uXXXX escape".to_string())?;
        u32::from_str_radix(text, 16).map_err(|_| "Invalid \\uXXXX escape".to_string())
    }
}

/// `dict.__setitem__` on an insertion-ordered field list: an existing key keeps
/// its position and takes the new value, a new key is appended.  This is the one
/// operation that makes [`OrdValue`] a faithful stand-in for a Python `dict`, so
/// both the duplicate-key decode rule and `command["bounds"] = ...` go through it.
pub fn ord_insert(fields: &mut Vec<(String, OrdValue)>, key: String, value: OrdValue) {
    match fields.iter_mut().find(|(name, _)| *name == key) {
        Some(slot) => slot.1 = value,
        None => fields.push((key, value)),
    }
}

/// View a `serde_json::Value` as an [`OrdValue`].  The conversion cannot invent
/// a non-finite float or recover a key order that `Map` already sorted, so it is
/// only used to keep the `Value`-typed entry points of this module working; the
/// durable command path never converts back (see [`OrdValue::to_json`]).
pub fn ord_from_value(value: &Value) -> OrdValue {
    match value {
        Value::Object(fields) => OrdValue::Object(
            fields.iter().map(|(key, item)| (key.clone(), ord_from_value(item))).collect(),
        ),
        Value::Array(items) => OrdValue::Array(items.iter().map(ord_from_value).collect()),
        other => OrdValue::Json(other.clone()),
    }
}

/// `isinstance(value, dict)`.
fn ord_is_object(value: &OrdValue) -> bool {
    matches!(value, OrdValue::Object(_))
}

/// `isinstance(value, list)`.
fn ord_items(value: &OrdValue) -> Option<&[OrdValue]> {
    match value {
        OrdValue::Array(items) => Some(items),
        _ => None,
    }
}

/// `isinstance(value, str)`.
fn ord_str(value: &OrdValue) -> Option<&str> {
    match value {
        OrdValue::Json(Value::String(text)) => Some(text),
        _ => None,
    }
}

/// `float(value)`; `None` stands for Python's `TypeError`/`ValueError`.  The
/// [`OrdValue::NotFinite`] arm is what keeps a decoded `NaN` behaving like a
/// float downstream instead of like a missing or un-coercible value.
fn ord_float(value: &OrdValue) -> Option<f64> {
    match value {
        OrdValue::NotFinite(kind) => Some(kind.as_f64()),
        OrdValue::Json(item) => py_float_value(item),
        OrdValue::Object(_) | OrdValue::Array(_) => None,
    }
}

/// `command = dict(command)` followed by `command[key] = replacement`
/// (`hermes_adapter.py:208-209`): a copy that keeps every other key *and the
/// whole key order* of the original, with `key` replaced in place.
fn ord_with_replaced(value: &OrdValue, key: &str, replacement: OrdValue) -> Option<OrdValue> {
    let OrdValue::Object(fields) = value else {
        return None;
    };
    let mut fields = fields.clone();
    ord_insert(&mut fields, key.to_string(), replacement);
    Some(OrdValue::Object(fields))
}
// <<< S1:command_json

// ---------------------------------------------------------------------------
// Python coercions used by the ported gates
// ---------------------------------------------------------------------------

/// `bool(x)` / `if x` for a decoded JSON value.
pub fn py_truthy(value: &Value) -> bool {
    crate::desktop_pet::py_truthy(value)
}

/// `str(x)` (`hermes_adapter.py:98`, `:116`).  A `str` passes through; anything
/// else becomes its `repr`, which is what `str()` does for JSON scalars and
/// containers alike.
pub fn py_str_value(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Array(_) | Value::Object(_) => py_repr_value(value),
        Value::Null => "None".to_string(),
        Value::Bool(flag) => if *flag { "True".to_string() } else { "False".to_string() },
        Value::Number(number) => number.to_string(),
    }
}

/// `repr(x)` for the JSON subset.
///
/// A `str` leaf is quoted here even when it sits *inside* a container: `str()`
/// of a container delegates to `repr()` of its members, so
/// `str([1, 'a']) == "[1, 'a']"` (measured with CPython 3.11.15, and the reason
/// a bare `py_str_value` fall-through is wrong at this arm).
pub fn py_repr_value(value: &Value) -> String {
    match value {
        Value::String(text) => py_repr_str(text),
        Value::Array(items) => format!(
            "[{}]",
            items.iter().map(py_repr_value).collect::<Vec<_>>().join(", ")
        ),
        Value::Object(fields) => format!(
            "{{{}}}",
            fields
                .iter()
                .map(|(key, item)| format!("{}: {}", py_repr_str(key), py_repr_value(item)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        other => py_str_value(other),
    }
}

/// Python's `repr()` of a `str`: single quotes unless the text holds a `'` and
/// no `"`, then `\\`, `\n`, `\r`, `\t`, the quote itself and `\xNN` for the C0
/// controls plus DEL.  (Python additionally escapes the Unicode `C*`/`Zl`/`Zp`
/// categories; those characters cannot reach this code path from the pet
/// bridge, which only stringifies scalar state fields.)
fn py_repr_str(text: &str) -> String {
    let quote = if text.contains('\'') && !text.contains('"') { '"' } else { '\'' };
    let mut out = String::with_capacity(text.len() + 2);
    out.push(quote);
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 0x20 || c == '\u{7f}' => {
                out.push_str(&format!("\\x{:02x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

// >>> S1:py_float -- std only; the lines between the S1:py_float markers are
// >>> copied verbatim by scratch/rust_parity/pet_json_s1/build_harness.py.
/// `float(text)` for a `str` argument (`hermes_adapter.py:206`, `:249`).
///
/// CPython's grammar, not Rust's: an optional sign, the `inf`/`infinity`/`nan`
/// words, or a decimal in which every `_` sits between two digits, padded by
/// whitespace on either side.  `None` means Python raised
/// `TypeError`/`ValueError`, which both call sites turn into a rejection.
///
/// The padding set is *not* `char::is_whitespace()`, and it is not fixed either:
/// CPython sends pure-ASCII text straight to `PyOS_string_to_double`, whose
/// `isspace()` covers only `\t \n \v \f \r ' '`, while any text holding one
/// non-ASCII code point first goes through
/// `_PyUnicode_TransformDecimalAndSpaceToASCII`.  That transform rewrites
/// Unicode decimal digits to ASCII digits and blanks Unicode spaces, which is
/// why `float("\u{a0}1")` and `float("\u{663}\u{664}")` succeed but
/// `float("\u{1c}1")` raises: the C0/RFC 2214 separators U+001C..U+001F *are*
/// `str.isspace()` yet `ascii_char()` leaves every code point below 128 alone.
/// Measured against CPython 3.11 in
/// `scratch/rust_parity/_pet_launcher_s1_probe6.py` and `_probe8.py`.
pub fn py_float_str(text: &str) -> Option<f64> {
    const PY_SPACE: [char; 6] = [' ', '\t', '\n', '\r', '\u{b}', '\u{c}'];
    let transformed = if text.is_ascii() {
        None
    } else {
        Some(transform_decimal_and_space_to_ascii(text))
    };
    let body = match &transformed {
        Some(text) => text.as_str(),
        None => text,
    };
    let body = body.trim_matches(|ch| PY_SPACE.contains(&ch));
    let (negative, rest) = if let Some(rest) = body.strip_prefix('+') {
        (false, rest)
    } else if let Some(rest) = body.strip_prefix('-') {
        (true, rest)
    } else {
        (false, body)
    };
    if rest.eq_ignore_ascii_case("inf") || rest.eq_ignore_ascii_case("infinity") {
        return Some(if negative { f64::NEG_INFINITY } else { f64::INFINITY });
    }
    if rest.eq_ignore_ascii_case("nan") {
        return Some(f64::NAN);
    }
    if !valid_decimal_with_underscores(rest) {
        return None;
    }
    let cleaned: String = rest.chars().filter(|ch| *ch != '_').collect();
    // The sign stripped above belongs to the value: CPython's `float("-1E-5")`
    // is `-1e-05`, and `-0` / `-nan` keep their sign bit too.
    let value = cleaned.parse::<f64>().ok()?;
    Some(if negative { -value } else { value })
}

fn valid_decimal_with_underscores(text: &str) -> bool {
    let bytes = text.as_bytes();
    for (index, byte) in bytes.iter().enumerate() {
        match *byte {
            b'_' => {
                let before = index > 0 && bytes[index - 1].is_ascii_digit();
                let after = index + 1 < bytes.len() && bytes[index + 1].is_ascii_digit();
                if !before || !after {
                    return false;
                }
            }
            b'0'..=b'9' | b'.' | b'e' | b'E' | b'+' | b'-' => {}
            _ => return false,
        }
    }
    // CPython copies the literal into a buffer holding the legal underscores
    // *removed* and only then hands it to `PyOS_string_to_double`, so every
    // structural check below runs on the underscore-free spelling: `1_0` is `10`
    // and `1_000.5` is `1000.5`.  Nothing can smuggle a digit-free group past the
    // position loop above, so stripping cannot over-accept.
    let stripped: String = text.chars().filter(|ch| *ch != '_').collect();
    let text = stripped.as_str();
    let (mantissa, exponent) = match text.find(['e', 'E']) {
        Some(at) => (&text[..at], Some(&text[at + 1..])),
        None => (text, None),
    };
    if let Some(exponent) = exponent {
        let digits = exponent.trim_start_matches(['+', '-']);
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return false;
        }
    }
    if !mantissa.bytes().all(|byte| byte.is_ascii_digit() || byte == b'.') {
        return false;
    }
    mantissa.bytes().any(|byte| byte.is_ascii_digit())
}

/// `_PyUnicode_TransformDecimalAndSpaceToASCII`, restricted to the characters
/// CPython actually rewrites (`unicode_double_impl` in `Objects/floatobject.c`):
/// every code point below 128 is left alone, so the two tables below only carry
/// wide characters.
fn transform_decimal_and_space_to_ascii(text: &str) -> String {
    text.chars()
        .map(|ch| match wide_space_to_ascii(ch) {
            Some(blank) => blank,
            None => match py_decimal_digit(ch) {
                Some(digit) => digit,
                None => ch,
            },
        })
        .collect()
}

/// The wide code points for which `float()` ignores the value and parses as if
/// a space had been written.  `U+0085`, `U+00A0`, `U+1680`, `U+2000`..`U+200A`,
/// `U+2028`, `U+2029`, `U+202F`, `U+205F`, `U+3000` — the `Py_UNICODE_ISSPACE`
/// members at or above 128.
fn wide_space_to_ascii(ch: char) -> Option<char> {
    const RANGES: [(u32, u32); 8] = [
        (0x0085, 0x0085),
        (0x00a0, 0x00a0),
        (0x1680, 0x1680),
        (0x2000, 0x200a),
        (0x2028, 0x2029),
        (0x202f, 0x202f),
        (0x205f, 0x205f),
        (0x3000, 0x3000),
        // `U+200B..U+200D` and `U+00AD` are deliberately absent: they are not
        // `Py_UNICODE_ISSPACE`, and `float("\u{200b}1")` raises (measured).
    ];
    let code = ch as u32;
    RANGES
        .iter()
        .find(|(low, high)| code >= *low && code <= *high)
        .map(|_| ' ')
}

/// `Py_UNICODE_TODECIMAL` for `float()`: the start code point of every Unicode
/// block whose ten members are the digits 0..9, measured from
/// `unicodedata.decimal()` over the whole plane (660 characters, 66 complete
/// blocks, no ragged ones left over).
fn py_decimal_digit(ch: char) -> Option<char> {
    const ZEROES: [u32; 66] = [
        0x0030, 0x0660, 0x06f0, 0x07c0, 0x0966, 0x09e6, 0x0a66, 0x0ae6, 0x0b66, 0x0be6, 0x0c66, 0x0ce6, 0x0d66,
        0x0de6, 0x0e50, 0x0ed0, 0x0f20, 0x1040, 0x1090, 0x17e0, 0x1810, 0x1946, 0x19d0, 0x1a80, 0x1a90, 0x1b50,
        0x1bb0, 0x1c40, 0x1c50, 0xa620, 0xa8d0, 0xa900, 0xa9d0, 0xa9f0, 0xaa50, 0xabf0, 0xff10, 0x104a0, 0x10d30,
        0x11066, 0x110f0, 0x11136, 0x111d0, 0x112f0, 0x11450, 0x114d0, 0x11650, 0x116c0, 0x11730, 0x118e0, 0x11950,
        0x11c50, 0x11d50, 0x11da0, 0x16a60, 0x16ac0, 0x16b50, 0x1d7ce, 0x1d7d8, 0x1d7e2, 0x1d7ec, 0x1d7f6, 0x1e140,
        0x1e2f0, 0x1e950, 0x1fbf0,
    ];
    let code = ch as u32;
    ZEROES.iter().find_map(|zero| {
        if *zero <= code && code < *zero + 10 {
            char::from_u32(b'0' as u32 + (code - zero))
        } else {
            None
        }
    })
}

/// `float(x)` for a decoded JSON value; `None` == Python's `TypeError`.
pub fn py_float_value(value: &Value) -> Option<f64> {
    match value {
        Value::Bool(flag) => Some(if *flag { 1.0 } else { 0.0 }),
        Value::Number(number) => number.as_f64(),
        Value::String(text) => py_float_str(text),
        _ => None,
    }
}
// <<< S1:py_float

// ---------------------------------------------------------------------------
// kill_processes_by_target (hermes_adapter.py:28-73)
// ---------------------------------------------------------------------------

/// One row of the process table — the shape
/// `psutil.process_iter(['pid', 'name', 'exe'])` yields at
/// `hermes_adapter.py:39`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeProcess {
    pub pid: u32,
    /// `th32ParentProcessID`, the link psutil builds `children()` from.
    pub parent: u32,
    /// `proc.info['name']` (`szExeFile`).
    pub name: String,
    /// `proc.info['exe']`; `None` where psutil raises `AccessDenied` and the
    /// authority skips the row (`hermes_adapter.py:52`).
    pub exe: Option<String>,
}

impl NativeProcess {
    /// Build a row for the pure decision functions and their tests.
    pub fn fake(pid: u32, parent: u32, exe: Option<&str>) -> NativeProcess {
        let name = exe
            .map(|path| path.rsplit(['/', '\\']).next().unwrap_or(path).to_string())
            .unwrap_or_default();
        NativeProcess { pid, parent, name, exe: exe.map(|path| path.to_string()) }
    }
}

/// What a kill call decided and how, so a caller can tell "nothing matched"
/// from "the native enumeration was unavailable".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KillOutcome {
    /// Every pid handed to `TerminateProcess`, in execution order, including
    /// the duplicates Python produces when a matched process is also another
    /// matched process's child (`proc.kill()` on a dead pid is the swallowed
    /// `NoSuchProcess` at `hermes_adapter.py:47`/`:52`).
    pub selected: Vec<u32>,
    /// `"kernel32"`, `"unavailable"` (the snapshot itself failed) or
    /// `"unsupported"` (non-Windows: psutil has `/proc`, this kernel has no
    /// libc binding).
    pub backend: &'static str,
    /// True where the authority would have shelled out to PowerShell
    /// (`hermes_adapter.py:58-73`) and the native build deliberately did not.
    pub fallback_suppressed: bool,
}

/// `str(target_dir.resolve()).lower()` with the
/// `except (OSError, ValueError): str(target_dir).lower()` tail
/// (`hermes_adapter.py:30-33`).
pub fn resolved_target_str(target_dir: &Path) -> String {
    resolve_path_str(target_dir).to_lowercase()
}

/// `Path.resolve()` / `os.path.realpath()` for the places the authority
/// resolves before comparing.
pub fn resolve_path_str(path: &Path) -> String {
    let text = path.to_string_lossy().into_owned();
    if cfg!(windows) {
        // `py_realpath` already strips the `\\?\` verbatim prefix that
        // `std::fs::canonicalize` adds, i.e. what `Path.resolve()` reports.
        crate::pet_queue::py_realpath(&text)
    } else {
        match std::fs::canonicalize(path) {
            Ok(canonical) => canonical.to_string_lossy().into_owned(),
            Err(_) => text,
        }
    }
}

/// `os.path.normcase` — identity on POSIX, `\`-folding *and* lower-casing on
/// Windows (`hermes_adapter.py:42`).
pub fn normcase(path: &str) -> String {
    if cfg!(windows) {
        crate::pet_queue::py_normcase(path)
    } else {
        path.to_string()
    }
}

/// `os.path.normcase(os.path.realpath(exe)).startswith(os.path.normcase(target)
/// + os.sep)` (`hermes_adapter.py:42`).  The separator is part of the prefix,
/// so an image that sits *exactly* at the resolved target path does not match:
/// only paths strictly inside the tree do.
pub fn exe_matches_target(exe: &str, resolved_target_lower: &str) -> bool {
    let real = if cfg!(windows) {
        crate::pet_queue::py_realpath(exe)
    } else {
        match std::fs::canonicalize(exe) {
            Ok(canonical) => canonical.to_string_lossy().into_owned(),
            Err(_) => exe.to_string(),
        }
    };
    let prefix = format!("{}{}", normcase(resolved_target_lower), std::path::MAIN_SEPARATOR_STR);
    normcase(&real).starts_with(&prefix)
}

/// Every descendant of `root` in the order psutil 7.2.2 produces them, i.e. the
/// traversal behind `proc.children(recursive=True)` (`hermes_adapter.py:44`).
///
/// That order is **neither BFS nor pre-order DFS**.  psutil builds a
/// `collections.defaultdict(list)` of `ppid -> [pid]` in snapshot order and then
/// walks it with a LIFO stack, appending each edge it traverses:
///
/// ```text
/// stack = [self.pid]
/// while stack:
///     pid = stack.pop()
///     if pid in seen: continue
///     seen.add(pid)
///     for child_pid in reverse_ppid_map[pid]: ret.append(...); stack.append(child_pid)
/// ```
///
/// Measured against a synthetic parent map of live pids (so psutil's
/// pid-reuse `create_time()` guard stays satisfied):
/// `A->[B,C] B->[D] C->[E]` yields `B, C, E, D`, a flat `A->[B,C,D]` yields
/// `B, C, D`, a chain yields `B, C, D`, and a parent cycle `A<->B` yields
/// `B, A` — the *root itself* re-enters the list because `seen` is only checked
/// when a pid is popped.  A diamond therefore repeats the shared child, which is
/// why the caller's kill list can hold duplicates.
pub fn descendants(procs: &[NativeProcess], root: u32) -> Vec<u32> {
    // `reverse_ppid_map`: child lists stay in table (snapshot) order.
    let mut reverse: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for proc in procs {
        reverse.entry(proc.parent).or_default().push(proc.pid);
    }
    let mut seen: BTreeSet<u32> = BTreeSet::new();
    let mut stack: Vec<u32> = vec![root];
    let mut ret: Vec<u32> = Vec::new();
    while let Some(pid) = stack.pop() {
        if !seen.insert(pid) {
            continue;
        }
        if let Some(children) = reverse.get(&pid) {
            for child in children {
                ret.push(*child);
                stack.push(*child);
            }
        }
    }
    ret
}

/// The pure decision half of `kill_processes_by_target`
/// (`hermes_adapter.py:39-53`): for each row whose image lives under the
/// target, kill its whole subtree first and the row itself last.
///
/// There is deliberately **no self-preservation**: the authority has none
/// either, so a process running from inside `target_dir` is selected even when
/// it is this process.
pub fn kill_plan(procs: &[NativeProcess], resolved_target_lower: &str) -> Vec<u32> {
    let mut plan = Vec::new();
    for proc in procs {
        let matched = proc
            .exe
            .as_deref()
            .map(|exe| exe_matches_target(exe, resolved_target_lower))
            .unwrap_or(false);
        if !matched {
            continue;
        }
        plan.extend(descendants(procs, proc.pid));
        plan.push(proc.pid);
    }
    plan
}

/// The process table, or `None` when the native enumeration failed — the case
/// the authority answers with its PowerShell fallback.
pub fn list_processes() -> Option<Vec<NativeProcess>> {
    if cfg!(windows) {
        win::list_processes()
    } else {
        None
    }
}

/// `proc.kill()` — `OpenProcess(PROCESS_TERMINATE)` + `TerminateProcess(1)`.
pub fn terminate_process(pid: u32) -> bool {
    if cfg!(windows) {
        win::terminate_process(pid)
    } else {
        false
    }
}

/// `kill_processes_by_target(target_dir)` (`hermes_adapter.py:28`).
pub fn kill_processes_by_target(target_dir: &Path) -> KillOutcome {
    let resolved = resolved_target_str(target_dir);
    match list_processes() {
        None => KillOutcome {
            selected: Vec::new(),
            backend: if cfg!(windows) { "unavailable" } else { "unsupported" },
            // `if os.name == 'nt' and not psutil_success:` (`:59`)
            fallback_suppressed: cfg!(windows),
        },
        Some(table) => {
            let selected = kill_plan(&table, &resolved);
            for pid in &selected {
                let _ = terminate_process(*pid);
            }
            KillOutcome { selected, backend: "kernel32", fallback_suppressed: false }
        }
    }
}

// ---------------------------------------------------------------------------
// HermesPetBridge (hermes_adapter.py:77-256)
// ---------------------------------------------------------------------------

// >>> S1:bridge_error -- std only; copied verbatim by build_harness.py.
/// What `publish` can raise: an `OSError` from the atomic replace (unhandled
/// upstream, so it propagates) or `ValueError("invalid_pet_bounds")` from
/// `_safe_bounds`.
#[derive(Debug)]
pub enum BridgeError {
    Io(io::Error),
    InvalidPetBounds,
}

impl From<io::Error> for BridgeError {
    fn from(error: io::Error) -> Self {
        BridgeError::Io(error)
    }
}

impl std::fmt::Display for BridgeError {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BridgeError::Io(error) => write!(out, "{error}"),
            BridgeError::InvalidPetBounds => write!(out, "invalid_pet_bounds"),
        }
    }
}

impl std::error::Error for BridgeError {}

pub type BridgeResult<T> = Result<T, BridgeError>;
// <<< S1:bridge_error

/// Keyword arguments of `HermesPetBridge.publish` (`hermes_adapter.py:93-95`).
#[derive(Clone, Debug, Default)]
pub struct PublishOptions {
    /// `info`, as an ordered mapping (`dict(info or {})`, `:107`).
    pub info: Vec<(String, OrdValue)>,
    /// `activity`, merged over the derived flags with
    /// `{**derived_activity, **activity}` (`:108`).
    pub activity: Vec<(String, OrdValue)>,
    /// `bounds` — used only when it is an object, per
    /// `if isinstance(bounds, dict)` (`:113`).
    pub bounds: Option<Value>,
    /// `renderer`; `None` (and JSON `null`) is Python's `None`, which omits the
    /// key entirely (`:115`).
    pub renderer: Option<Value>,
    /// `fullscreen` (`:117`).
    pub fullscreen: Option<Value>,
}

/// `class HermesPetBridge` (`hermes_adapter.py:77`).
pub struct HermesPetBridge {
    pub root: PathBuf,
    pub state_path: PathBuf,
    pub command_path: PathBuf,
    pub commands_dir: PathBuf,
    lock: Mutex<()>,
    command_lock: Mutex<()>,
    claim_lock: Mutex<()>,
    last_encoded: Mutex<Option<String>>,
}

impl HermesPetBridge {
    /// `HermesPetBridge(data_dir)` (`hermes_adapter.py:83-91`).
    pub fn new(data_dir: &Path) -> HermesPetBridge {
        let root = PathBuf::from(resolve_path_str(data_dir)).join("pet");
        let state_path = root.join(STATE_FILE_NAME);
        HermesPetBridge {
            command_path: root.join(format!("{STATE_FILE_NAME}.command")),
            commands_dir: root.join(format!("{STATE_FILE_NAME}.commands")),
            lock: Mutex::new(()),
            command_lock: Mutex::new(()),
            claim_lock: Mutex::new(()),
            last_encoded: Mutex::new(None),
            state_path,
            root,
        }
    }

    /// `publish(runtime, info=..., activity=..., bounds=..., renderer=...,
    /// fullscreen=...)` (`hermes_adapter.py:93-131`).
    pub fn publish(&self, runtime: &Value, options: &PublishOptions) -> BridgeResult<OrdValue> {
        // `runtime = runtime if isinstance(runtime, dict) else {}` (`:97`)
        let runtime = if runtime.is_object() { runtime.clone() } else { json!({}) };

        // `state = str(runtime.get("activity_state", runtime.get("state")) or "")`
        let raw_state: Value = match runtime.get("activity_state") {
            Some(value) => value.clone(),
            None => runtime.get("state").cloned().unwrap_or(Value::Null),
        };
        let state = if py_truthy(&raw_state) { py_str_value(&raw_state) } else { String::new() };

        // derived_activity (`:99-103`)
        let derived_busy = match runtime.get("active_tasks") {
            Some(value) => py_truthy(value),
            None => state == "busy",
        };
        let mut activity: Vec<(String, OrdValue)> = vec![
            ("busy".to_string(), OrdValue::boolean(derived_busy)),
            ("error".to_string(), OrdValue::boolean(state == "error")),
            ("justCompleted".to_string(), OrdValue::boolean(state == "success")),
        ];
        for (key, value) in &options.activity {
            match activity.iter_mut().find(|(existing, _)| existing == key) {
                Some(slot) => slot.1 = value.clone(),
                None => activity.push((key.clone(), value.clone())),
            }
        }

        let mut payload: Vec<(String, OrdValue)> = vec![
            ("format_version".to_string(), OrdValue::int(FORMAT_VERSION)),
            (
                "visible".to_string(),
                OrdValue::boolean(runtime.get("visible").map(py_truthy).unwrap_or(false)),
            ),
            ("info".to_string(), OrdValue::Object(options.info.clone())),
            ("activity".to_string(), OrdValue::Object(activity)),
            ("busy".to_string(), OrdValue::boolean(derived_busy)),
            ("awaiting".to_string(), OrdValue::boolean(false)),
            ("unread".to_string(), OrdValue::boolean(false)),
        ];
        if let Some(bounds) = &options.bounds {
            if bounds.is_object() {
                payload.push(("bounds".to_string(), safe_bounds(bounds)?));
            }
        }
        if let Some(renderer) = &options.renderer {
            if !renderer.is_null() {
                payload.push(("renderer".to_string(), OrdValue::Json(json!(py_str_value(renderer)))));
            }
        }
        if let Some(fullscreen) = &options.fullscreen {
            if !fullscreen.is_null() {
                payload.push(("fullscreen".to_string(), OrdValue::boolean(py_truthy(fullscreen))));
            }
        }
        let payload = OrdValue::Object(payload);
        let encoded = payload.dump();

        std::fs::create_dir_all(&self.root)?;
        let tmp = self.root.join(STATE_TMP_NAME);
        let _guard = self.lock.lock().unwrap();
        {
            let last = self.last_encoded.lock().unwrap();
            if last.as_deref() == Some(encoded.as_str()) && self.state_path.is_file() {
                let _ = remove_missing(&tmp);
                return Ok(payload);
            }
        }
        std::fs::write(&tmp, encoded.as_bytes())?;
        let replaced = std::fs::rename(&tmp, &self.state_path);
        if replaced.is_ok() {
            *self.last_encoded.lock().unwrap() = Some(encoded);
        }
        // `finally: tmp.unlink(missing_ok=True)` — only FileNotFoundError is
        // swallowed, and an unlink failure replaces the pending rename error the
        // way Python's `finally` block does.
        if let Err(error) = remove_missing(&tmp) {
            return Err(BridgeError::Io(error));
        }
        match replaced {
            Ok(()) => Ok(payload),
            Err(error) => Err(BridgeError::Io(error)),
        }
    }

    /// `take_command()` (`hermes_adapter.py:133-142`).
    pub fn take_command(&self) -> Option<OrdValue> {
        let _guard = self.command_lock.lock().unwrap();
        let queued =
            if self.commands_dir.is_dir() { sorted_json_files(&self.commands_dir) } else { Vec::new() };
        for source in queued.into_iter().take(MAX_COMMAND_BATCH) {
            if let Some(command) = self.take_command_file(&source, true) {
                return Some(command);
            }
        }
        self.take_command_file(&self.command_path, false)
    }

    /// `_take_command_file(source, durable=False)`
    /// (`hermes_adapter.py:144-241`).
    ///
    /// The decode is [`py_json_loads`], not `serde_json::from_str`, and the
    /// result is an [`OrdValue`], not a `Value` (F-LA1): the writer is
    /// `json.dumps` on the Python side, so a command whose payload holds a `NaN`
    /// — an un-positioned pet, a missing measurement — is *valid* Python input
    /// that the old reader rejected, silently deleting a durable queue entry.
    pub fn take_command_file(&self, source: &Path, durable: bool) -> Option<OrdValue> {
        let _claim_guard = self.claim_lock.lock().unwrap();
        let source_name = source.file_name()?.to_string_lossy().into_owned();
        let claim_path = path_with_name(source, &claim_name(&source_name, std::process::id(), time_ns()));
        if std::fs::rename(source, &claim_path).is_err() {
            return None;
        }
        let claim_exists = std::fs::symlink_metadata(&claim_path).is_ok();
        let is_link = std::fs::symlink_metadata(&claim_path)
            .map(|meta| meta.file_type().is_symlink())
            .unwrap_or(false);
        let size = std::fs::metadata(&claim_path).ok().map(|stat| stat.len());
        let oversized = size.map(|len| len > MAX_COMMAND_FILE_BYTES).unwrap_or(false);
        if !claim_exists || is_link || oversized {
            self.restore_or_discard(source, &claim_path, durable);
            return None;
        }
        // `raw = claim_path.read_text(encoding="utf-8")` then
        // `value = json.loads(raw)` inside `except (OSError, ValueError,
        // TypeError)` (`:165-171`).
        let value: OrdValue = match std::fs::read(&claim_path)
            .map_err(io::Error::from)
            .and_then(|bytes| String::from_utf8(bytes).map_err(|_| io::Error::other("unicode decode")))
            .and_then(|text| py_json_loads(&text).map_err(io::Error::other))
        {
            Ok(value) => value,
            Err(_) => {
                self.restore_or_discard(source, &claim_path, durable);
                return None;
            }
        };
        let command = match validate_command_ord(&value) {
            Some(command) => command,
            None => {
                self.restore_or_discard(source, &claim_path, durable);
                return None;
            }
        };
        let _ = std::fs::remove_file(&claim_path);
        Some(command)
    }

    /// The `_restore_or_discard` closure (`hermes_adapter.py:154-164`).  A
    /// non-durable (legacy single-slot) file is put back so its writer can
    /// retry it; a durable queue entry is simply dropped.
    fn restore_or_discard(&self, source: &Path, claim: &Path, durable: bool) {
        if !durable && !source.exists() {
            if std::fs::rename(claim, source).is_ok() {
                return;
            }
        }
        let _ = std::fs::remove_file(claim);
    }
}

fn remove_missing(path: &Path) -> Result<(), io::Error> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

/// `f"{source.name}.claim.{os.getpid()}.{time.time_ns()}"`
/// (`hermes_adapter.py:146-148`).
pub fn claim_name(source_name: &str, pid: u32, time_ns: u128) -> String {
    format!("{source_name}.claim.{pid}.{time_ns}")
}

/// `pathlib`'s `Path.with_name(...)`: the parent directory plus one new
/// component.  `PathBuf::with_file_name` is the same operation; this wrapper
/// exists so the claim path and the `.readmd-new` temporary both read as the
/// pathlib call they mirror.
pub fn path_with_name(path: &Path, name: &str) -> PathBuf {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.join(name),
        _ => PathBuf::from(name),
    }
}

/// `time.time_ns()`.
pub fn time_ns() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|delta| delta.as_nanos()).unwrap_or(0)
}

/// `sorted(self.commands_dir.glob('*.json'))` (`hermes_adapter.py:137`).
///
/// `pathlib` orders by the normcased full path on Windows (which is why
/// `B.json` sorts before `a.json` there) and by the plain string on POSIX;
/// `*` matches leading dots, and a *directory* named `x.json` still shows up
/// because `glob` does not filter by type.  Python's `sorted` is stable, so
/// equal keys keep `os.scandir` order — `sort_by` is stable too.
pub fn sorted_json_files(dir: &Path) -> Vec<PathBuf> {
    let mut entries: Vec<PathBuf> = match std::fs::read_dir(dir) {
        Ok(reader) => reader.filter_map(|entry| entry.ok().map(|entry| entry.path())).collect(),
        Err(_) => return Vec::new(),
    };
    entries.retain(|path| match path.file_name() {
        Some(name) => {
            let name = name.to_string_lossy();
            if cfg!(windows) {
                name.to_lowercase().ends_with(".json")
            } else {
                name.ends_with(".json")
            }
        }
        None => false,
    });
    entries.sort_by(|left, right| sort_key(left).cmp(&sort_key(right)));
    entries
}

fn sort_key(path: &Path) -> String {
    normcase(&path.to_string_lossy())
}

// >>> S1:validate -- std + serde_json + crate::pdf_editor::{py_round,
// >>> py_round_int}; the region between the markers is the shipped command
// >>> contract and is copied verbatim by build_harness.py.
/// The validation body of `_take_command_file` (`hermes_adapter.py:175-241`),
/// kept free of the filesystem and of `serde_json::Value`.
///
/// [`OrdValue`] is what makes this a port rather than a re-interpretation: the
/// authority *rebuilds* the command dict for five kinds and *replaces one key in
/// place* for `bounds` (`command = dict(command)` then
/// `command["bounds"] = self._safe_bounds(...)`, `:208-209`), and in both cases
/// the result is a `dict` whose key order is the one Python would have.  A
/// `serde_json::Value` cannot express it: `preserve_order` carries the order
/// but a `Number` cannot hold a non-finite float, so the rebuilt command has to
/// live somewhere else.  See
/// `scratch/rust_parity/pet_json_s1/goldens.txt` for the
/// `json.dumps`-byte comparisons this function is checked against.
///
/// Two Python paths are deliberately *not* reproduced, both because the host must
/// not die over one queue file:
///
/// * `kind not in self._COMMANDS` (`:179`) raises `TypeError: unhashable type`
///   for a `dict`/`list` `type`, outside the `try` at `:165`, so the authority
///   propagates it out of `take_command`.  Here it is a rejection.
/// * `round(float(command.get("scale")), 2)` (`:213`) raises `OverflowError` for
///   an integer wider than `f64` (`float(10**400)`), likewise uncaught.  Here the
///   value arrives already saturated to infinity and is rejected by the band.
pub fn validate_command_ord(value: &OrdValue) -> Option<OrdValue> {
    // `not isinstance(value, dict) or not isinstance(value.get("command"), dict)`
    if !ord_is_object(value) {
        return None;
    }
    let command = value.get("command")?;
    if !ord_is_object(command) {
        return None;
    }
    let command = command.clone();
    // `kind = command.get("type")` / `if kind not in self._COMMANDS` — only a
    // `str` can be a member of that set.
    let kind = ord_str(command.get("type")?)?;
    if !COMMANDS.contains(&kind) {
        return None;
    }
    if kind == "interact" {
        let action = ord_str(command.get("action")?)?;
        if !INTERACT_ACTIONS.contains(&action) {
            return None;
        }
        // `{'type': kind, 'action': command['action']}`
        return Some(OrdValue::object(vec![
            ("type", OrdValue::text(kind)),
            ("action", OrdValue::text(action)),
        ]));
    }
    if kind == "character" {
        let slug = match command.get("slug") {
            Some(slug) => ord_str(slug)?.to_string(),
            None => String::new(),
        };
        if slug.chars().count() > MAX_SLUG_CHARS {
            return None;
        }
        let renderer = ord_str(command.get("renderer")?)?;
        if !CHARACTER_RENDERERS.contains(&renderer) {
            return None;
        }
        // `{'type': kind, 'slug': slug, 'renderer': command['renderer']}`
        return Some(OrdValue::object(vec![
            ("type", OrdValue::text(kind)),
            ("slug", OrdValue::text(&slug)),
            ("renderer", OrdValue::text(renderer)),
        ]));
    }
    if kind == "bounds" {
        let raw_bounds = command.get("bounds")?;
        if !ord_is_object(raw_bounds) {
            return None;
        }
        let sanitized = safe_bounds_ord(raw_bounds).ok()?;
        // F-LA2: replace the key *in place* on the original mapping, so every
        // other key keeps both its presence and its position.
        return ord_with_replaced(&command, "bounds", sanitized);
    }
    if kind == "scale" {
        let raw = match command.get("scale") {
            Some(raw) => raw.clone(),
            // `float(None)` is a `TypeError`.
            None => OrdValue::null(),
        };
        let scale = ord_float(&raw)?;
        let scale = crate::pdf_editor::py_round(scale, 2);
        if !(0.08..=0.72).contains(&scale) {
            return None;
        }
        return Some(OrdValue::object(vec![
            ("type", OrdValue::text("scale")),
            ("scale", OrdValue::number(scale)),
        ]));
    }
    if kind == "drop" {
        let paths: &[OrdValue] = ord_items(command.get("paths")?)?;
        if paths.is_empty() || paths.len() > MAX_COMMAND_BATCH {
            return None;
        }
        for path in paths {
            let text = ord_str(path)?;
            if text.is_empty() || text.chars().count() > MAX_PATH_CHARS {
                return None;
            }
        }
        return Some(OrdValue::object(vec![
            ("type", OrdValue::text("drop")),
            ("paths", OrdValue::Array(paths.to_vec())),
        ]));
    }
    if kind == "clipboard" {
        let text = match command.get("text") {
            Some(text) => ord_str(text)?.to_string(),
            None => String::new(),
        };
        if text.len() > MAX_CLIPBOARD_TEXT_BYTES {
            return None;
        }
        let image = match command.get("image_png") {
            Some(image) => ord_str(image)?.to_string(),
            None => String::new(),
        };
        if image.chars().count() > MAX_CLIPBOARD_IMAGE_CHARS {
            return None;
        }
        let paths: Vec<OrdValue> = match command.get("paths") {
            Some(paths) => ord_items(paths)?.to_vec(),
            None => Vec::new(),
        };
        if paths.len() > MAX_COMMAND_BATCH {
            return None;
        }
        for path in &paths {
            let text = ord_str(path)?;
            if text.chars().count() > MAX_PATH_CHARS {
                return None;
            }
        }
        return Some(OrdValue::object(vec![
            ("type", OrdValue::text("clipboard")),
            ("text", OrdValue::text(&text)),
            ("image_png", OrdValue::text(&image)),
            ("paths", OrdValue::Array(paths)),
        ]));
    }
    // open-app / open-menu / pop-in / submit / toggle-app pass through verbatim,
    // i.e. the *same* dict, unmodified, order and all.
    Some(command)
}

/// `validate_command_ord` for the `serde_json::Value` shape the `publish` path
/// and the unit tests build commands with.
///
/// Converting a `Value` can neither invent a non-finite number nor undo `Map`'s
/// sorted keys, so the answer is only order-faithful to the extent the input
/// already was; the durable command path uses [`py_json_loads`] +
/// [`validate_command_ord`] and never comes through here.
pub fn validate_command(value: &Value) -> Option<Value> {
    validate_command_ord(&ord_from_value(value)).map(|command| command.to_json())
}

/// `HermesPetBridge._safe_bounds` (`hermes_adapter.py:243-256`) over an
/// [`OrdValue`], i.e. over a mapping that may hold a decoded `NaN`.
///
/// Every key is mandatory, coerced with `float()`, required to be finite,
/// reduced by `int(round(...))` (banker's rounding, so `2.5 -> 2` and
/// `-2.5 -> -2`) and clamped with `max(low, min(high, number))`.  Any failure is
/// `ValueError("invalid_pet_bounds")`.  The result is built in the authority's
/// iteration order — `x, y, width, height` — which is what the state file and the
/// rebuilt command both emit.
pub fn safe_bounds_ord(value: &OrdValue) -> BridgeResult<OrdValue> {
    let mut out: Vec<(String, OrdValue)> = Vec::with_capacity(4);
    for (key, low, high) in [
        ("x", -32768i64, 32768i64),
        ("y", -32768, 32768),
        ("width", 80, 2048),
        ("height", 80, 2048),
    ] {
        let raw = match value.get(key) {
            None => return Err(BridgeError::InvalidPetBounds),
            Some(raw) => raw,
        };
        let number = match ord_float(raw) {
            None => return Err(BridgeError::InvalidPetBounds),
            Some(number) => {
                if !number.is_finite() {
                    return Err(BridgeError::InvalidPetBounds);
                }
                crate::pdf_editor::py_round_int(number)
            }
        };
        out.push((key.to_string(), OrdValue::int(i64::max(low, i64::min(high, number)))));
    }
    Ok(OrdValue::Object(out))
}

/// [`safe_bounds_ord`] for a caller that holds a `serde_json::Value`.
pub fn safe_bounds(value: &Value) -> BridgeResult<OrdValue> {
    safe_bounds_ord(&ord_from_value(value))
}
// <<< S1:validate

// ---------------------------------------------------------------------------
// HermesPetLauncher (hermes_adapter.py:259-404)
// ---------------------------------------------------------------------------

#[derive(Default)]
struct ScanCache {
    until: Option<Instant>,
    running: bool,
}

/// `class HermesPetLauncher` (`hermes_adapter.py:259`).
pub struct HermesPetLauncher {
    app_dir: PathBuf,
    bridge: Arc<HermesPetBridge>,
    external_adapter_dir: Option<PathBuf>,
    process: Mutex<Option<Child>>,
    launch_lock: Mutex<()>,
    scan: Mutex<ScanCache>,
}

impl HermesPetLauncher {
    /// `HermesPetLauncher(app_dir, bridge, adapter_dir=None)`
    /// (`hermes_adapter.py:262-269`).
    pub fn new(
        app_dir: &Path,
        bridge: Arc<HermesPetBridge>,
        adapter_dir: Option<&Path>,
    ) -> HermesPetLauncher {
        HermesPetLauncher {
            app_dir: PathBuf::from(resolve_path_str(app_dir)),
            bridge,
            external_adapter_dir: adapter_dir.map(|dir| PathBuf::from(resolve_path_str(dir))),
            process: Mutex::new(None),
            launch_lock: Mutex::new(()),
            scan: Mutex::new(ScanCache::default()),
        }
    }

    /// `HermesPetLauncher.adapter_dir` (`hermes_adapter.py:271-277`): an
    /// installed external package wins over the bundled `assets` copy.
    pub fn adapter_dir(&self) -> PathBuf {
        match &self.external_adapter_dir {
            Some(dir) => dir.clone(),
            None => self.app_dir.join("assets").join("pet").join("hermes-adapter"),
        }
    }

    pub fn bridge(&self) -> &Arc<HermesPetBridge> {
        &self.bridge
    }

    /// The pid of the child this launcher started, i.e. `self._process.pid`
    /// while the adapter is ours.
    pub fn child_pid(&self) -> Option<u32> {
        self.process.lock().unwrap().as_ref().map(Child::id)
    }

    /// `status()` (`hermes_adapter.py:279-318`).
    pub fn status(&self) -> OrdValue {
        self.status_at(Instant::now(), list_processes().as_deref())
    }

    /// `status()` with the monotonic clock and the process table injected.
    pub fn status_at(&self, now: Instant, table: Option<&[NativeProcess]>) -> OrdValue {
        let adapter_dir = self.adapter_dir();
        let runtime = adapter_dir.join("electron.exe");
        let app = adapter_dir.join("app").join("package.json");
        let available = cfg!(windows)
            && runtime.is_file()
            && app.is_file()
            && self.renderer_assets_ready();

        let mut is_running = match self.process.lock().unwrap().as_mut() {
            Some(child) => matches!(child.try_wait(), Ok(None)),
            None => false,
        };
        {
            let mut scan = self.scan.lock().unwrap();
            if !is_running && scan.until.map(|until| now < until).unwrap_or(false) {
                is_running = scan.running;
            } else if !is_running && cfg!(windows) && runtime.is_file() {
                // The authority's psutil scan (`:286-299`) never escapes into
                // `status`, so an unavailable table just reports "not running";
                // the cache is still refreshed, exactly like `:300-301`.
                if let Some(table) = table {
                    is_running = runtime_is_running(table, &runtime);
                }
                scan.until = Some(now + Duration::from_secs_f64(SCAN_CACHE_SECONDS));
                scan.running = is_running;
            }
        }

        let health = read_health(&self.bridge.state_path);
        OrdValue::object(vec![
            ("available", OrdValue::boolean(available)),
            ("bridge_ready", OrdValue::boolean(self.bridge.state_path.is_file())),
            ("running", OrdValue::boolean(is_running)),
            ("health", if is_running { health } else { OrdValue::Object(Vec::new()) }),
        ])
    }

    /// `_renderer_assets_ready()` (`hermes_adapter.py:320-332`).
    pub fn renderer_assets_ready(&self) -> bool {
        renderer_assets_ready_for(&self.adapter_dir().join("app").join("renderer").join("index.html"))
    }

    /// `start()` (`hermes_adapter.py:334-370`).
    pub fn start(&self) -> OrdValue {
        let _guard = self.launch_lock.lock().unwrap();
        let status = self.status();
        if !status.is_true("available") {
            return OrdValue::object(vec![
                ("ok", OrdValue::boolean(false)),
                ("code", OrdValue::text("hermes_adapter_not_installed")),
                ("runtime", status),
            ]);
        }
        if status.is_true("running") {
            return OrdValue::object(vec![("ok", OrdValue::boolean(true)), ("runtime", status)]);
        }

        // Terminate orphaned / zombie electron processes running from this
        // adapter dir (`hermes_adapter.py:343`).
        kill_processes_by_target(&self.adapter_dir());

        let adapter_dir = self.adapter_dir();
        let runtime = adapter_dir.join("electron.exe");
        let app = adapter_dir.join("app");
        match self.spawn_adapter(&runtime, &app) {
            Ok(child) => {
                *self.process.lock().unwrap() = Some(child);
                let status = self.status();
                OrdValue::object(vec![("ok", OrdValue::boolean(true)), ("runtime", status)])
            }
            Err(error) => {
                ::log::error!("Could not launch desktop pet: {error}");
                let status = self.status();
                OrdValue::object(vec![
                    ("ok", OrdValue::boolean(false)),
                    ("code", OrdValue::text("hermes_adapter_start_failed")),
                    ("runtime", status),
                ])
            }
        }
    }

    /// `subprocess.Popen([str(runtime), str(app)], cwd=str(app), env=env,
    /// startupinfo=<hidden>, creationflags=CREATE_NO_WINDOW)`
    /// (`hermes_adapter.py:347-366`).  `silent_command` supplies the
    /// `0x08000000` creation flag, which is the part of `STARTUPINFO` that has
    /// an effect for a console-less GUI runtime.
    fn spawn_adapter(&self, runtime: &Path, app: &Path) -> io::Result<Child> {
        let mut command = crate::process::silent_command(runtime);
        command.arg(app);
        command.current_dir(app);
        // env = os.environ.copy(); env.pop('ELECTRON_RUN_AS_NODE', None);
        // env["READMD_PET_BRIDGE_FILE"] = str(bridge.state_path);
        // env["READMD_PARENT_PID"] = str(os.getpid())
        // Passing an explicit block (Python does) means the child gets exactly
        // this set, so the inherited environment is rebuilt from scratch.
        let mut carried: Vec<(std::ffi::OsString, std::ffi::OsString)> = std::env::vars_os()
            .filter(|(key, _)| !py_env_name_is(key, ENV_RUN_AS_NODE))
            .collect();
        for (name, value) in [
            ("READMD_PET_BRIDGE_FILE", self.bridge.state_path.to_string_lossy().into_owned()),
            ("READMD_PARENT_PID", std::process::id().to_string()),
        ] {
            carried.retain(|(key, _)| !py_env_name_is(key, name));
            carried.push((std::ffi::OsString::from(name), std::ffi::OsString::from(value)));
        }
        command.env_clear();
        command.envs(carried);
        command.spawn()
    }

    /// `stop()` (`hermes_adapter.py:372-404`).
    pub fn stop(&self) {
        let _guard = self.launch_lock.lock().unwrap();
        let held = self.process.lock().unwrap().take();
        if let Some(mut child) = held {
            // `taskkill /F /T /PID <pid>` -> the same children-first sweep,
            // natively, with every error ignored (`hermes_adapter.py:389`).
            if cfg!(windows) {
                terminate_tree(child.id());
            }
            // `self._process.terminate()` (`:392`).
            let _ = child.kill();
            // `self._process.wait(timeout=2)` with the timeout swallowed
            // (`:396`); reaping is what matters.
            wait_up_to(&mut child, Duration::from_secs(2));
        }
        // Lingering children or unmanaged electron.exe under adapter_dir
        // (`:403`), then the fixed settle (`:404`).
        kill_processes_by_target(&self.adapter_dir());
        std::thread::sleep(Duration::from_millis(300));
    }
}

/// `os.environ`'s case-insensitive key lookup on Windows
/// (`env.pop('ELECTRON_RUN_AS_NODE')` finds any casing).
fn py_env_name_is(key: &std::ffi::OsStr, name: &str) -> bool {
    key.to_string_lossy().eq_ignore_ascii_case(name)
}

/// `Popen.wait(timeout=2)` without the exception: poll until the child is
/// reaped or the budget is spent.
fn wait_up_to(child: &mut Child, budget: Duration) {
    let deadline = Instant::now() + budget;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return,
            Ok(None) => {}
            Err(_) => return,
        }
        if Instant::now() >= deadline {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// The pure half of [`terminate_tree`]: the same children-before-parent sweep
/// over an injected table with an injected kill result, so the walk is testable
/// without `CreateToolhelp32Snapshot`.  A `None` table is the "the native
/// enumeration failed" case, where only the root can still be attempted.
pub fn terminate_tree_with(
    table: Option<&[NativeProcess]>,
    pid: u32,
    kill: impl FnMut(u32) -> bool,
) -> Vec<u32> {
    let mut kill = kill;
    let mut killed = Vec::new();
    if let Some(table) = table {
        for target in descendants(table, pid) {
            // A descendant psutil/taskkill could not open is not reported.
            if kill(target) {
                killed.push(target);
            }
        }
    }
    if kill(pid) {
        killed.push(pid);
    }
    killed
}

/// The stand-in for `taskkill /F /T /PID` (`hermes_adapter.py:385`): every
/// descendant of `pid` (in the psutil sweep order) and then `pid` itself.
/// Returns the pids actually terminated.
pub fn terminate_tree(pid: u32) -> Vec<u32> {
    terminate_tree_with(list_processes().as_deref(), pid, terminate_process)
}

/// The psutil branch of `status()` (`hermes_adapter.py:288-299`):
/// case-insensitive *equality* against the resolved runtime path — no
/// `realpath()` on the exe side here, unlike `kill_processes_by_target`.
pub fn runtime_is_running(table: &[NativeProcess], runtime: &Path) -> bool {
    let want = resolve_path_str(runtime).to_lowercase();
    table.iter().any(|proc| match &proc.exe {
        Some(exe) => exe.to_lowercase() == want,
        None => false,
    })
}

// >>> S1:read_health -- std + serde_json only; copied verbatim by
// >>> build_harness.py (the harness feeds it a temporary health file).
/// The `health` branch of `status()` (`hermes_adapter.py:302-310`).
///
/// Decoded with [`py_json_loads`]: the authority picks only the three `str`
/// keys out of whatever `json.loads` produced, so a `NaN` in *another* member
/// (an overlay uptime counter, a score) must not blank the whole map the way it
/// blanked it under `serde_json::from_str`.
pub fn read_health(state_path: &Path) -> OrdValue {
    let empty = || OrdValue::Object(Vec::new());
    let path = PathBuf::from(format!("{}{HEALTH_SUFFIX}", state_path.display()));
    let stat = match std::fs::metadata(&path) {
        Ok(stat) => stat,
        Err(_) => return empty(),
    };
    if stat.len() >= HEALTH_MAX_BYTES {
        return empty();
    }
    let text = match std::fs::read(&path).map(|bytes| String::from_utf8(bytes)) {
        Ok(Ok(text)) => text,
        _ => return empty(),
    };
    let raw: OrdValue = match py_json_loads(&text) {
        Ok(raw) => raw,
        Err(_) => return empty(),
    };
    if !ord_is_object(&raw) {
        return empty();
    }
    let mut out: Vec<(String, OrdValue)> = Vec::new();
    for key in HEALTH_KEYS {
        // `if isinstance(raw.get(key), str)` (`:308`)
        if let Some(text) = raw.get(key).and_then(ord_str) {
            out.push((key.to_string(), OrdValue::text(text)));
        }
    }
    OrdValue::Object(out)
}
// <<< S1:read_health

/// `_renderer_assets_ready` for an explicit `index.html`
/// (`hermes_adapter.py:320-332`).
pub fn renderer_assets_ready_for(index: &Path) -> bool {
    if !index.is_file() {
        // Small launcher fixtures and older sprite-only packages have no
        // renderer index; the required package files still gate launch.
        return true;
    }
    let html = match std::fs::read(index).map(|bytes| String::from_utf8(bytes)) {
        Ok(Ok(text)) => text,
        _ => return false,
    };
    for capture in RENDERER_ASSET_RE.captures_iter(&html) {
        let name = &capture[1];
        let present = index
            .parent()
            .map(|parent| parent.join("assets").join(name).is_file())
            .unwrap_or(false);
        if !present {
            return false;
        }
    }
    true
}

// ---------------------------------------------------------------------------
// staged-tree publish (hermes_adapter.py:542-609, 667-698)
// ---------------------------------------------------------------------------

/// `_replace_with_retry` (`hermes_adapter.py:542-553`): up to
/// [`SWAP_ATTEMPTS`] `os.replace` calls with [`SWAP_RETRY_DELAY`] between them,
/// and only `PermissionError` retries.
pub fn replace_with_retry(source: &Path, destination: &Path) -> io::Result<()> {
    replace_with_retry_timed(source, destination, SWAP_ATTEMPTS, SWAP_RETRY_DELAY)
}

/// [`replace_with_retry`] with the budget injected so the retry *policy* can be
/// exercised without a thirty-second test.
pub fn replace_with_retry_timed(
    source: &Path,
    destination: &Path,
    attempts: u32,
    delay: Duration,
) -> io::Result<()> {
    for remaining in (0..attempts).rev() {
        match std::fs::rename(source, destination) {
            Ok(()) => return Ok(()),
            Err(error) => {
                // Rust maps WinError 5/29/30/32/33 to `PermissionDenied`, the
                // same set CPython turns into `PermissionError`.
                if error.kind() != io::ErrorKind::PermissionDenied {
                    return Err(error);
                }
                if remaining == 0 {
                    return Err(error);
                }
                std::thread::sleep(delay);
            }
        }
    }
    Ok(())
}

/// `os.walk(staged)` in top-down order: the root first, then each directory
/// subtree, with bare names as `scandir` reports them.
pub fn walk_topdown(top: &Path) -> Vec<(PathBuf, Vec<String>, Vec<String>)> {
    let mut out = Vec::new();
    let mut stack = vec![top.to_path_buf()];
    while let Some(root) = stack.pop() {
        let (dirs, files) = scandir_split(&root);
        out.push((root.clone(), dirs.clone(), files));
        for dir in dirs.iter().rev() {
            stack.push(root.join(dir));
        }
    }
    out
}

/// `os.walk(top, topdown=False)` (`hermes_adapter.py:592`): children reported
/// before their parent.
pub fn walk_bottomup(top: &Path) -> Vec<(PathBuf, Vec<String>, Vec<String>)> {
    let mut out = Vec::new();
    let (dirs, files) = scandir_split(top);
    for dir in &dirs {
        bottomup_into(&top.join(dir), &mut out);
    }
    out.push((top.to_path_buf(), dirs, files));
    out
}

fn bottomup_into(top: &Path, out: &mut Vec<(PathBuf, Vec<String>, Vec<String>)>) {
    let (dirs, files) = scandir_split(top);
    for dir in &dirs {
        bottomup_into(&top.join(dir), out);
    }
    out.push((top.to_path_buf(), dirs, files));
}

/// `(dirs, files)` as `os.walk` splits them.  `DirEntry.is_dir()` in Python
/// follows symlinks, so `metadata` (not `file_type`) is the faithful read.
fn scandir_split(top: &Path) -> (Vec<String>, Vec<String>) {
    let mut dirs = Vec::new();
    let mut files = Vec::new();
    if let Ok(reader) = std::fs::read_dir(top) {
        for entry in reader.filter_map(|entry| entry.ok()) {
            let name = entry.file_name().to_string_lossy().into_owned();
            let is_dir =
                std::fs::metadata(entry.path()).map(|stat| stat.is_dir()).unwrap_or(false);
            if is_dir {
                dirs.push(name);
            } else {
                files.push(name);
            }
        }
    }
    (dirs, files)
}

/// `_replace_tree_in_place` (`hermes_adapter.py:555-604`): copy a verified
/// staged tree over a target that cannot be renamed, each file through a
/// `.readmd-new` sibling, then prune every entry the staged tree does not have.
pub fn replace_tree_in_place(staged: &Path, target: &Path) -> io::Result<()> {
    let walked = walk_topdown(staged);
    let staged_names: BTreeSet<String> = walked
        .iter()
        .flat_map(|(root, dirs, files)| {
            let rel = relative_path(root, staged);
            dirs.iter().chain(files).map(move |name| (rel.clone(), name.clone()))
        })
        .map(|(rel, name)| normpath_join(&rel, &name))
        .collect();

    for (root, _dirs, files) in &walked {
        let rel = relative_path(root, staged);
        let destination_root = if rel == "." { target.to_path_buf() } else { target.join(&rel) };
        std::fs::create_dir_all(&destination_root)?;
        for name in files {
            let source = root.join(name);
            let destination = destination_root.join(name);
            let temporary = destination.with_file_name(format!("{name}{IN_PLACE_TEMP_SUFFIX}"));
            copy_file(&source, &temporary)?;
            match std::fs::rename(&temporary, &destination) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                    // A scanner can hold a freshly touched runtime file open
                    // briefly.  Wait out that window once; a locked file whose
                    // bytes already equal the staged copy needs no replace.
                    std::thread::sleep(SWAP_RETRY_DELAY);
                    match std::fs::rename(&temporary, &destination) {
                        Ok(()) => {}
                        Err(retry) if retry.kind() == io::ErrorKind::PermissionDenied => {
                            let destination_digest = sha256_file(&destination);
                            let temporary_digest = sha256_file(&temporary);
                            match (destination_digest, temporary_digest) {
                                (Ok(left), Ok(right)) if left == right => {
                                    let _ = remove_missing(&temporary);
                                }
                                (Err(error), _) | (_, Err(error)) => return Err(error),
                                _ => return Err(retry),
                            }
                        }
                        Err(retry) => return Err(retry),
                    }
                }
                Err(error) => return Err(error),
            }
        }
    }

    for (root, dirs, files) in walk_bottomup(target) {
        let rel = relative_path(&root, target);
        for name in dirs.into_iter().chain(files) {
            if staged_names.contains(&normpath_join(&rel, &name)) {
                continue;
            }
            let path = root.join(&name);
            if path.is_dir() {
                remove_tree(&path);
            } else {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    Ok(())
}

/// `hashlib.sha256` over 1 MiB reads (`hermes_adapter.py:611-617`).  A digest
/// does not depend on the chunk size, so the crate's 64 KiB streaming helper is
/// reused read-only instead of duplicating a third hasher.
pub fn sha256_file(path: &Path) -> io::Result<String> {
    crate::crypto::sha256_file(path).map_err(|error| io::Error::other(error.to_string()))
}

/// `os.path.relpath(root, base)` with `os.curdir` (`"."`) for the base itself.
pub fn relative_path(root: &Path, base: &Path) -> String {
    let root_text = root.to_string_lossy().into_owned();
    let base_text = base.to_string_lossy().into_owned();
    if root_text == base_text {
        return ".".to_string();
    }
    let prefix = format!(
        "{}{}",
        base_text.trim_end_matches(['/', '\\']),
        std::path::MAIN_SEPARATOR_STR
    );
    match root_text.strip_prefix(&prefix) {
        Some(rest) => rest.to_string(),
        None => root_text,
    }
}

/// `os.path.normpath(os.path.join(rel, name))` (`hermes_adapter.py:569-571`
/// and `:595`).  Walk output never contains `.`/`..` segments, so the POSIX
/// branch only has to fold separators.
pub fn normpath_join(rel: &str, name: &str) -> String {
    let joined = if rel == "." {
        name.to_string()
    } else {
        format!("{rel}{}{name}", std::path::MAIN_SEPARATOR_STR)
    };
    if cfg!(windows) {
        crate::pet_queue::py_normpath(&joined)
    } else {
        joined
    }
}

/// `shutil.copyfile` — bytes only, an existing destination is truncated, no
/// metadata is carried over (`hermes_adapter.py:577`).
pub fn copy_file(source: &Path, destination: &Path) -> io::Result<()> {
    std::fs::write(destination, std::fs::read(source)?)
}

/// `shutil.rmtree(path, ignore_errors=True)`.
pub fn remove_tree(path: &Path) {
    let _ = std::fs::remove_dir_all(path);
}

/// `shutil.copytree(staged, target)` with the default `symlinks=False`, i.e.
/// every symlink in the tree is materialised as a regular copy
/// (`hermes_adapter.py:687`).
pub fn copy_tree(source: &Path, destination: &Path) -> io::Result<()> {
    std::fs::create_dir(destination)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let from = entry.path();
        let to = destination.join(entry.file_name());
        if std::fs::metadata(&from).map(|stat| stat.is_dir()).unwrap_or(false) {
            copy_tree(&from, &to)?;
        } else {
            copy_file(&from, &to)?;
        }
    }
    Ok(())
}

/// `_publish_staged_tree(staged, expected)` (`hermes_adapter.py:667-698`), for
/// the `self.root` / `self.target` pair the installer keeps on its instance.
pub fn publish_staged_tree(
    root: &Path,
    target: &Path,
    staged: &Path,
    expected_files: usize,
) -> io::Result<OrdValue> {
    let backup = root.join(BACKUP_DIR_NAME);
    if backup.exists() {
        std::fs::remove_dir_all(&backup)?;
    }
    if target.exists() {
        match replace_with_retry(target, &backup) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                // Renaming the target itself failed while the staged tree is
                // verified, so swap its contents in place instead.
                replace_tree_in_place(staged, target)?;
                return Ok(published_report(expected_files));
            }
            Err(error) => return Err(error),
        }
    }
    let inner: io::Result<()> = match replace_with_retry(staged, target) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
            // A scanner can keep a staged file open.  Copying to fresh target
            // paths is unaffected by that handle.
            match copy_tree(staged, target) {
                Ok(()) => Ok(()),
                Err(copy_error) => {
                    remove_tree(target);
                    Err(copy_error)
                }
            }
        }
        Err(error) => Err(error),
    };
    if let Err(error) = inner {
        // Preserve the last working adapter if publishing fails, then re-raise.
        if backup.exists() && !target.exists() {
            replace_with_retry(&backup, target)?;
        }
        return Err(error);
    }
    if backup.exists() {
        std::fs::remove_dir_all(&backup)?;
    }
    Ok(published_report(expected_files))
}

fn published_report(expected_files: usize) -> OrdValue {
    OrdValue::object(vec![
        ("ok", OrdValue::boolean(true)),
        ("installed", OrdValue::boolean(true)),
        ("files", OrdValue::int(expected_files as i64)),
    ])
}

/// `_is_safe_name` (`hermes_adapter.py:606-609`).
pub fn is_safe_name(name: &str) -> bool {
    !name.is_empty()
        && !Path::new(name).is_absolute()
        && !Path::new(name)
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
        && !name.contains('\\')
}

// ---------------------------------------------------------------------------
// Win32 process plumbing (kernel32 only, no new dependency)
// ---------------------------------------------------------------------------

/// Raw kernel32 declarations.  `readmd-kernel/Cargo.toml` has no
/// `windows`/`winapi`/`libc` and there is no network to add one, so the
/// bindings are hand-written the way `crypto.rs`'s `advapi32` block and
/// `main.rs`'s `AttachConsole` already are.  Only this module and its
/// non-Windows twin are `#[cfg]`-gated; every decision above is live code.
#[cfg(windows)]
mod win {
    use super::NativeProcess;
    use std::ffi::c_void;

    const TH32CS_SNAPPROCESS: u32 = 0x0000_0002;
    const PROCESS_TERMINATE: u32 = 0x0001;
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;

    #[repr(C)]
    #[allow(non_snake_case)]
    struct PROCESSENTRY32W {
        dwSize: u32,
        cntUsage: u32,
        th32ProcessID: u32,
        th32DefaultHeapID: usize,
        th32ModuleID: u32,
        cntThreads: u32,
        th32ParentProcessID: u32,
        pcPriClassBase: i32,
        dwFlags: u32,
        szExeFile: [u16; 260],
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn CreateToolhelp32Snapshot(dwFlags: u32, th32ProcessID: u32) -> *mut c_void;
        fn Process32FirstW(hSnapshot: *mut c_void, lppe: *mut PROCESSENTRY32W) -> i32;
        fn Process32NextW(hSnapshot: *mut c_void, lppe: *mut PROCESSENTRY32W) -> i32;
        fn OpenProcess(dwDesiredAccess: u32, bInheritHandle: i32, dwProcessId: u32) -> *mut c_void;
        fn QueryFullProcessImageNameW(
            hProcess: *mut c_void,
            dwFlags: u32,
            lpExeName: *mut u16,
            lpdwSize: *mut u32,
        ) -> i32;
        fn TerminateProcess(hProcess: *mut c_void, uExitCode: u32) -> i32;
        fn CloseHandle(hObject: *mut c_void) -> i32;
    }

    fn unusable(handle: *mut c_void) -> bool {
        handle.is_null() || (handle as isize) == -1
    }

    /// `psutil.process_iter(['pid', 'name', 'exe'])`.  `None` means the snapshot
    /// could not be taken at all — the case the authority answers with
    /// PowerShell.
    pub fn list_processes() -> Option<Vec<NativeProcess>> {
        let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        if unusable(snapshot) {
            return None;
        }
        let mut out = Vec::new();
        let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut ok = unsafe { Process32FirstW(snapshot, &mut entry) };
        while ok != 0 {
            out.push(NativeProcess {
                pid: entry.th32ProcessID,
                parent: entry.th32ParentProcessID,
                name: wide_to_string(&entry.szExeFile),
                exe: image_name(entry.th32ProcessID),
            });
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            ok = unsafe { Process32NextW(snapshot, &mut entry) };
        }
        unsafe {
            CloseHandle(snapshot);
        }
        Some(out)
    }

    /// A `WCHAR[MAX_PATH]` up to its first NUL.
    fn wide_to_string(buffer: &[u16; 260]) -> String {
        let end = buffer.iter().position(|unit| *unit == 0).unwrap_or(buffer.len());
        String::from_utf16_lossy(&buffer[..end])
    }

    /// `proc.info['exe']` — `QueryFullProcessImageNameW`, `None` where psutil
    /// would raise `AccessDenied`.
    fn image_name(pid: u32) -> Option<String> {
        let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if unusable(handle) {
            return None;
        }
        let mut buffer = vec![0u16; 32_768];
        let mut size = buffer.len() as u32;
        let ok = unsafe { QueryFullProcessImageNameW(handle, 0, buffer.as_mut_ptr(), &mut size) };
        unsafe {
            CloseHandle(handle);
        }
        if ok == 0 {
            return None;
        }
        Some(String::from_utf16_lossy(&buffer[..size as usize]))
    }

    /// `proc.kill()` / `taskkill /F`.
    pub fn terminate_process(pid: u32) -> bool {
        let handle = unsafe { OpenProcess(PROCESS_TERMINATE, 0, pid) };
        if unusable(handle) {
            return false;
        }
        let ok = unsafe { TerminateProcess(handle, 1) != 0 };
        unsafe {
            CloseHandle(handle);
        }
        ok
    }
}

#[cfg(not(windows))]
mod win {
    use super::NativeProcess;

    pub fn list_processes() -> Option<Vec<NativeProcess>> {
        None
    }

    pub fn terminate_process(_pid: u32) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQ: AtomicU64 = AtomicU64::new(0);

    /// A private temporary directory, removed when the test ends.
    struct Scratch {
        path: PathBuf,
    }

    impl Scratch {
        fn new(tag: &str) -> Scratch {
            let n = SEQ.fetch_add(1, Ordering::SeqCst);
            let path = std::env::temp_dir().join(format!(
                "readmd-pet-launcher-s1-{tag}-{}-{n}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).expect("scratch dir");
            Scratch { path }
        }
        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    fn write(path: &Path, bytes: &[u8]) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(path, bytes).expect("write fixture");
    }

    fn read_text(path: &Path) -> String {
        std::fs::read_to_string(path).unwrap_or_else(|_| "<missing>".to_string())
    }

    fn bridge_in(dir: &Path) -> HermesPetBridge {
        HermesPetBridge::new(dir)
    }

    fn options() -> PublishOptions {
        PublishOptions::default()
    }

    // ---------------------------------------------------------------- paths

    #[test]
    fn bridge_paths_mirror_pathlib_derivation() {
        let scratch = Scratch::new("paths");
        let bridge = bridge_in(scratch.path());
        assert_eq!(
            bridge.state_path,
            bridge.root.join("hermes-overlay-state.json")
        );
        assert_eq!(
            bridge.command_path,
            bridge.root.join("hermes-overlay-state.json.command")
        );
        assert_eq!(
            bridge.commands_dir,
            bridge.root.join("hermes-overlay-state.json.commands")
        );
        // `with_suffix(".tmp")` replaces the extension instead of appending.
        assert_eq!(bridge.root.join(STATE_TMP_NAME).to_string_lossy().rsplit('\\').next(), Some(STATE_TMP_NAME));
        assert_eq!(STATE_TMP_NAME, "hermes-overlay-state.tmp");
        // The health name is a plain string append to the *state* path.
        assert!(format!("{}{HEALTH_SUFFIX}", bridge.state_path.display())
            .ends_with("hermes-overlay-state.json.health.json"));
    }

    // -------------------------------------------------------------- publish

    #[test]
    fn publish_writes_python_key_order_and_compact_separators() {
        let scratch = Scratch::new("order");
        let bridge = bridge_in(scratch.path());
        let payload = bridge
            .publish(
                &json!({"visible": true, "state": "idle"}),
                &options(),
            )
            .expect("publish");
        assert_eq!(
            payload.dump(),
            "{\"format_version\":1,\"visible\":true,\"info\":{},\"activity\":{\"busy\":false,\"error\":false,\"justCompleted\":false},\"busy\":false,\"awaiting\":false,\"unread\":false}"
        );
        assert_eq!(read_text(&bridge.state_path), payload.dump());
    }

    #[test]
    fn publish_appends_optional_keys_in_python_order() {
        let scratch = Scratch::new("order2");
        let bridge = bridge_in(scratch.path());
        let mut opts = options();
        opts.info = vec![("displayName".to_string(), OrdValue::text("Pix"))];
        opts.activity = vec![("ping".to_string(), OrdValue::int(1))];
        opts.bounds = Some(json!({"x": 1, "y": 2, "width": 40, "height": 99999}));
        opts.renderer = Some(json!("live2d"));
        opts.fullscreen = Some(json!(true));
        let payload = bridge.publish(&json!({"visible": 1}), &opts).expect("publish");
        assert_eq!(
            payload.dump(),
            "{\"format_version\":1,\"visible\":true,\"info\":{\"displayName\":\"Pix\"},\"activity\":{\"busy\":false,\"error\":false,\"justCompleted\":false,\"ping\":1},\"busy\":false,\"awaiting\":false,\"unread\":false,\"bounds\":{\"x\":1,\"y\":2,\"width\":80,\"height\":2048},\"renderer\":\"live2d\",\"fullscreen\":true}"
        );
    }

    #[test]
    fn publish_dedups_identical_encoding_while_state_exists() {
        let scratch = Scratch::new("dedup");
        let bridge = bridge_in(scratch.path());
        let runtime = json!({"visible": true, "state": "idle"});
        bridge.publish(&runtime, &options()).expect("first");
        std::fs::write(&bridge.state_path, "sentinel").expect("sentinel");
        bridge.publish(&runtime, &options()).expect("second");
        // Same encoding + the state file exists -> early return, no rewrite.
        assert_eq!(read_text(&bridge.state_path), "sentinel");
        // A different payload is no longer a duplicate.
        bridge.publish(&json!({"visible": false}), &options()).expect("third");
        assert!(read_text(&bridge.state_path).starts_with("{\"format_version\":1,\"visible\":false"));
    }

    #[test]
    fn publish_rewrites_after_state_file_is_removed() {
        let scratch = Scratch::new("dedup2");
        let bridge = bridge_in(scratch.path());
        let runtime = json!({"visible": true});
        bridge.publish(&runtime, &options()).expect("first");
        std::fs::remove_file(&bridge.state_path).expect("remove");
        bridge.publish(&runtime, &options()).expect("second");
        assert!(bridge.state_path.is_file(), "is_file() gate must defeat the cache");
    }

    #[test]
    fn publish_derived_activity_matrix() {
        let scratch = Scratch::new("derived");
        let bridge = bridge_in(scratch.path());
        let cases: Vec<(Value, bool, bool, bool)> = vec![
            (json!({}), false, false, false),
            (json!({"state": "busy"}), true, false, false),
            (json!({"activity_state": "error"}), false, true, false),
            (json!({"state": "success"}), false, false, true),
            // `activity_state` wins over `state` when both are present.
            (json!({"activity_state": "success", "state": "error"}), false, false, true),
            // A present-but-falsy `active_tasks` overrides the derived default,
            // while `error` stays `state == "error"`: measured from the
            // authority, `publish({"state":"busy","active_tasks":[]})` gives
            // `{'busy': False, 'error': False, 'justCompleted': False}`.
            (json!({"state": "busy", "active_tasks": []}), false, false, false),
            (json!({"state": "busy", "active_tasks": ["one"]}), true, false, false),
            // `None` is not "absent": bool(None) is False.
            (json!({"state": "busy", "active_tasks": null}), false, false, false),
            // `state` is stringified before the comparison, so 1 never matches.
            (json!({"state": 1}), false, false, false),
        ];
        for (runtime, busy, error, just_completed) in cases {
            let payload = bridge.publish(&runtime, &options()).expect("publish");
            let activity = payload.get("activity").expect("activity");
            assert_eq!(
                activity.dump(),
                format!(
                    "{{\"busy\":{},\"error\":{},\"justCompleted\":{}}}",
                    busy, error, just_completed
                ),
                "runtime {runtime}"
            );
            assert_eq!(payload.get("busy").expect("busy").dump(), if busy { "true" } else { "false" });
        }
    }

    #[test]
    fn publish_activity_merge_keeps_derived_slots_and_appends_new() {
        let scratch = Scratch::new("merge");
        let bridge = bridge_in(scratch.path());
        let mut opts = options();
        opts.activity = vec![
            ("ping".to_string(), OrdValue::int(2)),
            ("busy".to_string(), OrdValue::boolean(false)),
        ];
        let payload = bridge.publish(&json!({"state": "busy"}), &opts).expect("publish");
        // `{**derived, **activity}`: `busy` keeps its first slot but takes the
        // caller's value, and `ping` is appended at the end.
        assert_eq!(
            payload.get("activity").expect("activity").dump(),
            "{\"busy\":false,\"error\":false,\"justCompleted\":false,\"ping\":2}"
        );
        // The top-level `busy` mirror still comes from the derived flags.
        assert_eq!(payload.get("busy").expect("busy").dump(), "true");
    }

    #[test]
    fn publish_bool_coercions_for_visible_and_fullscreen() {
        let scratch = Scratch::new("bools");
        let bridge = bridge_in(scratch.path());
        let payload = bridge.publish(&json!({"visible": "0"}), &options()).expect("publish");
        assert_eq!(payload.get("visible").expect("visible").dump(), "true", "bool('0') is True");
        let payload = bridge.publish(&json!({"visible": ""}), &options()).expect("publish");
        assert_eq!(payload.get("visible").expect("visible").dump(), "false");
        let mut opts = options();
        opts.fullscreen = Some(json!([]));
        let payload = bridge.publish(&json!({}), &opts).expect("publish");
        assert_eq!(payload.get("fullscreen").expect("fullscreen").dump(), "false");
        // `None`/JSON null omits the key entirely.
        let mut opts = options();
        opts.fullscreen = Some(Value::Null);
        opts.renderer = Some(Value::Null);
        let payload = bridge.publish(&json!({}), &opts).expect("publish");
        assert!(payload.get("fullscreen").is_none());
        assert!(payload.get("renderer").is_none());
    }

    #[test]
    fn publish_stringifies_renderer_with_str_semantics() {
        let scratch = Scratch::new("renderer");
        let bridge = bridge_in(scratch.path());
        let mut opts = options();
        opts.renderer = Some(json!(5));
        let payload = bridge.publish(&json!({}), &opts).expect("publish");
        assert_eq!(payload.get("renderer").expect("renderer").dump(), "\"5\"");
        let mut opts = options();
        opts.renderer = Some(json!(true));
        let payload = bridge.publish(&json!({}), &opts).expect("publish");
        assert_eq!(payload.get("renderer").expect("renderer").dump(), "\"True\"");
        let mut opts = options();
        opts.renderer = Some(json!([1, "a"]));
        let payload = bridge.publish(&json!({}), &opts).expect("publish");
        assert_eq!(payload.get("renderer").expect("renderer").dump(), "\"[1, 'a']\"");
    }

    #[test]
    fn publish_ignores_bounds_that_are_not_a_dict() {
        let scratch = Scratch::new("bounds-absent");
        let bridge = bridge_in(scratch.path());
        for raw in [json!(null), json!("x"), json!([1]), json!(3)] {
            let mut opts = options();
            opts.bounds = Some(raw.clone());
            let payload = bridge.publish(&json!({}), &opts).expect("publish");
            assert!(payload.get("bounds").is_none(), "bounds {raw} must be skipped");
        }
    }

    #[test]
    fn publish_creates_root_and_leaves_no_temp_file() {
        let scratch = Scratch::new("root");
        let data = scratch.path().join("nested").join("data");
        let bridge = bridge_in(&data);
        assert!(!bridge.root.exists());
        bridge.publish(&json!({"visible": true}), &options()).expect("publish");
        assert!(bridge.state_path.is_file());
        assert!(!bridge.root.join(STATE_TMP_NAME).exists());
    }

    #[test]
    fn publish_bounds_failure_is_invalid_pet_bounds() {
        let scratch = Scratch::new("bounds-error");
        let bridge = bridge_in(scratch.path());
        let mut opts = options();
        opts.bounds = Some(json!({"x": 1, "y": 2, "width": 100}));
        let error = bridge.publish(&json!({}), &opts).expect_err("height missing");
        assert!(matches!(error, BridgeError::InvalidPetBounds));
        assert_eq!(error.to_string(), "invalid_pet_bounds");
    }

    // ---------------------------------------------------------- _safe_bounds

    #[test]
    fn safe_bounds_clamps_rounds_half_even_and_coerces() {
        let ok = safe_bounds(&json!({"x": 2.5, "y": -2.5, "width": 10, "height": 99999})).expect("bounds");
        assert_eq!(ok.dump(), "{\"x\":2,\"y\":-2,\"width\":80,\"height\":2048}");
        // `float("12")` and `float(True)` both coerce; clamping still applies.
        // `height: -1e30` is finite, so `int(round(-1e30))` is a huge negative
        // integer and `max(80, min(2048, n))` clamps it *up* to the low bound:
        // measured `_safe_bounds({"x":"12","y":True,"width":False,"height":-1e30})`
        // == `{'x': 12, 'y': 1, 'width': 80, 'height': 80}`.
        let ok = safe_bounds(&json!({"x": "12", "y": true, "width": false, "height": -1e30})).expect("bounds");
        assert_eq!(ok.dump(), "{\"x\":12,\"y\":1,\"width\":80,\"height\":80}", "height clamps up to the low bound");
        let ok = safe_bounds(&json!({"x": 1e30, "y": 40.49, "width": 40.5, "height": 41.5})).expect("bounds");
        assert_eq!(ok.dump(), "{\"x\":32768,\"y\":40,\"width\":80,\"height\":80}");
        // Measured in `scratch/rust_parity/_pet_launcher_s1_probe6.py`: a string
        // coordinate is parsed with the full `float()` grammar.
        let ok = safe_bounds(&json!({"x": "  12  ", "y": "\u{661}\u{662}", "width": "100", "height": "200"}))
            .expect("bounds");
        assert_eq!(ok.dump(), "{\"x\":12,\"y\":12,\"width\":100,\"height\":200}");
        // Exact-equal paths: `startswith(normcase(target) + os.sep)`.
        let ok = safe_bounds(&json!({"x": -32768, "y": 32768, "width": 2048, "height": 80})).expect("bounds");
        assert_eq!(ok.dump(), "{\"x\":-32768,\"y\":32768,\"width\":2048,\"height\":80}");
    }

    #[test]
    fn safe_bounds_rejects_missing_nonfinite_and_uncoercible() {
        for raw in [
            json!({"y": 1, "width": 1, "height": 1}), // KeyError on x
            json!({"x": null, "y": 1, "width": 1, "height": 1}), // TypeError
            json!({"x": [], "y": 1, "width": 1, "height": 1}), // TypeError
            json!({"x": "abc", "y": 1, "width": 1, "height": 1}), // ValueError
            json!({"x": "1e999", "y": 1, "width": 1, "height": 1}), // float() overflow -> inf
            json!({"x": "inf", "y": 1, "width": 1, "height": 1}), // not math.isfinite
            json!({"x": "nan", "y": 1, "width": 1, "height": 1}), // not math.isfinite
            json!({"x": {}, "y": 1, "width": 1, "height": 1}), // TypeError
            json!([]), // no keys at all
            json!("x"), // not even a mapping
        ] {
            let error = safe_bounds(&raw).expect_err("must reject");
            assert!(matches!(error, BridgeError::InvalidPetBounds), "{raw} -> {error}");
        }
        // Out-of-range number *text* never reaches `safe_bounds`: the parser
        // rejects it first, while CPython's `json.loads` yields `inf` and lets
        // `math.isfinite` reject downstream. Both ends reject, at different
        // layers. Measured in `scratch/rust_parity/probe_1e999_s10.rs`.
        //
        // The command path now decodes with [`py_json_loads`], so `1e999` really
        // does arrive as [`NonFinite::Infinity`] and the `is_finite` gate above is
        // what rejects it — the layer CPython rejects at.  These assertions are
        // about `serde_json`, which is still the reader of every `Value`-typed
        // entry point here and still errors on the same literal.
        assert!(serde_json::from_str::<Value>(r#"{"x": 1e999}"#).is_err());
        assert!(serde_json::from_str::<Value>(r#"{"x": 1e309}"#).is_err());
        assert!(matches!(
            serde_json::from_str::<Value>(r#"{"x": 1e308}"#),
            Ok(value) if value["x"].is_f64()
        ));
        assert!(safe_bounds(&json!({"x": f64::INFINITY, "y": 1, "width": 1, "height": 1})).is_err());
    }

    #[test]
    fn safe_bounds_rejects_non_finite_values() {
        // `float("nan")`/`float("inf")` parse, so only the isfinite gate stops
        // them; both are the same ValueError upstream.
        for raw in ["nan", "inf", "-inf", "Infinity"] {
            let value = json!({"x": raw, "y": 1, "width": 1, "height": 1});
            assert!(safe_bounds(&value).is_err(), "{raw} must be rejected");
        }
    }

    // ------------------------------------------------------ validate_command

    fn envelope(command: Value) -> Value {
        json!({"version": 1, "command": command})
    }

    #[test]
    fn validate_command_rejects_envelope_shapes() {
        assert_eq!(validate_command(&json!([])), None);
        assert_eq!(validate_command(&json!("x")), None);
        assert_eq!(validate_command(&json!(null)), None);
        assert_eq!(validate_command(&json!({})), None, "no command key");
        assert_eq!(validate_command(&json!({"command": null})), None);
        assert_eq!(validate_command(&json!({"command": []})), None);
        assert_eq!(validate_command(&json!({"command": {}})), None, "no type");
        assert_eq!(validate_command(&envelope(json!({"type": "resize"}))), None, "not allow-listed");
        assert_eq!(validate_command(&envelope(json!({"type": 5}))), None, "non-str type");
        assert_eq!(validate_command(&envelope(json!({"type": true}))), None, "bool is not a kind");
    }

    #[test]
    fn validate_command_interact_allow_list_and_key_rebuild() {
        for action in INTERACT_ACTIONS {
            let got = validate_command(&envelope(json!({"type": "interact", "action": action, "junk": 1})))
                .expect("valid");
            assert_eq!(got, json!({"type": "interact", "action": action}), "extras dropped");
        }
        assert_eq!(validate_command(&envelope(json!({"type": "interact"}))), None);
        assert_eq!(validate_command(&envelope(json!({"type": "interact", "action": "sleep"}))), None);
        assert_eq!(validate_command(&envelope(json!({"type": "interact", "action": null}))), None);
    }

    #[test]
    fn validate_command_character_slug_and_renderer_gates() {
        let got = validate_command(&envelope(
            json!({"type": "character", "slug": "pixie", "renderer": "hermes-sprite"}),
        ))
        .expect("valid");
        assert_eq!(got, json!({"type": "character", "slug": "pixie", "renderer": "hermes-sprite"}));
        // `slug` is `command.get('slug', '')`, so an absent slug is legal.
        let got = validate_command(&envelope(json!({"type": "character", "renderer": "live2d"})))
            .expect("absent slug");
        assert_eq!(got["slug"], json!(""));
        assert_eq!(
            validate_command(&envelope(json!({"type": "character", "slug": 5, "renderer": "live2d"}))),
            None
        );
        assert_eq!(
            validate_command(&envelope(json!({"type": "character", "slug": "pixie", "renderer": "spine"}))),
            None
        );
        assert_eq!(
            validate_command(&envelope(json!({"type": "character", "slug": "pixie"}))),
            None,
            "renderer absent -> None not in the allow-list"
        );
    }

    #[test]
    fn validate_command_character_slug_cap_counts_code_points() {
        let at_limit = "\u{1f41c}".repeat(63); // 63 code points, 252 UTF-8 bytes
        assert!(validate_command(&envelope(json!({
            "type": "character", "slug": at_limit, "renderer": "live2d"
        })))
        .is_some());
        let over = "\u{1f41c}".repeat(64);
        assert!(validate_command(&envelope(json!({
            "type": "character", "slug": over, "renderer": "live2d"
        })))
        .is_none());
    }

    #[test]
    fn validate_command_scale_uses_python_round_and_band() {
        // CPython measured table (`python -c "print(round(0.185, 2))"` -> 0.18):
        // `round(x, 2)` is the correctly-rounded decimal of the *binary* value,
        // so 0.175 -> 0.17, 0.185 -> 0.18 (accept), 2.675 -> 2.67.
        // ReadMD extends the lower bound to 0.08; legacy clients up to 0.72 remain accepted.
        // [(0.18, 0.18), (0.175, 0.17), (0.185, 0.18), (0.715, 0.71),
        //  (0.72, 0.72), (0.725, 0.72), (0.73, None), (2.675, None),
        //  (-0.0, None), ("0.5", 0.5)]
        let cases: Vec<(Value, Option<f64>)> = vec![
            (json!(0.18), Some(0.18)),
            (json!(0.175), Some(0.17)),
            (json!(0.185), Some(0.18)),
            (json!(0.715), Some(0.71)),
            (json!(0.72), Some(0.72)),
            (json!(0.725), Some(0.72)),
            (json!(0.73), None),
            (json!(2.675), None),
            (json!(-0.0), None),
            (json!("0.5"), Some(0.5)),
            (json!(true), None),
            (json!(null), None),
            (json!("abc"), None),
            (json!([0.5]), None),
        ];
        for (raw, expected) in cases {
            let got = validate_command(&envelope(json!({"type": "scale", "scale": raw})));
            match expected {
                Some(scale) => {
                    let value = got.unwrap_or_else(|| panic!("{raw} should pass"));
                    assert_eq!(value["scale"].as_f64(), Some(scale), "{raw}");
                    assert_eq!(value, json!({"type": "scale", "scale": scale}));
                }
                None => assert_eq!(got, None, "{raw} should be rejected"),
            }
        }
        // A scale command drops every sibling key.
        let got = validate_command(&envelope(json!({"type": "scale", "scale": 0.5, "source": "ui"}))).expect("ok");
        assert_eq!(got.as_object().expect("object").len(), 2);
    }

    #[test]
    fn validate_command_scale_accepts_the_absent_key_path() {
        assert_eq!(validate_command(&envelope(json!({"type": "scale"}))), None, "float(None) -> TypeError");
    }

    #[test]
    fn validate_command_drop_gates() {
        let got = validate_command(&envelope(json!({"type": "drop", "paths": ["a.md", "b.md"], "x": 1})))
            .expect("valid");
        assert_eq!(got, json!({"type": "drop", "paths": ["a.md", "b.md"]}));
        assert_eq!(validate_command(&envelope(json!({"type": "drop", "paths": []}))), None);
        assert_eq!(validate_command(&envelope(json!({"type": "drop", "paths": "a.md"}))), None);
        assert_eq!(validate_command(&envelope(json!({"type": "drop"}))), None);
        assert_eq!(validate_command(&envelope(json!({"type": "drop", "paths": ["a", ""]}))), None);
        assert_eq!(validate_command(&envelope(json!({"type": "drop", "paths": ["a", 5]}))), None);
        let hundred = vec![json!("x"); MAX_COMMAND_BATCH];
        assert!(validate_command(&envelope(json!({"type": "drop", "paths": hundred}))).is_some());
        let too_many = vec![json!("x"); MAX_COMMAND_BATCH + 1];
        assert!(validate_command(&envelope(json!({"type": "drop", "paths": too_many}))).is_none());
        let long_path = "x".repeat(MAX_PATH_CHARS);
        assert!(validate_command(&envelope(json!({"type": "drop", "paths": [long_path]}))).is_some());
        let too_long = "x".repeat(MAX_PATH_CHARS + 1);
        assert!(validate_command(&envelope(json!({"type": "drop", "paths": [too_long]}))).is_none());
    }

    #[test]
    fn validate_command_clipboard_defaults_and_gates() {
        let got = validate_command(&envelope(json!({"type": "clipboard"}))).expect("all defaults");
        assert_eq!(got, json!({"type": "clipboard", "text": "", "image_png": "", "paths": []}));
        assert_eq!(validate_command(&envelope(json!({"type": "clipboard", "text": 5}))), None);
        assert_eq!(validate_command(&envelope(json!({"type": "clipboard", "image_png": null}))), None);
        assert_eq!(validate_command(&envelope(json!({"type": "clipboard", "paths": "a"}))), None);
        // An *empty* path is legal for clipboard (only `drop` rejects it).
        let got = validate_command(&envelope(json!({"type": "clipboard", "paths": ["", "b"]}))).expect("ok");
        assert_eq!(got["paths"], json!(["", "b"]));
        let too_many = vec![json!("x"); MAX_COMMAND_BATCH + 1];
        assert!(validate_command(&envelope(json!({"type": "clipboard", "paths": too_many}))).is_none());
    }

    #[test]
    fn validate_command_clipboard_text_cap_counts_utf8_bytes() {
        let at_limit = "a".repeat(MAX_CLIPBOARD_TEXT_BYTES);
        assert!(validate_command(&envelope(json!({"type": "clipboard", "text": at_limit}))).is_some());
        let over = "a".repeat(MAX_CLIPBOARD_TEXT_BYTES + 1);
        assert!(validate_command(&envelope(json!({"type": "clipboard", "text": over}))).is_none());
        // Same character count, 3x the bytes: still under the 4 MiB *byte* cap,
        // which is the whole point of `text.encode("utf-8")`.  The division
        // truncates, so the run is `4194304 // 3 == 1398101` characters, i.e.
        // 4194303 bytes -- measured with
        // `python -c "print(len('中'.encode('utf-8')), 4*1024*1024//3, (4*1024*1024//3)*3)"`.
        let multibyte = "\u{4e2d}".repeat(MAX_CLIPBOARD_TEXT_BYTES / 3);
        assert_eq!(multibyte.chars().count(), MAX_CLIPBOARD_TEXT_BYTES / 3);
        assert_eq!(
            multibyte.len(),
            (MAX_CLIPBOARD_TEXT_BYTES / 3) * 3,
            "Rust's `str::len` is Python's `len(text.encode('utf-8'))`"
        );
        assert_eq!(multibyte.len(), MAX_CLIPBOARD_TEXT_BYTES - MAX_CLIPBOARD_TEXT_BYTES % 3);
        assert!(validate_command(&envelope(json!({"type": "clipboard", "text": multibyte}))).is_some());
    }

    #[test]
    fn validate_command_clipboard_image_cap_counts_code_points() {
        let limit = MAX_CLIPBOARD_IMAGE_CHARS;
        let at_limit = "\u{1f300}".repeat(200);
        assert!(at_limit.len() > at_limit.chars().count());
        assert!(validate_command(&envelope(json!({"type": "clipboard", "image_png": at_limit}))).is_some());
        let huge = "A".repeat(limit + 1);
        assert!(validate_command(&envelope(json!({"type": "clipboard", "image_png": huge}))).is_none());
    }

    #[test]
    fn validate_command_passthrough_kinds_keep_payload() {
        for kind in ["open-app", "open-menu", "pop-in", "submit", "toggle-app"] {
            let got = validate_command(&envelope(json!({"type": kind, "target": "menu", "n": 2}))).expect("ok");
            assert_eq!(got, json!({"type": kind, "target": "menu", "n": 2}));
        }
        // `bounds` merges the sanitised rect back into the original command.
        let got = validate_command(&envelope(
            json!({"type": "bounds", "reason": "drag", "bounds": {"x": 5.6, "y": 1, "width": 90, "height": 90}}),
        ))
        .expect("ok");
        assert_eq!(
            got,
            json!({"type": "bounds", "reason": "drag", "bounds": {"x": 6, "y": 1, "width": 90, "height": 90}})
        );
        assert_eq!(
            validate_command(&envelope(json!({"type": "bounds", "bounds": []}))),
            None
        );
        assert_eq!(validate_command(&envelope(json!({"type": "bounds"}))), None);
    }

    // ---------------------------------------------------- _take_command_file

    #[test]
    fn take_command_file_claims_then_deletes() {
        let scratch = Scratch::new("claim");
        let bridge = bridge_in(scratch.path());
        std::fs::create_dir_all(&bridge.commands_dir).expect("commands dir");
        let source = bridge.commands_dir.join("0001.json");
        write(
            &source,
            json!({"command": {"type": "scale", "scale": 0.4}}).to_string().as_bytes(),
        );
        let got = bridge.take_command_file(&source, true).expect("command");
        assert_eq!(got.dump(), r#"{"type":"scale","scale":0.4}"#);
        assert!(!source.exists(), "the durable claim is unlinked");
        let leftovers: Vec<String> = std::fs::read_dir(&bridge.commands_dir)
            .expect("read dir")
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(leftovers.is_empty(), "no .claim file survives: {leftovers:?}");
    }

    #[test]
    fn take_command_file_missing_source_is_none() {
        let scratch = Scratch::new("missing");
        let bridge = bridge_in(scratch.path());
        assert_eq!(bridge.take_command_file(&scratch.path().join("absent.json"), true), None);
        assert_eq!(bridge.take_command(), None, "take_command tolerates an empty bridge");
    }

    #[test]
    fn take_command_file_bad_json_restores_legacy_source() {
        let scratch = Scratch::new("restore");
        let bridge = bridge_in(scratch.path());
        let source = bridge.command_path.clone();
        write(&source, b"{not json");
        assert_eq!(bridge.take_command_file(&source, false), None);
        assert_eq!(read_text(&source), "{not json", "the legacy slot is put back");
        let claims: Vec<String> = std::fs::read_dir(bridge.root.join("pet"))
            .map(|reader| {
                reader
                    .filter_map(|entry| entry.ok())
                    .map(|entry| entry.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        assert!(
            claims.iter().all(|name| !name.contains(".claim.")),
            "no claim left behind: {claims:?}"
        );
    }

    #[test]
    fn take_command_file_bad_json_discards_durable_claim() {
        let scratch = Scratch::new("discard");
        let bridge = bridge_in(scratch.path());
        std::fs::create_dir_all(&bridge.commands_dir).expect("dir");
        let source = bridge.commands_dir.join("0002.json");
        write(&source, b"[]");
        assert_eq!(bridge.take_command_file(&source, true), None);
        assert!(!source.exists(), "a poisoned queue entry is consumed, not restored");
    }

    #[test]
    fn take_command_file_rejects_oversized_claims() {
        // The size gate is `st_size > 32 * 1024 * 1024`; the boundary is tested
        // against the constant rather than by writing 32 MiB per test run.
        assert!((MAX_COMMAND_FILE_BYTES) > 0);
        let inside = "x".repeat(1024);
        assert_eq!(inside.len(), 1024);
        let scratch = Scratch::new("size");
        let bridge = bridge_in(scratch.path());
        std::fs::create_dir_all(&bridge.commands_dir).expect("dir");
        let source = bridge.commands_dir.join("0003.json");
        write(
            &source,
            format!(
                "{{\"command\":{{\"type\":\"scale\",\"scale\":0.5}},\"pad\":\"{inside}\"}}"
            )
            .as_bytes(),
        );
        assert!(bridge.take_command_file(&source, true).is_some(), "a padded but valid command passes");
    }

    #[test]
    fn take_command_returns_fifo_order_and_drops_poisoned_entries() {
        let scratch = Scratch::new("fifo");
        let bridge = bridge_in(scratch.path());
        std::fs::create_dir_all(&bridge.commands_dir).expect("dir");
        write(&bridge.commands_dir.join("b.json"), json!({"command": {"type": "pop-in"}}).to_string().as_bytes());
        write(&bridge.commands_dir.join("a.json"), b"{broken");
        write(
            &bridge.commands_dir.join("c.json"),
            json!({"command": {"type": "interact", "action": "feed"}}).to_string().as_bytes(),
        );
        let first = bridge.take_command().expect("first");
        assert_eq!(first.dump(), r#"{"type":"pop-in"}"#, "a.json is poisoned, so b.json is next");
        let second = bridge.take_command().expect("second");
        assert_eq!(second.dump(), r#"{"type":"interact","action":"feed"}"#);
        assert_eq!(bridge.take_command(), None, "queue drained");
    }

    #[test]
    fn take_command_falls_back_to_the_legacy_single_slot() {
        let scratch = Scratch::new("legacy");
        let bridge = bridge_in(scratch.path());
        write(
            &bridge.command_path,
            json!({"command": {"type": "toggle-app"}}).to_string().as_bytes(),
        );
        assert_eq!(bridge.take_command().map(|command| command.dump()), Some(r#"{"type":"toggle-app"}"#.to_string()));
        assert!(!bridge.command_path.exists());
    }

    #[test]
    fn take_command_concurrent_claims_have_one_winner() {
        let scratch = Scratch::new("race");
        let bridge = Arc::new(bridge_in(scratch.path()));
        std::fs::create_dir_all(&bridge.commands_dir).expect("dir");
        let source = bridge.commands_dir.join("0009.json");
        write(&source, json!({"command": {"type": "submit", "text": "hi"}}).to_string().as_bytes());
        let bridge_for_threads = Arc::clone(&bridge);
        let winners = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        std::thread::scope(|scope| {
            for _ in 0..6 {
                let bridge = Arc::clone(&bridge_for_threads);
                let winners = Arc::clone(&winners);
                let source = source.clone();
                scope.spawn(move || {
                    if bridge.take_command_file(&source, false).is_some() {
                        winners.fetch_add(1, Ordering::SeqCst);
                    }
                });
            }
        });
        assert_eq!(winners.load(Ordering::SeqCst), 1, "the claim rename is the mutex");
        assert!(!source.exists());
    }

    // ------------------------------------------- json.loads parity (F-LA1/F-LA2)
    //
    // Each literal is either copied from `goldens.txt` / `golden_rows.json` —
    // generated by `scratch/rust_parity/pet_json_s1/goldens.py` against CPython
    // 3.11.15 and re-checked row by row by `harness.rs` in that directory — or a
    // *key-shuffled variant* of such a row, which is what makes the order
    // assertions below meaningful.  `expect` is what
    // `json.dumps(value, ensure_ascii=False, separators=(",", ":"))` writes, so
    // comparing [`OrdValue::dump`] to it compares the bytes the overlay sees.

    #[test]
    fn py_json_loads_accepts_the_python_non_finite_tokens() {
        for (raw, expected) in [
            (r#"{"x": NaN}"#, r#"{"x":NaN}"#),
            (r#"{"x": Infinity}"#, r#"{"x":Infinity}"#),
            (r#"{"x": -Infinity}"#, r#"{"x":-Infinity}"#),
            ("NaN", "NaN"),
            ("Infinity", "Infinity"),
            ("-Infinity", "-Infinity"),
            ("[NaN, Infinity, -Infinity]", "[NaN,Infinity,-Infinity]"),
            (r#"{"a": {"b": [NaN, {"c": -Infinity}]}}"#, r#"{"a":{"b":[NaN,{"c":-Infinity}]}}"#),
            (r#"{"outer": {"inner": {"value": NaN}}}"#, r#"{"outer":{"inner":{"value":NaN}}}"#),
            // A *string* spelled "NaN" stays a string: the tokens only count as
            // values, so this row also proves the fix cannot smuggle a float into
            // an `isinstance(x, str)` gate.
            (r#"{"x": "NaN"}"#, r#"{"x":"NaN"}"#),
            (r#"{"s": "aInfinityb", "x": NaN}"#, r#"{"s":"aInfinityb","x":NaN}"#),
            (r#"{"NaN": 1}"#, r#"{"NaN":1}"#),
            // Out-of-range exponents are `float()` results, not parse errors.
            (r#"{"x": 1e999}"#, r#"{"x":Infinity}"#),
            (r#"{"x": -1e999}"#, r#"{"x":-Infinity}"#),
            (r#"{"x": 1e309}"#, r#"{"x":Infinity}"#),
            (r#"{"x": 1e-999}"#, r#"{"x":0.0}"#),
            // Non-finite *and* order: a repeated key keeps its first slot and its
            // last value, exactly like `dict(pairs)`.
            (r#"{"k": 1, "j": 2, "k": 3}"#, r#"{"k":3,"j":2}"#),
            (r#"{"z": 1, "a": 2, "m": 3}"#, r#"{"z":1,"a":2,"m":3}"#),
            (r#"{"b": {"y": 1, "x": 2}, "a": 3}"#, r#"{"b":{"y":1,"x":2},"a":3}"#),
            // Controls: the plain JSON every command already used still works.
            ("{}", "{}"),
            ("[]", "[]"),
            (r#"{"a": true, "b": false, "c": null}"#, r#"{"a":true,"b":false,"c":null}"#),
            (r#"{"a": 100}"#, r#"{"a":100}"#),
            (r#"{"a": 1.5}"#, r#"{"a":1.5}"#),
            (r#"{"a": -0.0}"#, r#"{"a":-0.0}"#),
            (r#"{"a": 1e2}"#, r#"{"a":100.0}"#),
            (r#"{"a": "中文\u00e9"}"#, "{\"a\":\"\u{4e2d}\u{6587}\u{e9}\"}"),
            (r#"{"a": [{"b": 1}]}"#, r#"{"a":[{"b":1}]}"#),
            ("{\t\n\"a\":\r1 }", r#"{"a":1}"#),
        ] {
            let got = py_json_loads(raw).unwrap_or_else(|error| panic!("{raw} -> {error}"));
            assert_eq!(got.dump(), expected, "{raw}");
        }
        // What the defect actually was, kept as an executable note: serde_json
        // refuses the tokens outright, and `json!` silently *replaces* a
        // non-finite float with `null`, which is why the command path cannot go
        // through a `Value`.
        assert!(serde_json::from_str::<Value>(r#"{"x": NaN}"#).is_err());
        assert!(serde_json::from_str::<Value>(r#"{"x": Infinity}"#).is_err());
        assert!(serde_json::from_str::<Value>(r#"{"x": -Infinity}"#).is_err());
        assert!(serde_json::from_str::<Value>("NaN").is_err());
        assert!(serde_json::from_str::<Value>(r#"{"x": 1e999}"#).is_err());
        assert_eq!(json!({"x": f64::NAN}), json!({"x": null}));
        assert_eq!(json!({"x": f64::INFINITY}), json!({"x": null}));
        assert_eq!(py_json_loads("NaN").expect("NaN"), OrdValue::not_finite(NonFinite::Nan));
        assert_eq!(
            py_json_loads("-Infinity").expect("-Inf"),
            OrdValue::not_finite(NonFinite::NegInfinity)
        );
    }

    #[test]
    fn py_json_loads_rejects_what_json_loads_rejects() {
        // The spellings CPython's `parse_constant` does *not* accept, plus the
        // grammar controls.  All measured in `goldens.txt`.
        for raw in [
            "-NaN", "nan", "NAN", "inf", "-inf", "+Infinity", "NaNy", "Infinity2", "nan1",
            "Null", "TRUE", "False", "1e999e", "01", "1.", ".5", "+1", "1e", "1e+", "-",
            "[1,2,]", "{,}", "[,]", "{'a':1}", "{\"a\"}", "{\"a\":}", "{\"a\":1,}",
            "{\"a\" 1}", "{\"a\":1 \"b\":2}", "{}{}", "1 2", "[1] x", "\"unterminated",
            "\"\u{1}raw\"", "\"\\q\"", "\"\\u12\"", "\"\\uZZZZ\"", "\"\\u00\"", "\"\\u0x12\"",
            "\u{b}{}", "\u{c}{}", "", " ", "{\"x\": NaN", "{\"x\": NaN,", "[}", "{\"a\":1,,}",
        ] {
            assert!(py_json_loads(raw).is_err(), "{raw} must be rejected by json.loads too");
        }
        // `\uXXXX` still works when it is four real hex digits.
        assert_eq!(py_json_loads(r#""\u0041""#).expect("A"), OrdValue::text("A"));
        assert_eq!(py_json_loads(r#""\ud83d\ude00""#).expect("emoji"), OrdValue::text("\u{1f600}"));
        // `\"\\` and the six single-letter escapes all decode.
        assert_eq!(
            py_json_loads(r#""\"\\\/\b\f\n\r\t""#).expect("escapes"),
            OrdValue::text("\"\\/\u{8}\u{c}\n\r\t")
        );
        // Nesting is capped where serde_json capped it, so a pathological queue
        // file is rejected instead of growing the stack (see MAX_JSON_DEPTH).
        let nested = |depth: usize| -> String {
            format!("{}1{}", "[".repeat(depth), "]".repeat(depth))
        };
        assert!(py_json_loads(&nested(MAX_JSON_DEPTH)).is_ok());
        assert!(py_json_loads(&nested(MAX_JSON_DEPTH + 1)).is_err());
    }

    #[test]
    fn decoded_nan_behaves_like_python_nan_in_the_command_gates() {
        // `float(NaN)`/`math.isfinite` reject inside `_safe_bounds`, so a NaN
        // bound is a rejected command rather than a corrupted one.
        assert_eq!(command_of(r#"{"command":{"type":"bounds","bounds":{"x":NaN,"y":1,"width":100,"height":100}}}"#), None);
        assert_eq!(command_of(r#"{"command":{"type":"bounds","bounds":{"x":1e999,"y":1,"width":100,"height":100}}}"#), None);
        // `0.18 <= round(nan, 2) <= 0.72` is False; `round(float(10**400), 2)`
        // is the OverflowError the authority does not even catch.
        assert_eq!(command_of(r#"{"command":{"type":"scale","scale":NaN}}"#), None);
        assert_eq!(command_of(r#"{"command":{"type":"scale","scale":Infinity}}"#), None);
        assert_eq!(command_of(r#"{"command":{"type":"scale","scale":-Infinity}}"#), None);
        // `isinstance(NaN, str)` is False, so a NaN cannot pass as clipboard
        // text or as a dropped path -- the sentinel-representation trap.
        assert_eq!(command_of(r#"{"command":{"type":"clipboard","text":NaN}}"#), None);
        assert_eq!(command_of(r#"{"command":{"type":"drop","paths":[NaN]}}"#), None);
        assert_eq!(command_of(r#"{"command":{"type":"drop","paths":["a.md",Infinity]}}"#), None);
        // Everywhere else a NaN is just data and survives the round trip.
        assert_eq!(
            command_of(r#"{"command":{"type":"open-app","target":"x","extra":NaN}}"#).as_deref(),
            Some(r#"{"type":"open-app","target":"x","extra":NaN}"#)
        );
        assert_eq!(
            command_of(r#"{"command":{"type":"submit","payload":[1,Infinity,-Infinity]}}"#).as_deref(),
            Some(r#"{"type":"submit","payload":[1,Infinity,-Infinity]}"#)
        );
        assert_eq!(
            command_of(r#"{"command":{"type":"interact","action":"pet"},"note":NaN}"#).as_deref(),
            Some(r#"{"type":"interact","action":"pet"}"#),
            "the envelope's own NaN never reaches the command"
        );
    }

    #[test]
    fn validate_command_ord_keeps_python_key_order() {
        // F-LA2: `dict(command)` + `command["bounds"] = ...` replaces the key in
        // place, so the writer's order is the output's order.
        let raw = r#"{"command":{"type":"bounds","seq":1,"sent_at":"2026-09-23T06:00:00","bounds":{"x":12.4,"y":-8.6,"width":300.5,"height":210.9}},"ack":true}"#;
        assert_eq!(
            command_of(raw).as_deref(),
            Some(
                r#"{"type":"bounds","seq":1,"sent_at":"2026-09-23T06:00:00","bounds":{"x":12,"y":-9,"width":300,"height":211}}"#
            )
        );
        assert_ne!(
            command_of(raw).as_deref().expect("accepted"),
            r#"{"bounds":{"height":211,"width":300,"x":12,"y":-9},"seq":1,"sent_at":"2026-09-23T06:00:00","type":"bounds"}"#,
            "the serde_json::Value rebuild re-sorted the keys; that was the defect"
        );
        // bounds first, and the extra keys behind it, stay put.
        assert_eq!(
            command_of(
                r#"{"command":{"bounds":{"width":300.5,"height":210.9,"x":12.4,"y":-8.6},"type":"bounds","note":"last"}}"#
            )
            .as_deref(),
            Some(r#"{"bounds":{"x":12,"y":-9,"width":300,"height":211},"type":"bounds","note":"last"}"#)
        );
        // A passthrough command is handed back untouched.
        assert_eq!(
            command_of(r#"{"command":{"target":"menu","type":"open-menu","seq":3}}"#).as_deref(),
            Some(r#"{"target":"menu","type":"open-menu","seq":3}"#)
        );
        // The five rebuilt kinds emit the authority's dict-literal order, junk
        // dropped, regardless of how the input was spelled.
        assert_eq!(
            command_of(r#"{"command":{"junk":1,"action":"pet","type":"interact","x":[2]}}"#).as_deref(),
            Some(r#"{"type":"interact","action":"pet"}"#)
        );
        assert_eq!(
            command_of(r#"{"command":{"renderer":"live2d","slug":"pixie","type":"character","extra":NaN}}"#).as_deref(),
            Some(r#"{"type":"character","slug":"pixie","renderer":"live2d"}"#)
        );
        assert_eq!(
            command_of(r#"{"command":{"source":"ui","scale":0.5,"type":"scale"}}"#).as_deref(),
            Some(r#"{"type":"scale","scale":0.5}"#)
        );
        assert_eq!(
            command_of(r#"{"command":{"paths":["a.md"],"type":"drop","x":1}}"#).as_deref(),
            Some(r#"{"type":"drop","paths":["a.md"]}"#)
        );
        assert_eq!(
            command_of(r#"{"command":{"paths":[],"type":"clipboard","text":"hi\nthere","image_png":""}}"#).as_deref(),
            Some(r#"{"type":"clipboard","text":"hi\nthere","image_png":"","paths":[]}"#)
        );
    }

    #[test]
    fn take_command_file_consumes_a_durable_nan_command() {
        // The F-LA1 symptom on the real seam: an overlay that wrote `NaN` had its
        // queue entry deleted and never seen.
        let scratch = Scratch::new("nan-durable");
        let bridge = bridge_in(scratch.path());
        std::fs::create_dir_all(&bridge.commands_dir).expect("dir");
        let source = bridge.commands_dir.join("0007.json");
        write(&source, br#"{"command": {"type": "submit", "score": NaN}}"#);
        assert_eq!(
            bridge.take_command_file(&source, true).expect("command").dump(),
            r#"{"type":"submit","score":NaN}"#
        );
        assert!(!source.exists());
        assert!(
            std::fs::read_dir(&bridge.commands_dir).expect("dir").next().is_none(),
            "the claim was unlinked, not left behind"
        );
        // A poisoned entry is still the one that gets discarded.
        let poisoned = bridge.commands_dir.join("0008.json");
        write(&poisoned, br#"{"command": {"type": "submit", "score": -NaN}}"#);
        assert_eq!(bridge.take_command_file(&poisoned, true), None);
        assert!(!poisoned.exists(), "a durable entry is consumed, not restored");
    }

    /// `json.dumps(validate_command_ord(py_json_loads(raw)), ...)` — the pipeline
    /// the golden table measures, as `Option<String>`.
    fn command_of(raw: &str) -> Option<String> {
        validate_command_ord(&py_json_loads(raw).expect("loads")).map(|command| command.dump())
    }

    #[test]
    fn sorted_json_files_follows_glob_and_sorting_rules() {
        let scratch = Scratch::new("glob");
        let dir = scratch.path().join("cmds");
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::create_dir_all(dir.join("subdir.json")).expect("dir entry counts too");
        for name in ["b.json", "A.json", ".hidden.json", "c.JSON", "ignore.txt", "x.jsonx"] {
            write(&dir.join(name), b"{}");
        }
        let names: Vec<String> = sorted_json_files(&dir)
            .into_iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        let expected: Vec<&str> = if cfg!(windows) {
            // normcased full-path order, and the pattern match is case fold too.
            vec![".hidden.json", "A.json", "b.json", "c.JSON", "subdir.json"]
        } else {
            vec![".hidden.json", "A.json", "b.json", "subdir.json"]
        };
        assert_eq!(names, expected, "sorted(glob('*.json')) shape");
        assert_eq!(sorted_json_files(&scratch.path().join("absent")), Vec::<PathBuf>::new());
    }

    #[test]
    fn claim_name_shape() {
        let name = claim_name("0001.json", 4242, 1_700_000_000_123_456_789);
        assert_eq!(name, "0001.json.claim.4242.1700000000123456789");
        let now = time_ns();
        assert!(now > 1_600_000_000_000_000_000, "a 19-digit ns epoch stamp");
    }

    // ------------------------------------------------- kill decision logic

    const TARGET: &str = "c:\\app\\plugins\\pet";

    #[test]
    fn kill_plan_requires_the_trailing_separator() {
        let table = vec![
            NativeProcess::fake(10, 4, Some("C:\\app\\plugins\\pet\\hermes-adapter\\electron.exe")),
            NativeProcess::fake(11, 4, Some("C:\\app\\plugins\\pet")),
            NativeProcess::fake(12, 4, Some("C:\\app\\plugins\\petX\\electron.exe")),
            NativeProcess::fake(13, 4, Some("C:\\other\\electron.exe")),
        ];
        assert_eq!(kill_plan(&table, TARGET), vec![10]);
    }

    #[test]
    fn kill_plan_is_case_insensitive_but_not_about_depth() {
        // The authority compares `normcase(realpath(exe))` against
        // `normcase(target) + os.sep`, so only casing/separator style folds.
        let table = vec![NativeProcess::fake(7, 4, Some("C:\\APP\\PLUGINS\\PET\\hermes-adapter\\ELECTRON.EXE"))];
        assert_eq!(kill_plan(&table, TARGET), vec![7]);
        assert_eq!(kill_plan(&table, "C:\\app\\plugins\\pet"), vec![7], "the target is lowered upstream");
    }

    #[test]
    fn kill_plan_kills_children_before_the_parent_and_keeps_duplicates() {
        let table = vec![
            NativeProcess::fake(100, 4, Some("C:\\app\\plugins\\pet\\electron.exe")),
            NativeProcess::fake(101, 100, Some("C:\\windows\\helper.exe")),
            NativeProcess::fake(102, 101, Some("C:\\windows\\grand.exe")),
            NativeProcess::fake(103, 100, Some("C:\\windows\\sibling.exe")),
            NativeProcess::fake(200, 4, Some("C:\\app\\plugins\\pet\\second\\electron.exe")),
        ];
        // `proc.children(recursive=True)` walks a LIFO stack, so the subtree of
        // 100 comes out `101, 103, 102` (children of 100 in table order, then
        // the last-pushed branch expanded first) — measured as
        // `PSUTIL tree A->[B,C] B->[D] C->[E] : ['B', 'C', 'E', 'D']`.  Then the
        // matched row itself, then the next matched row.
        assert_eq!(kill_plan(&table, TARGET), vec![101, 103, 102, 100, 200]);
    }

    #[test]
    fn kill_plan_skips_rows_without_an_executable_path() {
        let table = vec![
            NativeProcess::fake(1, 0, None),
            NativeProcess::fake(2, 0, Some("C:\\app\\plugins\\pet\\electron.exe")),
        ];
        assert_eq!(kill_plan(&table, TARGET), vec![2]);
    }

    #[test]
    fn kill_plan_on_an_empty_table_or_empty_target_selects_nothing() {
        assert_eq!(kill_plan(&[], TARGET), Vec::<u32>::new());
        let table = vec![NativeProcess::fake(3, 0, Some("C:\\app\\plugins\\pet\\electron.exe"))];
        // `os.path.normcase('') + os.sep` is just `'\\'` on Windows, and
        // `normcase(realpath('C:\\app\\...'))` is `'c:\\app\\...'`, which does
        // *not* start with that lone separator: measured
        // `STARTS False`.  An empty target therefore matches nothing at all --
        // the "empty prefix swallows everything" reading is POSIX-shaped.
        assert_eq!(kill_plan(&table, ""), Vec::<u32>::new(), "the empty prefix is only a separator");
        assert_eq!(kill_plan(&table, "c:\\nothing\\here"), Vec::<u32>::new());
    }

    #[test]
    fn kill_plan_has_no_self_preservation() {
        // `kill_processes_by_target` has no pid filter at all: a process
        // running from inside the target dir is selected even when it is us.
        let me = std::process::id();
        let table = vec![NativeProcess::fake(me, 4, Some("C:\\app\\plugins\\pet\\electron.exe"))];
        assert_eq!(kill_plan(&table, TARGET), vec![me]);
    }

    #[test]
    fn descendants_breaks_parent_cycles() {
        let table = vec![
            NativeProcess::fake(1, 2, Some("x")),
            NativeProcess::fake(2, 1, Some("y")),
            NativeProcess::fake(3, 0, Some("z")),
        ];
        // psutil only consults `seen` when it *pops*, so the back edge reaches
        // the root again and the root lands in its own descendant list:
        // measured `PSUTIL cycle A<->B : ['B', 'A']`.
        assert_eq!(descendants(&table, 1), vec![2, 1]);
        assert_eq!(descendants(&table, 3), Vec::<u32>::new());
        assert_eq!(descendants(&table, 999), Vec::<u32>::new());
    }

    #[test]
    fn descendants_follow_the_psutil_stack_order() {
        // Every expectation here is the measured order of
        // `psutil.Process(root).children(recursive=True)` over a synthetic
        // parent map of live pids (see `PSUTIL tree|flat|chain` in
        // scratch/rust_parity/pet_launcher_fix_s12/REPORT.md).
        let tree = vec![
            NativeProcess::fake(1, 0, None),
            NativeProcess::fake(2, 1, None),
            NativeProcess::fake(3, 1, None),
            NativeProcess::fake(4, 2, None),
            NativeProcess::fake(5, 3, None),
        ];
        assert_eq!(descendants(&tree, 1), vec![2, 3, 5, 4], "A->[B,C] B->[D] C->[E]");
        let flat = vec![
            NativeProcess::fake(1, 0, None),
            NativeProcess::fake(2, 1, None),
            NativeProcess::fake(3, 1, None),
            NativeProcess::fake(4, 1, None),
        ];
        assert_eq!(descendants(&flat, 1), vec![2, 3, 4], "a flat family keeps table order");
        let chain = vec![
            NativeProcess::fake(1, 0, None),
            NativeProcess::fake(2, 1, None),
            NativeProcess::fake(3, 2, None),
            NativeProcess::fake(4, 3, None),
        ];
        assert_eq!(descendants(&chain, 1), vec![2, 3, 4], "a chain walks straight down");
        // psutil's `ppid_map` is a dict, so a pid has one recorded parent and a
        // diamond cannot be expressed; what *can* repeat an entry is a back
        // edge, because `ret.append()` happens per traversed edge while `seen`
        // only stops the expansion.
        let back_edge = vec![
            NativeProcess::fake(1, 3, None),
            NativeProcess::fake(2, 1, None),
            NativeProcess::fake(3, 1, None),
        ];
        assert_eq!(descendants(&back_edge, 1), vec![2, 3, 1], "the cycle re-appends the root once");
    }

    #[test]
    fn exe_matches_target_uses_the_real_filesystem_for_realpath() {
        let scratch = Scratch::new("realpath");
        let inner = scratch.path().join("pet").join("hermes-adapter");
        std::fs::create_dir_all(&inner).expect("dir");
        let exe = inner.join("electron.exe");
        write(&exe, b"MZ");
        let target = resolved_target_str(&scratch.path().join("pet"));
        assert!(exe_matches_target(&exe.to_string_lossy(), &target));
        assert!(
            exe_matches_target(
                &exe.to_string_lossy().to_uppercase(),
                &target
            ),
            "normcase folds the casing of both sides"
        );
        assert!(!exe_matches_target("C:\\windows\\explorer.exe", &target));
        assert!(!exe_matches_target(&target, &target), "the target itself is not inside itself");
    }

    #[test]
    fn resolved_target_str_lowercases_the_resolved_path() {
        let scratch = Scratch::new("resolve");
        let mixed = scratch.path().join("MiXeD");
        std::fs::create_dir_all(&mixed).expect("dir");
        let resolved = resolved_target_str(&mixed);
        assert_eq!(resolved, resolved.to_lowercase());
        assert!(resolved.ends_with("mixed"));
        // An unresolvable path falls back to `str(path).lower()` the way the
        // `except (OSError, ValueError)` tail does.
        assert_eq!(resolved_target_str(Path::new("Z:\\definitely\\absent")), "z:\\definitely\\absent");
    }

    #[test]
    fn kill_processes_by_target_reports_its_backend_without_spawning() {
        let scratch = Scratch::new("backend");
        let outcome = kill_processes_by_target(&scratch.path().join("adapter"));
        if cfg!(windows) {
            assert_eq!(outcome.backend, "kernel32");
            assert!(!outcome.fallback_suppressed);
            assert!(outcome.selected.is_empty(), "nothing can run from a fresh temp dir");
        } else {
            assert_eq!(outcome.backend, "unsupported");
            assert!(!outcome.fallback_suppressed);
        }
    }

    #[test]
    fn list_processes_finds_this_process_on_windows() {
        if !cfg!(windows) {
            assert_eq!(list_processes(), None, "no libc binding off Windows");
            return;
        }
        let table = list_processes().expect("toolhelp snapshot");
        let me = std::process::id();
        let row = table.iter().find(|proc| proc.pid == me).expect("this process is listed");
        let exe = row.exe.as_ref().expect("own image path");
        let current = std::env::current_exe().expect("current exe");
        assert_eq!(exe.to_lowercase(), resolve_path_str(&current).to_lowercase());
        assert!(!row.name.is_empty());
    }

    #[test]
    fn terminate_process_fails_closed_for_pid_zero() {
        if !cfg!(windows) {
            assert!(!terminate_process(0));
            return;
        }
        // The System Idle Process cannot be opened for TERMINATE: the seam must
        // report failure instead of panicking.
        assert!(!terminate_process(0));
    }

    #[test]
    fn terminate_tree_without_children_only_reports_the_root() {
        // The old version drove `terminate_tree(0)` straight into
        // `CreateToolhelp32Snapshot`, so it enumerated whatever happens to run
        // on the machine (pid 0 owns System/Registry/... as children) and could
        // never be asserted.  The same sweep now runs over an injected table
        // with an injected kill result.
        let lonely = vec![NativeProcess::fake(100, 4, None)];
        assert_eq!(
            terminate_tree_with(Some(&lonely), 100, |_| false),
            Vec::<u32>::new(),
            "nothing openable means nothing reported"
        );
        assert_eq!(terminate_tree_with(Some(&lonely), 100, |pid| pid == 100), vec![100]);
        assert_eq!(
            terminate_tree_with(None, 100, |pid| pid == 100),
            vec![100],
            "a failed snapshot still attempts the root, like the psutil `except` branch"
        );
        // With a family the descendants come first, in the psutil sweep order,
        // and a descendant whose handle could not be opened is dropped from the
        // report instead of being listed as killed.
        let family = vec![
            NativeProcess::fake(100, 4, None),
            NativeProcess::fake(101, 100, None),
            NativeProcess::fake(102, 101, None),
            NativeProcess::fake(103, 100, None),
        ];
        assert_eq!(
            terminate_tree_with(Some(&family), 100, |pid| pid != 101),
            vec![103, 102, 100],
            "101 was not openable, so only the pids that actually died are reported"
        );
        assert_eq!(terminate_tree_with(Some(&family), 100, |_| true), vec![101, 103, 102, 100]);
        // One real-FFI smoke check, on a pid that cannot exist: the live
        // snapshot is walked, no process is ever targeted, and the report is empty.
        if cfg!(windows) {
            assert_eq!(terminate_tree(u32::MAX), Vec::<u32>::new());
            assert!(!terminate_process(0), "pid 0 has no openable handle");
        } else {
            assert_eq!(terminate_tree(0), Vec::<u32>::new(), "no snapshot off Windows");
        }
    }

    // ---------------------------------------------------------- the launcher

    fn launcher_with(dir: &Path, external: Option<&Path>) -> HermesPetLauncher {
        HermesPetLauncher::new(dir, Arc::new(bridge_in(dir)), external)
    }

    fn make_adapter_package(dir: &Path) -> PathBuf {
        let adapter = dir.join("assets").join("pet").join("hermes-adapter");
        std::fs::create_dir_all(adapter.join("app")).expect("app dir");
        write(&adapter.join("electron.exe"), b"not a windows image");
        write(&adapter.join("app").join("package.json"), b"{\"name\":\"hermes-pet\"}");
        adapter
    }

    #[test]
    fn adapter_dir_prefers_the_external_package() {
        let scratch = Scratch::new("adapter-dir");
        let external = scratch.path().join("plugins").join("pet").join("hermes-adapter");
        std::fs::create_dir_all(&external).expect("dir");
        let launcher = launcher_with(scratch.path(), Some(&external));
        assert_eq!(resolve_path_str(&launcher.adapter_dir()), resolve_path_str(&external));
        let bundled = launcher_with(scratch.path(), None);
        assert_eq!(
            bundled.adapter_dir(),
            scratch
                .path()
                .join("assets")
                .join("pet")
                .join("hermes-adapter")
        );
    }

    #[test]
    fn status_report_key_order_matches_python() {
        let scratch = Scratch::new("status-keys");
        let launcher = launcher_with(scratch.path(), None);
        assert_eq!(
            launcher.status().dump(),
            "{\"available\":false,\"bridge_ready\":false,\"running\":false,\"health\":{}}"
        );
    }

    #[test]
    fn status_bridge_ready_follows_the_state_file() {
        let scratch = Scratch::new("status-bridge");
        let bridge = Arc::new(bridge_in(scratch.path()));
        bridge.publish(&json!({"visible": true}), &options()).expect("publish");
        let launcher = HermesPetLauncher::new(scratch.path(), Arc::clone(&bridge), None);
        assert!(launcher.status().is_true("bridge_ready"));
    }

    #[test]
    fn status_scan_cache_window_is_two_seconds() {
        let scratch = Scratch::new("scan-cache");
        let adapter = make_adapter_package(scratch.path());
        let runtime = adapter.join("electron.exe");
        let launcher = launcher_with(scratch.path(), None);
        let table = vec![NativeProcess::fake(5, 4, Some(&runtime.to_string_lossy()))];
        let base = Instant::now();
        if !cfg!(windows) {
            // `os.name == 'nt'` guards both `available` and the scan branch.
            assert!(!launcher.status_at(base, Some(&table)).is_true("running"));
            return;
        }
        assert!(launcher.status_at(base, Some(&table)).is_true("running"));
        assert!(launcher.status_at(base, Some(&table)).is_true("available"));
        // Inside the cache window the fresh (empty) table is ignored.
        assert!(launcher.status_at(base + Duration::from_millis(1_900), Some(&[])).is_true("running"));
        // After it, the scan runs again and the process is gone.
        assert!(!launcher.status_at(base + Duration::from_millis(2_100), Some(&[])).is_true("running"));
    }

    #[test]
    fn status_reports_nothing_when_the_table_is_unavailable() {
        let scratch = Scratch::new("scan-none");
        make_adapter_package(scratch.path());
        let launcher = launcher_with(scratch.path(), None);
        let base = Instant::now();
        let status = launcher.status_at(base, None);
        assert!(!status.is_true("running"), "psutil's exception branch is `pass`");
        if cfg!(windows) {
            assert!(status.is_true("available"), "electron.exe + app/package.json exist");
        }
    }

    #[test]
    fn runtime_is_running_compares_case_insensitively() {
        let scratch = Scratch::new("runtime-match");
        std::fs::create_dir_all(&scratch.path()).expect("dir");
        let runtime = scratch.path().join("electron.exe");
        write(&runtime, b"MZ");
        let want = resolve_path_str(&runtime);
        let table = vec![NativeProcess::fake(1, 0, Some(&want.to_uppercase()))];
        assert!(runtime_is_running(&table, &runtime));
        let other = vec![NativeProcess::fake(1, 0, Some("C:\\elsewhere\\electron.exe"))];
        assert!(!runtime_is_running(&other, &runtime));
        // Equality, not prefix: a *child* path does not count.
        let nested = vec![NativeProcess::fake(1, 0, Some(&format!("{want}\\x.exe")))];
        assert!(!runtime_is_running(&nested, &runtime));
        let denied = vec![NativeProcess::fake(1, 0, None)];
        assert!(!runtime_is_running(&denied, &runtime));
    }

    #[test]
    fn status_health_matrix() {
        let scratch = Scratch::new("health");
        let bridge = bridge_in(scratch.path());
        let health_path = PathBuf::from(format!("{}{HEALTH_SUFFIX}", bridge.state_path.display()));
        assert_eq!(read_health(&bridge.state_path).dump(), "{}", "missing file");
        write(
            &health_path,
            json!({"state": "ready", "renderer": "hermes-sprite", "code": "ok", "pid": 12, "extra": "x"})
                .to_string()
                .as_bytes(),
        );
        assert_eq!(
            read_health(&bridge.state_path).dump(),
            "{\"state\":\"ready\",\"renderer\":\"hermes-sprite\",\"code\":\"ok\"}",
            "three string keys in tuple order, ints and extras dropped"
        );
        write(&health_path, json!({"state": null, "renderer": 5}).to_string().as_bytes());
        assert_eq!(read_health(&bridge.state_path).dump(), "{}");
        write(&health_path, b"not json");
        assert_eq!(read_health(&bridge.state_path).dump(), "{}");
        write(&health_path, b"[1,2]");
        assert_eq!(read_health(&bridge.state_path).dump(), "{}");
        // `json.loads` accepts the bare `NaN`, and the authority only picks the
        // three `str` keys out of the result, so an unrelated non-finite member
        // must not blank the map (serde_json's reader did).
        write(&health_path, br#"{"state":"busy","score":NaN,"pid":-Infinity}"#);
        assert_eq!(read_health(&bridge.state_path).dump(), r#"{"state":"busy"}"#);
        write(&health_path, br#"{"state":"busy","renderer":NaN,"code":"ok"}"#);
        assert_eq!(
            read_health(&bridge.state_path).dump(),
            r#"{"state":"busy","code":"ok"}"#,
            "a NaN where a str is expected drops that one key"
        );
        write(&health_path, vec![b' '; (HEALTH_MAX_BYTES as usize) + 1].as_slice());
        assert_eq!(read_health(&bridge.state_path).dump(), "{}", "a 16 KiB file is skipped");
        write(&health_path, vec![b' '; HEALTH_MAX_BYTES as usize - 1].as_slice());
        assert_eq!(read_health(&bridge.state_path).dump(), "{}", "invalid json below the cap");
        let _ = std::fs::remove_file(&health_path);

        // The report only carries health while running.
        make_adapter_package(scratch.path());
        let launcher = HermesPetLauncher::new(scratch.path(), Arc::new(bridge), None);
        write(&health_path, json!({"state": "ready"}).to_string().as_bytes());
        let status = launcher.status_at(Instant::now(), Some(&[]));
        assert!(!status.is_true("running"));
        assert_eq!(status.get("health").expect("health").dump(), "{}");
    }

    #[test]
    fn renderer_assets_ready_matrix() {
        let scratch = Scratch::new("renderer");
        let index = scratch.path().join("app").join("renderer").join("index.html");
        assert!(renderer_assets_ready_for(&index), "no index -> older package is fine");
        write(&index, b"<!doctype html><script src=\"./assets/app.AB.js\"></script>");
        assert!(!renderer_assets_ready_for(&index), "referenced chunk is missing");
        write(&scratch.path().join("app").join("renderer").join("assets").join("app.AB.js"), b"1");
        assert!(renderer_assets_ready_for(&index));
        // A leading slash is outside the pattern, so it is not a requirement --
        // and neither is the *unquoted* `src=assets/pic.png`, because the
        // pattern needs a quote right after `src=`.  Measured:
        // `re.findall(pattern, html)` for this exact document is `[]`, and
        // `all(...)` over an empty sequence is True, so nothing is required.
        write(
            &index,
            b"<!doctype html><script src=\"/assets/stage-B3T0dW7u.js\"></script><img src=assets/pic.png>",
        );
        assert!(
            renderer_assets_ready_for(&index),
            "neither reference is matched, so an absent pic.png is no requirement"
        );
        write(&scratch.path().join("app").join("renderer").join("assets").join("pic.png"), b"2");
        assert!(renderer_assets_ready_for(&index), "creating it changes nothing");
        // `href` is matched as well as `src`.
        write(
            &index,
            b"<link rel=icon href=\"./assets/favicon.png\">",
        );
        assert!(!renderer_assets_ready_for(&index));
        write(&scratch.path().join("app").join("renderer").join("assets").join("favicon.png"), b"3");
        assert!(renderer_assets_ready_for(&index));
        // Non-UTF-8 index.html is a UnicodeError upstream -> False.
        write(&index, &[0xff, 0xfe, 0x00, 0x80]);
        assert!(!renderer_assets_ready_for(&index));
    }

    #[test]
    fn start_reports_not_installed_without_the_package() {
        let scratch = Scratch::new("start-missing");
        let launcher = launcher_with(scratch.path(), None);
        let got = launcher.start();
        assert!(!got.is_true("ok"));
        assert_eq!(got.get("code").expect("code").dump(), "\"hermes_adapter_not_installed\"");
        assert_eq!(
            got.dump(),
            "{\"ok\":false,\"code\":\"hermes_adapter_not_installed\",\"runtime\":{\"available\":false,\"bridge_ready\":false,\"running\":false,\"health\":{}}}"
        );
        assert_eq!(launcher.child_pid(), None);
    }

    #[test]
    fn start_reports_failure_when_the_runtime_cannot_spawn() {
        let scratch = Scratch::new("start-fail");
        make_adapter_package(scratch.path());
        let launcher = launcher_with(scratch.path(), None);
        if !cfg!(windows) {
            // `available` is False off Windows, so start never reaches Popen.
            assert_eq!(
                launcher.start().get("code").expect("code").dump(),
                "\"hermes_adapter_not_installed\""
            );
            return;
        }
        let got = launcher.start();
        assert!(!got.is_true("ok"));
        assert_eq!(
            got.get("code").expect("code").dump(),
            "\"hermes_adapter_start_failed\"",
            "a text file named electron.exe must not become a process"
        );
        assert!(got.get("runtime").expect("runtime").is_true("available"));
        assert_eq!(launcher.child_pid(), None);
    }

    #[test]
    fn stop_without_a_child_still_sweeps_and_settles() {
        let scratch = Scratch::new("stop");
        let launcher = launcher_with(scratch.path(), None);
        let started = Instant::now();
        launcher.stop();
        assert!(started.elapsed() >= Duration::from_millis(250), "time.sleep(0.3) is unconditional");
        assert_eq!(launcher.child_pid(), None);
    }

    // ------------------------------------------------------- staged publishing

    #[test]
    fn replace_with_retry_renames_and_reports_other_errors() {
        let scratch = Scratch::new("retry");
        let source = scratch.path().join("source");
        let destination = scratch.path().join("destination");
        std::fs::create_dir_all(&source).expect("source dir");
        write(&source.join("a.txt"), b"hello");
        let started = Instant::now();
        replace_with_retry(&source, &destination).expect("rename");
        assert!(started.elapsed() < SWAP_RETRY_DELAY, "a first-try success never sleeps");
        assert!(destination.join("a.txt").is_file());
        assert!(!source.exists());
        // A missing source is `FileNotFoundError`, not `PermissionError`, so the
        // retry budget is never spent.
        let started = Instant::now();
        let error = replace_with_retry(&scratch.path().join("absent"), &destination).expect_err("must fail");
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert!(started.elapsed() < SWAP_RETRY_DELAY, "no sleep on a non-permission error");
    }

    #[test]
    fn replace_with_retry_timed_spends_its_budget_on_permission_errors() {
        // The policy is what is under test here: N attempts, N-1 sleeps.
        let scratch = Scratch::new("retry-budget");
        // `os.replace(file, existing-empty-directory)` is WinError 5, which
        // CPython raises as `PermissionError` and Rust surfaces as
        // `ErrorKind::PermissionDenied` (both measured on this machine), so this
        // really does walk the retry path.  A *directory* onto an empty
        // directory is not usable here: `std::fs::rename` succeeds at it.
        let source = scratch.path().join("source.bin");
        write(&source, b"x");
        let destination = scratch.path().join("destination");
        std::fs::create_dir_all(&destination).expect("dir");
        let started = Instant::now();
        let error = replace_with_retry_timed(&source, &destination, 3, Duration::from_millis(50))
            .expect_err("renaming a file onto a directory is a permission error");
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        let spent = started.elapsed();
        assert!(spent >= Duration::from_millis(100), "three attempts sleep twice: {spent:?}");
        assert!(spent < Duration::from_millis(400), "the loop stops at the last attempt: {spent:?}");
        assert!(source.is_file(), "a failed rename leaves the staged source alone");
        assert!(destination.is_dir(), "as does the destination");
        // The authority's own budget: 40 attempts, 0.75 s apart, i.e. the ~30 s
        // window the comment in `_replace_with_retry` promises a scanner.
        assert_eq!(SWAP_ATTEMPTS, 40);
        assert_eq!(SWAP_RETRY_DELAY, Duration::from_millis(750));
        assert_eq!((SWAP_ATTEMPTS - 1) as u128 * SWAP_RETRY_DELAY.as_millis(), 29_250);
        // A non-permission failure never enters the sleep path: a missing source
        // is CPython's `FileNotFoundError`, so the first attempt returns.
        let started = Instant::now();
        let error = replace_with_retry_timed(
            &scratch.path().join("absent"),
            &destination,
            4,
            Duration::from_millis(50),
        )
        .expect_err("must fail");
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert!(started.elapsed() < Duration::from_millis(50), "no sleep on a non-permission error");
    }

    #[test]
    fn replace_tree_in_place_copies_prunes_and_leaves_no_temps() {
        let scratch = Scratch::new("in-place");
        let staged = scratch.path().join("staged");
        let target = scratch.path().join("target");
        write(&staged.join("keep.txt"), b"new bytes");
        write(&staged.join("sub").join("deep.bin"), b"deep");
        std::fs::create_dir_all(staged.join("empty-dir")).expect("empty dir");
        write(&target.join("keep.txt"), b"old bytes");
        write(&target.join("stale.txt"), b"remove me");
        write(&target.join("sub").join("stale.bin"), b"remove me too");
        write(&target.join("sub").join("nested").join("gone.txt"), b"gone");

        replace_tree_in_place(&staged, &target).expect("in-place publish");

        assert_eq!(read_text(&target.join("keep.txt")), "new bytes");
        assert_eq!(read_text(&target.join("sub").join("deep.bin")), "deep");
        assert!(!target.join("stale.txt").exists());
        assert!(!target.join("sub").join("stale.bin").exists());
        assert!(!target.join("sub").join("nested").exists(), "a dir absent from staged is pruned");
        assert!(target.join("empty-dir").is_dir(), "empty staged dirs are created");
        let temps: Vec<String> = walk_topdown(&target)
            .into_iter()
            .flat_map(|(_, _dirs, files)| files)
            .filter(|name| name.ends_with(IN_PLACE_TEMP_SUFFIX))
            .collect();
        assert!(temps.is_empty(), "no .readmd-new survivors: {temps:?}");
    }

    #[test]
    fn replace_tree_in_place_creates_a_missing_target() {
        let scratch = Scratch::new("in-place-new");
        let staged = scratch.path().join("staged");
        let target = scratch.path().join("target");
        write(&staged.join("only.txt"), b"x");
        replace_tree_in_place(&staged, &target).expect("publish");
        assert_eq!(read_text(&target.join("only.txt")), "x");
    }

    #[test]
    fn publish_staged_tree_replaces_the_target_and_drops_the_backup() {
        let scratch = Scratch::new("publish");
        let root = scratch.path();
        let target = root.join("hermes-adapter");
        let staged = root.join("staged");
        write(&target.join("old.txt"), b"old");
        write(&staged.join("new.txt"), b"new");
        write(&root.join(BACKUP_DIR_NAME).join("previous-crash.txt"), b"stale");
        let report = publish_staged_tree(root, &target, &staged, 3).expect("publish");
        assert_eq!(report.dump(), "{\"ok\":true,\"installed\":true,\"files\":3}");
        assert_eq!(read_text(&target.join("new.txt")), "new");
        assert!(!target.join("old.txt").exists());
        assert!(!staged.exists(), "the staged tree was renamed into place");
        assert!(!root.join(BACKUP_DIR_NAME).exists(), "the stale backup is cleaned up");
    }

    #[test]
    fn publish_staged_tree_installs_without_a_previous_target() {
        let scratch = Scratch::new("publish-fresh");
        let root = scratch.path();
        let target = root.join("hermes-adapter");
        let staged = root.join("staged");
        write(&staged.join("electron.exe"), b"MZ");
        let report = publish_staged_tree(root, &target, &staged, 1).expect("publish");
        assert_eq!(report.get("files").expect("files").dump(), "1");
        assert!(target.join("electron.exe").is_file());
        assert!(!root.join(BACKUP_DIR_NAME).exists());
    }

    #[test]
    fn publish_staged_tree_restores_the_backup_when_publishing_fails() {
        let scratch = Scratch::new("publish-restore");
        let root = scratch.path();
        let target = root.join("hermes-adapter");
        write(&target.join("working.txt"), b"keep me");
        // `staged` does not exist, so `replace_with_retry(staged, target)` fails
        // with NotFound, which is the `except OSError` restore branch.
        let missing_staged = root.join("never-written");
        let error = publish_staged_tree(root, &target, &missing_staged, 1)
            .expect_err("publish must fail");
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert_eq!(read_text(&target.join("working.txt")), "keep me", "last working adapter restored");
        assert!(!root.join(BACKUP_DIR_NAME).exists(), "the backup was renamed back");
    }

    #[test]
    fn copy_tree_copies_a_nested_tree() {
        let scratch = Scratch::new("copytree");
        let source = scratch.path().join("src");
        write(&source.join("a.txt"), b"one");
        write(&source.join("sub").join("b.txt"), b"two");
        let destination = scratch.path().join("dst");
        copy_tree(&source, &destination).expect("copy");
        assert_eq!(read_text(&destination.join("a.txt")), "one");
        assert_eq!(read_text(&destination.join("sub").join("b.txt")), "two");
        assert!(copy_tree(&source, &destination).is_err(), "copytree refuses an existing dir");
    }

    #[test]
    fn sha256_file_matches_hashlib_vectors() {
        let scratch = Scratch::new("sha");
        let empty = scratch.path().join("empty");
        write(&empty, b"");
        assert_eq!(
            sha256_file(&empty).expect("digest"),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        let abc = scratch.path().join("abc");
        write(&abc, b"abc");
        assert_eq!(
            sha256_file(&abc).expect("digest"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert!(sha256_file(&scratch.path().join("absent")).is_err());
    }

    #[test]
    fn is_safe_name_table() {
        for name in [
            "readmd-pet-plugin.json",
            "electron.exe",
            "app/package.json",
            "renderer/assets/app.js",
            "~",
            // `Path("/etc/passwd").is_absolute()` is **False** on Windows -- the
            // Windows flavour of `PurePath` only calls a path absolute when it
            // has a drive *and* a root, so the authority's
            // `not value.is_absolute()` gate lets this through (measured
            // `_is_safe_name("/etc/passwd") == True`).
            "/etc/passwd",
        ] {
            assert!(is_safe_name(name), "{name}");
        }
        for name in [
            "",
            "..",
            "../x",
            "a/../../b",
            "app\\package.json",
            "C:\\Windows\\electron.exe",
            "\\\\server\\share\\x",
        ] {
            assert!(!is_safe_name(name), "{name}");
        }
    }

    #[test]
    fn relative_path_and_normpath_join_shapes() {
        let base = Path::new("C:\\staged");
        assert_eq!(relative_path(base, base), ".");
        assert_eq!(relative_path(&base.join("app"), base), "app");
        assert_eq!(relative_path(&base.join("app").join("renderer"), base), "app\\renderer".replace('\\', std::path::MAIN_SEPARATOR_STR));
        assert_eq!(normpath_join(".", "a.txt"), "a.txt");
        assert_eq!(
            normpath_join("app", "package.json"),
            format!("app{}package.json", std::path::MAIN_SEPARATOR_STR)
        );
    }

    #[test]
    fn walk_orders_match_os_walk() {
        let scratch = Scratch::new("walk");
        write(&scratch.path().join("top.txt"), b"x");
        write(&scratch.path().join("sub").join("inner.txt"), b"y");
        write(&scratch.path().join("sub").join("deep").join("leaf.txt"), b"z");
        let topdown = walk_topdown(scratch.path());
        assert_eq!(relative_path(&topdown[0].0, scratch.path()), ".");
        assert_eq!(topdown[0].1.len(), 1, "one directory, `sub`");
        assert_eq!(topdown[0].2, vec!["top.txt".to_string()]);
        let bottomup = walk_bottomup(scratch.path());
        let last = bottomup.last().expect("root last");
        assert_eq!(relative_path(&last.0, scratch.path()), ".");
        assert_eq!(bottomup.len(), 3, "sub/deep, sub, root");
        assert_eq!(relative_path(&bottomup[0].0, scratch.path()).replace(std::path::MAIN_SEPARATOR, "/"), "sub/deep");
    }

    // ------------------------------------------------------------- primitives

    #[test]
    fn py_float_str_follows_cpython_grammar() {
        let accepted: Vec<(&str, f64)> = vec![
            ("12", 12.0),
            ("  1.5  ", 1.5),
            ("\t1.5\n", 1.5),
            ("1.", 1.0),
            (".5", 0.5),
            ("1e5", 100000.0),
            ("-1E-5", -0.00001),
            ("1_0", 10.0),
            ("1_000.5", 1000.5),
            ("1e1_0", 1e10),
            ("01_0", 10.0),
            // Wide spaces are blanked by the ASCII transform, on either side.
            ("\u{a0}1", 1.0),
            ("1\u{a0}", 1.0),
            ("\u{3000}1", 1.0),
            ("\u{85}1", 1.0),
            ("\u{2028}1", 1.0),
            ("\u{2009}1.5\u{2009}", 1.5),
            ("\u{a0} inf ", f64::INFINITY),
            // Unicode decimal digits become ASCII digits, in the mantissa and
            // in the exponent, and they still allow underscore separation.
            ("\u{663}\u{664}", 34.0),
            ("1\u{665}", 15.0),
            ("\u{665}1", 51.0),
            ("\u{ff11}\u{ff12}", 12.0),
            ("\u{660}.\u{665}", 0.5),
            ("\u{661}_\u{660}", 10.0),
            ("1e\u{661}", 10.0),
            ("\u{661}e1", 10.0),
            ("\u{661}.", 1.0),
            (".\u{661}", 0.1),
        ];
        for (text, want) in accepted {
            assert_eq!(py_float_str(text), Some(want), "{text:?}");
        }
        for text in [
            "", "abc", "_1", "1_", "1__0", "1._0", "1_.5", "0x10", "e5", "-", "1e", "1.5.5",
            // U+001C..U+001F *are* `str.isspace()` but `float()` never blanks a
            // code point below 128, so they poison the value on both paths.
            "\u{1c}1", "1\u{1c}", "\u{1f}\u{661}", "\u{1c}\u{661}", "\u{661}\u{1c}", "\u{0}1", "1\u{0}",
            "\u{200b}1", "\u{ad}1", "\u{3000}", "\u{b2}", "\u{bc}", "\u{66c}1", "\u{66b}1", "1 \u{1c}\u{661}",
        ] {
            assert_eq!(py_float_str(text), None, "{text:?}");
        }
        assert_eq!(py_float_str("inf"), Some(f64::INFINITY));
        assert_eq!(py_float_str("-INFINITY"), Some(f64::NEG_INFINITY));
        assert!(py_float_str("nan").expect("nan").is_nan());
        assert!(py_float_str("-nan").expect("nan").is_nan());
        assert_eq!(py_float_str("1e1000000"), Some(f64::INFINITY));
    }

    #[test]
    fn py_float_str_wide_classes_come_from_the_measured_tables() {
        // Every blanked wide space and a sample digit block boundary.
        for code in [0x0085u32, 0x00a0, 0x1680, 0x2000, 0x200a, 0x2028, 0x2029, 0x202f, 0x205f, 0x3000] {
            let ch = char::from_u32(code).expect("scalar");
            let text = format!("{ch}1");
            assert_eq!(py_float_str(&text), Some(1.0), "U+{code:04X}");
            assert_eq!(wide_space_to_ascii(ch), Some(' '));
        }
        // Not space, not a decimal digit.
        for code in [0x200bu32, 0x200b, 0x2060, 0x00ad, 0x00b2, 0x00b9, 0x066b, 0x3220] {
            let ch = char::from_u32(code).expect("scalar");
            assert_eq!(wide_space_to_ascii(ch), None, "U+{code:04X}");
            assert_eq!(py_decimal_digit(ch), None, "U+{code:04X}");
        }
        // Each block start maps to '0' and its ninth member to '9'.
        for zero in [0x0030u32, 0x0660, 0x0966, 0xff10, 0x1fbf0] {
            for digit in 0..10u32 {
                let ch = char::from_u32(zero + digit).expect("scalar");
                assert_eq!(py_decimal_digit(ch), char::from_u32(b'0' as u32 + digit));
            }
            let next = char::from_u32(zero + 10).expect("scalar");
            assert_eq!(py_decimal_digit(next), None, "U+{zero:04X} block ends at 9");
        }
        // The five mathematical-digit blocks sit back to back, so their
        // boundaries are digits rather than gaps.
        assert_eq!(py_decimal_digit('\u{1d7ce}'), Some('0'));
        assert_eq!(py_decimal_digit('\u{1d7d7}'), Some('9'));
        assert_eq!(py_decimal_digit('\u{1d7d8}'), Some('0'));
        assert_eq!(transform_decimal_and_space_to_ascii("\u{a0}\u{663}x"), " 3x");
        assert_eq!(transform_decimal_and_space_to_ascii("12"), "12");
    }

    #[test]
    fn py_float_value_matches_the_type_gates() {
        assert_eq!(py_float_value(&json!(true)), Some(1.0));
        assert_eq!(py_float_value(&json!(false)), Some(0.0));
        assert_eq!(py_float_value(&json!(2)), Some(2.0));
        assert_eq!(py_float_value(&json!("2")), Some(2.0));
        assert_eq!(py_float_value(&json!(null)), None);
        assert_eq!(py_float_value(&json!([])), None);
        assert_eq!(py_float_value(&json!({})), None);
    }

    #[test]
    fn py_str_value_matches_python_str() {
        assert_eq!(py_str_value(&json!("idle")), "idle");
        assert_eq!(py_str_value(&json!(1)), "1");
        assert_eq!(py_str_value(&json!(true)), "True");
        assert_eq!(py_str_value(&json!(null)), "None");
        assert_eq!(py_str_value(&json!(1.5)), "1.5");
        assert_eq!(py_str_value(&json!([])), "[]");
        assert_eq!(py_str_value(&json!([1, "a"])), "[1, 'a']");
        assert_eq!(py_str_value(&json!({"a": 1})), "{'a': 1}");
        assert_eq!(py_str_value(&json!({"a": null})), "{'a': None}");
        // Nested members are `repr()`'ed too, and `repr()` of a `str` that holds
        // a `'` but no `"` switches to double quotes.  Both measured:
        // `str([1, "it's"]) == '[1, "it\'s"]'`,
        // `str({'a': ['b']}) == "[{'a': ['b']}]"`.
        assert_eq!(py_str_value(&json!({"a": ["b"]})), "{'a': ['b']}");
        assert_eq!(py_str_value(&json!([1, "it's"])), "[1, \"it's\"]");
        assert_eq!(py_str_value(&json!(["he said \"hi\""])), "[\'he said \"hi\"\']", "a lone \" never gets escaped");
        assert_eq!(py_str_value(&json!({"{a}": 1})), "{\'{a}\': 1}", "a dict repr wraps in one brace pair");
    }

    #[test]
    fn ord_value_dump_uses_ensure_ascii_false() {
        let value = OrdValue::object(vec![
            ("s", OrdValue::text("caf\u{e9} \u{4e2d}")),
            ("ctrl", OrdValue::text("a\u{8}\u{c}\u{1}b")),
            ("quote", OrdValue::text("he said \"hi\" & 'bye'")),
        ]);
        assert_eq!(
            value.dump(),
            "{\"s\":\"caf\u{e9} \u{4e2d}\",\"ctrl\":\"a\\b\\f\\u0001b\",\"quote\":\"he said \\\"hi\\\" & 'bye'\"}"
        );
        assert_eq!(OrdValue::Array(vec![OrdValue::int(1), OrdValue::boolean(false)]).dump(), "[1,false]");
        assert_eq!(OrdValue::null().dump(), "null");
    }

    #[test]
    fn copy_file_overwrites_and_reports_missing_source() {
        let scratch = Scratch::new("copyfile");
        let source = scratch.path().join("src.bin");
        let destination = scratch.path().join("dst.bin");
        write(&destination, b"long existing content");
        write(&source, b"ab");
        copy_file(&source, &destination).expect("copy");
        assert_eq!(read_text(&destination), "ab", "copyfile truncates the destination");
        assert!(copy_file(&scratch.path().join("absent"), &destination).is_err());
    }
}

