# Split keyboard

You can use the [`keyboard.toml`](../keyboard_configuration) to define a split keyboard.

All split related configurations are defined under `[split]` section. The following is an example using BLE:

```toml
[split]
# split connection type
connection = "ble"

# Split central
[split.central]
# Central's matrix definition and offsets
rows = 2
cols = 2
row_offset = 0
col_offset = 0

# Central's ble addr will be automatically generated. You can override it if you want.
# ble_addr = [0x18, 0xe2, 0x21, 0x80, 0xc0, 0xc7]

# Central's matrix
[split.central.matrix]
matrix_type = "normal"
input_pins = ["P0_12", "P0_13"]
output_pins = ["P0_14", "P0_15"]

# Note there're TWO brackets, since the peripheral is a list
# Peripheral 0
[[split.peripheral]]
rows = 2
cols = 1
row_offset = 2
col_offset = 2
# Peripheral's ble addr will be automatically generated. You can override it if you want.
# ble_addr = [0x7e, 0xfe, 0x73, 0x9e, 0x11, 0xe3]

# Peripheral 0's matrix definition
[split.peripheral.matrix]
matrix_type = "normal"
input_pins = ["P1_11", "P1_10"]
output_pins = ["P0_30"]

# Peripheral 1
[[split.peripheral]]
# Matrix definition
rows = 2
cols = 1
row_offset = 2
col_offset = 2
# Peripheral's ble addr will be automatically generated. You can override it if you want.
# ble_addr = [0x7e, 0xfe, 0x71, 0x91, 0x11, 0xe3]

# Peripheral 1's matrix definition
[split.peripheral.matrix]
matrix_type = "normal"
input_pins = ["P1_11", "P1_10"]
output_pins = ["P0_30"]
```

## Split keyboard matrix configuration

When using split, the input/output pins defined in `[matrix]` section is not valid anymore. Instead, the input/output pins of split boards are defined in `[split.central.matrix]` and `[split.peripheral.matrix]`. The contents of the split matrix configuration is the same as for `[matrix]`. This means each peripheral and central keyboard also supports `direct_pin`.

The rows/cols in `[layout]` section is the total number of rows/cols of the whole keyboard. For each split(central and peripherals), rows/cols/row_offset/col_offset should be defined to indicate the current split's position in the whole keyboard's layout. Suppose we have a 2-row + 5-col split, the left(central) is 2\*2, and the right(peripheral) is 2\*3, the positions should be defined as:

```toml
[split.central]
rows = 2 # The number of rows in central
cols = 2 # The number of cols in central
row_offset = 0 # The row offset, for central(left) it's 0
col_offset = 0 # The col offset, for central(left) it's 0

[[split.peripheral]]
rows = 2 # The number of rows in the peripheral
cols = 3 # The number of cols in the peripheral
row_offset = 0 # The row offset of the peripheral, peripheral starts from row 0, so the offset is 0
col_offset = 2 # The col offset of the peripheral. Central has 2 cols, so the col_offset should be 2 for the peripheral
```

## Split keyboard connection configuration

If you're using BLE, `ble_addr` will be automatically generated. You can also override it if you want.

If you're using serial, in `[split.central]` you need to defined a list of serial ports, the number of the list should be same with the number of the peripherals:

```toml
[split]
connection = "serial"

[split.central]
..
# Two serial ports used in central. The order matters.
serial = [
    # Serial port which is connected to peripheral 0.
    { instance = "UART0", tx_pin = "PIN_0", rx_pin = "PIN_1" },
    # Serial port which is connected to peripheral 1.
    { instance = "UART1", tx_pin = "PIN_4", rx_pin = "PIN_5" },
]

# Peripheral 0
[[split.peripheral]]
..
# Serial port used in peripheral 0, it's a list with only one serial port element.
serial = [{ instance = "UART0", tx_pin = "PIN_0", rx_pin = "PIN_1" }]

# Peripheral 1
[[split.peripheral]]
..
serial = [{ instance = "UART0", tx_pin = "PIN_0", rx_pin = "PIN_1" }]
```

If you're using the Programmable IO (PIO) serial port with an RP2040 chip, subsitute the UART serial port interface with the PIO block, e.g. `PIO0`:

```toml
[split]
connection = "serial"

[split.central]
..
serial = [
    # Half-duplex serial port using Programmable IO block PIO0
    { instance = "PIO0", tx_pin = "PIN_0", rx_pin = "PIN_0" },
]

[[split.peripheral]]
..
serial = [{ instance = "PIO0", tx_pin = "PIN_0", rx_pin = "PIN_0" }]
```
