# Migration Guide

## From v0.6.x to v0.7.x

RMK v0.7.x is a major update, including a lot of changes. One notable change is that RMK changed BLE stack to [`TrouBLE`](https://github.com/embassy-rs/trouble), a great BLE host implementation with async support and better compatibility.

Updating to `TrouBLE` brings lots of benefits, including:

- More microcontrollers are supported, such as cyw(used on Pi Pico W)
- We don't need to maintain different BLE stacks for different microcontrollers. That makes BLE split keyboards are supported automatically for ESP32 and Pi Pico W
- For ESP32, RMK migrates to Espressif's official [`esp-hal`](https://github.com/esp-rs/esp-hal), making it possible to use USB on ESP32S3. Now you can build a dual-mode keyboard using ESP32S3, with all the features of RMK!
- You can even build a split keyboard using different microcontrollers, for example, use ESP32 as a dongle and use nRF52 as the peripherals

The following is the step-by-step guide to update your project to v0.7.x.

::: tip

The following guide is for local compilation. If you are using [cloud compilation](./user_guide/2-1_cloud_compilation.md), you can skip the following steps and just rerun the github action.

:::

## 1. Update `rmkit`

```shell
cargo install rmkit --force
```

## 2. Update Cargo dependencies

Lot of dependencies are updated from v0.6.x to v0.7.x. 

The best updating approach is to copy the new `Cargo.toml` file from examples to replace the old one, delete the old `Cargo.lock` file, then tune the RMK features used and then re-build the project using

```shell
cargo update
cargo build --release
```

## 3. Add the path of `keyboard.toml` to `.cargo/config.toml`

In v0.7.x, RMK requries to set the path of `keyboard.toml` in `.cargo/config.toml`. This makes the path of `keyboard.toml` configurable.

In versions before v0.7.x, `keyboard.toml` is located in the root directory of the project. So you will need to set the following in `.cargo/config.toml` after updating to v0.7.x:

```toml
[env]
KEYBOARD_TOML_PATH =  { value = "keyboard.toml", relative = true }
```

## 4. Platform specific changes

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

### ESP32 BLE stack migration

Older versions of RMK uses [`esp-idf-hal`](https://github.com/esp-rs/esp-idf-hal), which is now community-maintained. RMK v0.7.x now uses Espressif's official [`esp-hal`](https://github.com/esp-rs/esp-hal). Because the difference between [`esp-hal`](https://github.com/esp-rs/esp-hal) and [`esp-idf-hal`](https://github.com/esp-rs/esp-idf-hal) is too large, the recommended way to migrate to v0.7.x is to recreate your project from scratch and migrate your keymap & configuration to the new project.

## 5. Check out new features

RMK v0.7.x brings lots of exciting features, making configuration easier and more flexible. Check out the [CHANGELOG](https://github.com/HaoboGu/rmk/blob/main/rmk/CHANGELOG.md) for more details.

## Troubleshooting

1. BLE peripheral doesn't work

    In the new version, RMK uses nRF chip's unique address as the device address. If BLE peripheral doesn't work after updating, it's likely that the old fixed device address is stored in the chip. You should [clear the storage](/docs/features/storage) for both central and peripheral:

    1. Set `clear_storage` to true for both peripheral & central
    2. Flash both central & peripheral firmware
    3. Set `clear_storage` back to false and compile
    4. Flash both splits again

## Known issues

1. The dongle setting & multiple peripherals is not working
