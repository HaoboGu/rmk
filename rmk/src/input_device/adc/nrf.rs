use embassy_nrf::saadc::Saadc;
use embassy_time::{Duration, Instant};

use super::{AdcState, AnalogEventType};
use crate::event::{Axis, AxisEvent, AxisValType, Event};
use crate::input_device::InputDevice;

pub struct NrfAdc<'a, const PIN_NUM: usize, const EVENT_NUM: usize> {
    saadc: Saadc<'a, PIN_NUM>,
    polling_interval: Duration,
    light_sleep: Option<Duration>,
    buf: [[i16; PIN_NUM]; 2], // double buffer for waking up
    event_type: [AnalogEventType; EVENT_NUM],
    event_state: u8,
    channel_state: u8,
    buf_state: bool,
    adc_state: AdcState,
    active_instant: Instant,
}

/// SCALE = (GAIN/REFERENCE) * 2(RESOLUTION)
/// Single-ended or positive differential support
impl<'a, const PIN_NUM: usize, const EVENT_NUM: usize> NrfAdc<'a, PIN_NUM, EVENT_NUM> {
    pub fn new(
        saadc: Saadc<'a, PIN_NUM>,
        event_type: [AnalogEventType; EVENT_NUM],
        polling_interval: Duration,
        light_sleep: Option<Duration>,
    ) -> Self {
        Self {
            saadc,
            polling_interval,
            event_type,
            light_sleep,
            buf: [[0; PIN_NUM]; 2],
            event_state: 0,
            channel_state: 0,
            buf_state: false,
            adc_state: AdcState::LightSleep,
            active_instant: Instant::MIN,
        }
    }
}

impl<'a, const PIN_NUM: usize, const EVENT_NUM: usize> InputDevice for NrfAdc<'a, PIN_NUM, EVENT_NUM> {
    async fn read_event(&mut self) -> Event {
        if self.active_instant == Instant::MIN {
            // filling for the first polling
            self.saadc.sample(&mut self.buf[1]).await;
            self.active_instant = Instant::now();
        } else {
            if let Some(light_sleep) = self.light_sleep {
                if self.adc_state == AdcState::LightSleep {
                    embassy_time::Timer::after(light_sleep).await;
                } else {
                    embassy_time::Timer::after(self.polling_interval).await;
                }
            } else {
                embassy_time::Timer::after(self.polling_interval).await;
            }
        }

        if self.active_instant.elapsed().as_millis() > 1200 {
            self.adc_state = AdcState::LightSleep; // sleep :)
        }

        if self.event_state == EVENT_NUM as u8 {
            if self.channel_state != PIN_NUM as u8 {
                error!("NrfAdc's pin size and event's required is mismatch");
            }
            self.buf_state = !self.buf_state;
            let buf = if self.buf_state {
                &mut self.buf[0]
            } else {
                &mut self.buf[1]
            };
            self.saadc.sample(buf).await;
            for (a, b) in self.buf[0].iter().zip(self.buf[1].iter()) {
                if i16::abs(a - b) > 150 {
                    debug!("ADC Active");
                    self.adc_state = AdcState::Active;
                    self.active_instant = Instant::now();
                    break;
                }
            }
            self.channel_state = 0;
            self.event_state = 0;
        }

        let buf = if self.buf_state { &self.buf[0] } else { &self.buf[1] };

        let ret_e = match self.event_type[self.event_state as usize] {
            AnalogEventType::Joystick(sz) => {
                let mut e = [
                    AxisEvent {
                        typ: AxisValType::Rel,
                        axis: Axis::X,
                        value: 0,
                    },
                    AxisEvent {
                        typ: AxisValType::Rel,
                        axis: Axis::Y,
                        value: 0,
                    },
                    AxisEvent {
                        typ: AxisValType::Rel,
                        axis: Axis::Z,
                        value: 0,
                    },
                ];
                if sz > 3 || sz == 0 {
                    error!("Joystick with more than 3 dimensions or empty is not supported. Skip this event");
                } else {
                    for i in 0..core::cmp::min(sz, 2) {
                        e[i as usize].value = (buf[self.channel_state as usize] + i16::MIN / 2).saturating_mul(2);
                        self.channel_state += 1;
                    }
                }
                Event::Joystick(e)
            }
            AnalogEventType::Battery => {
                let e = Event::Battery(buf[self.channel_state as usize] as u16);
                self.channel_state += 1;
                e
            }
        };
        self.event_state += 1;
        ret_e
    }
}
