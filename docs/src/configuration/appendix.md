# Appendix

## `keyboard.toml`

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
