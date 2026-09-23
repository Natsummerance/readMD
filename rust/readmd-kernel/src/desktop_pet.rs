//! Desktop / pet-runtime plumbing (pure decision logic).
//!
//! Cluster-2 replication lane `plugin-desktop-s1`.  Python authority: `readmd.py`.
//!
//! Ported callables — the *decision* and observable result shapes, not the GUI
//! toolkit / filesystem side effects:
//!   * `_drain_pet_command`        (readmd.py:5620) — the bridge command router.
//!   * `_open_pet_clipboard`       (readmd.py:5746) — clipboard-action splitter.
//!   * `_publish_pet_runtime`      (readmd.py:5119) — the pure renderer /
//!     animation / slug decisions.  The spritesheet base64 + geometry assembly
//!     lives in `parity_pets.rs`/`batch2.rs` (host IO) and is not duplicated.
//!   * `_start_pet_fullscreen_loop`(readmd.py:5715) — the change-detection state
//!     machine only; the daemon thread is host scheduling.
//!   * `_start_tray`               (readmd.py:6884) — the icon-candidate
//!     selection only; the pystray menu / callbacks / `window.show()` are the OS
//!     tray owned by `main.rs`.
//!
//! Side-effectful steps (`push_pet_menu`, `save_settings`, `_queue_pet_drop`,
//! file writes, `push_control`) are returned as an instruction for a later
//! serialised lane to execute, never performed here.

use serde_json::Value;

/// Python truthiness for a JSON value (`bool(x)` / `x or y`).
pub fn py_truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => match (n.as_f64(), n.as_i64(), n.as_u64()) {
            (Some(f), _, _) => f != 0.0 && !f.is_nan(),
            (_, Some(i), _) => i != 0,
            (_, _, Some(u)) => u != 0,
            _ => true,
        },
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

/// `command.get(k) or ''` 的**三种**状态（对齐 readmd.py:5755-5756）：
///   * 键缺失 / 值为假 -> `Empty`（Python 拿到 `''`）；
///   * 真值字符串 -> `Str`；
///   * 真值但**非字符串** -> `NonStr(原对象)`。Python 会把这个对象原样留在
///     `image` / `content` 上，随后 `.encode('ascii')`、`content + '\n\n'` 或
///     `_write_md` 里的 `f.write(content)` 抛出 AttributeError / TypeError；
///     旧实现用 `as_str().unwrap_or("")` 把它塌缩成 `""`，失败模式完全不同。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OrVal<'a> {
    Empty,
    Str(&'a str),
    NonStr(&'a Value),
}

fn or_value(v: Option<&Value>) -> OrVal<'_> {
    match v {
        Some(x) if py_truthy(x) => match x {
            Value::String(s) => OrVal::Str(s),
            _ => OrVal::NonStr(x),
        },
        _ => OrVal::Empty,
    }
}

/// `type(x).__name__` 在 JSON 值域上的取值。
fn py_type_name(v: &Value) -> &'static str {
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

// 以下三条消息全部由本机 CPython 实测得到（见 scratch/rust_parity/wstate_pet_fix_s12/REPORT.md）。

fn concat_type_error(v: &Value) -> String {
    match py_type_name(v) {
        "list" => "can only concatenate list (not \"str\") to list".to_string(),
        t => format!("unsupported operand type(s) for +: '{t}' and 'str'"),
    }
}

fn write_type_error(v: &Value) -> String {
    format!("write() argument must be str, not {}", py_type_name(v))
}

fn encode_attribute_error(v: &Value) -> String {
    format!("'{}' object has no attribute 'encode'", py_type_name(v))
}

// ------------------------------------------------------- _drain_pet_command

/// What one drained bridge command tells the host to do.  Mirrors the exact
/// branch order of `_drain_pet_command`; the payloads are pure data so the
/// router itself is unit-testable.
#[derive(Debug, Clone, PartialEq)]
pub enum PetCommand {
    /// `if not command: return None` — no command queued.
    Nothing,
    /// `type == 'ready'` → publish runtime, echo `{'type':'ready','ok':True}`.
    Ready,
    /// `type == 'open-menu'` → `push_pet_menu()`, echo `ok`.
    OpenMenu,
    /// `type == 'bounds'` → `save_settings({'pet_bounds': bounds})`.
    Bounds { bounds: Value },
    /// `type == 'scale'` → `save_settings({'pet_scale': scale})`.
    Scale { scale: Value },
    /// `type == 'clipboard'` → delegate to `_open_pet_clipboard(command)`.
    Clipboard,
    /// Any other `type` (including a missing one) → echo the command verbatim.
    Passthrough(Value),
    /// `type == 'drop'` → `_queue_pet_drop(command.get('paths', []))`.
    Drop { paths: Value },
    /// `command['bounds']` / `command['scale']` KeyError — Python raises, the
    /// caller's loop swallows + logs.  Surfaced rather than fabricated.
    MissingKey(&'static str),
}

/// `_drain_pet_command` (readmd.py:5620) as a pure router over one bridge
/// command value.
pub fn route_pet_command(command: &Value) -> PetCommand {
    // `if not command` — null, empty dict/list/str, 0, false.
    let falsy = match command {
        Value::Null => true,
        Value::Bool(b) => !*b,
        Value::Number(n) => n.as_f64().map(|f| f == 0.0).unwrap_or(false),
        Value::String(s) => s.is_empty(),
        Value::Array(a) => a.is_empty(),
        Value::Object(o) => o.is_empty(),
    };
    if falsy {
        return PetCommand::Nothing;
    }
    let kind = command.get("type").and_then(Value::as_str);
    match kind {
        Some("ready") => PetCommand::Ready,
        Some("open-menu") => PetCommand::OpenMenu,
        Some("bounds") => match command.get("bounds") {
            Some(v) => PetCommand::Bounds { bounds: v.clone() },
            None => PetCommand::MissingKey("bounds"),
        },
        Some("scale") => match command.get("scale") {
            Some(v) => PetCommand::Scale { scale: v.clone() },
            None => PetCommand::MissingKey("scale"),
        },
        Some("clipboard") => PetCommand::Clipboard,
        Some("drop") => {
            // `_queue_pet_drop(command.get('paths', []))` — absent key defaults to [].
            let paths = command.get("paths").cloned().unwrap_or_else(|| Value::Array(vec![]));
            PetCommand::Drop { paths }
        }
        // `if command.get('type') != 'drop': return command` — any other type,
        // including a missing one, echoes the whole command back.
        _ => PetCommand::Passthrough(command.clone()),
    }
}

// ------------------------------------------------------ _open_pet_clipboard

const PNG_MAGIC: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

/// `_open_pet_clipboard` result decision.  `accepted`, `code` and `batch` map
/// one-to-one onto the Python dict fields; the file writes and `push_control`
/// are left to the caller (it owns `path`), so the pure contract is captured
/// without touching the filesystem.
#[derive(Debug, Clone, PartialEq)]
pub enum ClipboardOutcome {
    /// neither text nor image and no accepted drop → `{type:clipboard, accepted:0}`.
    Empty,
    /// neither text nor image but drop accepted → `{type:clipboard, accepted, batch}`.
    DropOnly { accepted: usize },
    /// image decoded but not a PNG → `{..., accepted:0, code:invalid_clipboard_image}`.
    InvalidImage,
    /// image raised (bad base64 / non-ascii / write error); `drop_accepted>0`
    /// also carries the batch.
    ImageError { drop_accepted: usize },
    /// a Markdown note is written; `content` is the exact body, `accepted =
    /// 1 + drop_accepted`, `batch` present only when a drop was accepted.
    Note {
        content: String,
        accepted: usize,
        has_batch: bool,
    },
    /// Python's exception escapes `_open_pet_clipboard` entirely: the handler is
    /// `except (OSError, ValueError, UnicodeError, binascii.Error)` and measured
    /// CPython shows `TypeError`/`AttributeError` are NOT members of that tuple,
    /// so no `{'type':'clipboard', ...}` response is produced at all.
    Raises { exc: &'static str, message: String },
}

impl ClipboardOutcome {
    /// The Python response's `code` field (`None` = the key is absent there).
    pub fn code(&self) -> Option<&'static str> {
        match self {
            ClipboardOutcome::InvalidImage => Some("invalid_clipboard_image"),
            ClipboardOutcome::ImageError { .. } => Some("clipboard_image_write_failed"),
            _ => None,
        }
    }

    /// The Python response's `accepted` field.  `batch` is always the caller's
    /// own `drop_result` dict (`_queue_pet_drop(paths)`), which is why it is not
    /// duplicated here: present exactly when `drop_accepted > 0`, and `None` for
    /// the successful `Note` branch (readmd.py:5789 writes `'batch': None`).
    pub fn accepted(&self) -> Option<usize> {
        match self {
            ClipboardOutcome::Empty => Some(0),
            ClipboardOutcome::DropOnly { accepted } => Some(*accepted),
            ClipboardOutcome::InvalidImage => Some(0),
            ClipboardOutcome::ImageError { drop_accepted } => Some(*drop_accepted),
            ClipboardOutcome::Note { accepted, .. } => Some(*accepted),
            ClipboardOutcome::Raises { .. } => None,
        }
    }
}

fn base64_ascii_validate(image: &str) -> Option<Option<Vec<u8>>> {
    // Returns Some(Some(bytes)) on a clean decode, Some(None) when it decoded
    // but Python would flag the PNG-magic check, and None when Python raises
    // (`UnicodeError` from `.encode('ascii')` or `binascii.Error`).
    if !image.is_ascii() {
        return None; // `image.encode('ascii')` → UnicodeEncodeError
    }
    match base64::Engine::decode(&base64::engine::general_purpose::STANDARD, image) {
        Ok(bytes) => Some(Some(bytes)),
        Err(_) => None, // validate=True → binascii.Error
    }
}

/// `_open_pet_clipboard` (readmd.py:5746).  `drop_accepted` is the `accepted`
/// count `_queue_pet_drop(paths)` produced for this command's `paths`; `image_rel`
/// is the basename the caller will use for the PNG file (only embedded in the
/// note when the image is present and valid).
pub fn classify_clipboard(command: &Value, drop_accepted: usize, image_rel: &str) -> ClipboardOutcome {
    // `text = command.get('text') or ''` / `image = command.get('image_png') or ''`
    let text = or_value(command.get("text"));
    let image = or_value(command.get("image_png"));

    // `if not text and not image:`
    if text == OrVal::Empty && image == OrVal::Empty {
        if drop_accepted > 0 {
            return ClipboardOutcome::DropOnly { accepted: drop_accepted };
        }
        return ClipboardOutcome::Empty;
    }

    // `if image:` -> `raw = base64.b64decode(image.encode('ascii'), validate=True)`
    // 非字符串 image 在这里就 AttributeError（不在 except 元组里 -> 穿出函数）。
    let bytes = match image {
        OrVal::Empty => None,
        OrVal::NonStr(v) => {
            return ClipboardOutcome::Raises {
                exc: "AttributeError",
                message: encode_attribute_error(v),
            }
        }
        OrVal::Str(s) => match base64_ascii_validate(s) {
            None => {
                // UnicodeError / binascii.Error -> clipboard_image_write_failed
                return ClipboardOutcome::ImageError { drop_accepted };
            }
            Some(Some(bytes)) => {
                // `if not raw.startswith(b'\x89PNG...')` 早于任何 content 拼接，
                // 所以即使 text 是非字符串也仍然返回 invalid_clipboard_image。
                if !bytes.starts_with(&PNG_MAGIC) {
                    return ClipboardOutcome::InvalidImage;
                }
                Some(bytes)
            }
            Some(None) => None,
        },
    };

    // `content = text`（Python 保留原对象），随后
    // `content = (content + '\n\n' if content else '') + '![' + relative + '](' ...`
    let content = match text {
        OrVal::Str(s) => s.to_string(),
        OrVal::Empty => String::new(),
        OrVal::NonStr(v) => {
            return ClipboardOutcome::Raises {
                exc: "TypeError",
                // 图片有效时才走到拼接；没有图片时 `_write_md` 的 f.write 先炸。
                message: if bytes.is_some() {
                    concat_type_error(v)
                } else {
                    write_type_error(v)
                },
            }
        }
    };

    let content = match image {
        // 图片分支已在上面确认是有效 PNG；这里只做 Markdown 追加。
        OrVal::Str(_) => {
            let sep = if content.is_empty() { "" } else { "\n\n" };
            format!("{}{}![{}]({})\n", content, sep, image_rel, image_rel)
        }
        _ => content,
    };

    ClipboardOutcome::Note {
        content,
        accepted: 1 + drop_accepted,
        has_batch: drop_accepted > 0,
    }
}

// ----------------------------------------------------- _publish_pet_runtime

/// `_publish_pet_runtime` renderer pick (readmd.py:5122): honour
/// `renderer_override` only when it is one of the two recognised renderers,
/// otherwise fall back to the stored preference.
pub fn resolve_renderer(renderer_override: Option<&str>, prefs_renderer: &str) -> String {
    match renderer_override {
        Some(r) if r == "hermes-sprite" || r == "live2d" => r.to_string(),
        _ => prefs_renderer.to_string(),
    }
}

/// `isinstance(runtime, dict)` guard (readmd.py:5120): only a dict runtime is
/// used as-is; anything else means "take the controller snapshot" (caller's job).
pub fn runtime_is_dict(runtime: &Value) -> bool {
    runtime.is_object()
}

/// The animation block (readmd.py:5123-5129): only applied when BOTH
/// `animation_enabled` and `fps_cap` keys are present.  `enabled` is `bool(...)`,
/// `fpsCap` is `runtime['fps_cap'] or 0`.
pub fn animation_update(runtime: &Value) -> Option<Value> {
    let obj = runtime.as_object()?;
    if !(obj.contains_key("animation_enabled") && obj.contains_key("fps_cap")) {
        return None;
    }
    let enabled = runtime.get("animation_enabled").map(py_truthy).unwrap_or(false);
    let fps = match runtime.get("fps_cap") {
        Some(v) if py_truthy(v) => v.clone(),
        _ => Value::from(0),
    };
    Some(serde_json::json!({ "enabled": enabled, "fpsCap": fps }))
}

/// `slug = settings.get('pet_slug') if isinstance(settings, dict) else None`
/// then `if slug:` (readmd.py:5750-5753).  Returns `Some(slug)` only for a dict
/// whose `pet_slug` is truthy — and hands the **raw object** back, because
/// Python passes it straight into `find_pet(DATA_DIR, slug)`: an `int` slug stays
/// an `int` there.  `None` covers the two distinct falsy outcomes (not a dict /
/// key absent / `null` / `""` / `0` / `[]`), which all skip `find_pet`.
pub fn pick_pet_slug(settings: &Value) -> Option<Value> {
    let obj = settings.as_object()?;
    let slug = obj.get("pet_slug")?;
    if py_truthy(slug) {
        Some(slug.clone())
    } else {
        None
    }
}

// ----------------------------------------------- _start_pet_fullscreen_loop

/// The change-detection state machine inside `_start_pet_fullscreen_loop`
/// (readmd.py:5721-5741).  Publishes only when the fullscreen flag differs from
/// the last published value; the "pet disabled" branch resets the memory so the
/// next enable re-publishes.  A Python exception while reading either flag is
/// handled by the caller (it maps to `continue` / `current=False`).
#[derive(Debug, Default)]
pub struct FullscreenEdge {
    last: Option<bool>,
}

impl FullscreenEdge {
    pub fn new() -> Self {
        FullscreenEdge { last: None }
    }

    /// One loop tick.  `enabled` is the pet-enabled bool; `current` is the
    /// foreground-fullscreen reading (already defaulted to `false` on error).
    /// Returns `true` when `_publish_pet_runtime()` must run this tick.
    pub fn tick(&mut self, enabled: bool, current: bool) -> bool {
        if !enabled {
            self.last = None;
            return false;
        }
        if self.last == Some(current) {
            return false; // `if current == last: continue`
        }
        self.last = Some(current);
        true
    }
}

// ------------------------------------------------------------- _start_tray

/// `os.path.join(app_dir, 'assets', fname)` 的 ntpath 语义（本机 CPython 实测）：
/// 分隔符永远是 `\`，但已经以 `\` 或 `/` 结尾的前缀不再补分隔符，
/// 裸盘符（`D:`）也不补，空前缀得到相对路径。
/// ```text
/// 'C:\App'   -> 'C:\App\assets\icon-256.png'
/// 'C:/App'   -> 'C:/App\assets\icon-256.png'
/// 'C:\App\'  -> 'C:\App\assets\icon-256.png'
/// 'C:/App/'  -> 'C:/App/assets\x.png'
/// ''         -> 'assets\x.png'
/// 'C:\'      -> 'C:\assets\icon-256.png'
/// '/app'     -> '/app\assets\icon-256.png'
/// 'D:'       -> 'D:assets\x.png'
/// ```
fn nt_join_assets(app_dir: &str, fname: &str) -> String {
    let mut b = app_dir.to_string();
    for pth in ["assets", fname] {
        b = if b.is_empty() {
            pth.to_string()
        } else if b.ends_with('\\') || b.ends_with('/') || b.ends_with(':') {
            format!("{b}{pth}")
        } else {
            format!("{b}\\{pth}")
        };
    }
    b
}

/// Icon-candidate order from `_start_tray` (readmd.py:6893): prefer
/// `icon-256.png` (macOS pystray wants PNG), fall back to `readmd.ico`.  Returns
/// the first candidate the predicate says is usable; the pystray menu itself is
/// owned by `main.rs`.
pub fn pick_tray_icon<F: Fn(&str) -> bool>(app_dir: &str, usable: F) -> Option<String> {
    for fname in ["icon-256.png", "readmd.ico"] {
        let path = nt_join_assets(app_dir, fname);
        if usable(&path) {
            return Some(path);
        }
    }
    None
}

// =========================================================== tests

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use serde_json::json;

    fn b64(bytes: &[u8]) -> String {
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    #[test]
    fn route_nothing_on_empty_or_null_command() {
        assert_eq!(route_pet_command(&json!(null)), PetCommand::Nothing);
        assert_eq!(route_pet_command(&json!({})), PetCommand::Nothing); // empty dict is falsy
    }

    #[test]
    fn route_ready_openmenu_clipboard() {
        assert_eq!(route_pet_command(&json!({"type": "ready"})), PetCommand::Ready);
        assert_eq!(route_pet_command(&json!({"type": "open-menu"})), PetCommand::OpenMenu);
        assert_eq!(route_pet_command(&json!({"type": "clipboard", "text": "x"})), PetCommand::Clipboard);
    }

    #[test]
    fn route_bounds_and_scale_with_values() {
        assert_eq!(
            route_pet_command(&json!({"type": "bounds", "bounds": [1, 2, 3, 4]})),
            PetCommand::Bounds { bounds: json!([1, 2, 3, 4]) }
        );
        assert_eq!(
            route_pet_command(&json!({"type": "scale", "scale": 0.4})),
            PetCommand::Scale { scale: json!(0.4) }
        );
        // present-but-null is NOT a KeyError (Python `command['bounds']` yields None).
        assert_eq!(
            route_pet_command(&json!({"type": "bounds", "bounds": null})),
            PetCommand::Bounds { bounds: json!(null) }
        );
    }

    #[test]
    fn route_bounds_and_scale_missing_key_is_python_keyerror() {
        assert_eq!(route_pet_command(&json!({"type": "bounds"})), PetCommand::MissingKey("bounds"));
        assert_eq!(route_pet_command(&json!({"type": "scale"})), PetCommand::MissingKey("scale"));
    }

    #[test]
    fn route_drop_default_paths_and_passthrough() {
        // command.get('paths', []) -> [] when absent.
        assert_eq!(route_pet_command(&json!({"type": "drop"})), PetCommand::Drop { paths: json!([]) });
        assert_eq!(
            route_pet_command(&json!({"type": "drop", "paths": ["a", "b"]})),
            PetCommand::Drop { paths: json!(["a", "b"]) }
        );
        // unknown type, and a missing type, echo the whole command back.
        assert_eq!(
            route_pet_command(&json!({"type": "noise", "x": 1})),
            PetCommand::Passthrough(json!({"type": "noise", "x": 1}))
        );
        assert_eq!(route_pet_command(&json!({"a": 1})), PetCommand::Passthrough(json!({"a": 1})));
    }

    #[test]
    fn clipboard_empty_and_drop_only() {
        // no text, no image, no accepted drop -> Empty.
        assert_eq!(classify_clipboard(&json!({}), 0, "clipboard.png"), ClipboardOutcome::Empty);
        // no text, no image, drop accepted -> DropOnly carrying the batch.
        assert_eq!(
            classify_clipboard(&json!({}), 3, "clipboard.png"),
            ClipboardOutcome::DropOnly { accepted: 3 }
        );
    }

    #[test]
    fn clipboard_text_only_produces_a_note() {
        let out = classify_clipboard(&json!({"text": "hello"}), 0, "clipboard.png");
        assert_eq!(
            out,
            ClipboardOutcome::Note { content: "hello".to_string(), accepted: 1, has_batch: false }
        );
    }

    #[test]
    fn clipboard_valid_png_appends_image_markdown() {
        let png = b64(&PNG_MAGIC);
        let out = classify_clipboard(
            &json!({"text": "note", "image_png": png}),
            2,
            "clipboard-stamp.png",
        );
        assert_eq!(
            out,
            ClipboardOutcome::Note {
                content: "note\n\n![clipboard-stamp.png](clipboard-stamp.png)\n".to_string(),
                accepted: 3, // 1 + drop_accepted
                has_batch: true,
            }
        );
        // image with no text -> no leading separator.
        let out2 = classify_clipboard(&json!({"image_png": png}), 0, "c.png");
        assert_eq!(
            out2,
            ClipboardOutcome::Note {
                content: "![c.png](c.png)\n".to_string(),
                accepted: 1,
                has_batch: false,
            }
        );
    }

    #[test]
    fn clipboard_bad_magic_is_invalid_image_not_an_error() {
        let notpng = b64(b"RIFFxxxx");
        assert_eq!(
            classify_clipboard(&json!({"image_png": notpng}), 0, "c.png"),
            ClipboardOutcome::InvalidImage
        );
    }

    #[test]
    fn clipboard_decode_failure_is_image_error() {
        // invalid base64 chars -> binascii.Error -> ImageError.
        assert_eq!(
            classify_clipboard(&json!({"image_png": "!!!not base64!!!"}), 0, "c.png"),
            ClipboardOutcome::ImageError { drop_accepted: 0 }
        );
        // non-ascii -> .encode('ascii') UnicodeError -> ImageError.
        assert_eq!(
            classify_clipboard(&json!({"image_png": "\u{e9}abc"}), 0, "c.png"),
            ClipboardOutcome::ImageError { drop_accepted: 0 }
        );
        // a write error (simulated by drop>0) still reports the batch count.
        assert_eq!(
            classify_clipboard(&json!({"image_png": "bad!!", "text": "t"}), 4, "c.png"),
            ClipboardOutcome::ImageError { drop_accepted: 4 }
        );
    }

    #[test]
    fn publish_renderer_override_rule() {
        assert_eq!(resolve_renderer(Some("hermes-sprite"), "live2d"), "hermes-sprite");
        assert_eq!(resolve_renderer(Some("live2d"), "hermes-sprite"), "live2d");
        // an unrecognised override falls back to the stored preference.
        assert_eq!(resolve_renderer(Some("bogus"), "live2d"), "live2d");
        assert_eq!(resolve_renderer(None, "hermes-sprite"), "hermes-sprite");
    }

    #[test]
    fn publish_runtime_dict_and_slug() {
        assert!(runtime_is_dict(&json!({"a": 1})));
        assert!(!runtime_is_dict(&json!(null)));
        assert!(!runtime_is_dict(&json!([1, 2])));
        assert_eq!(pick_pet_slug(&json!({"pet_slug": "cat"})), Some(json!("cat")));
        assert_eq!(pick_pet_slug(&json!({"pet_slug": ""})), None); // falsy slug
        assert_eq!(pick_pet_slug(&json!({"pet_slug": 0})), None);
        assert_eq!(pick_pet_slug(&json!({"pet_slug": false})), None);
        assert_eq!(pick_pet_slug(&json!({"pet_slug": []})), None);
        assert_eq!(pick_pet_slug(&json!({"pet_slug": null})), None); // present-null == falsy
        assert_eq!(pick_pet_slug(&json!("cat")), None); // not a dict
        assert_eq!(pick_pet_slug(&json!({})), None); // no key
    }

    #[test]
    fn slug_truthy_non_string_keeps_its_python_type() {
        // 实测：{'pet_slug': 7} -> bool(slug)=True，交给 find_pet 的值仍是 7。
        // 旧实现塌缩成 Some("")，与"真的空 slug"无法区分（空串其实是假值，Python 根本不调用 find_pet）。
        assert_eq!(pick_pet_slug(&json!({"pet_slug": 7})), Some(json!(7)));
        assert_eq!(pick_pet_slug(&json!({"pet_slug": 7.5})), Some(json!(7.5)));
        assert_eq!(pick_pet_slug(&json!({"pet_slug": [1, 2]})), Some(json!([1, 2])));
        assert_eq!(pick_pet_slug(&json!({"pet_slug": {"a": 1}})), Some(json!({"a": 1})));
    }

    #[test]
    fn publish_animation_gate_and_fps_or_zero() {
        // Both keys must be present.
        assert_eq!(animation_update(&json!({"animation_enabled": true})), None);
        assert_eq!(animation_update(&json!({"fps_cap": 30})), None);
        assert_eq!(animation_update(&json!({"a": 1})), None);
        // enabled coerced via bool(); fps_cap truthy preserved.
        assert_eq!(
            animation_update(&json!({"animation_enabled": 1, "fps_cap": 60})),
            Some(json!({"enabled": true, "fpsCap": 60}))
        );
        // `fps_cap or 0`: falsy -> 0.
        assert_eq!(
            animation_update(&json!({"animation_enabled": false, "fps_cap": null})),
            Some(json!({"enabled": false, "fpsCap": 0}))
        );
        assert_eq!(
            animation_update(&json!({"animation_enabled": "x", "fps_cap": 0})),
            Some(json!({"enabled": true, "fpsCap": 0}))
        );
        assert_eq!(animation_update(&json!("not a dict")), None);
    }

    #[test]
    fn fullscreen_edge_publishes_only_on_change() {
        let mut e = FullscreenEdge::new();
        assert!(e.tick(true, false), "first reading differs from None -> publish");
        assert!(!e.tick(true, false), "same as last -> skip");
        assert!(e.tick(true, true), "flipped -> publish");
        assert!(!e.tick(true, true), "same -> skip");
        // disabled resets memory, so the next enable re-publishes even if unchanged.
        assert!(!e.tick(false, true), "disabled -> no publish, resets memory");
        assert!(e.tick(true, true), "after reset, first enable republishes");
    }

    #[test]
    fn tray_icon_candidate_order() {
        // PNG preferred when present, else the .ico (readmd.py:6892-6897).
        let picked = pick_tray_icon("C:\\app", |p| p == "C:\\app\\assets\\icon-256.png");
        assert_eq!(picked, Some("C:\\app\\assets\\icon-256.png".to_string()));
        let picked2 = pick_tray_icon("C:\\app", |p| p == "C:\\app\\assets\\readmd.ico");
        assert_eq!(picked2, Some("C:\\app\\assets\\readmd.ico".to_string()));
        // neither usable -> None.
        assert_eq!(pick_tray_icon("C:\\app", |_| false), None);
    }

    #[test]
    fn tray_icon_path_uses_ntpath_join_not_forward_slash() {
        // 每一行都是本机 CPython `os.path.join(a, 'assets', f)` 的实测输出（ntpath）。
        let rows: &[(&str, &str, &str)] = &[
            ("C:\\App", "icon-256.png", "C:\\App\\assets\\icon-256.png"),
            ("C:/App", "icon-256.png", "C:/App\\assets\\icon-256.png"),
            ("C:\\App\\", "icon-256.png", "C:\\App\\assets\\icon-256.png"),
            ("C:/App/", "x.png", "C:/App/assets\\x.png"),
            ("", "x.png", "assets\\x.png"),
            ("C:\\", "icon-256.png", "C:\\assets\\icon-256.png"),
            ("/app", "icon-256.png", "/app\\assets\\icon-256.png"),
            ("D:", "x.png", "D:assets\\x.png"),
        ];
        for (a, f, want) in rows {
            assert_eq!(nt_join_assets(a, f), *want, "join({a:?}, 'assets', {f:?})");
        }
    }

    #[test]
    fn clipboard_non_string_image_raises_attribute_error() {
        // 实测：(5).encode('ascii') -> AttributeError: 'int' object has no attribute 'encode'
        // AttributeError 不在 except (OSError, ValueError, UnicodeError, binascii.Error) 里。
        assert_eq!(
            classify_clipboard(&json!({"image_png": 5}), 0, "c.png"),
            ClipboardOutcome::Raises {
                exc: "AttributeError",
                message: "'int' object has no attribute 'encode'".to_string(),
            }
        );
        assert_eq!(
            classify_clipboard(&json!({"image_png": [1], "text": "t"}), 4, "c.png"),
            ClipboardOutcome::Raises {
                exc: "AttributeError",
                message: "'list' object has no attribute 'encode'".to_string(),
            }
        );
    }

    #[test]
    fn clipboard_non_string_text_raises_type_error() {
        // 无图片 -> _write_md 的 f.write(5)：TypeError: write() argument must be str, not int
        assert_eq!(
            classify_clipboard(&json!({"text": 5}), 0, "c.png"),
            ClipboardOutcome::Raises {
                exc: "TypeError",
                message: "write() argument must be str, not int".to_string(),
            }
        );
        assert_eq!(
            classify_clipboard(&json!({"text": true}), 0, "c.png"),
            ClipboardOutcome::Raises {
                exc: "TypeError",
                message: "write() argument must be str, not bool".to_string(),
            }
        );
        // 有效 PNG -> content + '\n\n' 才炸（readmd.py:5782）。
        let png = b64(&PNG_MAGIC);
        assert_eq!(
            classify_clipboard(&json!({"text": 5.5, "image_png": png}), 0, "c.png"),
            ClipboardOutcome::Raises {
                exc: "TypeError",
                message: "unsupported operand type(s) for +: 'float' and 'str'".to_string(),
            }
        );
        assert_eq!(
            classify_clipboard(&json!({"text": [1], "image_png": png}), 0, "c.png"),
            ClipboardOutcome::Raises {
                exc: "TypeError",
                message: "can only concatenate list (not \"str\") to list".to_string(),
            }
        );
        // 顺序验证：magic 检查早于拼接 -> 坏 magic 仍是 invalid_clipboard_image；
        // 而 base64 解码失败被 except 捕获 -> ImageError。
        let notpng = b64(b"RIFFxxxx");
        assert_eq!(
            classify_clipboard(&json!({"text": 5, "image_png": notpng}), 0, "c.png"),
            ClipboardOutcome::InvalidImage
        );
        assert_eq!(
            classify_clipboard(&json!({"text": 5, "image_png": "!!!"}), 2, "c.png"),
            ClipboardOutcome::ImageError { drop_accepted: 2 }
        );
    }

    #[test]
    fn clipboard_outcome_exposes_python_code_and_accepted() {
        // readmd.py:5777 / 5784-5788 的 code 与 accepted 字段。
        assert_eq!(ClipboardOutcome::InvalidImage.code(), Some("invalid_clipboard_image"));
        assert_eq!(
            ClipboardOutcome::ImageError { drop_accepted: 3 }.code(),
            Some("clipboard_image_write_failed")
        );
        assert_eq!(ClipboardOutcome::Empty.code(), None);
        assert_eq!(
            classify_clipboard(&json!({"text": "hi"}), 0, "c.png").code(),
            None
        );
        assert_eq!(ClipboardOutcome::Empty.accepted(), Some(0));
        assert_eq!(ClipboardOutcome::DropOnly { accepted: 2 }.accepted(), Some(2));
        assert_eq!(ClipboardOutcome::InvalidImage.accepted(), Some(0));
        assert_eq!(ClipboardOutcome::ImageError { drop_accepted: 4 }.accepted(), Some(4));
        assert_eq!(
            ClipboardOutcome::Note { content: String::new(), accepted: 1, has_batch: false }
                .accepted(),
            Some(1)
        );
        assert_eq!(
            ClipboardOutcome::Raises { exc: "TypeError", message: String::new() }.accepted(),
            None
        );
    }
}

