use defmt::info;

use crate::impl_input_device;

use super::InputDevice;

pub struct RotaryEncoder {}

impl InputDevice for RotaryEncoder{
    async fn run(&mut self) {
        loop {
            embassy_time::Timer::after_secs(1).await;
            info!("hello device")
        }
    }
}

impl_input_device!(RotaryEncoder, rotary_encoder_task);