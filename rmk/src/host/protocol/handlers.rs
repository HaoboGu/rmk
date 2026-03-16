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
///
/// A fresh context is created for each USB connection session in
/// `ProtocolService::run()`, so lock state is per-session: disconnecting
/// and reconnecting re-locks the device. Each transport (USB, BLE) has
/// independent lock state.
pub(crate) struct ProtocolContext<'a> {
    pub keymap: &'a KeyMap<'a>,
    pub locked: bool,
}

impl ProtocolContext<'_> {
    /// Return `Err(Locked)` if the device is locked, used by mutating handlers.
    fn check_unlocked(&self) -> RmkResult {
        if self.locked { Err(RmkError::Locked) } else { Ok(()) }
    }

    /// Return `Err(InvalidParameter)` if `pos` is out of bounds.
    fn check_bounds(&self, pos: &KeyPosition) -> RmkResult {
        let (row_count, col_count, layer_count) = self.keymap.get_keymap_config();
        if (pos.row as usize) >= row_count || (pos.col as usize) >= col_count || (pos.layer as usize) >= layer_count {
            Err(RmkError::InvalidParameter)
        } else {
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// System handlers
// ---------------------------------------------------------------------------

pub(crate) async fn get_version(_ctx: &mut ProtocolContext<'_>, _hdr: VarHeader, _req: ()) -> ProtocolVersion {
    ProtocolVersion::CURRENT
}

pub(crate) async fn get_capabilities(ctx: &mut ProtocolContext<'_>, _hdr: VarHeader, _req: ()) -> DeviceCapabilities {
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

pub(crate) async fn get_lock_status(ctx: &mut ProtocolContext<'_>, _hdr: VarHeader, _req: ()) -> LockStatus {
    LockStatus {
        locked: ctx.locked,
        awaiting_keys: false,
        remaining_keys: 0,
    }
}

/// SECURITY: This currently unlocks immediately without a physical key
/// challenge. Any connected host can unlock the device with a single
/// request. A real challenge (Phase 8) will require pressing physical
/// keys before the device unlocks.
pub(crate) async fn unlock_request(ctx: &mut ProtocolContext<'_>, _hdr: VarHeader, _req: ()) -> UnlockChallenge {
    ctx.locked = false;
    UnlockChallenge {
        key_positions: heapless::Vec::new(),
    }
}

pub(crate) async fn lock_request(ctx: &mut ProtocolContext<'_>, _hdr: VarHeader, _req: ()) {
    ctx.locked = true;
}

pub(crate) async fn reboot(ctx: &mut ProtocolContext<'_>, _hdr: VarHeader, _req: ()) -> RmkResult {
    ctx.check_unlocked()?;
    crate::boot::reboot_keyboard();
    Ok(()) // unreachable on embedded
}

pub(crate) async fn bootloader_jump(ctx: &mut ProtocolContext<'_>, _hdr: VarHeader, _req: ()) -> RmkResult {
    ctx.check_unlocked()?;
    crate::boot::jump_to_bootloader();
    Ok(()) // unreachable on embedded
}

pub(crate) async fn storage_reset(ctx: &mut ProtocolContext<'_>, _hdr: VarHeader, mode: StorageResetMode) -> RmkResult {
    ctx.check_unlocked()?;
    #[cfg(feature = "storage")]
    {
        let msg = match mode {
            StorageResetMode::Full => FlashOperationMessage::ResetAndReboot,
            StorageResetMode::LayoutOnly => FlashOperationMessage::ResetLayout,
            // Forward-compat: a newer host tool may send variants unknown to this firmware.
        _ => return Err(RmkError::InvalidParameter),
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
    if ctx.check_bounds(&pos).is_err() {
        return rmk_types::action::KeyAction::No;
    }
    let event_pos = KeyboardEventPos::key_pos(pos.col, pos.row);
    ctx.keymap.get_action_at(event_pos, pos.layer as usize)
}

pub(crate) async fn set_key_action(ctx: &mut ProtocolContext<'_>, _hdr: VarHeader, req: SetKeyRequest) -> RmkResult {
    ctx.check_unlocked()?;
    ctx.check_bounds(&req.position)?;
    let pos = &req.position;
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
    let count = (req.count as usize).min(MAX_BULK);
    let mut actions: BulkKeyActions = heapless::Vec::new();
    let pos = KeyPosition { layer: req.layer, row: req.start_row, col: req.start_col };
    if ctx.check_bounds(&pos).is_ok() {
        ctx.keymap.get_actions_bulk(pos.layer as usize, pos.row as usize, pos.col as usize, count, &mut actions);
    }
    actions
}

pub(crate) async fn set_keymap_bulk(
    ctx: &mut ProtocolContext<'_>,
    _hdr: VarHeader,
    req: SetKeymapBulkRequest,
) -> RmkResult {
    ctx.check_unlocked()?;
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

pub(crate) async fn get_layer_count(ctx: &mut ProtocolContext<'_>, _hdr: VarHeader, _req: ()) -> u8 {
    let (_, _, num_layer) = ctx.keymap.get_keymap_config();
    num_layer as u8
}

pub(crate) async fn get_default_layer(ctx: &mut ProtocolContext<'_>, _hdr: VarHeader, _req: ()) -> u8 {
    ctx.keymap.get_default_layer()
}

pub(crate) async fn set_default_layer(ctx: &mut ProtocolContext<'_>, _hdr: VarHeader, layer: u8) -> RmkResult {
    ctx.check_unlocked()?;
    let (_, _, num_layer) = ctx.keymap.get_keymap_config();
    if layer as usize >= num_layer {
        return Err(RmkError::InvalidParameter);
    }
    ctx.keymap.set_default_layer(layer);
    #[cfg(feature = "storage")]
    FLASH_CHANNEL.send(FlashOperationMessage::DefaultLayer(layer)).await;
    Ok(())
}

pub(crate) async fn reset_keymap(ctx: &mut ProtocolContext<'_>, _hdr: VarHeader, _req: ()) -> RmkResult {
    ctx.check_unlocked()?;
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

pub(crate) async fn get_connection_info(_ctx: &mut ProtocolContext<'_>, _hdr: VarHeader, _req: ()) -> ConnectionInfo {
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

pub(crate) async fn get_current_layer(ctx: &mut ProtocolContext<'_>, _hdr: VarHeader, _req: ()) -> u8 {
    ctx.keymap.get_activated_layer()
}

pub(crate) async fn get_matrix_state(ctx: &mut ProtocolContext<'_>, _hdr: VarHeader, _req: ()) -> MatrixState {
    #[cfg(feature = "host_security")]
    {
        let (row_count, col_count, _) = ctx.keymap.get_keymap_config();
        let bitmap_len = row_count * col_count.div_ceil(8);
        let mut raw = [0u8; MAX_MATRIX_BITMAP_SIZE];
        ctx.keymap.read_matrix_state(&mut raw[..bitmap_len]);
        let pressed_bitmap = heapless::Vec::from_slice(&raw[..bitmap_len]).expect("matrix bitmap length fits");
        MatrixState { pressed_bitmap }
    }
    #[cfg(not(feature = "host_security"))]
    {
        let _ = ctx;
        MatrixState { pressed_bitmap: heapless::Vec::new() }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use embassy_futures::block_on;
    use postcard_rpc::header::{VarKey, VarSeq};
    use rmk_types::action::KeyAction;

    use crate::config::{BehaviorConfig, PositionalConfig};
    use crate::k;
    use crate::keymap::KeymapData;

    fn dummy_hdr() -> VarHeader {
        // Safety: all-zero is a valid Key; handlers ignore the header entirely.
        let key = unsafe { postcard_rpc::Key::from_bytes([0u8; 8]) };
        VarHeader {
            key: VarKey::Key8(key),
            seq_no: VarSeq::Seq2(0),
        }
    }

    /// 2 layers × 3 rows × 4 cols. Layer 0 has A–L, layer 1 is all No.
    fn make_keymap() -> &'static KeyMap<'static> {
        let layer0 = [
            [k!(A), k!(B), k!(C), k!(D)],
            [k!(E), k!(F), k!(G), k!(H)],
            [k!(I), k!(J), k!(K), k!(L)],
        ];
        let layer1 = [[KeyAction::No; 4]; 3];

        let data = Box::leak(Box::new(KeymapData::new([layer0, layer1])));
        let config = Box::leak(Box::new(BehaviorConfig::default()));
        let pos_config = Box::leak(Box::new(PositionalConfig::<3, 4>::default()));
        let km = block_on(KeyMap::new(data, config, pos_config));
        Box::leak(Box::new(km))
    }

    fn make_ctx<'a>(keymap: &'a KeyMap<'a>, locked: bool) -> ProtocolContext<'a> {
        ProtocolContext { keymap, locked }
    }

    // --- Bounds checking ---

    #[test]
    fn check_bounds_valid() {
        let km = make_keymap();
        let ctx = make_ctx(km, false);
        assert!(ctx.check_bounds(&KeyPosition { layer: 0, row: 0, col: 0 }).is_ok());
        assert!(ctx.check_bounds(&KeyPosition { layer: 1, row: 2, col: 3 }).is_ok());
    }

    #[test]
    fn check_bounds_out_of_range() {
        let km = make_keymap();
        let ctx = make_ctx(km, false);
        assert_eq!(
            ctx.check_bounds(&KeyPosition { layer: 2, row: 0, col: 0 }),
            Err(RmkError::InvalidParameter)
        );
        assert_eq!(
            ctx.check_bounds(&KeyPosition { layer: 0, row: 3, col: 0 }),
            Err(RmkError::InvalidParameter)
        );
        assert_eq!(
            ctx.check_bounds(&KeyPosition { layer: 0, row: 0, col: 4 }),
            Err(RmkError::InvalidParameter)
        );
    }

    // --- Lock checking ---

    #[test]
    fn locked_rejects_writes() {
        let km = make_keymap();
        let mut ctx = make_ctx(km, true);
        let hdr = dummy_hdr();

        let result = block_on(set_key_action(
            &mut ctx,
            hdr,
            SetKeyRequest {
                position: KeyPosition { layer: 0, row: 0, col: 0 },
                action: KeyAction::No,
            },
        ));
        assert_eq!(result, Err(RmkError::Locked));
    }

    #[test]
    fn lock_guards_write_operations() {
        let km = make_keymap();
        let mut ctx = make_ctx(km, true);
        let hdr = dummy_hdr();

        // Locked: write should fail
        assert_eq!(block_on(set_default_layer(&mut ctx, hdr, 0)), Err(RmkError::Locked));

        // Unlock directly (unlock_request tested via integration tests)
        ctx.locked = false;

        // Unlocked: write should succeed
        block_on(async {
            assert_eq!(set_default_layer(&mut ctx, hdr, 1).await, Ok(()));
            assert_eq!(get_default_layer(&mut ctx, hdr, ()).await, 1);
        });

        // Re-lock: write should fail again
        ctx.locked = true;
        assert_eq!(block_on(set_default_layer(&mut ctx, hdr, 0)), Err(RmkError::Locked));
    }

    // --- get_keymap_bulk ---

    #[test]
    fn bulk_get_single_row() {
        let km = make_keymap();
        let mut ctx = make_ctx(km, false);
        let hdr = dummy_hdr();

        let actions = block_on(get_keymap_bulk(
            &mut ctx,
            hdr,
            BulkRequest { layer: 0, start_row: 0, start_col: 0, count: 4 },
        ));
        assert_eq!(actions.len(), 4);
        assert_eq!(actions[0], k!(A));
        assert_eq!(actions[1], k!(B));
        assert_eq!(actions[2], k!(C));
        assert_eq!(actions[3], k!(D));
    }

    #[test]
    fn bulk_get_wraps_to_next_row() {
        let km = make_keymap();
        let mut ctx = make_ctx(km, false);
        let hdr = dummy_hdr();

        // Start at row 0 col 2 (C), request 4 → C D E F
        let actions = block_on(get_keymap_bulk(
            &mut ctx,
            hdr,
            BulkRequest { layer: 0, start_row: 0, start_col: 2, count: 4 },
        ));
        assert_eq!(actions.len(), 4);
        assert_eq!(actions[0], k!(C));
        assert_eq!(actions[1], k!(D));
        assert_eq!(actions[2], k!(E));
        assert_eq!(actions[3], k!(F));
    }

    #[test]
    fn bulk_get_crosses_layer_boundary() {
        let km = make_keymap();
        let mut ctx = make_ctx(km, false);
        let hdr = dummy_hdr();

        // Start at layer 0, row 2, col 3 (L), request 3 → L then layer 1 (No, No)
        let actions = block_on(get_keymap_bulk(
            &mut ctx,
            hdr,
            BulkRequest { layer: 0, start_row: 2, start_col: 3, count: 3 },
        ));
        assert_eq!(actions.len(), 3);
        assert_eq!(actions[0], k!(L));
        assert_eq!(actions[1], KeyAction::No);
        assert_eq!(actions[2], KeyAction::No);
    }

    #[test]
    fn bulk_get_out_of_bounds_returns_empty() {
        let km = make_keymap();
        let mut ctx = make_ctx(km, false);
        let hdr = dummy_hdr();

        let actions = block_on(get_keymap_bulk(
            &mut ctx,
            hdr,
            BulkRequest { layer: 5, start_row: 0, start_col: 0, count: 10 },
        ));
        assert_eq!(actions.len(), 0);
    }

    #[test]
    fn bulk_get_stops_at_end_of_keymap() {
        let km = make_keymap();
        let mut ctx = make_ctx(km, false);
        let hdr = dummy_hdr();

        // layer 1, row 2, col 2 — only 2 keys left in entire keymap
        let actions = block_on(get_keymap_bulk(
            &mut ctx,
            hdr,
            BulkRequest { layer: 1, start_row: 2, start_col: 2, count: 100 },
        ));
        assert_eq!(actions.len(), 2);
    }

    // --- set_keymap_bulk ---

    #[test]
    fn bulk_set_wraps_rows() {
        let km = make_keymap();
        let mut ctx = make_ctx(km, false);
        let hdr = dummy_hdr();

        block_on(async {
            // Set 3 keys starting at row 0 col 3 → wraps to row 1
            let mut actions = heapless::Vec::<_, MAX_BULK>::new();
            actions.push(k!(X)).unwrap();
            actions.push(k!(Y)).unwrap();
            actions.push(k!(Z)).unwrap();

            let result = set_keymap_bulk(
                &mut ctx,
                hdr,
                SetKeymapBulkRequest {
                    request: BulkRequest { layer: 0, start_row: 0, start_col: 3, count: 3 },
                    actions,
                },
            )
            .await;
            assert_eq!(result, Ok(()));

            // Verify: (0,0,3)=X, (0,1,0)=Y, (0,1,1)=Z
            assert_eq!(
                get_key_action(&mut ctx, hdr, KeyPosition { layer: 0, row: 0, col: 3 }).await,
                k!(X)
            );
            assert_eq!(
                get_key_action(&mut ctx, hdr, KeyPosition { layer: 0, row: 1, col: 0 }).await,
                k!(Y)
            );
            assert_eq!(
                get_key_action(&mut ctx, hdr, KeyPosition { layer: 0, row: 1, col: 1 }).await,
                k!(Z)
            );
        });
    }

    #[test]
    fn bulk_set_locked_fails() {
        let km = make_keymap();
        let mut ctx = make_ctx(km, true);
        let hdr = dummy_hdr();

        let result = block_on(set_keymap_bulk(
            &mut ctx,
            hdr,
            SetKeymapBulkRequest {
                request: BulkRequest { layer: 0, start_row: 0, start_col: 0, count: 0 },
                actions: heapless::Vec::new(),
            },
        ));
        assert_eq!(result, Err(RmkError::Locked));
    }

    #[test]
    fn bulk_set_invalid_layer_fails() {
        let km = make_keymap();
        let mut ctx = make_ctx(km, false);
        let hdr = dummy_hdr();

        let result = block_on(set_keymap_bulk(
            &mut ctx,
            hdr,
            SetKeymapBulkRequest {
                request: BulkRequest { layer: 5, start_row: 0, start_col: 0, count: 0 },
                actions: heapless::Vec::new(),
            },
        ));
        assert_eq!(result, Err(RmkError::InvalidParameter));
    }

    // --- set_default_layer ---

    #[test]
    fn set_default_layer_invalid_returns_error() {
        let km = make_keymap();
        let mut ctx = make_ctx(km, false);
        let hdr = dummy_hdr();

        // Layer 2 is out of bounds for a 2-layer keymap.
        assert_eq!(block_on(set_default_layer(&mut ctx, hdr, 2)), Err(RmkError::InvalidParameter));
    }

    // Note: set_default_layer success path and read-back are covered by
    // `lock_guards_write_operations`. Standalone tests that call
    // set_default_layer successfully cause hangs due to a known interaction
    // between embassy_futures::block_on's noop waker and the
    // `#[cfg(feature = "storage")]` async codegen in the handler.

    // --- get_key_action ---

    #[test]
    fn get_key_out_of_bounds_returns_no() {
        let km = make_keymap();
        let mut ctx = make_ctx(km, false);
        let hdr = dummy_hdr();

        let action = block_on(get_key_action(
            &mut ctx,
            hdr,
            KeyPosition { layer: 0, row: 99, col: 0 },
        ));
        assert_eq!(action, KeyAction::No);
    }
}
