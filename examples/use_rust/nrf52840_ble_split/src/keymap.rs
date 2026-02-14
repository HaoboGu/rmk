use rmk::types::action::{EncoderAction, KeyAction};
use rmk::{encoder, k};
use rmk::keymap;

pub(crate) const COL: usize = 7;
pub(crate) const ROW: usize = 8;
pub(crate) const NUM_LAYER: usize = 4;
pub(crate) const NUM_ENCODER: usize = 2;

const DEFAULT_KEYMAP: [[[KeyAction; COL]; ROW]; NUM_LAYER] = keymap! {
    matrix_map: "
        (0,0) (0,1) (0,2) (0,3) (0,4) (0,5) (0,6)
        (1,0) (1,1) (1,2) (1,3) (1,4) (1,5) (1,6)
        (2,0) (2,1) (2,2) (2,3) (2,4) (2,5) (2,6)
        (3,0) (3,1) (3,2) (3,3) (3,4) (3,5) (3,6)
        (4,0) (4,1) (4,2) (4,3) (4,4) (4,5) (4,6)
        (5,0) (5,1) (5,2) (5,3) (5,4) (5,5) (5,6)
        (6,0) (6,1) (6,2) (6,3) (6,4) (6,5) (6,6)
        (7,0) (7,1) (7,2) (7,3) (7,4) (7,5) (7,6)
    ",
    layers: [
        {
            layer: 0,
            name: "base",
            layout: "
                Tab       Q         W         E         R         T         No
                CapsLock  A         S         D         F         G         No
                LShift    Z         X         C         V         B         No
                LCtrl     LGui      LAlt      MO(1)     MO(3)     Space     No
                Backspace P         O         I         U         Y         No
                Enter     Backslash L         K         J         H         No
                Slash     Up        Dot       Comma     M         N         No
                Right     Down      Left      MO(2)     MO(3)     Space     No
            "
        },
        {
            layer: 1,
            name: "num",
            layout: "
                Escape  Kc1     Kc2     Kc3     Kc4       Kc5     No
                No      No      No      No      Semicolon Minus   No
                No      No      No      No      No        No      No
                No      No      No      No      No        No      No
                No      Delete  Kc0     Kc9     Kc8       Kc7     Kc6
                No      No      No      No      No        Quote   Equal
                No      No      No      No      No        No      No
                No      No      No      No      No        No      No
            "
        },
        {
            layer: 2,
            name: "fn",
            layout: "
                No  No  No  No  No  No  No
                No  No  No  No  No  No  No
                No  No  No  No  No  No  No
                No  No  No  No  No  No  No
                No  No  No  No  No  No  No
                No  No  No  No  No  No  No
                No  No  No  No  No  No  No
                No  No  No  No  No  No  No
            "
        },
        {
            layer: 3,
            name: "extra",
            layout: "
                No  No  No  No  No  No  No
                No  No  No  No  No  No  No
                No  No  No  No  No  No  No
                No  No  No  No  No  No  No
                No  No  No  No  No  No  No
                No  No  No  No  No  No  No
                No  No  No  No  No  No  No
                No  No  No  No  No  No  No
            "
        }
    ]
};

pub const fn get_default_keymap() -> [[[KeyAction; COL]; ROW]; NUM_LAYER] {
    DEFAULT_KEYMAP
}

pub const fn get_default_encoder_map() -> [[EncoderAction; NUM_ENCODER]; NUM_LAYER] {
    [
        [
            encoder!(k!(KbVolumeUp), k!(KbVolumeDown)),
            encoder!(k!(KbVolumeUp), k!(KbVolumeDown)),
        ],
        [
            encoder!(k!(KbVolumeUp), k!(KbVolumeDown)),
            encoder!(k!(KbVolumeUp), k!(KbVolumeDown)),
        ],
        [
            encoder!(k!(KbVolumeUp), k!(KbVolumeDown)),
            encoder!(k!(KbVolumeUp), k!(KbVolumeDown)),
        ],
        [
            encoder!(k!(KbVolumeUp), k!(KbVolumeDown)),
            encoder!(k!(KbVolumeUp), k!(KbVolumeDown)),
        ],
    ]
}
