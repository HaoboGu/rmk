# USB Logging

::: warning

Some microcontrollers (like ESP32S3) does't have enough USB endpoints, so USB logging cannot enabled for those microcontrollers. To enable the USB logging, make sure that your microcontroller has at least 5 In + 4 OUT endpoints available(except control endpoint, EP0)

:::

RMK uses [defmt](https://defmt.ferrous-systems.com) as the default logger, which works great if you have a debug probe. However, if you don’t have a debug probe, you can still view logs over USB by configuring USB as a serial port. RMK supports USB logging by enabling the `usb_log` feature:

```toml
rmk = { version = "0.7", default-features = false, features = [
    "col2row", 
    "storage",
    "usb_log", # <- enable USB logging
    "..",
] }
```

::: tip

The `usb_log` feature cannot be used together with the `defmt` feature, which is enabled by default. To use USB logging, set `default-features = false` and manually enable any other default features you need, such as `col2row` and `storage`.

:::

To view the logs, you’ll need to install a serial port monitor. Open your serial monitor, select the port corresponding to your keyboard, and connect. The logs will be displayed in the monitor window.

Note: Logs from the boot stage cannot be captured by the USB logger. You will only see logs after the serial port connection
