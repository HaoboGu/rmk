# Split keyboard

You can use the [`keyboard.toml`](./index#keyboardtoml) to define a split keyboard.

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
row_pins = ["P0_12", "P0_13"]
col_pins = ["P0_14", "P0_15"]

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
row_pins = ["P1_11", "P1_10"]
col_pins = ["P0_30"]

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
row_pins = ["P1_11", "P1_10"]
col_pins = ["P0_30"]
```

## Split keyboard matrix configuration

When using split, the input/output pins defined in the `[matrix]` section are not valid anymore. Instead, the input/output pins of split boards are defined in `[split.central.matrix]` and `[split.peripheral.matrix]`. The contents of the split matrix configuration are the same as for `[matrix]`. This means each peripheral and central keyboard also supports `direct_pin`.

The rows/cols in the `[layout]` section are the total number of rows/cols of the whole keyboard. For each split (central and peripherals), rows/cols/row_offset/col_offset should be defined to indicate the current split's position in the whole keyboard's layout. Suppose we have a 2-row + 5-col split, the left (central) is 2\*2, and the right (peripheral) is 2\*3, the positions should be defined as:

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

If you're using serial, in `[split.central]` you need to define a list of serial ports; the number of items in the list should be the same as the number of peripherals:

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

If you're using the Programmable IO (PIO) serial port with an RP2040 chip, substitute the UART serial port interface with the PIO block, e.g. `PIO0`:

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


## Define central and peripherals via `keyboard.toml`

See [this section](../configuration/split_keyboard) for more details.

## Define central and peripherals via Rust

In RMK, split keyboard's matrix are defined with row/col number and their offsets in the whole matrix.

### Central

Matrix configuration on the split central is quite similar with the general keyboard, the only difference is for split central, central matrix needs to be wrapped in an offset matrix:

```rust
// Suppose that the central matrix is col2row
let mut matrix = OffsetMatrixWrapper::<
    _,
    _,
    _,
    0, // ROW OFFSET
    0, // COL OFFSET
    >(Matrix::<
        _,
        _,
        _,
        4, // ROW
        7, // COL
        true, // COL2ROW = true, set it to false to use ROW2COL matrix
    >::new(row_pins, col_pins, debouncer));
```

On the central, you should also run the peripheral manager for each peripheral. This task monitors the peripheral key changes and forwards them to central core keyboard task


import { Rust, Toml } from '../../components/LangBadge'
import { Tab, Tabs } from '@theme'

<Tabs>
<Tab label={<Rust />}>

```rust title="BLE Split Central"
// BLE split central, arguments might be different for other microcontrollers, check the API docs or examples for other usages.
run_peripheral_manager::<
    2, // PERIPHERAL_ROW
    1, // PERIPHERAL_COL
    2, // PERIPHERAL_ROW_OFFSET
    2, // PERIPHERAL_COL_OFFSET
    _,
  >(peripheral_id, peripheral_addr, &stack)
```

</Tab>
<Tab label={<Rust />}>

```rust title="Serial Split Central"
// UART split central, arguments might be different for other microcontrollers, check the API docs or examples for other usages.
run_peripheral_manager::<
    2, // PERIPHERAL_ROW
    1, // PERIPHERAL_COL
    2, // PERIPHERAL_ROW_OFFSET
    2, // PERIPHERAL_COL_OFFSET
    _,
  >(peripheral_id, uart_receiver),
```

</Tab>
</Tabs>

### Peripheral

Running split peripheral is simpler. For the peripheral, we don't need to specify the peripheral matrix's offsets (we've done that in the central!). So, the split peripheral API is like:

<Tabs>
<Tab label={<Rust />}>

```rust title="BLE Split Peripheral"
// Use normal matrix on the peripheral
let mut matrix = Matrix::<_, _, _, 4, 7, true>::new(row_pins, col_pins, debouncer);

// BLE split peripheral, arguments might be different for other microcontrollers, check the API docs or examples for other usages.
run_rmk_split_peripheral(central_addr, &stack),
```

</Tab>
<Tab label={<Rust />}>

```rust title="Serial Split Peripheral"
// Use normal matrix on the peripheral
let mut matrix = Matrix::<_, _, _, 4, 7, true>::new(row_pins, col_pins, debouncer);
let uart_instance = BufferedUart::new(p.UART0, p.PIN_0, p.PIN_1, Irqs, tx_buf, rx_buf, uart::Config::default());

// UART split peripheral, arguments might be different for other microcontrollers, check the API docs or examples for other usages.
run_rmk_split_peripheral(uart_instance),
```

</Tab>
</Tabs>