use crate::protocol::SnapshotBounds;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

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
    pub visible: bool,
}

impl HitTestTarget {
    /// The pet surface in logical units, falling back to the protocol default
    /// when no geometry has been applied yet.
    pub fn effective_size(&self) -> (f64, f64) {
        if self.width.is_finite() && self.height.is_finite() && self.width > 0.0 && self.height > 0.0 {
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
        if target.rects.is_empty() {
            hit = contains(&InputRect { x: 0.0, y: 0.0, width, height }, rel_x, rel_y);
        } else {
            hit = target.rects.iter().any(|rect| contains(rect, rel_x, rel_y));
        }
    }

    let head_radius = height * HEAD_RADIUS_RATIO;
    let dx = rel_x - width * HEAD_CENTER_X_RATIO;
    let dy = rel_y - height * HEAD_CENTER_Y_RATIO;
    CursorProbe {
        rel_x,
        rel_y,
        hovering: hit,
        head_clicked: hit && left_pressed && (dx * dx + dy * dy).sqrt() < head_radius,
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
pub struct BongoInputState {
    pub left_down: bool,
    pub right_down: bool,
    pub mouse_down: bool,
    pub mouse_x: f64,
    pub mouse_y: f64,
    pub pet_clicked: bool,
    pub active: bool,
}

#[derive(Debug, Clone)]
pub enum InputEvent {
    Activity(bool),
    Hover(bool),
    Bongo(BongoInputState),
    DragMove { x: f64, y: f64 },
    DragEnd,
}

pub struct InputWatcher {
    target: Arc<Mutex<HitTestTarget>>,
    running: Arc<AtomicBool>,
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

        thread::Builder::new()
            .name("readmd-pet-input-watcher".into())
            .spawn(move || {
                run_watcher_loop(target_clone, running_clone, on_event);
            })
            .ok();

        Self { target, running }
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
    }
}

#[cfg(windows)]
fn is_left_key(vk: i32) -> bool {
    matches!(
        vk,
        // Digits: 1, 2, 3, 4, 5
        0x31..=0x35
        // Top row: Q, W, E, R, T
        | 0x51 | 0x57 | 0x45 | 0x52 | 0x54
        // Home row: A, S, D, F, G
        | 0x41 | 0x53 | 0x44 | 0x46 | 0x47
        // Bottom row: Z, X, C, V, B
        | 0x5A | 0x58 | 0x43 | 0x56 | 0x42
        // Modifiers & Controls
        | 0x09 // Tab
        | 0x14 // CapsLock
        | 0x10 | 0xA0 // Shift / LShift
        | 0x11 | 0xA2 // Ctrl / LCtrl
        | 0x12 | 0xA4 // Alt / LAlt
        | 0x20 // Space
        | 0x1B // Esc
        | 0xC0 // `~
    )
}

#[cfg(windows)]
fn is_right_key(vk: i32) -> bool {
    matches!(
        vk,
        // Digits: 6, 7, 8, 9, 0, -, =
        0x36..=0x39 | 0x30 | 0xBD | 0xBB
        // Top row: Y, U, I, O, P, [, ]
        | 0x59 | 0x55 | 0x49 | 0x4F | 0x50 | 0xDB | 0xDD
        // Home row: H, J, K, L, ;, '
        | 0x48 | 0x4A | 0x4B | 0x4C | 0xBA | 0xDE
        // Bottom row: N, M, ,, ., /
        | 0x4E | 0x4D | 0xBC | 0xBE | 0xBF
        // Controls
        | 0x08 // Backspace
        | 0x0D // Enter
        | 0xA1 // RShift
        | 0xA3 // RCtrl
        | 0xA5 // RAlt
        // Arrows & Navigation
        | 0x21..=0x28 // Prior, Next, End, Home, Left, Up, Right, Down
        | 0x2D | 0x2E // Insert, Delete
        // Numpad
        | 0x60..=0x6F
    )
}

#[cfg(windows)]
fn run_watcher_loop<F>(
    target: Arc<Mutex<HitTestTarget>>,
    running: Arc<AtomicBool>,
    on_event: F,
) where
    F: Fn(InputEvent) + Send + Sync + 'static,
{
    use windows_sys::Win32::Foundation::{HWND, POINT, RECT};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VK_LBUTTON, VK_MBUTTON, VK_RBUTTON,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetCursorPos, GetWindowRect};

    let mut is_active = false;
    let mut last_activity_time = Instant::now();
    let mut is_hovered = false;
    let mut is_dragging = false;
    let mut drag_offset_x = 0.0;
    let mut drag_offset_y = 0.0;

    let mut prev_left = false;
    let mut prev_right = false;
    let mut prev_mouse = false;
    let mut prev_lbutton_down = false;
    let mut prev_cursor_x = 0.0;
    let mut prev_cursor_y = 0.0;

    while running.load(Ordering::Relaxed) {
        let now = Instant::now();

        // 1. Precise Left / Right / Mouse button states
        let lbutton_down = unsafe { (GetAsyncKeyState(VK_LBUTTON as i32) as u16 & 0x8000) != 0 };
        let rbutton_down = unsafe { (GetAsyncKeyState(VK_RBUTTON as i32) as u16 & 0x8000) != 0 };
        let mbutton_down = unsafe { (GetAsyncKeyState(VK_MBUTTON as i32) as u16 & 0x8000) != 0 };
        let mouse_clicked = lbutton_down || rbutton_down || mbutton_down;

        let mut left_down = false;
        let mut right_down = rbutton_down || mbutton_down;

        // Scan keys for left side and right side
        for vk in 0x08..=0xFE {
            if vk == VK_LBUTTON as i32 || vk == VK_RBUTTON as i32 || vk == VK_MBUTTON as i32 {
                continue;
            }
            let key_is_down = unsafe { (GetAsyncKeyState(vk) as u16 & 0x8000) != 0 };
            if key_is_down {
                if is_left_key(vk) {
                    left_down = true;
                } else if is_right_key(vk) {
                    right_down = true;
                } else {
                    // Default unknown key to left
                    left_down = true;
                }
            }
        }

        let activity_detected = mouse_clicked || left_down || right_down;

        if activity_detected {
            last_activity_time = now;
            if !is_active {
                is_active = true;
                on_event(InputEvent::Activity(true));
            }
        } else if is_active && now.duration_since(last_activity_time) > Duration::from_millis(500) {
            is_active = false;
            on_event(InputEvent::Activity(false));
        }

        // 2. Cursor position & hit-testing against the surface the pet really
        //    occupies.
        let mut cursor_pt = POINT { x: 0, y: 0 };
        let has_cursor = unsafe { GetCursorPos(&mut cursor_pt) != 0 };

        let hit_target = target.lock().map(|g| g.clone()).unwrap_or_default();
        let scale = hit_target.scale_factor.max(0.1);
        // `GetWindowRect` is authoritative and physical; the cached bounds are
        // logical and only used when the window handle is already gone.
        let mut origin = (hit_target.window_x * scale, hit_target.window_y * scale);
        if has_cursor && hit_target.hwnd != 0 {
            let mut rect: RECT = unsafe { std::mem::zeroed() };
            if unsafe { GetWindowRect(hit_target.hwnd as HWND, &mut rect) } != 0 {
                origin = (rect.left as f64, rect.top as f64);
            }
        }
        let cursor_x = cursor_pt.x as f64;
        let cursor_y = cursor_pt.y as f64;
        let probe = has_cursor.then(|| {
            probe_cursor(
                &hit_target,
                origin,
                (cursor_x, cursor_y),
                lbutton_down && !prev_lbutton_down,
            )
        });
        let (rel_x, rel_y, pet_clicked) = match probe {
            Some(probe) => (probe.rel_x, probe.rel_y, probe.head_clicked),
            None => {
                let (x, y) = idle_gaze(&hit_target);
                (x, y, false)
            }
        };

        if let Some(probe) = probe {
            if is_hovered && lbutton_down {
                if !is_dragging {
                    is_dragging = true;
                    drag_offset_x = cursor_x - origin.0;
                    drag_offset_y = cursor_y - origin.1;
                } else {
                    on_event(InputEvent::DragMove {
                        x: (cursor_x - drag_offset_x) / scale,
                        y: (cursor_y - drag_offset_y) / scale,
                    });
                }
            } else if is_dragging && !lbutton_down {
                is_dragging = false;
                on_event(InputEvent::DragEnd);
            }

            if !is_dragging && probe.hovering != is_hovered {
                is_hovered = probe.hovering;
                on_event(InputEvent::Hover(probe.hovering));
            }
        }

        // 3. Dispatch BongoInput event unconditionally on key/mouse change or cursor movement
        let cursor_moved = (rel_x - prev_cursor_x).abs() > 3.0 || (rel_y - prev_cursor_y).abs() > 3.0;
        if left_down != prev_left
            || right_down != prev_right
            || mouse_clicked != prev_mouse
            || cursor_moved
            || pet_clicked
        {
            prev_left = left_down;
            prev_right = right_down;
            prev_mouse = mouse_clicked;
            prev_cursor_x = rel_x;
            prev_cursor_y = rel_y;

            on_event(InputEvent::Bongo(BongoInputState {
                left_down,
                right_down,
                mouse_down: mouse_clicked,
                mouse_x: rel_x,
                mouse_y: rel_y,
                pet_clicked,
                active: is_active,
            }));
        }

        prev_lbutton_down = lbutton_down;
        thread::sleep(Duration::from_millis(16));
    }
}

#[cfg(not(windows))]
fn run_watcher_loop<F>(
    _target: Arc<Mutex<HitTestTarget>>,
    running: Arc<AtomicBool>,
    _on_event: F,
) where
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
            visible: true,
        }
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
        sized.rects = vec![InputRect { x: 10.0, y: 20.0, width: 40.0, height: 50.0 }];
        assert!(probe_cursor(&sized, (0.0, 0.0), (30.0, 40.0), false).hovering);
        assert!(!probe_cursor(&sized, (0.0, 0.0), (300.0, 400.0), false).hovering);
    }

    #[test]
    fn the_window_origin_is_respected_when_the_pet_is_not_at_zero() {
        let pet = target(320.0, 420.0, 2.0);
        // Physical (1320, 850) on a window whose physical origin is
        // (1000, 500) at 200% is logical (160, 175) -- the head centre.
        let probe = probe_cursor(&pet, (1000.0, 500.0), (1320.0, 850.0), true);
        assert_eq!((probe.rel_x, probe.rel_y), (160.0, 175.0));
        assert!(probe.hovering);
        assert!(probe.head_clicked, "(160, 175) is the head of a 320x420 pet");
    }

    #[test]
    fn an_unmeasured_surface_falls_back_to_the_protocol_default() {
        assert_eq!(target(0.0, 0.0, 1.0).effective_size(), (320.0, 420.0));
        assert_eq!(target(f64::NAN, 420.0, 1.0).effective_size(), (320.0, 420.0));
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
