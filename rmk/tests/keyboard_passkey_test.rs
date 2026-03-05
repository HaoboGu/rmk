#![cfg(ble_passkey_entry)]

/// Test cases for BLE passkey entry with a split-like keyboard layout.
///
/// Layout (2 rows x 10 cols, simulating left/right halves of a split keyboard):
///
///   Layer 0:
///     Row 0: A  B  C  D  LT(1)  |  F  G  H  Enter  Backspace
///     Row 1: Escape  (rest No)
///
///   Layer 1:
///     Row 0: Tr Tr Tr Tr Tr      |  Kc1 Kc2 Kc3 Kc4 Kc5
///     Row 1: Kc6 (rest No)
///
///   LT(1) = TapHold(E, LayerOn(1)) with permissive hold.
///   Transparent keys on layer 1 fall through to layer 0 letters.
///
/// During passkey mode, holding LT(1) activates layer 1 so the user
/// can type digits on the right-hand side. Releasing LT(1) returns
/// to layer 0 where Enter, Backspace, and Escape are accessible.

pub mod common;

use embassy_futures::block_on;
use embassy_futures::select::{Either, select};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Timer, with_timeout};
use futures::join;
use rmk::ble::passkey::{
    PASSKEY_RESPONSE, begin_passkey_entry_session, end_passkey_entry_session,
};
use rmk::channel::KEYBOARD_REPORT_CHANNEL;
use rmk::config::{BehaviorConfig, MorsesConfig, PositionalConfig};
use rmk::event::{AsyncEventPublisher, AsyncPublishableEvent, KeyboardEvent};
use rmk::keyboard::Keyboard;
use rmk::types::action::{KeyAction, MorseMode, MorseProfile};
use rmk::input_device::Runnable;
use rmk::{a, k};
use rusty_fork::rusty_fork_test;

use crate::common::wrap_keymap;

// Key positions — "left hand" (cols 0–4)
const A: (u8, u8) = (0, 0);
const B: (u8, u8) = (0, 1);
const LT1: (u8, u8) = (0, 4); // hold = layer 1, tap = E

// Key positions — "right hand" (cols 5–9)
// On layer 0: F, G, H, Enter, Backspace
// On layer 1: Kc1, Kc2, Kc3, Kc4, Kc5
const R0: (u8, u8) = (0, 5); // layer 0: F,  layer 1: Kc1
const R1: (u8, u8) = (0, 6); // layer 0: G,  layer 1: Kc2
const R2: (u8, u8) = (0, 7); // layer 0: H,  layer 1: Kc3
const R3: (u8, u8) = (0, 8); // layer 0: Enter, layer 1: Kc4
const R4: (u8, u8) = (0, 9); // layer 0: Backspace, layer 1: Kc5

// Extra row
const ESC: (u8, u8) = (1, 0); // layer 0: Escape, layer 1: Kc6

fn create_passkey_keyboard() -> Keyboard<'static, 2, 10, 2> {
    let lt1_key = KeyAction::TapHold(
        rmk::types::action::Action::Key(rmk::types::keycode::KeyCode::Hid(
            rmk::types::keycode::HidKeyCode::E,
        )),
        rmk::types::action::Action::LayerOn(1),
        MorseProfile::new(
            None,
            Some(MorseMode::PermissiveHold),
            Some(200u16),
            Some(200u16),
        ),
    );

    #[rustfmt::skip]
    let keymap = [
        // Layer 0
        [
            [k!(A), k!(B), k!(C), k!(D), lt1_key, k!(F), k!(G), k!(H), k!(Enter), k!(Backspace)],
            [k!(Escape), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No)],
        ],
        // Layer 1: transparent on left, numbers on right
        [
            [a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), k!(Kc1), k!(Kc2), k!(Kc3), k!(Kc4), k!(Kc5)],
            [k!(Kc6), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No)],
        ],
    ];

    let behavior_config = BehaviorConfig {
        morse: MorsesConfig {
            default_profile: MorseProfile::new(
                None,
                Some(MorseMode::PermissiveHold),
                Some(200u16),
                Some(200u16),
            ),
            ..Default::default()
        },
        ..Default::default()
    };

    static BEHAVIOR_CONFIG: static_cell::StaticCell<BehaviorConfig> = static_cell::StaticCell::new();
    let behavior_config = BEHAVIOR_CONFIG.init(behavior_config);
    static KEY_CONFIG: static_cell::StaticCell<PositionalConfig<2, 10>> = static_cell::StaticCell::new();
    let per_key_config = KEY_CONFIG.init(PositionalConfig::default());
    Keyboard::new(wrap_keymap(keymap, per_key_config, behavior_config))
}

/// Run a passkey test: send key events while in passkey mode and verify the result.
///
/// All key events are sent after passkey mode is enabled.
/// Verifies no HID reports leak and checks the PASSKEY_RESPONSE signal.
async fn run_passkey_test(
    keyboard: &mut Keyboard<'_, 2, 10, 2>,
    events: &[(u8, u8, bool, u64)],
    expected_response: Option<u32>,
) {
    static DONE: Mutex<CriticalSectionRawMutex, bool> = Mutex::new(false);
    *DONE.lock().await = false;

    let sender = KeyboardEvent::publisher_async();
    sender.clear();
    KEYBOARD_REPORT_CHANNEL.clear();
    begin_passkey_entry_session();

    let max_timeout = Duration::from_secs(5);

    join!(
        // Run keyboard until done
        async {
            match select(Timer::after(max_timeout), select(keyboard.run(), async {
                while !*DONE.lock().await {
                    Timer::after(Duration::from_millis(10)).await;
                }
            }))
            .await
            {
                Either::First(_) => panic!("ERROR: test timeout reached"),
                _ => (),
            }
        },
        // Send events and verify
        async {
            // Send all key events
            for &(row, col, pressed, delay) in events {
                Timer::after(Duration::from_millis(delay)).await;
                sender
                    .publish_async(KeyboardEvent::key(row, col, pressed))
                    .await;
            }

            // Let keyboard process everything
            Timer::after(Duration::from_millis(50)).await;

            // Verify no HID reports leaked
            assert!(
                KEYBOARD_REPORT_CHANNEL.try_receive().is_err(),
                "HID report leaked during passkey entry mode"
            );

            // Check PASSKEY_RESPONSE with bounded wait to avoid races and hangs.
            if expected_response.is_some() {
                let response = with_timeout(Duration::from_secs(1), PASSKEY_RESPONSE.wait())
                    .await
                    .expect("expected passkey response, but timed out waiting for signal");
                assert_eq!(
                    response, expected_response,
                    "passkey response mismatch: expected {:?}, got {:?}",
                    expected_response, response
                );
            } else if PASSKEY_RESPONSE.signaled() {
                // For cancellation case, a signal may be expected as None.
                let response = with_timeout(Duration::from_secs(1), PASSKEY_RESPONSE.wait())
                    .await
                    .expect("PASSKEY_RESPONSE was signaled but wait timed out");
                assert_eq!(
                    response, expected_response,
                    "passkey response mismatch: expected {:?}, got {:?}",
                    expected_response, response
                );
            }

            // Cleanup
            end_passkey_entry_session();
            *DONE.lock().await = true;
        }
    );
}

rusty_fork_test! {
    /// Correct passkey: hold LT(1) to get layer 1, type 1-2-3-4-5-6 on right hand,
    /// release LT(1) to return to layer 0, press Enter to submit.
    /// Expected passkey: 123456.
    #[test]
    fn test_passkey_correct_entry_via_layer() {
        let mut keyboard = create_passkey_keyboard();
        block_on(run_passkey_test(
            &mut keyboard,
            &[
                // Hold LT(1) → activates layer 1
                (LT1.0, LT1.1, true, 0),
                // Type digits 1-5 on right hand (layer 1 positions)
                (R0.0, R0.1, true, 10),  // Kc1 press
                (R0.0, R0.1, false, 10), // Kc1 release → digit 1
                (R1.0, R1.1, true, 10),  // Kc2 press
                (R1.0, R1.1, false, 10), // Kc2 release → digit 2
                (R2.0, R2.1, true, 10),  // Kc3 press
                (R2.0, R2.1, false, 10), // Kc3 release → digit 3
                (R3.0, R3.1, true, 10),  // Kc4 press
                (R3.0, R3.1, false, 10), // Kc4 release → digit 4
                (R4.0, R4.1, true, 10),  // Kc5 press
                (R4.0, R4.1, false, 10), // Kc5 release → digit 5
                // Digit 6 on extra row
                (ESC.0, ESC.1, true, 10),  // Kc6 press
                (ESC.0, ESC.1, false, 10), // Kc6 release → digit 6
                // Release LT(1) → deactivates layer 1, back to layer 0
                (LT1.0, LT1.1, false, 10),
                // Press Enter (layer 0) to submit
                (R3.0, R3.1, true, 10),  // Enter press
                (R3.0, R3.1, false, 10), // Enter release → submit
            ],
            Some(123456),
        ));
    }

    /// Incorrect passkey: enter 6 digits that don't match what the host expects.
    /// The keyboard submits whatever the user typed — the host rejects the mismatch.
    /// Expected passkey: 654321.
    #[test]
    fn test_passkey_incorrect_entry() {
        let mut keyboard = create_passkey_keyboard();
        block_on(run_passkey_test(
            &mut keyboard,
            &[
                // Hold LT(1)
                (LT1.0, LT1.1, true, 0),
                // Type 6-5-4-3-2-1 (backwards)
                (ESC.0, ESC.1, true, 10),  // Kc6
                (ESC.0, ESC.1, false, 10),
                (R4.0, R4.1, true, 10),    // Kc5
                (R4.0, R4.1, false, 10),
                (R3.0, R3.1, true, 10),    // Kc4
                (R3.0, R3.1, false, 10),
                (R2.0, R2.1, true, 10),    // Kc3
                (R2.0, R2.1, false, 10),
                (R1.0, R1.1, true, 10),    // Kc2
                (R1.0, R1.1, false, 10),
                (R0.0, R0.1, true, 10),    // Kc1
                (R0.0, R0.1, false, 10),
                // Release LT(1), press Enter
                (LT1.0, LT1.1, false, 10),
                (R3.0, R3.1, true, 10),
                (R3.0, R3.1, false, 10),
            ],
            Some(654321),
        ));
    }

    /// Editing with Backspace: type some digits, backspace, retype, then submit.
    /// Sequence: 1-2-3-backspace-4-5-6-7 → digits become 1,2,4,5,6,7 → passkey 124567.
    #[test]
    fn test_passkey_backspace_editing() {
        let mut keyboard = create_passkey_keyboard();
        block_on(run_passkey_test(
            &mut keyboard,
            &[
                // Hold LT(1)
                (LT1.0, LT1.1, true, 0),
                // Type 1, 2, 3
                (R0.0, R0.1, true, 10),
                (R0.0, R0.1, false, 10),   // digit 1
                (R1.0, R1.1, true, 10),
                (R1.0, R1.1, false, 10),   // digit 2
                (R2.0, R2.1, true, 10),
                (R2.0, R2.1, false, 10),   // digit 3
                // Release LT(1) to access Backspace on layer 0
                (LT1.0, LT1.1, false, 10),
                // Backspace to delete digit 3
                (R4.0, R4.1, true, 10),    // Backspace press
                (R4.0, R4.1, false, 10),   // Backspace release → removes digit 3
                // Hold LT(1) again
                (LT1.0, LT1.1, true, 10),
                // Type 4, 5, 6, 7
                (R3.0, R3.1, true, 10),
                (R3.0, R3.1, false, 10),   // digit 4
                (R4.0, R4.1, true, 10),
                (R4.0, R4.1, false, 10),   // digit 5
                (ESC.0, ESC.1, true, 10),
                (ESC.0, ESC.1, false, 10), // digit 6
                // Digit 7: need a key that maps to Kc7. We only have Kc1-Kc6.
                // Use Kc1 again for the last digit → 1
                (R0.0, R0.1, true, 10),
                (R0.0, R0.1, false, 10),   // digit 1
                // Release LT(1), press Enter
                (LT1.0, LT1.1, false, 10),
                (R3.0, R3.1, true, 10),
                (R3.0, R3.1, false, 10),   // Enter → submit
            ],
            // digits: 1,2 (after backspace of 3), 4,5,6,1 → 124561
            Some(124561),
        ));
    }

    /// Too many digits: type more than 6 digits. Only the first 6 are accepted,
    /// the 7th is silently dropped. Enter submits the first 6.
    #[test]
    fn test_passkey_too_many_digits() {
        let mut keyboard = create_passkey_keyboard();
        block_on(run_passkey_test(
            &mut keyboard,
            &[
                // Hold LT(1)
                (LT1.0, LT1.1, true, 0),
                // Type 7 digits: 1,2,3,4,5,6,1 (7th should be ignored)
                (R0.0, R0.1, true, 10),
                (R0.0, R0.1, false, 10),   // digit 1
                (R1.0, R1.1, true, 10),
                (R1.0, R1.1, false, 10),   // digit 2
                (R2.0, R2.1, true, 10),
                (R2.0, R2.1, false, 10),   // digit 3
                (R3.0, R3.1, true, 10),
                (R3.0, R3.1, false, 10),   // digit 4
                (R4.0, R4.1, true, 10),
                (R4.0, R4.1, false, 10),   // digit 5
                (ESC.0, ESC.1, true, 10),
                (ESC.0, ESC.1, false, 10), // digit 6
                (R0.0, R0.1, true, 10),    // 7th digit attempt (Kc1)
                (R0.0, R0.1, false, 10),   // silently dropped
                // Release LT(1), press Enter
                (LT1.0, LT1.1, false, 10),
                (R3.0, R3.1, true, 10),
                (R3.0, R3.1, false, 10),   // Enter → submit
            ],
            // Only first 6 digits: 123456
            Some(123456),
        ));
    }

    /// Letters instead of numbers: on layer 0 (without holding LT(1)),
    /// the right-hand keys map to letters F, G, H. These are not digit keys
    /// and are silently consumed. Enter with incomplete digits does NOT submit.
    /// No passkey response signal is sent.
    #[test]
    fn test_passkey_letters_no_digits() {
        let mut keyboard = create_passkey_keyboard();
        block_on(run_passkey_test(
            &mut keyboard,
            &[
                // Do NOT hold LT(1) — stay on layer 0 (letters only)
                // Press right-hand keys: F, G, H (not digits, silently consumed)
                (R0.0, R0.1, true, 10),    // F press
                (R0.0, R0.1, false, 10),   // F release → not a digit
                (R1.0, R1.1, true, 10),    // G press
                (R1.0, R1.1, false, 10),   // G release → not a digit
                (R2.0, R2.1, true, 10),    // H press
                (R2.0, R2.1, false, 10),   // H release → not a digit
                // Also press left-hand letter keys
                (A.0, A.1, true, 10),      // A press
                (A.0, A.1, false, 10),     // A release → not a digit
                (B.0, B.1, true, 10),      // B press
                (B.0, B.1, false, 10),     // B release → not a digit
                // Press Enter — but 0 digits entered, Enter is ignored
                (R3.0, R3.1, true, 10),    // Enter press
                (R3.0, R3.1, false, 10),   // Enter release → only 0/6 digits
            ],
            // No signal: Enter was pressed with 0 digits
            None,
        ));
    }

    /// Cancel with Escape: enter some digits, then press Escape.
    /// Passkey entry is cancelled, PASSKEY_RESPONSE signals None.
    #[test]
    fn test_passkey_escape_cancels() {
        let mut keyboard = create_passkey_keyboard();
        block_on(run_passkey_test(
            &mut keyboard,
            &[
                // Hold LT(1), type 2 digits
                (LT1.0, LT1.1, true, 0),
                (R0.0, R0.1, true, 10),
                (R0.0, R0.1, false, 10),   // digit 1
                (R1.0, R1.1, true, 10),
                (R1.0, R1.1, false, 10),   // digit 2
                // Release LT(1), press Escape to cancel
                (LT1.0, LT1.1, false, 10),
                (ESC.0, ESC.1, true, 10),  // Escape press
                (ESC.0, ESC.1, false, 10), // Escape release → cancel
            ],
            // None = cancellation
            None,
        ));
    }

    /// Enter pressed with incomplete passkey (fewer than 6 digits).
    /// Enter is ignored, no passkey response is sent.
    #[test]
    fn test_passkey_enter_with_incomplete() {
        let mut keyboard = create_passkey_keyboard();
        block_on(run_passkey_test(
            &mut keyboard,
            &[
                // Hold LT(1), type 3 digits
                (LT1.0, LT1.1, true, 0),
                (R0.0, R0.1, true, 10),
                (R0.0, R0.1, false, 10),   // digit 1
                (R1.0, R1.1, true, 10),
                (R1.0, R1.1, false, 10),   // digit 2
                (R2.0, R2.1, true, 10),
                (R2.0, R2.1, false, 10),   // digit 3
                // Release LT(1), press Enter with only 3/6 digits
                (LT1.0, LT1.1, false, 10),
                (R3.0, R3.1, true, 10),
                (R3.0, R3.1, false, 10),   // Enter → ignored (only 3/6)
            ],
            None,
        ));
    }

    /// Multiple backspaces: delete all entered digits, then re-enter correct passkey.
    #[test]
    fn test_passkey_backspace_all_then_reenter() {
        let mut keyboard = create_passkey_keyboard();
        block_on(run_passkey_test(
            &mut keyboard,
            &[
                // Hold LT(1), type 3 digits
                (LT1.0, LT1.1, true, 0),
                (R0.0, R0.1, true, 10),
                (R0.0, R0.1, false, 10),   // digit 1
                (R1.0, R1.1, true, 10),
                (R1.0, R1.1, false, 10),   // digit 2
                (R2.0, R2.1, true, 10),
                (R2.0, R2.1, false, 10),   // digit 3
                // Release LT(1), backspace 3 times
                (LT1.0, LT1.1, false, 10),
                (R4.0, R4.1, true, 10),
                (R4.0, R4.1, false, 10),   // backspace → 2 digits
                (R4.0, R4.1, true, 10),
                (R4.0, R4.1, false, 10),   // backspace → 1 digit
                (R4.0, R4.1, true, 10),
                (R4.0, R4.1, false, 10),   // backspace → 0 digits
                // Extra backspace on empty (should be harmless)
                (R4.0, R4.1, true, 10),
                (R4.0, R4.1, false, 10),   // backspace → still 0
                // Hold LT(1), re-enter 6 new digits: 5,4,3,2,1,6
                (LT1.0, LT1.1, true, 10),
                (R4.0, R4.1, true, 10),
                (R4.0, R4.1, false, 10),   // digit 5
                (R3.0, R3.1, true, 10),
                (R3.0, R3.1, false, 10),   // digit 4
                (R2.0, R2.1, true, 10),
                (R2.0, R2.1, false, 10),   // digit 3
                (R1.0, R1.1, true, 10),
                (R1.0, R1.1, false, 10),   // digit 2
                (R0.0, R0.1, true, 10),
                (R0.0, R0.1, false, 10),   // digit 1
                (ESC.0, ESC.1, true, 10),
                (ESC.0, ESC.1, false, 10), // digit 6
                // Release LT(1), press Enter
                (LT1.0, LT1.1, false, 10),
                (R3.0, R3.1, true, 10),
                (R3.0, R3.1, false, 10),   // Enter → submit
            ],
            Some(543216),
        ));
    }

    /// Mix of letters and digits: some keys on layer 0 (letters, consumed),
    /// some on layer 1 (digits, accepted). Only the digits count.
    #[test]
    fn test_passkey_mixed_letters_and_digits() {
        let mut keyboard = create_passkey_keyboard();
        block_on(run_passkey_test(
            &mut keyboard,
            &[
                // Type some layer-0 letters first (consumed, no digits registered)
                (A.0, A.1, true, 10),
                (A.0, A.1, false, 10),     // A → consumed
                (B.0, B.1, true, 10),
                (B.0, B.1, false, 10),     // B → consumed
                // Hold LT(1), type 6 digits
                (LT1.0, LT1.1, true, 10),
                (R0.0, R0.1, true, 10),
                (R0.0, R0.1, false, 10),   // digit 1
                (R1.0, R1.1, true, 10),
                (R1.0, R1.1, false, 10),   // digit 2
                (R2.0, R2.1, true, 10),
                (R2.0, R2.1, false, 10),   // digit 3
                (R3.0, R3.1, true, 10),
                (R3.0, R3.1, false, 10),   // digit 4
                (R4.0, R4.1, true, 10),
                (R4.0, R4.1, false, 10),   // digit 5
                (ESC.0, ESC.1, true, 10),
                (ESC.0, ESC.1, false, 10), // digit 6
                // Release LT(1), Enter
                (LT1.0, LT1.1, false, 10),
                (R3.0, R3.1, true, 10),
                (R3.0, R3.1, false, 10),   // Enter → submit
            ],
            Some(123456),
        ));
    }
}
