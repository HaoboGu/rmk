use rmk_macro::keymap;
const ROW: usize = 2;
const COL: usize = 3;
const NUM_LAYER: usize = 2;
const KEYMAP: [[[rmk::types::action::KeyAction; COL]; ROW]; NUM_LAYER] = [
    [
        [
            ::rmk::types::action::KeyAction,
            ::rmk::types::action::KeyAction,
            ::rmk::types::action::KeyAction,
        ],
        [
            ::rmk::types::action::KeyAction,
            ::rmk::types::action::KeyAction,
            ::rmk::types::action::KeyAction,
        ],
    ],
    [
        [
            ::rmk::types::action::KeyAction,
            ::rmk::types::action::KeyAction,
            ::rmk::types::action::KeyAction,
        ],
        [
            ::rmk::types::action::KeyAction,
            ::rmk::types::action::KeyAction,
            ::rmk::types::action::KeyAction,
        ],
    ],
];
fn main() {}
