//! Handlers for the `sys/*` endpoint group.
//!
//! Lock-related endpoints (`GetLockStatus`, `UnlockRequest`, `LockRequest`)
//! are stubbed in v1 — see plan §3.7 / project memory
//! `rmk_protocol_lock_deferred`. The wire keys stay frozen in the rmk-types
//! snapshots; the v2 follow-up that resurrects the gate replaces the stubs.

use postcard_rpc::header::VarHeader;
use rmk_types::protocol::rmk::{DeviceCapabilities, LockStatus, ProtocolVersion, StorageResetMode, UnlockChallenge};

use super::super::Ctx;
use crate::boot;
#[cfg(feature = "storage")]
use crate::{channel::FLASH_CHANNEL, storage::FlashOperationMessage};

pub(crate) async fn get_version(_ctx: &mut Ctx<'_>, _hdr: VarHeader, _req: ()) -> ProtocolVersion {
    ProtocolVersion::CURRENT
}

pub(crate) async fn get_capabilities(ctx: &mut Ctx<'_>, _hdr: VarHeader, _req: ()) -> DeviceCapabilities {
    let (rows, cols, layers) = ctx.keymap.get_keymap_config();

    // `BULK_SIZE` is only emitted by `rmk-types/build.rs` when the `bulk`
    // feature is on (rmk's `bulk_transfer` pulls `rmk-types/bulk`).
    #[cfg(feature = "bulk_transfer")]
    let max_bulk_keys = rmk_types::constants::BULK_SIZE.min(u8::MAX as usize) as u8;
    #[cfg(not(feature = "bulk_transfer"))]
    let max_bulk_keys: u8 = 0;

    DeviceCapabilities {
        num_layers: layers as u8,
        num_rows: rows as u8,
        num_cols: cols as u8,
        num_encoders: ctx.keymap.num_encoders().min(u8::MAX as usize) as u8,
        max_combos: crate::COMBO_MAX_NUM.min(u8::MAX as usize) as u8,
        max_combo_keys: crate::COMBO_MAX_LENGTH.min(u8::MAX as usize) as u8,
        // RMK stores macros as a packed sequence buffer rather than a fixed
        // slot count; hosts should consult `macro_space_size` for the cap.
        max_macros: 0,
        macro_space_size: crate::MACRO_SPACE_SIZE.min(u16::MAX as usize) as u16,
        max_morse: crate::MORSE_MAX_NUM.min(u8::MAX as usize) as u8,
        max_patterns_per_key: crate::MAX_PATTERNS_PER_KEY.min(u8::MAX as usize) as u8,
        max_forks: crate::FORK_MAX_NUM.min(u8::MAX as usize) as u8,
        storage_enabled: cfg!(feature = "storage"),
        lighting_enabled: false,
        is_split: cfg!(feature = "split"),
        num_split_peripherals: crate::SPLIT_PERIPHERALS_NUM.min(u8::MAX as usize) as u8,
        ble_enabled: cfg!(feature = "_ble"),
        num_ble_profiles: {
            #[cfg(feature = "_ble")]
            {
                crate::NUM_BLE_PROFILE.min(u8::MAX as usize) as u8
            }
            #[cfg(not(feature = "_ble"))]
            {
                0
            }
        },
        // Both the USB and BLE wire transports buffer one full frame in a 512-byte
        // RX scratch (see entry_usb.rs::USB_RX_BUF_LEN, entry_ble.rs::BLE_RX_BUF).
        max_payload_size: 512,
        max_bulk_keys,
        macro_chunk_size: rmk_types::constants::MACRO_DATA_SIZE.min(u16::MAX as usize) as u16,
        bulk_transfer_supported: cfg!(feature = "bulk_transfer"),
    }
}

// --- v1 lock stubs (per plan §3.7) ---

pub(crate) async fn get_lock_status(_ctx: &mut Ctx<'_>, _hdr: VarHeader, _req: ()) -> LockStatus {
    LockStatus {
        locked: false,
        awaiting_keys: false,
        remaining_keys: 0,
    }
}

pub(crate) async fn unlock_request(_ctx: &mut Ctx<'_>, _hdr: VarHeader, _req: ()) -> UnlockChallenge {
    UnlockChallenge {
        key_positions: heapless::Vec::new(),
    }
}

pub(crate) async fn lock_request(_ctx: &mut Ctx<'_>, _hdr: VarHeader, _req: ()) {
    // No-op in v1; lock gate deferred to v2.
}

// --- end lock stubs ---

pub(crate) async fn reboot(_ctx: &mut Ctx<'_>, _hdr: VarHeader, _req: ()) {
    boot::reboot_keyboard();
}

pub(crate) async fn bootloader_jump(_ctx: &mut Ctx<'_>, _hdr: VarHeader, _req: ()) {
    boot::jump_to_bootloader();
}

pub(crate) async fn storage_reset(_ctx: &mut Ctx<'_>, _hdr: VarHeader, mode: StorageResetMode) {
    #[cfg(feature = "storage")]
    {
        let msg = match mode {
            StorageResetMode::Full => FlashOperationMessage::Reset,
            StorageResetMode::LayoutOnly => FlashOperationMessage::ResetLayout,
            // `StorageResetMode` is `#[non_exhaustive]`; future variants no-op
            // until the firmware learns to handle them.
            _ => return,
        };
        FLASH_CHANNEL.send(msg).await;
    }
    #[cfg(not(feature = "storage"))]
    {
        let _ = mode;
    }
}
