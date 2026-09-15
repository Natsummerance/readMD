use super::{InteractionRegionSnapshot, PlatformBackend};
use crate::error::HostResult;
use crate::protocol::SnapshotBounds;
use tao::dpi::{LogicalPosition, LogicalSize};
use tao::window::Window;

#[derive(Default)]
pub struct FallbackBackend;

impl PlatformBackend for FallbackBackend {
    fn name(&self) -> &'static str {
        "generic"
    }
    fn init(&mut self, _window: &Window) -> HostResult<()> {
        Ok(())
    }
    fn set_bounds(&mut self, window: &Window, bounds: SnapshotBounds) -> HostResult<()> {
        let b = bounds.clamp_host();
        window.set_outer_position(LogicalPosition::new(b.x, b.y));
        window.set_inner_size(LogicalSize::new(b.width, b.height));
        Ok(())
    }
    fn set_visible(&mut self, window: &Window, visible: bool) -> HostResult<()> {
        window.set_visible(visible);
        Ok(())
    }
    fn set_opacity(&mut self, _window: &Window, _opacity: f64) -> HostResult<()> {
        Ok(())
    }
    fn set_click_through(&mut self, window: &Window, value: bool) -> HostResult<()> {
        window
            .set_ignore_cursor_events(value)
            .map_err(|error| crate::error::HostError::Backend(error.to_string()))
    }
    fn set_focusable(&mut self, window: &Window, value: bool) -> HostResult<()> {
        window.set_focusable(value);
        Ok(())
    }
    fn update_interaction_regions(
        &mut self,
        _window: &Window,
        _regions: &InteractionRegionSnapshot,
    ) -> HostResult<()> {
        Ok(())
    }
}
