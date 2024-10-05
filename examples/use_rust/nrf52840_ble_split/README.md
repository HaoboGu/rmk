# nrf52840 BLE split example

RMK supports [nice!nano](https://nicekeyboards.com/) as well as any custom nrf52840 board you have. 

## Build firmware

You can build firmware for central and peripheral separately:

```shell
# Build central firmware
cargo build --release --bin central

# Build peripheral firmware
cargo build --release --bin peripheral
```

## Nice!nano support

nice!nano has a bootloader built-in, which supports .uf2 firmware format. That means you don't need any debugging probe to flash your firmware. RMK uses `cargo-make` tool to generate .uf2 firmware, then generation processing is defined in `Makefile.toml`

The following are steps of how to get .uf2 firmware work in RMK:

1. Get `cargo-make` tool:
   ```shell
   cargo install --force cargo-make
   ```
2. Compile RMK and get .uf2:
   ```shell
   cargo make uf2 --release
   ```
3. Flash

   Set your nice!nano to bootloader mode, a USB drive will show. Just drag the .uf2 firmware to USB drive. RMK will be automatically flashed. Check nice!nano's document: https://nicekeyboards.com/docs/nice-nano/getting-started#flashing-firmware-and-bootloaders. 

Note that RMK will switch to USB mode if an USB cable is connected. Remember to remove USB cable after flashing!

You can also check the instruction [here](https://nicekeyboards.com/docs/nice-nano/) for more info about nice!nano.

## With debug probe
With a debug probe, you can have the full control of you hardware. To use RMK you should have [nrf s140 softdevice 7.3.0](https://www.nordicsemi.com/Products/Development-software/s140/download) flashed to nrf52840 first. 

The following are the detailed steps for flashing both nrf's softdevice and RMK firmware:

1. Enter example folder:
   ```shell
   cd examples/use_rust/nrf52840_ble
   ```
2. Erase the flash:
   ```shell
   probe-rs erase --chip nrf52840_xxAA
   ```
3. Flash softdevice firmware to flash:
   ```shell
   probe-rs download --verify --format hex --chip nRF52840_xxAA s140_nrf52_7.3.0_softdevice.hex
   ```
4. Compile, flash and run the example
   ```shell
   # Run central firmware
   cargo run --release --bin central

   # Run peripheral firmware
   cargo run --release --bin peripheral
   ```
