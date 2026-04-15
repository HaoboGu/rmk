use rmk_types::action::KeyAction;
use rmk_types::constants::COMBO_MAX_LENGTH;

/// Combo config instantiated with firmware's combo Vec capacity.
pub type ComboConfig = rmk_types::combo::Combo;

use crate::event::KeyboardEvent;

// Combo.state is a u16 bitmask, so combos are limited to 16 keys.
// Use core::assert! explicitly — the crate-level `assert!` macro dispatches to
// defmt::assert! which is not const-compatible.
const _: () = core::assert!(
    COMBO_MAX_LENGTH <= 16,
    "COMBO_MAX_LENGTH exceeds 16 — Combo.state is u16 and cannot track more than 16 keys"
);

/// Runtime combo instance (config + runtime state)
#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Combo {
    pub(crate) config: ComboConfig,
    /// The state records the pressed keys of the combo
    state: u16,
    /// The flag indicates whether the combo is triggered
    is_triggered: bool,
}

impl Default for Combo {
    fn default() -> Self {
        Self::empty()
    }
}

impl Combo {
    pub fn new(config: ComboConfig) -> Self {
        Self {
            config,
            state: 0,
            is_triggered: false,
        }
    }

    pub fn empty() -> Self {
        Self::new(ComboConfig::empty())
    }

    /// Update the combo's state when a key is pressed.
    /// Returns true if the combo is updated.
    pub(crate) fn update(&mut self, key_action: &KeyAction, key_event: KeyboardEvent, active_layer: u8) -> bool {
        if !key_event.pressed || self.config.size() == 0 || self.is_triggered {
            // Ignore combo that without actions
            return false;
        }

        if let Some(layer) = self.config.layer
            && layer != active_layer
        {
            return false;
        }

        let action_idx = self.config.find_key_action_index(key_action);
        if let Some(i) = action_idx {
            self.state |= 1 << i;
        } else if !self.is_all_pressed() {
            self.reset();
        }
        action_idx.is_some()
    }

    /// Update the combo's state when a key is released
    /// When the combo is fully released from triggered state, this function returns true
    pub(crate) fn update_released(&mut self, key_action: &KeyAction) -> bool {
        if let Some(i) = self.config.find_key_action_index(key_action) {
            self.state &= !(1 << i);
        }

        // Reset the combo if all keys are released
        if self.state == 0 {
            if self.is_triggered {
                self.reset();
                return true;
            }
            self.reset();
        }
        false
    }

    /// Mark the combo as done, if all actions are satisfied
    pub(crate) fn trigger(&mut self) -> KeyAction {
        if self.is_triggered() {
            return self.config.output;
        }

        if self.config.output.is_empty() {
            return self.config.output;
        }

        if self.is_all_pressed() {
            self.is_triggered = true;
        }
        self.config.output
    }

    // Check if the combo is dispatched into key event
    pub(crate) fn is_triggered(&self) -> bool {
        self.is_triggered
    }

    // Check if all keys of this combo are pressed, but it does not mean the combo key event is sent
    pub(crate) fn is_all_pressed(&self) -> bool {
        let cnt = self.config.size();
        cnt > 0 && self.keys_pressed() == cnt as u32
    }

    // The size of the current combo
    pub(crate) fn size(&self) -> usize {
        self.config.size()
    }

    pub(crate) fn started(&self) -> bool {
        self.state != 0
    }

    pub(crate) fn keys_pressed(&self) -> u32 {
        self.state.count_ones()
    }

    pub(crate) fn reset(&mut self) {
        self.state = 0;
        self.is_triggered = false;
    }
}
