//! Keyboard state events

use core::cell::Cell;

use embassy_sync::blocking_mutex::Mutex;
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

/// Causal order for layer transitions produced outside the keyboard task.
///
/// Timer ticks cannot order two transitions that happen within the same tick,
/// so Sticky Key lifecycles compare this generation instead of timestamps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LayerTransitionGeneration(u64);

static LAYER_TRANSITION_GENERATION: Mutex<crate::RawMutex, Cell<u64>> = Mutex::new(Cell::new(0));

impl LayerTransitionGeneration {
    pub(crate) fn current() -> Self {
        LAYER_TRANSITION_GENERATION.lock(|generation| Self(generation.get()))
    }

    fn next() -> Self {
        LAYER_TRANSITION_GENERATION.lock(|generation| {
            let next = generation.get().wrapping_add(1);
            generation.set(next);
            Self(next)
        })
    }

    /// Return whether `self` is causally newer than `baseline`.
    ///
    /// Half-range wrapping order is unambiguous as long as fewer than 2^63
    /// external transitions can remain queued, which is guaranteed by the
    /// bounded event channel.
    pub(crate) const fn is_after(self, baseline: Self) -> bool {
        let distance = self.0.wrapping_sub(baseline.0);
        distance != 0 && distance <= (u64::MAX / 2)
    }

    #[cfg(test)]
    pub(crate) const fn from_raw(value: u64) -> Self {
        Self(value)
    }
}

/// A layer transition produced outside the main keyboard action loop.
#[event(channel_size = crate::LAYER_TRANSITION_EVENT_CHANNEL_SIZE, pubs = crate::LAYER_TRANSITION_EVENT_PUB_SIZE, subs = crate::LAYER_TRANSITION_EVENT_SUB_SIZE)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LayerTransitionEvent {
    /// Layer whose boolean state changed.
    pub(crate) layer: u8,
    pub(crate) transition: LayerTransition,
    /// Causal generation assigned when the layer state was changed.
    pub(crate) generation: LayerTransitionGeneration,
}

impl LayerTransitionEvent {
    pub(crate) fn new(layer: u8, transition: LayerTransition) -> Self {
        Self {
            layer,
            transition,
            generation: LayerTransitionGeneration::next(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::LayerTransitionGeneration;

    #[test]
    fn layer_transition_generation_orders_equal_tick_events() {
        let before = LayerTransitionGeneration::from_raw(10);
        let after = LayerTransitionGeneration::from_raw(11);

        assert!(after.is_after(before));
        assert!(!before.is_after(before));
        assert!(!before.is_after(after));
    }

    #[test]
    fn layer_transition_generation_has_defined_wrapping_order() {
        let before_wrap = LayerTransitionGeneration::from_raw(u64::MAX);
        let after_wrap = LayerTransitionGeneration::from_raw(0);

        assert!(after_wrap.is_after(before_wrap));
        assert!(!before_wrap.is_after(after_wrap));
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
