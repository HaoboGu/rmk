# nrf52832 BLE example

To use this example, you should have [nrf s132 softdevice 7.3.0](https://www.nordicsemi.com/Products/Development-software/s132/download) flashed to nrf52832 first. 

The following are the detailed steps:

1. Enter example folder:
   ```shell
   cd boards/nrf52832_ble
   ```
2. Erase the flash:
   ```shell
   probe-rs erase --chip nrf52832_xxAA
   ```
3. Flash softdevice firmware to flash:
   ```shell
   probe-rs download --verify --format hex --chip nRF52832_xxAA s132_nrf52_7.3.0_softdevice.hex
   ```
4. Compile, flash and run the example
   ```shell
   cargo run --release
   ```
