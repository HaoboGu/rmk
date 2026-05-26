//! Fork handlers.

use rmk_types::protocol::rynk::{RynkError, SetForkRequest};

use super::super::RynkService;

impl<'a> RynkService<'a> {
    pub(crate) async fn handle_get_fork(&self, payload: &mut [u8]) -> Result<usize, RynkError> {
        let (idx, _) = postcard::take_from_bytes::<u8>(payload).map_err(|_| RynkError::Malformed)?;
        let fork = self.ctx.get_fork(idx).ok_or(RynkError::Invalid)?;
        Self::write_response(&fork, payload)
    }

    pub(crate) async fn handle_set_fork(&self, payload: &mut [u8]) -> Result<usize, RynkError> {
        let (r, _) = postcard::take_from_bytes::<SetForkRequest>(payload).map_err(|_| RynkError::Malformed)?;
        if self.ctx.get_fork(r.index).is_none() {
            return Err(RynkError::Invalid);
        }
        self.ctx.set_fork(r.index, r.config).await;
        Self::write_response(&(), payload)
    }
}
