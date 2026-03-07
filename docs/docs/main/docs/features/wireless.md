# Wireless

RMK has built-in wireless (BLE) support for nRF52 series, ESP32, and Raspberry Pi Pico W. To use the wireless feature, you need to enable the corresponding feature gate in your `Cargo.toml`:

```toml
rmk = { version = "...", features = [
    "nrf52840_ble", # Enable BLE feature for nRF52840
] }
```

RMK also provides BLE examples; check out [nrf52840_ble](https://github.com/HaoboGu/rmk/tree/main/examples/use_config/nrf52840_ble), [nrf52832_ble](https://github.com/HaoboGu/rmk/tree/main/examples/use_config/nrf52832_ble), [pi_pico_w_ble](https://github.com/HaoboGu/rmk/tree/main/examples/use_config/pi_pico_w_ble), and [esp32c3_ble](https://github.com/HaoboGu/rmk/tree/main/examples/use_config/esp32c3_ble) for more details.

Since multiple targets are not currently supported by `docs.rs`, API documentation is not available on `docs.rs`. Check the examples for usage.

## Supported Microcontrollers

The following is the list of available feature gates (i.e., supported BLE chips):

- nrf52840_ble
- nrf52833_ble
- nrf52832_ble
- nrf52811_ble
- nrf52810_ble
- esp32c3_ble
- esp32c6_ble
- esp32s3_ble
- pico_w_ble (for Raspberry Pi Pico W and Raspberry Pi Pico 2 W)

## Nice!nano Support

RMK has special support for [nice!nano](https://nicekeyboards.com/), a widely used board for building wireless keyboards.

nice!nano has a built-in bootloader that enables flashing a .uf2 format firmware via USB drive. [`examples/use_rust/nrf52840_ble/README.md`](https://github.com/HaoboGu/rmk/blob/main/examples/use_rust/nrf52840_ble/README.md) provides instructions for converting RMK firmware to .uf2 format.

You can also refer to the [RMK user guide](../user_guide/flash_firmware#use-uf2-bootloader) for the instructions.

## Multiple-Profile Support

RMK has multiple BLE profile support. The number of profiles can be set in the [`[rmk]`](../configuration/rmk_config#wireless-configuration) section in the configuration; the default value is 3.

Vial user keycodes can be configured to operate wireless profiles. Suppose that you have N BLE profiles, then:

- `User0` - `User(N-1)`: switch to a specific profile
- `UserN`: switch to the next profile
- `User(N+1)`: switch to the previous profile
- `User(N+2)`: clear current profile bond info
- `User(N+3)`: switch default output between USB/BLE

Vial also provides a way to customize the displayed keycode, see `customKeycodes` in [this example](https://github.com/HaoboGu/rmk/blob/main/examples/use_rust/nrf52840_ble/vial.json). If `customKeycodes` are configured, the `User0` ~ `User(N+3)` will be displayed as `BT0`, ..., `Switch Output`.

If you've connected a host to a profile, other devices will not be able to connect to this profile without manually clearing it first.

## BLE Passkey Entry

When pairing with a new host device over BLE, the host may request a passkey for secure pairing.

This feature is enabled by default. When the host requests a passkey during pairing, the keyboard enters passkey mode:

1. The host displays a 6-digit code on screen.
2. The keyboard intercepts all keypresses - no input is sent to the host while passkey mode is active.
3. Type the 6-digit passkey using the number keys (top row or numpad).
4. Press **Enter** to submit the passkey.
5. Press **Escape** to cancel pairing.
6. Press **Backspace** to delete the last entered digit.

If the passkey is not entered within the timeout period (default: 120 seconds), passkey mode is automatically cancelled. The timeout resets with each digit or backspace keypress, so it only expires after a period of inactivity.

### Requirements

The `passkey_entry` Cargo feature must be enabled. This is automatically included when you enable any BLE feature gate (e.g., `nrf52840_ble`).

### Configuration

Passkey entry can be configured in `keyboard.toml` under the `[ble]` section. See [Configuration/Wireless](../configuration/wireless.md#passkey-entry) for details.

## Wireless Split Support

RMK also supports wireless split keyboards, where one of the splits acts as the central and the other splits act as peripherals. RMK also supports heterogeneous wireless split configurations; for example, you can use an ESP32S3 as the central and an nRF52 as a peripheral.

RMK provides many split keyboard examples in the examples folder. Check out the examples that end with `_split`.

For the configuration details, please refer to [Configuration/Wireless](../configuration/wireless.md) section.
