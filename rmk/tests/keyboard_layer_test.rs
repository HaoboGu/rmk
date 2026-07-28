//! Layer tests live in tests/scenarios/layer.toml; this residual test stays in
//! Rust because it needs an out-of-range PDF target, which scenario config
//! validation (correctly) rejects.

pub mod common;

use rmk::types::action::{Action, KeyAction};
use rmk::types::keycode::{HidKeyCode, KeyCode};

use crate::common::sim::SimKeyboard;

#[test]
fn test_pdf_invalid_layer_is_ignored() {
    // Only 2 layers exist, so PDF(5) is out of range: it must be rejected (base
    // layer stays 0, no panic), unlike a valid PDF that would switch the base.
    let keymap = [
        [[
            KeyAction::Single(Action::PersistentDefaultLayer(5)),
            KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::A))),
        ]],
        [[
            KeyAction::Single(Action::No),
            KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::B))),
        ]],
    ];
    crate::common::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(keymap).build().await;

        keyboard
            .tap(0, 1, 10)
            .tap(0, 0, 10)
            .tap(0, 1, 10)
            .expect_keys([HidKeyCode::A])
            .expect_all_up()
            .expect_keys([HidKeyCode::A])
            .expect_all_up()
            .run()
            .await;
    });
}
