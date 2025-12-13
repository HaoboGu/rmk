pub mod common;

use rmk::config::{BehaviorConfig, PositionalConfig};
use rmk::keyboard::Keyboard;
use rmk::types::action::{Action, KeyAction};
use rmk::types::keycode::KeyCode;
use rmk_types::modifier::ModifierCombination;
use rusty_fork::rusty_fork_test;

use crate::common::{KC_LSHIFT, wrap_keymap};

fn create_simple_keyboard(behavior_config: BehaviorConfig) -> Keyboard<'static, 1, 2, 2> {
    let keymap = [
        [[
            KeyAction::Single(Action::Key(KeyCode::A)),
            KeyAction::Single(Action::LayerOnWithModifier(1, ModifierCombination::LSHIFT)),
        ]],
        [[
            KeyAction::Single(Action::Key(KeyCode::B)),
            KeyAction::Single(Action::Key(KeyCode::C)),
        ]],
    ];
    static BEHAVIOR_CONFIG: static_cell::StaticCell<BehaviorConfig> = static_cell::StaticCell::new();
    let behavior_config: &'static mut BehaviorConfig = BEHAVIOR_CONFIG.init(behavior_config);
    static KEY_CONFIG: static_cell::StaticCell<PositionalConfig<1, 2>> = static_cell::StaticCell::new();
    let per_key_config = KEY_CONFIG.init(PositionalConfig::default());
    Keyboard::new(wrap_keymap(keymap, per_key_config, behavior_config))
}

rusty_fork_test! {
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
}
