#[cfg(feature = "_nrf_ble")]
pub mod nrf;

#[cfg(feature = "_nrf_ble")]
pub use nrf::*;

pub enum AnalogEventType {
    Joystick(u8),
    Battery,
}

#[derive(PartialEq)]
pub enum AdcState {
    Active,
    LightSleep,
    // DeepSleep,
}
