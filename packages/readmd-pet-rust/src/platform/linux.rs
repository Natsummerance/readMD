use super::{InteractionRegionSnapshot, PlatformBackend};
use crate::error::{HostError, HostResult};
use crate::protocol::SnapshotBounds;
use gtk::{cairo, prelude::*};
use std::env;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use tao::dpi::{LogicalPosition, LogicalSize};
use tao::platform::unix::WindowExtUnix;
use tao::window::Window;

#[derive(Default)]
pub struct X11Backend {
    initialized: bool,
    click_through: bool,
    regions: InteractionRegionSnapshot,
}

pub struct LayerShellBackend {
    initialized: bool,
    click_through: bool,
    regions: InteractionRegionSnapshot,
    namespace: String,
}

pub struct GnomeCompanionBackend {
    initialized: bool,
    click_through: bool,
    regions: InteractionRegionSnapshot,
    socket: Option<PathBuf>,
    token: String,
}

impl Default for LayerShellBackend {
    fn default() -> Self {
        Self {
            initialized: false,
            click_through: true,
            regions: InteractionRegionSnapshot::default(),
            namespace: "readmd-pet".into(),
        }
    }
}

impl Default for GnomeCompanionBackend {
    fn default() -> Self {
        let socket = env::var_os("READMD_GNOME_PET_SOCKET")
            .map(PathBuf::from)
            .or_else(|| {
                env::var_os("XDG_RUNTIME_DIR")
                    .map(|root| PathBuf::from(root).join("readmd-pet-gnome.sock"))
            });
        Self {
            initialized: false,
            click_through: true,
            regions: InteractionRegionSnapshot::default(),
            socket,
            token: env::var("READMD_PET_SESSION_TOKEN").unwrap_or_default(),
        }
    }
}

fn common_bounds(window: &Window, bounds: SnapshotBounds) {
    let bounds = bounds.clamp_host();
    window.set_outer_position(LogicalPosition::new(bounds.x, bounds.y));
    window.set_inner_size(LogicalSize::new(bounds.width, bounds.height));
}

fn apply_input_region(window: &Window, regions: &InteractionRegionSnapshot) -> HostResult<()> {
    let gtk_window = window.gtk_window();
    if regions.rects.is_empty() {
        gtk_window.input_shape_combine_region(None);
        return Ok(());
    }
    let scale = f64::from(gtk_window.scale_factor()).max(1.0);
    let mut native_rects = Vec::with_capacity(regions.rects.len());
    for rect in &regions.rects {
        let left = (rect.x.max(0.0) * scale).floor() as i32;
        let top = (rect.y.max(0.0) * scale).floor() as i32;
        let right = ((rect.x + rect.width).max(0.0) * scale).ceil() as i32;
        let bottom = ((rect.y + rect.height).max(0.0) * scale).ceil() as i32;
        if right > left && bottom > top {
            native_rects.push(cairo::RectangleInt::new(
                left,
                top,
                right - left,
                bottom - top,
            ));
        }
    }
    if native_rects.is_empty() {
        // GTK treats a one-pixel region as an empty practical hit target;
        // using it avoids accidentally restoring input for invalid rectangles.
        let empty = cairo::Region::create_rectangle(&cairo::RectangleInt::new(0, 0, 1, 1));
        gtk_window.input_shape_combine_region(Some(&empty));
        return Ok(());
    }
    let region = cairo::Region::create_rectangles(&native_rects);
    gtk_window.input_shape_combine_region(Some(&region));
    Ok(())
}

fn common_input(
    window: &Window,
    click_through: bool,
    regions: &InteractionRegionSnapshot,
) -> HostResult<()> {
    // An interaction region is the native hit-test boundary. `click_through`
    // only controls the whole-window fallback when no renderer surface exists.
    apply_input_region(window, regions)?;
    window
        .set_ignore_cursor_events(click_through && regions.rects.is_empty())
        .map_err(|error| HostError::Backend(format!("ignore_cursor_events:{error}")))
}

impl PlatformBackend for X11Backend {
    fn name(&self) -> &'static str {
        "x11"
    }
    fn init(&mut self, _window: &Window) -> HostResult<()> {
        self.initialized = true;
        Ok(())
    }
    fn set_bounds(&mut self, window: &Window, bounds: SnapshotBounds) -> HostResult<()> {
        common_bounds(window, bounds);
        Ok(())
    }
    fn set_visible(&mut self, window: &Window, visible: bool) -> HostResult<()> {
        window.set_visible(visible);
        Ok(())
    }
    fn set_opacity(&mut self, _window: &Window, _opacity: f64) -> HostResult<()> {
        Ok(())
    }
    fn set_click_through(&mut self, window: &Window, click_through: bool) -> HostResult<()> {
        self.click_through = click_through;
        common_input(window, click_through, &self.regions)
    }
    fn set_focusable(&mut self, window: &Window, focusable: bool) -> HostResult<()> {
        window.set_focusable(focusable);
        Ok(())
    }
    fn update_interaction_regions(
        &mut self,
        window: &Window,
        regions: &InteractionRegionSnapshot,
    ) -> HostResult<()> {
        self.regions = regions.clone();
        apply_input_region(window, regions)?;
        Ok(())
    }
}

impl PlatformBackend for LayerShellBackend {
    fn name(&self) -> &'static str {
        "layer-shell"
    }
    fn init(&mut self, window: &Window) -> HostResult<()> {
        // Tao exposes its GTK window. The layer-shell init happens while the
        // surface is hidden, before the first production show operation.
        let gtk_window = window.gtk_window();
        if !gtk_layer_shell::is_supported() {
            return Err(HostError::Backend("layer_shell_not_supported".into()));
        }
        gtk_layer_shell::init_for_window(gtk_window);
        gtk_layer_shell::set_namespace(gtk_window, &self.namespace);
        gtk_layer_shell::set_layer(gtk_window, gtk_layer_shell::Layer::Overlay);
        gtk_layer_shell::set_anchor(gtk_window, gtk_layer_shell::Edge::Left, false);
        gtk_layer_shell::set_anchor(gtk_window, gtk_layer_shell::Edge::Right, false);
        gtk_layer_shell::set_anchor(gtk_window, gtk_layer_shell::Edge::Top, false);
        gtk_layer_shell::set_anchor(gtk_window, gtk_layer_shell::Edge::Bottom, false);
        gtk_layer_shell::set_exclusive_zone(gtk_window, -1);
        gtk_layer_shell::set_keyboard_interactivity(gtk_window, false);
        self.initialized = true;
        Ok(())
    }
    fn set_bounds(&mut self, window: &Window, bounds: SnapshotBounds) -> HostResult<()> {
        common_bounds(window, bounds);
        Ok(())
    }
    fn set_visible(&mut self, window: &Window, visible: bool) -> HostResult<()> {
        window.set_visible(visible);
        Ok(())
    }
    fn set_opacity(&mut self, _window: &Window, _opacity: f64) -> HostResult<()> {
        Ok(())
    }
    fn set_click_through(&mut self, window: &Window, click_through: bool) -> HostResult<()> {
        self.click_through = click_through;
        common_input(window, click_through, &self.regions)
    }
    fn set_focusable(&mut self, window: &Window, focusable: bool) -> HostResult<()> {
        window.set_focusable(focusable);
        Ok(())
    }
    fn update_interaction_regions(
        &mut self,
        window: &Window,
        regions: &InteractionRegionSnapshot,
    ) -> HostResult<()> {
        self.regions = regions.clone();
        apply_input_region(window, regions)?;
        Ok(())
    }
}

impl GnomeCompanionBackend {
    fn send(&self, method: &str, params: serde_json::Value) {
        let Some(path) = &self.socket else {
            return;
        };
        let Ok(mut stream) = UnixStream::connect(path) else {
            return;
        };
        let message = serde_json::json!({"version":1,"app_id":"com.readmd.desktop","session_token":self.token,"method":method,"params":params});
        let bytes = serde_json::to_vec(&message).unwrap_or_default();
        let _ = stream.write_all(&bytes);
        let _ = stream.write_all(b"\n");
    }
}

impl PlatformBackend for GnomeCompanionBackend {
    fn name(&self) -> &'static str {
        "gnome-companion"
    }
    fn init(&mut self, _window: &Window) -> HostResult<()> {
        self.initialized = true;
        self.send("register", serde_json::json!({"pid":std::process::id()}));
        Ok(())
    }
    fn set_bounds(&mut self, window: &Window, bounds: SnapshotBounds) -> HostResult<()> {
        common_bounds(window, bounds);
        self.send("set-bounds", serde_json::json!(bounds));
        Ok(())
    }
    fn set_visible(&mut self, window: &Window, visible: bool) -> HostResult<()> {
        window.set_visible(visible);
        self.send(if visible { "show" } else { "hide" }, serde_json::json!({}));
        Ok(())
    }
    fn set_opacity(&mut self, _window: &Window, opacity: f64) -> HostResult<()> {
        self.send("set-opacity", serde_json::json!({"opacity":opacity}));
        Ok(())
    }
    fn set_click_through(&mut self, window: &Window, click_through: bool) -> HostResult<()> {
        self.click_through = click_through;
        common_input(window, click_through, &self.regions)
    }
    fn set_focusable(&mut self, window: &Window, focusable: bool) -> HostResult<()> {
        window.set_focusable(focusable);
        self.send("set-focusable", serde_json::json!({"focusable":focusable}));
        Ok(())
    }
    fn update_interaction_regions(
        &mut self,
        _window: &Window,
        regions: &InteractionRegionSnapshot,
    ) -> HostResult<()> {
        self.regions = regions.clone();
        self.send("set-input-regions", serde_json::json!({"generation":regions.generation,"rects":regions.rects.iter().map(|r| serde_json::json!({"x":r.x,"y":r.y,"width":r.width,"height":r.height})).collect::<Vec<_>>() }));
        Ok(())
    }
}

pub fn select_linux_backend(session_token: &str) -> Box<dyn PlatformBackend> {
    let desktop = env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_ascii_lowercase();
    let session = env::var("DESKTOP_SESSION")
        .unwrap_or_default()
        .to_ascii_lowercase();
    // GNOME's shell owns layer-shell policy through the companion extension;
    // never select gtk-layer-shell for a GNOME session.
    if desktop.contains("gnome") || session.contains("gnome") {
        let mut backend = GnomeCompanionBackend::default();
        if !session_token.is_empty() {
            backend.token = session_token.to_string();
        }
        return Box::new(backend);
    }
    if env::var_os("WAYLAND_DISPLAY").is_some()
        && env::var("READMD_PET_LAYER_SHELL").ok().as_deref() != Some("0")
    {
        return Box::new(LayerShellBackend::default());
    }
    Box::new(X11Backend::default())
}
