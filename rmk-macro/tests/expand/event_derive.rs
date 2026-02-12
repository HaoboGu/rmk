use rmk_macro::{Event, event};

#[event(channel_size = 8)]
#[derive(Clone, Copy, Debug)]
pub struct BatteryEvent {
    pub level: u8,
}

#[event(channel_size = 8)]
#[derive(Clone, Copy, Debug)]
pub struct PointingEvent {
    pub x: i16,
    pub y: i16,
}

#[derive(Event)]
pub enum MultiSensorEvent {
    Battery(BatteryEvent),
    Pointing(PointingEvent),
}
