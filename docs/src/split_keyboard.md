# Split keyboard

<div class="warning">
This feature is currently not fully implemented, only wired split via serial is implemented(and tested).
</div>

RMK supports multi-split keyboard, which contains at least one master board and at most 8 slave boards. The host is connected to the master board via USB or BLE. All features in RMK are supported in split mode, such as VIAL via USB, layers, etc.

## Example

See `examples/use_rust/rp2040_split` for the wired split keyboard example using rp2040.

## Communication

RMK plans to support both wired and wireless communication. 

When the master & slave talk to each other, the **debounced key states** are sent. The master board receives the key states, converts them to actual keycode and then sends keycodes to the host.

That means the master board should have a full keymap stored in the storage/ram. The slaves just do matrix scanning, debouncing and sending key states over serial/ble.

### Wired split

RMK uses `embedded-io-async` as the abstract layer of wired communication. Any device that implements `embedded-io-async::Read` and `embedded-io-async::Write` traits can be used as RMK split master/slave. The most common implementations of those traits are serial ports(UART/USART), such as `embassy_rp::uart::BufferedUart` and `embassy_stm32::usart::BufferedUart`. That unlocks many possibilities of RMK's split keyboard. For example, using different chips for master/slave is easy in RMK.

For hardwire connection, the TRRS cable is widely used in split keyboards to connect master and slaves. It's also compatible with UART/USART, that means RMK can be used in most existing opensource serial based split keyboard hardwares.

### Wireless split

This feature is under construction. BLE communication will be supported.


## Split keyboard project

A project of split keyboard should like:

```
src
 - bin
   - right.rs
   - left.rs
keyboard.toml
Cargo.toml
```