//! Keyboard state events

use rmk_macro::event;
use rmk_types::led_indicator::LedIndicator;

/// Active layer changed event
#[event(channel_size = crate::LAYER_CHANGE_EVENT_CHANNEL_SIZE, pubs = crate::LAYER_CHANGE_EVENT_PUB_SIZE, subs = crate::LAYER_CHANGE_EVENT_SUB_SIZE)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct LayerChangeEvent {
    pub layer: u8,
}

/// WPM updated event
#[event(channel_size = crate::WPM_UPDATE_EVENT_CHANNEL_SIZE, pubs = crate::WPM_UPDATE_EVENT_PUB_SIZE, subs = crate::WPM_UPDATE_EVENT_SUB_SIZE)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct WpmUpdateEvent {
    pub wpm: u16,
}

/// LED indicator state changed event
#[event(channel_size = crate::LED_INDICATOR_EVENT_CHANNEL_SIZE, pubs = crate::LED_INDICATOR_EVENT_PUB_SIZE, subs = crate::LED_INDICATOR_EVENT_SUB_SIZE)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct LedIndicatorEvent {
    pub indicator: LedIndicator,
}

/// Sleep state changed event
#[event(channel_size = crate::SLEEP_STATE_EVENT_CHANNEL_SIZE, pubs = crate::SLEEP_STATE_EVENT_PUB_SIZE, subs = crate::SLEEP_STATE_EVENT_SUB_SIZE)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SleepStateEvent {
    pub sleeping: bool,
}
