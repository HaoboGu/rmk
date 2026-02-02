use rmk_macro::input_event;

#[input_event(channel_size = 8)]
#[derive(Clone, Copy, Debug)]
pub struct TestEvent {
    pub value: u8,
}
