//! Endpoint handler functions for the RMK protocol dispatcher.
//!
//! Each handler follows the postcard-rpc convention:
//!   `async fn name(ctx: &mut Context, hdr: VarHeader, req: ReqType) -> RespType`
//!
//! Endpoints not listed in the `define_dispatch!` handler table are automatically
//! rejected with `WireError::UnknownKey` by the macro's default match arm.

use core::cell::RefCell;

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
pub(crate) struct ProtocolContext<
    'a,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
    const NUM_ENCODER: usize,
> {
    pub keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
    pub locked: bool,
}

// ---------------------------------------------------------------------------
// System handlers
// ---------------------------------------------------------------------------

pub(crate) async fn get_version<const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>(
    _ctx: &mut ProtocolContext<'_, ROW, COL, NUM_LAYER, NUM_ENCODER>,
    _hdr: VarHeader,
    _req: (),
) -> ProtocolVersion {
    ProtocolVersion::CURRENT
}

pub(crate) async fn get_capabilities<const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>(
    _ctx: &mut ProtocolContext<'_, ROW, COL, NUM_LAYER, NUM_ENCODER>,
    _hdr: VarHeader,
    _req: (),
) -> DeviceCapabilities {
    const {
        if ROW > u8::MAX as usize {
            core::panic!("ROW exceeds u8 range")
        }
    };
    const {
        if COL > u8::MAX as usize {
            core::panic!("COL exceeds u8 range")
        }
    };
    const {
        if NUM_LAYER > u8::MAX as usize {
            core::panic!("NUM_LAYER exceeds u8 range")
        }
    };
    const {
        if NUM_ENCODER > u8::MAX as usize {
            core::panic!("NUM_ENCODER exceeds u8 range")
        }
    };

    DeviceCapabilities {
        protocol_version: ProtocolVersion::CURRENT,
        num_layers: NUM_LAYER as u8,
        num_rows: ROW as u8,
        num_cols: COL as u8,
        num_encoders: NUM_ENCODER as u8,
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

pub(crate) async fn get_lock_status<const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>(
    ctx: &mut ProtocolContext<'_, ROW, COL, NUM_LAYER, NUM_ENCODER>,
    _hdr: VarHeader,
    _req: (),
) -> LockStatus {
    LockStatus {
        locked: ctx.locked,
        awaiting_keys: false,
        remaining_keys: 0,
    }
}

pub(crate) async fn unlock_request<const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>(
    ctx: &mut ProtocolContext<'_, ROW, COL, NUM_LAYER, NUM_ENCODER>,
    _hdr: VarHeader,
    _req: (),
) -> UnlockChallenge {
    // TODO: implement a physical key challenge.
    ctx.locked = false;
    UnlockChallenge {
        key_positions: heapless::Vec::new(),
    }
}

pub(crate) async fn lock_request<const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>(
    ctx: &mut ProtocolContext<'_, ROW, COL, NUM_LAYER, NUM_ENCODER>,
    _hdr: VarHeader,
    _req: (),
) -> () {
    ctx.locked = true;
}

pub(crate) async fn reboot<const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>(
    ctx: &mut ProtocolContext<'_, ROW, COL, NUM_LAYER, NUM_ENCODER>,
    _hdr: VarHeader,
    _req: (),
) -> RmkResult {
    if ctx.locked {
        return Err(RmkError::Locked);
    }
    crate::boot::reboot_keyboard();
    Ok(()) // unreachable on embedded
}

pub(crate) async fn bootloader_jump<const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>(
    ctx: &mut ProtocolContext<'_, ROW, COL, NUM_LAYER, NUM_ENCODER>,
    _hdr: VarHeader,
    _req: (),
) -> RmkResult {
    if ctx.locked {
        return Err(RmkError::Locked);
    }
    crate::boot::jump_to_bootloader();
    Ok(()) // unreachable on embedded
}

pub(crate) async fn storage_reset<const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>(
    ctx: &mut ProtocolContext<'_, ROW, COL, NUM_LAYER, NUM_ENCODER>,
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

pub(crate) async fn get_key_action<const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>(
    ctx: &mut ProtocolContext<'_, ROW, COL, NUM_LAYER, NUM_ENCODER>,
    _hdr: VarHeader,
    pos: KeyPosition,
) -> rmk_types::action::KeyAction {
    if (pos.row as usize) >= ROW || (pos.col as usize) >= COL || (pos.layer as usize) >= NUM_LAYER {
        return rmk_types::action::KeyAction::No;
    }
    let event_pos = KeyboardEventPos::key_pos(pos.col, pos.row);
    ctx.keymap.borrow().get_action_at(event_pos, pos.layer as usize)
}

pub(crate) async fn set_key_action<const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>(
    ctx: &mut ProtocolContext<'_, ROW, COL, NUM_LAYER, NUM_ENCODER>,
    _hdr: VarHeader,
    req: SetKeyRequest,
) -> RmkResult {
    if ctx.locked {
        return Err(RmkError::Locked);
    }
    let pos = &req.position;
    if (pos.row as usize) >= ROW || (pos.col as usize) >= COL || (pos.layer as usize) >= NUM_LAYER {
        return Err(RmkError::InvalidParameter);
    }
    let event_pos = KeyboardEventPos::key_pos(pos.col, pos.row);
    ctx.keymap
        .borrow_mut()
        .set_action_at(event_pos, pos.layer as usize, req.action);
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

pub(crate) async fn get_keymap_bulk<const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>(
    ctx: &mut ProtocolContext<'_, ROW, COL, NUM_LAYER, NUM_ENCODER>,
    _hdr: VarHeader,
    req: BulkRequest,
) -> BulkKeyActions {
    let mut actions: BulkKeyActions = heapless::Vec::new();
    let mut row = req.start_row as usize;
    let mut col = req.start_col as usize;
    let layer = req.layer as usize;
    let count = (req.count as usize).min(MAX_BULK);
    if layer < NUM_LAYER && row < ROW && col < COL {
        let km = ctx.keymap.borrow();
        for _ in 0..count {
            if row >= ROW {
                break;
            }
            let action = km.get_action_at(KeyboardEventPos::key_pos(col as u8, row as u8), layer);
            if actions.push(action).is_err() {
                break;
            }
            col += 1;
            if col >= COL {
                col = 0;
                row += 1;
            }
        }
    }
    actions
}

pub(crate) async fn set_keymap_bulk<const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>(
    ctx: &mut ProtocolContext<'_, ROW, COL, NUM_LAYER, NUM_ENCODER>,
    _hdr: VarHeader,
    req: SetKeymapBulkRequest,
) -> RmkResult {
    if ctx.locked {
        return Err(RmkError::Locked);
    }
    let layer = req.request.layer as usize;
    if layer >= NUM_LAYER {
        return Err(RmkError::InvalidParameter);
    }
    let mut row = req.request.start_row as usize;
    let mut col = req.request.start_col as usize;
    if row >= ROW || col >= COL {
        return Err(RmkError::InvalidParameter);
    }
    for action in req.actions.iter() {
        if row >= ROW {
            break;
        }
        ctx.keymap
            .borrow_mut()
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
        if col >= COL {
            col = 0;
            row += 1;
        }
    }
    Ok(())
}

pub(crate) async fn get_layer_count<const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>(
    _ctx: &mut ProtocolContext<'_, ROW, COL, NUM_LAYER, NUM_ENCODER>,
    _hdr: VarHeader,
    _req: (),
) -> u8 {
    NUM_LAYER as u8
}

pub(crate) async fn get_default_layer<const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>(
    ctx: &mut ProtocolContext<'_, ROW, COL, NUM_LAYER, NUM_ENCODER>,
    _hdr: VarHeader,
    _req: (),
) -> u8 {
    ctx.keymap.borrow().get_default_layer()
}

pub(crate) async fn set_default_layer<const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>(
    ctx: &mut ProtocolContext<'_, ROW, COL, NUM_LAYER, NUM_ENCODER>,
    _hdr: VarHeader,
    layer: u8,
) -> RmkResult {
    if ctx.locked {
        return Err(RmkError::Locked);
    }
    if layer as usize >= NUM_LAYER {
        return Err(RmkError::InvalidParameter);
    }
    ctx.keymap.borrow_mut().set_default_layer(layer);
    #[cfg(feature = "storage")]
    FLASH_CHANNEL.send(FlashOperationMessage::DefaultLayer(layer)).await;
    Ok(())
}

pub(crate) async fn reset_keymap<const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>(
    ctx: &mut ProtocolContext<'_, ROW, COL, NUM_LAYER, NUM_ENCODER>,
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

pub(crate) async fn get_connection_info<const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>(
    _ctx: &mut ProtocolContext<'_, ROW, COL, NUM_LAYER, NUM_ENCODER>,
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

pub(crate) async fn get_current_layer<const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>(
    ctx: &mut ProtocolContext<'_, ROW, COL, NUM_LAYER, NUM_ENCODER>,
    _hdr: VarHeader,
    _req: (),
) -> u8 {
    ctx.keymap.borrow().get_activated_layer()
}

pub(crate) async fn get_matrix_state<const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>(
    ctx: &mut ProtocolContext<'_, ROW, COL, NUM_LAYER, NUM_ENCODER>,
    _hdr: VarHeader,
    _req: (),
) -> MatrixState {
    let bitmap_len = ROW * COL.div_ceil(8);
    let mut raw = [0u8; MAX_MATRIX_BITMAP_SIZE];
    ctx.keymap
        .borrow()
        .matrix_state
        .read_protocol_bitmap(&mut raw[..bitmap_len]);
    let pressed_bitmap = heapless::Vec::from_slice(&raw[..bitmap_len]).expect("matrix bitmap length fits");
    MatrixState { pressed_bitmap }
}
