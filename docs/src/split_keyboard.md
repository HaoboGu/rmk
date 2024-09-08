# Split keyboard

<div class="warning">
This feature is currently not fully implemented, only wired split via serial is implemented(and tested).
</div>

RMK supports multi-split keyboard, which contains at least one master board and at most 8 slave boards. The host is connected to the master board via USB or BLE. All features in RMK are supported in split mode, such as VIAL via USB, layers, etc.

## Example

See `examples/use_rust/rp2040_split` for the wired split keyboard example using rp2040.

See `examples/use_rust/nrf52840_ble_split` for the wireless split keyboard example using nRF52840.

## Define master and slaves

In RMK, split keyboard's matrix are defined with row/col number and their offsets in the whole matrix.

### Master

Running split master is quite similar with the general keyboard, the only difference is for split master, total row/col number, master matrix's row/col number, and master matrix's offsets should be passed to `run_rmk_split_master`:

```rust
// nRF52840 split master, arguments might be different for other microcontrollers, check the API docs for the detail.
run_rmk_split_master::<
            Input<'_>,
            Output<'_>,
            Driver<'_, USBD, &SoftwareVbusDetect>,
            ROW, // TOTAL_ROW
            COL, // TOTAL_COL
            2, // MASTER_ROW
            2, // MASTER_COL
            0, // MASTER_ROW_OFFSET
            0, // MASTER_COL_OFFSET
            NUM_LAYER,
        >(
            input_pins,
            output_pins,
            driver,
            crate::keymap::KEYMAP,
            keyboard_config,
            master_addr,
            spawner,
        )
```

In slave master, you should also run the slave monitor for each slave. This task monitors the slave key changes and forwards them to master core keyboard task

```rust
run_slave_monitor<
    2, // SLAVE_ROW
    1, // SLAVE_COL
    2, // SLAVE_ROW_OFFSET
    2, // SLAVE_COL_OFFSET
  >(slave_id, slave_addr)
```

### Slave

Running split slave is simplier. For slave, we don't need to specify slave matrix's offsets(we've done it in master!). So, the split slave API is like:

```rust
run_rmk_split_slave::<Input<'_>, Output<'_>, 2, 2>(
    input_pins,
    output_pins,
    master_addr,
    slave_addr,
    spawner,
)
```

where `2,2` are the size of slave's matrix.

## Communication

RMK supports both wired and wireless communication. 

Currently, the communication type indicates that how split master communicates with split slaves. How the master talks with the host depends only on the master. 

- For communication over BLE: the master talks with the host via BLE or USB, depends on whether the USB cable is connected
- For communication over serial: the master can only use USB to talk with the host


### Wired split

RMK uses `embedded-io-async` as the abstract layer of wired communication. Any device that implements `embedded-io-async::Read` and `embedded-io-async::Write` traits can be used as RMK split master/slave. The most common implementations of those traits are serial ports(UART/USART), such as `embassy_rp::uart::BufferedUart` and `embassy_stm32::usart::BufferedUart`. That unlocks many possibilities of RMK's split keyboard. For example, using different chips for master/slave is easy in RMK.

For hardwire connection, the TRRS cable is widely used in split keyboards to connect master and slaves. It's also compatible with UART/USART, that means RMK can be used in most existing opensource serial based split keyboard hardwares.

### Wireless split

RMK supports BLE wireless split on only nRF chips right now. The [BLE random static address](https://novelbits.io/bluetooth-address-privacy-ble/) for both master and slave should be defined.


## Split keyboard project

A project of split keyboard could be like:

```
src
 - bin
   - master.rs
   - slave.rs
keyboard.toml
Cargo.toml
```