use core::sync::atomic::{AtomicBool, Ordering};

use embassy_sync::signal::Signal;
use embassy_time::{Duration, Instant};
use rmk_types::keycode::HidKeyCode;

/// Signal from gatt_events_task -> keyboard: passkey entry requested
pub(crate) static PASSKEY_REQUESTED: Signal<crate::RawMutex, ()> = Signal::new();

/// Signal from keyboard -> gatt_events_task: passkey result
/// Some(u32) = passkey entered, None = cancelled/timeout
pub(crate) static PASSKEY_RESPONSE: Signal<crate::RawMutex, Option<u32>> = Signal::new();

/// Signal from gatt_events_task -> keyboard: cancel active passkey entry (e.g. on disconnect)
pub(crate) static PASSKEY_CANCEL: Signal<crate::RawMutex, ()> = Signal::new();

/// Flag indicating the GATT task is actively waiting for a passkey response.
/// The keyboard checks this before entering passkey mode (prevents stale requests)
/// and before sending a timeout response (prevents responding to nobody).
pub(crate) static PASSKEY_PENDING: AtomicBool = AtomicBool::new(false);

/// Drop guard that clears `PASSKEY_PENDING` and signals `PASSKEY_CANCEL` when
/// the GATT task is dropped (e.g. profile switch mid-passkey).
pub(crate) struct PasskeyPendingGuard;

impl Drop for PasskeyPendingGuard {
    fn drop(&mut self) {
        if PASSKEY_PENDING.swap(false, Ordering::Release) {
            // GATT task was dropped while passkey was pending (e.g. profile switch).
            // Signal cancel so the keyboard exits passkey mode.
            PASSKEY_CANCEL.signal(());
        }
    }
}

const PASSKEY_DIGITS: usize = 6;

pub(crate) struct PasskeyState {
    digits: [u8; PASSKEY_DIGITS],
    count: u8,
    active: bool,
    deadline: Instant,
}

impl PasskeyState {
    pub fn new() -> Self {
        Self {
            digits: [0; PASSKEY_DIGITS],
            count: 0,
            active: false,
            deadline: Instant::now(),
        }
    }

    pub fn activate(&mut self, timeout_secs: u32) {
        self.digits = [0; PASSKEY_DIGITS];
        self.count = 0;
        self.active = true;
        self.deadline = Instant::now() + Duration::from_secs(timeout_secs as u64);
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn deadline(&self) -> Instant {
        self.deadline
    }

    /// Reset the deadline to `timeout_secs` from now.
    pub fn reset_deadline(&mut self, timeout_secs: u32) {
        self.deadline = Instant::now() + Duration::from_secs(timeout_secs as u64);
    }

    /// Push digit. Returns false if buffer full (rejects overflow).
    pub fn push_digit(&mut self, digit: u8) -> bool {
        if (self.count as usize) < PASSKEY_DIGITS {
            self.digits[self.count as usize] = digit;
            self.count += 1;
            true
        } else {
            false
        }
    }

    /// Delete last digit. Returns false if buffer empty.
    pub fn pop_digit(&mut self) -> bool {
        if self.count > 0 {
            self.count -= 1;
            true
        } else {
            false
        }
    }

    /// Submit passkey. Returns Some(u32) only if exactly 6 digits entered.
    pub fn submit(&mut self) -> Option<u32> {
        if self.count as usize != PASSKEY_DIGITS {
            return None;
        }
        let mut passkey: u32 = 0;
        for i in 0..PASSKEY_DIGITS {
            passkey = passkey * 10 + self.digits[i] as u32;
        }
        self.deactivate();
        Some(passkey)
    }

    pub fn cancel(&mut self) {
        self.deactivate();
    }

    fn deactivate(&mut self) {
        self.active = false;
        self.count = 0;
    }
}

/// Convert a HID keycode to a digit (0-9), if applicable.
pub(crate) fn keycode_to_digit(keycode: HidKeyCode) -> Option<u8> {
    match keycode {
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

pub(crate) fn is_enter(k: HidKeyCode) -> bool {
    matches!(k, HidKeyCode::Enter | HidKeyCode::KpEnter)
}

pub(crate) fn is_escape(k: HidKeyCode) -> bool {
    matches!(k, HidKeyCode::Escape)
}

pub(crate) fn is_backspace(k: HidKeyCode) -> bool {
    matches!(k, HidKeyCode::Backspace)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keycode_to_digit() {
        assert_eq!(keycode_to_digit(HidKeyCode::Kc0), Some(0));
        assert_eq!(keycode_to_digit(HidKeyCode::Kc1), Some(1));
        assert_eq!(keycode_to_digit(HidKeyCode::Kc9), Some(9));
        assert_eq!(keycode_to_digit(HidKeyCode::Kp0), Some(0));
        assert_eq!(keycode_to_digit(HidKeyCode::Kp5), Some(5));
        assert_eq!(keycode_to_digit(HidKeyCode::A), None);
        assert_eq!(keycode_to_digit(HidKeyCode::Enter), None);
    }

    #[test]
    fn test_passkey_state_push_and_submit() {
        let mut state = PasskeyState::new();
        state.activate(120);
        assert!(state.is_active());

        // Push 6 digits: 1, 2, 3, 4, 5, 6
        for d in 1..=6 {
            assert!(state.push_digit(d));
        }

        // 7th digit should be rejected
        assert!(!state.push_digit(7));

        // Submit should succeed with 123456
        assert_eq!(state.submit(), Some(123456));
        assert!(!state.is_active());
    }

    #[test]
    fn test_passkey_state_submit_incomplete() {
        let mut state = PasskeyState::new();
        state.activate(120);

        // Push only 3 digits
        state.push_digit(1);
        state.push_digit(2);
        state.push_digit(3);

        // Submit should fail with fewer than 6 digits
        assert_eq!(state.submit(), None);
        // State should still be active after failed submit
        assert!(state.is_active());
    }

    #[test]
    fn test_passkey_state_pop_digit() {
        let mut state = PasskeyState::new();
        state.activate(120);

        state.push_digit(1);
        state.push_digit(2);
        assert!(state.pop_digit());

        // Push more to fill 6 digits: now have [1], push 3,4,5,6,7
        for d in 3..=7 {
            state.push_digit(d);
        }

        // Should be [1, 3, 4, 5, 6, 7]
        assert_eq!(state.submit(), Some(134567));
    }

    #[test]
    fn test_passkey_state_pop_empty() {
        let mut state = PasskeyState::new();
        state.activate(120);
        assert!(!state.pop_digit());
    }

    #[test]
    fn test_passkey_state_cancel() {
        let mut state = PasskeyState::new();
        state.activate(120);
        state.push_digit(1);
        state.cancel();
        assert!(!state.is_active());
    }

    #[test]
    fn test_passkey_leading_zeros() {
        let mut state = PasskeyState::new();
        state.activate(120);

        // Enter 000000
        for _ in 0..6 {
            state.push_digit(0);
        }
        assert_eq!(state.submit(), Some(0));
    }

    #[test]
    fn test_passkey_with_leading_zeros() {
        let mut state = PasskeyState::new();
        state.activate(120);

        // Enter 001234
        state.push_digit(0);
        state.push_digit(0);
        state.push_digit(1);
        state.push_digit(2);
        state.push_digit(3);
        state.push_digit(4);
        assert_eq!(state.submit(), Some(1234));
    }

    #[test]
    fn test_is_enter() {
        assert!(is_enter(HidKeyCode::Enter));
        assert!(is_enter(HidKeyCode::KpEnter));
        assert!(!is_enter(HidKeyCode::A));
    }

    #[test]
    fn test_is_escape() {
        assert!(is_escape(HidKeyCode::Escape));
        assert!(!is_escape(HidKeyCode::A));
    }

    #[test]
    fn test_is_backspace() {
        assert!(is_backspace(HidKeyCode::Backspace));
        assert!(!is_backspace(HidKeyCode::A));
    }
}
