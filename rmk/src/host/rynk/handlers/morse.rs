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
#[cfg(feature = "bulk")]
use super::bulk::{bulk_page, bulk_write_start};

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
        // `bulk_page` keeps every index in range, so `get_morse` is always
        // `Some`; `filter_map` unwraps without a fallback.
        let page = bulk_page(req.start_index as usize, BULK_SIZE, self.ctx.morses_len());
        Ok(GetMorseBulkResponse::from_iter_bounded(
            page.filter_map(|idx| self.ctx.get_morse(idx as u8)),
        ))
    }
}

#[cfg(feature = "bulk")]
impl Handle<SetMorseBulk> for RynkService<'_> {
    async fn handle(&self, req: SetMorseBulkRequest) -> Result<(), RynkError> {
        // Validate the whole run first, so it applies whole or not at all.
        let start = bulk_write_start(req.start_index as usize, req.configs.len(), self.ctx.morses_len())?;
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
