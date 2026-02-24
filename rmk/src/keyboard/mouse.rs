//! Mouse key state machine and HID report generation.
//!
//! This module tracks movement, wheel, acceleration, and button states, then
//! derives `MouseReport` values from key press/release events.
//! Pressing a mouse direction/wheel key activates an automatic repeat mechanism:
//! it schedules per-category repeat ticks and updates movement speed from
//! `MouseKeyConfig` as repeat count increases.

use embassy_time::{Duration, Instant};
use rmk_types::keycode::HidKeyCode;
use usbd_hid::descriptor::MouseReport;

use crate::config::MouseKeyConfig;

/// Result of processing a mouse key event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MouseAction {
    /// Send report.
    SendReport,
    /// No report needed.
    None,
}

#[derive(Debug, Clone, Copy)]
enum MouseCategory {
    Movement,
    Wheel,
}

#[derive(Debug, Clone, Copy)]
enum MouseDirection {
    Up,
    Down,
    Left,
    Right,
}

/// Per-category (movement or wheel) direction press counts, repeat counter and deadline.
#[derive(Default)]
struct DirectionState {
    up: u8,
    down: u8,
    left: u8,
    right: u8,
    repeat: u8,
    deadline: Option<Instant>,
}

impl DirectionState {
    fn dir_pressed_count(&mut self, dir: MouseDirection) -> &mut u8 {
        match dir {
            MouseDirection::Up => &mut self.up,
            MouseDirection::Down => &mut self.down,
            MouseDirection::Left => &mut self.left,
            MouseDirection::Right => &mut self.right,
        }
    }

    fn press(&mut self, dir: MouseDirection) {
        let count = self.dir_pressed_count(dir);
        *count = count.saturating_add(1);
    }

    fn release(&mut self, dir: MouseDirection) {
        let count = self.dir_pressed_count(dir);
        *count = count.saturating_sub(1);
    }

    fn is_active(&self) -> bool {
        self.up > 0 || self.down > 0 || self.left > 0 || self.right > 0
    }

    /// Net horizontal axis: right(+) - left(-)
    fn net_x(&self) -> i8 {
        (self.right > 0) as i8 - (self.left > 0) as i8
    }

    /// Net vertical axis: down(+) - up(-)
    fn net_y(&self) -> i8 {
        (self.down > 0) as i8 - (self.up > 0) as i8
    }

    /// Increment repeat counter, clamped to the category's ticks_to_max.
    fn increment_repeat(&mut self, category: MouseCategory, config: &MouseKeyConfig) {
        let cap = match category {
            MouseCategory::Movement => config.ticks_to_max,
            MouseCategory::Wheel => config.wheel_ticks_to_max,
        };
        if self.repeat < cap {
            self.repeat += 1;
        }
    }
}

pub(crate) struct MouseState {
    pub report: MouseReport,
    pub accel: u8,
    movement: DirectionState,
    wheel: DirectionState,
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
            movement: DirectionState::default(),
            wheel: DirectionState::default(),
        }
    }

    /// Classify a HidKeyCode into a direction key (returning which category and direction),
    /// or None if it's not a direction key.
    fn classify(key: HidKeyCode) -> Option<(MouseCategory, MouseDirection)> {
        match key {
            HidKeyCode::MouseUp => Some((MouseCategory::Movement, MouseDirection::Up)),
            HidKeyCode::MouseDown => Some((MouseCategory::Movement, MouseDirection::Down)),
            HidKeyCode::MouseLeft => Some((MouseCategory::Movement, MouseDirection::Left)),
            HidKeyCode::MouseRight => Some((MouseCategory::Movement, MouseDirection::Right)),
            HidKeyCode::MouseWheelUp => Some((MouseCategory::Wheel, MouseDirection::Up)),
            HidKeyCode::MouseWheelDown => Some((MouseCategory::Wheel, MouseDirection::Down)),
            HidKeyCode::MouseWheelLeft => Some((MouseCategory::Wheel, MouseDirection::Left)),
            HidKeyCode::MouseWheelRight => Some((MouseCategory::Wheel, MouseDirection::Right)),
            _ => None,
        }
    }

    /// Process a mouse key press. Only mutates state; axes are derived via recalculate_report.
    /// Schedule repeat on first activation of a category(Movement/Wheel).
    pub fn process_press(&mut self, key: HidKeyCode, config: &MouseKeyConfig) -> MouseAction {
        if let Some((category, direction)) = Self::classify(key) {
            // Process movement/wheel key
            let state = match category {
                MouseCategory::Movement => &mut self.movement,
                MouseCategory::Wheel => &mut self.wheel,
            };
            let was_active = state.is_active();
            state.press(direction);

            // Schedule repeat on first activation of this category
            if !was_active {
                let delay = Self::get_repeat_delay(state, category, config);
                state.deadline = Some(Instant::now() + Duration::from_millis(delay as u64));
            }

            let old_report = self.report;
            self.recalculate_report(config);

            if Self::report_changed(&old_report, &self.report) {
                MouseAction::SendReport
            } else {
                MouseAction::None
            }
        } else if matches!(
            key,
            HidKeyCode::MouseAccel0 | HidKeyCode::MouseAccel1 | HidKeyCode::MouseAccel2
        ) {
            match key {
                HidKeyCode::MouseAccel0 => self.accel |= 1 << 0,
                HidKeyCode::MouseAccel1 => self.accel |= 1 << 1,
                HidKeyCode::MouseAccel2 => self.accel |= 1 << 2,
                _ => unreachable!(),
            }
            MouseAction::None
        } else {
            // Button keys
            if let Some(bit) = Self::button_index(key) {
                self.report.buttons |= 1 << bit;
            }
            MouseAction::SendReport
        }
    }

    /// Process a mouse key release. Only mutates state; axes are derived via recalculate_report.
    pub fn process_release(&mut self, key: HidKeyCode, config: &MouseKeyConfig) -> MouseAction {
        if let Some((category, direction)) = Self::classify(key) {
            let state = match category {
                MouseCategory::Movement => &mut self.movement,
                MouseCategory::Wheel => &mut self.wheel,
            };
            state.release(direction);

            // Reset repeat counter and cancel deadline when ALL keys in this category are released
            if !state.is_active() {
                state.repeat = 0;
                state.deadline = None;
            }

            let old_report = self.report;
            self.recalculate_report(config);

            if Self::report_changed(&old_report, &self.report) {
                MouseAction::SendReport
            } else {
                MouseAction::None
            }
        } else if matches!(
            key,
            HidKeyCode::MouseAccel0 | HidKeyCode::MouseAccel1 | HidKeyCode::MouseAccel2
        ) {
            match key {
                HidKeyCode::MouseAccel0 => self.accel &= !(1 << 0),
                HidKeyCode::MouseAccel1 => self.accel &= !(1 << 1),
                HidKeyCode::MouseAccel2 => self.accel &= !(1 << 2),
                _ => unreachable!(),
            }
            MouseAction::None
        } else {
            // Button keys
            if let Some(bit) = Self::button_index(key) {
                self.report.buttons &= !(1 << bit);
            }
            MouseAction::SendReport
        }
    }

    /// Recompute report axes from direction state + acceleration + accel multiplier.
    /// Buttons are NOT touched (managed directly by press/release).
    pub fn recalculate_report(&mut self, config: &MouseKeyConfig) {
        let net_x = self.movement.net_x();
        let net_y = self.movement.net_y();

        if net_x != 0 || net_y != 0 {
            let unit = self.calculate_move_unit(config);
            self.report.x = net_x.saturating_mul(unit);
            self.report.y = net_y.saturating_mul(unit);
        } else {
            self.report.x = 0;
            self.report.y = 0;
        }

        let net_wheel = self.wheel.net_y();
        let net_pan = self.wheel.net_x();

        if net_wheel != 0 || net_pan != 0 {
            let unit = self.calculate_wheel_unit(config);
            self.report.wheel = net_wheel.saturating_mul(unit);
            self.report.pan = net_pan.saturating_mul(unit);
        } else {
            self.report.wheel = 0;
            self.report.pan = 0;
        }
    }

    /// Handle a repeat tick for the given category.
    fn on_repeat_tick(state: &mut DirectionState, category: MouseCategory, config: &MouseKeyConfig) {
        state.increment_repeat(category, config);
        let delay = Self::get_repeat_delay(state, category, config);
        state.deadline = Some(Instant::now() + Duration::from_millis(delay as u64));
    }

    /// Check which categories have expired deadlines and fire them.
    /// Returns which categories were fired (for report masking).
    pub fn fire_due_repeats(&mut self, now: Instant, config: &MouseKeyConfig) -> (bool, bool) {
        let fire_movement = matches!(self.movement.deadline, Some(d) if d <= now) && self.movement.is_active();
        let fire_wheel = matches!(self.wheel.deadline, Some(d) if d <= now) && self.wheel.is_active();

        if fire_movement {
            self.movement.deadline = None;
            Self::on_repeat_tick(&mut self.movement, MouseCategory::Movement, config);
        }
        if fire_wheel {
            self.wheel.deadline = None;
            Self::on_repeat_tick(&mut self.wheel, MouseCategory::Wheel, config);
        }

        if fire_movement || fire_wheel {
            self.recalculate_report(config);
        }

        (fire_movement, fire_wheel)
    }

    /// Returns the earliest pending repeat deadline, if any.
    pub fn next_deadline(&self) -> Option<Instant> {
        match (self.movement.deadline, self.wheel.deadline) {
            (Some(m), Some(w)) => Some(if m <= w { m } else { w }),
            (Some(m), None) => Some(m),
            (None, Some(w)) => Some(w),
            (None, None) => None,
        }
    }

    /// Get the delay before the next auto-repeat.
    fn get_repeat_delay(state: &DirectionState, category: MouseCategory, config: &MouseKeyConfig) -> u16 {
        match category {
            MouseCategory::Movement => config.get_movement_delay(state.repeat),
            MouseCategory::Wheel => config.get_wheel_delay(state.repeat),
        }
    }

    /// Check if report axes/buttons changed.
    fn report_changed(old: &MouseReport, new: &MouseReport) -> bool {
        old.x != new.x || old.y != new.y || old.wheel != new.wheel || old.pan != new.pan || old.buttons != new.buttons
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
    /// repeated recalculations do not shrink the non-repeated axis over time.
    pub fn get_report(&self) -> MouseReport {
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

    /// Two-step speed calculation:
    /// Step 1: acceleration curve based on repeat count
    /// Step 2: accel multiplier (Accel0=0.25x, Accel1=0.5x, Accel2=2.0x, highest wins)
    fn calculate_unit(accel: u8, repeat: u8, delta: u8, max_speed: u8, ticks_to_max: u8, max: u8) -> i8 {
        // Step 1: Base value from acceleration curve
        let max_unit = (delta as u16).saturating_mul(max_speed as u16);
        let base: u16 = if repeat == 0 {
            delta as u16
        } else if repeat >= ticks_to_max {
            max_unit
        } else {
            let repeat_count = repeat as u16;
            let ttm = ticks_to_max as u16;
            let min_unit = delta as u16;
            let unit_range = max_unit - min_unit;
            let linear_term = 2u16.saturating_mul(repeat_count).saturating_mul(ttm);
            let quadratic_term = repeat_count.saturating_mul(repeat_count);
            let progress_num = linear_term.saturating_sub(quadratic_term);
            let progress_den = ttm.saturating_mul(ttm);
            min_unit + (unit_range.saturating_mul(progress_num) / progress_den.max(1))
        };

        // Step 2: Apply accel multiplier (highest active wins)
        let multiplied: u16 = if accel & (1 << 2) != 0 {
            // Accel2: 2.0x
            base.saturating_mul(2)
        } else if accel & (1 << 1) != 0 {
            // Accel1: 0.5x (round up to avoid zero)
            (base + 1) / 2
        } else if accel & (1 << 0) != 0 {
            // Accel0: 0.25x (round up to avoid zero)
            (base + 3) / 4
        } else {
            base
        };

        // Step 3: Clamp to [1, max] and i8 range
        let clamped = if multiplied > max as u16 {
            max as u16
        } else if multiplied == 0 {
            1
        } else {
            multiplied
        };
        clamped.min(i8::MAX as u16) as i8
    }

    /// Calculate mouse movement distance based on current repeat count and acceleration settings
    fn calculate_move_unit(&self, config: &MouseKeyConfig) -> i8 {
        Self::calculate_unit(
            self.accel,
            self.movement.repeat,
            config.move_delta,
            config.max_speed,
            config.ticks_to_max,
            config.move_max,
        )
    }

    /// Calculate mouse wheel movement distance based on current repeat count and acceleration settings
    fn calculate_wheel_unit(&self, config: &MouseKeyConfig) -> i8 {
        Self::calculate_unit(
            self.accel,
            self.wheel.repeat,
            config.wheel_delta,
            config.wheel_max_speed_multiplier,
            config.wheel_ticks_to_max,
            config.wheel_max,
        )
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

    fn default_config() -> MouseKeyConfig {
        MouseKeyConfig::default()
    }

    // -- A. Basic press/release -----------------------------------------------

    #[test]
    fn press_right_sets_positive_x() {
        let mut state = MouseState::new();
        let config = default_config();
        let action = state.process_press(HidKeyCode::MouseRight, &config);
        assert!(state.report.x > 0);
        assert_eq!(state.report.y, 0);
        assert_eq!(action, MouseAction::SendReport);
    }

    #[test]
    fn press_up_sets_negative_y() {
        let mut state = MouseState::new();
        let config = default_config();
        state.process_press(HidKeyCode::MouseUp, &config);
        assert!(state.report.y < 0);
        assert_eq!(state.report.x, 0);
    }

    #[test]
    fn release_clears_axis() {
        let mut state = MouseState::new();
        let config = default_config();
        state.process_press(HidKeyCode::MouseRight, &config);
        let action = state.process_release(HidKeyCode::MouseRight, &config);
        assert_eq!(state.report.x, 0);
        assert_eq!(action, MouseAction::SendReport);
    }

    #[test]
    fn button_press_and_release() {
        let mut state = MouseState::new();
        let config = default_config();
        state.process_press(HidKeyCode::MouseBtn1, &config);
        assert_eq!(state.report.buttons, 1);
        state.process_release(HidKeyCode::MouseBtn1, &config);
        assert_eq!(state.report.buttons, 0);
    }

    #[test]
    fn button_index_mapping() {
        assert_eq!(MouseState::button_index(HidKeyCode::MouseBtn1), Some(0));
        assert_eq!(MouseState::button_index(HidKeyCode::MouseBtn8), Some(7));
        assert_eq!(MouseState::button_index(HidKeyCode::MouseUp), None);
    }

    // -- B. Opposite direction cancellation (req 4.5) -------------------------

    #[test]
    fn opposite_x_cancels() {
        let mut state = MouseState::new();
        let config = default_config();
        state.process_press(HidKeyCode::MouseRight, &config);
        assert!(state.report.x > 0);
        state.process_press(HidKeyCode::MouseLeft, &config);
        assert_eq!(state.report.x, 0, "Left+Right should cancel to 0");
    }

    #[test]
    fn opposite_y_cancels() {
        let mut state = MouseState::new();
        let config = default_config();
        state.process_press(HidKeyCode::MouseDown, &config);
        state.process_press(HidKeyCode::MouseUp, &config);
        assert_eq!(state.report.y, 0, "Up+Down should cancel to 0");
    }

    #[test]
    fn opposite_release_restores() {
        let mut state = MouseState::new();
        let config = default_config();
        state.process_press(HidKeyCode::MouseRight, &config);
        state.process_press(HidKeyCode::MouseLeft, &config);
        assert_eq!(state.report.x, 0);
        state.process_release(HidKeyCode::MouseLeft, &config);
        assert!(state.report.x > 0, "Releasing Left should restore Right");
    }

    #[test]
    fn opposite_wheel_cancels() {
        let mut state = MouseState::new();
        let config = default_config();
        state.process_press(HidKeyCode::MouseWheelUp, &config);
        state.process_press(HidKeyCode::MouseWheelDown, &config);
        assert_eq!(state.report.wheel, 0);
    }

    // -- C. Acceleration continuity (req 4.1) ---------------------------------

    #[test]
    fn new_direction_preserves_repeat() {
        let mut state = MouseState::new();
        let config = default_config();
        state.process_press(HidKeyCode::MouseRight, &config);
        state.movement.repeat = 10;
        state.process_press(HidKeyCode::MouseDown, &config);
        assert_eq!(
            state.movement.repeat, 10,
            "Adding a new direction should not reset repeat"
        );
    }

    #[test]
    fn direction_change_preserves_repeat() {
        let mut state = MouseState::new();
        let config = default_config();
        state.process_press(HidKeyCode::MouseDown, &config);
        state.movement.repeat = 10;
        state.process_press(HidKeyCode::MouseUp, &config);
        assert_eq!(state.movement.repeat, 10, "Opposite direction should not reset repeat");
    }

    #[test]
    fn repeat_resets_only_when_all_released() {
        let mut state = MouseState::new();
        let config = default_config();
        state.process_press(HidKeyCode::MouseRight, &config);
        state.process_press(HidKeyCode::MouseDown, &config);
        state.movement.repeat = 10;
        state.process_release(HidKeyCode::MouseDown, &config);
        assert_eq!(state.movement.repeat, 10, "Repeat should stay while Right is held");
        state.process_release(HidKeyCode::MouseRight, &config);
        assert_eq!(state.movement.repeat, 0, "Repeat should reset when all released");
    }

    #[test]
    fn wheel_repeat_independent() {
        let mut state = MouseState::new();
        let config = default_config();
        state.process_press(HidKeyCode::MouseRight, &config);
        state.process_press(HidKeyCode::MouseWheelUp, &config);
        state.movement.repeat = 10;
        state.wheel.repeat = 5;
        state.process_release(HidKeyCode::MouseRight, &config);
        assert_eq!(state.movement.repeat, 0, "Movement repeat should reset");
        assert_eq!(state.wheel.repeat, 5, "Wheel repeat should be independent");
    }

    // -- D. Duplicate keys (req 4.4) ------------------------------------------

    #[test]
    fn duplicate_key_single_effect() {
        let mut state = MouseState::new();
        let config = default_config();
        state.process_press(HidKeyCode::MouseRight, &config);
        let x_single = state.report.x;
        let action = state.process_press(HidKeyCode::MouseRight, &config);
        assert_eq!(state.report.x, x_single, "Duplicate should not change magnitude");
        assert_eq!(
            action,
            MouseAction::None,
            "Duplicate same-direction press should not send report"
        );
    }

    #[test]
    fn duplicate_release_one_keeps_axis() {
        let mut state = MouseState::new();
        let config = default_config();
        state.process_press(HidKeyCode::MouseRight, &config);
        state.process_press(HidKeyCode::MouseRight, &config);
        state.process_release(HidKeyCode::MouseRight, &config);
        assert!(state.report.x > 0, "x should stay positive with one still held");
    }

    #[test]
    fn duplicate_release_both_clears() {
        let mut state = MouseState::new();
        let config = default_config();
        state.process_press(HidKeyCode::MouseRight, &config);
        state.process_press(HidKeyCode::MouseRight, &config);
        state.movement.repeat = 5;
        state.process_release(HidKeyCode::MouseRight, &config);
        state.process_release(HidKeyCode::MouseRight, &config);
        assert_eq!(state.report.x, 0);
        assert_eq!(state.movement.repeat, 0, "Repeat should reset when all released");
    }

    // -- E. Diagonal (req 4.6) ------------------------------------------------

    #[test]
    fn diagonal_both_axes_set() {
        let mut state = MouseState::new();
        let config = default_config();
        state.process_press(HidKeyCode::MouseRight, &config);
        state.process_press(HidKeyCode::MouseDown, &config);
        assert!(state.report.x > 0);
        assert!(state.report.y > 0);
    }

    #[test]
    fn diagonal_compensation_reduces() {
        let mut state = MouseState::new();
        let config = default_config();
        state.process_press(HidKeyCode::MouseRight, &config);
        state.process_press(HidKeyCode::MouseDown, &config);
        let comp = state.get_report();
        assert!(comp.x < state.report.x, "Compensation should reduce diagonal x");
        assert!(comp.y < state.report.y, "Compensation should reduce diagonal y");
    }

    #[test]
    fn diagonal_repeat_both_axes_accelerate() {
        let mut state = MouseState::new();
        let config = default_config();
        state.process_press(HidKeyCode::MouseRight, &config);
        state.process_press(HidKeyCode::MouseDown, &config);
        let initial_x = state.report.x;
        let initial_y = state.report.y;

        for _ in 0..10 {
            MouseState::on_repeat_tick(&mut state.movement, MouseCategory::Movement, &config);
            state.recalculate_report(&config);
        }

        assert!(state.report.x > initial_x, "x should accelerate");
        assert!(state.report.y > initial_y, "y should accelerate");
        assert_eq!(state.report.x, state.report.y);
    }

    #[test]
    fn single_axis_no_compensation() {
        let mut state = MouseState::new();
        let config = default_config();
        state.process_press(HidKeyCode::MouseRight, &config);
        let comp = state.get_report();
        assert_eq!(comp.x, state.report.x);
        assert_eq!(comp.y, 0);
    }

    // -- F. Accel multiplier (req 3) ------------------------------------------

    #[test]
    fn accel0_reduces_speed() {
        let result = MouseState::calculate_unit(1, 0, 6, 3, 50, 20);
        assert_eq!(result, 2); // (6+3)/4 = 2
    }

    #[test]
    fn accel1_halves_speed() {
        let result = MouseState::calculate_unit(2, 0, 6, 3, 50, 20);
        assert_eq!(result, 3); // (6+1)/2 = 3
    }

    #[test]
    fn accel2_doubles_speed() {
        let result = MouseState::calculate_unit(4, 0, 6, 3, 50, 20);
        assert_eq!(result, 12); // 6*2 = 12
    }

    #[test]
    fn accel_multiplies_accelerated_value() {
        // At repeat=25 (mid-curve), accel2 should roughly double the base
        // Use a high max to avoid clamping
        let base = MouseState::calculate_unit(0, 25, 6, 3, 50, 127);
        let with_accel2 = MouseState::calculate_unit(4, 25, 6, 3, 50, 127);
        assert!(with_accel2 > base);
        // Should be approximately 2x (integer rounding may cause ±1)
        assert_eq!(with_accel2, base * 2);
    }

    #[test]
    fn accel_respects_max_clamp() {
        let result = MouseState::calculate_unit(4, 50, 6, 3, 50, 20);
        assert_eq!(result, 20); // 18*2=36, clamped to 20
    }

    #[test]
    fn accel_highest_wins() {
        let accel2_only = MouseState::calculate_unit(4, 0, 6, 3, 50, 20);
        let accel0_and_2 = MouseState::calculate_unit(5, 0, 6, 3, 50, 20);
        assert_eq!(accel0_and_2, accel2_only);
    }

    #[test]
    fn accel_press_only_modifies_state() {
        let mut state = MouseState::new();
        let config = default_config();
        state.process_press(HidKeyCode::MouseRight, &config);
        let x_before = state.report.x;
        let action = state.process_press(HidKeyCode::MouseAccel2, &config);
        assert_eq!(action, MouseAction::None, "Accel should not send report");
        assert_eq!(
            state.report.x, x_before,
            "Report should not change until next repeat tick"
        );
        assert_eq!(state.accel, 1 << 2, "Accel state should be updated");
    }

    #[test]
    fn accel_without_direction_is_noop() {
        let mut state = MouseState::new();
        let config = default_config();
        let action = state.process_press(HidKeyCode::MouseAccel0, &config);
        assert_eq!(action, MouseAction::None);
        assert_eq!(state.report.x, 0);
    }

    #[test]
    fn accel_release_only_modifies_state() {
        let mut state = MouseState::new();
        let config = default_config();
        state.process_press(HidKeyCode::MouseRight, &config);
        state.process_press(HidKeyCode::MouseAccel2, &config);
        let x_before = state.report.x;
        let action = state.process_release(HidKeyCode::MouseAccel2, &config);
        assert_eq!(action, MouseAction::None, "Accel release should not send report");
        assert_eq!(
            state.report.x, x_before,
            "Report should not change until next repeat tick"
        );
        assert_eq!(state.accel, 0, "Accel state should be cleared");
    }

    // -- G. Edge cases --------------------------------------------------------

    #[test]
    fn no_directions_report_zero() {
        let state = MouseState::new();
        assert_eq!(state.report.x, 0);
        assert_eq!(state.report.y, 0);
        assert_eq!(state.report.wheel, 0);
        assert_eq!(state.report.pan, 0);
    }

    #[test]
    fn all_four_directions_cancel() {
        let mut state = MouseState::new();
        let config = default_config();
        state.process_press(HidKeyCode::MouseUp, &config);
        state.process_press(HidKeyCode::MouseDown, &config);
        state.process_press(HidKeyCode::MouseLeft, &config);
        state.process_press(HidKeyCode::MouseRight, &config);
        assert_eq!(state.report.x, 0);
        assert_eq!(state.report.y, 0);
    }

    #[test]
    fn three_directions_one_cancels() {
        let mut state = MouseState::new();
        let config = default_config();
        state.process_press(HidKeyCode::MouseUp, &config);
        state.process_press(HidKeyCode::MouseDown, &config);
        state.process_press(HidKeyCode::MouseRight, &config);
        assert_eq!(state.report.y, 0, "Up+Down should cancel");
        assert!(state.report.x > 0, "Right should still be active");
    }

    #[test]
    fn calculate_unit_never_zero() {
        let result = MouseState::calculate_unit(0, 0, 0, 1, 50, 20);
        assert_eq!(result, 1);
    }

    #[test]
    fn calculate_unit_i8_max_clamp() {
        // Very large values should clamp to i8::MAX (127)
        let result = MouseState::calculate_unit(4, 50, 100, 10, 50, 255);
        assert_eq!(result, 127);
    }

    // -- H. Return value semantics --------------------------------------------

    #[test]
    fn first_direction_schedules_repeat() {
        let mut state = MouseState::new();
        let config = default_config();
        let action = state.process_press(HidKeyCode::MouseRight, &config);
        assert_eq!(action, MouseAction::SendReport);
        assert!(state.movement.deadline.is_some());
    }

    #[test]
    fn second_direction_does_not_reschedule() {
        let mut state = MouseState::new();
        let config = default_config();
        state.process_press(HidKeyCode::MouseRight, &config);
        let deadline_after_first = state.movement.deadline;
        let action = state.process_press(HidKeyCode::MouseDown, &config);
        assert_eq!(action, MouseAction::SendReport);
        // Deadline unchanged — repeat was already running
        assert_eq!(state.movement.deadline, deadline_after_first);
    }

    #[test]
    fn wheel_first_schedules_repeat() {
        let mut state = MouseState::new();
        let config = default_config();
        let action = state.process_press(HidKeyCode::MouseWheelUp, &config);
        assert_eq!(action, MouseAction::SendReport);
        assert!(state.wheel.deadline.is_some());
    }

    #[test]
    fn button_returns_send_report() {
        let mut state = MouseState::new();
        let config = default_config();
        let action = state.process_press(HidKeyCode::MouseBtn1, &config);
        assert_eq!(action, MouseAction::SendReport);
    }

    #[test]
    fn release_direction_returns_send_report() {
        let mut state = MouseState::new();
        let config = default_config();
        state.process_press(HidKeyCode::MouseRight, &config);
        let action = state.process_release(HidKeyCode::MouseRight, &config);
        assert_eq!(action, MouseAction::SendReport);
    }

    #[test]
    fn release_accel_with_active_returns_none() {
        let mut state = MouseState::new();
        let config = default_config();
        state.process_press(HidKeyCode::MouseRight, &config);
        state.process_press(HidKeyCode::MouseAccel0, &config);
        let action = state.process_release(HidKeyCode::MouseAccel0, &config);
        assert_eq!(action, MouseAction::None, "Accel release should not send report");
    }

    #[test]
    fn release_accel_without_active_returns_none() {
        let mut state = MouseState::new();
        let config = default_config();
        state.process_press(HidKeyCode::MouseAccel0, &config);
        let action = state.process_release(HidKeyCode::MouseAccel0, &config);
        assert_eq!(action, MouseAction::None);
    }

    // -- I. on_repeat_tick ----------------------------------------------------

    #[test]
    fn on_repeat_tick_increments_and_recalculates() {
        let mut state = MouseState::new();
        let config = default_config();
        state.process_press(HidKeyCode::MouseRight, &config);
        assert_eq!(state.movement.repeat, 0);
        let x_initial = state.report.x;

        MouseState::on_repeat_tick(&mut state.movement, MouseCategory::Movement, &config);
        state.recalculate_report(&config);
        assert_eq!(state.movement.repeat, 1);
        assert!(state.report.x >= x_initial);
    }

    #[test]
    fn on_repeat_tick_schedules_next_deadline() {
        let mut state = MouseState::new();
        let config = default_config();
        state.process_press(HidKeyCode::MouseRight, &config);
        // Clear the deadline set by process_press so we can verify on_repeat_tick sets it
        state.movement.deadline = None;
        MouseState::on_repeat_tick(&mut state.movement, MouseCategory::Movement, &config);
        assert!(state.movement.deadline.is_some());
    }

    // -- J. Diagonal compensation static tests --------------------------------

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

    // -- K. calculate_unit curve tests ----------------------------------------

    #[test]
    fn calculate_unit_initial_returns_delta() {
        let result = MouseState::calculate_unit(0, 0, 6, 3, 50, 20);
        assert_eq!(result, 6);
    }

    #[test]
    fn calculate_unit_at_max_speed() {
        let result = MouseState::calculate_unit(0, 50, 6, 3, 50, 20);
        assert_eq!(result, 18);
    }

    #[test]
    fn calculate_unit_clamped_to_max() {
        let result = MouseState::calculate_unit(0, 50, 6, 3, 50, 10);
        assert_eq!(result, 10);
    }
}
