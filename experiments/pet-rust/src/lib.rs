//! Phase 0 protocol and reconciliation spike.
//!
//! This crate is deliberately isolated under `experiments/pet-rust/`.  It is
//! not linked from ReadMD and does not create windows, load WebViews, or start
//! a desktop process.  Physical platform evidence is required before any of
//! these types can be promoted to a production host.

use serde::{Deserialize, Serialize};

pub const SNAPSHOT_FORMAT_VERSION: u32 = 1;
pub const MAX_SNAPSHOT_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_HOST_WIDTH: f64 = 640.0;
pub const MAX_HOST_HEIGHT: f64 = 720.0;
pub const MIN_HOST_WIDTH: f64 = 240.0;
pub const MIN_HOST_HEIGHT: f64 = 300.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CssPxPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CssPxRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceLocalDipRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl SurfaceLocalDipRect {
    pub fn is_finite(self) -> bool {
        [self.x, self.y, self.width, self.height]
            .into_iter()
            .all(f64::is_finite)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BridgeGlobalDipRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// The protocol calls the globally positioned DIP rectangle `BridgeDipRect`.
/// Keep the descriptive name above for diagnostics while exposing the exact
/// contract name to backend implementations.
pub type BridgeDipRect = BridgeGlobalDipRect;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OutputLocalDipRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalPxRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl BridgeGlobalDipRect {
    pub fn validate(self) -> Result<Self, BackendError> {
        if ![self.x, self.y, self.width, self.height]
            .into_iter()
            .all(f64::is_finite)
        {
            return Err(BackendError::NonFiniteGeometry);
        }
        if self.width < MIN_HOST_WIDTH
            || self.height < MIN_HOST_HEIGHT
            || self.width > MAX_HOST_WIDTH
            || self.height > MAX_HOST_HEIGHT
        {
            return Err(BackendError::InvalidGeometry);
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct InteractionRegionSnapshot {
    pub generation: u64,
    pub rects: Vec<SurfaceLocalDipRect>,
}

impl InteractionRegionSnapshot {
    pub fn validate(&self) -> Result<(), BackendError> {
        if self.rects.iter().any(|rect| !rect.is_finite() || rect.width < 0.0 || rect.height < 0.0)
        {
            return Err(BackendError::InvalidGeometry);
        }
        Ok(())
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
    pub bounds: Option<SerializedRect>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SerializedRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl TryFrom<SerializedRect> for BridgeGlobalDipRect {
    type Error = BackendError;

    fn try_from(value: SerializedRect) -> Result<Self, Self::Error> {
        BridgeGlobalDipRect {
            x: value.x,
            y: value.y,
            width: value.width,
            height: value.height,
        }
        .validate()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandEnvelope {
    pub command: serde_json::Value,
    pub created_at: u64,
}

impl CommandEnvelope {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if !self.command.is_object() || self.created_at == 0 {
            return Err(ProtocolError::InvalidEnvelope);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolError {
    SnapshotTooLarge,
    InvalidSnapshot,
    InvalidEnvelope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendError {
    NonFiniteGeometry,
    InvalidGeometry,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererKind {
    Sprite,
    Live2D,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceConfig {
    pub transparent: bool,
    pub always_on_top: bool,
    pub focusable: bool,
}

impl Default for SurfaceConfig {
    fn default() -> Self {
        Self {
            transparent: true,
            always_on_top: true,
            focusable: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DesiredOverlayState {
    pub visible: bool,
    pub fullscreen: bool,
    pub bounds: BridgeDipRect,
    pub opacity: f64,
    pub renderer: RendererKind,
    pub snapshot_revision: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppliedOverlayState {
    pub surface_exists: bool,
    pub visible: bool,
    pub bounds: BridgeDipRect,
    pub opacity: f64,
    pub renderer: RendererKind,
    pub backend_generation: u64,
    pub navigation_generation: u64,
}

impl Default for AppliedOverlayState {
    fn default() -> Self {
        Self {
            surface_exists: false,
            visible: false,
            bounds: BridgeDipRect { x: 0.0, y: 0.0, width: 300.0, height: 420.0 },
            opacity: 1.0,
            renderer: RendererKind::Sprite,
            backend_generation: 0,
            navigation_generation: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostLifecycle {
    Booting,
    Probing,
    Running,
    Suspended,
    Degraded,
    ShuttingDown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceState {
    Absent,
    Creating,
    Loading,
    Ready,
    Recovering,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputState {
    PassThrough,
    Interactive,
    Dragging,
    MenuOpen,
    TextInput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackendCapabilities {
    pub can_create_surface: bool,
    pub can_update_input_region: bool,
}

impl Default for BackendCapabilities {
    fn default() -> Self {
        Self {
            can_create_surface: true,
            can_update_input_region: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Effect {
    AbortDrag,
    DestroySurface,
    HideSurface,
    ShowSurface,
    CreateSurface,
    LoadRenderer(RendererKind),
    UpdateGeometry(BridgeDipRect),
    SetOpacity(f64),
}

/// Resolve host intent into ordered, idempotent backend effects.  The
/// high-priority branches mirror the architecture contract: shutdown and
/// suspension win over visibility, and a drag is aborted before hiding or
/// destroying its surface.
pub fn reconcile(
    desired: &DesiredOverlayState,
    applied: &AppliedOverlayState,
    lifecycle: HostLifecycle,
    surface: SurfaceState,
    input: InputState,
    capabilities: &BackendCapabilities,
) -> Vec<Effect> {
    let mut effects = Vec::new();
    let needs_abort = input == InputState::Dragging
        && (matches!(lifecycle, HostLifecycle::ShuttingDown | HostLifecycle::Suspended)
            || (!desired.visible && !desired.fullscreen)
            || desired.fullscreen);
    if needs_abort {
        effects.push(Effect::AbortDrag);
    }

    if lifecycle == HostLifecycle::ShuttingDown {
        if applied.surface_exists {
            effects.push(Effect::DestroySurface);
        }
        return effects;
    }
    if lifecycle == HostLifecycle::Suspended {
        if applied.surface_exists && applied.visible {
            effects.push(Effect::HideSurface);
        }
        return effects;
    }
    if !desired.visible && !desired.fullscreen {
        if applied.surface_exists {
            effects.push(Effect::DestroySurface);
        }
        return effects;
    }
    if desired.fullscreen {
        if applied.surface_exists && applied.visible {
            effects.push(Effect::HideSurface);
        }
        return effects;
    }
    if !applied.surface_exists && capabilities.can_create_surface {
        effects.push(Effect::CreateSurface);
    }
    if applied.surface_exists
        && matches!(surface, SurfaceState::Recovering | SurfaceState::Loading | SurfaceState::Ready)
        && applied.renderer != desired.renderer
    {
        effects.push(Effect::LoadRenderer(desired.renderer));
    }
    if applied.bounds != desired.bounds {
        effects.push(Effect::UpdateGeometry(desired.bounds));
    }
    if (applied.opacity - desired.opacity).abs() > f64::EPSILON {
        effects.push(Effect::SetOpacity(desired.opacity));
    }
    if applied.surface_exists && !applied.visible {
        effects.push(Effect::ShowSurface);
    }
    effects
}

/// Parse only the bounded, versioned snapshot payload.  A caller must keep
/// the previous successful signature when this returns an error; parse failure
/// must never poison SnapshotReader retry state.
pub fn parse_snapshot(bytes: &[u8]) -> Result<PetSnapshot, ProtocolError> {
    if bytes.len() > MAX_SNAPSHOT_BYTES {
        return Err(ProtocolError::SnapshotTooLarge);
    }
    let snapshot: PetSnapshot = serde_json::from_slice(bytes).map_err(|_| ProtocolError::InvalidSnapshot)?;
    if snapshot.format_version != SNAPSHOT_FORMAT_VERSION {
        return Err(ProtocolError::InvalidSnapshot);
    }
    if let Some(bounds) = snapshot.bounds {
        BridgeGlobalDipRect::try_from(bounds).map_err(|_| ProtocolError::InvalidSnapshot)?;
    }
    Ok(snapshot)
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppliedState {
    pub navigation_generation: u64,
    pub snapshot_generation: u64,
    pub renderer: Option<String>,
    pub bounds: Option<BridgeGlobalDipRect>,
}

impl Default for AppliedState {
    fn default() -> Self {
        Self {
            navigation_generation: 0,
            snapshot_generation: 0,
            renderer: None,
            bounds: None,
        }
    }
}

impl AppliedState {
    pub fn begin_navigation(&mut self) {
        self.navigation_generation = self.navigation_generation.saturating_add(1);
    }

    /// Apply only current-or-newer host state.  Delayed callbacks from an old
    /// WebView generation are rejected before they can snap the window back.
    pub fn apply(&mut self, snapshot: &PetSnapshot) -> Result<bool, ProtocolError> {
        if snapshot.generation < self.snapshot_generation {
            return Ok(false);
        }
        let bounds = snapshot
            .bounds
            .map(BridgeGlobalDipRect::try_from)
            .transpose()
            .map_err(|_| ProtocolError::InvalidSnapshot)?;
        self.snapshot_generation = snapshot.generation;
        self.renderer = snapshot.renderer.clone();
        self.bounds = bounds;
        Ok(true)
    }
}

/// The production backends will implement this trait only after Phase 0
/// physical validation.  Keeping the contract here makes the type boundary
/// reviewable without pretending that a desktop backend has been certified.
pub trait PlatformBackend: Send + Sync {
    fn init(&mut self) -> Result<(), BackendError>;
    fn create_surface(&mut self, config: &SurfaceConfig) -> Result<(), BackendError>;
    fn update_geometry(&mut self, rect: &BridgeDipRect) -> Result<(), BackendError>;
    fn update_input_region(&mut self, snapshot: &InteractionRegionSnapshot) -> Result<(), BackendError>;
    fn set_visible(&mut self, visible: bool) -> Result<(), BackendError>;
    fn set_opacity(&mut self, opacity: f64) -> Result<(), BackendError>;
    fn destroy_surface(&mut self) -> Result<(), BackendError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(generation: u64, x: f64) -> PetSnapshot {
        PetSnapshot {
            format_version: SNAPSHOT_FORMAT_VERSION,
            visible: true,
            fullscreen: false,
            generation,
            renderer: Some("live2d".to_string()),
            bounds: Some(SerializedRect { x, y: 0.0, width: 300.0, height: 420.0 }),
        }
    }

    #[test]
    fn stale_snapshot_cannot_snap_state_back() {
        let mut state = AppliedState::default();
        assert!(state.apply(&snapshot(2, 200.0)).unwrap());
        assert!(!state.apply(&snapshot(1, 10.0)).unwrap());
        assert_eq!(state.bounds.unwrap().x, 200.0);
    }

    #[test]
    fn malformed_snapshot_does_not_look_valid() {
        let raw = br#"{"format_version":1,"bounds":{"x":0,"y":0,"width":1,"height":1}}"#;
        assert_eq!(parse_snapshot(raw), Err(ProtocolError::InvalidSnapshot));
    }

    #[test]
    fn command_envelope_has_no_extra_protocol_namespace() {
        let value = CommandEnvelope {
            command: serde_json::json!({"type": "open-menu"}),
            created_at: 1,
        };
        assert!(value.validate().is_ok());
        assert!(CommandEnvelope { created_at: 1, command: serde_json::json!(null) }.validate().is_err());
    }

    #[test]
    fn reconcile_aborts_drag_before_fullscreen_hide() {
        let desired = DesiredOverlayState {
            visible: true,
            fullscreen: true,
            bounds: BridgeDipRect { x: 0.0, y: 0.0, width: 300.0, height: 420.0 },
            opacity: 1.0,
            renderer: RendererKind::Sprite,
            snapshot_revision: 2,
        };
        let mut applied = AppliedOverlayState::default();
        applied.surface_exists = true;
        applied.visible = true;
        let effects = reconcile(
            &desired,
            &applied,
            HostLifecycle::Running,
            SurfaceState::Ready,
            InputState::Dragging,
            &BackendCapabilities::default(),
        );
        assert_eq!(effects, vec![Effect::AbortDrag, Effect::HideSurface]);
    }
}
