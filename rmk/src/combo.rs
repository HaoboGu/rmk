use heapless::Vec;

use crate::{action::KeyAction, event::KeyEvent};

// Max number of macros
pub(crate) const COMBO_MAX_NUM: usize = 8;
// Max size of macros
pub(crate) const COMBO_MAX_LENGTH: usize = 4;

#[derive(Clone)]
pub struct Combo {
    pub(crate) actions: Vec<KeyAction, COMBO_MAX_LENGTH>,
    pub(crate) output: KeyAction,
    state: u8,
}

impl Default for Combo {
    fn default() -> Self {
        Self::empty()
    }
}

impl Combo {
    pub fn new<I: IntoIterator<Item = KeyAction>>(actions: I, output: KeyAction) -> Self {
        Self {
            actions: Vec::from_iter(actions),
            output,
            state: 0,
        }
    }

    pub fn empty() -> Self {
        Self::new(Vec::<KeyAction, COMBO_MAX_LENGTH>::new(), KeyAction::No)
    }

    pub(crate) fn update(&mut self, key_action: KeyAction, key_event: KeyEvent) -> bool {
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

    pub(crate) fn done(&self) -> bool {
        self.started() && self.keys_pressed() == self.actions.len() as u32
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
