# Behavior

The `[behavior]` section contains configuration for how different keyboard actions should behave:

```toml
[behavior]
tri_layer = { upper = 1, lower = 2, adjust = 3 }
one_shot = { timeout = "1s" }
```

## Tap Hold

In the `tap_hold` sub-table, you can configure the following parameters:

- `enable_hrm`: Enables or disables HRM (Home Row Mod) mode. When enabled, the `prior_idle_time` setting becomes functional. Defaults to `false`.
- `permissive_hold`: Enables permissive hold mode. When enabled, hold action will be triggered when a key is pressed and released during tap-hold decision. This option is recommended to set to true when `enable_hrm` is set to true.
- `chordal_hold`: (Experimental) Enables chordal hold mode. When enabled, hold action will be triggered when a key from "opposite" hand is pressed. In current experimental version, the "opposite" hand is calculated [according to the number of cols/rows](https://github.com/HaoboGu/rmk/blob/c0ef95b1185c25972c62458c878ee9f1a8e1a837/rmk/src/tap_hold.rs#L111-L136). This option is recommended to set to true when `enable_hrm` is set to true.
- `prior_idle_time`: If the previous non-modifier key is released within this period before pressing the current tap-hold key, the tap action for the tap-hold behavior will be triggered. This parameter is effective only when enable_hrm is set to `true`. Defaults to 120ms.
- `hold_timeout`: Defines the duration a tap-hold key must be pressed to determine hold behavior. If tap-hold key is released within this time, the key is recognized as a "tap". Holding it beyond this duration triggers the "hold" action. Defaults to 250ms.
- `post_wait_time`: Adds an additional delay after releasing a tap-hold key to check if any keys pressed during the `hold_timeout` are released. This helps accommodate fast typing scenarios where some keys may not be fully released during a hold. Defaults to 50ms

The following are the typical configurations:

```toml
[behavior]
# Enable HRM
tap_hold = { enable_hrm = true, permissive_hold = true, chordal_hold = true, prior_idle_time = "120ms", hold_timeout = "250ms" }
# Disable HRM, you can safely ignore any fields if you don't want to change them
tap_hold = { enable_hrm = false, hold_timeout = "200ms" }
```

## Tri Layer

`Tri Layer` works by enabling a layer (called `adjust`) when other two layers (`upper` and `lower`) are both enabled.

You can enable Tri Layer by specifying the `upper`, `lower` and `adjust` layers in the `tri_layer` sub-table:

```toml
[behavior.tri_layer]
upper = 1
lower = 2
adjust = 3
```

In this example, when both layers 1 (`upper`) and 2 (`lower`) are active, layer 3 (`adjust`) will also be enabled.

Note that `"#layer_name"` could also be used in place of layer numbers.

## One Shot

In the `one_shot` sub-table you can define how long OSM or OSL will wait before releasing the modifier/layer with the `timeout` option, default is one second. `timeout` is a string with a suffix of either "s" or "ms".

```toml
[behavior.one_shot]
timeout = "5s"
```

## Combo

In the `combo` sub-table, you can configure the keyboard's combo key functionality. Combo allows you to define a group of keys that, when pressed simultaneously, will trigger a specific output action.

Combo configuration includes the following parameters:

- `timeout`: Defines the maximum time window for pressing all combo keys. If the time exceeds this, the combo key will not be triggered. The format is a string, which can be milliseconds (e.g. "200ms") or seconds (e.g. "1s").
- `combos`: An array containing all defined combos. Each combo configuration is an object containing the following attributes:
  - `actions`: An array of strings defining the keys that need to be pressed simultaneously to trigger the combo action.
  - `output`: A string defining the output action to be triggered when all keys in `actions` are pressed simultaneously.
  - `layer`: An optional parameter, a number, specifying which layer the combo is valid on. If not specified, the combo is valid on all layers.

Here is an example of combo configuration:

```toml
[behavior.combo]
timeout = "150ms"
combos = [
  # Press J and K keys simultaneously to output Escape key
  { actions = ["J", "K"], output = "Escape" },
  # Press F and D keys simultaneously to output Tab key, but only valid on layer 0
  { actions = ["F", "D"], output = "Tab", layer = 0 },
  # Three-key combo, press A, S, and D keys to switch to layer 2
  { actions = ["A", "S", "D"], output = "TO(2)" }
]
```

## Fork

In the `fork` sub-table, you can configure the keyboard's state based key fork functionality. Forks allows you to define a trigger key and condition dependent possible replacement keys. When the trigger key is pressed, the condition is checked by the following rule: If any of the `match_any` states are active AND none of the `match_none` states active, the trigger key will be replaced with positive_output, otherwise with the negative_output. By default the modifiers listed in `match_any` will be suppressed (even the one-shot modifiers) for the time the replacement key action is executed. However, with `kept_modifiers` some of them can be kept instead of automatic suppression.

Fork configuration includes the following parameters:

- `forks`: An array containing all defined forks. Each fork configuration is an object containing the following attributes:
  - `trigger`: Defines the triggering key.
  - `negative_output`: A string defining the output action to be triggered when the conditions are not met
  - `positive_output`: A string defining the output action to be triggered when the conditions are met
  - `match_any`: A strings defining a combination of modifier keys, lock leds, mouse buttons (optional)
  - `match_none`: A strings defining a combination of modifier keys, lock leds, mouse buttons (optional)
  - `kept_modifiers`: A strings defining a combination of modifier keys, which should not be 'suppressed' form the keyboard state for the time the replacement action is executed. (optional)
  - `bindable`: Enables the evaluation of not yet triggered forks on the output of this fork to further manipulate the output. Advanced use cases can be solved using this option. (optional)

For `match_any`, `match_none` the legal values are listed below (many values may be combined with "|"):

- `LShift`, `LCtrl`, `LAlt`, `LGui`, `RShift`, `RCtrl`, `RAlt`, `RGui` (these are including the effect of explicitly held and one-shot modifiers too)
- `CapsLock`, `ScrollLock`, `NumLock`, `Compose`, `Kana`
- `MouseBtn1` .. `MouseBtn8`

Here is a sample of fork configuration with random examples:

```toml
[behavior.fork]
forks = [
  # Shift + '.' output ':' key
  { trigger = "Dot", negative_output = "Dot", positive_output = "WM(Semicolon, LShift)", match_any = "LShift|RShift" },

  # Shift + ',' output ';' key but only if no Alt is pressed
  { trigger = "Comma", negative_output = "Comma", positive_output = "Semicolon", match_any = "LShift|RShift", match_none = "LAlt|RAlt" },

  # left bracket outputs by default '{', with shifts pressed outputs '['
  { trigger = "LeftBracket", negative_output = "WM(LeftBracket, LShift)", positive_output = "LeftBracket", match_any = "LShift|RShift" },

  # Flip the effect of shift on 'x'/'X'
  { trigger = "X", negative_output = "WM(X, LShift)", positive_output = "X", match_any = "LShift|RShift" },

  # F24 usually outputs 'a', except when Left Shift or Ctrl pressed, in that case triggers a macro
  { trigger = "F24", negative_output = "A", positive_output = "Macro1", match_any = "LShift|LCtrl" },

  # Swap Z and Y keys if MouseBtn1 is pressed (on the keyboard) (Note that these must not be bindable to avoid infinite fork loops!)
  { trigger = "Y", negative_output = "Y", positive_output = "Z", match_any = "MouseBtn1", bindable = false },
  { trigger = "Z", negative_output = "Z", positive_output = "Y", match_any = "MouseBtn1", bindable = false },

  # Shift + Backspace output Delete key (inside a layer tap/hold)
  { trigger = "LT(2, Backspace)", negative_output = "LT(2, Backspace)", positive_output = "LT(2, Delete)", match_any = "LShift|RShift" },

  # Ctrl + play/pause will send next track. MediaPlayPause -> MediaNextTrack
  # Ctrl + Shift + play/pause will send previous track. MediaPlayPause -> MediaPrevTrack
  # Alt + play/pause will send volume up. MediaPlayPause -> AudioVolUp
  # Alt + Shift + play/pause will send volume down. MediaPlayPause -> AudioVolDown
  # Ctrl + Alt + play/pause will send brightness up. MediaPlayPause -> BrightnessUp
  # Ctrl + Alt + Shift + play/pause will send brightness down. MediaPlayPause -> BrightnessDown
  # ( Note that the trigger and immediate trigger keys of the fork chain could be 'virtual keys',
  #   which will never output, like F23, but here multiple overrides demonstrated.)
    { trigger = "MediaPlayPause", negative_output = "MediaPlayPause", positive_output = "MediaNextTrack", match_any = "LCtrl|RCtrl", bindable = true },
  { trigger = "MediaNextTrack", negative_output = "MediaNextTrack", positive_output = "BrightnessUp", match_any = "LAlt|RAlt", bindable = true },
  { trigger = "BrightnessUp", negative_output = "BrightnessUp", positive_output = "BrightnessDown", match_any = "LShift|RShift", bindable = false },
  { trigger = "MediaNextTrack", negative_output = "MediaNextTrack", positive_output = "MediaPrevTrack", match_any = "LShift|RShift", match_none = "LAlt|RAlt", bindable = false},
  { trigger = "MediaPlayPause", negative_output = "MediaPlayPause", positive_output = "AudioVolUp", match_any = "LAlt|RAlt", match_none = "LCtrl|RCtrl", bindable = true },
  { trigger = "AudioVolUp", negative_output = "AudioVolUp", positive_output = "AudioVolDown", match_any = "LShift|RShift", match_none = "LCtrl|RCtrl", bindable = false }
]
```

Please note that the processing of forks happen after combos and before others, so the trigger key must be the one listed in your keymap (or combo output). For example if `LT(2, Backspace)` is in your keymap, then `trigger = "Backspace"` will NOT work, you should "replace" the full key and use `trigger = "LT(2, Backspace)"` instead, like in the example above. You may want to include `F24` or similar dummy keys in your keymap, and use them as trigger for your pre-configured forks, such as Shift/CapsLock dependent macros to enter unicode characters of your language.

Vial does not support fork configuration yet.
