//! Global behavior config endpoints.

use rmk_types::protocol::rynk::{BehaviorConfig, Cmd};

use crate::RynkResult;
use crate::transport::{Transport, TransportError};

pub async fn get_behavior<T: Transport>(t: &mut T) -> Result<RynkResult<BehaviorConfig>, TransportError> {
    t.request::<(), RynkResult<BehaviorConfig>>(Cmd::GetBehaviorConfig, &())
        .await
}

pub async fn set_behavior<T: Transport>(t: &mut T, config: BehaviorConfig) -> Result<RynkResult, TransportError> {
    t.request::<BehaviorConfig, RynkResult>(Cmd::SetBehaviorConfig, &config)
        .await
}
