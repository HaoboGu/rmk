//! Combo handlers.

use rmk_types::combo::Combo as ComboConfig;
use rmk_types::protocol::rynk::{RynkError, SetComboRequest};

use super::super::RynkService;

impl<'a> RynkService<'a> {
    pub(crate) async fn handle_get_combo(&self, payload: &mut [u8]) -> Result<usize, RynkError> {
        let (idx, _) = postcard::take_from_bytes::<u8>(payload).map_err(|_| RynkError::Malformed)?;
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
        Self::write_response(&cfg, payload)
    }

    pub(crate) async fn handle_set_combo(&self, payload: &mut [u8]) -> Result<usize, RynkError> {
        let (r, _) = postcard::take_from_bytes::<SetComboRequest>(payload).map_err(|_| RynkError::Malformed)?;
        if !self.ctx.with_combos(|combos| (r.index as usize) < combos.len()) {
            return Err(RynkError::Invalid);
        }
        self.ctx.set_combo(r.index, r.config).await;
        Self::write_response(&(), payload)
    }

    #[cfg(feature = "bulk_transfer")]
    pub(crate) async fn handle_get_combo_bulk(&self, _payload: &mut [u8]) -> Result<usize, RynkError> {
        Err(RynkError::Unimplemented)
    }

    #[cfg(feature = "bulk_transfer")]
    pub(crate) async fn handle_set_combo_bulk(&self, _payload: &mut [u8]) -> Result<usize, RynkError> {
        Err(RynkError::Unimplemented)
    }
}
