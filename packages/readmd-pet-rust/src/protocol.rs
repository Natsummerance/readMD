use serde::{Deserialize, Serialize};

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
                420.0
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

#[derive(Debug, Clone, PartialEq)]
pub struct RendererMessage {
    pub kind: String,
    pub payload: serde_json::Value,
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
}
