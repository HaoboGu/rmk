//! Expand tests for #[processor] macro.
//!
//! Tests:
//! - Single event subscription
//! - Multiple event subscription
//! - Polling processor with poll_interval

use rmk_macro::processor;

#[derive(Clone, Copy, Debug)]
pub struct KeyEvent {
    pub row: u8,
    pub col: u8,
    pub pressed: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct EncoderEvent {
    pub index: u8,
    pub direction: i8,
}

#[derive(Clone, Copy, Debug)]
pub struct ConfigEvent {
    pub threshold: u16,
}

/// Single event subscription
mod basic {
    use super::{KeyEvent, processor};

    #[processor(subscribe = [KeyEvent])]
    pub struct SingleEventProcessor;
}

/// Multiple event subscription
mod multi_sub {
    use super::{EncoderEvent, KeyEvent, processor};

    #[processor(subscribe = [KeyEvent, EncoderEvent])]
    pub struct KeyProcessor;
}

/// Polling processor
mod polling {
    use super::{ConfigEvent, processor};

    #[processor(subscribe = [ConfigEvent], poll_interval = 100)]
    pub struct PollingProcessor {
        pub counter: u32,
    }
}

/// Polling processor with multiple events
mod polling_multi {
    use super::{ConfigEvent, EncoderEvent, KeyEvent, processor};

    #[processor(subscribe = [KeyEvent, EncoderEvent, ConfigEvent], poll_interval = 50)]
    pub struct MultiPollingProcessor {
        pub state: u8,
    }
}
