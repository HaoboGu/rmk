use rmk_macro::event;

#[event(subs)]
#[derive(Clone, Copy, Debug)]
pub struct BareSubs;

#[event(channel_size = 8 subs = 4)]
#[derive(Clone, Copy, Debug)]
pub struct MissingComma;

fn main() {}
