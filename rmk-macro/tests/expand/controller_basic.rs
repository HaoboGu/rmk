use rmk_macro::controller;

#[derive(Clone, Copy, Debug)]
pub struct LedStateEvent {
    pub on: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct BrightnessEvent {
    pub level: u8,
}

#[controller(subscribe = [LedStateEvent, BrightnessEvent])]
pub struct LedController {
    pub pin: u8,
}
