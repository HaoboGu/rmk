use rmk_macro::controller;

pub struct InputEvent;

#[controller(subscribe = [InputEvent, 123])]
pub struct BadController;

#[controller(subscribe = [InputEvent], poll_interval = "fast")]
pub struct BadPollInterval;

fn main() {}
