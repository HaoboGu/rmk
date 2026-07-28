//! Macro tests live in tests/scenarios/macros.toml; this residual test stays in
//! Rust because `MacroOperation::Delay` takes Vial's packed two-byte delay,
//! while the scenario codegen passes `duration` through as raw milliseconds —
//! `duration = "50ms"` would play back as 12495ms.

pub mod common;

use heapless::Vec;
use rmk::keyboard_macros::{MacroOperation, define_macro_sequences};
use rmk::types::action::{Action, KeyAction};
use rmk_types::keycode::HidKeyCode;

use crate::common::TEST_KEYMAP;
use crate::common::sim::SimKeyboard;

#[test]
fn test_macro_with_delay() {
    let macro_sequences = &[Vec::from_slice(&[
        MacroOperation::Tap(HidKeyCode::A),
        MacroOperation::Delay(50 << 8), // 50 ms
        MacroOperation::Tap(HidKeyCode::B),
    ])
    .expect("too many elements")];

    let macro_data = define_macro_sequences(macro_sequences);

    crate::common::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .key(0, 0, 0, KeyAction::Single(Action::TriggerMacro(0)))
            .key(0, 0, 1, KeyAction::Single(Action::TriggerMacro(1)))
            .macro_sequences(macro_data)
            .build()
            .await;

        keyboard
            .delay(0)
            .press(0, 0)
            .delay(100)
            .release(0, 0)
            .expect_keys([HidKeyCode::A]) // press A
            .expect_all_up() // release A
            .expect_keys([HidKeyCode::B]) // press B
            .expect_all_up() // release B
            .run()
            .await;
    });
}
