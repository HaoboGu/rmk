# Special Keys

RMK maps all [keys](https://docs.rs/rmk/latest/rmk/keycode/index.html) QMK does. However, at the time of writing, not all features are supported.

The following keys are supported (some further keys might work, but are not documented).

## Repeat/Again key

[Similar to QMK](https://docs.qmk.fm/features/repeat_key), pressing this key repeats the last key pressed. Note that QMK binds this function to `Kc_RepeatKey`, while RMK binds it to `Kc_Again`. This ensures better compatibility with Vial, which features the `Again` key as a dedicated key (unlike the `RepeatKey`, which doesn't exist in Vial). Although some old keyboards might have a key for `Again`, it is not used in modern operating systems anymore.

In QMK an `AlternativeRepeatKey` is supported. This functionality is not implemented in RMK.

## Caps Word

RMK includes `CapsWordToggle`. It can be aliased with any of `caps_word` or `cword` in a keymap. Caps word capitalizes all characters until a breaking character such as space occurs.

## LatchTap

`LatchTap(modifier, key)` is a keymap action that **latches** a modifier for the lifetime of the current layer and **taps a key under it on each press**.
It is designed for Alt/Ctrl/Gui-Tab style window switching, where you want to hold a modifier across several taps of a key without holding the modifier key yourself.

Behavior:

1. **First press**: the modifier is latched (engaged and kept active) and `key` is sent together with it. For example `LatchTap(LAlt, Tab)` sends `Alt+Tab`.
2. **Release**: only `key` is released; the modifier stays latched. So after releasing you are left with `LAlt` still held.
3. **Subsequent presses**: `key` is tapped again while the modifier remains engaged (e.g. `Tab` cycles through windows, `Alt` stays down).
4. **Layer exit**: the latched modifier is released automatically when the layer it was engaged on is deactivated (for example when the momentary layer key `MO(n)` is released).

This differs from the related action:

- Unlike `LM(layer, modifier)`, the modifier is **not** bound to the layer-switch key. It is bound to this dedicated key, so you can place several independent `LatchTap` keys (with different modifiers/keys) on the **same** layer.

`LatchTap` cooperates with other modifiers: its latched modifier is combined into the same HID report as held modifier keys, one-shot modifiers, and `WM`/`SHIFTED` keys.
For example, if `LatchTap(LCtrl, Tab)` has latched `Ctrl` and you then activate `OSM(LShift)`, the next `LatchTap(LCtrl, Tab)` press reports `Ctrl+Shift+Tab`.

Syntax: `LatchTap(modifier, key)` — the modifier comes first, the key second (matching the modifier-first order).
The modifier accepts the same names as other actions (`LShift`, `LCtrl`, `LAlt`, `LGui`, `RShift`, `RCtrl`, `RAlt`, `RGui`), optionally combined with `|`.

Example layout — a layer with three independent latching window-switch keys:

```toml
[[layer]]
keys = """
LatchTap(LCtrl, Tab)   LatchTap(LAlt, Tab)   LatchTap(LGui, Tab)
"""
```

Typical usage: put `MO(n)` on a thumb key to enter the layer above, hold it, then tap `LatchTap(LAlt, Tab)` repeatedly to Alt-Tab through windows. Releasing `MO(n)` releases the latched `Alt`.

::: note Rust API / Vial
In a Rust keymap use the `latchtap!` macro, e.g. `latchtap!(ModifierCombination::LCTRL, Tab)`. `LatchTap` is not yet representable as a Vial keycode, so it can only be configured via `keyboard.toml` or the Rust API.
:::
