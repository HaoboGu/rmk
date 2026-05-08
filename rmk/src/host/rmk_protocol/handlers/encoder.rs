//! Handlers for the `encoder/*` endpoint group.

use postcard_rpc::header::VarHeader;
use rmk_types::action::EncoderAction;
use rmk_types::protocol::rmk::{GetEncoderRequest, RmkError, RmkResult, SetEncoderRequest};

use super::super::Ctx;

pub(crate) async fn get_encoder_action(ctx: &mut Ctx<'_>, _hdr: VarHeader, req: GetEncoderRequest) -> EncoderAction {
    ctx.keymap
        .get_encoder_action(req.layer as usize, req.encoder_id as usize)
        .unwrap_or_default()
}

pub(crate) async fn set_encoder_action(ctx: &mut Ctx<'_>, _hdr: VarHeader, req: SetEncoderRequest) -> RmkResult {
    let layer = req.layer as usize;
    let id = req.encoder_id as usize;
    if ctx
        .keymap
        .set_encoder_clockwise(layer, id, req.action.clockwise)
        .is_none()
    {
        return Err(RmkError::InvalidParameter);
    }
    if ctx
        .keymap
        .set_encoder_counter_clockwise(layer, id, req.action.counter_clockwise)
        .is_none()
    {
        return Err(RmkError::InvalidParameter);
    }
    #[cfg(feature = "storage")]
    crate::channel::FLASH_CHANNEL
        .send(crate::storage::FlashOperationMessage::Encoder {
            layer: req.layer,
            idx: req.encoder_id,
            action: req.action,
        })
        .await;
    Ok(())
}
