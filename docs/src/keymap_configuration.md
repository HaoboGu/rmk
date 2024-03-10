# Keymap configuration

RMK supports configuring the default keymap at the compile time. Keymap in RMK is a 3-D matrix of [`KeyAction`](https://docs.rs/rmk/latest/rmk/action/enum.KeyAction.html), which represent the keyboard's action after you trigger a physical key. The 3 dimensions are the number of columns, rows and layers.

The default keymap should be defined at a Rust source file, [rmk-template](https://github.com/HaoboGu/rmk-template) provides an initial [`keymap.rs`](https://github.com/HaoboGu/rmk-template/blob/master/src/keymap.rs) which could be a good example of definiting keymaps in RMK:

```rust
/// https://github.com/HaoboGu/rmk-template/blob/master/src/keymap.rs
use rmk::action::KeyAction;
use rmk::{a, k, layer, mo};
pub(crate) const COL: usize = 3;
pub(crate) const ROW: usize = 4;
pub(crate) const NUM_LAYER: usize = 2;

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
```

First of all, the keyboard matrix's basic info is defined as consts:

```rust
pub(crate) const COL: usize = 3;
pub(crate) const ROW: usize = 4;
pub(crate) const NUM_LAYER: usize = 2;
```

Then, the keymap is defined as a static 3-D matrix of `KeyAction`: 

```rust
pub static KEYMAP: [[[KeyAction; COL]; ROW]; NUM_LAYER] = [
    ...
]
```

`KEYMAP` is defined as a slice of layers and a layer is defined as a slice of rows and a row is defined as a slice of cols. So the order of keymap matrix is fixed. 

RMK provides a bunch of macros which simplify the keymap definition a lot. You can check all the macros [here](https://docs.rs/rmk/latest/rmk/index.html#macros). For example, `layer!` macro is used to define a layer. Each layer contains several row slices. And in each row slice, the `KeyAction` is defined. To define a normal key in the keymap, `k!` macro is used. If there is no actual key at a position, you can use `a!(No)` to represent `KeyAction::No`.

