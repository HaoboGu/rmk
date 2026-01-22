//! Input device events
//!
//! This module contains event types for various input devices like keyboards,
//! touchpads, joysticks, and rotary encoders.

mod battery;
mod keyboard;
mod pointing;
mod touchpad;

pub use battery::{BatteryEvent, ChargingStateEvent as InputChargingStateEvent};
pub use keyboard::{KeyPos, KeyboardEvent, KeyboardEventPos, RotaryEncoderPos};
pub use pointing::{Axis, AxisEvent, AxisValType};
pub use touchpad::TouchpadEvent;

use crate::event::{AsyncEvent, AsyncEventPublisher as _, Event, EventPublisher as _};

/// Publish an input event (non-blocking, may fail if buffer full)
///
/// Example: `publish_input_event(BatteryEvent(80))`
pub fn publish_input_event<E: Event>(e: E) {
    E::publisher().publish(e);
}

/// Publish an input event with backpressure (waits if buffer full)
///
/// Example: `publish_input_event_async(KeyboardEvent::key(0, 0, true)).await`
pub async fn publish_input_event_async<E: AsyncEvent>(e: E) {
    E::publisher_async().publish_async(e).await;
}
