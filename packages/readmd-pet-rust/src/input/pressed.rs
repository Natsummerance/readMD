//! BongoCat's edge/pressed-set contract, adapted for ReadMD's renderer ABI.
//! Upstream: ayangweb/BongoCat e5922f3, Apache-2.0; see third_party/bongocat.
use super::BongoInputState;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

#[derive(Clone, Copy)]
struct HeldKey {
    virtual_key: i32,
    missing: u8,
}

pub(super) struct PressedInput {
    keys: BTreeMap<u16, HeldKey>,
    buttons: u8,
    missing_buttons: [u8; 5],
    snapshot: BongoInputState,
    last_activity: Instant,
}

impl PressedInput {
    pub(super) fn new() -> Self {
        Self {
            keys: BTreeMap::new(),
            buttons: 0,
            missing_buttons: [0; 5],
            snapshot: BongoInputState::default(),
            last_activity: Instant::now(),
        }
    }

    pub(super) fn key(&mut self, hid: u16, virtual_key: i32, down: bool) -> bool {
        if down {
            if self.keys.contains_key(&hid) {
                return false;
            }
            self.keys.insert(
                hid,
                HeldKey {
                    virtual_key,
                    missing: 0,
                },
            );
            self.snapshot.keyboard_taps = self.snapshot.keyboard_taps.wrapping_add(1);
            self.snapshot.last_key = Some(hid);
            if matches!(hid, 0x4f..=0x52) {
                self.snapshot.right_taps = self.snapshot.right_taps.wrapping_add(1);
            } else {
                self.snapshot.left_taps = self.snapshot.left_taps.wrapping_add(1);
            }
            self.last_activity = Instant::now();
        } else if self.keys.remove(&hid).is_none() {
            return false;
        }
        self.snapshot.sequence = self.snapshot.sequence.wrapping_add(1);
        true
    }

    pub(super) fn mouse(&mut self, button: usize, down: bool) -> bool {
        if button >= 5 {
            return false;
        }
        let bit = 1 << button;
        if (self.buttons & bit != 0) == down {
            return false;
        }
        if down {
            self.buttons |= bit;
            self.snapshot.mouse_taps = self.snapshot.mouse_taps.wrapping_add(1);
            self.last_activity = Instant::now();
        } else {
            self.buttons &= !bit;
        }
        self.missing_buttons[button] = 0;
        self.snapshot.sequence = self.snapshot.sequence.wrapping_add(1);
        true
    }

    /// Reconciliation only releases observed controls, never invents presses.
    /// Two missing samples avoid clearing an edge before user32 catches up.
    pub(super) fn reconcile(&mut self, is_down: impl Fn(i32) -> bool) -> bool {
        let mut releases = Vec::new();
        for (&hid, held) in &mut self.keys {
            held.missing = if is_down(held.virtual_key) {
                0
            } else {
                held.missing + 1
            };
            if held.missing >= 2 {
                releases.push(hid);
            }
        }
        let mut changed = false;
        for hid in releases {
            changed |= self.key(hid, 0, false);
        }
        for (button, vk) in [1, 2, 4, 5, 6].into_iter().enumerate() {
            if self.buttons & (1 << button) == 0 {
                continue;
            }
            self.missing_buttons[button] = if is_down(vk) {
                0
            } else {
                self.missing_buttons[button] + 1
            };
            if self.missing_buttons[button] >= 2 {
                changed |= self.mouse(button, false);
            }
        }
        changed
    }

    pub(super) fn reset(&mut self) {
        self.keys.clear();
        self.buttons = 0;
        self.missing_buttons = [0; 5];
        self.snapshot.sequence = self.snapshot.sequence.wrapping_add(1);
    }

    pub(super) fn snapshot(&self) -> BongoInputState {
        let mut snapshot = self.snapshot.clone();
        snapshot.pressed_keys = self.keys.keys().copied().collect();
        snapshot.keyboard_down = !self.keys.is_empty();
        snapshot.left_down = self.keys.keys().any(|hid| !matches!(hid, 0x4f..=0x52));
        snapshot.right_down = self.keys.keys().any(|hid| matches!(hid, 0x4f..=0x52));
        snapshot.mouse_buttons = self.buttons;
        snapshot.mouse_down = self.buttons != 0;
        snapshot.active = snapshot.keyboard_down
            || snapshot.mouse_down
            || self.last_activity.elapsed() < Duration::from_millis(500);
        snapshot
    }
}

/// The upstream 60 Hz cursor smoothing, made independent of frame rate.
pub fn smoothing_alpha(elapsed: Duration) -> f64 {
    1.0 - 0.75_f64.powf(elapsed.as_secs_f64() * 60.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chord_edges_are_independent_and_repeats_do_not_retrigger() {
        let mut input = PressedInput::new();
        assert!(input.key(0xe0, 0xa2, true));
        assert!(input.key(4, 65, true));
        assert!(!input.key(4, 65, true));
        assert_eq!(input.snapshot().keyboard_taps, 2);
        assert!(input.key(4, 65, false));
        assert!(input.snapshot().left_down);
        assert_eq!(input.snapshot().pressed_keys, vec![0xe0]);
    }
    #[test]
    fn quick_taps_survive_even_between_cursor_frames() {
        let mut input = PressedInput::new();
        for _ in 0..1000 {
            input.key(4, 65, true);
            input.key(4, 65, false);
        }
        assert_eq!(input.snapshot().keyboard_taps, 1000);
        assert_eq!(input.snapshot().sequence, 2000);
        assert!(!input.snapshot().keyboard_down);
    }
    #[test]
    fn mouse_release_does_not_release_the_keyboard_hand() {
        let mut input = PressedInput::new();
        input.key(0x4f, 39, true);
        input.mouse(0, true);
        input.mouse(1, true);
        input.mouse(0, false);
        assert!(input.snapshot().right_down);
        assert_eq!(input.snapshot().mouse_buttons, 2);
        input.mouse(1, false);
        assert!(input.snapshot().right_down);
        assert!(!input.snapshot().mouse_down);
    }
    #[test]
    fn missing_release_is_corrected_without_synthesizing_a_press() {
        let mut input = PressedInput::new();
        input.key(4, 65, true);
        input.mouse(0, true);
        assert!(!input.reconcile(|_| false));
        assert!(input.reconcile(|_| false));
        assert!(!input.snapshot().keyboard_down && !input.snapshot().mouse_down);
        input.reconcile(|_| true);
        assert!(input.snapshot().pressed_keys.is_empty());
    }
    #[test]
    fn reset_releases_every_control_and_smoothing_is_time_based() {
        let mut input = PressedInput::new();
        input.key(4, 65, true);
        input.mouse(4, true);
        input.reset();
        assert!(!input.snapshot().keyboard_down && !input.snapshot().mouse_down);
        let half = smoothing_alpha(Duration::from_secs_f64(1.0 / 120.0));
        let frame = smoothing_alpha(Duration::from_secs_f64(1.0 / 60.0));
        assert!((1.0 - (1.0 - half).powi(2) - frame).abs() < 1e-7);
    }
}
