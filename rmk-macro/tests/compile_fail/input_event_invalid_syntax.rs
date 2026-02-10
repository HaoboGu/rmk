use rmk_macro::input_event;

#[input_event(subs)]
#[derive(Clone, Copy, Debug)]
pub struct BareSubs;

#[input_event(channel_size = 8 subs = 4)]
#[derive(Clone, Copy, Debug)]
pub struct MissingComma;

fn main() {}
