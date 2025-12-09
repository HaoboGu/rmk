# Appendix

### `keyboard.toml`

The following TOML contains all available settings in `keyboard.toml`

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
# Either "board" or "chip" can be set, but not both
chip = "rp2040"
board = "nice!nano_v2"
# USB is enabled by default for most chips
# Set to false if you don't want USB
usb_enable = true

# Set matrix IO for the board. This section is for non-split keyboards and is in conflict with the [split] section
[matrix]
# `matrix_type` is optional. Default is "normal"
matrix_type = "normal"
# Input and output pins
row_pins = ["PIN_6", "PIN_7", "PIN_8", "PIN_9"]
col_pins = ["PIN_19", "PIN_20", "PIN_21"]
# RMK uses col2row as the default matrix diode direction, if you want to use a row2col matrix, add `row2col = true`
row2col = false

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
# Number of rows. For a split keyboard, this is the total number of rows for all splits
rows = 5
# Number of cols. For a split keyboard, this is the total number of cols for all splits
cols = 4
# Number of layers. Be careful, since large layer number takes more flash and RAM
layers = 3
# keypad example: (for the key in position (2,1) the `H1` profile is activated)
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
matrix_map = """
(0,0,R) (0,1,R)    (0,2,R) (0,3,R)
(1,0,R) (1,1,R)    (1,2,R) (1,3,R)
(2,0,R) (2,1,R:H1) (2,2,R)
(3,0,R) (3,1,R)    (3,2,R) (3,3,R)
(4,0,R)            (4,1,R)
"""

# here are the aliases for the example layer.keys below
[aliases]
my_cut = "WM(X, LCtrl)"
my_copy = "WM(C, LCtrl)"
my_paste = "WM(V, LCtrl)"

# Key map definitions per layer:
# The number (and order) of entries on each layer should be
# identical with the number (and order) of entries in `matrix_map`.
# Empty layers will be used to fill if the number of explicitly
# defined layers is smaller than the `layout.layers` setting

# layer 0 (default):
# (the number comes from the order of '[[layer]] entries' in the file)
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
TO(base_layer)   @MyCut     @MyCopy          @MyPaste
MouseBtn1        MouseUp    MouseBtn2        MouseWheelUp
MouseLeft        MouseBtn4  MouseRight
MouseWheelLeft   MouseDown  MouseWheelRight  MouseWheelDown
          MouseBtn1         MouseBtn2
"""

# Behavior configuration, if you don't want to customize anything, just ignore this section
[behavior]
# Tri Layer configuration
tri_layer = { upper = 1, lower = 2, adjust = 3 }
# One Shot configuration
one_shot = { timeout = "1s" }

[behavior.morse]
# default profile for morse, tap dance and tap-hold keys:
enable_flow_tap = true
prior_idle_time = "120ms"
hold_on_other_press = true
unilateral_false = false
hold_timeout = "250ms"
gap_timeout = "250ms"

# list of morse (tap dance) keys:
morses = [
  # TD(0) Function key that outputs F1 on tap, F2 on double tap, layer 1 on hold
  { tap = "F1", hold = "MO(1)", double_tap = "F2" },

  # TD(1) Extended tap dance representation for function keys
  { tap_actions = ["F1", "F2", "F3", "F4", "F5"], hold_actions = ["MO(1)", "MO(2)", "MO(3)", "MO(4)", "MO(5)"] }

  # TD(2) Morse code like representation
  { morse_actions = [
      {pattern = ".-", action = "A"},
      {pattern = "-...", action = "B"},
      {pattern = "-.-.", action = "C"},
      {pattern = "-..", action = "D"},
      {pattern = ".", action = "E"},
      {pattern = "..-.", action = "F"},
      {pattern = "--.", action = "G"},
      {pattern = "....", action = "H"},
      {pattern = "..", action = "I"},
      {pattern = ".---", action = "J"},
      {pattern = "-.-", action = "K"},
      {pattern = ".-..", action = "L"},
      {pattern = "--", action = "M"},
      {pattern = "-.", action = "N"},
      {pattern = "---", action = "O"},
      {pattern = ".--.", action = "P"},
      {pattern = "--.-", action = "Q"},
      {pattern = ".-.", action = "R"},
      {pattern = "...", action = "S"},
      {pattern = "-", action = "T"},
      {pattern = "..-", action = "U"},
      {pattern = "...-", action = "V"},
      {pattern = ".--", action = "W"},
      {pattern = "-..-", action = "X"},
      {pattern = "-.--", action = "Y"},
      {pattern = "--..", action = "Z"},
      {pattern = ".----", action = "Kc1"},
      {pattern = "..---", action = "Kc2"},
      {pattern = "...--", action = "Kc3"},
      {pattern = "....-", action = "Kc4"},
      {pattern = ".....", action = "Kc5"},
      {pattern = "-....", action = "Kc6"},
      {pattern = "--...", action = "Kc7"},
      {pattern = "---..", action = "Kc8"},
      {pattern = "----.", action = "Kc9"},
      {pattern = "-----", action = "Kc0"}
    ], profile = "MRZ" }
]

[behavior.morse.profiles]
# matrix_map may refer these to override the defaults given in [behavior.morse] for some key positions - this example is a home row mod
H1 = { permissive_hold = true, unilateral_tap = true, hold_timeout = "250ms", gap_timeout = "250ms" }
H2 = { permissive_hold = true, unilateral_tap = true, hold_timeout = "200ms", gap_timeout = "200ms" }
MRZ = { normal_mode = true, unilateral_tap = false, hold_timeout = "200ms", gap_timeout = "200ms" }

# Combo configuration
[behavior.combo]
timeout = "150ms"
combos = [
  # Press J and K keys simultaneously to output Escape key
  { actions = ["J", "K"], output = "Escape" }
]

# Macro configuration
[[behavior.macro.macros]]
operations = [
    { operation = "text", text = "Hello" }
]

# Fork configuration
[behavior.fork]
forks = [
  # Shift + '.' output ':' key
  { trigger = "Dot", negative_output = "Dot", positive_output = "WM(Semicolon, LShift)", match_any = "LShift|RShift" }
]

# Lighting configuration, if you don't have any light, just ignore this section.
[light]
# LED pins, capslock, scrolllock, numslock. You can safely ignore any of them if you don't have
capslock = { pin = "PIN_0", low_active = true }
scrolllock = { pin = "PIN_1", low_active = true }
numslock = { pin = "PIN_2", low_active = true }

# Storage configuration.
# To use the default configuration, ignore this section completely
[storage]
# Whether the storage is enabled
enabled = true
# The start address of storage
start_addr = 0xA0000
# Number of sectors used for storage, >= 2
start_addr = 16
# Clear storage at keyboard boot.
# Set it to true will reset the storage(including keymap, BLE bond info, etc.) at each reboot.
# This option is useful when testing the firmware.
clear_storage = false

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
# [Deprecated] Pin that reads battery's charging state, `low-active` means the battery is charging when `charge_state.pin` is low
# Input pin that indicates the charging state
# charge_state = { pin = "PIN_1", low_active = true }
# [Deprecated] Output LED pin that blinks when the battery is low
# charge_led= { pin = "PIN_2", low_active = true }

# RMK internal configuration
[rmk]
# Mouse key interval (ms) - controls mouse movement speed
mouse_key_interval = 20
# Mouse wheel interval (ms) - controls scrolling speed
mouse_wheel_interval = 80
# Maximum number of combos keyboard can store
combo_max_num = 8
# Maximum number of keys pressed simultaneously in a combo
combo_max_length = 4
# Maximum number of forks for conditional key actions
fork_max_num = 8
# Maximum number of morse keys keyboard can store (max 256)
# (Each morse key is a programmable multi-tap/hold key)
morse_max_num = 8
# Maximum number of patterns a morse key can handle
max_patterns_per_key = 36
# Macro space size in bytes for storing sequences
macro_space_size = 256
# Default debounce time in ms
debounce_time = 20
# Event channel size
event_channel_size = 16
# Report channel size
report_channel_size = 16
# Vial channel size
vial_channel_size = 4
# Flash channel size
flash_channel_size = 4
# The number of the split peripherals
split_peripherals_num = 1
# The number of available BLE profiles
ble_profiles_num = 3

# Split configuration
# This section conflicts with the [matrix] section. You can only have either [matrix] or [split], but NOT BOTH
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
    # For the RP2040 only, you can also use RMK's Programmable IO (PIO) UART serial port using either or both of the RP2040's two PIO blocks, PIO0 and PIO1, by enabling the RMK `rp2040` feature gate in Cargo.toml.
    # The PIO serial port can be used in half-duplex mode using the same pin for RX/TX
    { instance = "PIO0", tx_pin = "PIN_6", rx_pin = "PIN_6" },
    # Or use the PIO serial port in full-duplex mode using different pins for RX/TX
    { instance = "PIO1", tx_pin = "PIN_7", rx_pin = "PIN_8" },
]
# If the connection type is "ble", we can override the BLE static address used by setting `ble_addr`.
# This address should be a valid BLE random static address, see: https://academy.nordicsemi.com/courses/bluetooth-low-energy-fundamentals/lessons/lesson-2-bluetooth-le-advertising/topic/bluetooth-address/
ble_addr = [0x18, 0xe2, 0x21, 0x80, 0xc0, 0xc7]

[split.central.matrix]
matrix_type = "normal"
# Matrix IO definition on central board
row_pins = ["PIN_9", "PIN_11"]
col_pins = ["PIN_10", "PIN_12"]

# Configuration for the first split peripheral
# Note the double brackets [[ ]], which indicate that multiple split peripherals can be defined.
# The order of peripherals is important: it should match the order of the serial instances (if serial is used).
[[split.peripheral]]
# Number of rows on peripheral board
rows = 2
# Number of cols on peripheral board
cols = 1
# Row offset of peripheral matrix to the whole matrix
row_offset = 2
# Col offset of peripheral matrix to the whole matrix
col_offset = 2
# The serial instance used to communicate with the central board, if the connection type is "serial"
serial = [{ instance = "UART0", tx_pin = "PIN_0", rx_pin = "PIN_1" }]
# Override the BLE random static address of the peripheral board
ble_addr = [0x7e, 0xfe, 0x73, 0x9e, 0x66, 0xe3]

[split.peripheral.matrix]
matrix_type = "normal"
# Matrix IO definition on peripheral board
row_pins = ["PIN_9", "PIN_11"]
col_pins = ["PIN_10"]

# More split peripherals (if you have any)
[[split.peripheral]]
# The configuration is the same as the first split peripheral
...
...
...

# Dependency config
[dependency]
# Whether to enable defmt, set to false for reducing binary size
defmt_log = true

# Host-side tools configuration
[host]
# Whether Vial is enabled (default: true)
vial_enabled = true
# The unlock keys are the combo of the row 0, col 0 key and
# the row 0, col 1 key
unlock_keys = [[0, 0], [0, 1]]

# Chip-specific configuration
# To use the default configuration, ignore this section completely
# Use chip-specific sections like [chip.nrf52840] for chip-specific settings
[chip.nrf52840]
# DCDC regulator 0 enabled (nrf52840 only, default: true)
# **Note**: Do not enable DC/DC regulator without an external LC filter being connected
# as this will inhibit device operation, including debug access, until an LC filter is connected.
dcdc_reg0 = true
# DCDC regulator 1 enabled (nrf52840, nrf52833, default: true)
# **Note**: Do not enable DC/DC regulator without an external LC filter being connected
# as this will inhibit device operation, including debug access, until an LC filter is connected.
dcdc_reg1 = true
# DCDC regulator 0 voltage (nrf52840 only, default: "3V3")
# Valid values: "3V3" or "1V8"
dcdc_reg0_voltage = "3V3"
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

- `nice!nano`
- `nice!nano_v2`
- `XIAO BLE`
- `pi_pico_w`

If you want to add more built-in boards, feel free to open a PR!
