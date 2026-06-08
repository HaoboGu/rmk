//! Macro handlers — chunked read/write of the shared macro buffer.

use heapless::Vec;
use rmk_types::constants::MACRO_DATA_SIZE;
use rmk_types::protocol::rynk::{GetMacroRequest, MacroData, RynkError, RynkMessage, SetMacroRequest};

use super::super::RynkService;

impl<'a> RynkService<'a> {
    pub(crate) async fn handle_get_macro(&self, msg: &mut RynkMessage<'_>) -> Result<usize, RynkError> {
        let r = msg.request::<GetMacroRequest>()?;
        // Read exactly MACRO_DATA_SIZE bytes starting at `offset` — the reply
        // is ALWAYS a full-length chunk. The keymap accessor zero-fills any
        // read that runs past the end of the macro space, so a short chunk is
        // never an end signal: the host bounds its reads by the
        // `macro_space_size` capability and parses the macro encoding itself
        // for termination.
        //
        // `index` is reserved for a future per-macro indirection layer and is
        // not used today — the entire buffer is one flat region.
        let _ = r.index;
        let mut buf = [0u8; MACRO_DATA_SIZE];
        self.ctx.read_macro_buffer(r.offset as usize, &mut buf);
        let mut data: Vec<u8, MACRO_DATA_SIZE> = Vec::new();
        data.extend_from_slice(&buf).expect("MACRO_DATA_SIZE matches");
        Self::write_response(&MacroData { data }, msg.response_payload_mut())
    }

    pub(crate) async fn handle_set_macro(&self, msg: &mut RynkMessage<'_>) -> Result<usize, RynkError> {
        let r = msg.request::<SetMacroRequest>()?;
        let _ = r.index;
        self.ctx.write_macro_buffer(r.offset as usize, &r.data.data).await;
        Self::write_response(&(), msg.response_payload_mut())
    }
}
