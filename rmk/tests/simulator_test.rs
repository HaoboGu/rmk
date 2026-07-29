//! Self-tests for the simulator harness in `tests/common/simulator.rs`.
//!
//! Everything else in `rmk/tests` — the TOML scenarios and the host
//! integration tests — is built on the guarantees asserted here: that a
//! timeline runs, and that the four end-of-run checks actually fire. They stay
//! hand-written, because asserting a harness through the codegen that consumes
//! it would be circular.

#![cfg(any(not(feature = "_no_usb"), feature = "_ble"))]

pub mod common;

use rmk::k;
use rmk::types::keycode::HidKeyCode;

use crate::common::simulator::SimKeyboard;

#[test]
fn simulator_runs_keyboard_sequence() {
    common::test_block_on(async {
        let mut keyboard = SimKeyboard::create([[[k!(A)]]]).await;

        keyboard
            .press(0, 0)
            .expect_keys([HidKeyCode::A])
            .delay(10)
            .release(0, 0)
            .expect_all_up()
            .run()
            .await;
    });
}

#[test]
fn simulator_drains_balanced_input_without_hid_output() {
    common::test_block_on(async {
        let mut keyboard = SimKeyboard::create([[[rmk::types::action::KeyAction::No]]]).await;

        keyboard.press(0, 0).release(0, 0).run().await;
    });
}

#[test]
#[should_panic(expected = "unexpected trailing HID report")]
fn simulator_rejects_unasserted_trailing_report() {
    common::test_block_on(async {
        let mut keyboard = SimKeyboard::create([[[k!(A)]]]).await;

        keyboard.press(0, 0).run().await;
    });
}

#[test]
#[should_panic(expected = "simulator ended with pressed inputs")]
fn simulator_rejects_unreleased_input() {
    common::test_block_on(async {
        let mut keyboard = SimKeyboard::create([[[k!(A)]]]).await;

        keyboard.press(0, 0).expect_keys([HidKeyCode::A]).run().await;
    });
}

#[test]
#[should_panic(expected = "released without a matching press")]
fn simulator_rejects_release_without_press() {
    common::test_block_on(async {
        let mut keyboard = SimKeyboard::create([[[k!(A)]]]).await;

        keyboard.release(0, 0).run().await;
    });
}

#[test]
#[should_panic(expected = "pressed twice without a release")]
fn simulator_rejects_duplicate_press() {
    common::test_block_on(async {
        let mut keyboard = SimKeyboard::create([[[k!(A)]]]).await;

        keyboard
            .press(0, 0)
            .expect_keys([HidKeyCode::A])
            .press(0, 0)
            .run()
            .await;
    });
}
