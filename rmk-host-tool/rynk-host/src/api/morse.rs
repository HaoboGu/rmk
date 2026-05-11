//! Morse endpoints.

use rmk_types::morse::Morse;
use rmk_types::protocol::rynk::{Cmd, RynkResult, SetMorseRequest};

use crate::transport::{Transport, TransportError};

/// Read one morse entry by index.
pub async fn get_morse<T: Transport>(t: &mut T, index: u8) -> Result<Morse, TransportError> {
    t.request::<u8, Morse>(Cmd::GetMorse, &index).await
}

/// Write one morse entry by index.
pub async fn set_morse<T: Transport>(t: &mut T, index: u8, config: Morse) -> Result<RynkResult, TransportError> {
    let req = SetMorseRequest { index, config };
    t.request::<SetMorseRequest, RynkResult>(Cmd::SetMorse, &req).await
}
