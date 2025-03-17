use super::EventType;
use crate::{
    event::{Axis, AxisEvent, AxisValType, Event},
    input_device::InputDevice,
};
use embassy_nrf::saadc::Saadc;

pub struct NrfAdc<'a, const PIN_NUM: usize, const EVENT_NUM: usize> {
    saadc: Saadc<'a, PIN_NUM>,
    polling_interval: u16,
    buf: [i16; PIN_NUM],
    event_type: [EventType; EVENT_NUM],
    event_state: u8,
    buf_state: u8,
}

/// SCALE = (GAIN/REFERENCE) * 2(RESOLUTION)
/// Single-ended or positive differential support
impl<'a, const PIN_NUM: usize, const EVENT_NUM: usize> NrfAdc<'a, PIN_NUM, EVENT_NUM> {
    pub fn new(
        saadc: Saadc<'a, PIN_NUM>,
        event_type: [EventType; EVENT_NUM],
        polling_interval: u16,
    ) -> Self {
        Self {
            saadc,
            polling_interval,
            event_type,
            buf: [0; PIN_NUM],
            event_state: 0,
            buf_state: 0,
        }
    }
}

impl<'a, const PIN_NUM: usize, const EVENT_NUM: usize> InputDevice
    for NrfAdc<'a, PIN_NUM, EVENT_NUM>
{
    async fn read_event(&mut self) -> Event {
        embassy_time::Timer::after_millis(self.polling_interval as u64).await;

        if self.event_state == EVENT_NUM as u8 {
            if self.buf_state != PIN_NUM as u8 {
                error!("NrfAdc's pin size and event's required is mismatch");
            }
            self.saadc.sample(&mut self.buf).await;
            self.buf_state = 0;
            self.event_state = 0;
        }
        let ret_e = match self.event_type[self.event_state as usize] {
            EventType::Joystick(sz) => {
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
                        e[i as usize].value =
                            (self.buf[self.buf_state as usize] + i16::MIN / 2).saturating_mul(2);
                        self.buf_state += 1;
                    }
                }
                Event::Joystick(e)
            }
            EventType::Battery => {
                let value = self.buf[self.buf_state as usize];
                let e = Event::Battery(self.buf[self.buf_state as usize] as u16);
                self.buf_state += 1;
                e
            }
        };
        self.event_state += 1;
        ret_e
    }
}
