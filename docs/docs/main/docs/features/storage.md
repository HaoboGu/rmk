# Storage

RMK's storage system provides persistent flash memory for storing data like keyboard configurations and BLE bonding information.

## Storage Feature

RMK's storage system is enabled by the `storage` feature, which is part of the default feature set. Enabling BLE automatically pulls in `storage`, since BLE bonding data must be persisted to non-volatile storage. The host configurator protocols (`rynk` and `vial`) rely on `storage` to persist keymap edits across reboots but do not enable it themselves, so keep it enabled when you use them.

## Storage Configuration

By default, RMK saves data to your microcontroller's internal flash memory.

- For users configuring with `keyboard.toml`, the default storage space details are located in the `rmk-config/src/default_config` folder. If your microcontroller's configuration isn't found there, RMK defaults to using the **last two flash sectors** of your microcontroller's internal flash memory.

- For Rust API users, create a `StorageConfig` struct and pass it to `initialize_keymap_and_storage`, which sets up the storage from your flash peripheral:

```rust
use rmk::config::{BehaviorConfig, PositionalConfig, StorageConfig};
use rmk::{KeymapData, initialize_keymap_and_storage};

let storage_config = StorageConfig::default();
let mut behavior_config = BehaviorConfig::default();
let per_key_config = PositionalConfig::default();
let mut keymap_data = KeymapData::new(keymap::get_default_keymap());
let (keymap, mut storage) = initialize_keymap_and_storage(
    &mut keymap_data,
    flash,
    &storage_config,
    &mut behavior_config,
    &per_key_config,
)
.await;

// `storage` is a runnable — pass it to `run_all!` with everything else
run_all!(matrix, storage, usb_transport, keyboard).await;
```

::: warning
Ensure you allocate sufficient storage space for your keymap and bonding information. 32 KiB is generally adequate for most keyboards.
:::

## Storage Is Cleared on Firmware Updates

Every firmware build embeds a unique build hash (computed in `rmk`'s build script from the git commit and build time). The hash is written to storage when storage is first initialized, and checked on every boot: if the stored hash doesn't match the running firmware's hash, RMK erases the storage and re-initializes it from the firmware's defaults.

This guard keeps storage consistent with the firmware — stored keymaps and configs always match the layout compiled into the running build. The consequence: **flashing new firmware clears all stored data**, including keymap edits made via Vial/Rynk and BLE bonding information, so you'll need to re-pair BLE hosts after a firmware update.
