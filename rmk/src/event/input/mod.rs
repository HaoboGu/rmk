//! Input device events
//!
//! This module contains event types for various input devices like keyboards,
//! touchpads, joysticks, and rotary encoders.

mod battery;
mod keyboard;
mod pointing;
mod touchpad;

pub use battery::{BatteryAdcEvent, ChargingStateEvent};
pub use keyboard::{KeyPos, KeyboardEvent, KeyboardEventPos, RotaryEncoderPos};
pub use pointing::{Axis, AxisEvent, AxisValType, PointingEvent};
pub use touchpad::TouchpadEvent;

use crate::event::{AsyncEventPublisher as _, AsyncPublishableInputEvent, EventPublisher as _, PublishableInputEvent};

/// Publish an input event (non-blocking, may fail if buffer full)
///
/// Example: `publish_input_event(BatteryEvent(80))`
pub fn publish_input_event<E: PublishableInputEvent>(e: E) {
    E::input_publisher().publish(e);
}

/// Publish an input event with backpressure (waits if buffer full)
///
/// Example: `publish_input_event_async(KeyboardEvent::key(0, 0, true)).await`
pub async fn publish_input_event_async<E: AsyncPublishableInputEvent>(e: E) {
    E::input_publisher_async().publish_async(e).await;
}
