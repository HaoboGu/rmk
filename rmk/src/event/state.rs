//! Keyboard state events

use embassy_time::Instant;
use rmk_macro::event;
use rmk_types::led_indicator::LedIndicator;

/// Active layer changed event
#[event(channel_size = crate::LAYER_CHANGE_EVENT_CHANNEL_SIZE, pubs = crate::LAYER_CHANGE_EVENT_PUB_SIZE, subs = crate::LAYER_CHANGE_EVENT_SUB_SIZE)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct LayerChangeEvent(pub u8);

impl LayerChangeEvent {
    pub fn new(layer: u8) -> Self {
        Self(layer)
    }
}

impl_payload_wrapper!(LayerChangeEvent, u8);

/// The direction of a layer state transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LayerTransition {
    Enter,
    Exit,
}

/// A layer transition produced outside the main keyboard action loop.
#[event(channel_size = crate::LAYER_TRANSITION_EVENT_CHANNEL_SIZE, pubs = crate::LAYER_TRANSITION_EVENT_PUB_SIZE, subs = crate::LAYER_TRANSITION_EVENT_SUB_SIZE)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LayerTransitionEvent {
    pub(crate) transition: LayerTransition,
    /// Timestamp at which the layer state was actually changed.
    pub(crate) occurred_at: Instant,
}

impl LayerTransitionEvent {
    pub(crate) fn new(transition: LayerTransition) -> Self {
        Self {
            transition,
            occurred_at: Instant::now(),
        }
    }
}

/// WPM updated event
#[event(channel_size = crate::WPM_UPDATE_EVENT_CHANNEL_SIZE, pubs = crate::WPM_UPDATE_EVENT_PUB_SIZE, subs = crate::WPM_UPDATE_EVENT_SUB_SIZE)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct WpmUpdateEvent(pub u16);

impl WpmUpdateEvent {
    pub fn new(wpm: u16) -> Self {
        Self(wpm)
    }
}

impl_payload_wrapper!(WpmUpdateEvent, u16);

/// LED indicator state changed event
#[event(channel_size = crate::LED_INDICATOR_EVENT_CHANNEL_SIZE, pubs = crate::LED_INDICATOR_EVENT_PUB_SIZE, subs = crate::LED_INDICATOR_EVENT_SUB_SIZE)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct LedIndicatorEvent(pub LedIndicator);

impl LedIndicatorEvent {
    pub fn new(indicator: LedIndicator) -> Self {
        Self(indicator)
    }
}

impl_payload_wrapper!(LedIndicatorEvent, LedIndicator);

/// Sleep state changed event
#[event(channel_size = crate::SLEEP_STATE_EVENT_CHANNEL_SIZE, pubs = crate::SLEEP_STATE_EVENT_PUB_SIZE, subs = crate::SLEEP_STATE_EVENT_SUB_SIZE)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SleepStateEvent(pub bool);

impl SleepStateEvent {
    pub fn new(sleeping: bool) -> Self {
        Self(sleeping)
    }
}

impl_payload_wrapper!(SleepStateEvent, bool);
