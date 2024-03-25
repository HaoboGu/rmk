# nrf52840 BLE example

RMK supports [nice!nano](https://nicekeyboards.com/) as well as any custom nrf52840 board you have. 

## Nice!nano support

nice!nano has a bootloader built-in, which supports UF2 firmware format. That means you don't need any debugging probe to flash your firmware. 

If you're using nice!nano, there are steps of how to get .UF2 firmware of RMK:

1. Get `cargo-binutil` tool:
   ```shell
   cargo install cargo-binutils
   rustup component add llvm-tools
   ```
2. Compile RMK using `cargo objcopy`, get .hex firmware:
   ```shell
   cargo objcopy -- -O ihex rmk-52840.hex
   ```
3. Download uf2util from UF2 repo https://github.com/microsoft/uf2
   ```shell
   git clone https://github.com/microsoft/uf2.git
   cd uf2/utils
   ``` 
4. Convert your .hex firmware to uf2 format
   ```shell
   python uf2conv.py <PATH_TO_YOUR_HEX_FIRMWARE> -c -f 0xADA52840 -o rmk-52840.uf2 
   ```
5. Flash
   Check nice!nano's document: https://nicekeyboards.com/docs/nice-nano/getting-started#flashing-firmware-and-bootloaders

You can also check the instruction [here](https://nicekeyboards.com/docs/nice-nano/) for more info about nice!nano.

## With debugging probe
With a debugging probe, you can have the full control of you hardware. To use RMK you should have [nrf s140 softdevice 7.3.0](https://www.nordicsemi.com/Products/Development-software/s140/download) flashed to nrf52840 first. 

The following are the detailed steps for flashing both nrf's softdevice and RMK firmware:

1. Enter example folder:
   ```shell
   cd boards/nrf52840_ble
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
   cargo run --release
   ```
