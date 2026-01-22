//! Controller event system
//!
//! This module provides the infrastructure for type-safe controller events.
//! Each event type has its own dedicated channel and can be subscribed to independently.

use crate::event::{AsyncEvent, AsyncEventPublisher as _, Event, EventPublisher as _};

pub trait ControllerEvent: Event {}
pub trait AsyncControllerEvent: AsyncEvent {}

impl<T: Event> ControllerEvent for T {}
impl<T: AsyncEvent> AsyncControllerEvent for T {}

/// Publish a controller event (non-blocking, may drop if buffer full)
///
/// Example: `publish_controller_event(BatteryLevelEvent { level: 80 })`
pub fn publish_controller_event<E: ControllerEvent>(e: E) {
    E::publisher().publish(e);
}

/// Publish event with backpressure (waits if buffer full, requires `channel_size`)
///
/// Example: `publish_controller_event_async(KeyEvent { pressed: true }).await`
pub async fn publish_controller_event_async<E: AsyncControllerEvent>(e: E) {
    E::publisher_async().publish_async(e).await;
}
