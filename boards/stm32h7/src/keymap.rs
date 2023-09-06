use rmk::{a, action::Action, k, keycode::KeyCode, layer, mo};
const COL: usize = 3;
const ROW: usize = 4;
const NUM_LAYER: usize = 2;

pub static KEYMAP: [[[Action; COL]; ROW]; NUM_LAYER] = [
    layer!([
        [k!(Kp9), k!(Kp8), k!(Kp7)],
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
