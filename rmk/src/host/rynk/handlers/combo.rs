//! Combo handlers.

use rmk_types::combo::Combo as ComboConfig;
use rmk_types::protocol::rynk::{RynkError, RynkMessage, SetComboRequest};

use super::super::RynkService;

impl<'a> RynkService<'a> {
    pub(crate) async fn handle_get_combo(&self, msg: &mut RynkMessage<'_>) -> Result<usize, RynkError> {
        let idx = msg.request::<u8>()?;
        // An in-range but empty slot returns the empty config so the host
        // gets a uniform shape across hits and misses; an out-of-range index
        // is a semantic error.
        let cfg = self.ctx.with_combos(|combos| {
            if (idx as usize) >= combos.len() {
                return Err(RynkError::Invalid);
            }
            Ok(combos[idx as usize]
                .as_ref()
                .map(|c| c.config.clone())
                .unwrap_or_else(ComboConfig::empty))
        })?;
        Self::write_response(&cfg, msg.response_payload_mut())
    }

    pub(crate) async fn handle_set_combo(&self, msg: &mut RynkMessage<'_>) -> Result<usize, RynkError> {
        let r = msg.request::<SetComboRequest>()?;
        if !self.ctx.with_combos(|combos| (r.index as usize) < combos.len()) {
            return Err(RynkError::Invalid);
        }
        self.ctx.set_combo(r.index, r.config).await;
        Self::write_response(&(), msg.response_payload_mut())
    }

    #[cfg(feature = "bulk")]
    pub(crate) async fn handle_get_combo_bulk(&self, _msg: &mut RynkMessage<'_>) -> Result<usize, RynkError> {
        Err(RynkError::Unimplemented)
    }

    #[cfg(feature = "bulk")]
    pub(crate) async fn handle_set_combo_bulk(&self, _msg: &mut RynkMessage<'_>) -> Result<usize, RynkError> {
        Err(RynkError::Unimplemented)
    }
}
