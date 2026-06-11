//! Combo handlers.

use rmk_types::combo::Combo as ComboConfig;
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
        if !self.ctx.with_combos(|combos| (r.index as usize) < combos.len()) {
            return Err(RynkError::Invalid);
        }
        self.ctx.set_combo(r.index, r.config).await;
        Ok(())
    }
}

#[cfg(feature = "bulk")]
impl Handle<GetComboBulk> for RynkService<'_> {
    async fn handle(&self, _req: GetComboBulkRequest) -> Result<GetComboBulkResponse, RynkError> {
        Err(RynkError::Unimplemented)
    }
}

#[cfg(feature = "bulk")]
impl Handle<SetComboBulk> for RynkService<'_> {
    async fn handle(&self, _req: SetComboBulkRequest) -> Result<(), RynkError> {
        Err(RynkError::Unimplemented)
    }
}
