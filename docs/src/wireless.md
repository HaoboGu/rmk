# Wireless

RMK has built-in wireless(BLE) support for nRF52840 and nRF52832. Supporting of other nRF52 microcontrollers should be quite easy to add. To use the wireless feature, enable `nrf52832_ble` or `nrf52840_ble` feature gate in your `Cargo.toml`:

```toml
rmk = { version = "0.1.15", features = [
    "nrf52840_ble",
    "col2row",
] }
```

RMK also provides ble examples, check [nrf52840_ble](https://github.com/HaoboGu/rmk/tree/main/examples/use_rust/nrf52840_ble) and [nrf52832_ble](https://github.com/HaoboGu/rmk/tree/main/examples/use_rust/nrf52832_ble).

Due to multiple targets are not supported by `docs.rs` right now, so API documentations are not there. Check examples for the usage. I'll add a separate doc site later.

## Flashing to your board

RMK can be flashed via a debug probe or USB. Follow the instruction in the [`examples/use_rust/nrf52840_ble/README.md`](https://github.com/HaoboGu/rmk/blob/main/examples/use_rust/nrf52840_ble/README.md)

## Nice!nano support

RMK has special support for [nice!nano](https://nicekeyboards.com/), a widely used board for building wireless keyboard.

nice!nano has a built-in bootloader, enables flashing a .uf2 format firmware via USB drive. [`examples/use_rust/nrf52840_ble/README.md`](https://github.com/HaoboGu/rmk/blob/main/examples/use_rust/nrf52840_ble/README.md) provides instructions to convert RMK firmware to .uf2 format and flash to nice!nano.

There is another way to flash RMK firmware to nice!nano. It requires a modified version of `elf2uf2-rs`. The following are the steps:

1. Install `elf2uf2-rs` from <https://github.com/simmsb/elf2uf2-rs>:
   ```
   cargo install --git https://github.com/simmsb/elf2uf2-rs
   ```
2. Enter nice!nano's bootloader mode, a USB drive should appear in your machine
3. Check the softdevice version of your nice!nano. If it's v6.x.x, edit `memory.x`:
   ```diff
   - FLASH : ORIGIN = 0x00027000, LENGTH = 868K
   + FLASH : ORIGIN = 0x00026000, LENGTH = 872K
   ```
4. Update cargo runner in `.cargo/config.toml`, using `elf2uf2-rs`:
    ```diff
    [target.'cfg(all(target_arch = "arm", target_os = "none"))']
    - runner = "probe-rs run --chip nRF52840_xxAA"
    + runner = "elf2uf2-rs -d"
    ```
5. Flash using `cargo run --release`