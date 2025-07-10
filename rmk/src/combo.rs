use heapless::Vec;

use crate::action::KeyAction;
use crate::event::KeyEvent;
use crate::COMBO_MAX_LENGTH;

#[derive(Clone, Debug)]
pub struct Combo {
    pub(crate) actions: Vec<KeyAction, COMBO_MAX_LENGTH>,
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
        Self {
            actions: Vec::from_iter(actions),
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
    pub(crate) fn update(&mut self, key_action: KeyAction, key_event: KeyEvent, active_layer: u8) -> bool {
        if !key_event.pressed || self.actions.is_empty() || self.is_triggered {
            // Ignore combo that without actions
            return false;
        }

        if let Some(layer) = self.layer {
            if layer != active_layer {
                return false;
            }
        }

        let action_idx = self.actions.iter().position(|&a| a == key_action);
        if let Some(i) = action_idx {
            debug!("[COMBO] {:?} registered {:?} ", self.output, key_action);
            self.state |= 1 << i;
        } else if !self.is_all_pressed() {
            self.reset();
        }
        action_idx.is_some()
    }

    /// Update the combo's state when a key is released
    /// When the combo is fully released from triggered state, this function returns true
    pub(crate) fn update_released(&mut self, key_action: KeyAction) -> bool {
        if let Some(i) = self.actions.iter().position(|&a| a == key_action) {
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

        if self.output == KeyAction::No {
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
        !self.actions.is_empty() && self.keys_pressed() == self.actions.len() as u32
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
