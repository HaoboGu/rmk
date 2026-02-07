use rmk_macro::{controller_event, input_event};

/// Test case: reversed order - #[controller_event] before #[input_event].
/// This should generate the same code as combined_events.rs.
#[controller_event(subs = 2)]
#[input_event(channel_size = 8)]
#[derive(Clone, Copy, Debug)]
pub struct DualChannelEventReversed {
    pub data: u16,
}
