# Keymap configuration

RMK supports configuring the default keymap at the compile time. Keymap in RMK is a 3-D matrix of [`KeyAction`](https://docs.rs/rmk/latest/rmk/action/enum.KeyAction.html), which represent the keyboard's action after you trigger a physical key. The 3 dimensions are the number of columns, rows and layers.

RMK provides both Rust code or config ways to set your default keymap.

## Define default keymap in `keyboard.toml`

Please check [this section](keyboard_configuration.md#layout) in keyboard configuration doc.

## Define default keymap in Rust source file

The default keymap could also be defined at a Rust source file, [rmk-template](https://github.com/HaoboGu/rmk-template) provides an initial [`keymap.rs`](https://github.com/HaoboGu/rmk-template/blob/central/src/keymap.rs) which could be a good example of defining keymaps in RMK:

```rust
/// https://github.com/HaoboGu/rmk-template/blob/central/src/keymap.rs
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

First of all, the keyboard matrix's basic info(number of rows, cols and layers) is defined as consts:

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

A keymap in RMK is a 3-level hierarchy: layer - row - column. Each keymap is a slice of layers whose length is `NUM_LAYER`. Each layer is a slice of rows whose length is `ROW`, and each row is a slice of `KeyAction`s whose length is `COL`.

RMK provides a bunch of macros which simplify the keymap definition a lot. You can check all available macros in [RMK doc](https://docs.rs/rmk/latest/rmk/index.html#macros). For example, `layer!` macro is used to define a layer. `k!` macro is used to define a normal key in the keymap. If there is no actual key at a position, you can use `a!(No)` to represent `KeyAction::No`.
