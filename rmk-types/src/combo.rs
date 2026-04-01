//! Combo configuration types shared between firmware and protocol layers.

use heapless::Vec;
use postcard::experimental::max_size::MaxSize;
#[cfg(feature = "rmk_protocol")]
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use crate::action::KeyAction;
use crate::constants::PROTOCOL_COMBO_VEC_SIZE;

/// Configuration data for a combo.
///
/// A combo triggers an output action when a set of keys are pressed simultaneously.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "rmk_protocol", derive(Schema))]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ComboConfig {
    pub actions: Vec<KeyAction, PROTOCOL_COMBO_VEC_SIZE>,
    pub output: KeyAction,
    pub layer: Option<u8>,
}

impl MaxSize for ComboConfig {
    const POSTCARD_MAX_SIZE: usize = KeyAction::POSTCARD_MAX_SIZE * PROTOCOL_COMBO_VEC_SIZE
        + crate::varint_max_size(PROTOCOL_COMBO_VEC_SIZE)
        + KeyAction::POSTCARD_MAX_SIZE
        + <Option<u8>>::POSTCARD_MAX_SIZE;
}

impl ComboConfig {
    pub fn new<I: IntoIterator<Item = KeyAction>>(actions: I, output: KeyAction, layer: Option<u8>) -> Self {
        let mut combo_actions = Vec::new();
        for action in actions {
            if combo_actions.push(action).is_err() {
                break;
            }
        }
        Self {
            actions: combo_actions,
            output,
            layer,
        }
    }

    /// Get an empty combo.
    pub fn empty() -> Self {
        Self {
            actions: Vec::new(),
            output: KeyAction::No,
            layer: None,
        }
    }

    /// Returns the number of key actions in the combo.
    pub fn size(&self) -> usize {
        self.actions.iter().filter(|&&a| a != KeyAction::No).count()
    }

    /// Find the index of a key action in the combo.
    pub fn find_key_action_index(&self, key_action: &KeyAction) -> Option<usize> {
        self.actions.iter().position(|a| a == key_action)
    }
}
