//! Request types for protocol endpoints.

use postcard::experimental::max_size::MaxSize;
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use super::{ComboConfig, ForkConfig, MacroData, MorseConfig};
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

/// Request payload for `GetMacro`.
///
/// The host reads macro data in chunks of up to `MAX_MACRO_DATA` bytes.
/// A response shorter than `MAX_MACRO_DATA` signals the end of the macro.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, MaxSize, Schema)]
pub struct GetMacroRequest {
    pub index: u8,
    pub offset: u16,
}

/// Request payload for `SetMacro`.
///
/// The host writes macro data in chunks of up to `MAX_MACRO_DATA` bytes.
/// A final chunk shorter than `MAX_MACRO_DATA` signals the end of the macro.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, MaxSize, Schema)]
pub struct SetMacroRequest {
    pub index: u8,
    pub offset: u16,
    pub data: MacroData,
}

/// Request payload for `SetCombo`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, MaxSize, Schema)]
pub struct SetComboRequest {
    pub index: u8,
    pub config: ComboConfig,
}

/// Request payload for `SetMorse`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, MaxSize, Schema)]
pub struct SetMorseRequest {
    pub index: u8,
    pub config: MorseConfig,
}

/// Request payload for `SetFork`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, MaxSize, Schema)]
pub struct SetForkRequest {
    pub index: u8,
    pub config: ForkConfig,
}
