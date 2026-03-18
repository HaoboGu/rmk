//! Request types for protocol endpoints.

use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use super::{ComboConfig, ForkConfig, MacroData, MorseConfig};
use crate::action::EncoderAction;

/// Request payload for `GetEncoderAction`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct GetEncoderRequest {
    pub encoder_id: u8,
    pub layer: u8,
}

/// Request payload for `SetEncoderAction`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct SetEncoderRequest {
    pub encoder_id: u8,
    pub layer: u8,
    pub action: EncoderAction,
}

/// Request payload for `SetMacro`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct SetMacroRequest {
    pub index: u8,
    pub data: MacroData,
}

/// Request payload for `SetCombo`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct SetComboRequest {
    pub index: u8,
    pub config: ComboConfig,
}

/// Request payload for `SetMorse`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct SetMorseRequest {
    pub index: u8,
    pub config: MorseConfig,
}

/// Request payload for `SetFork`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct SetForkRequest {
    pub index: u8,
    pub config: ForkConfig,
}
