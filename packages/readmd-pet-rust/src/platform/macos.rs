use super::{InteractionRegionSnapshot, PlatformBackend};
use crate::error::HostResult;
use crate::protocol::SnapshotBounds;
use objc2::ClassType;
use objc2_app_kit::{
    NSFloatingWindowLevel, NSPanel, NSWindow, NSWindowCollectionBehavior, NSWindowStyleMask,
};
use tao::dpi::{LogicalPosition, LogicalSize};
use tao::platform::macos::WindowExtMacOS;
use tao::window::Window;

/// macOS keeps the transparent panel on the main AppKit thread. Tao owns the
/// NSWindow while this backend applies the panel-level behavior exposed by the
/// cross-platform interface.
#[derive(Default)]
pub struct MacOsBackend {
    click_through: bool,
    focusable: bool,
    regions: InteractionRegionSnapshot,
}

impl PlatformBackend for MacOsBackend {
    fn name(&self) -> &'static str {
        "macos-nspanel"
    }
    fn init(&mut self, window: &Window) -> HostResult<()> {
        window.set_always_on_top(true);
        window.set_visible_on_all_workspaces(true);
        configure_native_panel(window)?;
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
        let raw = _window.ns_window();
        if raw.is_null() {
            return Err(crate::error::HostError::Backend(
                "macos_nswindow_unavailable".into(),
            ));
        }
        // AppKit applies alpha to the native surface, keeping the WebView's
        // transparent pixels transparent while preserving hit testing.
        unsafe {
            (&*(raw as *const NSWindow)).setAlphaValue(_opacity.clamp(0.0, 1.0));
        }
        Ok(())
    }
    fn set_click_through(&mut self, window: &Window, click_through: bool) -> HostResult<()> {
        self.click_through = click_through;
        let raw = window.ns_window();
        if raw.is_null() {
            return Err(crate::error::HostError::Backend(
                "macos_nswindow_unavailable".into(),
            ));
        }
        unsafe {
            (&*(raw as *const NSWindow)).setIgnoresMouseEvents(click_through);
        }
        window
            .set_ignore_cursor_events(click_through)
            .map_err(|error| crate::error::HostError::Backend(error.to_string()))
    }
    fn set_focusable(&mut self, window: &Window, focusable: bool) -> HostResult<()> {
        self.focusable = focusable;
        window.set_focusable(focusable);
        Ok(())
    }
    fn update_interaction_regions(
        &mut self,
        _window: &Window,
        regions: &InteractionRegionSnapshot,
    ) -> HostResult<()> {
        self.regions = regions.clone();
        Ok(())
    }
}

/// Tao creates the AppKit `NSWindow`; configure that real object as a
/// non-activating floating panel on the main event-loop thread.  Some Tao
/// versions return an `NSPanel` directly, while others return an NSWindow with
/// the equivalent non-activating style mask, so panel-only selectors are sent
/// only after an Objective-C class check.
fn configure_native_panel(window: &Window) -> HostResult<()> {
    let raw = window.ns_window();
    if raw.is_null() {
        return Err(crate::error::HostError::Backend(
            "macos_nswindow_unavailable".into(),
        ));
    }
    unsafe {
        let native = &*(raw as *const NSWindow);
        native.setStyleMask(native.styleMask() | NSWindowStyleMask::NonactivatingPanel);
        native.setLevel(NSFloatingWindowLevel);
        native.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::Stationary
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
        native.setHidesOnDeactivate(false);
        native.setHasShadow(false);
        native.setOpaque(false);
        native.setMovable(true);
        native.setMovableByWindowBackground(true);
        native.setIgnoresMouseEvents(true);
        if native.isKindOfClass(NSPanel::class()) {
            // A panel is intentionally non-activating and remains visible
            // while the host application changes focus.
            let panel = &*(raw as *const NSPanel);
            panel.setFloatingPanel(true);
            panel.setBecomesKeyOnlyIfNeeded(false);
            panel.setWorksWhenModal(true);
        }
    }
    Ok(())
}
