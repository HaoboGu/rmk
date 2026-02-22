use crate::config::MouseKeyConfig;
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

/// Fixed speed overrides when MouseAccel keys are held.
const ACCEL0_MOVE_SPEED: u16 = 4;
const ACCEL1_MOVE_SPEED: u16 = 12;
const ACCEL2_MOVE_SPEED: u16 = 20;
const ACCEL0_WHEEL_SPEED: u16 = 1;
const ACCEL1_WHEEL_SPEED: u16 = 2;
const ACCEL2_WHEEL_SPEED: u16 = 4;

pub(crate) struct MouseState {
    pub report: MouseReport,
    pub accel: u8,
    pub repeat: u8,
    pub wheel_repeat: u8,
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
        }
    }

    /// Process a mouse key press. Returns what action the caller should take.
    pub fn process_press(&mut self, key: HidKeyCode, config: &MouseKeyConfig) -> MouseAction {
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
        self.apply_diagonal_compensation_in_place();
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
    pub fn process_release(&mut self, key: HidKeyCode) -> MouseAction {
        match key {
            HidKeyCode::MouseUp    => { if self.report.y < 0 { self.report.y = 0; } }
            HidKeyCode::MouseDown  => { if self.report.y > 0 { self.report.y = 0; } }
            HidKeyCode::MouseLeft  => { if self.report.x < 0 { self.report.x = 0; } }
            HidKeyCode::MouseRight => { if self.report.x > 0 { self.report.x = 0; } }
            HidKeyCode::MouseWheelUp   => { if self.report.wheel > 0 { self.report.wheel = 0; } }
            HidKeyCode::MouseWheelDown => { if self.report.wheel < 0 { self.report.wheel = 0; } }
            HidKeyCode::MouseWheelLeft => { if self.report.pan < 0 { self.report.pan = 0; } }
            HidKeyCode::MouseWheelRight => { if self.report.pan > 0 { self.report.pan = 0; } }
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
        self.apply_diagonal_compensation_in_place();
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

    /// Apply diagonal compensation in-place on both movement and wheel axes.
    fn apply_diagonal_compensation_in_place(&mut self) {
        if self.report.x != 0 && self.report.y != 0 {
            let (x, y) = Self::apply_diagonal_compensation(self.report.x, self.report.y);
            self.report.x = x;
            self.report.y = y;
        }
        if self.report.wheel != 0 && self.report.pan != 0 {
            let (w, p) = Self::apply_diagonal_compensation(self.report.wheel, self.report.pan);
            self.report.wheel = w;
            self.report.pan = p;
        }
    }

    fn calculate_unit(
        accel: u8,
        repeat: u8,
        accel_fast: u16,
        accel_mid: u16,
        accel_slow: u16,
        delta: u8,
        max_speed: u8,
        time_to_max: u8,
        max: u8,
    ) -> i8 {
        let unit: u16 = if accel & (1 << 2) != 0 {
            accel_fast
        } else if accel & (1 << 1) != 0 {
            accel_mid
        } else if accel & (1 << 0) != 0 {
            accel_slow
        } else if repeat == 0 {
            delta as u16
        } else if repeat >= time_to_max {
            (delta as u16).saturating_mul(max_speed as u16)
        } else {
            let repeat_count = repeat as u16;
            let ttm = time_to_max as u16;
            let min_unit = delta as u16;
            let max_unit = (delta as u16).saturating_mul(max_speed as u16);
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
            ACCEL2_MOVE_SPEED, ACCEL1_MOVE_SPEED, ACCEL0_MOVE_SPEED,
            config.move_delta, config.max_speed, config.time_to_max, config.move_max)
    }

    /// Calculate mouse wheel movement distance based on current repeat count and acceleration settings
    fn calculate_wheel_unit(&self, config: &MouseKeyConfig) -> i8 {
        Self::calculate_unit(self.accel, self.wheel_repeat,
            ACCEL2_WHEEL_SPEED, ACCEL1_WHEEL_SPEED, ACCEL0_WHEEL_SPEED,
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

    fn default_config() -> MouseKeyConfig {
        MouseKeyConfig::default()
    }

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
        // repeat=0, no accel → should return delta
        let result = MouseState::calculate_unit(0, 0, 20, 12, 4, 6, 3, 50, 20);
        assert_eq!(result, 6);
    }

    #[test]
    fn calculate_unit_at_max_speed() {
        // repeat >= time_to_max → delta * max_speed = 6 * 3 = 18
        let result = MouseState::calculate_unit(0, 50, 20, 12, 4, 6, 3, 50, 20);
        assert_eq!(result, 18);
    }

    #[test]
    fn calculate_unit_clamped_to_max() {
        // delta * max_speed = 6 * 3 = 18, but max = 10 → clamped to 10
        let result = MouseState::calculate_unit(0, 50, 20, 12, 4, 6, 3, 50, 10);
        assert_eq!(result, 10);
    }

    #[test]
    fn calculate_unit_accel_overrides() {
        // Accel0 (bit 0) → accel_slow
        assert_eq!(MouseState::calculate_unit(1, 0, 20, 12, 4, 6, 3, 50, 20), 4);
        // Accel1 (bit 1) → accel_mid
        assert_eq!(MouseState::calculate_unit(2, 0, 20, 12, 4, 6, 3, 50, 20), 12);
        // Accel2 (bit 2) → accel_fast
        assert_eq!(MouseState::calculate_unit(4, 0, 20, 12, 4, 6, 3, 50, 20), 20);
        // Higher accel wins (bit 2 set along with bit 0)
        assert_eq!(MouseState::calculate_unit(5, 0, 20, 12, 4, 6, 3, 50, 20), 20);
    }

    #[test]
    fn calculate_unit_never_zero() {
        // delta=0 would produce 0, but clamped to 1
        let result = MouseState::calculate_unit(0, 0, 20, 12, 4, 0, 1, 50, 20);
        assert_eq!(result, 1);
    }

    // -- diagonal compensation -------------------------------------------

    #[test]
    fn diagonal_compensation_reduces_magnitude() {
        let (x, y) = MouseState::apply_diagonal_compensation(10, 10);
        // 10 * 181/256 ≈ 7
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
        // Value of 1 would compensate to 0, but the guard ensures ±1
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
        let action = state.process_press(HidKeyCode::MouseRight, &config);
        assert!(state.report.x > 0);
        assert_eq!(action, MouseAction::SendAndRepeat(MouseKeyCategory::Movement));
    }

    #[test]
    fn release_right_clears_x() {
        let mut state = MouseState::new();
        let config = default_config();
        state.process_press(HidKeyCode::MouseRight, &config);
        let action = state.process_release(HidKeyCode::MouseRight);
        assert_eq!(state.report.x, 0);
        assert_eq!(action, MouseAction::SendReport);
    }

    #[test]
    fn button_press_and_release() {
        let mut state = MouseState::new();
        let config = default_config();
        state.process_press(HidKeyCode::MouseBtn1, &config);
        assert_eq!(state.report.buttons, 1);
        state.process_release(HidKeyCode::MouseBtn1);
        assert_eq!(state.report.buttons, 0);
    }

    #[test]
    fn accel_press_returns_none() {
        let mut state = MouseState::new();
        let config = default_config();
        let action = state.process_press(HidKeyCode::MouseAccel0, &config);
        assert_eq!(action, MouseAction::None);
        assert_eq!(state.accel, 1);
    }

    #[test]
    fn direction_change_resets_repeat() {
        let mut state = MouseState::new();
        let config = default_config();
        state.process_press(HidKeyCode::MouseDown, &config);
        state.repeat = 10;
        // Pressing Up while Down is active should reset repeat
        state.process_press(HidKeyCode::MouseUp, &config);
        assert_eq!(state.repeat, 0);
    }

    #[test]
    fn release_resets_repeat_when_stopped() {
        let mut state = MouseState::new();
        let config = default_config();
        state.process_press(HidKeyCode::MouseRight, &config);
        state.repeat = 5;
        state.process_release(HidKeyCode::MouseRight);
        assert_eq!(state.repeat, 0);
    }
}
