use rmk::action::KeyAction;
use rmk::{a, k, layer, mo};
pub(crate) const COL: usize = 12;
pub(crate) const ROW: usize = 8;
pub(crate) const NUM_LAYER: usize = 4;

#[rustfmt::skip]
pub static KEYMAP: [[[KeyAction; COL]; ROW]; NUM_LAYER] = [
    layer!([
        [ k!(Kc1), k!(Kc2), k!(Kc3), k!(Kc4), k!(Kc5), k!(Kc6), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No)],
        [ k!(Tab), k!(Q), k!(W), k!(E), k!(R), k!(T), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No)],
        [ k!(CapsLock), k!(A), k!(S), k!(D), k!(F), k!(G), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No)],
        [ k!(C), k!(Z), k!(X), k!(C), k!(V), k!(B), k!(C), a!(No), k!(C), k!(C), k!(C), a!(No)],
        [ a!(No), k!(C), k!(C), k!(C), a!(No), k!(C),        k!(Kc7), k!(Kc8), k!(Kc9), k!(Kc0), k!(Minus), k!(Equal) ],
        [ k!(C), k!(C), k!(C), k!(C), k!(C), k!(C),        k!(Y), k!(U), k!(I), k!(O), k!(P), k!(C) ],
        [ k!(C), k!(C), k!(C), k!(C), k!(C), k!(C),        k!(H), k!(J), k!(K), k!(L), k!(C), k!(C) ],
        [ k!(C), k!(C), k!(C), k!(C), k!(C), k!(C),        k!(N), k!(M), k!(LeftBracket), k!(RightBracket), k!(C), k!(C) ]
    ]),
    layer!([
        [ k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C) ],
        [ k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C) ],
        [ k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C) ],
        [ k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C) ],
        [ k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C) ],
        [ k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C) ],
        [ k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C) ],
        [ k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C) ]
    ]),
    layer!([
        [ k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C) ],
        [ k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C) ],
        [ k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C) ],
        [ k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C) ],
        [ k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C) ],
        [ k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C) ],
        [ k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C) ],
        [ k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C) ]
    ]),
    layer!([
        [ k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C) ],
        [ k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C) ],
        [ k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C) ],
        [ k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C) ],
        [ k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C) ],
        [ k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C) ],
        [ k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C) ],
        [ k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C), k!(C) ]
    ]),
];
