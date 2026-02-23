use crate::config::MouseKeyConfig;
use crate::event::KeyboardEventPos;
use embassy_time::Instant;
use heapless::Vec as HVec;
use rmk_types::keycode::HidKeyCode;
use usbd_hid::descriptor::MouseReport;

/// Result of processing a mouse key event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MouseAction {
    /// Send report and enter auto-repeat loop for this key category.
    SendAndRepeat(MouseKeyCategory),
    /// Send report only (button press/release).
    SendReport,
    /// No report needed (accel key).
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MouseKeyCategory {
    Movement,
    Wheel,
}

/// Pending mouse auto-repeat, checked by the main event loop.
pub(crate) struct MouseRepeatState {
    /// The HID key code to repeat
    pub key: HidKeyCode,
    /// Physical key position that owns this repeat
    pub pos: KeyboardEventPos,
    /// When to fire the next repeat
    pub deadline: Instant,
}


pub(crate) struct MouseState {
    pub report: MouseReport,
    pub accel: u8,
    pub repeat: u8,
    pub wheel_repeat: u8,
    /// Currently held direction keys with their physical positions.
    held_directions: HVec<(HidKeyCode, KeyboardEventPos), 12>,
}

impl Default for MouseState {
    fn default() -> Self {
        Self::new()
    }
}

impl MouseState {
    pub fn new() -> Self {
        MouseState {
            report: MouseReport {
                buttons: 0,
                x: 0,
                y: 0,
                wheel: 0,
                pan: 0,
            },
            accel: 0,
            repeat: 0,
            wheel_repeat: 0,
            held_directions: HVec::new(),
        }
    }

    /// Process a mouse key press. Returns what action the caller should take.
    pub fn process_press(&mut self, key: HidKeyCode, pos: KeyboardEventPos, config: &MouseKeyConfig) -> MouseAction {
        // Track direction keys by position (deduplicate for repeat ticks)
        if Self::is_direction_key(key) && !self.held_directions.iter().any(|e| e.0 == key && e.1 == pos) {
            if self.held_directions.push((key, pos)).is_err() {
                warn!("Mouse held_directions overflow, dropping key");
            }
        }
        match key {
            HidKeyCode::MouseUp => {
                if self.report.y > 0 { self.repeat = 0; }
                let unit = self.calculate_move_unit(config);
                self.report.y = -unit;
            }
            HidKeyCode::MouseDown => {
                if self.report.y < 0 { self.repeat = 0; }
                let unit = self.calculate_move_unit(config);
                self.report.y = unit;
            }
            HidKeyCode::MouseLeft => {
                if self.report.x > 0 { self.repeat = 0; }
                let unit = self.calculate_move_unit(config);
                self.report.x = -unit;
            }
            HidKeyCode::MouseRight => {
                if self.report.x < 0 { self.repeat = 0; }
                let unit = self.calculate_move_unit(config);
                self.report.x = unit;
            }
            HidKeyCode::MouseWheelUp => {
                if self.report.wheel < 0 { self.wheel_repeat = 0; }
                let unit = self.calculate_wheel_unit(config);
                self.report.wheel = unit;
            }
            HidKeyCode::MouseWheelDown => {
                if self.report.wheel > 0 { self.wheel_repeat = 0; }
                let unit = self.calculate_wheel_unit(config);
                self.report.wheel = -unit;
            }
            HidKeyCode::MouseWheelLeft => {
                if self.report.pan > 0 { self.wheel_repeat = 0; }
                let unit = self.calculate_wheel_unit(config);
                self.report.pan = -unit;
            }
            HidKeyCode::MouseWheelRight => {
                if self.report.pan < 0 { self.wheel_repeat = 0; }
                let unit = self.calculate_wheel_unit(config);
                self.report.pan = unit;
            }
            HidKeyCode::MouseAccel0 => { self.accel |= 1 << 0; }
            HidKeyCode::MouseAccel1 => { self.accel |= 1 << 1; }
            HidKeyCode::MouseAccel2 => { self.accel |= 1 << 2; }
            _ => {
                if let Some(bit) = Self::button_index(key) {
                    self.report.buttons |= 1 << bit;
                }
            }
        }
        if matches!(key, HidKeyCode::MouseAccel0 | HidKeyCode::MouseAccel1 | HidKeyCode::MouseAccel2) {
            MouseAction::None
        } else if matches!(key, HidKeyCode::MouseUp | HidKeyCode::MouseDown | HidKeyCode::MouseLeft | HidKeyCode::MouseRight) {
            MouseAction::SendAndRepeat(MouseKeyCategory::Movement)
        } else if matches!(key, HidKeyCode::MouseWheelUp | HidKeyCode::MouseWheelDown | HidKeyCode::MouseWheelLeft | HidKeyCode::MouseWheelRight) {
            MouseAction::SendAndRepeat(MouseKeyCategory::Wheel)
        } else {
            MouseAction::SendReport
        }
    }

    /// Process a mouse key release. Returns what action the caller should take.
    pub fn process_release(&mut self, key: HidKeyCode, pos: KeyboardEventPos, config: &MouseKeyConfig) -> MouseAction {
        // Remove this specific (key, pos) from held set
        if Self::is_direction_key(key) {
            if let Some(idx) = self.held_directions.iter().position(|e| e.0 == key && e.1 == pos) {
                self.held_directions.swap_remove(idx);
            }
        }
        match key {
            HidKeyCode::MouseUp => {
                if self.report.y < 0 {
                    if self.is_direction_held(HidKeyCode::MouseUp) {
                        // Same direction still held from another position — keep value
                    } else if self.is_direction_held(HidKeyCode::MouseDown) {
                        let unit = self.calculate_move_unit(config);
                        self.report.y = unit;
                    } else {
                        self.report.y = 0;
                    }
                }
            }
            HidKeyCode::MouseDown => {
                if self.report.y > 0 {
                    if self.is_direction_held(HidKeyCode::MouseDown) {
                        // Same direction still held
                    } else if self.is_direction_held(HidKeyCode::MouseUp) {
                        let unit = self.calculate_move_unit(config);
                        self.report.y = -unit;
                    } else {
                        self.report.y = 0;
                    }
                }
            }
            HidKeyCode::MouseLeft => {
                if self.report.x < 0 {
                    if self.is_direction_held(HidKeyCode::MouseLeft) {
                        // Same direction still held
                    } else if self.is_direction_held(HidKeyCode::MouseRight) {
                        let unit = self.calculate_move_unit(config);
                        self.report.x = unit;
                    } else {
                        self.report.x = 0;
                    }
                }
            }
            HidKeyCode::MouseRight => {
                if self.report.x > 0 {
                    if self.is_direction_held(HidKeyCode::MouseRight) {
                        // Same direction still held
                    } else if self.is_direction_held(HidKeyCode::MouseLeft) {
                        let unit = self.calculate_move_unit(config);
                        self.report.x = -unit;
                    } else {
                        self.report.x = 0;
                    }
                }
            }
            HidKeyCode::MouseWheelUp => {
                if self.report.wheel > 0 {
                    if self.is_direction_held(HidKeyCode::MouseWheelUp) {
                        // Same direction still held
                    } else if self.is_direction_held(HidKeyCode::MouseWheelDown) {
                        let unit = self.calculate_wheel_unit(config);
                        self.report.wheel = -unit;
                    } else {
                        self.report.wheel = 0;
                    }
                }
            }
            HidKeyCode::MouseWheelDown => {
                if self.report.wheel < 0 {
                    if self.is_direction_held(HidKeyCode::MouseWheelDown) {
                        // Same direction still held
                    } else if self.is_direction_held(HidKeyCode::MouseWheelUp) {
                        let unit = self.calculate_wheel_unit(config);
                        self.report.wheel = unit;
                    } else {
                        self.report.wheel = 0;
                    }
                }
            }
            HidKeyCode::MouseWheelLeft => {
                if self.report.pan < 0 {
                    if self.is_direction_held(HidKeyCode::MouseWheelLeft) {
                        // Same direction still held
                    } else if self.is_direction_held(HidKeyCode::MouseWheelRight) {
                        let unit = self.calculate_wheel_unit(config);
                        self.report.pan = unit;
                    } else {
                        self.report.pan = 0;
                    }
                }
            }
            HidKeyCode::MouseWheelRight => {
                if self.report.pan > 0 {
                    if self.is_direction_held(HidKeyCode::MouseWheelRight) {
                        // Same direction still held
                    } else if self.is_direction_held(HidKeyCode::MouseWheelLeft) {
                        let unit = self.calculate_wheel_unit(config);
                        self.report.pan = -unit;
                    } else {
                        self.report.pan = 0;
                    }
                }
            }
            HidKeyCode::MouseAccel0 => { self.accel &= !(1 << 0); }
            HidKeyCode::MouseAccel1 => { self.accel &= !(1 << 1); }
            HidKeyCode::MouseAccel2 => { self.accel &= !(1 << 2); }
            _ => {
                if let Some(bit) = Self::button_index(key) {
                    self.report.buttons &= !(1 << bit);
                }
            }
        }
        // Reset repeat counters when movement stops
        if self.report.x == 0 && self.report.y == 0 {
            self.repeat = 0;
        }
        if self.report.wheel == 0 && self.report.pan == 0 {
            self.wheel_repeat = 0;
        }
        match key {
            HidKeyCode::MouseAccel0 | HidKeyCode::MouseAccel1 | HidKeyCode::MouseAccel2 => MouseAction::None,
            _ => MouseAction::SendReport,
        }
    }

    /// Increment the repeat counter for the given key category.
    pub fn increment_repeat(&mut self, category: MouseKeyCategory) {
        match category {
            MouseKeyCategory::Movement => {
                if self.repeat < u8::MAX { self.repeat += 1; }
            }
            MouseKeyCategory::Wheel => {
                if self.wheel_repeat < u8::MAX { self.wheel_repeat += 1; }
            }
        }
    }

    /// Get the delay before the next auto-repeat.
    pub fn get_repeat_delay(&self, category: MouseKeyCategory, config: &MouseKeyConfig) -> u16 {
        match category {
            MouseKeyCategory::Movement => config.get_movement_delay(self.repeat),
            MouseKeyCategory::Wheel => config.get_wheel_delay(self.wheel_repeat),
        }
    }

    /// Find a still-held movement key and its physical position.
    pub fn active_movement_key(&self) -> Option<(HidKeyCode, KeyboardEventPos)> {
        self.held_directions.iter()
            .find(|(k, _)| matches!(k,
                HidKeyCode::MouseRight | HidKeyCode::MouseLeft |
                HidKeyCode::MouseUp | HidKeyCode::MouseDown))
            .copied()
    }

    /// Find a still-held wheel key and its physical position.
    pub fn active_wheel_key(&self) -> Option<(HidKeyCode, KeyboardEventPos)> {
        self.held_directions.iter()
            .find(|(k, _)| matches!(k,
                HidKeyCode::MouseWheelUp | HidKeyCode::MouseWheelDown |
                HidKeyCode::MouseWheelLeft | HidKeyCode::MouseWheelRight))
            .copied()
    }

    /// Check whether any instance of the given direction key is still held.
    fn is_direction_held(&self, key: HidKeyCode) -> bool {
        self.held_directions.iter().any(|(k, _)| *k == key)
    }

    /// Whether the key is a direction (movement or wheel) key.
    fn is_direction_key(key: HidKeyCode) -> bool {
        matches!(key,
            HidKeyCode::MouseUp | HidKeyCode::MouseDown |
            HidKeyCode::MouseLeft | HidKeyCode::MouseRight |
            HidKeyCode::MouseWheelUp | HidKeyCode::MouseWheelDown |
            HidKeyCode::MouseWheelLeft | HidKeyCode::MouseWheelRight)
    }

    fn button_index(key: HidKeyCode) -> Option<u8> {
        match key {
            HidKeyCode::MouseBtn1 => Some(0),
            HidKeyCode::MouseBtn2 => Some(1),
            HidKeyCode::MouseBtn3 => Some(2),
            HidKeyCode::MouseBtn4 => Some(3),
            HidKeyCode::MouseBtn5 => Some(4),
            HidKeyCode::MouseBtn6 => Some(5),
            HidKeyCode::MouseBtn7 => Some(6),
            HidKeyCode::MouseBtn8 => Some(7),
            _ => None,
        }
    }

    /// Return a copy of the current report with diagonal compensation applied.
    /// The internal `self.report` retains raw (uncompensated) axis values so that
    /// repeated `process_press` calls do not shrink the non-repeated axis over time.
    pub fn compensated_report(&self) -> MouseReport {
        let mut r = self.report;
        if r.x != 0 && r.y != 0 {
            let (x, y) = Self::apply_diagonal_compensation(r.x, r.y);
            r.x = x;
            r.y = y;
        }
        if r.wheel != 0 && r.pan != 0 {
            let (w, p) = Self::apply_diagonal_compensation(r.wheel, r.pan);
            r.wheel = w;
            r.pan = p;
        }
        r
    }

    fn calculate_unit(
        accel: u8,
        repeat: u8,
        delta: u8,
        max_speed: u8,
        time_to_max: u8,
        max: u8,
    ) -> i8 {
        let max_unit = (delta as u16).saturating_mul(max_speed as u16);
        let unit: u16 = if accel & (1 << 2) != 0 {
            max_unit
        } else if accel & (1 << 1) != 0 {
            (delta as u16 + max_unit) / 2
        } else if accel & (1 << 0) != 0 {
            delta as u16
        } else if repeat == 0 {
            delta as u16
        } else if repeat >= time_to_max {
            max_unit
        } else {
            let repeat_count = repeat as u16;
            let ttm = time_to_max as u16;
            let min_unit = delta as u16;
            let unit_range = max_unit - min_unit;
            let linear_term = 2u16.saturating_mul(repeat_count).saturating_mul(ttm);
            let quadratic_term = repeat_count.saturating_mul(repeat_count);
            let progress_num = linear_term.saturating_sub(quadratic_term);
            let progress_den = ttm.saturating_mul(ttm);
            min_unit + (unit_range.saturating_mul(progress_num) / progress_den.max(1))
        };

        let clamped = if unit > max as u16 { max as u16 } else if unit == 0 { 1 } else { unit };
        clamped.min(i8::MAX as u16) as i8
    }

    /// Calculate mouse movement distance based on current repeat count and acceleration settings
    fn calculate_move_unit(&self, config: &MouseKeyConfig) -> i8 {
        Self::calculate_unit(self.accel, self.repeat,
            config.move_delta, config.max_speed, config.time_to_max, config.move_max)
    }

    /// Calculate mouse wheel movement distance based on current repeat count and acceleration settings
    fn calculate_wheel_unit(&self, config: &MouseKeyConfig) -> i8 {
        Self::calculate_unit(self.accel, self.wheel_repeat,
            config.wheel_delta, config.wheel_max_speed_multiplier, config.wheel_time_to_max, config.wheel_max)
    }

    /// Apply diagonal movement compensation (approximation of 1/sqrt(2))
    fn apply_diagonal_compensation(mut x: i8, mut y: i8) -> (i8, i8) {
        if x != 0 && y != 0 {
            let x_compensated = (x as i16 * 181 + 128) / 256;
            let y_compensated = (y as i16 * 181 + 128) / 256;
            x = if x_compensated == 0 && x != 0 {
                if x > 0 { 1 } else { -1 }
            } else {
                x_compensated as i8
            };
            y = if y_compensated == 0 && y != 0 {
                if y > 0 { 1 } else { -1 }
            } else {
                y_compensated as i8
            };
        }
        (x, y)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::config::MouseKeyConfig;
    use crate::event::KeyPos;

    fn default_config() -> MouseKeyConfig {
        MouseKeyConfig::default()
    }

    /// Dummy position for tests. Use different (row, col) to simulate distinct physical keys.
    fn pos(row: u8, col: u8) -> KeyboardEventPos {
        KeyboardEventPos::Key(KeyPos { row, col })
    }

    const P0: KeyboardEventPos = KeyboardEventPos::Key(KeyPos { row: 0, col: 0 });

    // -- button_index ----------------------------------------------------

    #[test]
    fn button_index_mapping() {
        assert_eq!(MouseState::button_index(HidKeyCode::MouseBtn1), Some(0));
        assert_eq!(MouseState::button_index(HidKeyCode::MouseBtn8), Some(7));
        assert_eq!(MouseState::button_index(HidKeyCode::MouseUp), None);
    }

    // -- calculate_unit --------------------------------------------------

    #[test]
    fn calculate_unit_initial_returns_delta() {
        // accel=0, repeat=0, delta=6, max_speed=3, time_to_max=50, max=20
        let result = MouseState::calculate_unit(0, 0, 6, 3, 50, 20);
        assert_eq!(result, 6);
    }

    #[test]
    fn calculate_unit_at_max_speed() {
        // repeat >= time_to_max → delta * max_speed = 6 * 3 = 18
        let result = MouseState::calculate_unit(0, 50, 6, 3, 50, 20);
        assert_eq!(result, 18);
    }

    #[test]
    fn calculate_unit_clamped_to_max() {
        // delta * max_speed = 18, but max=10 → clamped to 10
        let result = MouseState::calculate_unit(0, 50, 6, 3, 50, 10);
        assert_eq!(result, 10);
    }

    #[test]
    fn calculate_unit_accel_overrides() {
        // accel0 (bit 0) → delta = 6
        assert_eq!(MouseState::calculate_unit(1, 0, 6, 3, 50, 20), 6);
        // accel1 (bit 1) → (delta + delta*max_speed) / 2 = (6 + 18) / 2 = 12
        assert_eq!(MouseState::calculate_unit(2, 0, 6, 3, 50, 20), 12);
        // accel2 (bit 2) → delta * max_speed = 18
        assert_eq!(MouseState::calculate_unit(4, 0, 6, 3, 50, 20), 18);
        // accel0+accel2 (bits 0+2) → highest wins → 18
        assert_eq!(MouseState::calculate_unit(5, 0, 6, 3, 50, 20), 18);
    }

    #[test]
    fn calculate_unit_never_zero() {
        // delta=0 → would be 0, but clamped to 1
        let result = MouseState::calculate_unit(0, 0, 0, 1, 50, 20);
        assert_eq!(result, 1);
    }

    // -- diagonal compensation -------------------------------------------

    #[test]
    fn diagonal_compensation_reduces_magnitude() {
        let (x, y) = MouseState::apply_diagonal_compensation(10, 10);
        assert!(x < 10 && x > 0);
        assert!(y < 10 && y > 0);
        assert_eq!(x, y);
    }

    #[test]
    fn diagonal_compensation_preserves_sign() {
        let (x, y) = MouseState::apply_diagonal_compensation(-10, 10);
        assert!(x < 0);
        assert!(y > 0);
    }

    #[test]
    fn diagonal_compensation_small_values_never_zero() {
        let (x, y) = MouseState::apply_diagonal_compensation(1, 1);
        assert_eq!(x, 1);
        assert_eq!(y, 1);
    }

    #[test]
    fn diagonal_compensation_single_axis_unchanged() {
        let (x, y) = MouseState::apply_diagonal_compensation(10, 0);
        assert_eq!(x, 10);
        assert_eq!(y, 0);
    }

    // -- press / release integration -------------------------------------

    #[test]
    fn press_right_sets_positive_x() {
        let mut state = MouseState::new();
        let config = default_config();
        let action = state.process_press(HidKeyCode::MouseRight, P0, &config);
        assert!(state.report.x > 0);
        assert_eq!(action, MouseAction::SendAndRepeat(MouseKeyCategory::Movement));
    }

    #[test]
    fn release_right_clears_x() {
        let mut state = MouseState::new();
        let config = default_config();
        state.process_press(HidKeyCode::MouseRight, P0, &config);
        let action = state.process_release(HidKeyCode::MouseRight, P0, &config);
        assert_eq!(state.report.x, 0);
        assert_eq!(action, MouseAction::SendReport);
    }

    #[test]
    fn button_press_and_release() {
        let mut state = MouseState::new();
        let config = default_config();
        state.process_press(HidKeyCode::MouseBtn1, P0, &config);
        assert_eq!(state.report.buttons, 1);
        state.process_release(HidKeyCode::MouseBtn1, P0, &config);
        assert_eq!(state.report.buttons, 0);
    }

    #[test]
    fn accel_press_returns_none() {
        let mut state = MouseState::new();
        let config = default_config();
        let action = state.process_press(HidKeyCode::MouseAccel0, P0, &config);
        assert_eq!(action, MouseAction::None);
        assert_eq!(state.accel, 1);
    }

    #[test]
    fn direction_change_resets_repeat() {
        let mut state = MouseState::new();
        let config = default_config();
        state.process_press(HidKeyCode::MouseDown, P0, &config);
        state.repeat = 10;
        state.process_press(HidKeyCode::MouseUp, pos(0, 1), &config);
        assert_eq!(state.repeat, 0);
    }

    #[test]
    fn release_resets_repeat_when_stopped() {
        let mut state = MouseState::new();
        let config = default_config();
        state.process_press(HidKeyCode::MouseRight, P0, &config);
        state.repeat = 5;
        state.process_release(HidKeyCode::MouseRight, P0, &config);
        assert_eq!(state.repeat, 0);
    }

    // -- held-key tracking (P2 bug fixes) --------------------------------

    #[test]
    fn opposite_direction_release_reverts_axis() {
        // Bug 2: hold Up, press Down, release Down → should revert to Up
        let mut state = MouseState::new();
        let config = default_config();
        let pos_a = pos(0, 0);
        let pos_b = pos(0, 1);
        state.process_press(HidKeyCode::MouseUp, pos_a, &config);
        assert!(state.report.y < 0);
        state.process_press(HidKeyCode::MouseDown, pos_b, &config);
        assert!(state.report.y > 0);
        state.process_release(HidKeyCode::MouseDown, pos_b, &config);
        // Should revert to negative (MouseUp still held)
        assert!(state.report.y < 0, "y should be negative (MouseUp held), got {}", state.report.y);
        assert!(state.active_movement_key().is_some());
    }

    #[test]
    fn duplicate_binding_release_keeps_axis() {
        // Bug 1: two physical keys mapped to MouseRight
        let mut state = MouseState::new();
        let config = default_config();
        let pos_a = pos(0, 0);
        let pos_b = pos(0, 1);
        state.process_press(HidKeyCode::MouseRight, pos_a, &config);
        state.process_press(HidKeyCode::MouseRight, pos_b, &config);
        assert!(state.report.x > 0);
        // Release one — axis should stay positive
        state.process_release(HidKeyCode::MouseRight, pos_a, &config);
        assert!(state.report.x > 0, "x should stay positive, got {}", state.report.x);
    }

    // -- diagonal repeat drift regression (P1 fix) -----------------------

    #[test]
    fn diagonal_repeat_does_not_drift() {
        let mut state = MouseState::new();
        let config = default_config();
        let pos_right = pos(0, 0);
        let pos_down = pos(0, 1);

        // Hold Right + Down (diagonal)
        state.process_press(HidKeyCode::MouseRight, pos_right, &config);
        state.process_press(HidKeyCode::MouseDown, pos_down, &config);
        let initial_x = state.report.x;
        let initial_y = state.report.y;
        assert!(initial_x > 0);
        assert!(initial_y > 0);

        // Simulate 10 repeat ticks of MouseRight (as fire_mouse_repeat does)
        for _ in 0..10 {
            state.process_press(HidKeyCode::MouseRight, pos_right, &config);
            // y must not drift — raw value should stay exactly the same
            assert_eq!(state.report.y, initial_y,
                "y drifted after Right repeat: expected {}, got {}", initial_y, state.report.y);
        }

        // Verify compensated_report still applies diagonal reduction
        let comp = state.compensated_report();
        assert!(comp.x < state.report.x, "compensation should reduce diagonal x");
        assert!(comp.y < state.report.y, "compensation should reduce diagonal y");
    }

    #[test]
    fn compensated_report_single_axis_unchanged() {
        let mut state = MouseState::new();
        let config = default_config();
        state.process_press(HidKeyCode::MouseRight, P0, &config);
        // Single axis: compensated report should equal raw report
        let comp = state.compensated_report();
        assert_eq!(comp.x, state.report.x);
        assert_eq!(comp.y, 0);
    }

    #[test]
    fn same_category_fallback_after_release() {
        let mut state = MouseState::new();
        let config = default_config();
        state.process_press(HidKeyCode::MouseRight, pos(0, 0), &config);
        state.process_press(HidKeyCode::MouseDown, pos(0, 1), &config);
        state.process_release(HidKeyCode::MouseDown, pos(0, 1), &config);
        assert_eq!(
            state.active_movement_key(),
            Some((HidKeyCode::MouseRight, pos(0, 0)))
        );
        assert_eq!(state.active_wheel_key(), None);
    }
}
