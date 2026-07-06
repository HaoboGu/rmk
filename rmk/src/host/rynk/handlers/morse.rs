//! Morse handlers.

#[cfg(feature = "bulk")]
use rmk_types::constants::BULK_SIZE;
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
    async fn handle(&self, req: GetMorseBulkRequest) -> Result<GetMorseBulkResponse, RynkError> {
        let start = req.start_index as usize;
        let count = req.count as usize;
        if count == 0 || count > BULK_SIZE || start + count > self.ctx.morses_len() {
            return Err(RynkError::Invalid);
        }
        let mut configs = heapless::Vec::new();
        for idx in start..start + count {
            configs
                .push(self.ctx.get_morse(idx as u8).ok_or(RynkError::Invalid)?)
                .map_err(|_| RynkError::Internal)?;
        }
        Ok(GetMorseBulkResponse { configs })
    }
}

#[cfg(feature = "bulk")]
impl Handle<SetMorseBulk> for RynkService<'_> {
    async fn handle(&self, req: SetMorseBulkRequest) -> Result<(), RynkError> {
        let start = req.start_index as usize;
        // Bounds are fully validated before the first write, so the run either
        // applies whole or the morses stay untouched.
        if req.configs.is_empty() || start + req.configs.len() > self.ctx.morses_len() {
            return Err(RynkError::Invalid);
        }
        for (idx, config) in (start..).zip(req.configs.iter()) {
            self.ctx
                .update_morse(idx as u8, |m| {
                    *m = config.clone();
                })
                .await;
        }
        Ok(())
    }
}
