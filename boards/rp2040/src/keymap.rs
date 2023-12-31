use rmk::action::KeyAction;
use rmk::{a, k, layer, mo};
const COL: usize = 3;
const ROW: usize = 4;
const NUM_LAYER: usize = 2;

pub static KEYMAP: [[[KeyAction; COL]; ROW]; NUM_LAYER] = [
    layer!([
        [k!(A), k!(B), k!(C)],
        [k!(Kp4), k!(LShift), k!(Kp6)],
        [k!(Kp1), k!(Kp2), k!(Kp3)],
        [mo!(1), a!(No), k!(Kp0)]
    ]),
    layer!([
        [k!(Kp7), k!(Kp8), k!(Kp9)],
        [k!(Kp4), k!(LCtrl), k!(Kp6)],
        [k!(Kp1), k!(Kp2), k!(Kp3)],
        [mo!(1), a!(No), k!(Kp0)]
    ]),
];
