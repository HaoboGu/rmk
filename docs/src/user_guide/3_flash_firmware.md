# Flash the firmware

The last step is to flash compiled firmware to your microcontroller. RMK supports flashing the firmware via uf2 bootloader or debug probe. 

## Use uf2 bootloader

Flashing using uf2 bootloader is easy: set your board to bootloader mode, then a USB drive should appear in your computer. Copy the uf2 firmware to the USB drive, and that's it!

If you're using macOS, an error might appear, you can safely ignore it.

### Tips for nRF52840

For nRF52840, there are several widely used UF2 bootloaders, they require slight different configs.

First, you should check the used softdevice version of your bootloader. Enter bootloader mode, there will be an USB driver shown in your computer. Open `INFO_UF2.TXT` in the USB driver, the content of `INFO_UF2.TXT` should be like:

```
UF2 Bootloader 0.6.0 lib/nrfx (v2.0.0) lib/tinyusb (0.10.1-41-gdf0cda2d) lib/uf2 (remotes/origin/configupdate-9-gadbb8c7)
Model: nice!nano
Board-ID: nRF52840-nicenano
SoftDevice: S140 version 6.1.1
Date: Jun 19 2021
```

As you can see, the version of softdevice is `S140 version 6.1.1`. For nRF52840, RMK supports S140 version 6.X and 7.X. The `memory.x` config is slightly different for softdevice 6.X and 7.X:

```ld
MEMORY
{
  /* These values correspond to the NRF52840 with Softdevices S140 6.1.1 */
  /* FLASH : ORIGIN = 0x00026000, LENGTH = 824K */

  /* These values correspond to the NRF52840 with Softdevices S140 7.3.0 */
  FLASH : ORIGIN = 0x00027000, LENGTH = 820K
  RAM : ORIGIN = 0x20020000, LENGTH = 128K
}
```

You can edit your `memory.x` to choose correct value for your bootloader.

## Use debug probe

If you have a debug probe like [daplink](https://daplink.io/), [jlink](https://www.segger.com/products/debug-probes/j-link/) or [stlink](https://github.com/stlink-org/stlink)(stm32 only), things become much easier: connect it with your board and host, make sure you have installed [probe-rs](https://probe.rs/), then just run

```shell
cargo run --release
```

Then the command configured in `.cargo/config.toml` will be executed. The firmware will be flashed to your microcontroller and run automatically, yay!

For more configurations of RMK, you can check out feature documentations on the left.