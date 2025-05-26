# Wireless

RMK has built-in wireless(BLE) support for nRF52 series and ESP32. To use the wireless feature, you need to enable ble feature gate in your `Cargo.toml`:

```toml
rmk = { version = "0.4", features = [
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

You can also refer to [RMK user guide](/docs/user_guide/3_flash_firmware.md#use-uf2-bootloader) about the instructions.

## Multiple-profile support

RMK supports at most 8 wireless profiles, profile 0 is activated by default. Vial user keycode can be configured to operate wireless profiles:

- `User0` - `User7`: switch to specific profile
- `User8`: switch to next profile
- `User9`: switch to previous profile
- `User10`: clear current profile bond info
- `User11`: switch default output between USB/BLE

Vial also provides a way to customize the displayed keycode, see `customKeycodes` in [this example](https://github.com/HaoboGu/rmk/blob/main/examples/use_rust/nrf52840_ble/vial.json). If `customKeycodes` are configured, the `User0` ~ `User11` will be displayed as `BT0`, ..., `Switch Output`.

If you've connected a host for a profile, other devices would not be able to connect to this profile before doing manually clearing.
