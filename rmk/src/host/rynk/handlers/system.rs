//! System handlers — handshake, device identity, reboot, bootloader jump, storage reset.

use rmk_types::constants;
use rmk_types::protocol::rynk::command::{
    BootloaderJump, GetCapabilities, GetDeviceInfo, GetLockStatus, GetVersion, Lock, Reboot, StorageReset, UnlockPoll,
};
use rmk_types::protocol::rynk::{
    DEVICE_INFO_STRING_SIZE, DeviceCapabilities, DeviceInfo, LockStatus, ProtocolVersion, RYNK_HEADER_SIZE, RynkError,
    StorageResetMode,
};

use super::super::{RMK_VERSION, RynkService};
use super::Handle;

impl Handle<GetVersion> for RynkService<'_> {
    async fn handle(&self, _: ()) -> Result<ProtocolVersion, RynkError> {
        Ok(ProtocolVersion::CURRENT)
    }
}

impl Handle<GetCapabilities> for RynkService<'_> {
    async fn handle(&self, _: ()) -> Result<DeviceCapabilities, RynkError> {
        let (rows, cols, num_layers) = self.ctx.keymap_dimensions();
        Ok(DeviceCapabilities {
            // Layout (live, from the configured keymap)
            num_layers: num_layers as u8,
            num_rows: rows as u8,
            num_cols: cols as u8,

            // Input device limits (compile-time from keyboard.toml)
            num_encoders: self.ctx.num_encoders() as u8,
            max_combos: constants::COMBO_MAX_NUM as u8,
            max_combo_keys: constants::COMBO_MAX_LENGTH as u8,
            // TODO: make this a user-defined constant in keyboard.toml ([rmk] section).
            max_macros: 16,
            macro_space_size: constants::MACRO_SPACE_SIZE as u16,
            max_morse: constants::MORSE_MAX_NUM as u8,
            max_patterns_per_key: constants::MAX_PATTERNS_PER_KEY as u8,
            max_forks: constants::FORK_MAX_NUM as u8,

            // Feature flags
            storage_enabled: cfg!(feature = "storage"),
            lighting_enabled: false, // TODO Phase 6: surface light_service

            // Connectivity
            is_split: cfg!(feature = "split"),
            num_split_peripherals: constants::SPLIT_PERIPHERALS_NUM as u8,
            ble_enabled: cfg!(feature = "_ble"),
            num_ble_profiles: constants::NUM_BLE_PROFILE as u8,

            // Protocol limits
            max_payload_size: (constants::RYNK_BUFFER_SIZE - RYNK_HEADER_SIZE) as u16,
            macro_chunk_size: constants::MACRO_DATA_SIZE as u16,
            // The BULK_* constants only exist under `bulk`, hence #[cfg] over cfg!().
            #[cfg(feature = "bulk")]
            max_bulk_keys: constants::BULK_KEYMAP_SIZE as u8,
            #[cfg(not(feature = "bulk"))]
            max_bulk_keys: 0,
            #[cfg(feature = "bulk")]
            max_bulk_configs: constants::BULK_SIZE as u8,
            #[cfg(not(feature = "bulk"))]
            max_bulk_configs: 0,
            bulk_transfer_supported: cfg!(feature = "bulk"),
        })
    }
}

impl Handle<Reboot> for RynkService<'_> {
    async fn handle(&self, _: ()) -> Result<(), RynkError> {
        // Fire-and-forget: synchronous reset never returns on real hardware.
        crate::boot::reboot_keyboard();
        Ok(())
    }
}

impl Handle<BootloaderJump> for RynkService<'_> {
    async fn handle(&self, _: ()) -> Result<(), RynkError> {
        // Fire-and-forget, same reasoning as `Reboot`.
        crate::boot::jump_to_bootloader();
        Ok(())
    }
}

impl Handle<StorageReset> for RynkService<'_> {
    async fn handle(&self, mode: StorageResetMode) -> Result<(), RynkError> {
        if mode != StorageResetMode::Full {
            // TODO: Reset required storage range
            return Err(RynkError::Unimplemented);
        }
        self.ctx.reset_storage().await;
        Ok(())
    }
}

// ── Lock gate ── all three stay dispatchable while locked (see the gate in
// `RynkService::dispatch`).

impl Handle<GetLockStatus> for RynkService<'_> {
    async fn handle(&self, _: ()) -> Result<LockStatus, RynkError> {
        Ok(self.locker.status())
    }
}

impl Handle<UnlockPoll> for RynkService<'_> {
    async fn handle(&self, _: ()) -> Result<LockStatus, RynkError> {
        Ok(self.locker.poll())
    }
}

impl Handle<Lock> for RynkService<'_> {
    async fn handle(&self, _: ()) -> Result<(), RynkError> {
        self.locker.lock();
        Ok(())
    }
}

impl Handle<GetDeviceInfo> for RynkService<'_> {
    async fn handle(&self, _: ()) -> Result<DeviceInfo, RynkError> {
        Ok(DeviceInfo {
            rmk_version: RMK_VERSION,
            vendor_id: self.device.vid,
            product_id: self.device.pid,
            manufacturer: truncated(self.device.manufacturer),
            product_name: truncated(self.device.product_name),
            serial_number: truncated(self.device.serial_number),
        })
    }
}

/// Copy `s` into the bounded wire string; over-long input is cut at the last
/// whole char that fits, so multi-byte content can never panic or split.
fn truncated(s: &str) -> heapless::String<DEVICE_INFO_STRING_SIZE> {
    let mut out = heapless::String::new();
    for c in s.chars() {
        if out.push(c).is_err() {
            break;
        }
    }
    out
}
