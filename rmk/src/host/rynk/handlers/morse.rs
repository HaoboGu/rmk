//! Morse handlers.

use rmk_types::protocol::rynk::{RynkError, RynkMessage, SetMorseRequest};

use super::super::RynkService;

impl<'a> RynkService<'a> {
    pub(crate) async fn handle_get_morse(&self, msg: &mut RynkMessage<'_>) -> Result<usize, RynkError> {
        let idx = msg.request::<u8>()?;
        let morse = self.ctx.get_morse(idx).ok_or(RynkError::Invalid)?;
        Self::write_response(&morse, msg.response_payload_mut())
    }

    pub(crate) async fn handle_set_morse(&self, msg: &mut RynkMessage<'_>) -> Result<usize, RynkError> {
        let r = msg.request::<SetMorseRequest>()?;
        if (r.index as usize) >= self.ctx.morses_len() {
            return Err(RynkError::Invalid);
        }
        self.ctx
            .update_morse(r.index, |m| {
                *m = r.config;
            })
            .await;
        Self::write_response(&(), msg.response_payload_mut())
    }

    #[cfg(feature = "bulk")]
    pub(crate) async fn handle_get_morse_bulk(&self, _msg: &mut RynkMessage<'_>) -> Result<usize, RynkError> {
        Err(RynkError::Unimplemented)
    }

    #[cfg(feature = "bulk")]
    pub(crate) async fn handle_set_morse_bulk(&self, _msg: &mut RynkMessage<'_>) -> Result<usize, RynkError> {
        Err(RynkError::Unimplemented)
    }
}
