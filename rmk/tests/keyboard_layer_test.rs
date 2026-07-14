pub mod common;

use rmk::sim::{KeymapOverride, SimKeyboard, SimKeyboardSetup};
use rmk::types::action::{Action, KeyAction};
use rmk::types::keycode::{HidKeyCode, KeyCode};
use rmk_types::modifier::ModifierCombination;

use crate::common::{KC_LSHIFT, TEST_KEYMAP};

const LAYER_KEY_OVERRIDES: [KeymapOverride; 4] = [
    KeymapOverride::new(0, 0, 0, KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::A)))),
    KeymapOverride::new(
        0,
        0,
        1,
        KeyAction::Single(Action::LayerOnWithModifier(1, ModifierCombination::LSHIFT)),
    ),
    KeymapOverride::new(1, 0, 0, KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::B)))),
    KeymapOverride::new(1, 0, 1, KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::C)))),
];
const LAYER_SETUP: SimKeyboardSetup = SimKeyboardSetup::new().keys(&LAYER_KEY_OVERRIDES);

#[test]
fn test_pdf_sets_default_layer() {
    crate::common::test_block_on(async {
        let mut keyboard = SimKeyboard::builder([
            [[
                KeyAction::Single(Action::PersistentDefaultLayer(1)),
                KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::A))),
            ]],
            [[
                KeyAction::Single(Action::No),
                KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::B))),
            ]],
        ])
        .build()
        .await;

        keyboard
            .tap(0, 1, 10)
            .tap(0, 0, 10)
            .tap(0, 1, 10)
            .expect_keys([HidKeyCode::A])
            .expect_all_up()
            .expect_keys([HidKeyCode::B])
            .expect_all_up()
            .run()
            .await;
    });
}

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

#[test]
fn test_lm_release() {
    crate::common::test_block_on(async {
        let mut keyboard = SimKeyboard::builder(TEST_KEYMAP).setup(LAYER_SETUP).build().await;

        keyboard
            .delay(0)
            .press(0, 1)
            .delay(100)
            .press(0, 0)
            .delay(100)
            .release(0, 0)
            .delay(0)
            .release(0, 1)
            .expect_only_mods(KC_LSHIFT) // press B
            .expect_keys_with_mods(KC_LSHIFT, [HidKeyCode::B]) // press B
            .expect_only_mods(KC_LSHIFT) // press B
            .expect_all_up() // release B
            .run()
            .await;
    });
}
