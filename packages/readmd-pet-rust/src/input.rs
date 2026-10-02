use crate::protocol::SnapshotBounds;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
#[cfg(not(windows))]
use std::time::Duration;
mod pressed;
#[cfg(windows)]
mod windows;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InputRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Default)]
pub struct HitTestTarget {
    pub hwnd: isize,
    pub window_x: f64,
    pub window_y: f64,
    /// Logical size the overlay really has, taken from the backend's applied
    /// bounds.  Hard-coding this to the original 320x380 surface made every
    /// hit test wrong as soon as the application resized or scaled the pet.
    pub width: f64,
    pub height: f64,
    pub scale_factor: f64,
    pub rects: Vec<InputRect>,
    pub pet_rects: Vec<InputRect>,
    pub regions_declared: bool,
    pub lock_position: bool,
    pub head: Option<InputRect>,
    pub visible: bool,
}

impl HitTestTarget {
    /// The pet surface in logical units, falling back to the protocol default
    /// when no geometry has been applied yet.
    pub fn effective_size(&self) -> (f64, f64) {
        if self.width.is_finite()
            && self.height.is_finite()
            && self.width > 0.0
            && self.height > 0.0
        {
            (self.width, self.height)
        } else {
            let default = SnapshotBounds::default();
            (default.width, default.height)
        }
    }
}

/// Head hit-frame as fractions of the pet surface.  These were absolute
/// numbers (`(160, 160)` radius `75`) valid only for the original 320x380
/// surface; expressed as ratios they follow the pet when it is resized.
const HEAD_CENTER_X_RATIO: f64 = 160.0 / 320.0;
const HEAD_CENTER_Y_RATIO: f64 = 160.0 / 380.0;
const HEAD_RADIUS_RATIO: f64 = 75.0 / 380.0;

/// Where the renderer's eyes rest when no cursor position is available: the
/// original `(210, 275)` inside a 320x380 surface.
const IDLE_GAZE_X_RATIO: f64 = 210.0 / 320.0;
const IDLE_GAZE_Y_RATIO: f64 = 275.0 / 380.0;

/// Result of projecting the OS cursor onto the overlay.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CursorProbe {
    /// Cursor position in the overlay's logical coordinate space.
    pub rel_x: f64,
    pub rel_y: f64,
    /// The cursor is over an interactive part of a *visible* pet.
    pub hovering: bool,
    /// The head was pressed: this is the only gesture that maps to `interact`.
    pub head_clicked: bool,
}

/// Project a physical cursor position onto the logical pet surface.
///
/// `origin` is the window's physical top-left, which on Windows comes from
/// `GetWindowRect` because a layered popup's logical position and its device
/// rectangle differ under per-monitor DPI.
pub fn probe_cursor(
    target: &HitTestTarget,
    origin: (f64, f64),
    cursor: (f64, f64),
    left_pressed: bool,
) -> CursorProbe {
    let scale = target.scale_factor.max(0.1);
    let (width, height) = target.effective_size();
    let rel_x = (cursor.0 - origin.0) / scale;
    let rel_y = (cursor.1 - origin.1) / scale;

    let mut hit = false;
    if target.visible {
        if target.rects.is_empty() && !target.regions_declared {
            hit = contains(
                &InputRect {
                    x: 0.0,
                    y: 0.0,
                    width,
                    height,
                },
                rel_x,
                rel_y,
            );
        } else {
            hit = target.rects.iter().any(|rect| contains(rect, rel_x, rel_y));
        }
    }

    let head = target.head.unwrap_or(InputRect {
        x: width * HEAD_CENTER_X_RATIO - height * HEAD_RADIUS_RATIO,
        y: height * (HEAD_CENTER_Y_RATIO - HEAD_RADIUS_RATIO),
        width: height * HEAD_RADIUS_RATIO * 2.0,
        height: height * HEAD_RADIUS_RATIO * 2.0,
    });
    let dx = (rel_x - head.x - head.width / 2.0) / (head.width / 2.0).max(0.1);
    let dy = (rel_y - head.y - head.height / 2.0) / (head.height / 2.0).max(0.1);
    CursorProbe {
        rel_x,
        rel_y,
        hovering: hit,
        head_clicked: hit && left_pressed && if target.regions_declared { target.pet_rects.iter().any(|r| contains(r, rel_x, rel_y)) } else { dx * dx + dy * dy < 1.0 },
    }
}

fn contains(rect: &InputRect, x: f64, y: f64) -> bool {
    x >= rect.x && x <= rect.x + rect.width && y >= rect.y && y <= rect.y + rect.height
}

/// Neutral gaze target for a frame where the cursor could not be read.
pub fn idle_gaze(target: &HitTestTarget) -> (f64, f64) {
    let (width, height) = target.effective_size();
    (width * IDLE_GAZE_X_RATIO, height * IDLE_GAZE_Y_RATIO)
}

#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct BongoInputState {
    pub left_down: bool,
    pub right_down: bool,
    pub mouse_down: bool,
    pub mouse_x: f64,
    pub mouse_y: f64,
    pub pet_clicked: bool,
    pub active: bool,
    pub sequence: u32,
    pub keyboard_down: bool,
    pub keyboard_taps: u32,
    pub left_taps: u32,
    pub right_taps: u32,
    pub mouse_taps: u32,
    pub mouse_buttons: u8,
    pub last_key: Option<u16>,
    pub pressed_keys: Vec<u16>,
    pub pointer_x: f64,
    pub pointer_y: f64,
}

#[derive(Debug, Clone)]
pub enum InputEvent {
    Activity(bool),
    Hover(bool),
    Bongo(BongoInputState),
    Fault(&'static str),
    DragStart,
    PetClick,
}

pub struct InputWatcher {
    target: Arc<Mutex<HitTestTarget>>,
    running: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}

impl InputWatcher {
    pub fn new<F>(on_event: F) -> Self
    where
        F: Fn(InputEvent) + Send + Sync + 'static,
    {
        let target = Arc::new(Mutex::new(HitTestTarget::default()));
        let running = Arc::new(AtomicBool::new(true));

        let target_clone = target.clone();
        let running_clone = running.clone();
        let on_event = Arc::new(on_event);
        let worker_event = on_event.clone();
        let worker = thread::Builder::new()
            .name("readmd-pet-input-watcher".into())
            .spawn(move || {
                run_watcher_loop(target_clone, running_clone, move |event| {
                    worker_event(event)
                });
            })
            .ok();
        if worker.is_none() {
            on_event(InputEvent::Fault("input_worker_start_failed"));
        }

        Self {
            target,
            running,
            worker,
        }
    }

    pub fn update_target(&self, target: HitTestTarget) {
        if let Ok(mut lock) = self.target.lock() {
            *lock = target;
        }
    }
}

impl Drop for InputWatcher {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(windows)]
fn run_watcher_loop<F>(target: Arc<Mutex<HitTestTarget>>, running: Arc<AtomicBool>, on_event: F)
where
    F: Fn(InputEvent) + Send + Sync + 'static,
{
    windows::run(target, running, on_event);
}

#[cfg(not(windows))]
fn run_watcher_loop<F>(_target: Arc<Mutex<HitTestTarget>>, running: Arc<AtomicBool>, _on_event: F)
where
    F: Fn(InputEvent) + Send + Sync + 'static,
{
    while running.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(500));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(width: f64, height: f64, scale: f64) -> HitTestTarget {
        HitTestTarget {
            hwnd: 0,
            window_x: 0.0,
            window_y: 0.0,
            width,
            height,
            scale_factor: scale,
            rects: Vec::new(),
            head: None,
            visible: true,
            ..Default::default()
        }
    }

    #[test]
    fn declared_empty_surface_and_transparent_holes_pass_through() {
        let mut pet = target(320.0, 420.0, 1.5);
        pet.regions_declared = true;
        assert!(!probe_cursor(&pet,(0.0,0.0),(160.0,160.0),true).hovering);
        pet.rects = vec![InputRect {x:100.0,y:300.0,width:40.0,height:80.0}];
        pet.pet_rects = pet.rects.clone();
        assert!(probe_cursor(&pet,(0.0,0.0),(180.0,480.0),true).head_clicked);
        assert!(!probe_cursor(&pet,(0.0,0.0),(240.0,480.0),true).hovering);
        pet.rects.push(InputRect {x:10.0,y:10.0,width:80.0,height:44.0});
        let menu = probe_cursor(&pet,(0.0,0.0),(30.0,30.0),true);
        assert!(menu.hovering);
        assert!(!menu.head_clicked, "UI clicks must not pet or start native dragging");
    }

    #[test]
    fn hit_testing_follows_the_surface_the_pet_actually_occupies() {
        // A 640x700 overlay at 150% DPI: the shipped watcher compared against a
        // hard-coded 320x380 rect, so the entire right half of a scaled-up pet
        // was dead to hover, click and drag.
        let big = target(640.0, 700.0, 1.5);
        let probe = probe_cursor(&big, (0.0, 0.0), (900.0, 900.0), false);
        assert_eq!(probe.rel_x, 600.0);
        assert_eq!(probe.rel_y, 600.0);
        assert!(probe.hovering, "600,600 is inside a 640x700 pet");
        assert!(!probe_cursor(&big, (0.0, 0.0), (1200.0, 1200.0), false).hovering);
    }

    #[test]
    fn the_head_frame_scales_with_the_pet() {
        let big = target(640.0, 700.0, 1.0);
        // Head centre is 0.5*width by 0.42*height, radius 0.197*height.
        assert!(probe_cursor(&big, (0.0, 0.0), (320.0, 295.0), true).head_clicked);
        // The old absolute head centre is now the pet's left flank.
        assert!(!probe_cursor(&big, (0.0, 0.0), (160.0, 160.0), true).head_clicked);
        // A press on the body still hovers but never counts as a head click.
        let body = probe_cursor(&big, (0.0, 0.0), (320.0, 640.0), true);
        assert!(body.hovering && !body.head_clicked);
    }

    #[test]
    fn a_hidden_overlay_is_never_hovered_or_clicked() {
        let mut hidden = target(320.0, 420.0, 1.0);
        hidden.visible = false;
        let probe = probe_cursor(&hidden, (0.0, 0.0), (160.0, 160.0), true);
        assert!(!probe.hovering);
        assert!(!probe.head_clicked);
    }

    #[test]
    fn declared_interaction_rects_take_precedence_over_the_window() {
        let mut sized = target(320.0, 420.0, 1.0);
        sized.rects = vec![InputRect {
            x: 10.0,
            y: 20.0,
            width: 40.0,
            height: 50.0,
        }];
        assert!(probe_cursor(&sized, (0.0, 0.0), (30.0, 40.0), false).hovering);
        assert!(!probe_cursor(&sized, (0.0, 0.0), (300.0, 400.0), false).hovering);
    }

    #[test]
    fn bottom_aligned_model_head_uses_the_declared_frame_at_high_dpi() {
        let mut pet = target(320.0, 420.0, 1.5);
        pet.head = Some(InputRect {
            x: 100.0,
            y: 260.0,
            width: 120.0,
            height: 90.0,
        });
        assert!(probe_cursor(&pet, (300.0, 150.0), (540.0, 607.5), true).head_clicked);
        assert!(!probe_cursor(&pet, (300.0, 150.0), (540.0, 412.5), true).head_clicked);
    }

    #[test]
    fn the_window_origin_is_respected_when_the_pet_is_not_at_zero() {
        let pet = target(320.0, 420.0, 2.0);
        // Physical (1320, 850) on a window whose physical origin is
        // (1000, 500) at 200% is logical (160, 175) -- the head centre.
        let probe = probe_cursor(&pet, (1000.0, 500.0), (1320.0, 850.0), true);
        assert_eq!((probe.rel_x, probe.rel_y), (160.0, 175.0));
        assert!(probe.hovering);
        assert!(
            probe.head_clicked,
            "(160, 175) is the head of a 320x420 pet"
        );
    }

    #[test]
    fn an_unmeasured_surface_falls_back_to_the_protocol_default() {
        assert_eq!(target(0.0, 0.0, 1.0).effective_size(), (320.0, 420.0));
        assert_eq!(
            target(f64::NAN, 420.0, 1.0).effective_size(),
            (320.0, 420.0)
        );
        assert_eq!(target(500.0, 600.0, 1.0).effective_size(), (500.0, 600.0));
    }

    #[test]
    fn the_idle_gaze_point_tracks_the_surface_size() {
        let (x, y) = idle_gaze(&target(320.0, 380.0, 1.0));
        assert!((x - 210.0).abs() < 1e-9 && (y - 275.0).abs() < 1e-9);
        let (x, y) = idle_gaze(&target(640.0, 760.0, 1.0));
        assert!((x - 420.0).abs() < 1e-9 && (y - 550.0).abs() < 1e-9);
    }
}
