use rmk::{k, layer};
use rmk::types::action::KeyAction;

pub(crate) const COL: usize = 3;
pub(crate) const ROW: usize = 4;
pub(crate) const NUM_LAYER: usize = 1;

#[rustfmt::skip]
pub const fn get_default_keymap() -> [[[KeyAction; COL]; ROW]; NUM_LAYER] {
    [
        layer!([
            [k!(A), k!(B), k!(C)],
            [k!(D), k!(E), k!(F)],
            [k!(G), k!(H), k!(I)],
            [k!(J), k!(K), k!(L)]
        ]),
    ]
}
