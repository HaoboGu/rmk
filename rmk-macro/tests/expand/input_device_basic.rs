use rmk_macro::input_device;

#[derive(Clone, Copy, Debug)]
pub struct BatteryEvent {
    pub level: u8,
}

#[input_device(publish = BatteryEvent)]
pub struct BatteryReader {
    pub pin: u8,
}
