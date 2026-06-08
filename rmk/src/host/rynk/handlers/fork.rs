//! Fork handlers.

use rmk_types::protocol::rynk::{RynkError, RynkMessage, SetForkRequest};

use super::super::RynkService;

impl<'a> RynkService<'a> {
    pub(crate) async fn handle_get_fork(&self, msg: &mut RynkMessage<'_>) -> Result<usize, RynkError> {
        let idx = msg.request::<u8>()?;
        let fork = self.ctx.get_fork(idx).ok_or(RynkError::Invalid)?;
        Self::write_response(&fork, msg.response_payload_mut())
    }

    pub(crate) async fn handle_set_fork(&self, msg: &mut RynkMessage<'_>) -> Result<usize, RynkError> {
        let r = msg.request::<SetForkRequest>()?;
        if self.ctx.get_fork(r.index).is_none() {
            return Err(RynkError::Invalid);
        }
        self.ctx.set_fork(r.index, r.config).await;
        Self::write_response(&(), msg.response_payload_mut())
    }
}
