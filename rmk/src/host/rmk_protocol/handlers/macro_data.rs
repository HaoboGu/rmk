//! Handlers for the `macro/*` endpoint group.

use heapless::Vec;
use postcard_rpc::header::VarHeader;
use rmk_types::constants::MACRO_DATA_SIZE;
use rmk_types::protocol::rmk::{GetMacroRequest, MacroData, RmkResult, SetMacroRequest};

use super::super::Ctx;
use crate::MACRO_SPACE_SIZE;

pub(crate) async fn get_macro(ctx: &mut Ctx<'_>, _hdr: VarHeader, req: GetMacroRequest) -> MacroData {
    let mut buf = [0u8; MACRO_DATA_SIZE];
    let offset = req.offset as usize;
    let mut data: Vec<u8, MACRO_DATA_SIZE> = Vec::new();
    if offset >= MACRO_SPACE_SIZE {
        return MacroData { data };
    }
    let take = MACRO_DATA_SIZE.min(MACRO_SPACE_SIZE.saturating_sub(offset));
    ctx.keymap.read_macro_buffer(offset, &mut buf[..take]);
    let _ = data.extend_from_slice(&buf[..take]);
    MacroData { data }
}

pub(crate) async fn set_macro(ctx: &mut Ctx<'_>, _hdr: VarHeader, req: SetMacroRequest) -> RmkResult {
    let offset = req.offset as usize;
    if offset >= MACRO_SPACE_SIZE {
        return Err(rmk_types::protocol::rmk::RmkError::InvalidParameter);
    }
    if offset == 0 {
        ctx.keymap.reset_macro_buffer();
    }
    ctx.keymap.write_macro_buffer(offset, &req.data.data);
    #[cfg(feature = "storage")]
    {
        let buf = ctx.keymap.get_macro_sequences();
        crate::channel::FLASH_CHANNEL
            .send(crate::storage::FlashOperationMessage::MacroData(buf))
            .await;
    }
    Ok(())
}
