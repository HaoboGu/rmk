use rmk::config::BehaviorConfig;
use rmk::keyboard::Keyboard;
use rmk::keycode::ModifierCombination;
use rmk::{k, lt, mt};

use crate::common::wrap_keymap;

pub fn create_simple_morse_keyboard(behavior_config: BehaviorConfig) -> Keyboard<'static, 1, 4, 2> {
    let keymap = [
        [[
            k!(A),
            mt!(B, ModifierCombination::SHIFT),
            mt!(C, ModifierCombination::GUI),
            lt!(1, D),
        ]],
        [[k!(Kp1), k!(Kp2), k!(Kp3), k!(Kp4)]],
    ];

    Keyboard::new(wrap_keymap(keymap, behavior_config))
}
