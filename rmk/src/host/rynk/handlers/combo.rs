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
        let start = req.start_index as usize;
        let count = req.count as usize;
        // Same slot mapping as the single Get: empty slots read back as the
        // empty config, only out-of-range indices are errors.
        self.ctx.with_combos(|combos| {
            if count == 0 || count > BULK_SIZE || start + count > combos.len() {
                return Err(RynkError::Invalid);
            }
            let mut configs = heapless::Vec::new();
            for slot in &combos[start..start + count] {
                configs
                    .push(
                        slot.as_ref()
                            .map(|c| c.config.clone())
                            .unwrap_or_else(ComboConfig::empty),
                    )
                    .map_err(|_| RynkError::Internal)?;
            }
            Ok(GetComboBulkResponse { configs })
        })
    }
}

#[cfg(feature = "bulk")]
impl Handle<SetComboBulk> for RynkService<'_> {
    async fn handle(&self, req: SetComboBulkRequest) -> Result<(), RynkError> {
        let start = req.start_index as usize;
        // Bounds are fully validated before the first write, so the run either
        // applies whole or the combos stay untouched.
        let num_combos = self.ctx.with_combos(|combos| combos.len());
        if req.configs.is_empty() || start + req.configs.len() > num_combos {
            return Err(RynkError::Invalid);
        }
        for (idx, config) in (start..).zip(req.configs.iter()) {
            if !self.ctx.set_combo(idx as u8, config.clone()).await {
                return Err(RynkError::Invalid);
            }
        }
        Ok(())
    }
}
