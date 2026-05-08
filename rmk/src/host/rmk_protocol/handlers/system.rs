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
    DeviceCapabilities {
        num_layers: layers as u8,
        num_rows: rows as u8,
        num_cols: cols as u8,
        num_encoders: 0,
        max_combos: 0,
        max_combo_keys: 0,
        max_macros: 0,
        macro_space_size: crate::MACRO_SPACE_SIZE as u16,
        max_morse: 0,
        max_patterns_per_key: 0,
        max_forks: 0,
        storage_enabled: cfg!(feature = "storage"),
        lighting_enabled: false,
        is_split: cfg!(feature = "split"),
        num_split_peripherals: 0,
        ble_enabled: cfg!(feature = "_ble"),
        num_ble_profiles: 0,
        max_payload_size: 0,
        max_bulk_keys: 0,
        macro_chunk_size: 0,
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
