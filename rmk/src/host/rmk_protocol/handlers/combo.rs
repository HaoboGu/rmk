//! Handlers for the `combo/*` endpoint group.
//!
//! `KeyMap::with_combos*` exposes runtime `keyboard::combo::Combo` (config plus
//! transient state); the wire type `rmk_types::combo::Combo` is the inner
//! `config`, so we adapt at the handler boundary.

use postcard_rpc::header::VarHeader;
use rmk_types::combo::Combo as WireCombo;
use rmk_types::protocol::rmk::{RmkError, RmkResult, SetComboRequest};

use super::super::Ctx;
use crate::keyboard::combo::Combo as RuntimeCombo;

pub(crate) async fn get_combo(ctx: &mut Ctx<'_>, _hdr: VarHeader, idx: u8) -> WireCombo {
    ctx.keymap
        .with_combos(|combos| {
            combos
                .get(idx as usize)
                .and_then(|c| c.as_ref())
                .map(|c| c.config.clone())
        })
        .unwrap_or_else(WireCombo::empty)
}

pub(crate) async fn set_combo(ctx: &mut Ctx<'_>, _hdr: VarHeader, req: SetComboRequest) -> RmkResult {
    let placed = ctx.keymap.with_combos_mut(|combos| {
        if let Some(slot) = combos.get_mut(req.index as usize) {
            *slot = Some(RuntimeCombo::new(req.config.clone()));
            true
        } else {
            false
        }
    });
    if !placed {
        return Err(RmkError::InvalidParameter);
    }
    #[cfg(feature = "storage")]
    crate::channel::FLASH_CHANNEL
        .send(crate::storage::FlashOperationMessage::Combo {
            idx: req.index,
            config: req.config,
        })
        .await;
    Ok(())
}

#[cfg(feature = "bulk_transfer")]
pub(crate) mod bulk {
    use heapless::Vec;
    use postcard_rpc::header::VarHeader;
    use rmk_types::combo::Combo as WireCombo;
    use rmk_types::constants::BULK_SIZE;
    use rmk_types::protocol::rmk::{
        GetComboBulkRequest, GetComboBulkResponse, RmkError, RmkResult, SetComboBulkRequest,
    };

    use super::super::super::Ctx;
    use crate::keyboard::combo::Combo as RuntimeCombo;

    pub(crate) async fn get_combo_bulk(
        ctx: &mut Ctx<'_>,
        _hdr: VarHeader,
        req: GetComboBulkRequest,
    ) -> GetComboBulkResponse {
        let mut configs: Vec<WireCombo, BULK_SIZE> = Vec::new();
        ctx.keymap.with_combos(|combos| {
            for i in 0..req.count as usize {
                let idx = req.start_index as usize + i;
                let c = combos
                    .get(idx)
                    .and_then(|c| c.as_ref())
                    .map(|c| c.config.clone())
                    .unwrap_or_else(WireCombo::empty);
                if configs.push(c).is_err() {
                    break;
                }
            }
        });
        GetComboBulkResponse { configs }
    }

    pub(crate) async fn set_combo_bulk(ctx: &mut Ctx<'_>, _hdr: VarHeader, req: SetComboBulkRequest) -> RmkResult {
        let len = ctx.keymap.with_combos_mut(|combos| combos.len());
        let start = req.start_index as usize;
        if start >= len {
            return Err(RmkError::InvalidParameter);
        }
        for (i, cfg) in req.configs.iter().enumerate() {
            let idx = start + i;
            if idx >= len {
                break;
            }
            ctx.keymap.with_combos_mut(|combos| {
                combos[idx] = Some(RuntimeCombo::new(cfg.clone()));
            });
            #[cfg(feature = "storage")]
            {
                crate::channel::FLASH_CHANNEL
                    .send(crate::storage::FlashOperationMessage::Combo {
                        idx: idx as u8,
                        config: cfg.clone(),
                    })
                    .await;
                let _ = crate::storage::FLASH_OPERATION_FINISHED.wait().await;
            }
        }
        Ok(())
    }
}
