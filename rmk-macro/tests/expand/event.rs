//! Expand tests for #[event] macro.
//!
//! Tests:
//! - MPSC channel (default, single consumer)
//! - PubSub channel (with subs/pubs parameters)

use rmk_macro::event;

/// MPSC channel event (single consumer)
mod mpsc {
    use super::event;

    #[event(channel_size = 16)]
    #[derive(Clone, Copy, Debug)]
    pub struct KeyboardEvent {
        pub row: u8,
        pub col: u8,
        pub pressed: bool,
    }
}

/// PubSub channel event (multiple subscribers)
mod pubsub {
    use super::event;

    #[event(channel_size = 4, subs = 8, pubs = 2)]
    #[derive(Clone, Copy, Debug)]
    pub struct LedIndicatorEvent {
        pub caps_lock: bool,
        pub num_lock: bool,
        pub scroll_lock: bool,
    }
}

/// Tuple struct event
mod tuple_struct {
    use super::event;

    #[event(channel_size = 8)]
    #[derive(Clone, Copy, Debug)]
    pub struct BatteryAdcEvent(pub u16);
}

/// Event with default channel size
mod default_size {
    use super::event;

    #[event]
    #[derive(Clone, Copy, Debug)]
    pub struct LayerChangeEvent {
        pub layer: u8,
    }
}
