//! Fork handlers.

use rmk_types::protocol::rynk::{RynkError, SetForkRequest};

use super::super::RynkService;

impl<'a> RynkService<'a> {
    pub(crate) async fn handle_get_fork(&self, payload: &mut [u8]) -> Result<usize, RynkError> {
        let (idx, _) = postcard::take_from_bytes::<u8>(payload).map_err(|_| RynkError::InvalidRequest)?;
        let fork = self.ctx.get_fork(idx).unwrap_or_default();
        Self::write_response(&fork, payload)
    }

    pub(crate) async fn handle_set_fork(&self, payload: &mut [u8]) -> Result<usize, RynkError> {
        let (r, _) = postcard::take_from_bytes::<SetForkRequest>(payload).map_err(|_| RynkError::InvalidRequest)?;
        self.ctx.set_fork(r.index, r.config).await;
        Self::write_response(&(), payload)
    }
}
