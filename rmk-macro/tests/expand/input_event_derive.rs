use rmk_macro::InputEvent;

#[input_event]
#[derive(Clone, Copy, Debug)]
pub struct BatteryEvent {
    pub level: u8,
}

#[input_event]
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
