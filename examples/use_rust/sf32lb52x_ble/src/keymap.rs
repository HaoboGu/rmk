use rmk::types::action::KeyAction;
use rmk::{k, layer};
pub(crate) const COL: usize = 1;
pub(crate) const ROW: usize = 1;
pub(crate) const SIZE: usize = 1;
pub(crate) const NUM_LAYER: usize = 1;

#[rustfmt::skip]
pub const fn get_default_keymap() -> [[[KeyAction; COL]; ROW]; NUM_LAYER] {
    [
        layer!([
            [k!(A)]
        ]),
    ]
}
