use rmk_macro::controller_event;

#[controller_event(channel_size = 4, subs = 2, pubs = 1)]
#[derive(Clone, Copy, Debug)]
pub struct BatteryEvent {
    pub level: u8,
}

/// Battery state changed event
#[controller_event(channel_size = 8, pubs = 2, subs = 3)]
#[derive(Clone, Copy, Debug)]
pub enum BatteryState {
    /// The battery state is not available
    NotAvailable,
    /// The value range is 0~100
    Normal(u8),
    /// Battery is currently charging
    Charging,
    /// Charging completed, ideally the battery level after charging completed is 100
    Charged,
}
