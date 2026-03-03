//! Keyboard state events

use core::ops::Deref;

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

impl Deref for LayerChangeEvent {
    type Target = LayerChangePayload;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<LayerChangeEvent> for LayerChangePayload {
    fn from(event: LayerChangeEvent) -> Self {
        event.0
    }
}

impl From<LayerChangePayload> for LayerChangeEvent {
    fn from(payload: LayerChangePayload) -> Self {
        Self(payload)
    }
}

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

impl Deref for WpmUpdateEvent {
    type Target = WpmPayload;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<WpmUpdateEvent> for WpmPayload {
    fn from(event: WpmUpdateEvent) -> Self {
        event.0
    }
}

impl From<WpmPayload> for WpmUpdateEvent {
    fn from(payload: WpmPayload) -> Self {
        Self(payload)
    }
}

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

impl Deref for LedIndicatorEvent {
    type Target = LedPayload;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<LedIndicatorEvent> for LedPayload {
    fn from(event: LedIndicatorEvent) -> Self {
        event.0
    }
}

impl From<LedPayload> for LedIndicatorEvent {
    fn from(payload: LedPayload) -> Self {
        Self(payload)
    }
}

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

impl Deref for SleepStateEvent {
    type Target = SleepPayload;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<SleepStateEvent> for SleepPayload {
    fn from(event: SleepStateEvent) -> Self {
        event.0
    }
}

impl From<SleepPayload> for SleepStateEvent {
    fn from(payload: SleepPayload) -> Self {
        Self(payload)
    }
}
