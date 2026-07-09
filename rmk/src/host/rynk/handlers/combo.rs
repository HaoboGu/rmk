//! Combo handlers.

use rmk_types::combo::Combo as ComboConfig;
#[cfg(feature = "bulk")]
use rmk_types::constants::BULK_SIZE;
use rmk_types::protocol::rynk::command::{GetCombo, SetCombo};
#[cfg(feature = "bulk")]
use rmk_types::protocol::rynk::command::{GetComboBulk, SetComboBulk};
#[cfg(feature = "bulk")]
use rmk_types::protocol::rynk::{GetComboBulkRequest, GetComboBulkResponse, SetComboBulkRequest};
use rmk_types::protocol::rynk::{RynkError, SetComboRequest};

use super::super::RynkService;
use super::Handle;
#[cfg(feature = "bulk")]
use super::bulk::{bulk_page, bulk_write_start};

impl Handle<GetCombo> for RynkService<'_> {
    async fn handle(&self, idx: u8) -> Result<ComboConfig, RynkError> {
        // An in-range but empty slot returns the empty config so the host
        // gets a uniform shape across hits and misses; an out-of-range index
        // is a semantic error.
        self.ctx.with_combos(|combos| {
            if (idx as usize) >= combos.len() {
                return Err(RynkError::Invalid);
            }
            Ok(combos[idx as usize]
                .as_ref()
                .map(|c| c.config.clone())
                .unwrap_or_else(ComboConfig::empty))
        })
    }
}

impl Handle<SetCombo> for RynkService<'_> {
    async fn handle(&self, r: SetComboRequest) -> Result<(), RynkError> {
        if self.ctx.set_combo(r.index, r.config).await {
            Ok(())
        } else {
            Err(RynkError::Invalid)
        }
    }
}

#[cfg(feature = "bulk")]
impl Handle<GetComboBulk> for RynkService<'_> {
    async fn handle(&self, req: GetComboBulkRequest) -> Result<GetComboBulkResponse, RynkError> {
        // Empty slots read back as the empty config, same as the single Get; an
        // out-of-range `start_index` yields an empty page.
        self.ctx.with_combos(|combos| {
            let page = bulk_page(req.start_index as usize, BULK_SIZE, combos.len());
            Ok(GetComboBulkResponse::from_iter_bounded(page.map(|i| {
                combos[i]
                    .as_ref()
                    .map(|c| c.config.clone())
                    .unwrap_or_else(ComboConfig::empty)
            })))
        })
    }
}

#[cfg(feature = "bulk")]
impl Handle<SetComboBulk> for RynkService<'_> {
    async fn handle(&self, req: SetComboBulkRequest) -> Result<(), RynkError> {
        // Validate the whole run first, so it applies whole or not at all. The
        // range then stays in bounds, so `set_combo`'s success is guaranteed.
        let num_combos = self.ctx.with_combos(|combos| combos.len());
        let start = bulk_write_start(req.start_index as usize, req.configs.len(), num_combos)?;
        for (idx, config) in (start..).zip(req.configs.iter()) {
            self.ctx.set_combo(idx as u8, config.clone()).await;
        }
        Ok(())
    }
}
