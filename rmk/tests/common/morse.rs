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
                    if behavior_config.morse.unilateral_tap {
                        morse.unilateral_tap = true;
                    }
                    morse.mode = behavior_config.morse.mode;
                }
            }
        }
    }

    static BEHAVIOR_CONFIG: static_cell::StaticCell<BehaviorConfig> = static_cell::StaticCell::new();
    let behavior_config = BEHAVIOR_CONFIG.init(behavior_config);
    Keyboard::new(wrap_keymap(keymap, behavior_config))
}
