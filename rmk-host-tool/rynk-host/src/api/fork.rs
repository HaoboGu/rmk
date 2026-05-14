//! Fork endpoints.

use rmk_types::fork::Fork;
use rmk_types::protocol::rynk::{Cmd, SetForkRequest};

use crate::transport::{Transport, TransportError};
use crate::RynkResult;

/// Read one fork entry by index.
pub async fn get_fork<T: Transport>(t: &mut T, index: u8) -> Result<RynkResult<Fork>, TransportError> {
    t.request::<u8, RynkResult<Fork>>(Cmd::GetFork, &index).await
}

/// Write one fork entry by index.
pub async fn set_fork<T: Transport>(t: &mut T, index: u8, config: Fork) -> Result<RynkResult, TransportError> {
    let req = SetForkRequest { index, config };
    t.request::<SetForkRequest, RynkResult>(Cmd::SetFork, &req).await
}
