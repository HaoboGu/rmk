# Flash the firmware

The last step is to flash compiled firmware to your microcontroller. RMK supports flashing the firmware via uf2 bootloader or debug probe.

## Use uf2 bootloader

Flashing using uf2 bootloader is easy: set your board to bootloader mode, then a USB drive should appear in your computer. Copy the uf2 firmware to the USB drive, and that's it!

If you're using macOS, an error might appear, you can safely ignore it.

### Tips for nRF52840

For nRF52840, you need to check whether your have a UF2 bootloader flashed to your board. If you can enter bootloader mode, there will be an USB drive shown in your computer. If there's `INFO_UF2.TXT` in the USB drive, you have it! Then check the `memory.x`, ensure that the flash origin starts with 0x00001000:

```
MEMORY
{
  /* NOTE 1 K = 1 KiB = 1024 bytes */
  /* These values correspond to the nRF52840 WITH Adafruit nRF52 bootloader */
  FLASH : ORIGIN = 0x00001000, LENGTH = 1020K
  RAM : ORIGIN = 0x20000008, LENGTH = 255K
}
```

If you have a debug probe and don't want to use the bootloader, use the following `memory.x` config:

```
{
  /* NOTE 1 K = 1 KiB = 1024 bytes */
  /* These values correspond to the nRF52840 WITHOUT Adafruit nRF52 bootloader */
  FLASH : ORIGIN = 0x00000000, LENGTH = 1024K
  RAM : ORIGIN = 0x20000000, LENGTH = 256K
}
```

## Use debug probe

If you have a debug probe like [daplink](https://daplink.io/), [jlink](https://www.segger.com/products/debug-probes/j-link/) or [stlink](https://github.com/stlink-org/stlink)(stm32 only), things become much easier: connect it with your board and host, make sure you have installed [probe-rs](https://probe.rs/), then just run

```shell
cargo run --release
```

Then the command configured in `.cargo/config.toml` will be executed. The firmware will be flashed to your microcontroller and run automatically, yay!

For more configurations of RMK, you can check out feature documentations on the left.
