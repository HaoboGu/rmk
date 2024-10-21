# Wireless

RMK has built-in wireless(BLE) support for nRF52 series and ESP32. To use the wireless feature, you need to enable ble feature gate in your `Cargo.toml`:

```toml
rmk = { version = "0.3.1", features = [
    "nrf52840_ble", # Enable BLE feature for nRF52840
] }
```

RMK also provides ble examples, check [nrf52840_ble](https://github.com/HaoboGu/rmk/tree/main/examples/use_config/nrf52840_ble), [nrf52832_ble](https://github.com/HaoboGu/rmk/tree/main/examples/use_config/nrf52832_ble) and [esp32c3_ble](https://github.com/HaoboGu/rmk/tree/main/examples/use_config/esp32c3_ble).

Due to multiple targets are not supported by `docs.rs` right now, so API documentations are not there. Check examples for the usage. I'll add a separate doc site later.

## Supported microcontrollers

The following is the list of available feature gates(aka supported BLE chips): 

- nrf52840_ble
- nrf52833_ble
- nrf52832_ble
- nrf52810_ble
- nrf52811_ble
- esp32c3_ble
- esp32c6_ble
- esp32s3_ble

## Flashing to your board

RMK can be flashed via a debug probe or USB. Follow the instruction in the [`examples/use_rust/nrf52840_ble/README.md`](https://github.com/HaoboGu/rmk/blob/main/examples/use_rust/nrf52840_ble/README.md)

## Nice!nano support

RMK has special support for [nice!nano](https://nicekeyboards.com/), a widely used board for building wireless keyboard.

nice!nano has a built-in bootloader, enables flashing a .uf2 format firmware via USB drive. [`examples/use_rust/nrf52840_ble/README.md`](https://github.com/HaoboGu/rmk/blob/main/examples/use_rust/nrf52840_ble/README.md) provides instructions to convert RMK firmware to .uf2 format.

You can also refer to [RMK user guide](./user_guide/4_compile_and_flash.md#use-uf2-bootloader) about the instructions.

## Multiple-profile support

RMK supports at most 8 wireless profiles, profile 0 is activated by default. Vial key `User0` - `User7` are used to switch to specific profile, `User8` and `User9` are switching to next, previous profile, `User10` is clear profile.

### Implementation

1. A simple `manual disconnection -> re-advertise with BLACKLIST -> new connection` loop
2. Multiple-device multiple slot, slot switching 
3. + USB