//! Fork endpoint types.

use postcard::experimental::max_size::MaxSize;
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use crate::fork::Fork;

/// Request payload for `SetFork`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize, Schema)]
pub struct SetForkRequest {
    pub index: u8,
    pub config: Fork,
}
