# Wireless

RMK has built-in wireless(BLE) support for nRF52 series and ESP32. To use the wireless feature, you need to enable ble feature gate in your `Cargo.toml`:

```toml
rmk = { version = "0.7", features = [
    "nrf52840_ble", # Enable BLE feature for nRF52840
] }
```

RMK also provides BLE examples, checkout [nrf52840_ble](https://github.com/HaoboGu/rmk/tree/main/examples/use_config/nrf52840_ble), [nrf52832_ble](https://github.com/HaoboGu/rmk/tree/main/examples/use_config/nrf52832_ble), [pi_pico_w_ble](https://github.com/HaoboGu/rmk/tree/main/examples/use_config/pi_pico_w_ble) and [esp32c3_ble](https://github.com/HaoboGu/rmk/tree/main/examples/use_config/esp32c3_ble) for more details.

Due to multiple targets are not supported by `docs.rs` right now, so API documentations are not on `docs.rs`. Check examples for the usage.

## Supported microcontrollers

The following is the list of available feature gates(aka supported BLE chips):

- nrf52840_ble
- nrf52833_ble
- nrf52832_ble
- nrf52811_ble
- nrf52810_ble
- esp32c3_ble
- esp32c6_ble
- esp32s3_ble
- pico_w_ble

## Flashing to your board

RMK can be flashed via a debug probe or USB. Follow the instruction in the [`examples/use_rust/nrf52840_ble/README.md`](https://github.com/HaoboGu/rmk/blob/main/examples/use_rust/nrf52840_ble/README.md)

## Nice!nano support

RMK has special support for [nice!nano](https://nicekeyboards.com/), a widely used board for building wireless keyboard.

nice!nano has a built-in bootloader, enables flashing a .uf2 format firmware via USB drive. [`examples/use_rust/nrf52840_ble/README.md`](https://github.com/HaoboGu/rmk/blob/main/examples/use_rust/nrf52840_ble/README.md) provides instructions to convert RMK firmware to .uf2 format.

You can also refer to [RMK user guide](../user_guide/3_flash_firmware#use-uf2-bootloader) about the instructions.

## Multiple-profile support

RMK has multiple BLE profiles support. The number of profile can be set in [`[rmk]`](./configuration/rmk_config#wireless-configuration) section in the configuration, the default value is 3.

Vial user keycode can be configured to operate wireless profiles, suppose that you have N BLE profiles, then:

- `User0` - `User(N-1)`: switch to specific profile
- `UserN`: switch to next profile
- `User(N+1)`: switch to previous profile
- `User(N+2)`: clear current profile bond info
- `User(N+3)`: switch default output between USB/BLE

Vial also provides a way to customize the displayed keycode, see `customKeycodes` in [this example](https://github.com/HaoboGu/rmk/blob/main/examples/use_rust/nrf52840_ble/vial.json). If `customKeycodes` are configured, the `User0` ~ `User(N+3)` will be displayed as `BT0`, ..., `Switch Output`.

If you've connected a host for a profile, other devices would not be able to connect to this profile before doing manually clearing.
