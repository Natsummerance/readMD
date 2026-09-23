use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const SNAPSHOT_FORMAT_VERSION: u32 = 1;
pub const PROTOCOL_VERSION: u32 = 1;
pub const MAX_SNAPSHOT_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_COMMAND_BODY_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_PENDING_COMMAND_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_PENDING_COMMANDS: usize = 128;
pub const MIN_HOST_WIDTH: f64 = 240.0;
pub const MAX_HOST_WIDTH: f64 = 640.0;
pub const MIN_HOST_HEIGHT: f64 = 300.0;
pub const MAX_HOST_HEIGHT: f64 = 720.0;
pub const MIN_RENDERER_SIZE: f64 = 80.0;

/// Caps for the `clipboard` command. A single over-limit field makes the
/// Python authority discard the whole command
/// (`src/readmd_modules/pet/hermes_adapter.py:223-236`), so the host clamps
/// instead of publishing something that cannot be consumed.
pub const MAX_CLIPBOARD_TEXT_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_CLIPBOARD_IMAGE_PNG_CHARS: usize = 24 * 1024 * 1024;
pub const MAX_CLIPBOARD_PATHS: usize = 128;
pub const MAX_CLIPBOARD_PATH_CHARS: usize = 32_768;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SnapshotBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Default for SnapshotBounds {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 320.0,
            height: 420.0,
        }
    }
}

impl SnapshotBounds {
    pub fn clamp_host(self) -> Self {
        Self {
            x: if self.x.is_finite() {
                self.x.clamp(-32768.0, 32768.0)
            } else {
                0.0
            },
            y: if self.y.is_finite() {
                self.y.clamp(-32768.0, 32768.0)
            } else {
                0.0
            },
            width: if self.width.is_finite() {
                self.width.clamp(MIN_HOST_WIDTH, MAX_HOST_WIDTH)
            } else {
                320.0
            },
            height: if self.height.is_finite() {
                self.height.clamp(MIN_HOST_HEIGHT, MAX_HOST_HEIGHT)
            } else {
                380.0
            },
        }
    }

    pub fn validate_renderer_rect(self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && self.width.is_finite()
            && self.height.is_finite()
            && self.width >= MIN_RENDERER_SIZE
            && self.height >= MIN_RENDERER_SIZE
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PetSnapshot {
    pub format_version: u32,
    #[serde(default)]
    pub visible: bool,
    #[serde(default)]
    pub fullscreen: bool,
    #[serde(default)]
    pub generation: u64,
    #[serde(default)]
    pub renderer: Option<String>,
    #[serde(default)]
    pub bounds: Option<SnapshotBounds>,
    #[serde(default)]
    pub info: serde_json::Value,
    #[serde(default)]
    pub activity: serde_json::Value,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl PetSnapshot {
    pub fn renderer_kind(&self) -> RendererKind {
        RendererKind::parse(self.renderer.as_deref()).unwrap_or(RendererKind::Sprite)
    }

    pub fn opacity(&self) -> f64 {
        self.info
            .get("opacity")
            .and_then(serde_json::Value::as_f64)
            .or_else(|| {
                self.extra
                    .get("opacity")
                    .and_then(serde_json::Value::as_f64)
            })
            .unwrap_or(1.0)
            .clamp(0.35, 1.0)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandEnvelope {
    pub command: serde_json::Value,
    pub created_at: u64,
}

/// The one command whose wire name differs from the renderer control that
/// triggers it. The shipped overlay emits `toggle-app`
/// (`renderer/assets/overlay-root-*.js`) and the Electron reference adapter
/// renames it to a clipboard capture (`electron-main.ts:296`); the app only
/// consumes `clipboard` (`readmd.py:5637`).
///
/// The four keys below are the complete set the consumer reads, so
/// unavailable sources serialise as `""` / `[]` rather than being omitted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipboardCommand {
    #[serde(rename = "type")]
    pub kind: String,
    pub text: String,
    pub image_png: String,
    pub paths: Vec<String>,
}

impl ClipboardCommand {
    pub const TYPE_NAME: &'static str = "clipboard";

    /// An empty capture: a clipboard that cannot be read, or a platform with
    /// no reader, still produces a schema-correct command.
    pub fn empty() -> Self {
        Self {
            kind: Self::TYPE_NAME.to_string(),
            text: String::new(),
            image_png: String::new(),
            paths: Vec::new(),
        }
    }

    /// Clamp raw clipboard contents into the shape the authority accepts.
    pub fn new(text: String, image_png: String, paths: Vec<String>) -> Self {
        let paths = paths
            .into_iter()
            .filter(|path| !path.is_empty() && path.chars().count() <= MAX_CLIPBOARD_PATH_CHARS)
            .take(MAX_CLIPBOARD_PATHS)
            .collect::<Vec<_>>();
        Self {
            kind: Self::TYPE_NAME.to_string(),
            text: truncate_utf8(&text, MAX_CLIPBOARD_TEXT_BYTES),
            image_png: truncate_utf8(&image_png, MAX_CLIPBOARD_IMAGE_PNG_CHARS),
            paths,
        }
    }
}

/// Truncate to `max_bytes` without splitting a UTF-8 sequence, so the result
/// can never violate a byte-length limit the consumer measures in UTF-8.
pub fn truncate_utf8(source: &str, max_bytes: usize) -> String {
    if source.len() <= max_bytes {
        return source.to_owned();
    }
    let mut end = max_bytes;
    while end > 0 && !source.is_char_boundary(end) {
        end -= 1;
    }
    source[..end].to_owned()
}

/// Every command type `HermesPetBridge._take_command_file` admits
/// (`src/readmd_modules/pet/hermes_adapter.py:81`).  Anything else is deleted
/// unread at `hermes_adapter.py:180-182`, so publishing it from the host would
/// be a silent no-op reported as success.
pub const CONSUMER_COMMANDS: &[&str] = &[
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

const INTERACT_ACTIONS: &[&str] = &["pet", "feed", "play", "rest", "wake"];
const MIN_COMMAND_SCALE: f64 = 0.18;
const MAX_COMMAND_SCALE: f64 = 0.72;
const MAX_COMMAND_PATH_CHARS: usize = 32_768;

/// `round(number, 2)` from `hermes_adapter.py:206`, which uses
/// round-half-to-even; `f64::round` would break ties away from zero and let
/// the host publish a scale the consumer then rewrites.
fn round2(value: f64) -> f64 {
    (value * 100.0).round_ties_even() / 100.0
}

fn as_finite_f64(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|text| text.trim().parse::<f64>().ok()))
        .filter(|number| number.is_finite())
}

/// `HermesPetBridge._safe_bounds` (`hermes_adapter.py:243-256`): all four keys
/// are mandatory, must be finite, are rounded half-to-even to integers and are
/// clamped per axis.
pub fn safe_bounds(object: &Value) -> Option<Value> {
    let mut bounds = serde_json::Map::new();
    for (key, low, high) in [
        ("x", -32768.0_f64, 32768.0_f64),
        ("y", -32768.0, 32768.0),
        ("width", 80.0, 2048.0),
        ("height", 80.0, 2048.0),
    ] {
        let number = as_finite_f64(object.get(key)?)?;
        let number = number.round_ties_even();
        bounds.insert(
            key.to_string(),
            Value::from(number.clamp(low, high) as i64),
        );
    }
    Some(Value::Object(bounds))
}

/// Validate and strip a command to the exact shape the application consumes
/// (`hermes_adapter.py:144-241`).  `Err` means the bridge would delete the
/// file, so the host must not write it and must not claim success.
pub fn normalise_command(command: &Value) -> Result<Value, &'static str> {
    let object = command
        .as_object()
        .ok_or("pet_command_must_be_object")?;
    let kind = object
        .get("type")
        .and_then(Value::as_str)
        .ok_or("pet_command_type_missing")?;
    if kind.len() > 64 || !CONSUMER_COMMANDS.contains(&kind) {
        return Err("pet_command_type_unsupported");
    }
    let normalised = match kind {
        "bounds" => {
            let bounds = object
                .get("bounds")
                .and_then(Value::as_object)
                .ok_or("pet_command_bounds_invalid")?;
            serde_json::json!({
                "type": "bounds",
                "bounds": safe_bounds(&Value::Object(bounds.clone()))
                    .ok_or("pet_command_bounds_invalid")?,
            })
        }
        "scale" => {
            let scale = object
                .get("scale")
                .and_then(as_finite_f64)
                .ok_or("pet_command_scale_invalid")?;
            let scale = round2(scale);
            if !(MIN_COMMAND_SCALE..=MAX_COMMAND_SCALE).contains(&scale) {
                return Err("pet_command_scale_out_of_range");
            }
            serde_json::json!({"type":"scale","scale":scale})
        }
        "drop" => {
            let paths = object
                .get("paths")
                .and_then(Value::as_array)
                .ok_or("pet_command_paths_invalid")?;
            if paths.is_empty() || paths.len() > MAX_CLIPBOARD_PATHS {
                return Err("pet_command_paths_invalid");
            }
            let mut clean = Vec::with_capacity(paths.len());
            for path in paths {
                let path = path.as_str().ok_or("pet_command_paths_invalid")?;
                if path.is_empty() || path.chars().count() > MAX_COMMAND_PATH_CHARS {
                    return Err("pet_command_paths_invalid");
                }
                clean.push(Value::String(path.to_string()));
            }
            serde_json::json!({"type":"drop","paths":clean})
        }
        "clipboard" => {
            let text = match object.get("text") {
                None | Some(Value::Null) => "",
                Some(value) => value.as_str().ok_or("pet_command_clipboard_invalid")?,
            };
            if text.len() > MAX_CLIPBOARD_TEXT_BYTES {
                return Err("pet_command_clipboard_invalid");
            }
            let image = match object.get("image_png") {
                None | Some(Value::Null) => "",
                Some(value) => value.as_str().ok_or("pet_command_clipboard_invalid")?,
            };
            if image.chars().count() > MAX_CLIPBOARD_IMAGE_PNG_CHARS {
                return Err("pet_command_clipboard_invalid");
            }
            let paths = match object.get("paths") {
                None | Some(Value::Null) => Vec::new(),
                Some(value) => value
                    .as_array()
                    .cloned()
                    .ok_or("pet_command_clipboard_invalid")?,
            };
            if paths.len() > MAX_CLIPBOARD_PATHS
                || paths.iter().any(|path| {
                    !path.is_string()
                        || path
                            .as_str()
                            .map(|text| text.chars().count() > MAX_COMMAND_PATH_CHARS)
                            .unwrap_or(true)
                })
            {
                return Err("pet_command_clipboard_invalid");
            }
            serde_json::json!({
                "type": "clipboard",
                "text": text,
                "image_png": image,
                "paths": paths,
            })
        }
        "interact" => {
            let action = object
                .get("action")
                .and_then(Value::as_str)
                .ok_or("pet_command_action_invalid")?;
            if !INTERACT_ACTIONS.contains(&action) {
                return Err("pet_command_action_invalid");
            }
            serde_json::json!({"type":"interact","action":action})
        }
        "character" => {
            let slug = object
                .get("slug")
                .and_then(Value::as_str)
                .filter(|slug| slug.len() <= 63)
                .ok_or("pet_command_character_invalid")?;
            let renderer = object
                .get("renderer")
                .and_then(Value::as_str)
                .filter(|renderer| {
                    *renderer == RendererKind::Sprite.query_value()
                        || *renderer == RendererKind::Live2D.query_value()
                })
                .ok_or("pet_command_character_invalid")?;
            serde_json::json!({"type":"character","slug":slug,"renderer":renderer})
        }
        _ => command.clone(),
    };
    Ok(normalised)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RendererKind {
    #[serde(rename = "hermes-sprite")]
    Sprite,
    #[serde(rename = "live2d")]
    Live2D,
}

impl RendererKind {
    pub fn parse(value: Option<&str>) -> Option<Self> {
        match value.unwrap_or("hermes-sprite") {
            "hermes-sprite" | "sprite" => Some(Self::Sprite),
            "live2d" => Some(Self::Live2D),
            _ => None,
        }
    }

    pub fn query_value(self) -> &'static str {
        match self {
            Self::Sprite => "hermes-sprite",
            Self::Live2D => "live2d",
        }
    }
}

/// Who produced a message.  Only `Page` messages cross the untrusted
/// boundary: `PRELOAD_ABI` (`src/webview/mod.rs`) stamps every page-posted
/// message with the `session` and `generation` taken from the renderer URL,
/// while `drop` and `page-finished` are synthesised by the host itself from
/// native Win32/GTK input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageOrigin {
    Page,
    Host,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RendererMessage {
    pub kind: String,
    pub payload: serde_json::Value,
    pub origin: MessageOrigin,
}

impl RendererMessage {
    pub fn page(kind: impl Into<String>, payload: serde_json::Value) -> Self {
        Self {
            kind: kind.into(),
            payload,
            origin: MessageOrigin::Page,
        }
    }

    pub fn host(kind: impl Into<String>, payload: serde_json::Value) -> Self {
        Self {
            kind: kind.into(),
            payload,
            origin: MessageOrigin::Host,
        }
    }

    /// The stamps live on the message body.  Older bridge shims double-wrap a
    /// control body as `{type:"control", payload:{...}}`, so the nested object
    /// is consulted before the message is treated as unauthenticated.
    pub fn session(&self) -> Option<&str> {
        let payload = self.payload.as_object()?;
        let direct = payload.get("session").and_then(Value::as_str);
        if direct.is_some() {
            return direct;
        }
        payload
            .get("payload")
            .and_then(Value::as_object)
            .and_then(|inner| inner.get("session"))
            .and_then(Value::as_str)
    }

    pub fn generation(&self) -> Option<u64> {
        let payload = self.payload.as_object()?;
        let direct = payload.get("generation").and_then(Value::as_u64);
        if direct.is_some() {
            return direct;
        }
        payload
            .get("payload")
            .and_then(Value::as_object)
            .and_then(|inner| inner.get("generation"))
            .and_then(Value::as_u64)
    }

    /// A page message is only actionable when it carries the session token and
    /// the navigation generation of the document the host is currently
    /// tracking.  Omitting either stamp is a rejection, not a pass: the
    /// optional check this replaces let any frame that reached the IPC handler
    /// enqueue durable commands the application then executed.
    pub fn authenticated(&self, session: &str, generation: u64) -> bool {
        if self.origin == MessageOrigin::Host {
            return true;
        }
        self.session() == Some(session) && self.generation() == Some(generation)
    }
}

pub fn parse_snapshot(bytes: &[u8]) -> Result<PetSnapshot, &'static str> {
    if bytes.len() > MAX_SNAPSHOT_BYTES {
        return Err("snapshot_too_large");
    }
    let value: PetSnapshot = serde_json::from_slice(bytes).map_err(|_| "invalid_snapshot")?;
    if value.format_version != SNAPSHOT_FORMAT_VERSION {
        return Err("unsupported_snapshot");
    }
    if let Some(bounds) = value.bounds {
        if !bounds.validate_renderer_rect() {
            return Err("invalid_snapshot_bounds");
        }
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_parser_is_bounded_and_versioned() {
        let raw = br#"{"format_version":1,"visible":true,"bounds":{"x":0,"y":0,"width":320,"height":420}}"#;
        let value = parse_snapshot(raw).expect("valid snapshot");
        assert!(value.visible);
        assert!(parse_snapshot(br#"{"format_version":2}"#).is_err());
    }

    #[test]
    fn renderer_and_bounds_are_clamped_at_host_boundary() {
        let bounds = SnapshotBounds {
            x: 40000.0,
            y: -40000.0,
            width: 1.0,
            height: 10000.0,
        }
        .clamp_host();
        assert_eq!(bounds.x, 32768.0);
        assert_eq!(bounds.width, MIN_HOST_WIDTH);
        assert_eq!(bounds.height, MAX_HOST_HEIGHT);
    }

    #[test]
    fn snapshot_opacity_is_clamped_and_defaults_to_opaque() {
        for (raw, expected) in [
            (-1.0, 0.35),
            (0.1, 0.35),
            (0.35, 0.35),
            (0.5, 0.5),
            (1.0, 1.0),
            (1.2, 1.0),
        ] {
            let value = serde_json::json!({"format_version":1,"info":{"opacity":raw}});
            let snapshot = parse_snapshot(&serde_json::to_vec(&value).unwrap()).unwrap();
            assert_eq!(snapshot.opacity(), expected);
        }
        let snapshot = parse_snapshot(br#"{"format_version":1}"#).unwrap();
        assert_eq!(snapshot.opacity(), 1.0);
    }

    #[test]
    fn snapshot_bounds_reject_non_finite_or_too_small_renderer_rects() {
        let missing = br#"{"format_version":1,"bounds":{"x":0,"y":0,"width":80,"height":79}}"#;
        assert_eq!(parse_snapshot(missing), Err("invalid_snapshot_bounds"));
        let missing_fields = br#"{"format_version":1,"bounds":{"x":0,"width":80,"height":80}}"#;
        assert_eq!(parse_snapshot(missing_fields), Err("invalid_snapshot"));
    }

    #[test]
    fn clipboard_text_is_capped_on_a_utf8_boundary() {
        let ascii = "x".repeat(MAX_CLIPBOARD_TEXT_BYTES + 10);
        assert_eq!(
            truncate_utf8(&ascii, MAX_CLIPBOARD_TEXT_BYTES).len(),
            MAX_CLIPBOARD_TEXT_BYTES
        );
        // A 4-byte emoji straddling the limit must be dropped whole, never
        // sliced: the consumer measures `len(text.encode('utf-8'))` and would
        // discard the entire command otherwise.
        let text = "a".repeat(MAX_CLIPBOARD_TEXT_BYTES - 2) + "😀tail";
        let capped = truncate_utf8(&text, MAX_CLIPBOARD_TEXT_BYTES);
        assert!(capped.len() < text.len());
        assert!(capped.is_char_boundary(capped.len()));
        assert!(capped.ends_with('a'));
        assert_eq!(truncate_utf8("😀", 0), "");
        assert_eq!(truncate_utf8("abc", 99), "abc");
    }

    #[test]
    fn clipboard_command_respects_every_consumer_limit() {
        let paths = (0..400)
            .map(|index| format!("C:\\notes\\file-{index}.md"))
            .chain(std::iter::once(String::new()))
            .chain(std::iter::once("y".repeat(MAX_CLIPBOARD_PATH_CHARS + 1)))
            .collect();
        let command = ClipboardCommand::new(
            "t".repeat(MAX_CLIPBOARD_TEXT_BYTES + 10),
            "i".repeat(MAX_CLIPBOARD_IMAGE_PNG_CHARS + 10),
            paths,
        );
        assert_eq!(command.kind, ClipboardCommand::TYPE_NAME);
        assert!(command.text.len() <= MAX_CLIPBOARD_TEXT_BYTES);
        assert!(command.image_png.len() <= MAX_CLIPBOARD_IMAGE_PNG_CHARS);
        assert_eq!(command.paths.len(), MAX_CLIPBOARD_PATHS);
        assert_eq!(command.paths[0], "C:\\notes\\file-0.md");
        assert_eq!(command.paths[MAX_CLIPBOARD_PATHS - 1], "C:\\notes\\file-127.md");
        assert!(command
            .paths
            .iter()
            .all(|path| !path.is_empty() && path.chars().count() <= MAX_CLIPBOARD_PATH_CHARS));
    }

    #[test]
    fn page_messages_must_carry_the_current_session_and_generation() {
        let message = |payload: Value| RendererMessage::page("open-app", payload);
        assert!(message(serde_json::json!({"session":"s", "generation":3}))
            .authenticated("s", 3));
        assert!(!message(serde_json::json!({"session":"s", "generation":4}))
            .authenticated("s", 3));
        assert!(!message(serde_json::json!({"session":"other", "generation":3}))
            .authenticated("s", 3));
        // The defect this closes: an unauthenticated frame used to pass simply
        // by sending no stamps at all.
        assert!(!message(serde_json::json!({})).authenticated("s", 3));
        assert!(!message(serde_json::json!({"session":"s"})).authenticated("s", 3));
        assert!(!message(serde_json::json!({"generation":3})).authenticated("s", 3));
        assert!(!message(serde_json::json!({"session":3, "generation":3})).authenticated("s", 3));
        // Host-synthesised native input has no page session to present.
        assert!(RendererMessage::host("drop", serde_json::json!({"paths":["a.md"]}))
            .authenticated("s", 3));
    }

    #[test]
    fn only_consumer_command_types_are_publishable() {
        for kind in CONSUMER_COMMANDS {
            assert!(
                normalise_command(&serde_json::json!({"type":kind})).is_ok()
                    || matches!(*kind, "bounds" | "scale" | "drop" | "interact" | "character"),
                "whitelisted type rejected: {kind}"
            );
        }
        // `close` is what the host used to invent: the overlay's hide request
        // was written as a durable command the bridge deletes unread.
        for invented in ["close", "ready", "host-ready", "page-finished", "abi-ready", ""] {
            assert_eq!(
                normalise_command(&serde_json::json!({"type":invented})),
                Err("pet_command_type_unsupported"),
                "invented type accepted: {invented}"
            );
        }
        assert_eq!(
            normalise_command(&serde_json::json!({"type":"x".repeat(65)})),
            Err("pet_command_type_unsupported")
        );
        assert_eq!(normalise_command(&Value::Null), Err("pet_command_must_be_object"));
        assert_eq!(
            normalise_command(&serde_json::json!({})),
            Err("pet_command_type_missing")
        );
        // Type-only commands are forwarded verbatim, exactly as the consumer
        // returns them after the whitelist check.
        assert_eq!(
            normalise_command(&serde_json::json!({"type":"open-menu","extra":1})),
            Ok(serde_json::json!({"type":"open-menu","extra":1}))
        );
    }

    #[test]
    fn every_command_kind_is_normalised_to_the_consumer_shape() {
        // bounds: all four keys mandatory, rounded half-to-even, then clamped.
        assert_eq!(
            normalise_command(&serde_json::json!({
                "type":"bounds","bounds":{"x":12.5,"y":-40000,"width":10,"height":99999.4}
            })),
            Ok(serde_json::json!({
                "type":"bounds","bounds":{"x":12,"y":-32768,"width":80,"height":2048}
            }))
        );
        assert_eq!(
            normalise_command(&serde_json::json!({"type":"bounds","bounds":{"x":0,"y":0}})),
            Err("pet_command_bounds_invalid")
        );
        assert_eq!(
            normalise_command(&serde_json::json!({"type":"bounds","bounds":"0,0"})),
            Err("pet_command_bounds_invalid")
        );
        // scale: round(x, 2) then 0.18 <= scale <= 0.72, no invented default.
        assert_eq!(
            normalise_command(&serde_json::json!({"type":"scale","scale":0.3349})),
            Ok(serde_json::json!({"type":"scale","scale":0.33}))
        );
        assert_eq!(
            normalise_command(&serde_json::json!({"type":"scale"})),
            Err("pet_command_scale_invalid")
        );
        assert_eq!(
            normalise_command(&serde_json::json!({"type":"scale","scale":0.9})),
            Err("pet_command_scale_out_of_range")
        );
        assert_eq!(
            normalise_command(&serde_json::json!({"type":"scale","scale":"nan"})),
            Err("pet_command_scale_invalid")
        );
        // drop: a non-empty list of non-empty strings.
        assert_eq!(
            normalise_command(&serde_json::json!({"type":"drop","paths":[]})),
            Err("pet_command_paths_invalid")
        );
        assert_eq!(
            normalise_command(&serde_json::json!({"type":"drop","paths":["a.md",""]})),
            Err("pet_command_paths_invalid")
        );
        assert_eq!(
            normalise_command(&serde_json::json!({"type":"drop","paths":[1]})),
            Err("pet_command_paths_invalid")
        );
        assert_eq!(
            normalise_command(&serde_json::json!({"type":"drop","paths":["a.md"]})),
            Ok(serde_json::json!({"type":"drop","paths":["a.md"]}))
        );
        // interact and character carry closed value sets.
        assert_eq!(
            normalise_command(&serde_json::json!({"type":"interact","action":"kick"})),
            Err("pet_command_action_invalid")
        );
        assert_eq!(
            normalise_command(&serde_json::json!({
                "type":"interact","action":"feed","extra":true
            })),
            Ok(serde_json::json!({"type":"interact","action":"feed"}))
        );
        assert_eq!(
            normalise_command(&serde_json::json!({
                "type":"character","slug":"arch-chan","renderer":"sprite"
            })),
            Err("pet_command_character_invalid")
        );
        assert_eq!(
            normalise_command(&serde_json::json!({
                "type":"character","slug":"arch-chan","renderer":"hermes-sprite"
            })),
            Ok(serde_json::json!({
                "type":"character","slug":"arch-chan","renderer":"hermes-sprite"
            }))
        );
        // clipboard keeps every key the consumer reads even when empty.
        assert_eq!(
            normalise_command(&serde_json::json!({
                "type":"clipboard","text":"hi","image_png":"","paths":[]
            })),
            Ok(serde_json::json!({
                "type":"clipboard","text":"hi","image_png":"","paths":[]
            }))
        );
        assert_eq!(
            normalise_command(&serde_json::json!({"type":"clipboard","text":1})),
            Err("pet_command_clipboard_invalid")
        );
    }

    #[test]
    fn published_clipboard_command_survives_consumer_validation() {
        let command = ClipboardCommand::new(
            "hello".into(),
            "aGVsbG8=".into(),
            vec!["C:/notes/a.md".into()],
        );
        let value = serde_json::to_value(&command).unwrap();
        assert_eq!(normalise_command(&value), Ok(value));
        assert_eq!(
            normalise_command(&serde_json::to_value(ClipboardCommand::empty()).unwrap()),
            Ok(serde_json::json!({
                "type":"clipboard","text":"","image_png":"","paths":[]
            }))
        );
    }
}
