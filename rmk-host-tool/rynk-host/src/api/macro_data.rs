//! Macro endpoints.

use rmk_types::protocol::rynk::{Cmd, GetMacroRequest, MacroData, SetMacroRequest};

use crate::RynkResult;
use crate::transport::{Transport, TransportError};

/// Read a chunk of one macro starting at `offset`. A response shorter
/// than `MACRO_DATA_SIZE` signals the end of the macro.
pub async fn get_macro<T: Transport>(
    t: &mut T,
    index: u8,
    offset: u16,
) -> Result<RynkResult<MacroData>, TransportError> {
    let req = GetMacroRequest { index, offset };
    t.request::<GetMacroRequest, RynkResult<MacroData>>(Cmd::GetMacro, &req)
        .await
}

/// Write a chunk of one macro starting at `offset`. A final chunk shorter
/// than `MACRO_DATA_SIZE` signals the end of the macro to the firmware.
pub async fn set_macro<T: Transport>(
    t: &mut T,
    index: u8,
    offset: u16,
    data: MacroData,
) -> Result<RynkResult, TransportError> {
    let req = SetMacroRequest { index, offset, data };
    t.request::<SetMacroRequest, RynkResult>(Cmd::SetMacro, &req).await
}
