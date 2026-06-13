//! Macro handlers — chunked read/write of the shared macro buffer.

use heapless::Vec;
use rmk_types::constants::MACRO_DATA_SIZE;
use rmk_types::protocol::rynk::command::{GetMacro, SetMacro};
use rmk_types::protocol::rynk::{GetMacroRequest, MacroData, RynkError, SetMacroRequest};

use super::super::RynkService;
use super::Handle;

impl Handle<GetMacro> for RynkService<'_> {
    async fn handle(&self, r: GetMacroRequest) -> Result<MacroData, RynkError> {
        // Reply is always a full MACRO_DATA_SIZE chunk (zero-filled past the
        // macro space), so length is no end signal — the host terminates by
        // its capability size and the macro encoding itself.
        let _ = r.index; // reserved for a future per-macro indirection layer
        let mut buf = [0u8; MACRO_DATA_SIZE];
        self.ctx.read_macro_buffer(r.offset as usize, &mut buf);
        let mut data: Vec<u8, MACRO_DATA_SIZE> = Vec::new();
        data.extend_from_slice(&buf).expect("MACRO_DATA_SIZE matches");
        Ok(MacroData { data })
    }
}

impl Handle<SetMacro> for RynkService<'_> {
    async fn handle(&self, r: SetMacroRequest) -> Result<(), RynkError> {
        let _ = r.index;
        self.ctx.write_macro_buffer(r.offset as usize, &r.data.data).await;
        Ok(())
    }
}
