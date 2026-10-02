use crate::bridge::{
    spawn_parent_watcher, DurableCommandPublisher, HealthWriter, SnapshotReader, SnapshotUpdate,
};
use crate::error::{HostError, HostResult};
use crate::input::{HitTestTarget, InputEvent, InputRect, InputWatcher};
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
    InputActivity(bool),
    CursorHover(bool),
    Bongo(crate::input::BongoInputState),
    InputFault(&'static str),
    DragStart,
    PetClick,
}

pub struct PetHost;

impl PetHost {
    pub fn run(config: HostConfig) -> HostResult<()> {
        // `RustPetRuntime.start` deletes the previous health report, spawns this
        // process and then accepts nothing but a `ready` state written by our own
        // PID (`runtime.py:410-426`, `runtime.py:508-536`).  A host that cannot
        // write health is therefore guaranteed to be torn down as
        // `rust_health_timeout`; say why before spending time on a window.
        let publisher = DurableCommandPublisher::new(&config.bridge_file);
        let health = HealthWriter::new(&config.bridge_file);
        let mut state = HostState::new(config.renderer, config.session_token.clone());
        health
            .write(HealthWriter::new_state(
                "booting",
                state.renderer.query_value(),
                "host_starting",
                state.navigation_generation,
            ))
            .map_err(|error| HostError::Backend(format!("health_unwritable:{error}")))?;
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
                // No caption buttons even if tao re-applies its own styles.
                .with_maximizable(false)
                .with_minimizable(false)
                .with_closable(false)
                .with_always_on_top(true)
                .with_focused(false)
                .with_inner_size(LogicalSize::new(320.0, 380.0)),
        );
        let window = builder
            .build(&event_loop)
            .map_err(|error| HostError::Backend(format!("window_create:{error}")))?;
        // The GNOME companion authenticates both the application id and the
        // per-host session token. Pass the token directly to the backend rather
        // than mutating the process environment.
        let mut backend = create_backend(&config.session_token);
        backend.init(&window)?;
        backend.set_bounds(&window, SnapshotBounds::default())?;
        backend.set_click_through(&window, false)?;
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

        let input_proxy = event_loop.create_proxy();
        let input_watcher = InputWatcher::new(move |event| match event {
            InputEvent::Activity(active) => {
                let _ = input_proxy.send_event(UserEvent::InputActivity(active));
            }
            InputEvent::Hover(hovered) => {
                let _ = input_proxy.send_event(UserEvent::CursorHover(hovered));
            }
            InputEvent::Bongo(bongo_state) => {
                let _ = input_proxy.send_event(UserEvent::Bongo(bongo_state));
            }
            InputEvent::Fault(code) => {
                let _ = input_proxy.send_event(UserEvent::InputFault(code));
            }
            InputEvent::DragStart => {
                let _ = input_proxy.send_event(UserEvent::DragStart);
            }
            InputEvent::PetClick => {
                let _ = input_proxy.send_event(UserEvent::PetClick);
            }
        });

        event_loop.run(move |event, _target, control_flow| {
            *control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(100));
            if crate::platform::take_completed_drag() {
                if let Ok(pos) = window.outer_position() {
                    let scale = window.scale_factor().max(0.1);
                    state.bounds.x = pos.x as f64 / scale;
                    state.bounds.y = pos.y as f64 / scale;
                    state.pending_drag = Some((state.bounds, Instant::now()));
                    if let Err(error) =
                        commit_bounds(&window, backend.as_mut(), &mut state, &publisher)
                    {
                        write_health(&health, &state, "degraded", &format_error(&error));
                    }
                    sync_input_watcher(&input_watcher, backend.as_ref(), &window, &state);
                }
            }
            match event {
                Event::NewEvents(StartCause::Init) => {
                    write_health(&health, &state, "loading", "window_ready");
                }
                Event::UserEvent(UserEvent::InputActivity(active)) => {
                    if let Some(snapshot) = state.last_snapshot.as_mut() {
                        if let Some(obj) = snapshot.as_object_mut() {
                            let mut activity = obj
                                .get("activity")
                                .cloned()
                                .unwrap_or(serde_json::json!({}));
                            if let Some(act_obj) = activity.as_object_mut() {
                                act_obj.insert(
                                    "toolRunning".to_string(),
                                    serde_json::Value::Bool(active),
                                );
                            }
                            obj.insert("activity".to_string(), activity);
                            let _ = webview.send_state(snapshot);
                        }
                    }
                }
                Event::UserEvent(UserEvent::CursorHover(hovered)) => {
                    let _ = backend.set_click_through(&window, !hovered);
                }
                Event::UserEvent(UserEvent::Bongo(bongo_state)) => {
                    let _ = webview.send_bongo_input(&bongo_state);
                }
                Event::UserEvent(UserEvent::InputFault(code)) => {
                    state.input_fault = Some(code);
                    write_health(&health, &state, "degraded", code);
                }
                Event::UserEvent(UserEvent::DragStart) => {
                    if let Err(error) = backend.drag_window(&window) {
                        write_health(&health, &state, "degraded", &format_error(&error));
                    }
                }
                Event::UserEvent(UserEvent::PetClick) => {
                    let _ = webview.send_control(&serde_json::json!({"type":"pet"}));
                }
                Event::UserEvent(UserEvent::Snapshot(update)) => {
                    if let Err(error) = maybe_recover_renderer(&mut webview, &mut state) {
                        write_health(&health, &state, "failed", &format_error(&error));
                    }
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
                        sync_input_watcher(&input_watcher, backend.as_ref(), &window, &state);
                    }
                }
                Event::UserEvent(UserEvent::Renderer(message)) => {
                    if let Err(error) = maybe_recover_renderer(&mut webview, &mut state) {
                        write_health(&health, &state, "failed", &format_error(&error));
                    }
                    if let Err(error) = handle_renderer_message(
                        &window,
                        backend.as_mut(),
                        &mut webview,
                        &mut state,
                        &publisher,
                        message,
                    ) {
                        write_health(
                            &health,
                            &state,
                            if state.renderer_circuit_open {
                                "failed"
                            } else {
                                "degraded"
                            },
                            &format_error(&error),
                        );
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
                    sync_input_watcher(&input_watcher, backend.as_ref(), &window, &state);
                }
                Event::UserEvent(UserEvent::ParentGone) => {
                    let _ = backend.set_visible(&window, false);
                    write_health(&health, &state, "stopped", "parent_eof");
                    *control_flow = ControlFlow::Exit;
                }
                Event::MainEventsCleared => {
                    if let Err(error) = maybe_recover_renderer(&mut webview, &mut state) {
                        write_health(&health, &state, "failed", &format_error(&error));
                    }
                    if health.consecutive_failures() >= HEALTH_FAILURE_WRITE_LIMIT {
                        // The app can no longer observe or control this host; it
                        // would be torn down as `rust_health_timeout` anyway.
                        // Exit loudly instead of leaving an orphan overlay on
                        // top of every window.
                        eprintln!("readmd-pet: health reporting lost, exiting");
                        *control_flow = ControlFlow::Exit;
                    }
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
    renderer_failures: u32,
    renderer_retry_at: Option<Instant>,
    renderer_circuit_open: bool,
    interaction_rects: Vec<InputRect>,
    interaction_head: Option<InputRect>,
    input_fault: Option<&'static str>,
    pending_drag: Option<(SnapshotBounds, Instant)>,
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
            visible: true,
            renderer_failures: 0,
            renderer_retry_at: None,
            renderer_circuit_open: false,
            interaction_rects: Vec::new(),
            interaction_head: None,
            input_fault: None,
            pending_drag: None,
        }
    }
}

fn sync_input_watcher(
    watcher: &InputWatcher,
    backend: &dyn PlatformBackend,
    window: &tao::window::Window,
    state: &HostState,
) {
    let hwnd = backend.win32_hwnd().unwrap_or(0);
    watcher.update_target(HitTestTarget {
        hwnd,
        window_x: state.bounds.x,
        window_y: state.bounds.y,
        width: state.bounds.width,
        height: state.bounds.height,
        scale_factor: window.scale_factor().max(0.1),
        rects: state.interaction_rects.clone(),
        head: state.interaction_head,
        visible: state.visible,
    });
}

fn mark_renderer_ready(webview: &WebViewHost, state: &mut HostState) -> HostResult<()> {
    state.renderer_ready = true;
    state.renderer_failures = 0;
    state.renderer_retry_at = None;
    state.renderer_circuit_open = false;
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

fn schedule_renderer_recovery(state: &mut HostState) {
    state.renderer_ready = false;
    state.renderer_failures = state.renderer_failures.saturating_add(1);
    if state.renderer_failures > 3 {
        state.renderer_retry_at = None;
        state.renderer_circuit_open = true;
        return;
    }
    let backoff_ms = 100u64.saturating_mul(1u64 << (state.renderer_failures - 1));
    state.renderer_retry_at = Some(Instant::now() + Duration::from_millis(backoff_ms));
}

fn maybe_recover_renderer(webview: &mut WebViewHost, state: &mut HostState) -> HostResult<()> {
    let Some(retry_at) = state.renderer_retry_at else {
        return Ok(());
    };
    if state.renderer_circuit_open || Instant::now() < retry_at {
        return Ok(());
    }
    state.renderer_retry_at = None;
    webview.reload_renderer()?;
    state.navigation_generation = webview.generation();
    state.renderer_ready = false;
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
        state.renderer_failures = 0;
        state.renderer_retry_at = None;
        state.renderer_circuit_open = false;
    }
    let bounds = protect_drag_bounds(state, snapshot.bounds.unwrap_or(state.bounds).clamp_host());
    if !crate::platform::native_drag_active() {
        backend.set_bounds(window, bounds)?;
        state.bounds = backend.applied_bounds().unwrap_or(bounds);
    }
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
    // Anything posted by the page must prove it belongs to the document this
    // host navigated.  The check used to be conditional on the stamps being
    // present, so a frame that simply omitted them was trusted and could
    // enqueue durable commands the application then executed.
    if !message.authenticated(&state.session_token, state.navigation_generation) {
        return Err(HostError::WebView("renderer_unauthenticated".into()));
    }
    let payload = message.payload;
    match message.kind.as_str() {
        "ready" | "renderer-ready" => {
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
            schedule_renderer_recovery(state);
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
            schedule_renderer_recovery(state);
            return Err(HostError::WebView(
                payload
                    .get("code")
                    .and_then(Value::as_str)
                    .unwrap_or("renderer_failed")
                    .into(),
            ));
        }
        "drag-start" => {
            start_native_drag(window, backend, state, publisher)?;
        }
        "bounds" => {
            let bounds = value_bounds(payload.get("bounds").unwrap_or(&payload))
                .ok_or_else(|| HostError::Backend("renderer_bounds_invalid".into()))?;
            state.bounds = bounds.clamp_host();
            backend.set_bounds(window, state.bounds)?;
            state.bounds = backend.applied_bounds().unwrap_or(state.bounds);
            publish(
                publisher,
                serde_json::json!({"type":"bounds","bounds":state.bounds}),
            )?;
        }
        "scale" => publish(
            publisher,
            serde_json::json!({"type":"scale","scale":payload.get("scale").cloned().unwrap_or(Value::Null)}),
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
            // Hiding the overlay is host-local state.  `close` is not in
            // `HermesPetBridge._COMMANDS` (hermes_adapter.py:81), so publishing
            // it only ever produced a file the consumer deleted unread while
            // this host reported the command as a success.
            state.visible = false;
            backend.set_visible(window, false)?;
        }
        "drop" => publish(
            publisher,
            serde_json::json!({"type":"drop","paths":payload.get("paths").cloned().unwrap_or(Value::Null)}),
        )?,
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
                if kind == "state-request" {
                    // Subscribe/replay is separate from visible readiness.
                    // A late character snapshot can still require model loading.
                    if let Some(snapshot) = state.last_snapshot.as_ref() {
                        webview.send_state(snapshot)?;
                    }
                    return Ok(());
                }
                if kind == "ready" {
                    mark_renderer_ready(webview, state)?;
                    return Ok(());
                }
                if kind == "drag-start" {
                    start_native_drag(window, backend, state, publisher)?;
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
                    schedule_renderer_recovery(state);
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
                    let rects: Vec<InteractionRect> = control
                        .get("rects")
                        .and_then(Value::as_array)
                        .map(|items| items.iter().take(128).filter_map(value_rect).collect())
                        .unwrap_or_default();
                    state.interaction_generation = generation;
                    state.interaction_head =
                        control.get("head").and_then(value_rect).map(input_rect);
                    state.interaction_rects = rects
                        .iter()
                        .map(|r| InputRect {
                            x: r.x,
                            y: r.y,
                            width: r.width,
                            height: r.height,
                        })
                        .collect();
                    let regions = InteractionRegionSnapshot { generation, rects };
                    backend.update_interaction_regions(window, &regions)?;
                    return Ok(());
                }
                if kind == "toggle-app" {
                    // `toggle-app` is the overlay's name for "paste whatever is
                    // on the clipboard". It is not a command the app consumes:
                    // the reference adapter renames it to a clipboard capture
                    // (`electron-main.ts:296`), and only `clipboard` reaches
                    // `_open_pet_clipboard` (`readmd.py:5637`).
                    publish(publisher, crate::clipboard::clipboard_command())?;
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
            let rects: Vec<InteractionRect> = payload
                .get("rects")
                .and_then(Value::as_array)
                .map(|items| items.iter().take(128).filter_map(value_rect).collect())
                .unwrap_or_default();
            state.interaction_generation = generation;
            state.interaction_head = payload.get("head").and_then(value_rect).map(input_rect);
            state.interaction_rects = rects
                .iter()
                .map(|r| InputRect {
                    x: r.x,
                    y: r.y,
                    width: r.width,
                    height: r.height,
                })
                .collect();
            let regions = InteractionRegionSnapshot { generation, rects };
            backend.update_interaction_regions(window, &regions)?;
        }
        "toggle-app" => publish(publisher, crate::clipboard::clipboard_command())?,
        "open-menu" | "submit" | "interact" | "character" => {
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

/// Apply the current position through the platform backend and publish the
/// geometry the window really ended up with.  A drag is the only chance to
/// persist the user's chosen position, so a failed write must not be swallowed.
fn commit_bounds(
    window: &tao::window::Window,
    backend: &mut dyn PlatformBackend,
    state: &mut HostState,
    publisher: &DurableCommandPublisher,
) -> HostResult<()> {
    backend.set_bounds(window, state.bounds)?;
    state.bounds = backend.applied_bounds().unwrap_or(state.bounds);
    publish(
        publisher,
        serde_json::json!({"type":"bounds","bounds":state.bounds}),
    )
}

fn publish(publisher: &DurableCommandPublisher, command: Value) -> HostResult<()> {
    // A command the application cannot consume must never be written: the
    // bridge deletes it unread, so the only signal the user would get is a pet
    // that silently stops reacting while the host health says "ok".
    let command = crate::protocol::normalise_command(&command)
        .map_err(|code| HostError::Backend(code.to_string()))?;
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

/// How many consecutive health-report failures the host tolerates before it
/// gives up.  Python's own patience is longer (`rust_health_timeout`), so this
/// only fires when the bridge directory has genuinely become unwritable.
const HEALTH_FAILURE_WRITE_LIMIT: u32 = 5;

/// Publish a lifecycle state.  Failures are counted inside [`HealthWriter`] and
/// acted on by the event loop through `consecutive_failures()`; swallowing them
/// here is what previously let an unobservable host keep painting.
fn write_health(writer: &HealthWriter, state: &HostState, lifecycle: &str, code: &str) {
    let (lifecycle, code) = if lifecycle == "ready" {
        state
            .input_fault
            .map(|fault| ("degraded", fault))
            .unwrap_or((lifecycle, code))
    } else {
        (lifecycle, code)
    };
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

fn input_rect(rect: InteractionRect) -> InputRect {
    InputRect {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
    }
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

    #[test]
    fn renderer_failure_recovery_has_bounded_backoff_and_circuit_breaker() {
        let mut state = HostState::new(RendererKind::Sprite, "session".into());
        schedule_renderer_recovery(&mut state);
        assert_eq!(state.renderer_failures, 1);
        assert!(state.renderer_retry_at.is_some());
        schedule_renderer_recovery(&mut state);
        schedule_renderer_recovery(&mut state);
        assert!(!state.renderer_circuit_open);
        schedule_renderer_recovery(&mut state);
        assert!(state.renderer_circuit_open);
        assert!(state.renderer_retry_at.is_none());
    }
}

fn start_native_drag(
    window: &tao::window::Window,
    backend: &mut dyn PlatformBackend,
    _state: &mut HostState,
    _publisher: &DurableCommandPublisher,
) -> HostResult<()> {
    backend.drag_window(window)?;
    // Other platforms retain their existing Tao drag/persistence path. Windows
    // publishes the final measured position after WM_EXITSIZEMOVE instead.
    #[cfg(not(windows))]
    {
        if let Ok(pos) = window.outer_position() {
            let scale = window.scale_factor().max(0.1);
            _state.bounds.x = pos.x as f64 / scale;
            _state.bounds.y = pos.y as f64 / scale;
        }
        commit_bounds(window, backend, _state, _publisher)?;
    }
    Ok(())
}

fn protect_drag_bounds(state: &mut HostState, mut bounds: SnapshotBounds) -> SnapshotBounds {
    if let Some((dragged, at)) = state.pending_drag {
        let acknowledged = (bounds.x - dragged.x).abs() < 0.5 && (bounds.y - dragged.y).abs() < 0.5;
        if acknowledged || at.elapsed() > Duration::from_secs(4) {
            state.pending_drag = None;
        } else {
            bounds.x = dragged.x;
            bounds.y = dragged.y;
        }
    }
    bounds
}

#[cfg(test)]
mod drag_tests {
    use super::*;

    #[test]
    fn queued_snapshot_cannot_undo_drag_and_acknowledgement_releases_guard() {
        let mut state = HostState::new(RendererKind::Sprite, "session".into());
        let dragged = SnapshotBounds {
            x: 400.0,
            y: 200.0,
            ..SnapshotBounds::default()
        };
        state.pending_drag = Some((dragged, Instant::now()));
        let old = SnapshotBounds {
            x: 100.0,
            y: 100.0,
            width: 480.0,
            height: 560.0,
        };
        let guarded = protect_drag_bounds(&mut state, old);
        assert_eq!((guarded.x, guarded.y), (400.0, 200.0));
        assert_eq!((guarded.width, guarded.height), (480.0, 560.0));
        assert!(state.pending_drag.is_some());
        assert_eq!(protect_drag_bounds(&mut state, dragged), dragged);
        assert!(state.pending_drag.is_none());
        assert_eq!(protect_drag_bounds(&mut state, old), old);
    }

    #[test]
    fn missing_acknowledgement_does_not_permanently_lock_position() {
        let mut state = HostState::new(RendererKind::Sprite, "session".into());
        state.pending_drag = Some((
            SnapshotBounds::default(),
            Instant::now() - Duration::from_secs(5),
        ));
        let requested = SnapshotBounds {
            x: 300.0,
            y: 200.0,
            ..SnapshotBounds::default()
        };
        assert_eq!(protect_drag_bounds(&mut state, requested), requested);
        assert!(state.pending_drag.is_none());
    }
}
