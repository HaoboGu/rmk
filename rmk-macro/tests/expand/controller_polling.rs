use rmk_macro::controller;

#[derive(Clone, Copy, Debug)]
pub struct LedStateEvent {
    pub on: bool,
}

#[controller(subscribe = [LedStateEvent], poll_interval = 100)]
pub struct PollingLedController {
    pub pin: u8,
}
