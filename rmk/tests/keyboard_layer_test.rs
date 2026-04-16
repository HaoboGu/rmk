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
