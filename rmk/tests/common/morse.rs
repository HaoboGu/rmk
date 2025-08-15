use heapless::Vec;

use rmk::action::Action;
use rmk::config::{BehaviorConfig, MorseConfig};
use rmk::keyboard::Keyboard;
use rmk::keycode::{KeyCode, ModifierCombination};
use rmk::morse::{Morse, MorsePattern};
use rmk::{k, lt, mrs, mt};

use crate::common::wrap_keymap;

pub fn create_simple_morse_keyboard(behavior_config: BehaviorConfig) -> Keyboard<'static, 1, 5, 2> {
    let keymap = [
        [[
            k!(A),
            mt!(B, ModifierCombination::SHIFT),
            mt!(C, ModifierCombination::GUI),
            lt!(1, D),
            mrs!(0),
        ]],
        [[k!(Kp1), k!(Kp2), k!(Kp3), k!(Kp4), k!(Kp5)]],
    ];

    let morse0 = Morse {
        actions: Vec::from_slice(&[
            (MorsePattern::from_u16(0b1_01), Action::Key(KeyCode::A)),
            (MorsePattern::from_u16(0b1_1000), Action::Key(KeyCode::B)),
            (MorsePattern::from_u16(0b1_1010), Action::Key(KeyCode::C)),
            (MorsePattern::from_u16(0b1_100), Action::Key(KeyCode::D)),
            (MorsePattern::from_u16(0b1_0), Action::Key(KeyCode::E)),
            (MorsePattern::from_u16(0b1_0010), Action::Key(KeyCode::F)),
            (MorsePattern::from_u16(0b1_110), Action::Key(KeyCode::G)),
            (MorsePattern::from_u16(0b1_0000), Action::Key(KeyCode::H)),
            (MorsePattern::from_u16(0b1_00), Action::Key(KeyCode::I)),
            (MorsePattern::from_u16(0b1_0111), Action::Key(KeyCode::J)),
            (MorsePattern::from_u16(0b1_101), Action::Key(KeyCode::K)),
            (MorsePattern::from_u16(0b1_0100), Action::Key(KeyCode::L)),
            (MorsePattern::from_u16(0b1_11), Action::Key(KeyCode::M)),
            (MorsePattern::from_u16(0b1_10), Action::Key(KeyCode::N)),
            (MorsePattern::from_u16(0b1_111), Action::Key(KeyCode::O)),
            (MorsePattern::from_u16(0b1_0110), Action::Key(KeyCode::P)),
            (MorsePattern::from_u16(0b1_1101), Action::Key(KeyCode::Q)),
            (MorsePattern::from_u16(0b1_010), Action::Key(KeyCode::R)),
            (MorsePattern::from_u16(0b1_000), Action::Key(KeyCode::S)),
            (MorsePattern::from_u16(0b1_1), Action::Key(KeyCode::T)),
            (MorsePattern::from_u16(0b1_001), Action::Key(KeyCode::U)),
            (MorsePattern::from_u16(0b1_0001), Action::Key(KeyCode::V)),
            (MorsePattern::from_u16(0b1_011), Action::Key(KeyCode::W)),
            (MorsePattern::from_u16(0b1_1001), Action::Key(KeyCode::X)),
            (MorsePattern::from_u16(0b1_1011), Action::Key(KeyCode::Y)),
            (MorsePattern::from_u16(0b1_1100), Action::Key(KeyCode::Z)),
            (MorsePattern::from_u16(0b1_01111), Action::Key(KeyCode::Kc1)),
            (MorsePattern::from_u16(0b1_00111), Action::Key(KeyCode::Kc2)),
            (MorsePattern::from_u16(0b1_00011), Action::Key(KeyCode::Kc3)),
            (MorsePattern::from_u16(0b1_00001), Action::Key(KeyCode::Kc4)),
            (MorsePattern::from_u16(0b1_00000), Action::Key(KeyCode::Kc5)),
            (MorsePattern::from_u16(0b1_10000), Action::Key(KeyCode::Kc6)),
            (MorsePattern::from_u16(0b1_11000), Action::Key(KeyCode::Kc7)),
            (MorsePattern::from_u16(0b1_11100), Action::Key(KeyCode::Kc8)),
            (MorsePattern::from_u16(0b1_11110), Action::Key(KeyCode::Kc9)),
            (MorsePattern::from_u16(0b1_11111), Action::Key(KeyCode::Kc0)),
        ])
        .unwrap(),
        ..Default::default()
    };

    let behavior_config = BehaviorConfig {
        morse: MorseConfig {
            action_sets: Vec::from_slice(&[morse0]).unwrap(),
            ..behavior_config.morse
        },
        ..behavior_config
    };

    Keyboard::new(wrap_keymap(keymap, behavior_config))
}
