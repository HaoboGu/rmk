use heapless::Vec;

use crate::action::KeyAction;
use crate::event::KeyEvent;

// Max number of combos
pub(crate) const COMBO_MAX_NUM: usize = 8;
// Max size of combos
pub(crate) const COMBO_MAX_LENGTH: usize = 4;

#[derive(Clone, Debug)]
pub struct Combo {
    pub(crate) actions: Vec<KeyAction, COMBO_MAX_LENGTH>,
    pub(crate) output: KeyAction,
    pub(crate) layer: Option<u8>,
    state: u8,
}

impl Default for Combo {
    fn default() -> Self {
        Self::empty()
    }
}

// magic code for marking the combo as triggered
const COMBO_TRIGGERED: u8 = u8::MAX;

impl Combo {
    pub fn new<I: IntoIterator<Item = KeyAction>>(actions: I, output: KeyAction, layer: Option<u8>) -> Self {
        Self {
            actions: Vec::from_iter(actions),
            output,
            layer,
            state: 0,
        }
    }

    pub fn empty() -> Self {
        Self::new(Vec::<KeyAction, COMBO_MAX_LENGTH>::new(), KeyAction::No, None)
    }

    pub(crate) fn update(&mut self, key_action: KeyAction, key_event: KeyEvent, active_layer: u8) -> bool {
        if !key_event.pressed || self.actions.is_empty() || self.state == COMBO_TRIGGERED {
            //ignore combo that without actions
            return false;
        }

        if let Some(layer) = self.layer {
            if layer != active_layer {
                return false;
            }
        }

        debug!("combo {:?} search key action {:?} ", self.output, key_action);
        let action_idx = self.actions.iter().position(|&a| a == key_action);
        if let Some(i) = action_idx {
            self.state |= 1 << i;
            debug!(
                "combo {:?} found index {} updated state: {}",
                self.output, i, self.state
            );
        } else if !self.is_all_pressed() {
            self.reset();
            debug!("combo {:?} reset state: {}", self.output, self.state);
        }
        action_idx.is_some()
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
            self.state = COMBO_TRIGGERED;
            debug!("combo {:?} mark triggered, updated state: {}", self.output, self.state);
        }
        self.output
    }

    // Check if the combo is dispatched into key event
    pub(crate) fn is_triggered(&self) -> bool {
        self.state == COMBO_TRIGGERED
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
    }
}
