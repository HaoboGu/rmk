use rmk::types::action::{EncoderAction, KeyAction};
use rmk::{encoder, k};
use rmk::keymap;

pub(crate) const COL: usize = 14;
pub(crate) const ROW: usize = 5;
pub(crate) const NUM_LAYER: usize = 8;
pub(crate) const NUM_ENCODER: usize = 2;

const DEFAULT_KEYMAP: [[[KeyAction; COL]; ROW]; NUM_LAYER] = keymap! {
    matrix_map: "
        (0,0) (0,1) (0,2) (0,3) (0,4) (0,5) (0,6) (0,7) (0,8) (0,9) (0,10) (0,11) (0,12) (0,13)
        (1,0) (1,1) (1,2) (1,3) (1,4) (1,5) (1,6) (1,7) (1,8) (1,9) (1,10) (1,11) (1,12) (1,13)
        (2,0) (2,1) (2,2) (2,3) (2,4) (2,5) (2,6) (2,7) (2,8) (2,9) (2,10) (2,11) (2,12) (2,13)
        (3,0) (3,1) (3,2) (3,3) (3,4) (3,5) (3,6) (3,7) (3,8) (3,9) (3,10) (3,11) (3,12) (3,13)
        (4,0) (4,1) (4,2) (4,3) (4,4) (4,5) (4,6) (4,7) (4,8) (4,9) (4,10) (4,11) (4,12) (4,13)
    ",
    layers: [
        {
            layer: 0,
            name: "base",
            layout: "
                Grave   Kc1   Kc2   Kc3  Kc4  Kc5    Kc6  Kc7  Kc8    Kc9  Kc0       Minus        Equal        Backspace
                Tab     Q     W     E    R    T      Y    U    I      O    P         LeftBracket  RightBracket Backslash
                Escape  A     S     D    F    G      H    J    K      L    Semicolon Quote        No           Enter
                LShift  Z     X     C    V    B      N    M    Comma  Dot  Slash     No           No           RShift
                LCtrl   LGui  LAlt  No   No   Space  No   No   No     MO(1) RAlt     No           RGui         RCtrl
            "
        },
        {
            layer: 1,
            name: "fn1",
            layout: "
                Grave    F1  F2  F3  F4  F5  F6  F7  F8  F9  F10   F11  F12  Delete
                No       No  No  No  No  No  No  No  No  No  No    No   No   No
                CapsLock No  No  No  No  No  No  No  No  No  No    No   No   No
                No       No  No  No  No  No  No  No  No  No  No    No   No   Up
                No       No  No  No  No  No  No  No  No  No  Left  No   Down Right
            "
        },
        {
            layer: 2,
            name: "fn2",
            layout: "
                Grave    F1  F2  F3  F4  F5  F6  F7  F8  F9  F10   F11  F12  Delete
                No       No  No  No  No  No  No  No  No  No  No    No   No   No
                CapsLock No  No  No  No  No  No  No  No  No  No    No   No   No
                No       No  No  No  No  No  No  No  No  No  No    No   No   Up
                No       No  No  No  No  No  No  No  No  No  Left  No   Down Right
            "
        },
        {
            layer: 3,
            name: "fn3",
            layout: "
                Grave    F1  F2  F3  F4  F5  F6  F7  F8  F9  F10   F11  F12  Delete
                No       No  No  No  No  No  No  No  No  No  No    No   No   No
                CapsLock No  No  No  No  No  No  No  No  No  No    No   No   No
                No       No  No  No  No  No  No  No  No  No  No    No   No   Up
                No       No  No  No  No  No  No  No  No  No  Left  No   Down Right
            "
        },
        {
            layer: 4,
            name: "fn4",
            layout: "
                Grave    F1  F2  F3  F4  F5  F6  F7  F8  F9  F10   F11  F12  Delete
                No       No  No  No  No  No  No  No  No  No  No    No   No   No
                CapsLock No  No  No  No  No  No  No  No  No  No    No   No   No
                No       No  No  No  No  No  No  No  No  No  No    No   No   Up
                No       No  No  No  No  No  No  No  No  No  Left  No   Down Right
            "
        },
        {
            layer: 5,
            name: "fn5",
            layout: "
                Grave    F1  F2  F3  F4  F5  F6  F7  F8  F9  F10   F11  F12  Delete
                No       No  No  No  No  No  No  No  No  No  No    No   No   No
                CapsLock No  No  No  No  No  No  No  No  No  No    No   No   No
                No       No  No  No  No  No  No  No  No  No  No    No   No   Up
                No       No  No  No  No  No  No  No  No  No  Left  No   Down Right
            "
        },
        {
            layer: 6,
            name: "fn6",
            layout: "
                Grave    F1  F2  F3  F4  F5  F6  F7  F8  F9  F10   F11  F12  Delete
                No       No  No  No  No  No  No  No  No  No  No    No   No   No
                CapsLock No  No  No  No  No  No  No  No  No  No    No   No   No
                No       No  No  No  No  No  No  No  No  No  No    No   No   Up
                No       No  No  No  No  No  No  No  No  No  Left  No   Down Right
            "
        },
        {
            layer: 7,
            name: "fn7",
            layout: "
                Grave    F1  F2  F3  F4  F5  F6  F7  F8  F9  F10   F11  F12  Delete
                No       No  No  No  No  No  No  No  No  No  No    No   No   No
                CapsLock No  No  No  No  No  No  No  No  No  No    No   No   No
                No       No  No  No  No  No  No  No  No  No  No    No   No   Up
                No       No  No  No  No  No  No  No  No  No  Left  No   Down Right
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
