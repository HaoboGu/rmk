//! Morse handlers.

use rmk_types::constants::BULK_SIZE;
use rmk_types::morse::Morse;
use rmk_types::protocol::rynk::command::{GetMorse, GetMorseBulk, SetMorse, SetMorseBulk};
use rmk_types::protocol::rynk::{GetMorseBulkRequest, RynkError, RynkMessage, SetMorseRequest};

use super::super::RynkService;
use super::bulk::{bulk_page, bulk_write_start, take_element, take_seq_len, validate_bulk_elements};
use super::{Handle, HandleBulk};

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

impl HandleBulk<GetMorseBulk> for RynkService<'_> {
    async fn handle_bulk(&self, msg: &mut RynkMessage<'_>) -> Result<(), RynkError> {
        let req = msg.decode_request::<GetMorseBulkRequest>()?;
        let page = bulk_page(req.start_index as usize, BULK_SIZE, self.ctx.morses_len());
        let count = page.len();
        msg.encode_bulk_ok(count, page.map(|idx| self.ctx.get_morse(idx as u8).unwrap_or_default()))
    }
}

impl HandleBulk<SetMorseBulk> for RynkService<'_> {
    async fn handle_bulk(&self, msg: &mut RynkMessage<'_>) -> Result<(), RynkError> {
        let (start_index, rest) = postcard::take_from_bytes::<u8>(msg.payload()).map_err(|_| RynkError::Malformed)?;
        let (count, elements) = take_seq_len(rest)?;

        let start = bulk_write_start(start_index as usize, count, self.ctx.morses_len())?;
        validate_bulk_elements::<Morse>(elements, count)?;

        let mut cursor = elements;
        for idx in start..start + count {
            let config = take_element::<Morse>(&mut cursor)?;
            self.ctx.update_morse(idx as u8, |m| *m = config).await;
        }
        msg.encode_response(&())
    }
}
