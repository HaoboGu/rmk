# USB Logging

RMK uses [defmt](https://defmt.ferrous-systems.com) as the default logger, which is great when you have a debug probe. But if you don't have a debug probe, it's also possible to see the logs from USB -- by making USB a serial port. RMK supports logging from USB by enabling `usb_log` feature:

```toml
rmk = { version = "0.7", default-features = false, features = [
    "col2row", 
    "storage",
    "usb_log", // <- enable usb logging
    "..",
] }
```

::: tip

`usb_log` feature cannot be used together with `defmt` feature, which is a default feature. So to use USB logger, you need to set `default-features = false`, and enable other used default features, such as `col2row` and `storage`.

:::