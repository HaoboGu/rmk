// This test requires the passkey_entry feature (which implies _ble).
// When compiled without it, the entire module is empty.
#![cfg(feature = "passkey_entry")]

pub mod common;

use rmk::sim::SimKeyboard;

/// Helper: send key events while keyboard is running and verify the passkey result.
///
/// Verifies that:
///   - No keyboard reports are sent to the host while in passkey mode.
///   - The passkey response signal receives the expected value.
async fn run_passkey_test<'a>(
    keyboard: &mut SimKeyboard<'a>,
    key_sequence: &[common::TestKeyPress],
    expected_passkey: Option<u32>,
) {
    keyboard.begin_passkey_entry();
    for key in key_sequence {
        keyboard.delay(key.delay);
        if key.pressed {
            keyboard.press(key.row, key.col);
        } else {
            keyboard.release(key.row, key.col);
        }
    }
    keyboard
        .delay(100)
        .expect_passkey_response(expected_passkey)
        .expect_no_report(200)
        .end_passkey_entry()
        .run()
        .await;
}

/// Typing a full 6-digit passkey and pressing Enter submits the passkey
/// and no keyboard reports are sent to the host.
#[test]
fn test_passkey_entry_submits_passkey() {
    // The test keymap has digit keys in row 0:
    //   col 1 = Kc1, col 2 = Kc2, col 3 = Kc3, col 4 = Kc4, col 5 = Kc5, col 6 = Kc6
    // Enter is at row 2, col 13
    // Passkey processes on key release, so we need press+release for each key
    common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(crate::common::TEST_KEYMAP).build().await;

        run_passkey_test(
            &mut keyboard,
            &[
                // Type "123456"
                common::TestKeyPress {
                    row: 0,
                    col: 1,
                    pressed: true,
                    delay: 0,
                },
                common::TestKeyPress {
                    row: 0,
                    col: 1,
                    pressed: false,
                    delay: 50,
                },
                common::TestKeyPress {
                    row: 0,
                    col: 2,
                    pressed: true,
                    delay: 50,
                },
                common::TestKeyPress {
                    row: 0,
                    col: 2,
                    pressed: false,
                    delay: 50,
                },
                common::TestKeyPress {
                    row: 0,
                    col: 3,
                    pressed: true,
                    delay: 50,
                },
                common::TestKeyPress {
                    row: 0,
                    col: 3,
                    pressed: false,
                    delay: 50,
                },
                common::TestKeyPress {
                    row: 0,
                    col: 4,
                    pressed: true,
                    delay: 50,
                },
                common::TestKeyPress {
                    row: 0,
                    col: 4,
                    pressed: false,
                    delay: 50,
                },
                common::TestKeyPress {
                    row: 0,
                    col: 5,
                    pressed: true,
                    delay: 50,
                },
                common::TestKeyPress {
                    row: 0,
                    col: 5,
                    pressed: false,
                    delay: 50,
                },
                common::TestKeyPress {
                    row: 0,
                    col: 6,
                    pressed: true,
                    delay: 50,
                },
                common::TestKeyPress {
                    row: 0,
                    col: 6,
                    pressed: false,
                    delay: 50,
                },
                // Press Enter to submit
                common::TestKeyPress {
                    row: 2,
                    col: 13,
                    pressed: true,
                    delay: 50,
                },
                common::TestKeyPress {
                    row: 2,
                    col: 13,
                    pressed: false,
                    delay: 50,
                },
            ],
            Some(123456),
        )
        .await;
    });
}

/// Pressing Escape during passkey entry cancels and signals None.
#[test]
fn test_passkey_entry_cancel() {
    // Type a couple digits then cancel with Escape
    // Escape is at row 2, col 0
    common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(crate::common::TEST_KEYMAP).build().await;

        run_passkey_test(
            &mut keyboard,
            &[
                common::TestKeyPress {
                    row: 0,
                    col: 1,
                    pressed: true,
                    delay: 0,
                },
                common::TestKeyPress {
                    row: 0,
                    col: 1,
                    pressed: false,
                    delay: 50,
                },
                common::TestKeyPress {
                    row: 0,
                    col: 2,
                    pressed: true,
                    delay: 50,
                },
                common::TestKeyPress {
                    row: 0,
                    col: 2,
                    pressed: false,
                    delay: 50,
                },
                // Press Escape to cancel
                common::TestKeyPress {
                    row: 2,
                    col: 0,
                    pressed: true,
                    delay: 50,
                },
                common::TestKeyPress {
                    row: 2,
                    col: 0,
                    pressed: false,
                    delay: 50,
                },
            ],
            None,
        )
        .await;
    });
}
