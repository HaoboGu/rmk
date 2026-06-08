//! System handlers — handshake, reboot, bootloader jump, storage reset.

use rmk_types::constants;
use rmk_types::protocol::rynk::{
    DeviceCapabilities, ProtocolVersion, RYNK_HEADER_SIZE, RynkError, RynkMessage, StorageResetMode,
};

use super::super::RynkService;

impl<'a> RynkService<'a> {
    pub(crate) async fn handle_get_version(&self, msg: &mut RynkMessage<'_>) -> Result<usize, RynkError> {
        Self::write_response(&ProtocolVersion::CURRENT, msg.response_payload_mut())
    }

    pub(crate) async fn handle_get_capabilities(&self, msg: &mut RynkMessage<'_>) -> Result<usize, RynkError> {
        let (rows, cols, num_layers) = self.ctx.keymap_dimensions();
        let caps = DeviceCapabilities {
            // Layout (live, from the configured keymap)
            num_layers: num_layers as u8,
            num_rows: rows as u8,
            num_cols: cols as u8,

            // Input device limits (compile-time from keyboard.toml)
            num_encoders: self.ctx.num_encoders() as u8,
            max_combos: constants::COMBO_MAX_NUM as u8,
            max_combo_keys: constants::COMBO_MAX_LENGTH as u8,
            max_macros: 0, // macro slots are implicit in MACRO_SPACE_SIZE
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
            // The real per-frame acceptance ceiling: this build's session
            // buffer minus the header — not the compile-time protocol bound.
            max_payload_size: (constants::RYNK_BUFFER_SIZE - RYNK_HEADER_SIZE) as u16,
            macro_chunk_size: constants::MACRO_DATA_SIZE as u16,
            // Bulk transfer reads as fully absent — flag AND count — until
            // the bulk handlers are implemented; today they all answer
            // `Unimplemented`, and advertising support would send hosts
            // straight into that wall. Restore `cfg!(feature =
            // "bulk_transfer")` and `constants::BULK_SIZE` with the handlers.
            max_bulk_keys: 0,
            bulk_transfer_supported: false,
        };
        Self::write_response(&caps, msg.response_payload_mut())
    }

    pub(crate) async fn handle_reboot(&self, msg: &mut RynkMessage<'_>) -> Result<usize, RynkError> {
        // Fire-and-forget: synchronous reset never returns on real hardware,
        // and there's no way to guarantee a response makes it onto the wire
        // before the reset takes effect — the host doesn't await one (its
        // `reboot()` is send-only). On targets where the reset is a no-op,
        // `reboot_keyboard` returns and a proper `Ok(())` envelope goes out
        // (a bare empty payload would fail the host's `Result` decode).
        crate::boot::reboot_keyboard();
        Self::write_response(&(), msg.response_payload_mut())
    }

    pub(crate) async fn handle_bootloader_jump(&self, msg: &mut RynkMessage<'_>) -> Result<usize, RynkError> {
        // Fire-and-forget, same reasoning as `handle_reboot`.
        crate::boot::jump_to_bootloader();
        Self::write_response(&(), msg.response_payload_mut())
    }

    pub(crate) async fn handle_storage_reset(&self, msg: &mut RynkMessage<'_>) -> Result<usize, RynkError> {
        let mode = msg.request::<StorageResetMode>()?;
        // KeyboardContext::reset_storage() performs a full wipe (including
        // BLE bonds). Until Phase 6 wires mode-aware reset, honor only the
        // mode whose semantics we actually implement — silently full-wiping
        // on a `LayoutOnly` request would destroy bonds the host promised to
        // keep.
        if mode != StorageResetMode::Full {
            return Err(RynkError::Unimplemented);
        }
        self.ctx.reset_storage().await;
        Self::write_response(&(), msg.response_payload_mut())
    }
}
