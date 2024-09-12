# Split keyboard

RMK supports multi-split keyboard, which contains at least one central board and at most 8 peripheral boards. The host is connected to the central board via USB or BLE. All features in RMK are supported in split mode, such as VIAL via USB, layers, etc.

## Example

See `examples/use_rust/rp2040_split` and for the wired split keyboard example using rp2040.

See `examples/use_rust/nrf52840_ble_split` for the wireless split keyboard example using nRF52840.

See `examples/use_config/rp2040_split` and for the `keyboard.toml` + wired split keyboard example using rp2040.

See `examples/use_config/nrf52840_ble_split` for the `keyboard.toml` + wireless split keyboard example using nRF52840.


## Define central and peripherals via Rust

In RMK, split keyboard's matrix are defined with row/col number and their offsets in the whole matrix.

### Central

Running split central is quite similar with the general keyboard, the only difference is for split central, total row/col number, central matrix's row/col number, and central matrix's offsets should be passed to `run_rmk_split_central`:

```rust
// nRF52840 split central, arguments might be different for other microcontrollers, check the API docs for the detail.
run_rmk_split_central::<
            Input<'_>,
            Output<'_>,
            Driver<'_, USBD, &SoftwareVbusDetect>,
            ROW, // TOTAL_ROW
            COL, // TOTAL_COL
            2, // CENTRAL_ROW
            2, // CENTRAL_COL
            0, // CENTRAL_ROW_OFFSET
            0, // CENTRAL_COL_OFFSET
            NUM_LAYER,
        >(
            input_pins,
            output_pins,
            driver,
            crate::keymap::KEYMAP,
            keyboard_config,
            central_addr,
            spawner,
        )
```

In peripheral central, you should also run the peripheral monitor for each peripheral. This task monitors the peripheral key changes and forwards them to central core keyboard task

```rust
run_peripheral_monitor<
    2, // PERIPHERAL_ROW
    1, // PERIPHERAL_COL
    2, // PERIPHERAL_ROW_OFFSET
    2, // PERIPHERAL_COL_OFFSET
  >(peripheral_id, peripheral_addr)
```

### Peripheral

Running split peripheral is simplier. For peripheral, we don't need to specify peripheral matrix's offsets(we've done it in central!). So, the split peripheral API is like:

```rust
run_rmk_split_peripheral::<Input<'_>, Output<'_>, 2, 2>(
    input_pins,
    output_pins,
    central_addr,
    peripheral_addr,
    spawner,
)
```

where `2,2` are the size of peripheral's matrix.

## Define central and peripherals via `keyboard.toml`

You can also use the [`keyboard.toml`](./keyboard_configuration.md) to define a split keyboard.

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
# Central's matrix pins 
input_pins = ["P0_12", "P0_13"]
output_pins = ["P0_14", "P0_15"]
# Central's ble addr
ble_addr = [0x18, 0xe2, 0x21, 0x80, 0xc0, 0xc7]

# Note there're TWO brackets, since the peripheral is a list
# Peripheral 0
[[split.peripheral]]
# Matrix definition
rows = 2
cols = 1
row_offset = 2
col_offset = 2
input_pins = ["P1_11", "P1_10"]
output_pins = ["P0_30"]
# Peripheral's ble addr
ble_addr = [0x7e, 0xfe, 0x73, 0x9e, 0x11, 0xe3]

# Peripheral 1
[[split.peripheral]]
# Matrix definition
rows = 2
cols = 1
row_offset = 2
col_offset = 2
input_pins = ["P1_11", "P1_10"]
output_pins = ["P0_30"]
# Peripheral's ble addr
ble_addr = [0x7e, 0xfe, 0x71, 0x91, 0x11, 0xe3]
```

When using split, the input/output pins defined in `[matrix]` section is not valid anymore. Instead, the input/output pins of split boards are defined in `[split.central]` and `[[split.peripheral]]`. The rows/cols in `[matrix]` section is the total number of rows/cols of the whole keyboard.

If you're using BLE, `ble_addr` is required for both central and peripheral. Each device needs a `ble_addr`.

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



## Communication

RMK supports both wired and wireless communication. 

Currently, the communication type indicates that how split central communicates with split peripherals. How the central talks with the host depends only on the central. 

- For communication over BLE: the central talks with the host via BLE or USB, depends on whether the USB cable is connected
- For communication over serial: the central can only use USB to talk with the host


### Wired split

RMK uses `embedded-io-async` as the abstract layer of wired communication. Any device that implements `embedded-io-async::Read` and `embedded-io-async::Write` traits can be used as RMK split central/peripheral. The most common implementations of those traits are serial ports(UART/USART), such as `embassy_rp::uart::BufferedUart` and `embassy_stm32::usart::BufferedUart`. That unlocks many possibilities of RMK's split keyboard. For example, using different chips for central/peripheral is easy in RMK.

For hardwire connection, the TRRS cable is widely used in split keyboards to connect central and peripherals. It's also compatible with UART/USART, that means RMK can be used in most existing opensource serial based split keyboard hardwares.

### Wireless split

RMK supports BLE wireless split on only nRF chips right now. The [BLE random static address](https://novelbits.io/bluetooth-address-privacy-ble/) for both central and peripheral should be defined.


## Split keyboard project

A project of split keyboard could be like:

```
src
 - bin
   - central.rs
   - peripheral.rs
keyboard.toml
Cargo.toml
```