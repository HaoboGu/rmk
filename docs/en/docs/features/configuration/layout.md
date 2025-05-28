# Layout

## `[layout]`

`[matrix]` defines the physical key matrix on your board, while `[layout]` section contains the layout and the default keymap for the keyboard:

```toml
[layout]
rows = 5
cols = 4
layers = 3
matrix_map = """
    ... the mapping between the "electronic matrix" of your keyboard
        and your key map configuration is described here ...
"""
```

The `matrix_map` is a string built from `(row, col)` coordinate pairs, listed in the same order as you want to define your keys in your key map. The `(row, col)` coordinates are using zero based indexing and referring to the position in the "electronic matrix" of your keyboard. As you can see in [matrix configuration](keyboard_matrix.md), even the direct pin based keyboards are represented with a matrix. In case of split keyboards, the positions refer to the position in the "big unified matrix" of all split parts. With the help of this matrix map, the configuration of non-regular key matrices can be intuitively arranged in your key maps. (Triple quote mark `"""` is used to limit multi-line strings

```toml
# ┌───┬───┬───┬───┐
# │NUM│ / │ * │ - │ <-- row 0, col 0..4
# ├───┼───┼───┼───┤
# │ 7 │ 8 │ 9 │   │
# ├───┼───┼───┤ + │
# │ 4 │ 5 │ 6 │   │
# ├───┼───┼───┼───┤
# │ 1 │ 2 │ 3 │ E │
# ├───┴───┼───┤ N │
# │   0   │ . │ T │
# └───────┴───┴───┘
[layout]
rows = 5
cols = 4
layers = 3
matrix_map = """
(0,0) (0,1) (0,2) (0,3)
(1,0) (1,1) (1,2) (1,3)
(2,0) (2,1) (2,2)
(3,0) (3,1) (3,2) (3,3)
   (4,0)    (4,1)
"""
```

Once the layout is defined, the key mapping can be described for each layer:

```toml
# layer 0 (default):
[[layer]]
name = "base_layer" #optional name for the layer
keys = """
NumLock KpSlash KpAsterisk KpMinus
Kp7     Kp8     Kp9        KpPlus
Kp4     Kp5     Kp6
Kp1     Kp2     Kp3        Enter
    Kp0         KpDot
"""

# layer 1:
[[layer]]
name = "mouse_navigation" #optional name for the layer
keys = """
TO(base_layer)   @my_cut    @my_copy         @my_paste
MouseBtn1        MouseUp    MouseBtn2        MouseWheelUp
MouseLeft        MouseBtn4  MouseRight
MouseWheelLeft   MouseDown  MouseWheelRight  MouseWheelDown
       MouseBtn1            MouseBtn12
"""
```

The number and order of entries on each defined layers must be identical with the number and order of entries in `matrix_map`. White spaces, line breaks are free to vary, but its worth to keep a consistent arrangement with the real keyboard.

::: warning

If the number of defined layers is smaller than what was defined in `layout.layers`, RMK will fill empty layers automatically (so you can configure them freely in Vial). But the empty layers still consumes flash and RAM, so if you don't have a enough space for them, it's not recommended to use a big layer count.

:::

In each `layer.keys`, the keys are bound to various key actions. Due to the limitation of `toml` file, this is done in a string. RMK parses the string and fill the to actual keymap initializer, like what's in [`keymap.rs`](https://github.com/HaoboGu/rmk/tree/main/examples/use_rust/rp2040/src/keymap.rs)

The `layer.keys` string should follow several rules:

1. For a simple keycode(aka keys in RMK's [`KeyCode`](https://docs.rs/rmk/latest/rmk/keycode/enum.KeyCode.html) enum), just fill its name.

   For example, if you set a keycode `Backspace`, it will be turned to `KeyCode::Backspace`. So you have to ensure that the keycode string is valid, or RMK wouldn't compile! However, to make things easier a number of [alternative key names](https://github.com/HaoboGu/rmk/blob/main/rmk-macro/src/keycode_alias.rs) were added and also case-insensitive search is used to find the valid [KeyCode](https://docs.rs/rmk/latest/rmk/keycode/enum.KeyCode.html).

   For simple keycodes with modifiers active, you can use `WM(key, modifier)` to create a keypress with modifier action. Modifiers can be chained together like `LShift | RGui` to have multiple modifiers active.

   You may use aliases, prefixed with `@`, like `@my_copy` in the above example. The alias names are case sensitive. The definition of aliases is described below.

   You may use layer names instead of layer numbers, like `TO(base_layer)` in the above example.
   ::: warning 

   Please note that layer name if used like this, may not contain white spaces and may not be a number. Layer names are case sensitive.
   
   :::

2. For no-key (`KeyAction::No`), use `No`

3. For transparent key (`KeyAction::Transparent`), use `_` or `__` (you can put any number of `_`)

4. RMK supports many advanced layer operations:
   1. Use `DF(n)` to create a switch default layer action, `n` is the layer number
   2. Use `MO(n)` to create a layer activate action, `n` is the layer number
   3. Use `LM(n, modifier)` to create layer activate with modifier action. The modifier can be chained in the same way as `WM`
   4. Use `LT(n, key)` to create a layer activate action or tap key(tap/hold). The `key` here is the RMK [`KeyCode`](https://docs.rs/rmk/latest/rmk/keycode/enum.KeyCode.html)
   5. Use `OSL(n)` to create a one-shot layer action, `n` is the layer number
   6. Use `OSM(modifier)` to create a one-shot modifier action. The modifier can be chained in the same way as `WM`
   7. Use `TT(n)` to create a layer activate or tap toggle action, `n` is the layer number
   8. Use `TG(n)` to create a layer toggle action, `n` is the layer number
   9. Use `TO(n)` to create a layer toggle only action (activate layer `n` and deactivate all other layers), `n` is the layer number

The definitions of those operations are same with QMK, you can found [here](https://docs.qmk.fm/#/feature_layers). If you want other actions, please [fire an issue](https://github.com/HaoboGu/rmk/issues/new).

5. For modifier-tap-hold, use `MT(key, modifier)` where the modifier can be a chain like explained on point 1. For example for a Home row modifier config you can use `MT(F, LShift)`

6. For generic key tap-hold, use `TH(key-tap, key-hold)`

7. For shifted key, use `SHIFTED(key)`

### `[aliases]`

`[aliases]` section contains a table of user defined names and an associated replacement string, which can be used in the `layer.keys`:

```toml
# here are the aliases for the example above
[aliases]
my_cut = "WM(X, LCtrl)"
my_copy = "WM(C, LCtrl)"
my_paste = "WM(V, LCtrl)"
```

::: warning

Please note that alias names may not contain white spaces and they are case sensitive.

:::
