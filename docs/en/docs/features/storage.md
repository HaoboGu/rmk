# Storage

::: tip

RMK has default values for used flash area, check out the [default config](https://github.com/HaoboGu/rmk/tree/main/rmk-config/src/default_config) for your microcontroller. If you're using Rust API, it's recommended to use the default values.

:::

Storage feature is used by saving keymap edits to internal flash.

## Storage configuration

If you're using the `keyboard.toml`, you can set the storage using the following config:

```toml
[storage]
# Storage feature is enabled by default
enabled = true
# Start address of local storage, MUST BE start of a sector.
# If start_addr is set to 0(this is the default value), the last `num_sectors` sectors will be used.
start_addr = 0x00000000
# How many sectors are used for storage, the default value is 2
num_sectors = 2
# Clear storage at keyboard boot.
# Set it to true will reset the storage(including keymap, BLE bond info, etc.) at each reboot.
# This option is useful when testing the firmware.
clear_storage = false
```

You can also edit `storage_config` field in `RmkConfig` if you're using Rust API:

```rust
// https://github.com/HaoboGu/rmk/blob/main/examples/use_rust/nrf52832_ble/src/main.rs#L48

let storage_config = StorageConfig {
    start_addr: 0x70000,
    num_sectors: 2,
    ..Default::default()
};
let rmk_config = RmkConfig {
    usb_config: keyboard_usb_config,
    vial_config,
    storage_config,
    ..Default::default()
};

```

When manually setting the storage area, you have to ensure that you have enough flash space for storage feature. If there is not enough space, passing `None` is acceptable.
