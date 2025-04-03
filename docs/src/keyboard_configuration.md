# Configuration

RMK provides an easy and accessible way to set up the keyboard with a toml config file, even without Rust code!

## Usage 

A `toml` file named `keyboard.toml` is used as a configuration file. The following is the spec of `toml` if you're unfamiliar with toml:
  - [English](https://toml.io/en/v1.0.0) / [中文](https://toml.io/cn/v1.0.0)

RMK provides a proc-macro to load the `keyboard.toml` at your projects root: `#[rmk_keyboard]`, add it to your `main.rs` like:

```rust
use rmk::macros::rmk_keyboard;

#[rmk_keyboard]
mod my_keyboard {}
```

And, that's it! `#[rmk_keyboard]` macro would load your `keyboard.toml` config and create everything that's needed for creating a RMK keyboard instance.

If you don't want any other customizations beyond the `keyboard.toml`, `#[rmk_keyboard]` macro will just work. For the full examples, please check the [`example/use_config`](https://github.com/HaoboGu/rmk/tree/main/examples/use_config) folder.

## What's in the config file?

The config file contains almost EVERYTHING to customize a keyboard. For the full reference of `keyboard.toml`, please refer to [this](#keyboardtoml). Also, we have pre-defined default configurations for chips, at [`rmk-macro/src/default_config`](https://github.com/HaoboGu/rmk/blob/main/rmk-macro/src/default_config) folder. We're going to add default configurations for more chips, contributions are welcome!

The following is the introduction of each section:

### `[keyboard]`

`[keyboard]` section contains basic information of the keyboard, such as keyboard's name, chip, etc:

```toml
[keyboard]
name = "RMK Keyboard"
vendor_id = 0x4c4b
product_id = 0x4643
manufacturer = "RMK"
chip = "stm32h7b0vb"
# If your chip doesn't have a functional USB peripheral, for example, nRF52832/esp32c3(esp32c3 has only USB serial, not full functional USB), set `usb_enable` to false
usb_enable = true
```

### `[matrix]`

`[matrix]` section defines the [key matrix](https://docs.qmk.fm/how_a_matrix_works) information of the keyboard, aka input/output pins. 

<div class="warning">
For split keyboard, this section should be just ignored, the matrix IO pins for split keyboard are defined in `[split]` section.
</div>

In order to identify the IO pins take a look at your keyboard's schematic: The pin going to the [diode](https://en.wikipedia.org/wiki/Diode) (called anode) is an output pin, the pin coming out (called cathode) is an input pin:
```
output_pin =>   >|   => input_pin
                 ↑
              diode(be aware of it's direction)
```
<div class="warning">
Per default RMK assumes that your pins are `col2row`, meaning that the output pins (anodes) represent the columns and the input pins (cathodes) represent the rows. If your schemata shows the opposite you need to [change the configuration to `row2col`](https://haobogu.github.io/rmk/faq.html#my-matrix-is-row2col-the-matrix-doesnt-work)
</div>

IO pins are represented with an array of string, the string value should be the **GPIO peripheral name** of the chip. For example, if you're using stm32h750xb, you can go to <https://docs.embassy.dev/embassy-stm32/git/stm32h750xb/peripherals/index.html> to get the valid GPIO peripheral name:

![gpio_peripheral_name](images/gpio_peripheral_name.png)

The GPIO peripheral name varies for different chips. For example, RP2040 has `PIN_0`, nRF52840 has `P0_00` and stm32 has `PA0`. So it's recommended to check the embassy's doc for your chip to get the valid GPIO name first.

Here is an example toml of `[matrix]` section for stm32:

```toml
[matrix]
# Input and output pins are mandatory
input_pins = ["PD4", "PD5", "PD6", "PD3"]
output_pins = ["PD7", "PD8", "PD9"]
# WARNING: Currently row2col/col2row is set in RMK's feature gate, row2col config here is valid ONLY when you're using cloud compilation
# row2col = true
```

If your keys are directly connected to the microcontroller pins, set `matrix_type` to `direct_pin`. (The default value for `matrix_type` is `normal`)

`direct_pins` is a two-dimensional array that represents the physical layout of your keys.

If your pin requires a pull-up resistor and the button press pulls the pin low, set `direct_pin_low_active` to true. Conversely, set it to false if your pin requires a pull-down resistor and the button press pulls the pin high.

Currently, col2row is used as the default matrix type. If you want to use row2col matrix, you should edit your `Cargo.toml`, disable the default feature as the following:

```toml
# Cargo.toml
rmk = { version = "0.4", default-features = false, features = ["nrf52840_ble"] }
```

Here is an example for rp2040.
```toml
matrix_type = "direct_pin"
direct_pins = [
    ["PIN_0", "PIN_1", "PIN_2"],
    ["PIN_3", "_", "PIN_5"]
]
# `direct_pin_low_active` is optional. Default is `true`.
direct_pin_low_active = true
```

### `[layout]`

`[layout]` section contains the layout and the default keymap for the keyboard:

```toml
[layout]
rows = 4
cols = 3
layers = 2
keymap = [
  # Your default keymap here
]
```

The keymap inside is a 2-D array, which represents layer -> row -> key structure of your keymap:

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

  The definitions of those operations are same with QMK, you can found [here](https://docs.qmk.fm/#/feature_layers). If you want other actions, please [fire an issue](https://github.com/HaoboGu/rmk/issues/new).

5. For modifier-tap-hold, use `MT(key, modifier)` where the modifier can be a chain like explained on point 1. For example for a Home row modifier config you can use `MT(F,LShift)`

6. For generic key tap-hold, use `TH(key-tap, key-hold)`

7. For shifted key, use `SHIFTED(key)`

### `[behavior]`

`[behavior]` section contains configuration for how different keyboard actions should behave:

```toml
[behavior]
tri_layer = { uppper = 1, lower = 2, adjust = 3 }
one_shot = { timeout = "1s" }
```

#### Tri Layer

`Tri Layer` works by enabling a layer (called `adjust`) when other two layers (`upper` and `lower`) are both enabled.

You can enable Tri Layer by specifying the `upper`, `lower` and `adjust` layers in the `tri_layer` sub-table:

```toml
[behavior.tri_layer]
uppper = 1
lower = 2
adjust = 3
```
In this example, when both layers 1 (`upper`) and 2 (`lower`) are active, layer 3 (`adjust`) will also be enabled.

#### Tap Hold

In the `tap_hold` sub-table, you can configure the following parameters:

- `enable_hrm`: Enables or disables HRM (Home Row Mod) mode. When enabled, the `prior_idle_time` setting becomes functional. Defaults to `false`.
- `prior_idle_time`: If the previous non-modifier key is released within this period before pressing the current tap-hold key, the tap action for the tap-hold behavior will be triggered. This parameter is effective only when enable_hrm is set to `true`. Defaults to 120ms.
- `hold_timeout`: Defines the duration a tap-hold key must be pressed to determine hold behavior. If tap-hold key is released within this time, the key is recognized as a "tap". Holding it beyond this duration triggers the "hold" action. Defaults to 250ms.
- `post_wait_time`: Adds an additional delay after releasing a tap-hold key to check if any keys pressed during the `hold_timeout` are released. This helps accommodate fast typing scenarios where some keys may not be fully released during a hold. Defaults to 50ms

The following are the typical configurations:

```toml
[behavior]
# Enable HRM 
tap_hold = { enable_hrm = true, prior_idle_time = "120ms", hold_timeout = "250ms", post_wait_time = "50ms"}
# Disable HRM, you can safely ignore any fields if you don't want to change them
tap_hold = { enable_hrm = false, hold_timeout = "200ms" }
```

#### One Shot

In the `one_shot` sub-table you can define how long OSM or OSL will wait before releasing the modifier/layer with the `timeout` option, default is one second.
`timeout` is a string with a suffix of either "s" or "ms".

```toml
[behavior.one_shot]
timeout = "5s"
```

#### Combo

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

#### Fork

In the `fork` sub-table, you can configure the keyboard's state based key fork functionality. Forks allows you to define a trigger key and condition dependent possible replacement keys. When the trigger key is pressed, the condition is checked by the following rule:
If any of the `match_any` states are active AND none of the `match_none` states active, the trigger key will be replaced with positive_output, otherwise with the negative_output. By default the modifiers listed in `match_any` will be suppressed (even the one-shot modifiers) for the time the replacement key action is executed. However with `kept_modifiers` some of them can be kept instead of automatic suppression.

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
  { trigger = "LT(2,Backspace)", negative_output = "LT(2,Backspace)", positive_output = "LT(2,Delete)", match_any = "LShift|RShift" },

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

Please note that the processing of forks happen after combos and before others, so the trigger key must be the one listed in your keymap (or combo output).
For example if `LT(2,Backspace)` is in your keymap, then trigger = `Backspace` will NOT work, you should "replace" the full key and use `trigger = "LT(2,Backspace)` instead, like in the last example above.
You may want to include `F24` or similar dummy keys in your keymap, and use them as trigger for your pre-configured forks, such as Shift/CapsLock dependent macros to enter unicode characters of your language.

Vial does not support fork configuration yet.

### `[light]`

`[light]` section defines lights of the keyboard, aka `capslock`, `scrolllock` and `numslock`. They are actually an input pin, so there are two fields available: `pin` and `low_active`.

`pin` field is just like IO pins in `[matrix]`, `low_active` defines whether the light low-active or high-active(`true` means low-active).

You can safely ignore any of them, or the whole `[light]` section if you don't need them.

```toml
[light]
capslock = { pin = "PIN_0", low_active = true }
scrolllock = { pin = "PIN_1", low_active = true }
numslock= { pin = "PIN_2", low_active = true }
```

### `[storage]`

`[storage]` section defines storage related configs. Storage feature is required to persist keymap data, it's strongly recommended to make it enabled(and it's enabled by default!). RMK will automatically use the last two section of chip's internal flash as the pre-served storage space. For some chips, there's also predefined default configuration, such as [nRF52840](https://github.com/HaoboGu/rmk/blob/main/rmk-macro/src/default_config/nrf52840.rs). If you don't want to change the default setting, just ignore this section.
```toml
[storage]
# Storage feature is enabled by default
enabled = true
# Start address of local storage, MUST BE start of a sector.
# If start_addr is set to 0(this is the default value), the last `num_sectors` sectors will be used.
start_addr = 0x00000000
# How many sectors are used for storage, the default value is 2
num_sectors = 2
```

### `[ble]`

To enable BLE, add `enabled = true` under the `[ble]` section. 

There are several more configs for reading battery level and charging state, now they are available for nRF52840 only.

```toml
# Ble configuration
# To use the default configuration, ignore this section completely
[ble]
# Whether to enable BLE feature
enabled = true
# nRF52840's saadc pin for reading battery level, you can use a pin number or "vddh"
battery_adc_pin = "vddh"
# The voltage divider setting for saadc. 
# For example, nice!nano have 806 + 2M resistors, the saadc measures voltage on 2M resistor, so the two values should be set to 2000 and 2806
adc_divider_measured = 2000
adc_divider_total = 2806
# Pin that reads battery's charging state, `low-active` means the battery is charging when `charge_state.pin` is low
charge_state = { pin = "PIN_1", low_active = true }
# Output LED pin that blinks when the battery is low
charge_led= { pin = "PIN_2", low_active = true }
```

<!-- ## More customization

`#[rmk_keyboard]` macro also provides some flexibilities of customizing the keyboard's behavior. For example, the clock config:

```rust
#[rmk]
mod MyKeyboard {
  use embassy_stm32::Config;

  #[config]
  fn config() -> Config {
    let mut config = Config::default();
    {
        use embassy_stm32::rcc::*;
        config.rcc.hsi = Some(HSIPrescaler::DIV1);
        // ... other rcc configs below
    }
    config
  }
}
```

RMK should use the config from the user defined function to initialize the singleton of chip peripheral, for stm32, you can assume that it's initialized using `let p = embassy_stm32::init(config);`. -->


## Appendix

### `keyboard.toml`

The following toml contains all available settings in `keyboard.toml`

```toml
# Basic info of the keyboard
[keyboard]
name = "RMK Keyboard" # Keyboard name
product_name = "RMK Keyboard" # Display name of this keyboard
vendor_id = 0x4c4b
product_id = 0x4643
manufacturer = "haobo"
serial_number = "vial:f64c2b3c:000001"
# The chip or existing board used in keyboard
# Either \"board\" or \"chip\" can be set, but not both
chip = "rp2040" 
board = "nice!nano_v2"
# USB is enabled by default for most chips
# Set to false if you don't want USB
usb_enable = true

# Set matrix IO for the board. This section is for non-split keyboard and is conflict with [split] section
[matrix]
# `matrix_type` is optional. Default is "normal"
matrix_type = "normal"
# Input and output pins
input_pins = ["PIN_6", "PIN_7", "PIN_8", "PIN_9"]
output_pins = ["PIN_19", "PIN_20", "PIN_21"]
# WARNING: Currently row2col/col2row is set in RMK's feature gate, row2col config here is valid ONLY when you're using cloud compilation

# Direct Pin Matrix is a Matrix of buttons connected directly to pins. It conflicts with the above.
matrix_type = "direct_pin"
direct_pins = [
    ["PIN_0", "PIN_1", "PIN_2"],
    ["PIN_3", "_", "PIN_5"]
]

# `direct_pin_low_active` is optional. Default is `true`.
# If your pin needs to be pulled up and the pin is pulled down when the button is turned on, please set it to true
# WARNING: If you use a normal matrix, it will be ineffective
direct_pin_low_active = true

# Layout info for the keyboard, this section is mandatory
[layout]
# Number of rows. For split keyboard, this is the total rows contains all splits
rows = 4
# Number of cols. For split keyboard, this is the total cols contains all splits
cols = 3
# Number of layers. Be careful, since large layer number takes more flash and RAM
layers = 2
# Default keymap definition, the size should be consist with rows/cols
# Empty layers will be used to fill if the number of layers set in default keymap is less than `layers` setting
keymap = [
    [
        ["A", "B", "C"],
        ["Kc1", "Kc2", "Kc3"],
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

# Behavior configuration, if you don't want to customize anything, just ignore this section
[behavior]
# Tri Layer configuration
tri_layer = { uppper = 1, lower = 2, adjust = 3 }
# One Shot configuration
one_shot = { timeout = "1s" }

# Lighting configuration, if you don't have any light, just ignore this section.
[light]
# LED pins, capslock, scrolllock, numslock. You can safely ignore any of them if you don't have
capslock = { pin = "PIN_0", low_active = true }
scrolllock = { pin = "PIN_1", low_active = true }
numslock= { pin = "PIN_2", low_active = true }

# Storage configuration.
# To use the default configuration, ignore this section completely
[storage]
# Whether the storage is enabled
enabled = true
# The start address of storage
start_addr = 0x60000
# Number of sectors used for storage, >= 2
start_addr = 16

# Ble configuration
# To use the default configuration, ignore this section completely
[ble]
# Whether the ble is enabled
enabled = true
# BLE related pins, ignore any of them if you don't have
battery_adc_pin = "vddh"
# If the voltage divider is used for adc, you can use the following two values to define a voltage divider.
# For example, nice!nano have 806 + 2M resistors, the saadc measures voltage on 2M resistor, so the two values should be set to 2000 and 2806
# Measured resistance for input adc, it should be less than adc_divider_total
adc_divider_measured = 2000
# Total resistance of the full path for input adc
adc_divider_total = 2806
# Pin that reads battery's charging state, `low-active` means the battery is charging when `charge_state.pin` is low
# Input pin that indicates the charging state
charge_state = { pin = "PIN_1", low_active = true }
# Output LED pin that blinks when the battery is low
charge_led= { pin = "PIN_2", low_active = true }

# Split configuration
# This section is conflict with [split] section, you could only have either [matrix] or [split], but NOT BOTH
[split]
# Connection type of split, "serial" or "ble"
connection = "serial"

# Split central config
[split.central]
# Number of rows on central board
rows = 2
# Number of cols on central board
cols = 2
# Row offset of central matrix to the whole matrix
row_offset = 0
# Col offset of central matrix to the whole matrix
col_offset = 0
# If the connection type is "serial", the serial instances used on the central board are defined using "serial" field.
# It's a list of serial instances with a length equal to the number of splits.
# The order of the serial instances is important: the first serial instance on the central board
# communicates with the first split peripheral defined, and so on.
serial = [
    { instance = "UART0", tx_pin = "PIN_0", rx_pin = "PIN_1" },
    { instance = "UART1", tx_pin = "PIN_4", rx_pin = "PIN_5" },
    # For the RP2040 only, you can also use RMK's Programmable IO (PIO) UART serial port using either or both of the RP2040's two PIO blocks, PIO0 and PIO1, by enabling the RMK `rp2040_pio` feature gate in Cargo.toml.
    # The PIO serial port can be used in half-duplex mode using the same pin for RX/TX
    { instance = "PIO0", tx_pin = "PIN_6", rx_pin = "PIN_6" },
    # Or use the PIO serial port in full-duplex mode using different pins for RX/TX
    { instance = "PIO1", tx_pin = "PIN_7", rx_pin = "PIN_8" },
]
# If the connection type is "ble", we should have `ble_addr` to define the central's BLE static address
# This address should be a valid BLE random static address, see: https://academy.nordicsemi.com/courses/bluetooth-low-energy-fundamentals/lessons/lesson-2-bluetooth-le-advertising/topic/bluetooth-address/
ble_addr = [0x18, 0xe2, 0x21, 0x80, 0xc0, 0xc7]

[split.central.matrix]
matrix_type = "normal"
# Matrix IO definition on central board
input_pins = ["PIN_9", "PIN_11"]
output_pins = ["PIN_10", "PIN_12"]

# Configuration for the first split peripheral
# Note the double brackets [[ ]], which indicate that multiple split peripherals can be defined.
# The order of peripherals is important: it should match the order of the serial instances(if serial is used).
[[split.peripheral]]
# Number of rows on peripheral board
rows = 2
# Number of cols on peripheral board
cols = 1
# Row offset of peripheral matrix to the whole matrix
row_offset = 2
# Col offset of peripheral matrix to the whole matrix
col_offset = 2
# The serial instance used to communication with the central board, if the connection type is "serial"
serial = [{ instance = "UART0", tx_pin = "PIN_0", rx_pin = "PIN_1" }]
# The BLE random static address of the peripheral board
ble_addr = [0x7e, 0xfe, 0x73, 0x9e, 0x66, 0xe3]

[split.peripheral.matrix]
matrix_type = "normal"
# Matrix IO definition on peripheral board
input_pins = ["PIN_9", "PIN_11"]
output_pins = ["PIN_10"]

# More split peripherals(if you have)
[[split.peripheral]]
# The configuration is same with the first split peripheral
...
...
...

# Dependency config
[dependency]
# Whether to enable defmt, set to false for reducing binary size 
defmt_log = true
```

### Available chip names

Available chip names in `chip` field:
- rp2040
- nrf52840
- nrf52833
- nrf52832
- nrf52811
- nrf52810
- esp32c3
- esp32c6
- esp32s3
- ALL stm32s supported by [embassy-stm32](https://github.com/embassy-rs/embassy/blob/main/embassy-stm32/Cargo.toml) with USB

### Available board names

Available board names in `board` field:
- nice!nano
- nice!nano_v2
- XIAO BLE

If you want to add more built-in boards, feel free to open a PR!

## TODOs:

- [x] gen keymap from `keyboard.toml`
- [ ] read vial.json and gen
