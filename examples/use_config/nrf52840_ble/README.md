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
2. Compile RMK using `cargo objcopy`, get .bin firmware:
   ```shell
   cargo objcopy --release -- -O binary rmk-52840.bin
   ```
3. Download uf2util from UF2 repo https://github.com/microsoft/uf2
   ```shell
   git clone https://github.com/microsoft/uf2.git
   cd uf2/utils
   ``` 
4. Convert your .hex firmware to uf2 format
   ```shell
   # If your nice!nano uses softdevice v6.x.x
   python uf2conv.py <PATH_TO_YOUR_HEX_FIRMWARE> -c -b 0x26000 -f 0xADA52840 -o rmk-52840.uf2 
   # If your nice!nano uses softdevice v7.x.x
   python uf2conv.py <PATH_TO_YOUR_HEX_FIRMWARE> -c -b 0x27000 -f 0xADA52840 -o rmk-52840.uf2 
   ```
5. Flash

   Set your nice!nano to bootloader mode, a USB drive will show. Just drag the .uf2 firmware to USB drive. RMK will be automatically flashed. Check nice!nano's document: https://nicekeyboards.com/docs/nice-nano/getting-started#flashing-firmware-and-bootloaders. 

Note that RMK will switch to USB mode if an USB cable is connected. Remember to remove USB cable after flashing!

You can also check the instruction [here](https://nicekeyboards.com/docs/nice-nano/) for more info about nice!nano.

## With debugging probe
With a debugging probe, you can have the full control of you hardware. To use RMK you should have [nrf s140 softdevice 7.3.0](https://www.nordicsemi.com/Products/Development-software/s140/download) flashed to nrf52840 first. 

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
   cargo run --release
   ```
