use rmk_macro::{InputEvent, input_device};

#[derive(Clone, Copy, Debug)]
pub struct PointingEvent {}

#[derive(Clone, Copy, Debug)]
pub struct BatteryEvent {
    pub level: u8,
}

#[derive(InputEvent, Clone, Debug)]
pub enum NrfAdcEvent {
    Pointing(PointingEvent),
    Battery(BatteryEvent),
}

mod basic {
    use super::{BatteryEvent, input_device};

    #[input_device(publish = BatteryEvent)]
    pub struct BatteryReader {
        pub pin: u8,
    }
}

mod multi_event {
    use super::{NrfAdcEvent, input_device};

    #[input_device(publish = NrfAdcEvent)]
    pub struct NrfAdc<'a, const PIN_NUM: usize, const EVENT_NUM: usize> {
        saadc: Saadc<'a, PIN_NUM>,
        polling_interval: Duration,
        light_sleep: Option<Duration>,
        buf: [[i16; PIN_NUM]; 2],
        event_type: [AnalogEventType; EVENT_NUM],
        event_state: u8,
        channel_state: u8,
        buf_state: bool,
        adc_state: AdcState,
        active_instant: Instant,
    }
}
