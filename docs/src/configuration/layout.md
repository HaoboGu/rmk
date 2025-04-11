# Layout


## `[layout]`

`[matrix]` defines the physical key matrix on your board, while `[layout]` defines the logical(software-level) layout(number of rows/cols) and default keymap of your keyboard.

```toml
[layout]
rows = 4
cols = 3
layers = 2
keymap = [
  # Your default keymap here
]
```

The keymap inside is a 3-D array, which represents layer -> row -> key structure of your keymap:

```toml
keymap = [
  # Layer 1
  [
    ["key1", "key2"], # Row 1
    ["key1", "key2"], # Row 2
    ...
  ],
  # Layer 2
  [
    [..], # Row 1
    [..], # Row 2
    ...
  ],
  ...
]
```

The number of rows/cols in default keymap should be identical with what's already defined. [Here](https://github.com/HaoboGu/rmk/blob/main/examples/use_config/stm32h7/keyboard.toml) is an example of keymap definition. 

<div class="warning">
If the number of layer in default keymap is smaller than defined layer number, RMK will fill empty layers automatically. But the empty layers still consumes flash and RAM, so if you don't have a enough space for them, it's not recommended to use a big layer num.
</div>

In each row, some keys are set. Due to the limitation of `toml` file, all keys are strings. RMK would parse the strings and fill them to actual keymap initializer, like what's in [`keymap.rs`](https://github.com/HaoboGu/rmk/tree/main/examples/use_rust/rp2040/src/keymap.rs)

The key string should follow several rules:

1. For a simple keycode(aka keys in RMK's [`KeyCode`](https://docs.rs/rmk/latest/rmk/keycode/enum.KeyCode.html) enum), just fill its name.

    For example, if you set a keycode `"Backspace"`, it will be turned to `KeyCode::Backspace`. So you have to ensure that the keycode string is valid, or RMK wouldn't compile!

    RMK also provides some alias for simpler config, documentation: TODO. 

    For simple keycodes with modifiers active, you can use `WM(key, modifier)` to create a keypress with modifier action. Modifiers can be chained together like `LShift | RGui` to have multiple modifiers active.

2. For no-key (`KeyAction::No`), use `"No"`

3. For transparent key (`KeyAction::Transparent`), use `"_"` or `"__"` (you can put any number of `_`)

4. RMK supports many advanced layer operations:
    1. Use `"DF(n)"` to create a switch default layer actiov, `n` is the layer number
    2. Use `"MO(n)"` to create a layer activate action, `n` is the layer number
    3. Use `"LM(n, modifier)"` to create layer activate with modifier action. The modifier can be chained in the same way as `WM`
    4. Use `"LT(n, key)"` to create a layer activate action or tap key(tap/hold). The `key` here is the RMK [`KeyCode`](https://docs.rs/rmk/latest/rmk/keycode/enum.KeyCode.html)
    5. Use `"OSL(n)"` to create a one-shot layer action, `n` is the layer number
    6. Use `"OSM(modifier)"` to create a one-shot modifier action. The modifier can be chained in the same way as `WM`
    7. Use `"TT(n)"` to create a layer activate or tap toggle action, `n` is the layer number
    8. Use `"TG(n)"` to create a layer toggle action, `n` is the layer number
    9. Use `"TO(n)"` to create a layer toggle only action (activate layer `n` and deactivate all other layers), `n` is the layer number

  The definitions of those operations are same with QMK, you can found [here](https://docs.qmk.fm/#/feature_layers). If you want other actions, please [open an issue](https://github.com/HaoboGu/rmk/issues/new).

5. For modifier-tap-hold, use `MT(key, modifier)` where the modifier can be a chain like explained on point 1. For example for a Home row modifier config you can use `MT(F,LShift)`

6. For generic key tap-hold, use `TH(key-tap, key-hold)`

7. For shifted key, use `SHIFTED(key)`
