use core::sync::atomic::{AtomicBool, Ordering};

use embassy_sync::signal::Signal;
use rmk_types::action::{Action, KeyAction};
use rmk_types::keycode::{HidKeyCode, KeyCode};

/// Global flag indicating passkey entry mode is active.
/// Set when `PassKeyInput` event arrives, cleared on submit/cancel/timeout.
pub static PASSKEY_ENTRY_MODE: AtomicBool = AtomicBool::new(false);

/// Signal to carry the passkey result from the keyboard task back to the GATT task.
/// `Some(passkey)` = submit, `None` = cancel.
pub static PASSKEY_RESPONSE: Signal<crate::RawMutex, Option<u32>> = Signal::new();

/// Start a new passkey entry session.
///
/// IMPORTANT: reset the response signal before enabling passkey mode, so an
/// immediate keyboard response cannot be dropped by a late reset.
pub fn begin_passkey_entry_session() {
    PASSKEY_RESPONSE.reset();
    PASSKEY_ENTRY_MODE.store(true, Ordering::Release);
}

/// End the current passkey entry session.
pub fn end_passkey_entry_session() {
    PASSKEY_ENTRY_MODE.store(false, Ordering::Release);
}

/// State for passkey digit entry (up to 6 digits).
pub struct PasskeyEntryState {
    digits: [u8; 6],
    count: usize,
}

impl Default for PasskeyEntryState {
    fn default() -> Self {
        Self::new()
    }
}

impl PasskeyEntryState {
    pub const fn new() -> Self {
        Self {
            digits: [0; 6],
            count: 0,
        }
    }

    /// Reset the state for a new passkey entry session.
    pub fn reset(&mut self) {
        self.digits = [0; 6];
        self.count = 0;
    }

    /// Add a digit (0-9). Returns false if already at 6 digits.
    pub fn add_digit(&mut self, digit: u8) -> bool {
        if self.count < 6 {
            self.digits[self.count] = digit;
            self.count += 1;
            true
        } else {
            false
        }
    }

    /// Remove the last digit. Returns false if empty.
    pub fn remove_digit(&mut self) -> bool {
        if self.count > 0 {
            self.count -= 1;
            self.digits[self.count] = 0;
            true
        } else {
            false
        }
    }

    /// Whether we have all 6 digits.
    pub fn is_complete(&self) -> bool {
        self.count == 6
    }

    /// Convert the entered digits to a u32 passkey.
    pub fn to_passkey(&self) -> u32 {
        let mut result: u32 = 0;
        for i in 0..self.count {
            result = result * 10 + self.digits[i] as u32;
        }
        result
    }

    /// Number of digits entered so far.
    pub fn digit_count(&self) -> usize {
        self.count
    }
}

/// Extract a digit (0-9) from a `KeyAction`, if it represents a number key.
/// Handles `Single`, `Tap`, and `TapHold` (uses the tap action).
pub fn extract_digit(action: &KeyAction) -> Option<u8> {
    extract_digit_from_action(match action {
        KeyAction::Single(a) | KeyAction::Tap(a) => a,
        KeyAction::TapHold(tap, _, _) => tap,
        _ => return None,
    })
}

/// Check if the action is an Enter key (regular or keypad).
pub fn is_enter(action: &KeyAction) -> bool {
    is_hid_key(action, |k| matches!(k, HidKeyCode::Enter | HidKeyCode::KpEnter))
}

/// Check if the action is an Escape key.
pub fn is_escape(action: &KeyAction) -> bool {
    is_hid_key(action, |k| matches!(k, HidKeyCode::Escape))
}

/// Check if the action is a Backspace key.
pub fn is_backspace(action: &KeyAction) -> bool {
    is_hid_key(action, |k| matches!(k, HidKeyCode::Backspace))
}

fn extract_digit_from_action(action: &Action) -> Option<u8> {
    match action {
        Action::Key(KeyCode::Hid(k)) => hid_keycode_to_digit(*k),
        _ => None,
    }
}

pub fn hid_keycode_to_digit(k: HidKeyCode) -> Option<u8> {
    match k {
        HidKeyCode::Kc1 => Some(1),
        HidKeyCode::Kc2 => Some(2),
        HidKeyCode::Kc3 => Some(3),
        HidKeyCode::Kc4 => Some(4),
        HidKeyCode::Kc5 => Some(5),
        HidKeyCode::Kc6 => Some(6),
        HidKeyCode::Kc7 => Some(7),
        HidKeyCode::Kc8 => Some(8),
        HidKeyCode::Kc9 => Some(9),
        HidKeyCode::Kc0 => Some(0),
        HidKeyCode::Kp1 => Some(1),
        HidKeyCode::Kp2 => Some(2),
        HidKeyCode::Kp3 => Some(3),
        HidKeyCode::Kp4 => Some(4),
        HidKeyCode::Kp5 => Some(5),
        HidKeyCode::Kp6 => Some(6),
        HidKeyCode::Kp7 => Some(7),
        HidKeyCode::Kp8 => Some(8),
        HidKeyCode::Kp9 => Some(9),
        HidKeyCode::Kp0 => Some(0),
        _ => None,
    }
}

/// Extract a layer number from an action if it's a layer switch.
pub fn extract_layer_num(action: &Action) -> Option<u8> {
    match action {
        Action::LayerOn(n)
        | Action::LayerOff(n)
        | Action::LayerToggle(n)
        | Action::DefaultLayer(n)
        | Action::LayerToggleOnly(n) => Some(*n),
        _ => None,
    }
}

fn is_hid_key(action: &KeyAction, predicate: impl Fn(HidKeyCode) -> bool) -> bool {
    let inner = match action {
        KeyAction::Single(a) | KeyAction::Tap(a) => a,
        KeyAction::TapHold(tap, _, _) => tap,
        _ => return false,
    };
    matches!(inner, Action::Key(KeyCode::Hid(k)) if predicate(*k))
}

#[cfg(test)]
mod tests {
    use super::*;
    use embassy_futures::block_on;
    use rmk_types::action::MorseProfile;

    #[test]
    fn test_passkey_entry_state_basic() {
        let mut state = PasskeyEntryState::new();
        assert_eq!(state.digit_count(), 0);
        assert!(!state.is_complete());

        // Add digits 1-6
        for d in 1..=6 {
            assert!(state.add_digit(d));
        }
        assert!(state.is_complete());
        assert_eq!(state.to_passkey(), 123456);

        // Can't add 7th digit
        assert!(!state.add_digit(7));
        assert_eq!(state.to_passkey(), 123456);
    }

    #[test]
    fn test_passkey_entry_state_remove() {
        let mut state = PasskeyEntryState::new();
        assert!(!state.remove_digit()); // empty

        state.add_digit(1);
        state.add_digit(2);
        state.add_digit(3);
        assert_eq!(state.to_passkey(), 123);

        assert!(state.remove_digit());
        assert_eq!(state.to_passkey(), 12);
        assert_eq!(state.digit_count(), 2);
    }

    #[test]
    fn test_passkey_entry_state_reset() {
        let mut state = PasskeyEntryState::new();
        state.add_digit(9);
        state.add_digit(8);
        state.reset();
        assert_eq!(state.digit_count(), 0);
        assert_eq!(state.to_passkey(), 0);
    }

    #[test]
    fn test_passkey_with_zeros() {
        let mut state = PasskeyEntryState::new();
        // 007890
        state.add_digit(0);
        state.add_digit(0);
        state.add_digit(7);
        state.add_digit(8);
        state.add_digit(9);
        state.add_digit(0);
        assert_eq!(state.to_passkey(), 7890);
    }

    #[test]
    fn test_extract_digit_single() {
        let action = KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::Kc1)));
        assert_eq!(extract_digit(&action), Some(1));

        let action = KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::Kc0)));
        assert_eq!(extract_digit(&action), Some(0));
    }

    #[test]
    fn test_extract_digit_keypad() {
        let action = KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::Kp5)));
        assert_eq!(extract_digit(&action), Some(5));

        let action = KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::Kp0)));
        assert_eq!(extract_digit(&action), Some(0));
    }

    #[test]
    fn test_extract_digit_tap() {
        let action = KeyAction::Tap(Action::Key(KeyCode::Hid(HidKeyCode::Kc3)));
        assert_eq!(extract_digit(&action), Some(3));
    }

    #[test]
    fn test_extract_digit_taphold() {
        let action = KeyAction::TapHold(
            Action::Key(KeyCode::Hid(HidKeyCode::Kc7)),
            Action::LayerOn(1),
            MorseProfile::const_default(),
        );
        assert_eq!(extract_digit(&action), Some(7));
    }

    #[test]
    fn test_extract_digit_non_digit() {
        let action = KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::A)));
        assert_eq!(extract_digit(&action), None);

        assert_eq!(extract_digit(&KeyAction::No), None);
        assert_eq!(extract_digit(&KeyAction::Transparent), None);
    }

    #[test]
    fn test_is_enter() {
        let enter = KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::Enter)));
        assert!(is_enter(&enter));

        let kp_enter = KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::KpEnter)));
        assert!(is_enter(&kp_enter));

        let a = KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::A)));
        assert!(!is_enter(&a));

        let tap_enter = KeyAction::TapHold(
            Action::Key(KeyCode::Hid(HidKeyCode::Enter)),
            Action::LayerOn(1),
            MorseProfile::const_default(),
        );
        assert!(is_enter(&tap_enter));
    }

    #[test]
    fn test_is_escape() {
        let esc = KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::Escape)));
        assert!(is_escape(&esc));

        let a = KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::A)));
        assert!(!is_escape(&a));
    }

    #[test]
    fn test_is_backspace() {
        let bs = KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::Backspace)));
        assert!(is_backspace(&bs));

        let a = KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::A)));
        assert!(!is_backspace(&a));

        let tap_bs = KeyAction::Tap(Action::Key(KeyCode::Hid(HidKeyCode::Backspace)));
        assert!(is_backspace(&tap_bs));
    }

    #[test]
    fn test_all_number_row_digits() {
        let keycodes = [
            (HidKeyCode::Kc0, 0),
            (HidKeyCode::Kc1, 1),
            (HidKeyCode::Kc2, 2),
            (HidKeyCode::Kc3, 3),
            (HidKeyCode::Kc4, 4),
            (HidKeyCode::Kc5, 5),
            (HidKeyCode::Kc6, 6),
            (HidKeyCode::Kc7, 7),
            (HidKeyCode::Kc8, 8),
            (HidKeyCode::Kc9, 9),
        ];
        for (kc, expected) in keycodes {
            let action = KeyAction::Single(Action::Key(KeyCode::Hid(kc)));
            assert_eq!(extract_digit(&action), Some(expected), "Failed for {:?}", kc);
        }
    }

    #[test]
    fn test_all_keypad_digits() {
        let keycodes = [
            (HidKeyCode::Kp0, 0),
            (HidKeyCode::Kp1, 1),
            (HidKeyCode::Kp2, 2),
            (HidKeyCode::Kp3, 3),
            (HidKeyCode::Kp4, 4),
            (HidKeyCode::Kp5, 5),
            (HidKeyCode::Kp6, 6),
            (HidKeyCode::Kp7, 7),
            (HidKeyCode::Kp8, 8),
            (HidKeyCode::Kp9, 9),
        ];
        for (kc, expected) in keycodes {
            let action = KeyAction::Single(Action::Key(KeyCode::Hid(kc)));
            assert_eq!(extract_digit(&action), Some(expected), "Failed for {:?}", kc);
        }
    }

    #[test]
    fn test_passkey_session_begin_order_allows_immediate_response() {
        // Simulate stale data from a previous session.
        PASSKEY_RESPONSE.signal(Some(111111));

        begin_passkey_entry_session();
        assert!(PASSKEY_ENTRY_MODE.load(Ordering::Acquire));

        // Simulate immediate keyboard response right after session begin.
        PASSKEY_RESPONSE.signal(Some(222222));
        let got = block_on(async { PASSKEY_RESPONSE.wait().await });
        assert_eq!(got, Some(222222));

        end_passkey_entry_session();
        assert!(!PASSKEY_ENTRY_MODE.load(Ordering::Acquire));
    }
}
