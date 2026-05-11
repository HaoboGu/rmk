//! Combo endpoints.

use rmk_types::combo::Combo;
use rmk_types::protocol::rynk::{Cmd, RynkResult, SetComboRequest};

use crate::transport::{Transport, TransportError};

/// Read one combo entry by index.
pub async fn get_combo<T: Transport>(t: &mut T, index: u8) -> Result<Combo, TransportError> {
    t.request::<u8, Combo>(Cmd::GetCombo, &index).await
}

/// Write one combo entry by index.
pub async fn set_combo<T: Transport>(t: &mut T, index: u8, config: Combo) -> Result<RynkResult, TransportError> {
    let req = SetComboRequest { index, config };
    t.request::<SetComboRequest, RynkResult>(Cmd::SetCombo, &req).await
}
