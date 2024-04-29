use crate::{COL, NUM_LAYER, ROW};
use rmk::action::KeyAction;
use rmk::{a, k, layer, mo};

#[rustfmt::skip]
pub static KEYMAP: [[[KeyAction; COL]; ROW]; NUM_LAYER] = [
    layer!([
        [k!(AudioVolUp), k!(B), k!(AudioVolDown)],
        [k!(Kp4), k!(LShift), k!(Kp6)],
        [mo!(1), k!(Kp2), k!(Kp3)],
        [mo!(1), a!(No), k!(Kp0)]
    ]),
    layer!([
        [k!(Kp7), k!(Kp8), k!(Kp9)],
        [k!(Kp4), k!(LCtrl), k!(Kp6)],
        [mo!(1), k!(Kp2), k!(Kp3)],
        [mo!(1), a!(No), k!(Kp0)]
    ]),
];
