//! Combo configuration types shared between firmware and protocol layers.

use postcard::experimental::max_size::MaxSize;
#[cfg(feature = "rmk_protocol")]
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use crate::action::KeyAction;
use crate::protocol_vec::ProtocolVec;

/// Configuration data for a combo.
///
/// A combo triggers an output action when a set of keys are pressed simultaneously.
/// `MAX_KEYS` controls the maximum number of trigger keys; on firmware this is
/// typically `COMBO_MAX_LENGTH` (from keyboard.toml), on host it uses the protocol
/// upper bound.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "rmk_protocol", derive(Schema))]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ComboConfig<const MAX_KEYS: usize> {
    pub actions: ProtocolVec<KeyAction, MAX_KEYS>,
    pub output: KeyAction,
    pub layer: Option<u8>,
}

impl<const MAX_KEYS: usize> MaxSize for ComboConfig<MAX_KEYS> {
    const POSTCARD_MAX_SIZE: usize = KeyAction::POSTCARD_MAX_SIZE * MAX_KEYS
        + crate::varint_max_size(MAX_KEYS)
        + KeyAction::POSTCARD_MAX_SIZE
        + <Option<u8>>::POSTCARD_MAX_SIZE;
}

impl<const MAX_KEYS: usize> ComboConfig<MAX_KEYS> {
    pub fn new<I: IntoIterator<Item = KeyAction>>(actions: I, output: KeyAction, layer: Option<u8>) -> Self {
        let mut combo_actions = ProtocolVec::new();
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
            actions: ProtocolVec::new(),
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
