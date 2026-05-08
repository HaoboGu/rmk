//! Handlers for the `morse/*` endpoint group.

use postcard_rpc::header::VarHeader;
use rmk_types::morse::Morse;
use rmk_types::protocol::rmk::{RmkError, RmkResult, SetMorseRequest};

use super::super::Ctx;

pub(crate) async fn get_morse(ctx: &mut Ctx<'_>, _hdr: VarHeader, idx: u8) -> Morse {
    ctx.keymap.get_morse(idx as usize).unwrap_or_default()
}

pub(crate) async fn set_morse(ctx: &mut Ctx<'_>, _hdr: VarHeader, req: SetMorseRequest) -> RmkResult {
    let placed = ctx
        .keymap
        .with_morse_mut(req.index as usize, |slot| *slot = req.config.clone())
        .is_some();
    if !placed {
        return Err(RmkError::InvalidParameter);
    }
    #[cfg(feature = "storage")]
    crate::channel::FLASH_CHANNEL
        .send(crate::storage::FlashOperationMessage::Morse {
            idx: req.index,
            morse: req.config,
        })
        .await;
    Ok(())
}

#[cfg(feature = "bulk_transfer")]
pub(crate) mod bulk {
    use heapless::Vec;
    use postcard_rpc::header::VarHeader;
    use rmk_types::constants::BULK_SIZE;
    use rmk_types::morse::Morse;
    use rmk_types::protocol::rmk::{
        GetMorseBulkRequest, GetMorseBulkResponse, RmkError, RmkResult, SetMorseBulkRequest,
    };

    use super::super::super::Ctx;

    pub(crate) async fn get_morse_bulk(
        ctx: &mut Ctx<'_>,
        _hdr: VarHeader,
        req: GetMorseBulkRequest,
    ) -> GetMorseBulkResponse {
        let mut configs: Vec<Morse, BULK_SIZE> = Vec::new();
        for i in 0..req.count as usize {
            let idx = req.start_index as usize + i;
            let m = ctx.keymap.get_morse(idx).unwrap_or_default();
            if configs.push(m).is_err() {
                break;
            }
        }
        GetMorseBulkResponse { configs }
    }

    pub(crate) async fn set_morse_bulk(ctx: &mut Ctx<'_>, _hdr: VarHeader, req: SetMorseBulkRequest) -> RmkResult {
        for (i, cfg) in req.configs.iter().enumerate() {
            let idx = req.start_index as usize + i;
            let placed = ctx.keymap.with_morse_mut(idx, |slot| *slot = cfg.clone()).is_some();
            if !placed {
                return Err(RmkError::InvalidParameter);
            }
            #[cfg(feature = "storage")]
            {
                crate::channel::FLASH_CHANNEL
                    .send(crate::storage::FlashOperationMessage::Morse {
                        idx: idx as u8,
                        morse: cfg.clone(),
                    })
                    .await;
                let _ = crate::storage::FLASH_OPERATION_FINISHED.wait().await;
            }
        }
        Ok(())
    }
}
