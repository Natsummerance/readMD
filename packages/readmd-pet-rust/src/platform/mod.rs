use crate::error::HostResult;
use crate::protocol::SnapshotBounds;
use tao::window::{Window, WindowBuilder};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InteractionRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct InteractionRegionSnapshot {
    pub generation: u64,
    pub rects: Vec<InteractionRect>,
}

/// This trait intentionally has no `Send + Sync` bound. Tao windows, native
/// handles, and WRY WebViews belong to the event-loop thread.
pub trait PlatformBackend {
    fn name(&self) -> &'static str;
    fn init(&mut self, window: &Window) -> HostResult<()>;
    fn set_bounds(&mut self, window: &Window, bounds: SnapshotBounds) -> HostResult<()>;
    /// The rect `set_bounds` actually applied.  A backend that adjusts the
    /// requested position (keeping the pet on screen, for instance) has to be
    /// able to say so, otherwise the host echoes geometry the application then
    /// persists and the pet reappears somewhere else on the next launch.
    /// `None` means the backend applied the request unchanged.
    fn applied_bounds(&self) -> Option<SnapshotBounds> {
        None
    }
    fn set_visible(&mut self, window: &Window, visible: bool) -> HostResult<()>;
    fn set_opacity(&mut self, window: &Window, opacity: f64) -> HostResult<()>;
    fn set_click_through(&mut self, window: &Window, click_through: bool) -> HostResult<()>;
    fn set_focusable(&mut self, window: &Window, focusable: bool) -> HostResult<()>;
    fn update_interaction_regions(
        &mut self,
        window: &Window,
        regions: &InteractionRegionSnapshot,
    ) -> HostResult<()>;
    fn show_context_menu(&mut self, _window: &Window) -> HostResult<()> {
        Ok(())
    }
    fn drag_window(&self, window: &Window) -> HostResult<()> {
        let _ = window.drag_window();
        Ok(())
    }
    fn win32_hwnd(&self) -> Option<isize> {
        None
    }
}

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::WindowsBackend;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::MacOsBackend;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::{select_linux_backend, GnomeCompanionBackend, LayerShellBackend, X11Backend};

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
mod fallback;

pub fn configure_builder(builder: WindowBuilder) -> WindowBuilder {
    configure_builder_platform(builder)
}

#[cfg(windows)]
fn configure_builder_platform(builder: WindowBuilder) -> WindowBuilder {
    use tao::platform::windows::WindowBuilderExtWindows;
    builder
        .with_skip_taskbar(true)
        .with_undecorated_shadow(false)
}

#[cfg(target_os = "macos")]
fn configure_builder_platform(builder: WindowBuilder) -> WindowBuilder {
    use tao::platform::macos::WindowBuilderExtMacOS;
    builder.with_skip_taskbar(true)
}

#[cfg(target_os = "linux")]
fn configure_builder_platform(builder: WindowBuilder) -> WindowBuilder {
    use tao::platform::unix::WindowBuilderExtUnix;
    builder.with_skip_taskbar(true)
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
fn configure_builder_platform(builder: WindowBuilder) -> WindowBuilder {
    builder
}

#[cfg(windows)]
pub fn create_backend(_session_token: &str) -> Box<dyn PlatformBackend> {
    Box::new(WindowsBackend::default())
}

#[cfg(target_os = "macos")]
pub fn create_backend(_session_token: &str) -> Box<dyn PlatformBackend> {
    Box::new(MacOsBackend::default())
}

#[cfg(target_os = "linux")]
pub fn create_backend(session_token: &str) -> Box<dyn PlatformBackend> {
    select_linux_backend(session_token)
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
pub fn create_backend(_session_token: &str) -> Box<dyn PlatformBackend> {
    Box::new(fallback::FallbackBackend::default())
}
