use embassy_nrf::{
    gpio::{AnyPin, Input},
    saadc::Saadc,
};

#[derive(Default)]
pub struct BleBatteryConfig<'a> {
    pub charge_state_pin: Option<Input<'a, AnyPin>>,
    pub saadc: Option<Saadc<'a, 1>>,
}

impl<'a> BleBatteryConfig<'a> {
    pub fn new(charge_state_pin: Option<Input<'a, AnyPin>>, saadc: Option<Saadc<'a, 1>>) -> Self {
        Self {
            charge_state_pin,
            saadc,
        }
    }
}
