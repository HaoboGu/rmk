use rmk::action::KeyAction;
use rmk::config::BehaviorConfig;
use rmk::keyboard::Keyboard;
use rmk::keycode::ModifierCombination;
use rmk::{k, lt, mt};

use crate::common::wrap_keymap;

pub fn create_simple_morse_keyboard(behavior_config: BehaviorConfig) -> Keyboard<'static, 1, 4, 2> {
    let mut keymap = [
        [[
            k!(A),
            mt!(B, ModifierCombination::SHIFT),
            mt!(C, ModifierCombination::GUI),
            lt!(1, D),
        ]],
        [[k!(Kp1), k!(Kp2), k!(Kp3), k!(Kp4)]],
    ];

    // Update all keys according to behavior config
    for layer in keymap.iter_mut() {
        for row in layer {
            for key in row {
                if let KeyAction::Morse(morse) = key {
                    if behavior_config.tap_hold.unilateral_tap {
                        morse.unilateral_tap = true;
                    }
                    morse.mode = behavior_config.tap_hold.mode;
                }
            }
        }
    }

    Keyboard::new(wrap_keymap(keymap, behavior_config))
}
