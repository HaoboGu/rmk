//! Keyboard input events

use rmk_macro::controller_event;
use rmk_types::action::KeyAction;
use rmk_types::modifier::ModifierCombination;

use crate::event::KeyboardEvent;

/// TODO: Split the KeyEvent to KeyboardEvent and processed KeyAction, or maybe HidReportEvent?
/// Key press/release event
#[controller_event(channel_size = crate::KEY_EVENT_CHANNEL_SIZE, pubs = crate::KEY_EVENT_PUB_SIZE, subs = crate::KEY_EVENT_SUB_SIZE)]
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct KeyEvent {
    pub keyboard_event: KeyboardEvent,
    pub key_action: KeyAction,
}

/// Modifier keys combination changed event
#[controller_event(channel_size = crate::MODIFIER_EVENT_CHANNEL_SIZE, pubs = crate::MODIFIER_EVENT_PUB_SIZE, subs = crate::MODIFIER_EVENT_SUB_SIZE)]
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ModifierEvent {
    pub modifier: ModifierCombination,
}
