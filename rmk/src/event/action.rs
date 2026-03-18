use postcard::experimental::max_size::MaxSize;
use rmk_macro::event;
use rmk_types::action::Action;
use serde::{Deserialize, Serialize};

use crate::event::KeyboardEvent;

#[event(
    channel_size = crate::ACTION_EVENT_CHANNEL_SIZE,
    pubs = crate::ACTION_EVENT_PUB_SIZE,
    subs = crate::ACTION_EVENT_SUB_SIZE
)]
#[derive(Serialize, Deserialize, Clone, Copy, Debug, MaxSize, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ActionEvent {
    pub action: Action,
    pub keyboard_event: KeyboardEvent,
}
