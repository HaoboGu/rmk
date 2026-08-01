# Storage

RMK's storage system provides persistent flash memory for storing data like keyboard configurations and BLE bonding information.

## Storage Feature

RMK's storage system is enabled by the `storage` feature, which is part of the default feature set. Enabling BLE automatically pulls in `storage`, since BLE bonding data must be persisted to non-volatile storage. The host configurator protocols (`rynk` and `vial`) rely on `storage` to persist keymap edits across reboots but do not enable it themselves, so keep it enabled when you use them.

## Storage Configuration

By default, RMK saves data to your microcontroller's internal flash memory.

- For users configuring with `keyboard.toml`, the default storage space details are located in the `rmk-config/src/default_config` folder. If your microcontroller's configuration isn't found there, RMK defaults to using the **last two flash sections** of your microcontroller's internal flash memory.

- For Rust API users, you can configure storage via the `RmkConfig.storage_config` field, which accepts a `StorageConfig` struct.

::: warning
Ensure you allocate sufficient storage space for your keymap and bonding information. 32 KiB is generally adequate for most keyboards.
:::
