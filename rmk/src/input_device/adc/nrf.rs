use crate::{
    event::{AnalogEvent, Event},
    input_device::InputDevice,
};
use embassy_nrf::saadc::Saadc;

pub struct NrfAdc<'a, const N: usize> {
    saadc: Saadc<'a, N>,
    polling_interval: u16,
    buf: [i16; N],
    state: u8,
}

/// SCALE = (GAIN/REFERENCE) * 2(RESOLUTION)
/// Single-ended or positive differential support
impl<'a, const N: usize> NrfAdc<'a, N> {
    pub fn new(saadc: Saadc<'a, N>, polling_interval: u16) -> Self {
        Self {
            saadc,
            polling_interval,
            buf: [0; N],
            state: 0,
        }
    }
}

impl<'a, const N: usize> InputDevice for NrfAdc<'a, N> {
    async fn read_event(&mut self) -> Event {
        embassy_time::Timer::after_millis(self.polling_interval as u64).await;

        self.state += 1;
        let value = if self.state as usize == N {
            self.saadc.sample(&mut self.buf).await;
            self.state = 0;
            self.buf[self.state as usize]
        } else {
            self.buf[self.state as usize]
        };
        debug!("ADC value: {}({})", value, self.state);
        Event::Analog(AnalogEvent {
            id: self.state,
            value: value
                .try_into()
                .expect("ADC only support single-ended or positive differential"),
        })
    }
}
