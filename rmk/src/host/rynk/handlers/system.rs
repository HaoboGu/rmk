//! System handlers — handshake, reboot, bootloader jump, storage reset.

use rmk_types::constants;
use rmk_types::protocol::rynk::{DeviceCapabilities, ProtocolVersion, RynkError, StorageResetMode};

use super::super::RynkService;

impl<'a> RynkService<'a> {
    pub(crate) async fn handle_get_version(&self, payload: &mut [u8]) -> Result<usize, RynkError> {
        Self::write_response(&ProtocolVersion::CURRENT, payload)
    }

    pub(crate) async fn handle_get_capabilities(&self, payload: &mut [u8]) -> Result<usize, RynkError> {
        let (rows, cols, num_layers) = self.ctx.keymap_dimensions();
        let caps = DeviceCapabilities {
            // -- Layout (live, from the configured keymap) --
            num_layers: num_layers as u8,
            num_rows: rows as u8,
            num_cols: cols as u8,

            // -- Input device limits (compile-time from keyboard.toml) --
            num_encoders: 0, // TODO Phase 6: surface encoder count
            max_combos: constants::COMBO_MAX_NUM as u8,
            max_combo_keys: constants::COMBO_MAX_LENGTH as u8,
            max_macros: 0, // macro slots are implicit in MACRO_SPACE_SIZE
            macro_space_size: constants::MACRO_SPACE_SIZE as u16,
            max_morse: constants::MORSE_MAX_NUM as u8,
            max_patterns_per_key: constants::MAX_PATTERNS_PER_KEY as u8,
            max_forks: constants::FORK_MAX_NUM as u8,

            // -- Feature flags --
            storage_enabled: cfg!(feature = "storage"),
            lighting_enabled: false, // TODO Phase 6: surface light_service

            // -- Connectivity --
            is_split: cfg!(feature = "split"),
            num_split_peripherals: constants::SPLIT_PERIPHERALS_NUM as u8,
            ble_enabled: cfg!(feature = "_ble"),
            num_ble_profiles: constants::NUM_BLE_PROFILE as u8,

            // -- Protocol limits --
            max_payload_size: rmk_types::protocol::rynk::RYNK_MAX_PAYLOAD as u16,
            max_bulk_keys: bulk_size() as u8,
            macro_chunk_size: constants::MACRO_DATA_SIZE as u16,
            bulk_transfer_supported: cfg!(feature = "bulk_transfer"),
        };
        Self::write_response(&caps, payload)
    }

    pub(crate) async fn handle_reboot(&self, payload: &mut [u8]) -> Result<usize, RynkError> {
        // Synchronous reset — function never returns on real hardware. The
        // wire response is moot; the host treats post-Reboot disconnect as
        // success. On std/test targets the call falls through and we send
        // an Ok envelope so loopback tests complete.
        crate::boot::reboot_keyboard();
        Self::write_response(&(), payload)
    }

    pub(crate) async fn handle_bootloader_jump(&self, payload: &mut [u8]) -> Result<usize, RynkError> {
        crate::boot::jump_to_bootloader();
        Self::write_response(&(), payload)
    }

    pub(crate) async fn handle_storage_reset(&self, payload: &mut [u8]) -> Result<usize, RynkError> {
        let (_mode, _) =
            postcard::take_from_bytes::<StorageResetMode>(payload).map_err(|_| RynkError::InvalidRequest)?;
        // KeyboardContext::reset_storage() does not currently honor the
        // `LayoutOnly` mode (always Full). Phase 6 wires mode-aware reset.
        self.ctx.reset_storage().await;
        Self::write_response(&(), payload)
    }
}

/// Bulk-size constant under feature gate. `bulk_transfer` enables the
/// `bulk` feature in `rmk-types` which emits `constants::BULK_SIZE`.
fn bulk_size() -> usize {
    #[cfg(feature = "bulk_transfer")]
    {
        constants::BULK_SIZE
    }
    #[cfg(not(feature = "bulk_transfer"))]
    {
        0
    }
}
