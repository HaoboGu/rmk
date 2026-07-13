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
const LAYER_SETUP: SimKeyboardSetup<5, 14> = SimKeyboardSetup::new().keys(&LAYER_KEY_OVERRIDES);

/// Base keymap: col0 switches the default layer, col1 differs per layer so the
/// active base layer is observable from the emitted report.
fn create_pdf_keyboard(behavior_config: BehaviorConfig) -> Keyboard<'static> {
    let keymap = [
        // Layer 0 (initial default): col0 = PDF(1), col1 = A
        [[
            KeyAction::Single(Action::PersistentDefaultLayer(1)),
            KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::A))),
        ]],
        // Layer 1: col1 = B
        [[
            KeyAction::Single(Action::No),
            KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::B))),
        ]],
    ];
    let behavior_config: &'static mut BehaviorConfig = Box::leak(Box::new(behavior_config));
    let per_key_config: &'static PositionalConfig<1, 2> = Box::leak(Box::new(PositionalConfig::default()));
    Keyboard::new(wrap_keymap(keymap, per_key_config, behavior_config))
}

#[test]
fn test_pdf_sets_default_layer() {
    let keyboard = create_pdf_keyboard(BehaviorConfig::default());

    // PDF emits no HID report itself; it changes the default (base) layer, so
    // col1 resolves to A before the PDF press and to B afterwards.
    key_sequence_test!(
        keyboard: keyboard,
        sequence: [
            [0, 1, true, 10],  // col1 -> A (default layer 0)
            [0, 1, false, 10],
            [0, 0, true, 10],  // col0 -> PDF(1): default layer becomes 1
            [0, 0, false, 10],
            [0, 1, true, 10],  // col1 -> now B (default layer 1)
            [0, 1, false, 10],
        ],
        expected_reports: [
            [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // press col1 -> A
            [0, [0, 0, 0, 0, 0, 0]],            // release
            [0, [kc_to_u8!(B), 0, 0, 0, 0, 0]], // press col1 -> B
            [0, [0, 0, 0, 0, 0, 0]],            // release
        ]
    );
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
    let behavior_config: &'static mut BehaviorConfig = Box::leak(Box::new(BehaviorConfig::default()));
    let per_key_config: &'static PositionalConfig<1, 2> = Box::leak(Box::new(PositionalConfig::default()));
    let keyboard = Keyboard::new(wrap_keymap(keymap, per_key_config, behavior_config));

    key_sequence_test!(
        keyboard: keyboard,
        sequence: [
            [0, 1, true, 10],  // col1 -> A (default layer 0)
            [0, 1, false, 10],
            [0, 0, true, 10],  // col0 -> PDF(5): out of range, ignored
            [0, 0, false, 10],
            [0, 1, true, 10],  // col1 -> still A (default layer unchanged)
            [0, 1, false, 10],
        ],
        expected_reports: [
            [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // press col1 -> A
            [0, [0, 0, 0, 0, 0, 0]],            // release
            [0, [kc_to_u8!(A), 0, 0, 0, 0, 0]], // col1 -> still A (PDF(5) ignored)
            [0, [0, 0, 0, 0, 0, 0]],            // release
        ]
    );
}

#[test]
fn test_lm_release() {
    crate::common::test_block_on::test_block_on(async {
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
