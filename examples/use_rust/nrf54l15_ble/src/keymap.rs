use rmk::types::action::KeyAction;
use rmk::{k, layer, mo};

pub(crate) const COL: usize = 2;
pub(crate) const ROW: usize = 2;
pub(crate) const SIZE: usize = 4;
pub(crate) const NUM_LAYER: usize = 2;

#[rustfmt::skip]
pub const fn get_default_keymap() -> [[[KeyAction; COL]; ROW]; NUM_LAYER] {
    [
        layer!([
            [k!(Escape), k!(Enter)],
            [k!(Space), mo!(1)]
        ]),
        layer!([
            [k!(KbVolumeDown), k!(KbVolumeUp)],
            [k!(Tab), mo!(1)]
        ]),
    ]
}
