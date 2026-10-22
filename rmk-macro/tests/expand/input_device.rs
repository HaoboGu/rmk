//! Expand tests for #[input_device] macro.
//!
//! Tests:
//! - Basic single-event device
//! - Multi-event device using #[derive(Event)] wrapper enum

use rmk_macro::{Event, input_device};

#[derive(Clone, Copy, Debug)]
pub struct PointingEvent {}

#[derive(Clone, Copy, Debug)]
pub struct BatteryEvent {
    pub level: u8,
}

#[derive(Event, Clone, Debug)]
pub enum NrfAdcEvent {
    Pointing(PointingEvent),
    Battery(BatteryEvent),
}

/// Basic single-event device
mod basic {
    use super::{BatteryEvent, input_device};

    #[input_device(publish = BatteryEvent)]
    pub struct BatteryReader {
        pub pin: u8,
    }
}

/// Multi-event device using wrapper enum
mod multi_event {
    use super::{NrfAdcEvent, input_device};

    #[input_device(publish = NrfAdcEvent)]
    pub struct NrfAdc<'a, const PIN_NUM: usize, const EVENT_NUM: usize> {
        samples: &'a [[i16; PIN_NUM]; EVENT_NUM],
    }
}
