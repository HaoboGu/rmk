//! Keyboard state events

use rmk_macro::controller_event;
use rmk_types::led_indicator::LedIndicator;

/// Active layer changed event
#[controller_event(subs = 2)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct LayerChangeEvent {
    pub layer: u8,
}

/// WPM updated event
#[controller_event(subs = 1)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct WpmUpdateEvent {
    pub wpm: u16,
}

/// LED indicator state changed event
#[controller_event(channel_size = 2, subs = 4)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct LedIndicatorEvent {
    pub indicator: LedIndicator,
}

/// Sleep state changed event
#[controller_event(subs = 2)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SleepStateEvent {
    pub sleeping: bool,
}
