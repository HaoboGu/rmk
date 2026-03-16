use postcard::experimental::max_size::MaxSize;
use rmk_macro::event;
use serde::{Deserialize, Serialize};

use crate::event::KeyboardEvent;

#[event(
    channel_size = crate::USER_ACTION_EVENT_CHANNEL_SIZE,
    pubs = crate::USER_ACTION_EVENT_PUB_SIZE,
    subs = crate::USER_ACTION_EVENT_SUB_SIZE
)]
#[derive(Serialize, Deserialize, Clone, Copy, Debug, MaxSize, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct UserAction {
    pub id: u8,
    pub keyboard_event: KeyboardEvent,
}
