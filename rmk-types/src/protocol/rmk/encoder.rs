//! Encoder endpoint types.

use postcard::experimental::max_size::MaxSize;
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use crate::action::EncoderAction;

/// Request payload for `GetEncoderAction`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize, Schema)]
pub struct GetEncoderRequest {
    pub encoder_id: u8,
    pub layer: u8,
}

/// Request payload for `SetEncoderAction`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize, Schema)]
pub struct SetEncoderRequest {
    pub encoder_id: u8,
    pub layer: u8,
    pub action: EncoderAction,
}
