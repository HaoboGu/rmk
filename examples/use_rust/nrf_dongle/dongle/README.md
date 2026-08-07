# RMK tri-mode dongle on nRF54LM20A

The dongle firmware from the tri-mode dongle design
(`docs/docs/main/docs/development/dongle_mode_design.md`): a BLE central that
relays bonded RMK keyboards to the host over USB (HID + a Rynk CDC port).

Any `rynk` BLE keyboard of a matching RMK version can pair with it; see
[`../central`](../central) for the split-keyboard central in this example.

## Flash

```shell
cargo build --release
# via J-Link:
./run.sh target/thumbv8m.main-none-eabihf/release/rmk-nrf54lm20-dongle
```

Pairing window opens for 30s at power-on, or when a connected keyboard holds
its dongle key for 5s.
