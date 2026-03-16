//! Behavior configuration protocol types.

use heapless::Vec;
use serde::{Deserialize, Serialize};

use super::{MAX_COMBO_KEYS, MAX_MORSE_PATTERNS};
use crate::action::{KeyAction, MorseProfile};

// Re-export BehaviorConfig so it remains accessible via protocol::rmk::BehaviorConfig
pub use crate::config::BehaviorConfig;

/// Protocol-facing morse/tap-dance configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct MorseConfig {
    pub profile: MorseProfile,
    pub patterns: Vec<MorsePatternEntry, MAX_MORSE_PATTERNS>,
}

/// A single morse pattern/action pair.
///
/// The `pattern` field encodes the morse sequence as a bitfield:
/// - Bits are read LSB-first; 0 = short press (dot), 1 = long press (dash).
/// - The sequence length is determined by the highest set bit + 1 in the
///   pattern (or by the firmware's morse implementation).
/// - See the firmware's `MorseProfile` for timing thresholds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct MorsePatternEntry {
    pub pattern: u16,
    pub action: KeyAction,
}

/// Protocol-facing combo configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ComboConfig {
    /// The input trigger keys that activate this combo.
    pub triggers: Vec<KeyAction, MAX_COMBO_KEYS>,
    pub output: KeyAction,
    pub layer: Option<u8>,
}

/// Summary information about macro capabilities.
///
/// These fields are also available via [`DeviceCapabilities`]; this type
/// provides a lightweight query for tools that only need macro info.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct MacroInfo {
    pub max_macros: u8,
    pub macro_space_size: u16,
}

impl From<super::DeviceCapabilities> for MacroInfo {
    fn from(caps: super::DeviceCapabilities) -> Self {
        Self {
            max_macros: caps.max_macros,
            macro_space_size: caps.macro_space_size,
        }
    }
}

/// Raw macro data for a single macro.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, postcard_schema::Schema)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct MacroData {
    pub data: heapless::Vec<u8, { super::MAX_MACRO_DATA }>,
}
