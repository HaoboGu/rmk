use embassy_nrf::gpio::{Input, Output};

pub struct BleBatteryConfig<'a> {
    pub charge_state_pin: Option<Input<'a>>,
    pub charge_led_pin: Option<Output<'a>>,
    pub charge_state_low_active: bool,
    pub charge_led_low_active: bool,
    pub adc_divider_measured: u32,
    pub adc_divider_total: u32,
}

impl<'a> Default for BleBatteryConfig<'a> {
    fn default() -> Self {
        Self {
            charge_state_pin: None,
            charge_led_pin: None,
            charge_state_low_active: false,
            charge_led_low_active: false,
            adc_divider_measured: 1,
            adc_divider_total: 1,
        }
    }
}

impl<'a> BleBatteryConfig<'a> {
    pub fn new(
        charge_state_pin: Option<Input<'a>>,
        charge_state_low_active: bool,
        charge_led_pin: Option<Output<'a>>,
        charge_led_low_active: bool,
        adc_divider_measured: u32,
        adc_divider_total: u32,
    ) -> Self {
        Self {
            charge_state_pin,
            charge_state_low_active,
            charge_led_pin,
            charge_led_low_active,
            adc_divider_measured,
            adc_divider_total,
        }
    }
}
