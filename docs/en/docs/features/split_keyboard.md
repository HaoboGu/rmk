# Split keyboard

RMK supports multi-split keyboard, which contains at least one central board and at most 8 peripheral boards. The host is connected to the central board via USB or BLE. All features in RMK are supported in split mode, such as VIAL via USB, layers, etc.

## Examples

There are many examples for split keyboards in [RMK examples folder](https://github.com/HaoboGu/rmk/tree/main/examples/use_config).


## Define central and peripherals via `keyboard.toml`

See [this section](./configuration/split_keyboard) for more details.

## Define central and peripherals via Rust

In RMK, split keyboard's matrix are defined with row/col number and their offsets in the whole matrix.

### Central

Matrix configuration on the split central is quite similar with the general keyboard, the only difference is for split central, central matrix's row/col number, and central matrix's offsets should be passed to the central matrix:

```rust
// Suppose that the central matrix is col2row
let mut matrix = CentralMatrix::<
    _,
    _,
    _,
    0, // ROW OFFSET
    0, // COL OFFSET
    4, // ROW
    7, // COL
>::new(input_pins, output_pins, debouncer);
```

On the central, you should also run the peripheral manager for each peripheral. This task monitors the peripheral key changes and forwards them to central core keyboard task

::: code-group

```rust[BLE split]
// BLE split central, arguments might be different for other microcontrollers, check the API docs or examples for other usages.
run_peripheral_manager::<
    2, // PERIPHERAL_ROW
    1, // PERIPHERAL_COL
    2, // PERIPHERAL_ROW_OFFSET
    2, // PERIPHERAL_COL_OFFSET
    _,
  >(peripheral_id, peripheral_addr, &stack)
```


```rust[Serial split]
// UART split central, arguments might be different for other microcontrollers, check the API docs or examples for other usages.
run_peripheral_manager::<    
    2, // PERIPHERAL_ROW
    1, // PERIPHERAL_COL
    2, // PERIPHERAL_ROW_OFFSET
    2, // PERIPHERAL_COL_OFFSET
    _,
  >(0, uart_receiver),
```
:::

### Peripheral

Running split peripheral is simplier. For peripheral, we don't need to specify peripheral matrix's offsets(we've done it in central!). So, the split peripheral API is like:

::: code-group

```rust[BLE split]
// Use normal matrix on the peripheral
let mut matrix = Matrix::<_, _, _, 4, 7>::new(input_pins, output_pins, debouncer);

// BLE split peripheral, arguments might be different for other microcontrollers, check the API docs or examples for other usages.
run_rmk_split_peripheral(central_addr, &stack),
```

```rust[Serial split]
// Use normal matrix on the peripheral
let mut matrix = Matrix::<_, _, _, 4, 7>::new(input_pins, output_pins, debouncer);
let uart_instance = BufferedUart::new(p.UART0, Irqs, p.PIN_0, p.PIN_1, tx_buf, rx_buf, uart::Config::default());

// UART split peripheral, arguments might be different for other microcontrollers, check the API docs or examples for other usages.
run_rmk_split_peripheral(uart_instance),

```

:::

where `2,2` are the size of peripheral's matrix.

## Communication

RMK supports both wired and wireless communication.

Currently, the communication type indicates that how split central communicates with split peripherals. How the central talks with the host depends only on the central.

- For communication over BLE: the central talks with the host via BLE or USB, depends on whether the USB cable is connected
- For communication over serial: the central can only use USB to talk with the host

### Wired split

Powered by great Rust embedded ecosystem, RMK supports most existing opensource serial based split keyboard hardwares using UART, USART, PIO, etc.

::: details

RMK uses `embedded-io-async` as the abstract layer of wired communication. Any device that implements `embedded-io-async::Read` and `embedded-io-async::Write` traits can be used as RMK split central/peripheral. The most common implementations of those traits are serial ports(UART/USART), such as `embassy_rp::uart::BufferedUart` and `embassy_stm32::usart::BufferedUart`. That unlocks many possibilities of RMK's split keyboard. For example, using different chips for central/peripheral is easy in RMK.

:::

For keyboards connected using only a single wire, e.g. a 3-pole TRS cable, for the **RP2040 only** RMK implements a half-duplex UART serial port, `rmk::split::rp::uart::BufferedUart`, using one or both of the Programmable IO (PIO) blocks available on the RP2040 chip. The PIO serial port also supports full-duplex over two wires, and can be used when the central/peripheral connection does not use the pins connected to the chip's standard UART ports.

To use the the PIO UART driver feature, you need to enable the `rp2040_pio` feature gate in your `Cargo.toml`:

```toml
rmk = { version = "0.7", features = [
    "split",
    "rp2040_pio", # Enable PIO UART driver for rp2040
] }
```

### Wireless split

RMK supports BLE wireless split on nRF52, ESP32 and Pi Pico W right now. For BLE split, the central and peripheral parts are connected via BLE, and the host is connected to the central via USB or BLE.

::: tip

`storage` feature is required for BLE split.

:::


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

In `Cargo.toml`, the `split` feature should be enabled, and the `[[bin]]` section should be added for both central and peripheral:

```toml
rmk = { version = "0.7", features = [
    "nrf52840_ble",
    "split", # Enable split keyboard feature
    "async_matrix",
    "adafruit_bl",
] }

# ..
# ..

# Split keyboard entry files
[[bin]]
name = "central"
path = "src/central.rs"

[[bin]]
name = "peripheral"
path = "src/peripheral.rs"
```