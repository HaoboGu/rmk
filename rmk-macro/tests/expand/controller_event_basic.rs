use rmk_macro::controller_event;

#[controller_event(channel_size = 4, subs = 2, pubs = 1)]
#[derive(Clone, Copy, Debug)]
pub struct BatteryEvent {
    pub level: u8,
}
