//! Controller event system
//!
//! This module provides the infrastructure for type-safe controller events and built-in contrller events.
//! Each event type has its own dedicated channel and can be subscribed to independently.
#[cfg(feature = "_ble")]
mod ble;
mod connection;
mod input;
mod keyboard_state;
mod power;
#[cfg(feature = "split")]
mod split;

#[cfg(feature = "_ble")]
pub use ble::{BleProfileChangeEvent, BleStateChangeEvent};
pub use connection::{ConnectionChangeEvent, ConnectionType};
pub use input::{KeyEvent, ModifierEvent};
pub use keyboard_state::{LayerChangeEvent, LedIndicatorEvent, SleepStateEvent, WpmUpdateEvent};
#[cfg(feature = "_ble")]
pub use power::{BatteryLevelEvent, ChargingStateEvent};
#[cfg(all(feature = "split", feature = "_ble"))]
pub use split::ClearPeerEvent;
#[cfg(feature = "split")]
pub use split::{CentralConnectedEvent, PeripheralBatteryEvent, PeripheralConnectedEvent};

use crate::event::{AsyncEvent, AsyncEventPublisher as _, Event, EventPublisher as _};

/// Trait for controller event
pub trait ControllerEvent: Event {}
impl<T: Event> ControllerEvent for T {}

/// Trait for async controller event
pub trait AsyncControllerEvent: AsyncEvent {}
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
