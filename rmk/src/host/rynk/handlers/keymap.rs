//! Keymap and encoder handlers (encoder is part of keymap's `0x01xx` Cmd group).

use rmk_types::action::EncoderAction;
use rmk_types::protocol::rynk::{GetEncoderRequest, KeyPosition, RynkError, SetEncoderRequest, SetKeyRequest};

use super::super::RynkService;

impl<'a> RynkService<'a> {
    pub(crate) async fn handle_get_key_action(&self, payload: &mut [u8]) -> Result<usize, RynkError> {
        let (pos, _) = postcard::take_from_bytes::<KeyPosition>(payload).map_err(|_| RynkError::InvalidRequest)?;
        let (rows, cols, num_layers) = self.ctx.keymap_dimensions();
        // Out-of-range reads return KeyAction::No so callers don't need a
        // separate sentinel — keeps the wire envelope shape uniform with hits.
        let action = if (pos.layer as usize) >= num_layers || (pos.row as usize) >= rows || (pos.col as usize) >= cols {
            rmk_types::action::KeyAction::No
        } else {
            self.ctx.get_action(pos.layer, pos.row, pos.col)
        };
        Self::write_response(&action, payload)
    }

    pub(crate) async fn handle_set_key_action(&self, payload: &mut [u8]) -> Result<usize, RynkError> {
        let (set, _) = postcard::take_from_bytes::<SetKeyRequest>(payload).map_err(|_| RynkError::InvalidRequest)?;
        let (rows, cols, num_layers) = self.ctx.keymap_dimensions();
        if (set.position.layer as usize) >= num_layers
            || (set.position.row as usize) >= rows
            || (set.position.col as usize) >= cols
        {
            return Err(RynkError::InvalidRequest);
        }
        self.ctx
            .set_action(set.position.layer, set.position.row, set.position.col, set.action)
            .await;
        Self::write_response(&(), payload)
    }

    pub(crate) async fn handle_get_default_layer(&self, payload: &mut [u8]) -> Result<usize, RynkError> {
        let layer = self.ctx.default_layer();
        Self::write_response(&layer, payload)
    }

    pub(crate) async fn handle_set_default_layer(&self, payload: &mut [u8]) -> Result<usize, RynkError> {
        let (layer, _) = postcard::take_from_bytes::<u8>(payload).map_err(|_| RynkError::InvalidRequest)?;
        let (_, _, num_layers) = self.ctx.keymap_dimensions();
        if (layer as usize) >= num_layers {
            return Err(RynkError::InvalidRequest);
        }
        self.ctx.set_default_layer(layer).await;
        Self::write_response(&(), payload)
    }

    pub(crate) async fn handle_get_encoder_action(&self, payload: &mut [u8]) -> Result<usize, RynkError> {
        let (r, _) = postcard::take_from_bytes::<GetEncoderRequest>(payload).map_err(|_| RynkError::InvalidRequest)?;
        let action = self
            .ctx
            .get_encoder(r.layer, r.encoder_id)
            .unwrap_or_else(EncoderAction::default);
        Self::write_response(&action, payload)
    }

    pub(crate) async fn handle_set_encoder_action(&self, payload: &mut [u8]) -> Result<usize, RynkError> {
        let (r, _) = postcard::take_from_bytes::<SetEncoderRequest>(payload).map_err(|_| RynkError::InvalidRequest)?;
        // No clean "set whole encoder" accessor exists yet — split into two writes.
        self.ctx
            .set_encoder_clockwise(r.layer, r.encoder_id, r.action.clockwise)
            .await;
        self.ctx
            .set_encoder_counter_clockwise(r.layer, r.encoder_id, r.action.counter_clockwise)
            .await;
        Self::write_response(&(), payload)
    }

    #[cfg(feature = "bulk_transfer")]
    pub(crate) async fn handle_get_keymap_bulk(&self, _payload: &mut [u8]) -> Result<usize, RynkError> {
        // Bulk handlers wired in a follow-up pass.
        Err(RynkError::Internal)
    }

    #[cfg(feature = "bulk_transfer")]
    pub(crate) async fn handle_set_keymap_bulk(&self, _payload: &mut [u8]) -> Result<usize, RynkError> {
        Err(RynkError::Internal)
    }
}
