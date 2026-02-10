use rmk_macro::processor;

pub struct KeyboardEvent;

#[processor(subscribe = [KeyboardEvent, 123])]
pub struct BadProcessor;

#[processor(subscribe = [KeyboardEvent], poll_interval = "fast")]
pub struct BadPollInterval;

fn main() {}
