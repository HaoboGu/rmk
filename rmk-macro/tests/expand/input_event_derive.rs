use rmk_macro::InputEvent;

#[derive(Clone, Copy, Debug)]
pub struct BatteryEvent {
    pub level: u8,
}

#[derive(Clone, Copy, Debug)]
pub struct PointingEvent {
    pub x: i16,
    pub y: i16,
}

#[derive(InputEvent)]
pub enum MultiSensorEvent {
    Battery(BatteryEvent),
    Pointing(PointingEvent),
}
