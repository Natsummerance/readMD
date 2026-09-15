mod ipc;

use crate::error::{HostError, HostResult};
use crate::protocol::{RendererKind, RendererMessage};
use percent_encoding::percent_decode_str;
use std::path::{Path, PathBuf};
use tao::window::Window;
use url::Url;
use wry::{DragDropEvent, WebView, WebViewBuilder};

pub use ipc::parse_renderer_message;

/// The Rust host emulates the exact Hermes preload surface consumed by the
/// existing renderer. No Node/Electron API is exposed to page JavaScript.
pub const PRELOAD_ABI: &str = r#"
(function () {
  const stateListeners = new Set();
  const controlListeners = new Set();
  const query = new URLSearchParams(window.location.search);
  const generation = Number(query.get('generation') || 0);
  const session = query.get('session') || '';
  const send = (message) => {
    try {
      const contextual = Object.assign({}, message, generation > 0 ? {generation} : {}, session ? {session} : {});
      if (contextual.type === 'control' && contextual.payload && typeof contextual.payload === 'object') {
        contextual.payload = Object.assign({}, contextual.payload, generation > 0 ? {generation} : {}, session ? {session} : {});
      }
      window.ipc && window.ipc.postMessage(JSON.stringify(contextual));
    } catch (_) {}
  };
  window.__readmdRustDispatch = {
    state: (payload) => stateListeners.forEach((callback) => { try { callback(payload); } catch (_) {} }),
    control: (payload) => controlListeners.forEach((callback) => { try { callback(payload); } catch (_) {} })
  };
  const on = (set, callback) => { set.add(callback); return () => set.delete(callback); };
  window.hermesDesktop = window.hermesDesktop || {};
  window.hermesDesktop.petOverlay = {
    open: (request) => { send({type: 'open', request: request || {}}); return Promise.resolve({ok: true}); },
    close: () => { send({type: 'close'}); return Promise.resolve({ok: true}); },
    setBounds: (bounds) => send({type: 'bounds', bounds: bounds || {}}),
    setIgnoreMouse: (ignore) => send({type: 'ignore-mouse', ignore: !!ignore}),
    setFocusable: (focusable) => send({type: 'focusable', focusable: !!focusable}),
    pushState: (payload) => send({type: 'state', payload: payload || {}}),
    control: (payload) => send({type: 'control', payload: payload || {}}),
    onState: (callback) => on(stateListeners, callback),
    onControl: (callback) => on(controlListeners, callback)
  };
  window.readmdPet = window.readmdPet || {};
  window.readmdPet.dropFiles = (files) => {
    const paths = Array.from(files || []).slice(0, 128).map((file) => file && (file.path || file.name)).filter(Boolean);
    send({type: 'drop', paths});
  };
  window.addEventListener('error', (event) => send({type: 'renderer-error', message: String(event.message || 'script_error')}));
  window.addEventListener('unhandledrejection', (event) => send({type: 'renderer-error', message: String(event.reason || 'promise_rejection')}));
  // Electron's host opens the companion menu for a native context-menu
  // request. WRY has no cross-platform context-menu callback, so preserve
  // that observable behavior at the ABI boundary and keep Chromium's own
  // editing menu from leaking into the transparent pet surface.
  window.addEventListener('contextmenu', (event) => {
    try { event.preventDefault(); } catch (_) {}
    send({type: 'control', payload: {type: 'open-menu'}});
  });
  send({type: 'abi-ready'});
})();
"#;

pub struct WebViewHost {
    pub view: WebView,
    renderer: RendererKind,
    navigation_generation: u64,
    session_token: String,
}

impl WebViewHost {
    pub fn new<F>(
        window: &Window,
        renderer_root: impl Into<PathBuf>,
        renderer: RendererKind,
        session_token: impl Into<String>,
        on_message: F,
    ) -> HostResult<Self>
    where
        F: Fn(RendererMessage) + 'static,
    {
        let renderer_root = renderer_root.into();
        let session_token = session_token.into();
        let url = renderer_url(renderer, 1, &session_token)?;
        let callback: std::rc::Rc<dyn Fn(RendererMessage)> = std::rc::Rc::new(on_message);
        let ipc_callback = callback.clone();
        let handler = move |request: wry::http::Request<String>| {
            if let Some(message) = parse_renderer_message(request.body()) {
                ipc_callback(message);
            }
        };
        let page_callback = callback.clone();
        let page_load_handler = move |event: wry::PageLoadEvent, url: String| {
            if matches!(event, wry::PageLoadEvent::Finished) {
                page_callback(crate::protocol::RendererMessage {
                    kind: "page-finished".into(),
                    payload: serde_json::json!({"url": url}),
                });
            }
        };
        let drop_callback = callback;
        let drag_handler = move |event: DragDropEvent| {
            if let DragDropEvent::Drop { paths, .. } = event {
                let payload = serde_json::json!({"paths":paths.iter().map(|path| path.to_string_lossy().to_string()).collect::<Vec<_>>()});
                drop_callback(crate::protocol::RendererMessage {
                    kind: "drop".into(),
                    payload,
                });
            }
            true
        };
        let protocol_root = renderer_root.clone();
        let builder = WebViewBuilder::new()
            .with_transparent(true)
            // WebView2 does not dispatch document-start IPC while its
            // controller is created hidden. The native window is hidden
            // immediately after construction by the host when the snapshot
            // says the pet is not visible.
            .with_visible(true)
            .with_initialization_script(PRELOAD_ABI)
            .with_ipc_handler(handler)
            .with_on_page_load_handler(page_load_handler)
            .with_drag_drop_handler(drag_handler)
            .with_custom_protocol(
                "readmd-pet".into(),
                move |_id, request| match asset_response(&protocol_root, request) {
                    Ok(response) => response.map(Into::into),
                    Err(error) => wry::http::Response::builder()
                        .status(404)
                        .header("Content-Type", "text/plain")
                        .body(error.into_bytes())
                        .expect("static error response")
                        .map(Into::into),
                },
            )
            .with_url(url.as_str());
        #[cfg(any(
            target_os = "windows",
            target_os = "macos",
            target_os = "ios",
            target_os = "android"
        ))]
        let view = builder
            .build(window)
            .map_err(|error| HostError::WebView(error.to_string()))?;
        #[cfg(not(any(
            target_os = "windows",
            target_os = "macos",
            target_os = "ios",
            target_os = "android"
        )))]
        let view = {
            use tao::platform::unix::WindowExtUnix;
            use wry::WebViewBuilderExtUnix;
            let vbox = window
                .default_vbox()
                .ok_or_else(|| HostError::WebView("gtk_default_vbox_missing".into()))?;
            builder
                .build_gtk(vbox)
                .map_err(|error| HostError::WebView(error.to_string()))?
        };
        Ok(Self {
            view,
            renderer,
            navigation_generation: 1,
            session_token,
        })
    }

    pub fn renderer(&self) -> RendererKind {
        self.renderer
    }
    pub fn generation(&self) -> u64 {
        self.navigation_generation
    }

    pub fn switch_renderer(&mut self, renderer: RendererKind) -> HostResult<()> {
        if renderer == self.renderer {
            return Ok(());
        }
        let generation = self.navigation_generation.saturating_add(1);
        let url = renderer_url(renderer, generation, &self.session_token)?;
        self.view
            .load_url(&navigation_url(&url))
            .map_err(|error| HostError::WebView(error.to_string()))?;
        self.renderer = renderer;
        self.navigation_generation = generation;
        Ok(())
    }

    pub fn send_state(&self, payload: &serde_json::Value) -> HostResult<()> {
        let encoded = serde_json::to_string(payload)?;
        self.view
            .evaluate_script(&format!(
                "window.__readmdRustDispatch && window.__readmdRustDispatch.state({encoded});"
            ))
            .map_err(|error| HostError::WebView(error.to_string()))
    }

    pub fn send_control(&self, payload: &serde_json::Value) -> HostResult<()> {
        let encoded = serde_json::to_string(payload)?;
        self.view
            .evaluate_script(&format!(
                "window.__readmdRustDispatch && window.__readmdRustDispatch.control({encoded});"
            ))
            .map_err(|error| HostError::WebView(error.to_string()))
    }
}

fn renderer_url(renderer: RendererKind, generation: u64, session: &str) -> HostResult<Url> {
    let mut url = Url::parse("readmd-pet://localhost/index.html")
        .map_err(|_| HostError::InvalidConfig("renderer_url_invalid".into()))?;
    url.query_pairs_mut()
        .append_pair("renderer", renderer.query_value())
        .append_pair("generation", &generation.to_string())
        .append_pair("session", session);
    Ok(url)
}

fn navigation_url(url: &Url) -> String {
    // WRY applies this custom-protocol workaround only while building a
    // WebView.  WebView2's Navigate API (used by renderer switching) needs
    // the same host form explicitly, otherwise it keeps the old document.
    #[cfg(target_os = "windows")]
    return url
        .as_str()
        .replacen("readmd-pet://localhost", "http://readmd-pet.localhost", 1);
    #[cfg(not(target_os = "windows"))]
    url.as_str().to_string()
}

fn asset_response(
    root: &Path,
    request: wry::http::Request<Vec<u8>>,
) -> Result<wry::http::Response<Vec<u8>>, String> {
    let raw = percent_decode_str(request.uri().path())
        .decode_utf8()
        .map_err(|_| "invalid_asset_path".to_string())?;
    let relative = raw.trim_start_matches('/');
    let relative_path = Path::new(relative);
    if relative.is_empty()
        || relative_path.is_absolute()
        || relative_path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
        || relative.contains('\\')
    {
        return Err("unsafe_asset_path".into());
    }
    let renderer_root = root.canonicalize().map_err(|error| error.to_string())?;
    // The renderer bundle is kept under `renderer/`, while the native
    // package deliberately keeps large Live2D models and the Cubism vendor
    // runtime as sibling directories. Expose only those two fixed siblings;
    // never turn the custom protocol into a general filesystem server.
    let (asset_root, asset_relative) = if let Some(path) = relative.strip_prefix("vendor/") {
        (
            renderer_root
                .parent()
                .unwrap_or(&renderer_root)
                .join("vendor"),
            path,
        )
    } else if let Some(path) = relative.strip_prefix("models/") {
        (
            renderer_root
                .parent()
                .unwrap_or(&renderer_root)
                .join("models"),
            path,
        )
    } else {
        (renderer_root.clone(), relative)
    };
    let asset_root = asset_root
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let candidate = asset_root
        .join(asset_relative)
        .canonicalize()
        .map_err(|error| error.to_string())?;
    if !candidate.starts_with(&asset_root) {
        return Err("unsafe_asset_path".into());
    }
    let bytes = std::fs::read(&candidate).map_err(|error| error.to_string())?;
    let content_type = match candidate
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
    {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json",
        "png" => "image/png",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    };
    wry::http::Response::builder()
        .status(200)
        .header("Content-Type", content_type)
        .body(bytes)
        .map_err(|error| error.to_string())
}
