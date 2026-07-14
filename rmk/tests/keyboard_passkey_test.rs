// This test requires the passkey_entry feature (which implies _ble).
// When compiled without it, the entire module is empty.
#![cfg(feature = "passkey_entry")]

pub mod common;

use rmk::sim::SimKeyboard;

/// Typing a full 6-digit passkey and pressing Enter submits the passkey
/// and no keyboard reports are sent to the host.
#[test]
fn test_passkey_entry_submits_passkey() {
    // The test keymap has digit keys in row 0:
    //   col 1 = Kc1, col 2 = Kc2, col 3 = Kc3, col 4 = Kc4, col 5 = Kc5, col 6 = Kc6
    // Enter is at row 2, col 13
    // Passkey processes on key release, so we need press+release for each key
    common::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(crate::common::TEST_KEYMAP).build().await;

        keyboard
            .begin_passkey_entry()
            .tap(0, 1, 50)
            .delay(50)
            .tap(0, 2, 50)
            .delay(50)
            .tap(0, 3, 50)
            .delay(50)
            .tap(0, 4, 50)
            .delay(50)
            .tap(0, 5, 50)
            .delay(50)
            .tap(0, 6, 50)
            .delay(50)
            .tap(2, 13, 50)
            .delay(100)
            .expect_passkey_response(Some(123456))
            .expect_no_report(200)
            .end_passkey_entry()
            .run()
            .await;
    });
}

/// Pressing Escape during passkey entry cancels and signals None.
#[test]
fn test_passkey_entry_cancel() {
    // Type a couple digits then cancel with Escape
    // Escape is at row 2, col 0
    common::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(crate::common::TEST_KEYMAP).build().await;

        keyboard
            .begin_passkey_entry()
            .tap(0, 1, 50)
            .delay(50)
            .tap(0, 2, 50)
            .delay(50)
            .tap(2, 0, 50)
            .delay(100)
            .expect_passkey_response(None)
            .expect_no_report(200)
            .end_passkey_entry()
            .run()
            .await;
    });
}
