//! Keymap and encoder handlers (encoder is part of keymap's `0x01xx` Cmd group).

use rmk_types::action::{EncoderAction, KeyAction};
#[cfg(feature = "bulk")]
use rmk_types::constants::BULK_SIZE;
use rmk_types::protocol::rynk::command::{
    GetDefaultLayer, GetEncoderAction, GetKeyAction, SetDefaultLayer, SetEncoderAction, SetKeyAction,
};
#[cfg(feature = "bulk")]
use rmk_types::protocol::rynk::command::{GetKeymapBulk, SetKeymapBulk};
use rmk_types::protocol::rynk::{GetEncoderRequest, KeyPosition, RynkError, SetEncoderRequest, SetKeyRequest};
#[cfg(feature = "bulk")]
use rmk_types::protocol::rynk::{GetKeymapBulkRequest, GetKeymapBulkResponse, SetKeymapBulkRequest};

use super::super::RynkService;
use super::Handle;

impl Handle<GetKeyAction> for RynkService<'_> {
    async fn handle(&self, pos: KeyPosition) -> Result<KeyAction, RynkError> {
        self.check_key_position(&pos)?;
        Ok(self.ctx.get_action(pos.layer, pos.row, pos.col))
    }
}

impl Handle<SetKeyAction> for RynkService<'_> {
    async fn handle(&self, set: SetKeyRequest) -> Result<(), RynkError> {
        self.check_key_position(&set.position)?;
        self.ctx
            .set_action(set.position.layer, set.position.row, set.position.col, set.action)
            .await;
        Ok(())
    }
}

impl Handle<GetDefaultLayer> for RynkService<'_> {
    async fn handle(&self, _: ()) -> Result<u8, RynkError> {
        Ok(self.ctx.default_layer())
    }
}

impl Handle<SetDefaultLayer> for RynkService<'_> {
    async fn handle(&self, layer: u8) -> Result<(), RynkError> {
        let (_, _, num_layers) = self.ctx.keymap_dimensions();
        if (layer as usize) >= num_layers {
            return Err(RynkError::Invalid);
        }
        self.ctx.set_default_layer(layer).await;
        Ok(())
    }
}

impl Handle<GetEncoderAction> for RynkService<'_> {
    async fn handle(&self, r: GetEncoderRequest) -> Result<EncoderAction, RynkError> {
        self.check_encoder_bounds(r.layer, r.encoder_id)?;
        self.ctx.get_encoder(r.layer, r.encoder_id).ok_or(RynkError::Invalid)
    }
}

impl Handle<SetEncoderAction> for RynkService<'_> {
    async fn handle(&self, r: SetEncoderRequest) -> Result<(), RynkError> {
        self.check_encoder_bounds(r.layer, r.encoder_id)?;
        self.ctx.set_encoder(r.layer, r.encoder_id, r.action).await;
        Ok(())
    }
}

impl RynkService<'_> {
    /// `Invalid` for a key position outside the live keymap grid. Reads and
    /// writes share these bounds.
    fn check_key_position(&self, pos: &KeyPosition) -> Result<(), RynkError> {
        let (rows, cols, num_layers) = self.ctx.keymap_dimensions();
        if (pos.layer as usize) >= num_layers || (pos.row as usize) >= rows || (pos.col as usize) >= cols {
            return Err(RynkError::Invalid);
        }
        Ok(())
    }

    /// `Invalid` for an out-of-range encoder. Checks `layer` and `encoder_id`
    /// explicitly rather than relying on `get_encoder` returning `None`: the
    /// keymap flat-indexes encoders (`layer * num_encoder + id`), so an
    /// over-range `id` would otherwise alias into another layer's slot.
    fn check_encoder_bounds(&self, layer: u8, encoder_id: u8) -> Result<(), RynkError> {
        let (_, _, num_layers) = self.ctx.keymap_dimensions();
        if (layer as usize) >= num_layers || (encoder_id as usize) >= self.ctx.num_encoders() {
            return Err(RynkError::Invalid);
        }
        Ok(())
    }
}

#[cfg(feature = "bulk")]
impl RynkService<'_> {
    /// Bounds check shared by both bulk keymap handlers: a valid start key and
    /// a row-major run of `count` keys ending within the same layer — bulk
    /// never wraps across layers. Returns the start key's row-major offset.
    fn check_keymap_run(&self, layer: u8, start_row: u8, start_col: u8, count: usize) -> Result<usize, RynkError> {
        self.check_key_position(&KeyPosition {
            layer,
            row: start_row,
            col: start_col,
        })?;
        let (rows, cols, _) = self.ctx.keymap_dimensions();
        let start = start_row as usize * cols + start_col as usize;
        if count == 0 || count > BULK_SIZE || start + count > rows * cols {
            return Err(RynkError::Invalid);
        }
        Ok(start)
    }
}

#[cfg(feature = "bulk")]
impl Handle<GetKeymapBulk> for RynkService<'_> {
    async fn handle(&self, req: GetKeymapBulkRequest) -> Result<GetKeymapBulkResponse, RynkError> {
        let count = req.count as usize;
        let start = self.check_keymap_run(req.layer, req.start_row, req.start_col, count)?;
        let (_, cols, _) = self.ctx.keymap_dimensions();
        let mut actions = heapless::Vec::new();
        for offset in start..start + count {
            actions
                .push(
                    self.ctx
                        .get_action(req.layer, (offset / cols) as u8, (offset % cols) as u8),
                )
                .map_err(|_| RynkError::Internal)?;
        }
        Ok(GetKeymapBulkResponse { actions })
    }
}

#[cfg(feature = "bulk")]
impl Handle<SetKeymapBulk> for RynkService<'_> {
    async fn handle(&self, req: SetKeymapBulkRequest) -> Result<(), RynkError> {
        // Bounds are fully validated before the first write, so the run either
        // applies whole or the keymap stays untouched.
        let start = self.check_keymap_run(req.layer, req.start_row, req.start_col, req.actions.len())?;
        let (_, cols, _) = self.ctx.keymap_dimensions();
        for (offset, action) in (start..).zip(req.actions.iter()) {
            self.ctx
                .set_action(req.layer, (offset / cols) as u8, (offset % cols) as u8, *action)
                .await;
        }
        Ok(())
    }
}
