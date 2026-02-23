# Keymap configuration

RMK supports configuring the default keymap at the compile time. Keymap in RMK is a 3-D matrix of [`KeyAction`](https://docs.rs/rmk/latest/rmk/action/enum.KeyAction.html), which represent the keyboard's action after you trigger a physical key. The 3 dimensions are the number of columns, rows and layers.

RMK provides both Rust code or config ways to set your default keymap.

## Define default keymap in `keyboard.toml`

Please check [layout section](../layout) in keyboard configuration doc.

## Define default keymap in Rust source file

The default keymap could also be defined in a Rust source file using the `keymap!` macro. The macro uses the same key action syntax as `keyboard.toml`, so you can use the same key names and actions in both places.

There are `keymap.rs` files in the example folder, such as [this](https://github.com/HaoboGu/rmk/blob/main/examples/use_rust/rp2040/src/keymap.rs), which is a good example of defining keymaps using Rust in RMK:

```rust
// https://github.com/HaoboGu/rmk/blob/main/examples/use_rust/rp2040/src/keymap.rs
use rmk::types::action::KeyAction;
use rmk::keymap;

pub(crate) const COL: usize = 3;
pub(crate) const ROW: usize = 4;
pub(crate) const NUM_LAYER: usize = 2;

const DEFAULT_KEYMAP: [[[KeyAction; COL]; ROW]; NUM_LAYER] = keymap! {
    matrix_map: "
        (0,0) (0,1) (0,2)
        (1,0) (1,1) (1,2)
        (2,0) (2,1) (2,2)
        (3,0) (3,1) (3,2)
    ",
    aliases: {
        fn_layer = "MO(fn)",
    },
    layers: [
        {
            layer: 0,
            name: "base",
            layout: "
                AudioVolUp  B           AudioVolDown
                Kp4         LShift      Kp6
                @fn_layer   Kp2         Kp3
                @fn_layer   No          Kp0
            "
        },
        {
            layer: 1,
            name: "fn",
            layout: "
                Kp7     Kp8     Kp9
                Kp4     LCtrl   Kp6
                MO(1)   Kp2     Kp3
                MO(1)   No      Kp0
            "
        }
    ]
};

pub const fn get_default_keymap() -> [[[KeyAction; COL]; ROW]; NUM_LAYER] {
    DEFAULT_KEYMAP
}
```

First of all, the keyboard matrix's basic info (number of rows, cols and layers) is defined as consts:

```rust
pub(crate) const COL: usize = 3;
pub(crate) const ROW: usize = 4;
pub(crate) const NUM_LAYER: usize = 2;
```

Then, the keymap is defined using the `keymap!` macro. It has three sections:

- **`matrix_map`**: Defines the physical layout as `(row, col)` coordinates. Each coordinate maps a position in the layout strings to a position in the key matrix.
- **`aliases`** (optional): Defines shorthand names for key actions. Use `@alias_name` in the layout to reference them.
- **`layers`**: A list of layer definitions. Each layer has a numeric `layer` id (must be contiguous starting from 0), an optional `name` (which can be used in layer-switching actions like `MO(fn)`), and a `layout` string with key actions.

The layout strings use the same key action syntax as `keyboard.toml`. For example, `No` means no action, `MO(1)` activates layer 1 momentarily, and `WM(C, LCtrl)` sends Ctrl+C.

A `get_default_keymap()` function should return the keymap constant for use by the rest of the firmware. You can check all available key actions in the [layout section](../layout) of the keyboard configuration doc.
