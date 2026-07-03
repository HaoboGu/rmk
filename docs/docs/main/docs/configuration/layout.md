# Layout

## Keyboard layout configuration

RMK splits the keyboard description into three sections:

- `[matrix]` — the **electrical** key matrix (GPIO wiring), see [matrix configuration](./keyboard_matrix#matrix-configuration).
- `[layout]` — the **physical** arrangement: the grid size (`rows`/`cols`) and the `map` of key positions.
- `[keymap]` — the **logical** assignments: the layer count and what each key does.

If your keyboard already has a [KLE](http://www.keyboard-layout-editor.com/) layout or a Vial `vial.json`, the `[layout]` section can be generated from it — see [converting from KLE or Vial](#converting-from-kle-or-vial).

```toml
[layout]
rows = 5
cols = 4
map = """
    ... the mapping between the "electronic matrix" of your keyboard
        and your key map configuration is described here ...
"""

[keymap]
layers = 3
```

`[keymap].layers` is optional: it defaults to the number of `[[keymap.layer]]` blocks you define. Set it explicitly only to reserve extra empty layers (for example, to leave room for on-the-fly Vial/Rynk editing).

The `[layout].map` is a string built from `(row, col, <hand>)` tuples, listed in the same order as you want to define your keys in your keymap.

The `(row, col)` coordinates are using zero based indexing and referring to the position in the "electronic matrix" of your keyboard. As you can see in [matrix configuration](./keyboard_matrix#matrix-configuration), even the direct pin based keyboards are represented with a matrix. In case of split keyboards, the positions refer to the position in the "big unified matrix" of all split parts.
With the help of this matrix map, the configuration of non-regular key matrices can be intuitively arranged in your key maps. (Triple quote mark `"""` is used to limit multi-line strings)

The `map` and `[[keymap.layer]].keys` strings hold data only — they don't support inline comments. Put any annotations in normal TOML `#` comments outside the `"""…"""` block.

The `<hand>` is optional, it should only be used when `unilateral_tap = true`. By assigning `L` or `R` to `<hand>`, each key can be associated with either the left or right hand. If the `<hand>` is set to `*` it will be considered a "bilateral" key, meaning that in `unilateral_tap = true` it will be treated as opposite hand regardless of on which hand modifier was pressed.

```toml
# simple numpad example:
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
map = """
(0,0) (0,1) (0,2) (0,3)
(1,0) (1,1) (1,2) (1,3)
(2,0) (2,1) (2,2)
(3,0) (3,1) (3,2) (3,3)
   (4,0)      (4,1)
"""

[keymap]
layers = 3

# split ortho example for the layout map, with L/R hand information filled
[layout]
rows = 4
cols = 10
map = """
(0, 0, L)  (0, 1, L)  (0, 2, L)  (0, 3, L)  (0, 4, L)    (0, 5, R)  (0, 6, R)  (0, 7, R)  (0, 8, R)  (0, 9, R)
(1, 0, L)  (1, 1, L)  (1, 2, L)  (1, 3, L)  (1, 4, L)    (1, 5, R)  (1, 6, R)  (1, 7, R)  (1, 8, R)  (1, 9, R)
(2, 0, L)  (2, 1, L)  (2, 2, L)  (2, 3, L)  (2, 4, L)    (2, 5, R)  (2, 6, R)  (2, 7, R)  (2, 8, R)  (2, 9, R)
                                 (3, 3, L)  (3, 4, L)    (3, 5, R)  (3, 6, R)
"""

[keymap]
layers = 3
```

Once the layout is defined, the keymap is described for each layer under `[[keymap.layer]]`:

```toml
# layer 0 (default):
[[keymap.layer]]
name = "base_layer" #optional name for the layer
keys = """
NumLock KpSlash KpAsterisk KpMinus
Kp7     Kp8     Kp9        KpPlus
Kp4     Kp5     Kp6
Kp1     Kp2     Kp3        Enter
    Kp0         KpDot
"""

# layer 1:
[[keymap.layer]]
name = "mouse_navigation" #optional name for the layer
keys = """
TO(base_layer)   @my_cut    @my_copy         @my_paste
MouseBtn1        MouseUp    MouseBtn2        MouseWheelUp
MouseLeft        MouseBtn4  MouseRight
MouseWheelLeft   MouseDown  MouseWheelRight  MouseWheelDown
       MouseBtn1            MouseBtn2
"""
```

The number and order of entries on each defined layer must be identical with the number and order of entries in `layout.map`. White spaces and line breaks are free to vary, but it's worth keeping a consistent arrangement with the real keyboard.

::: warning

If the number of defined layers is smaller than what was defined in `keymap.layers`, RMK will fill empty layers automatically (so you can configure them freely in Vial). But the empty layers still consume flash and RAM, so if you don't have enough space for them, it's not recommended to use a big layer count.

:::

In each `layer.keys`, the keys are bound to various key actions. Due to the limitation of the `toml` file format, this is done in a string. RMK parses the string and fills in the actual keymap initializer, like what's in [`keymap.rs`](https://github.com/HaoboGu/rmk/tree/main/examples/use_rust/rp2040/src/keymap.rs)

The `layer.keys` string should follow several rules:

1. For a simple keycode (i.e., keys in RMK's [`HidKeyCode`](https://docs.rs/rmk/latest/rmk/keycode/enum.HidKeyCode.html) enum), just fill in its name.

   For example, if you set a keycode `Backspace`, it will be turned to the corresponding HID keycode. So you have to ensure that the keycode string is valid, or RMK wouldn't compile! However, to make things easier a number of alternative key names (see alias column in [KeyCode table](./keymap_configuration/keycodes)) were added and also case-insensitive search is used to find the valid keycode.

   For simple keycodes with modifiers active, you can use `WM(key, modifier)` to create a keypress with modifier action. Modifiers can be chained together like `LShift | RGui` to have multiple modifiers active.

   You may use aliases, prefixed with `@`, like `@my_copy` in the above example. The alias names are case sensitive. The definition of aliases is described below.

   You may use layer names instead of layer numbers, like `TO(base_layer)` in the above example.
   ::: warning

   Please note that layer name if used like this, may not contain white spaces and may not be a number. Layer names are case sensitive.

   :::

2. For no-key (`KeyAction::No`), use `No`

3. For transparent key (`KeyAction::Transparent`), use `_` or `__` (you can put any number of `_`)

4. RMK supports many advanced layer operations:
   1. Use `DF(n)` to create a switch default layer action, `n` is the layer number. Use `PDF(n)` for a persistent version that is saved to storage and restored after reboot
   2. Use `MO(n)` to create a layer activate action, `n` is the layer number
   3. Use `LM(n, modifier)` to create layer activate with modifier action. The modifier can be chained in the same way as `WM`
   4. Use `LT(n, key, <profile_name>)` to create a layer activate action or tap key(tap/hold). The `key` here is the RMK [`KeyCode`](https://docs.rs/rmk/latest/rmk/keycode/enum.KeyCode.html), The `profile_name` is optional, which defines the key's [profile](./behavior#per-key-profiles-for-morse-tapdance-tap-hold-fine-tuning)
   5. Use `OSL(n)` to create a one-shot layer action, `n` is the layer number
   6. Use `OSM(modifier)` to create a one-shot modifier action. The modifier can be chained in the same way as `WM`
   7. Use `TT(n)` to create a layer activate or tap toggle action, `n` is the layer number
   8. Use `TG(n)` to create a layer toggle action, `n` is the layer number
   9. Use `TO(n)` to create a layer toggle only action (activate layer `n` and deactivate all other layers), `n` is the layer number

The definitions of these operations are the same as QMK's; you can find them [here](https://docs.qmk.fm/#/feature_layers). If you want other actions, please [file an issue](https://github.com/HaoboGu/rmk/issues/new).

5. For modifier-tap-hold, use `MT(key, modifier, <profile_name>)` where the modifier can be a chain like explained on point 1. The `profile_name` is optional, which defines the key's [profile](./behavior#per-key-profiles-for-morse-tapdance-tap-hold-fine-tuning)
<!-- If you're using home-row mod(HRM), you can also use `HRM(key, modifier)` to create a modifier-tap-hold whose configuration is optimized for home-row mod. -->

6. For generic key tap-hold, use `TH(key-tap, key-hold, <profile_name>)`, The `profile_name` is optional, which defines the key's [profile](./behavior#per-key-profiles-for-morse-tapdance-tap-hold-fine-tuning)

   The tap/hold slots of `MT`, `TH` and `LT` are not limited to plain keycodes — they accept any single action, so you can nest other actions inside them. For example `MT(WM(P, RAlt), LShift, HRM)` taps `RAlt+P` and holds `LShift` with the `HRM` profile, and `TH(WM(A, LShift), MO(2))` taps `Shift+A` and holds momentary-layer 2. Composite tap-hold/morse forms (`MT`/`TH`/`LT`/`TT`/`TD`) cannot be nested inside a slot.

7. For shifted key, use `SHIFTED(key)`

8. For Morse/Tap Dance, use `TD(n)` or `Morse(n)`, they are same

9. For keyboard macros, use `Macro(n)`

## Aliases

The `[aliases]` section contains a table of user defined names and an associated replacement string, which can be used in the `layer.keys`:

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

## Assigning the left/right hand to a position

The optional `<hand>` marker on each `[layout].map` position tells RMK which hand a key belongs to. It is only used when `unilateral_tap = true` (see [behavior](./behavior#per-key-profiles-for-morse-tapdance-tap-hold-fine-tuning)); otherwise it is ignored.

- `L` — left hand
- `R` — right hand
- `*` — bilateral; treated as the opposite hand no matter which hand's modifier was held

The marker is the third element of the position tuple, e.g. `(0, 0, L)`. See the split ortho example above for a full map with hand information filled in.

## Converting from KLE or Vial

If your keyboard already has a [KLE](http://www.keyboard-layout-editor.com/) layout or a [Vial](https://get.vial.today/) definition, you don't have to write the `[layout]` section by hand: `rmkit layout convert` (part of [rmkit](https://github.com/haobogu/rmkit)) converts both. It accepts a raw KLE JSON export ("Download JSON" on keyboard-layout-editor.com) or a `vial.json` (which embeds the same KLE data in `layouts.keymap`), and emits the equivalent `[layout]`:

```bash
rmkit layout convert path/to/vial.json -o layout.toml   # vial.json → [layout]
rmkit layout convert path/to/kle_export.json            # a raw KLE export works too
rmkit layout convert --to-vial keyboard.toml            # reverse: [layout] → vial.json
```

Key positions, cap sizes, split gaps, rotation, ISO/L-shaped caps, encoders, and VIA layout options are all converted (to `map` tokens plus `[layout.shapes]` / `[[layout.variant]]` entries as needed), and the result is round-tripped through RMK's own layout builder before it is printed, so it is guaranteed to build.

Two things to review in the output:

- **Matrix positions** are taken from the VIA-style `row,col` legends. A plain KLE export usually has key labels instead (`Esc`, `Q`, …) — the converter then assigns positions row-major and prints a warning; adjust them to match your `[matrix]` wiring.
- **No keycodes are converted** (KLE and `vial.json` carry none), so author the `[keymap]` yourself: each layer's `keys` follow the map's key order, plus one `["cw", "ccw"]` pair per encoder in `encoders`.

The reverse direction (`--to-vial`) turns a `keyboard.toml`'s `[layout]` into a starting `vial.json` for [Vial support](../features/vial_support). To check the geometry without flashing, render it in the terminal with `rmkit layout show` — it takes a `keyboard.toml`, or a `vial.json` / KLE export directly.
