//! Macro handlers — chunked read/write of the shared macro buffer.

use heapless::Vec;
use rmk_types::constants::MACRO_DATA_SIZE;
use rmk_types::protocol::rynk::{GetMacroRequest, MacroData, RynkError, RynkMessage, SetMacroRequest};

use super::super::RynkService;

impl<'a> RynkService<'a> {
    pub(crate) async fn handle_get_macro(&self, msg: &mut RynkMessage<'_>) -> Result<usize, RynkError> {
        let r = msg.request::<GetMacroRequest>()?;
        // Reply is always a full MACRO_DATA_SIZE chunk (zero-filled past the
        // macro space), so length is no end signal — the host terminates by
        // its capability size and the macro encoding itself.
        let _ = r.index; // reserved for a future per-macro indirection layer
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
