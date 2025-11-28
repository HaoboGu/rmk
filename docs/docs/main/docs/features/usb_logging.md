# USB Logging

RMK uses [defmt](https://defmt.ferrous-systems.com) as the default logger, which works great if you have a debug probe. If you don’t have a debug probe, you can still view logs over USB by configuring USB as a serial port.

## Usage

To enable USB logging, disable the default features and then enable the `usb_log` feature in `Cargo.toml`:

```toml
rmk = { version = "...", default-features = false, features = [
    "storage",
    "usb_log", # Enable USB logging
    "..",
] }
```

::: tip
Don't forget to enable all other features that you need, especially the default ones.
:::

To view the logs, you’ll need to install a serial port monitor. Open your serial monitor, select the port corresponding to your keyboard, and connect. The logs will be displayed in the monitor window. Note that logs from the boot stage cannot be captured by the USB logger. You will only be able to see logs after the serial port connection established.

Some microcontrollers (like ESP32S3) doesn't have enough USB endpoints, so USB logging cannot enabled for those microcontrollers. To enable the USB logging, make sure that your microcontroller has at least 5 In + 4 OUT endpoints available(except control endpoint, EP0)
