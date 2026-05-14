//! Macro handlers — chunked read/write of the shared macro buffer.

use heapless::Vec;
use rmk_types::constants::MACRO_DATA_SIZE;
use rmk_types::protocol::rynk::{GetMacroRequest, MacroData, RynkError, SetMacroRequest};

use super::super::RynkService;

impl<'a> RynkService<'a> {
    pub(crate) async fn handle_get_macro(&self, payload: &mut [u8]) -> Result<usize, RynkError> {
        let (r, _) = postcard::take_from_bytes::<GetMacroRequest>(payload).map_err(|_| RynkError::InvalidRequest)?;
        // Read up to MACRO_DATA_SIZE bytes starting at `offset`. The keymap
        // accessor zero-fills any read that runs past the end of the macro
        // space, so the host detects "macro complete" by a chunk shorter
        // than MACRO_DATA_SIZE (or an all-zero terminator).
        //
        // `index` is reserved for a future per-macro indirection layer and is
        // not used today — the entire buffer is one flat region.
        let _ = r.index;
        let mut buf = [0u8; MACRO_DATA_SIZE];
        self.ctx.read_macro_buffer(r.offset as usize, &mut buf);
        let mut data: Vec<u8, MACRO_DATA_SIZE> = Vec::new();
        data.extend_from_slice(&buf).expect("MACRO_DATA_SIZE matches");
        Self::write_response(&MacroData { data }, payload)
    }

    pub(crate) async fn handle_set_macro(&self, payload: &mut [u8]) -> Result<usize, RynkError> {
        let (r, _) = postcard::take_from_bytes::<SetMacroRequest>(payload).map_err(|_| RynkError::InvalidRequest)?;
        let _ = r.index;
        self.ctx.write_macro_buffer(r.offset as usize, &r.data.data).await;
        Self::write_response(&(), payload)
    }
}
