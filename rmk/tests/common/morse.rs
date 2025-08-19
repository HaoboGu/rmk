use heapless::Vec;

use rmk::action::Action;
use rmk::config::{BehaviorConfig, TapDancesConfig};
use rmk::keyboard::Keyboard;
use rmk::keycode::{KeyCode, ModifierCombination};
use rmk::tap_dance::{MorsePattern, TapDance};
use rmk::{k, lt, mt, td};

use crate::common::wrap_keymap;

pub fn create_simple_morse_keyboard(behavior_config: BehaviorConfig) -> Keyboard<'static, 1, 5, 2> {
    let keymap = [
        [[
            k!(A),
            mt!(B, ModifierCombination::SHIFT),
            mt!(C, ModifierCombination::GUI),
            lt!(1, D),
            td!(0),
        ]],
        [[k!(Kp1), k!(Kp2), k!(Kp3), k!(Kp4), k!(Kp5)]],
    ];

    let morse0 = TapDance {
        actions: Vec::from_slice(&[
            (MorsePattern::from_u16(0b1_01), Action::Key(KeyCode::A)),
            (MorsePattern::from_u16(0b1_1000), Action::Key(KeyCode::B)),
            (MorsePattern::from_u16(0b1_1010), Action::Key(KeyCode::C)),
            (MorsePattern::from_u16(0b1_101), Action::Key(KeyCode::K)),
            (MorsePattern::from_u16(0b1_11), Action::Key(KeyCode::M)),
            (MorsePattern::from_u16(0b1_111), Action::Key(KeyCode::O)),
            (MorsePattern::from_u16(0b1_010), Action::Key(KeyCode::R)),
            (MorsePattern::from_u16(0b1_000), Action::Key(KeyCode::S)),
        ])
        .unwrap(),
        strict_pattern_checking: true,
        ..Default::default()
    };

    let behavior_config = BehaviorConfig {
        tap_dance: TapDancesConfig {
            tap_dances: Vec::from_slice(&[morse0]).unwrap(),
        },
        ..behavior_config
    };

    Keyboard::new(wrap_keymap(keymap, behavior_config))
}
