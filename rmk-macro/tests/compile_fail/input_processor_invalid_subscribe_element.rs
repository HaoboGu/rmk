use rmk_macro::input_processor;

pub struct InputEvent;

#[input_processor(subscribe = [InputEvent, 123])]
pub struct BadInputProcessor;

fn main() {}
