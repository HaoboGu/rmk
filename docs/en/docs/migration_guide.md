# Migration Guide

## From v0.6.x to v0.7.x

RMK v0.7.x is a major update, including a lot of changes. One notable change is that RMK changed BLE stack to [`TrouBLE`](https://github.com/embassy-rs/trouble), a great BLE host implementation with async support and better compatibility.

Updating to `TrouBLE` brings lots of benefits, including:

- More microcontrollers are supported, such as cyw(used on Pi Pico W)
- We don't need to maintain different BLE stacks for different microcontrollers. That makes BLE split keyboards are supported automatically for ESP32 and Pi Pico W
- For ESP32, RMK migrates to Espressif's official [`esp-hal`](https://github.com/esp-rs/esp-hal), making it possible to use USB on ESP32S3. Now you can build a dual-mode keyboard using ESP32S3, with all the features of RMK!
- You can even build a split keyboard using different microcontrollers, for example, use ESP32 as a dongle and use nRF52 as the peripherals

### nRF BLE stack migration

For nRF chips, now RMK uses Nordic latest SoftDevice Controller(sdc) in [`nrfxlib`](https://github.com/nrfconnect/sdk-nrfxlib) as the low-level BLE controller, brings better performance and stability, but it's not compatible with the old SoftDevice stack.

For more details about the difference between SoftDevice Controller and old SoftDevice, you can refer to [this article](https://devzone.nordicsemi.com/nordic/nordic-blog/b/blog/posts/nrf-connect-sdk-and-nrf5-sdk-statement).

::: danger

The new Nordic SoftDevice Controller will be compiled into the firmware. Flashing the new firmware will clear the old pre-flashed SoftDevice stack, so if you want to rollback to v0.6.x, or switch to firmwares that use SoftDevice stack(for example, zmk), you will need to [re-flash the bootloader](https://nicekeyboards.com/docs/nice-nano/troubleshooting#my-nicenano-seems-to-be-acting-up-and-i-want-to-re-flash-the-bootloader).

:::

The migration process is simple, you just need to:

1. Update RMK version in `Cargo.toml`
2. Update your memory.x according to whether you are using uf2 bootloader or not

    <!-- ::: code-group -->
    ```diff [With Adafruit nRF52 bootloader]
    // These values correspond to the nRF52840 WITH Adafruit nRF52 bootloader
    MEMORY
    {
    -  FLASH : ORIGIN = 0x00027000, LENGTH = 820K 
    -  RAM : ORIGIN = 0x20020000, LENGTH = 128K 
    +  FLASH : ORIGIN = 0x00001000, LENGTH = 1020K 
    +  RAM : ORIGIN = 0x20000008, LENGTH = 255K 
    }
    ```
    ```diff [Without Adafruit nRF52 bootloader]
    // These values correspond to the nRF52840 WITHOUT Adafruit nRF52 bootloader
    MEMORY
    {
    -  FLASH : ORIGIN = 0x00027000, LENGTH = 820K 
    -  RAM : ORIGIN = 0x20020000, LENGTH = 128K 
    +  FLASH : ORIGIN = 0x00000000, LENGTH = 1024K
    +  RAM : ORIGIN = 0x20000000, LENGTH = 256K
    }
    ```
    <!-- ::: -->

3. Compile your firmware and flash it to your controller

#### Troubleshooting

1. BLE peripheral doesn't work

    In the new version, RMK uses nRF chip's unique address as the device address. If BLE peripheral doesn't work after updating, it's likely that the old fixed device address is stored in the chip. You should [clear the storage](/docs/features/storage) for both central and peripheral:

    1. Set `clear_storage` to true for both peripheral & central
    2. Flash both central & peripheral firmware
    3. Set `clear_storage` back to false and compile
    4. Flash both splits again

### ESP32 BLE stack migration

Older versions of RMK uses [`esp-idf-hal`](https://github.com/esp-rs/esp-idf-hal), which is now community-maintained. RMK v0.7.x now uses Espressif's official [`esp-hal`](https://github.com/esp-rs/esp-hal). Because the difference between [`esp-hal`](https://github.com/esp-rs/esp-hal) and [`esp-idf-hal`](https://github.com/esp-rs/esp-idf-hal) is too large, the recommended way to migrate to v0.7.x is to recreate your project from scratch and migrate your keymap & configuration to the new project.
