//! Macro tests live in tests/scenarios/macros.toml; these residual tests stay
//! in Rust because they need `MacroOperation` values the scenario schema can't
//! spell: `Delay` takes Vial's packed two-byte delay (the codegen passes
//! `duration` through as raw milliseconds, so `duration = "50ms"` would play
//! back as 12495ms), and `TapAction` wraps a full `Action`.

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

// A 16-bit Vial keycode (LCtrl(A) = 0x0104) used as a macro TAP action is serialized as
// VIAL_MACRO_EXT_TAP, decoded, and routed through the shared action path so the modifier
// is applied exactly like a physical key. This is the mechanism that makes BT/PDF (and any
// other 16-bit keycode) work inside a macro.
#[cfg(feature = "vial")]
#[test]
fn test_macro_extended_tap_key_with_modifier() {
    use rmk::types::modifier::ModifierCombination;

    let macro_sequences = &[Vec::from_slice(&[MacroOperation::TapAction(Action::KeyWithModifier(
        HidKeyCode::A,
        ModifierCombination::LCTRL,
    ))])
    .expect("too many elements")];

    let macro_data = define_macro_sequences(macro_sequences);

    crate::common::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP)
            .key(0, 0, 0, KeyAction::Single(Action::TriggerMacro(0)))
            .macro_sequences(macro_data)
            .build()
            .await;

        const KC_LCTRL: u8 = 1 << 0;
        keyboard
            .delay(0)
            .press(0, 0)
            .delay(100)
            .release(0, 0)
            .expect_keys_with_mods(KC_LCTRL, [HidKeyCode::A]) // press A with Left Ctrl
            .expect_all_up() // release
            .run()
            .await;
    });
}

// A macro cannot trigger another macro (which would re-enter the trigger queue and could loop
// forever). Macro 0 tries to trigger macro 1, then taps A: the nested trigger is dropped, so
// only A is emitted — B (macro 1) never runs — and the rest of macro 0 still executes.
#[cfg(feature = "vial")]
#[test]
fn test_macro_cannot_trigger_macro() {
    let macro_sequences = &[
        Vec::from_slice(&[
            MacroOperation::TapAction(Action::TriggerMacro(1)),
            MacroOperation::Tap(HidKeyCode::A),
        ])
        .expect("too many elements"),
        Vec::from_slice(&[MacroOperation::Tap(HidKeyCode::B)]).expect("too many elements"),
    ];

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
            .expect_keys([HidKeyCode::A]) // A only; macro 1 was not triggered
            .expect_all_up()
            .run()
            .await;
    });
}
