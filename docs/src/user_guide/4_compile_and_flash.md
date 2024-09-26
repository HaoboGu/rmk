# Compile and flash!

In this section, you'll be able to compile your firmware and flash it to your microcontroller.

## Compile the firmware

To compile the firmware is easy, just run

```shell
cargo build --release
```

If you've done all the previous steps correctly, you can find your compiled firmware at `target/<your_target>/release` folder.

If you encountered any problems when compiling the firmware, please report it [here](https://github.com/HaoboGu/rmk/issues).

## Flash the firmware

The last step is to flash compiled firmware to your microcontroller. RMK supports flashing the firmware via uf2 bootloader or debug probe. [Here](https://github.com/HaoboGu/rmk/tree/main/examples/use_config/nrf52840_ble#nicenano-support) is an example for using nice!nano and converting and flashing uf2 firmware. 

If you have a debug probe like [daplink](https://daplink.io/), [jlink](https://www.segger.com/products/debug-probes/j-link/) or [stlink](https://github.com/stlink-org/stlink)(stm32 only), things become much easier: connect it with your board and host, make sure you have installed [probe-rs](https://probe.rs/), then just run

```shell
cargo run --release
```

The firmware will be flashed to your microcontroller and the firmware will run automatically, yay!

For more configurations of RMK, you can check out feature documentations on the left.