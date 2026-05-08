//! Handlers for the `keymap/*` endpoint group.

use postcard_rpc::header::VarHeader;
use rmk_types::action::KeyAction;
use rmk_types::protocol::rmk::{KeyPosition, RmkResult, SetKeyRequest};

use super::super::Ctx;
use crate::event::KeyboardEventPos;
#[cfg(feature = "storage")]
use crate::{channel::FLASH_CHANNEL, storage::FlashOperationMessage};

pub(crate) async fn get_key_action(ctx: &mut Ctx<'_>, _hdr: VarHeader, pos: KeyPosition) -> KeyAction {
    ctx.keymap.action_at_pos(pos.layer as usize, pos.row, pos.col)
}

pub(crate) async fn set_key_action(ctx: &mut Ctx<'_>, _hdr: VarHeader, req: SetKeyRequest) -> RmkResult {
    ctx.keymap.set_action_at(
        KeyboardEventPos::key_pos(req.position.col, req.position.row),
        req.position.layer as usize,
        req.action,
    );
    #[cfg(feature = "storage")]
    FLASH_CHANNEL
        .send(FlashOperationMessage::KeymapKey {
            layer: req.position.layer,
            row: req.position.row,
            col: req.position.col,
            action: req.action,
        })
        .await;
    Ok(())
}

pub(crate) async fn get_default_layer(ctx: &mut Ctx<'_>, _hdr: VarHeader, _req: ()) -> u8 {
    ctx.keymap.get_default_layer()
}

pub(crate) async fn set_default_layer(ctx: &mut Ctx<'_>, _hdr: VarHeader, layer: u8) -> RmkResult {
    ctx.keymap.set_default_layer(layer);
    #[cfg(feature = "storage")]
    FLASH_CHANNEL.send(FlashOperationMessage::DefaultLayer(layer)).await;
    Ok(())
}

#[cfg(feature = "bulk_transfer")]
pub(crate) mod bulk {
    use heapless::Vec;
    use postcard_rpc::header::VarHeader;
    use rmk_types::action::KeyAction;
    use rmk_types::constants::BULK_SIZE;
    use rmk_types::protocol::rmk::{
        GetKeymapBulkRequest, GetKeymapBulkResponse, RmkError, RmkResult, SetKeymapBulkRequest,
    };

    use super::super::super::Ctx;
    use crate::event::KeyboardEventPos;
    #[cfg(feature = "storage")]
    use crate::{
        channel::FLASH_CHANNEL,
        storage::{FLASH_OPERATION_FINISHED, FlashOperationMessage},
    };

    pub(crate) async fn get_keymap_bulk(
        ctx: &mut Ctx<'_>,
        _hdr: VarHeader,
        req: GetKeymapBulkRequest,
    ) -> GetKeymapBulkResponse {
        let (rows, cols, layers) = ctx.keymap.get_keymap_config();
        let mut actions: Vec<KeyAction, BULK_SIZE> = Vec::new();
        if (req.layer as usize) >= layers {
            return GetKeymapBulkResponse { actions };
        }
        let mut row = req.start_row as usize;
        let mut col = req.start_col as usize;
        for _ in 0..req.count {
            if row >= rows {
                break;
            }
            let action = ctx.keymap.action_at_pos(req.layer as usize, row as u8, col as u8);
            if actions.push(action).is_err() {
                break;
            }
            col += 1;
            if col >= cols {
                col = 0;
                row += 1;
            }
        }
        GetKeymapBulkResponse { actions }
    }

    pub(crate) async fn set_keymap_bulk(ctx: &mut Ctx<'_>, _hdr: VarHeader, req: SetKeymapBulkRequest) -> RmkResult {
        let (rows, cols, layers) = ctx.keymap.get_keymap_config();
        if (req.layer as usize) >= layers {
            return Err(RmkError::InvalidParameter);
        }
        let mut row = req.start_row as usize;
        let mut col = req.start_col as usize;
        for action in req.actions.iter() {
            if row >= rows || col >= cols {
                break;
            }
            ctx.keymap.set_action_at(
                KeyboardEventPos::key_pos(col as u8, row as u8),
                req.layer as usize,
                *action,
            );
            #[cfg(feature = "storage")]
            {
                FLASH_CHANNEL
                    .send(FlashOperationMessage::KeymapKey {
                        layer: req.layer,
                        row: row as u8,
                        col: col as u8,
                        action: *action,
                    })
                    .await;
                let _ = FLASH_OPERATION_FINISHED.wait().await;
            }
            col += 1;
            if col >= cols {
                col = 0;
                row += 1;
            }
        }
        Ok(())
    }
}
