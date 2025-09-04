use heapless::Vec;
use rmk::config::{BehaviorConfig, MorsesConfig};
use rmk::keyboard::Keyboard;
use rmk::morse::{Morse, MorsePattern};
use rmk::types::action::Action;
use rmk::types::keycode::KeyCode;
use rmk::types::modifier::ModifierCombination;
use rmk::{k, lt, mt, td};

use crate::common::wrap_keymap;

pub fn create_simple_morse_keyboard(behavior_config: BehaviorConfig<1, 5>) -> Keyboard<'static, 1, 5, 2> {
    let keymap = [
        [[
            k!(A),
            mt!(B, ModifierCombination::LSHIFT),
            mt!(C, ModifierCombination::LGUI),
            lt!(1, D),
            td!(0),
        ]],
        [[k!(Kp1), k!(Kp2), k!(Kp3), k!(Kp4), k!(Kp5)]],
    ];

    let morse0 = Morse {
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
        ..Default::default()
    };

    let behavior_config = BehaviorConfig {
        morse: MorsesConfig {
            morses: Vec::from_slice(&[morse0]).unwrap(),
        },
        ..behavior_config
    };

    static BEHAVIOR_CONFIG: static_cell::StaticCell<BehaviorConfig<1, 5>> = static_cell::StaticCell::new();
    let behavior_config = BEHAVIOR_CONFIG.init(behavior_config);
    Keyboard::new(wrap_keymap(keymap, behavior_config))
}
