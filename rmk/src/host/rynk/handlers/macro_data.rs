//! Macro handlers — chunked read/write of the shared macro buffer.

use heapless::Vec;
use rmk_types::constants::MACRO_DATA_SIZE;
use rmk_types::protocol::rynk::command::{GetMacro, SetMacro};
use rmk_types::protocol::rynk::{GetMacroRequest, MacroData, RynkError, SetMacroRequest};

use super::super::RynkService;
use super::Handle;

impl Handle<GetMacro> for RynkService<'_> {
    async fn handle(&self, r: GetMacroRequest) -> Result<MacroData, RynkError> {
        // Full chunks are zero-filled; length is not an end signal.
        let mut data: Vec<u8, MACRO_DATA_SIZE> = Vec::new();
        data.resize_default(MACRO_DATA_SIZE).expect("MACRO_DATA_SIZE matches");
        self.ctx.read_macro_buffer(r.offset as usize, &mut data);
        Ok(MacroData { data })
    }
}

impl Handle<SetMacro> for RynkService<'_> {
    async fn handle(&self, r: SetMacroRequest) -> Result<(), RynkError> {
        self.ctx.write_macro_buffer(r.offset as usize, &r.data.data).await;
        Ok(())
    }
}
