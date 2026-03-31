//! Behavior configuration protocol types.

use heapless::Vec;
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use super::{MAX_COMBO_KEYS, MAX_MORSE_PATTERNS};
use crate::action::{KeyAction, MorseProfile};
use crate::fork::StateBits;
use crate::modifier::ModifierCombination;

/// Protocol-facing morse/tap-dance configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct MorseConfig {
    pub profile: MorseProfile,
    pub patterns: Vec<MorsePatternEntry, MAX_MORSE_PATTERNS>,
}

/// A single morse pattern/action pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct MorsePatternEntry {
    pub pattern: u16,
    pub action: KeyAction,
}

/// Protocol-facing fork (key override) configuration.
///
/// This mirrors firmware `Fork` fields without reducing match-state dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct ForkConfig {
    pub trigger: KeyAction,
    pub negative_output: KeyAction,
    pub positive_output: KeyAction,
    pub match_any: StateBits,
    pub match_none: StateBits,
    pub kept_modifiers: ModifierCombination,
    pub bindable: bool,
}

/// Protocol-facing combo configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct ComboConfig {
    pub actions: Vec<KeyAction, MAX_COMBO_KEYS>,
    pub output: KeyAction,
    pub layer: Option<u8>,
}

/// Protocol-facing behavior configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct BehaviorConfig {
    pub combo_timeout_ms: u16,
    pub oneshot_timeout_ms: u16,
    pub tap_interval_ms: u16,
    pub tap_tolerance: u8,
}

/// Raw macro data for a single macro.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct MacroData {
    pub data: Vec<u8, { super::MAX_MACRO_DATA }>,
}
