use heapless::Vec;

use crate::{action::KeyAction, keyboard::KeyEvent};

// Default number of macros
pub(crate) const COMBO_MAX_NUM: usize = 8;
// Default size of macros
pub(crate) const COMBO_MAX_LENGTH: usize = 4;

pub(crate) struct Combo {
    pub(crate) actions: Vec<KeyAction, COMBO_MAX_LENGTH>,
    pub(crate) output: KeyAction,
    state: u8,
}

impl Combo {
    pub fn new(actions: Vec<KeyAction, COMBO_MAX_LENGTH>, output: KeyAction) -> Self {
        Self {
            actions,
            output,
            state: 0,
        }
    }

    pub fn empty() -> Self {
        Self::new(Vec::new(), KeyAction::No)
    }

    pub fn update(&mut self, key_action: KeyAction, key_event: KeyEvent) -> bool {
        if !key_event.pressed || key_action == KeyAction::No {
            return false;
        }

        let action_idx = self.actions.iter().position(|&a| a == key_action);
        if let Some(i) = action_idx {
            self.state |= 1 << i;
        } else {
            self.reset();
        }
        action_idx.is_some()
    }

    pub fn done(&self) -> bool {
        self.started() && self.keys_pressed() == self.actions.len() as u32
    }

    pub fn started(&self) -> bool {
        self.state != 0
    }

    pub fn keys_pressed(&self) -> u32 {
        self.state.count_ones()
    }

    pub fn reset(&mut self) {
        self.state = 0;
    }
}
