//! Keyboard input events

use rmk_macro::controller_event;
use rmk_types::action::KeyAction;
use rmk_types::modifier::ModifierCombination;

use crate::event::KeyboardEvent;

/// Key press/release event
#[controller_event(channel_size = 8, subs = 4)]
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct KeyEvent {
    pub keyboard_event: KeyboardEvent,
    pub key_action: KeyAction,
}

/// Modifier keys combination changed event
#[controller_event(channel_size = 8, subs = 4)]
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ModifierEvent {
    pub modifier: ModifierCombination,
}
