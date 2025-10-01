use heapless::Vec;
use rmk_types::action::KeyAction;

use crate::COMBO_MAX_LENGTH;
use crate::event::KeyboardEvent;

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Combo {
    pub(crate) actions: [KeyAction; COMBO_MAX_LENGTH],
    pub(crate) output: KeyAction,
    pub(crate) layer: Option<u8>,
    /// The state records the pressed keys of the combo
    state: u8,
    /// The flag indicates whether the combo is triggered
    is_triggered: bool,
}

impl Default for Combo {
    fn default() -> Self {
        Self::empty()
    }
}

impl Combo {
    pub fn new<I: IntoIterator<Item = KeyAction>>(actions: I, output: KeyAction, layer: Option<u8>) -> Self {
        let mut combo_actions = [KeyAction::No; COMBO_MAX_LENGTH];
        for (id, action) in actions.into_iter().enumerate() {
            if id < COMBO_MAX_LENGTH {
                combo_actions[id] = action;
            }
        }
        Self {
            actions: combo_actions,
            output,
            layer,
            state: 0,
            is_triggered: false,
        }
    }

    pub fn empty() -> Self {
        Self::new(Vec::<KeyAction, COMBO_MAX_LENGTH>::new(), KeyAction::No, None)
    }

    /// Update the combo's state when a key is pressed.
    /// Returns true if the combo is updated.
    pub(crate) fn update(&mut self, key_action: &KeyAction, key_event: KeyboardEvent, active_layer: u8) -> bool {
        if !key_event.pressed || self.actions.is_empty() || self.is_triggered {
            // Ignore combo that without actions
            return false;
        }

        if let Some(layer) = self.layer
            && layer != active_layer {
                return false;
            }

        let action_idx = self.actions.iter().position(|&a| a == *key_action);
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
        if let Some(i) = self.actions.iter().position(|&a| a == *key_action) {
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
            return self.output;
        }

        if self.output.is_empty() {
            return self.output;
        }

        if self.is_all_pressed() {
            self.is_triggered = true;
        }
        self.output
    }

    // Check if the combo is dispatched into key event
    pub(crate) fn is_triggered(&self) -> bool {
        self.is_triggered
    }

    // Check if all keys of this combo are pressed, but it does not mean the combo key event is sent
    pub(crate) fn is_all_pressed(&self) -> bool {
        let cnt = self.actions.iter().filter(|&&a| a != KeyAction::No).count();
        cnt > 0 && self.keys_pressed() == cnt as u32
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
