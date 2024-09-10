# Split keyboard

<div class="warning">
This feature is currently not fully implemented, only wired split via serial is implemented(and tested).
</div>

RMK supports multi-split keyboard, which contains at least one central board and at most 8 peripheral boards. The host is connected to the central board via USB or BLE. All features in RMK are supported in split mode, such as VIAL via USB, layers, etc.

## Example

See `examples/use_rust/rp2040_split` for the wired split keyboard example using rp2040.

See `examples/use_rust/nrf52840_ble_split` for the wireless split keyboard example using nRF52840.

## Define central and peripherals

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