use rmk::types::action::{EncoderAction, KeyAction};
use rmk::{encoder, k, layer};
pub(crate) const COL: usize = 4;
pub(crate) const ROW: usize = 1;
pub(crate) const SIZE: usize = 4;
pub(crate) const NUM_LAYER: usize = 1;
pub(crate) const NUM_ENCODER: usize = 1;

#[rustfmt::skip]
pub const fn get_default_keymap() -> [[[KeyAction; COL]; ROW]; NUM_LAYER] {
    [
        layer!([
            [k!(A), k!(B), k!(C), k!(D)]
        ]),
    ]
}

pub const fn get_default_encoder_map() -> [[EncoderAction; NUM_ENCODER]; NUM_LAYER] {
    [[encoder!(k!(KbVolumeUp), k!(KbVolumeDown))]]
}
