use rmk_macro::{controller_event, input_event};

#[input_event(channel_size = 8)]
#[controller_event(subs = 2)]
#[derive(Clone, Copy, Debug)]
pub struct DualChannelEvent {
    pub data: u16,
}
