# Storage

RMK's storage system provides persistent flash memory for storing data like keyboard configurations and BLE bonding information.

## Storage Feature

RMK's storage system is enabled by `storage` feature. Enabling features related to `vial` and `ble` automatically enables the `storage` feature because they require keymap and BLE bonding data to be persisted to non-volatile storage

## Storage Configuration

By default, RMK saves data to your microcontroller's internal flash memory.

- For users configuring with `keyboard.toml`, the default storage space details are located in the `rmk-config/src/default_config` folder. If your microcontroller's configuration isn't found there, RMK defaults to using the **last two flash sections** of your microcontroller's internal flash memory.

- For Rust API users, you can configure storage via the `RmkConfig.storage_config` field, which accepts a `StorageConfig` struct.


::: warning
Ensure you allocate sufficient storage space for your keymap and bonding information. 32KiB is generally adequate for most keyboards. 
:::