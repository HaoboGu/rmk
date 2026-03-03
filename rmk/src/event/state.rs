//! Keyboard state events

use rmk_macro::event;
use rmk_types::led_indicator::LedIndicator;
use rmk_types::event::{LayerChangePayload, LedPayload, SleepPayload, WpmPayload};

/// Active layer changed event
#[event(channel_size = crate::LAYER_CHANGE_EVENT_CHANNEL_SIZE, pubs = crate::LAYER_CHANGE_EVENT_PUB_SIZE, subs = crate::LAYER_CHANGE_EVENT_SUB_SIZE)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct LayerChangeEvent(pub LayerChangePayload);

impl LayerChangeEvent {
    pub fn new(layer: u8) -> Self {
        Self(LayerChangePayload { layer })
    }
}

impl_payload_wrapper!(LayerChangeEvent, LayerChangePayload);

/// WPM updated event
#[event(channel_size = crate::WPM_UPDATE_EVENT_CHANNEL_SIZE, pubs = crate::WPM_UPDATE_EVENT_PUB_SIZE, subs = crate::WPM_UPDATE_EVENT_SUB_SIZE)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct WpmUpdateEvent(pub WpmPayload);

impl WpmUpdateEvent {
    pub fn new(wpm: u16) -> Self {
        Self(WpmPayload { wpm })
    }
}

impl_payload_wrapper!(WpmUpdateEvent, WpmPayload);

/// LED indicator state changed event
#[event(channel_size = crate::LED_INDICATOR_EVENT_CHANNEL_SIZE, pubs = crate::LED_INDICATOR_EVENT_PUB_SIZE, subs = crate::LED_INDICATOR_EVENT_SUB_SIZE)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct LedIndicatorEvent(pub LedPayload);

impl LedIndicatorEvent {
    pub fn new(indicator: LedIndicator) -> Self {
        Self(LedPayload { indicator })
    }
}

impl_payload_wrapper!(LedIndicatorEvent, LedPayload);

/// Sleep state changed event
#[event(channel_size = crate::SLEEP_STATE_EVENT_CHANNEL_SIZE, pubs = crate::SLEEP_STATE_EVENT_PUB_SIZE, subs = crate::SLEEP_STATE_EVENT_SUB_SIZE)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SleepStateEvent(pub SleepPayload);

impl SleepStateEvent {
    pub fn new(sleeping: bool) -> Self {
        Self(SleepPayload { sleeping })
    }
}

impl_payload_wrapper!(SleepStateEvent, SleepPayload);
