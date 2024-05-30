use embassy_nrf::{
    gpio::{AnyPin, Input, Output},
    saadc::Saadc,
};

#[derive(Default)]
pub struct BleBatteryConfig<'a> {
    pub charge_state_pin: Option<Input<'a, AnyPin>>,
    pub charge_led_pin: Option<Output<'a, AnyPin>>,
    pub charge_state_low_active: bool,
    pub charge_led_low_active: bool,
    pub saadc: Option<Saadc<'a, 1>>,
}

impl<'a> BleBatteryConfig<'a> {
    pub fn new(
        charge_state_pin: Option<Input<'a, AnyPin>>,
        charge_state_low_active: bool,
        charge_led_pin: Option<Output<'a, AnyPin>>,
        charge_led_low_active: bool,
        saadc: Option<Saadc<'a, 1>>,
    ) -> Self {
        Self {
            charge_state_pin,
            charge_state_low_active,
            charge_led_pin,
            charge_led_low_active,
            saadc,
        }
    }
}
