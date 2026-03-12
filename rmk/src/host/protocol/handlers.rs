//! Endpoint handler functions for the RMK protocol dispatcher.
//!
//! Each handler follows the postcard-rpc convention:
//!   `async fn name(ctx: &mut Context, hdr: VarHeader, req: ReqType) -> RespType`
//!
//! Endpoints not listed in the `define_dispatch!` handler table are automatically
//! rejected with `WireError::UnknownKey` by the macro's default match arm.

use postcard_rpc::header::VarHeader;
use rmk_types::protocol::rmk::*;

#[cfg(feature = "storage")]
use crate::channel::FLASH_CHANNEL;
use crate::event::KeyboardEventPos;
#[cfg(feature = "storage")]
use crate::host::storage::{KeymapData, KeymapKey};
use crate::keymap::KeyMap;
#[cfg(feature = "storage")]
use crate::storage::FlashOperationMessage;

use super::RX_BUF_SIZE;

/// Shared context passed to every handler by the dispatcher.
pub(crate) struct ProtocolContext<'a> {
    pub keymap: &'a KeyMap<'a>,
    pub locked: bool,
}

// ---------------------------------------------------------------------------
// System handlers
// ---------------------------------------------------------------------------

pub(crate) async fn get_version(
    _ctx: &mut ProtocolContext<'_>,
    _hdr: VarHeader,
    _req: (),
) -> ProtocolVersion {
    ProtocolVersion::CURRENT
}

pub(crate) async fn get_capabilities(
    ctx: &mut ProtocolContext<'_>,
    _hdr: VarHeader,
    _req: (),
) -> DeviceCapabilities {
    let (num_rows, num_cols, num_layers) = ctx.keymap.get_keymap_config();
    let num_encoders = ctx.keymap.num_encoder();

    DeviceCapabilities {
        protocol_version: ProtocolVersion::CURRENT,
        num_layers: num_layers as u8,
        num_rows: num_rows as u8,
        num_cols: num_cols as u8,
        num_encoders: num_encoders as u8,
        max_combos: crate::COMBO_MAX_NUM as u8,
        max_macros: crate::MACRO_MAX_NUM as u8,
        macro_space_size: crate::MACRO_SPACE_SIZE as u16,
        max_morse: crate::MORSE_MAX_NUM as u8,
        max_forks: crate::FORK_MAX_NUM as u8,
        has_storage: cfg!(feature = "storage"),
        has_split: cfg!(feature = "split"),
        num_split_peripherals: crate::SPLIT_PERIPHERALS_NUM as u8,
        has_ble: cfg!(feature = "_ble"),
        num_ble_profiles: crate::NUM_BLE_PROFILE as u8,
        has_lighting: false,
        max_payload_size: RX_BUF_SIZE as u16,
    }
}

pub(crate) async fn get_lock_status(
    ctx: &mut ProtocolContext<'_>,
    _hdr: VarHeader,
    _req: (),
) -> LockStatus {
    LockStatus {
        locked: ctx.locked,
        awaiting_keys: false,
        remaining_keys: 0,
    }
}

pub(crate) async fn unlock_request(
    ctx: &mut ProtocolContext<'_>,
    _hdr: VarHeader,
    _req: (),
) -> UnlockChallenge {
    // TODO: implement a physical key challenge.
    ctx.locked = false;
    UnlockChallenge {
        key_positions: heapless::Vec::new(),
    }
}

pub(crate) async fn lock_request(
    ctx: &mut ProtocolContext<'_>,
    _hdr: VarHeader,
    _req: (),
) -> () {
    ctx.locked = true;
}

pub(crate) async fn reboot(
    ctx: &mut ProtocolContext<'_>,
    _hdr: VarHeader,
    _req: (),
) -> RmkResult {
    if ctx.locked {
        return Err(RmkError::Locked);
    }
    crate::boot::reboot_keyboard();
    Ok(()) // unreachable on embedded
}

pub(crate) async fn bootloader_jump(
    ctx: &mut ProtocolContext<'_>,
    _hdr: VarHeader,
    _req: (),
) -> RmkResult {
    if ctx.locked {
        return Err(RmkError::Locked);
    }
    crate::boot::jump_to_bootloader();
    Ok(()) // unreachable on embedded
}

pub(crate) async fn storage_reset(
    ctx: &mut ProtocolContext<'_>,
    _hdr: VarHeader,
    mode: StorageResetMode,
) -> RmkResult {
    if ctx.locked {
        return Err(RmkError::Locked);
    }
    #[cfg(feature = "storage")]
    {
        let msg = match mode {
            StorageResetMode::Full => FlashOperationMessage::ResetAndReboot,
            StorageResetMode::LayoutOnly => FlashOperationMessage::ResetLayout,
            _ => FlashOperationMessage::ResetAndReboot,
        };
        FLASH_CHANNEL.send(msg).await;
    }
    #[cfg(not(feature = "storage"))]
    {
        let _ = mode;
        crate::boot::reboot_keyboard();
    }
    core::future::pending::<()>().await;
    Ok(()) // unreachable
}

// ---------------------------------------------------------------------------
// Keymap handlers
// ---------------------------------------------------------------------------

pub(crate) async fn get_key_action(
    ctx: &mut ProtocolContext<'_>,
    _hdr: VarHeader,
    pos: KeyPosition,
) -> rmk_types::action::KeyAction {
    let (row_count, col_count, layer_count) = ctx.keymap.get_keymap_config();
    if (pos.row as usize) >= row_count || (pos.col as usize) >= col_count || (pos.layer as usize) >= layer_count {
        return rmk_types::action::KeyAction::No;
    }
    let event_pos = KeyboardEventPos::key_pos(pos.col, pos.row);
    ctx.keymap.get_action_at(event_pos, pos.layer as usize)
}

pub(crate) async fn set_key_action(
    ctx: &mut ProtocolContext<'_>,
    _hdr: VarHeader,
    req: SetKeyRequest,
) -> RmkResult {
    if ctx.locked {
        return Err(RmkError::Locked);
    }
    let pos = &req.position;
    let (row_count, col_count, layer_count) = ctx.keymap.get_keymap_config();
    if (pos.row as usize) >= row_count || (pos.col as usize) >= col_count || (pos.layer as usize) >= layer_count {
        return Err(RmkError::InvalidParameter);
    }
    let event_pos = KeyboardEventPos::key_pos(pos.col, pos.row);
    ctx.keymap.set_action_at(event_pos, pos.layer as usize, req.action);
    #[cfg(feature = "storage")]
    FLASH_CHANNEL
        .send(FlashOperationMessage::HostMessage(KeymapData::KeymapKey(KeymapKey {
            row: pos.row,
            col: pos.col,
            layer: pos.layer,
            action: req.action,
        })))
        .await;
    Ok(())
}

pub(crate) async fn get_keymap_bulk(
    ctx: &mut ProtocolContext<'_>,
    _hdr: VarHeader,
    req: BulkRequest,
) -> BulkKeyActions {
    let (row_count, col_count, layer_count) = ctx.keymap.get_keymap_config();
    let mut actions: BulkKeyActions = heapless::Vec::new();
    let mut row = req.start_row as usize;
    let mut col = req.start_col as usize;
    let layer = req.layer as usize;
    let count = (req.count as usize).min(MAX_BULK);
    if layer < layer_count && row < row_count && col < col_count {
        for _ in 0..count {
            if row >= row_count {
                break;
            }
            let action = ctx.keymap.get_action_at(KeyboardEventPos::key_pos(col as u8, row as u8), layer);
            if actions.push(action).is_err() {
                break;
            }
            col += 1;
            if col >= col_count {
                col = 0;
                row += 1;
            }
        }
    }
    actions
}

pub(crate) async fn set_keymap_bulk(
    ctx: &mut ProtocolContext<'_>,
    _hdr: VarHeader,
    req: SetKeymapBulkRequest,
) -> RmkResult {
    if ctx.locked {
        return Err(RmkError::Locked);
    }
    let (row_count, col_count, layer_count) = ctx.keymap.get_keymap_config();
    let layer = req.request.layer as usize;
    if layer >= layer_count {
        return Err(RmkError::InvalidParameter);
    }
    let mut row = req.request.start_row as usize;
    let mut col = req.request.start_col as usize;
    if row >= row_count || col >= col_count {
        return Err(RmkError::InvalidParameter);
    }
    for action in req.actions.iter() {
        if row >= row_count {
            break;
        }
        ctx.keymap
            .set_action_at(KeyboardEventPos::key_pos(col as u8, row as u8), layer, *action);
        #[cfg(feature = "storage")]
        FLASH_CHANNEL
            .send(FlashOperationMessage::HostMessage(KeymapData::KeymapKey(KeymapKey {
                row: row as u8,
                col: col as u8,
                layer: layer as u8,
                action: *action,
            })))
            .await;
        col += 1;
        if col >= col_count {
            col = 0;
            row += 1;
        }
    }
    Ok(())
}

pub(crate) async fn get_layer_count(
    ctx: &mut ProtocolContext<'_>,
    _hdr: VarHeader,
    _req: (),
) -> u8 {
    let (_, _, num_layer) = ctx.keymap.get_keymap_config();
    num_layer as u8
}

pub(crate) async fn get_default_layer(
    ctx: &mut ProtocolContext<'_>,
    _hdr: VarHeader,
    _req: (),
) -> u8 {
    ctx.keymap.get_default_layer()
}

pub(crate) async fn set_default_layer(
    ctx: &mut ProtocolContext<'_>,
    _hdr: VarHeader,
    layer: u8,
) -> RmkResult {
    if ctx.locked {
        return Err(RmkError::Locked);
    }
    let (_, _, num_layer) = ctx.keymap.get_keymap_config();
    if layer as usize >= num_layer {
        return Err(RmkError::InvalidParameter);
    }
    ctx.keymap.set_default_layer(layer);
    #[cfg(feature = "storage")]
    FLASH_CHANNEL.send(FlashOperationMessage::DefaultLayer(layer)).await;
    Ok(())
}

pub(crate) async fn reset_keymap(
    ctx: &mut ProtocolContext<'_>,
    _hdr: VarHeader,
    _req: (),
) -> RmkResult {
    if ctx.locked {
        return Err(RmkError::Locked);
    }
    #[cfg(feature = "storage")]
    FLASH_CHANNEL.send(FlashOperationMessage::ResetLayout).await;
    #[cfg(not(feature = "storage"))]
    crate::boot::reboot_keyboard();
    core::future::pending::<()>().await;
    Ok(()) // unreachable
}

// ---------------------------------------------------------------------------
// Connection / Status handlers
// ---------------------------------------------------------------------------

pub(crate) async fn get_connection_info(
    _ctx: &mut ProtocolContext<'_>,
    _hdr: VarHeader,
    _req: (),
) -> ConnectionInfo {
    #[cfg(feature = "_ble")]
    let ble_status = crate::ble::BLE_STATUS.lock(|c| c.get());
    #[cfg(not(feature = "_ble"))]
    let ble_status = rmk_types::ble::BleStatus::default();

    ConnectionInfo {
        connection_type: crate::state::get_connection_type(),
        ble_profile: ble_status.profile,
        ble_connected: matches!(ble_status.state, rmk_types::ble::BleState::Connected),
    }
}

pub(crate) async fn get_current_layer(
    ctx: &mut ProtocolContext<'_>,
    _hdr: VarHeader,
    _req: (),
) -> u8 {
    ctx.keymap.get_activated_layer()
}

pub(crate) async fn get_matrix_state(
    ctx: &mut ProtocolContext<'_>,
    _hdr: VarHeader,
    _req: (),
) -> MatrixState {
    let (row_count, col_count, _) = ctx.keymap.get_keymap_config();
    let bitmap_len = row_count * col_count.div_ceil(8);
    let mut raw = [0u8; MAX_MATRIX_BITMAP_SIZE];
    ctx.keymap.read_matrix_state(&mut raw[..bitmap_len]);
    let pressed_bitmap = heapless::Vec::from_slice(&raw[..bitmap_len]).expect("matrix bitmap length fits");
    MatrixState { pressed_bitmap }
}
