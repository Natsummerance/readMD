use crate::bridge::{
    spawn_parent_watcher, DurableCommandPublisher, HealthWriter, SnapshotReader, SnapshotUpdate,
};
use crate::error::{HostError, HostResult};
use crate::platform::{
    configure_builder, create_backend, InteractionRect, InteractionRegionSnapshot, PlatformBackend,
};
use crate::protocol::{PetSnapshot, RendererKind, RendererMessage, SnapshotBounds};
use crate::webview::WebViewHost;
use serde_json::Value;
use std::env;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tao::dpi::LogicalSize;
use tao::event::{Event, StartCause, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop, EventLoopBuilder, EventLoopProxy};
use tao::window::WindowBuilder;

#[derive(Debug, Clone)]
pub struct HostConfig {
    pub bridge_file: PathBuf,
    pub renderer_root: PathBuf,
    pub renderer: RendererKind,
    pub data_dir: PathBuf,
    pub parent_pipe_handle: Option<String>,
    pub parent_pid: Option<u32>,
    pub session_token: String,
}

impl HostConfig {
    pub fn from_env() -> HostResult<Self> {
        let bridge_file = env::var_os("READMD_PET_BRIDGE_FILE")
            .map(PathBuf::from)
            .or_else(|| {
                env::var_os("READMD_DATA_DIR").map(|value| {
                    PathBuf::from(value)
                        .join("pet")
                        .join("hermes-overlay-state.json")
                })
            })
            .ok_or_else(|| HostError::InvalidConfig("READMD_PET_BRIDGE_FILE is required".into()))?;
        let renderer_root = env::var_os("READMD_PET_RENDERER_ROOT")
            .map(PathBuf::from)
            .or_else(|| {
                env::var_os("READMD_PET_RUNTIME_DIR")
                    .map(|value| PathBuf::from(value).join("renderer"))
            })
            .unwrap_or_else(|| PathBuf::from("renderer"));
        let renderer = RendererKind::parse(env::var("READMD_PET_RENDERER").ok().as_deref())
            .unwrap_or(RendererKind::Sprite);
        let data_dir = env::var_os("READMD_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                bridge_file
                    .parent()
                    .unwrap_or_else(|| std::path::Path::new("."))
                    .to_path_buf()
            });
        let parent_pid = env::var("READMD_PARENT_PID")
            .ok()
            .and_then(|value| value.parse::<u32>().ok());
        let session_token = env::var("READMD_PET_SESSION_TOKEN").unwrap_or_else(|_| {
            format!(
                "{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|value| value.as_nanos())
                    .unwrap_or_default()
            )
        });
        Ok(Self {
            bridge_file,
            renderer_root,
            renderer,
            data_dir,
            parent_pipe_handle: env::var("READMD_PARENT_PIPE_HANDLE").ok(),
            parent_pid,
            session_token,
        })
    }
}

#[derive(Debug)]
enum UserEvent {
    Snapshot(SnapshotUpdate),
    ParentGone,
    Renderer(RendererMessage),
}

pub struct PetHost;

impl PetHost {
    pub fn run(config: HostConfig) -> HostResult<()> {
        let event_loop: EventLoop<UserEvent> = EventLoopBuilder::with_user_event().build();
        let proxy = event_loop.create_proxy();
        spawn_snapshot_reader(config.bridge_file.clone(), proxy.clone());
        spawn_parent_watcher(
            config.parent_pipe_handle.clone(),
            config.parent_pid,
            move || {
                let _ = proxy.send_event(UserEvent::ParentGone);
            },
        );

        let builder = configure_builder(
            WindowBuilder::new()
                .with_title("ReadMD Desktop Pet")
                // Create the native controller visible so WebView2 executes
                // document-start scripts; hide it immediately after WRY
                // attaches, before the first snapshot is applied.
                .with_visible(true)
                .with_transparent(true)
                .with_decorations(false)
                .with_resizable(false)
                .with_always_on_top(true)
                .with_focused(false)
                .with_inner_size(LogicalSize::new(320.0, 420.0)),
        );
        let window = builder
            .build(&event_loop)
            .map_err(|error| HostError::Backend(format!("window_create:{error}")))?;
        // The GNOME companion authenticates both the application id and the
        // per-host session token.  HostConfig owns the token used by WRY, so
        // make the same value available to the Linux platform backend before
        // it is constructed.
        env::set_var("READMD_PET_SESSION_TOKEN", &config.session_token);
        let mut backend = create_backend();
        backend.init(&window)?;
        backend.set_bounds(&window, SnapshotBounds::default())?;
        backend.set_click_through(&window, true)?;
        backend.set_focusable(&window, false)?;

        let renderer_proxy = event_loop.create_proxy();
        let mut webview = WebViewHost::new(
            &window,
            config.renderer_root.clone(),
            config.renderer,
            config.session_token.clone(),
            move |message| {
                let _ = renderer_proxy.send_event(UserEvent::Renderer(message));
            },
        )?;
        // Keep the surface alive through the first WRY navigation. A hidden
        // WebView2 controller can defer document-start scripts indefinitely;
        // the first bridge snapshot below immediately applies the requested
        // visibility (including the normal hidden state).
        let publisher = DurableCommandPublisher::new(&config.bridge_file);
        let health = HealthWriter::new(&config.bridge_file);
        let mut state = HostState::new(config.renderer, config.session_token.clone());
        write_health(&health, &state, "booting", "host_starting");

        event_loop.run(move |event, _target, control_flow| {
            *control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(100));
            match event {
                Event::NewEvents(StartCause::Init) => {
                    write_health(&health, &state, "loading", "window_ready");
                }
                Event::UserEvent(UserEvent::Snapshot(update)) => {
                    if update.snapshot.generation < state.snapshot_generation {
                        return;
                    }
                    if let Err(error) = apply_snapshot(
                        &window,
                        backend.as_mut(),
                        &mut webview,
                        &mut state,
                        &update.snapshot,
                    ) {
                        write_health(&health, &state, "degraded", &format_error(&error));
                    } else {
                        let snapshot_value =
                            serde_json::to_value(&update.snapshot).unwrap_or(Value::Null);
                        state.last_snapshot = Some(snapshot_value.clone());
                        let _ = webview.send_state(&snapshot_value);
                        write_health(
                            &health,
                            &state,
                            if state.renderer_ready {
                                "ready"
                            } else {
                                "loading"
                            },
                            "snapshot_applied",
                        );
                    }
                }
                Event::UserEvent(UserEvent::Renderer(message)) => {
                    if let Err(error) = handle_renderer_message(
                        &window,
                        backend.as_mut(),
                        &mut webview,
                        &mut state,
                        &publisher,
                        message,
                    ) {
                        write_health(&health, &state, "degraded", &format_error(&error));
                    } else {
                        write_health(
                            &health,
                            &state,
                            if state.renderer_ready {
                                "ready"
                            } else {
                                "loading"
                            },
                            "ok",
                        );
                    }
                }
                Event::UserEvent(UserEvent::ParentGone) => {
                    let _ = backend.set_visible(&window, false);
                    write_health(&health, &state, "stopped", "parent_eof");
                    *control_flow = ControlFlow::Exit;
                }
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => {
                    let _ = backend.set_visible(&window, false);
                    write_health(&health, &state, "stopped", "window_closed");
                    *control_flow = ControlFlow::Exit;
                }
                Event::LoopDestroyed => {
                    write_health(&health, &state, "stopped", "shutdown");
                }
                _ => {}
            }
        });
    }
}

#[derive(Debug)]
struct HostState {
    renderer: RendererKind,
    renderer_ready: bool,
    last_snapshot: Option<Value>,
    snapshot_generation: u64,
    navigation_generation: u64,
    interaction_generation: u64,
    session_token: String,
    bounds: SnapshotBounds,
    visible: bool,
}

impl HostState {
    fn new(renderer: RendererKind, session_token: String) -> Self {
        Self {
            renderer,
            renderer_ready: false,
            last_snapshot: None,
            snapshot_generation: 0,
            navigation_generation: 1,
            interaction_generation: 0,
            session_token,
            bounds: SnapshotBounds::default(),
            visible: false,
        }
    }
}

fn mark_renderer_ready(webview: &WebViewHost, state: &mut HostState) -> HostResult<()> {
    state.renderer_ready = true;
    // The renderer subscribes after its async mount. Replaying the latest
    // snapshot here closes that startup race and is required for the Sprite
    // component to receive its spritesheet before it renders.
    if let Some(snapshot) = state.last_snapshot.as_ref() {
        webview.send_state(snapshot)?;
    }
    webview.send_control(&serde_json::json!({
        "type":"host-ready",
        "renderer":state.renderer.query_value(),
        "generation":state.navigation_generation
    }))?;
    Ok(())
}

fn spawn_snapshot_reader(path: PathBuf, proxy: EventLoopProxy<UserEvent>) {
    thread::Builder::new()
        .name("readmd-pet-snapshot".into())
        .spawn(move || {
            let mut reader = SnapshotReader::new(path);
            loop {
                match reader.read() {
                    Ok(Some(update)) => {
                        if proxy.send_event(UserEvent::Snapshot(update)).is_err() {
                            break;
                        }
                    }
                    Ok(None) => {}
                    Err(error) => {
                        eprintln!("readmd-pet snapshot: {error}");
                    }
                }
                thread::sleep(Duration::from_millis(80));
            }
        })
        .ok();
}

fn apply_snapshot(
    window: &tao::window::Window,
    backend: &mut dyn PlatformBackend,
    webview: &mut WebViewHost,
    state: &mut HostState,
    snapshot: &PetSnapshot,
) -> HostResult<()> {
    state.snapshot_generation = snapshot.generation;
    let renderer = snapshot.renderer_kind();
    if renderer != state.renderer {
        webview.switch_renderer(renderer)?;
        state.renderer = renderer;
        state.navigation_generation = webview.generation();
        state.renderer_ready = false;
    }
    let bounds = snapshot.bounds.unwrap_or(state.bounds).clamp_host();
    state.bounds = bounds;
    backend.set_bounds(window, bounds)?;
    backend.set_opacity(window, snapshot.opacity())?;
    state.visible = snapshot.visible && !snapshot.fullscreen;
    backend.set_visible(window, state.visible)?;
    Ok(())
}

fn handle_renderer_message(
    window: &tao::window::Window,
    backend: &mut dyn PlatformBackend,
    webview: &mut WebViewHost,
    state: &mut HostState,
    publisher: &DurableCommandPublisher,
    message: RendererMessage,
) -> HostResult<()> {
    let payload = message.payload;
    if let Some(session) = payload.get("session").and_then(Value::as_str).or_else(|| {
        payload
            .get("payload")
            .and_then(|value| value.get("session"))
            .and_then(Value::as_str)
    }) {
        if session != state.session_token {
            return Ok(());
        }
    }
    if let Some(generation) = payload
        .get("generation")
        .and_then(Value::as_u64)
        .or_else(|| {
            payload
                .get("payload")
                .and_then(|value| value.get("generation"))
                .and_then(Value::as_u64)
        })
    {
        if generation != state.navigation_generation {
            return Ok(());
        }
    }
    match message.kind.as_str() {
        "ready" | "renderer-ready" => {
            if let Some(generation) = payload.get("generation").and_then(Value::as_u64) {
                if generation != state.navigation_generation {
                    return Ok(());
                }
            }
            mark_renderer_ready(webview, state)?;
        }
        "abi-ready" => {
            // Document-start injection succeeded. Keep the health state
            // visible while the existing renderer performs its async mount.
            state.renderer_ready = false;
        }
        "page-finished" => {
            // Navigation completion can be delivered after document-start
            // IPC and after the renderer's own ready signal. It is only a
            // diagnostic milestone; never roll a healthy renderer back to
            // loading because the browser reported the page finished late.
        }
        "renderer-error" => {
            state.renderer_ready = false;
            return Err(HostError::WebView(
                payload
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("renderer_script_failed")
                    .chars()
                    .take(128)
                    .collect(),
            ));
        }
        "renderer-failed" => {
            state.renderer_ready = false;
            return Err(HostError::WebView(
                payload
                    .get("code")
                    .and_then(Value::as_str)
                    .unwrap_or("renderer_failed")
                    .into(),
            ));
        }
        "bounds" => {
            if let Some(bounds) = value_bounds(payload.get("bounds").unwrap_or(&payload)) {
                state.bounds = bounds.clamp_host();
                backend.set_bounds(window, state.bounds)?;
                publish(
                    publisher,
                    serde_json::json!({"type":"bounds","bounds":state.bounds}),
                )?;
            }
        }
        "scale" => publish(
            publisher,
            serde_json::json!({"type":"scale","scale":payload.get("scale").and_then(Value::as_f64).unwrap_or(0.42)}),
        )?,
        "ignore-mouse" => backend.set_click_through(
            window,
            payload
                .get("ignore")
                .and_then(Value::as_bool)
                .unwrap_or(true),
        )?,
        "focusable" => backend.set_focusable(
            window,
            payload
                .get("focusable")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        )?,
        "open" => {
            state.visible = true;
            backend.set_visible(window, true)?;
            publish(publisher, serde_json::json!({"type":"open-app"}))?;
        }
        "close" => {
            state.visible = false;
            backend.set_visible(window, false)?;
            publish(publisher, serde_json::json!({"type":"close"}))?;
        }
        "drop" => {
            let paths = payload
                .get("paths")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            if paths.len() <= 128 {
                publish(publisher, serde_json::json!({"type":"drop","paths":paths}))?;
            }
        }
        "control" => {
            // `parse_renderer_message` unwraps the outer `payload` field, so
            // control callbacks normally arrive here as the control object
            // itself. Keep accepting an explicitly nested object for older
            // bridge shims.
            let control = payload.get("payload").unwrap_or(&payload);
            // Hermes sends renderer lifecycle notifications through the
            // `control` channel. They are host lifecycle signals, not
            // durable application commands; consume them here so a
            // successful mount can move health from loading to ready.
            if let Some(kind) = control.get("type").and_then(Value::as_str) {
                if kind == "ready" {
                    mark_renderer_ready(webview, state)?;
                    return Ok(());
                }
                if kind == "renderer-ready" {
                    if let Some(generation) = control.get("generation").and_then(Value::as_u64) {
                        if generation != state.navigation_generation {
                            return Ok(());
                        }
                    }
                    mark_renderer_ready(webview, state)?;
                    return Ok(());
                }
                if kind == "renderer-failed" {
                    state.renderer_ready = false;
                    return Err(HostError::WebView(
                        control
                            .get("code")
                            .and_then(Value::as_str)
                            .unwrap_or("renderer_failed")
                            .into(),
                    ));
                }
                if kind == "interaction-regions" {
                    let generation = control
                        .get("generation")
                        .and_then(Value::as_u64)
                        .unwrap_or(state.interaction_generation.saturating_add(1));
                    if generation < state.interaction_generation {
                        return Ok(());
                    }
                    let rects = control
                        .get("rects")
                        .and_then(Value::as_array)
                        .map(|items| items.iter().take(128).filter_map(value_rect).collect())
                        .unwrap_or_default();
                    state.interaction_generation = generation;
                    let regions = InteractionRegionSnapshot { generation, rects };
                    backend.update_interaction_regions(window, &regions)?;
                    backend.set_click_through(window, regions.rects.is_empty())?;
                    return Ok(());
                }
            }
            publish(publisher, control.clone())?;
        }
        "state" => {}
        "interaction-regions" => {
            let generation = payload
                .get("generation")
                .and_then(Value::as_u64)
                .unwrap_or(state.interaction_generation.saturating_add(1));
            if generation < state.interaction_generation {
                return Ok(());
            }
            let rects = payload
                .get("rects")
                .and_then(Value::as_array)
                .map(|items| items.iter().take(128).filter_map(value_rect).collect())
                .unwrap_or_default();
            state.interaction_generation = generation;
            let regions = InteractionRegionSnapshot { generation, rects };
            backend.update_interaction_regions(window, &regions)?;
            backend.set_click_through(window, regions.rects.is_empty())?;
        }
        "toggle-app" | "open-menu" | "submit" | "interact" | "character" => {
            let mut command = payload.clone();
            if !command.is_object() {
                return Ok(());
            }
            command["type"] = Value::String(message.kind.clone());
            publish(publisher, command)?;
        }
        _ => {}
    }
    Ok(())
}

fn publish(publisher: &DurableCommandPublisher, command: Value) -> HostResult<()> {
    publisher
        .publish(command)
        .map(|_| ())
        .map_err(HostError::Backend)
}

fn value_bounds(value: &Value) -> Option<SnapshotBounds> {
    Some(SnapshotBounds {
        x: value.get("x")?.as_f64()?,
        y: value.get("y")?.as_f64()?,
        width: value.get("width")?.as_f64()?,
        height: value.get("height")?.as_f64()?,
    })
}

fn value_rect(value: &Value) -> Option<InteractionRect> {
    let rect = InteractionRect {
        x: value.get("x")?.as_f64()?,
        y: value.get("y")?.as_f64()?,
        width: value.get("width")?.as_f64()?,
        height: value.get("height")?.as_f64()?,
    };
    (rect.x.is_finite()
        && rect.y.is_finite()
        && rect.width.is_finite()
        && rect.height.is_finite()
        && rect.width > 0.0
        && rect.height > 0.0)
        .then_some(rect)
}

fn write_health(writer: &HealthWriter, state: &HostState, lifecycle: &str, code: &str) {
    let _ = writer.write(HealthWriter::new_state(
        lifecycle,
        state.renderer.query_value(),
        code,
        state.navigation_generation,
    ));
}

fn format_error(error: &HostError) -> String {
    error.to_string().chars().take(128).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renderer_message_bounds_respect_host_limits() {
        let payload = serde_json::json!({"x":0,"y":0,"width":1000,"height":100});
        let bounds = value_bounds(&payload).unwrap().clamp_host();
        assert_eq!(bounds.width, 640.0);
        assert_eq!(bounds.height, 300.0);
    }

    #[test]
    fn protocol_version_is_stable() {
        assert_eq!(crate::protocol::PROTOCOL_VERSION, 1);
    }
}
