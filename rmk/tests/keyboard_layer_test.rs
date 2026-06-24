pub mod common;

use rmk::config::{BehaviorConfig, PositionalConfig};
use rmk::keyboard::Keyboard;
use rmk::types::action::{Action, KeyAction};
use rmk::types::keycode::{HidKeyCode, KeyCode};
use rmk_types::modifier::ModifierCombination;

use crate::common::{KC_LSHIFT, wrap_keymap};

fn create_simple_keyboard(behavior_config: BehaviorConfig) -> Keyboard<'static> {
    let keymap = [
        [[
            KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::A))),
            KeyAction::Single(Action::LayerOnWithModifier(1, ModifierCombination::LSHIFT)),
        ]],
        [[
            KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::B))),
            KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::C))),
        ]],
    ];
    let behavior_config: &'static mut BehaviorConfig = Box::leak(Box::new(behavior_config));
    let per_key_config: &'static PositionalConfig<1, 2> = Box::leak(Box::new(PositionalConfig::default()));
    Keyboard::new(wrap_keymap(keymap, per_key_config, behavior_config))
}

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
    let config = BehaviorConfig::default();
    let keyboard = create_simple_keyboard(config);

    key_sequence_test!(
        keyboard: keyboard,
        sequence: [
            [0, 1, true, 0],
            [0, 0, true, 100],
            [0, 0, false, 100],
            [0, 1, false, 0],
        ],
        expected_reports: [
            [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // press B
            [KC_LSHIFT, [kc_to_u8!(B), 0, 0, 0, 0, 0]], // press B
            [KC_LSHIFT, [0, 0, 0, 0, 0, 0]], // press B
            [0, [0, 0, 0, 0, 0, 0]],            // release B
        ]
    );
}
