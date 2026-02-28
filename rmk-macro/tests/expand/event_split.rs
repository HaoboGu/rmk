//! Expand tests for #[event(split)] macro.
//!
//! Tests:
//! - Split event with auto kind (split = 0) using PubSub channel
//! - Split event with explicit kind using MPSC channel

use rmk_macro::event;

/// Split event with auto kind and PubSub channel
mod split_pubsub {
    use super::event;

    #[event(split = 0, subs = 4, pubs = 2)]
    #[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize, postcard::experimental::max_size::MaxSize)]
    pub struct CustomSplitEvent {
        pub value: u16,
        pub flag: bool,
    }
}

/// Split event with explicit kind and MPSC channel
mod split_mpsc {
    use super::event;

    #[event(split = 42, channel_size = 8)]
    #[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize, postcard::experimental::max_size::MaxSize)]
    pub struct SensorEvent {
        pub reading: i16,
    }
}
