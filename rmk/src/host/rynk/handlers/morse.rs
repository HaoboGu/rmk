//! Morse handlers.

use rmk_types::morse::Morse;
use rmk_types::protocol::rynk::command::{GetMorse, SetMorse};
#[cfg(feature = "bulk")]
use rmk_types::protocol::rynk::command::{GetMorseBulk, SetMorseBulk};
#[cfg(feature = "bulk")]
use rmk_types::protocol::rynk::{GetMorseBulkRequest, GetMorseBulkResponse, SetMorseBulkRequest};
use rmk_types::protocol::rynk::{RynkError, SetMorseRequest};

use super::super::RynkService;
use super::Handle;

impl Handle<GetMorse> for RynkService<'_> {
    async fn handle(&self, idx: u8) -> Result<Morse, RynkError> {
        self.ctx.get_morse(idx).ok_or(RynkError::Invalid)
    }
}

impl Handle<SetMorse> for RynkService<'_> {
    async fn handle(&self, r: SetMorseRequest) -> Result<(), RynkError> {
        if (r.index as usize) >= self.ctx.morses_len() {
            return Err(RynkError::Invalid);
        }
        self.ctx
            .update_morse(r.index, |m| {
                *m = r.config;
            })
            .await;
        Ok(())
    }
}

#[cfg(feature = "bulk")]
impl Handle<GetMorseBulk> for RynkService<'_> {
    async fn handle(&self, _req: GetMorseBulkRequest) -> Result<GetMorseBulkResponse, RynkError> {
        Err(RynkError::Unimplemented)
    }
}

#[cfg(feature = "bulk")]
impl Handle<SetMorseBulk> for RynkService<'_> {
    async fn handle(&self, _req: SetMorseBulkRequest) -> Result<(), RynkError> {
        Err(RynkError::Unimplemented)
    }
}
