# Behavior

The `[behavior]` section contains configuration for how different keyboard actions should behave:

```toml
[behavior]
tri_layer = { upper = 1, lower = 2, adjust = 3 }
one_shot = { timeout = "1s" }
```

## Tap Hold

In the `tap_hold` sub-table you can configure tap-hold behavior which performs one action when tapped and another action when held.

Available fields:

- `enable_hrm`: Enables HRM (Home Row Mod) mode. When enabled, the `prior_idle_time` setting becomes functional. Defaults to `false`.
- `permissive_hold`: Enables permissive hold mode. When enabled, hold action will be triggered when a key is pressed and released during tap-hold decision. This option is recommended to set to true when `enable_hrm` is set to true.
- `unilateral_tap`: (Experimental) Enables unilateral tap mode. When enabled, tap action will be triggered when a key from "same" hand is pressed. In current experimental version, the "opposite" hand is calculated [according to the number of cols/rows](https://github.com/HaoboGu/rmk/blob/c0ef95b1185c25972c62458c878ee9f1a8e1a837/rmk/src/tap_hold.rs#L111-L136). This option is recommended to set to true when `enable_hrm` is set to true.
- `hold_on_other_press`: Enables hold-on-other-key-press mode. When enabled, hold action will be triggered immediately when any other non-tap-hold key is pressed while a tap-hold key is being held. This provides faster modifier activation without waiting for the timeout. **Priority rules**: When HRM is disabled, permissive hold takes precedence over this feature. When HRM is enabled, this feature works normally. Defaults to `false`.
- `prior_idle_time`: If the previous non-modifier key is released within this period before pressing the current tap-hold key, the tap action for the tap-hold behavior will be triggered. This parameter is effective only when enable_hrm is set to `true`. Defaults to 120ms.
- `hold_timeout`: Defines the duration a tap-hold key must be pressed to determine hold behavior. If tap-hold key is released within this time, the key is recognized as a "tap". Holding it beyond this duration triggers the "hold" action. Defaults to 250ms.
- `post_wait_time`: Adds an additional delay after releasing a tap-hold key to check if any keys pressed during the `hold_timeout` are released. This helps accommodate fast typing scenarios where some keys may not be fully released during a hold. Defaults to 50ms

The following are the typical configurations:

```toml
[behavior]
# Enable HRM with all tap-hold features
tap_hold = { enable_hrm = true, permissive_hold = true, unilateral_tap = true, hold_on_other_press = true, prior_idle_time = "120ms", hold_timeout = "250ms" }

# Fast modifiers without HRM
tap_hold = { enable_hrm = false, hold_on_other_press = true, hold_timeout = "200ms" }

# HRM disabled; unspecified fields keep their defaults
tap_hold = { enable_hrm = false, hold_timeout = "200ms" }
```

## Tri Layer

Tri-layer enables a third layer (often called `adjust`) automatically when two other layers(`upper` and `lower`) are both active.

You can enable Tri-Layer by specifying the `upper`, `lower` and `adjust` layers in the `tri_layer` sub-table:

```toml
[behavior.tri_layer]
upper = 1
lower = 2
adjust = 3
```

In this example, when both layers 1 (`upper`) and 2 (`lower`) are active, layer 3 (`adjust`) will also be enabled.

Note that `"#layer_name"` could also be used in place of layer numbers.

## One Shot
The `one_shot` sub-table configures one-shot modifiers or one-shot layers (OSM/OSL). Use `timeout` to specify how long the modifier/layer remains active. The value is a string suffixed with `s` or `ms` (default: `1s`).

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

## Macro

In the `macro` sub-table, you can configure the keyboard's macro functionality. Macros are explained in more detail in the [keyboard macros](/docs/features/keymap/keyboard_macros.md) page.

Macro operations are defined with an `operation` and a `keycode`, `duration` or `text` field depending on the operation. Available operations are:

```toml
[[behavior.macro.macros]]
operations = [
  { operation = "down", keycode = "_" }, # [!code focus:5]
  { operation = "up", keycode = "_" },
  { operation = "tap", keycode = "_" },
  { operation = "delay", duration = "0ms" },
  { operation = "text", text = "foo" }
]
```

```toml
# Outputs "Hello"
[[behavior.macro.macros]]
operations = [
    { operation = "text", text = "Hello" }
]

# Outputs "Hello" with a 1 second delay after the first letter
[[behavior.macro.macros]]
operations = [
    { operation = "down", keycode = "LShift" },
    { operation = "tap", keycode = "H" },
    { operation = "up", keycode = "LShift" },
    { operation = "delay", duration = "1s" },
    { operation = "tap", keycode = "E" },
    { operation = "tap", keycode = "L" },
    { operation = "tap", keycode = "L" },
    { operation = "tap", keycode = "O" },
]
```

## Morse(Tap Dance)

In the `morse` sub-table, you can configure the keyboard's morse functionality. Morse is a superset of the well-known [tap dance](https://docs.qmk.fm/features/tap_dance), enabling you to assign different actions to various combinations of taps and holds performed within a specific time window.

Morse keys are defined as a list under the `[behavior.morse]` section:

```toml
[behavior.morse]
morses = [
  # ... morse entries ...
]
```

RMK provides three methods for defining a Morse key.

### Define a morse key

#### 1. Vial-style Tap Dance

This method is fully compatible with Vial's Tap Dance, it defines four specific actions:

1. `tap`: The action to be triggered on the first tap. This is the default action when the key is tapped once.
2. `hold`: The action to be triggered when the key is held down (not tapped) beyond the tapping term.
3. `hold_after_tap`: The action to be triggered when the key is held down after being tapped once.
4. `double_tap`: The action to be triggered when the key is tapped twice within the tapping term.

Example:

```toml
[behavior.morse]
morses = [
  # A Vial-style tap dance key
  { tap = "F1", hold = "MO(1)", hold_after_tap = "MO(2)", double_tap = "F2" }
]
```

#### 2. Tap and Hold Arrays

This is an extended version of tap dance. It allows you to define sequences of actions for multiple taps and for holds that occur after a specific number of taps.

- `tap_actions`: An array of actions triggered by sequential taps. Each tap within the tapping term increments the tap count and triggers the corresponding action from the `tap_actions` array. For example, `tap_actions = ["F1", "F2", "F3"]` means a single tap triggers "F1", double tap triggers "F2", triple tap triggers "F3", and so on. If the tap count exceeds the length of the array, the last action is used.
- `hold_actions`: An array of actions triggered when the key is held *after* a certain number of taps. When a key is held after multiple taps, the corresponding action from the `hold_actions` array is triggered. For example, `hold_actions = ["MO(1)", "MO(2)", "MO(3)"]` means holding after one tap triggers "MO(1)", holding after two taps triggers "MO(2)", and so on.

Example:

```toml
[behavior.morse]
morses = [
  # A morse key defined with tap and hold-after-tap actions array
  { tap_actions = ["F1", "F2", "F3", "F4", "F5"], hold_actions = ["MO(1)", "MO(2)", "MO(3)", "MO(4)", "MO(5)"] }
]
```

#### 3. Full Morse Patterns

This is the most powerful method, allowing you to define actions based on [Morse code](https://en.wikipedia.org/wiki/Morse_code)-like patterns of taps and holds. This lets you assign a large number of actions to a single key

- `morse_actions`: A list of pattern-to-action mappings. The pattern is a tap/hold sequence, a tap is represented by a `.` or `0`, a hold is represented by a `_`, `-` or `1`. For example, the morse pattern of `C` can be described like this: `"-.-."` or `"_._."` or `"1010"`. The maximum length of the pattern is 15.

Example:

```toml
[behavior.morse]
morses = [
  # A morse key defined using full Morse patterns
  { morse_actions = [
        { pattern = ".-", action = "A" },
        { pattern = "-...", action = "B" },
        { pattern = "-.-.", action = "C" },
        { pattern = "-..", action = "D" },
    ] },
]
```

::: warning

The three definition methods are mutually exclusive. For any single Morse key definition, you must choose only one of the following approaches:

- Full Morse: `morse_actions`
- Tap and Hold Arrays: `tap_actions` and/or `hold_actions`
- Vial-style: `tap`, `hold`, `hold_after_tap`, `double_tap`.

Mixing fields from different methods in the same definition is not allowed.

:::

### Common configuration

The following setting applies to all three definition methods:

  - `timeout`: The time window (in milliseconds or seconds) within which taps are considered part of the same morse sequence. Defaults to 200ms if not specified.

### Global Configuration Limits

The following parameters in the `[rmk]` section control the resource allocation for the Morse feature:

- `morse_max_num`: The maximum number of Morse key you can create. (Default: 8, Range: 0-256)
- `max_patterns_per_key`: The maximum number of individual patterns (like ".-") or actions that a single Morse key can contain. (Default: 8, Range: 4-65536)

```toml
[rmk]
morse_max_num = 10  # To support up to 10 morse keys
max_patterns_per_key = 36  # To support up to 36 morse patterns per morse key
```

Note that the Vial-style method (using `tap`, `hold`, `hold_after_tap`, `double_tap`) needs at least 4 patterns. If you create a key with a long `tap_actions`/`hold_actions` array or many `morse_actions`, you might need to increase `max_patterns_per_key` accordingly.

::: warning Vial Compatibility
Please note that while the firmware can handle all Morse configurations, Vial can only recognize and edit the four basic Vial-style actions. These correspond to the patterns for single tap (.), hold (-), double tap (..), and hold-after-tap (.-). More complex patterns defined using morse_actions or extended tap_actions will not be visible or editable in Vial.
:::

### Comprehensive Example

Here is a comprehensive example of morse configuration:

```toml
[rmk]
# Maximum number of morses keyboard can store (max 256)
morse_max_num = 9
# Maximum number of patterns a morse key can handle
max_patterns_per_key = 36

[behavior.morse]
morses = [
  # td(0): Function key that outputs F1 on tap, F2 on double tap, layer 1 on hold
  { tap = "F1", hold = "MO(1)", double_tap = "F2" },
  
  # td(1): Modifier key that outputs Ctrl on tap, Alt on double tap, Shift on hold
  { tap = "LCtrl", hold = "LShift", double_tap = "LAlt" },
  
  # td(2): Navigation key that outputs Tab on tap, Escape on double tap, layer 2 on hold
  { tap = "Tab", hold = "MO(2)", double_tap = "Escape", timeout = "250ms" },
  
  # td(3): Extended morse for function keys
  { tap_actions = ["F1", "F2", "F3", "F4", "F5"], hold_actions = ["MO(1)", "MO(2)", "MO(3)", "MO(4)", "MO(5)"], timeout = "300ms" }

  # td(4): the morse ABC
  { timeout = "250ms", morse_actions = [
      { pattern = ".-", action = "A" }, 
      { pattern = "-...", action = "B" }, 
      { pattern = "-.-.", action = "C" }, 
      { pattern = "-..", action = "D" }, 
      { pattern = ".", action = "E" }, 
      { pattern = "..-.", action = "F" }, 
      { pattern = "--.", action = "G" }, 
      { pattern = "....", action = "H" }, 
      { pattern = "..", action = "I" }, 
      { pattern = ".---", action = "J" }, 
      { pattern = "-.-", action = "K" }, 
      { pattern = ".-..", action = "L" }, 
      { pattern = "--", action = "M" }, 
      { pattern = "-.", action = "N" }, 
      { pattern = "---", action = "O"}, 
      { pattern = ".--.", action = "P" }, 
      { pattern = "--.-", action = "Q" }, 
      { pattern = ".-.", action = "R" }, 
      { pattern = "...", action = "S" }, 
      { pattern = "-", action = "T" }, 
      { pattern = "..-", action = "U" }, 
      { pattern = "...-", action = "V" }, 
      { pattern = ".--", action = "W" }, 
      { pattern = "-..-", action = "X" }, 
      { pattern = "-.--", action = "Y" }, 
      { pattern = "--..", action = "Z" }, 
      { pattern = ".----", action = "Kc1" }, 
      { pattern = "..---", action = "Kc2" }, 
      { pattern = "...--", action = "Kc3" }, 
      { pattern = "....-", action = "Kc4" }, 
      { pattern = ".....", action = "Kc5" }, 
      { pattern = "-....", action = "Kc6" }, 
      { pattern = "--...", action = "Kc7" }, 
      { pattern = "---..", action = "Kc8" }, 
      { pattern = "----.", action = "Kc9" }, 
      { pattern = "-----", action = "Kc0" }
    ] }
]
```

### Using Morse(Tap Dance) in Keymaps

You can use both `Morse` and `TD` to represent a morse key in your keymap, you can reference it by its index (starting from 0):

```toml
[layout]
rows = 4
cols = 3
layers = 2
keymap = [
    [
        ["A", "B", "C"], 
        ["TD(0)", "TD(1)", "TD(2)"],  # Use morse dances 0, 1, and 2
        ["LCtrl", "MO(1)", "LShift"],
        ["OSL(1)", "LT(2, Kc9)", "LM(1, LShift | LGui)"]
    ],
    [
        ["_", "TT(1)", "TG(2)"],
        ["_", "_", "_"],
        ["_", "_", "_"],
        ["_", "_", "_"]
    ],
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
