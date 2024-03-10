# nrf52840 BLE example

To use this example, you should have [nrf s140 softdevice 7.3.0](https://www.nordicsemi.com/Products/Development-software/s140/download) flashed to nrf52840 first. 

The following are the detailed steps:

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
