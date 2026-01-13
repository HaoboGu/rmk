//! Controller event system
//!
//! This module provides the infrastructure for type-safe controller events.
//! Each event type has its own dedicated channel and can be subscribed to independently.

use super::{EventPublisher, EventSubscriber};
use crate::event::AsyncEventPublisher;

/// Trait for controller event types
///
/// Automatically implemented by `#[controller_event]` macro.
///
/// # Usage
///
/// ```ignore
/// use rmk_macro::controller_event;
///
/// // Watch channel (default): latest value only
/// #[controller_event(subs = 1)]
/// #[derive(Clone, Copy)]
/// pub struct BatteryLevelEvent { pub level: u8 }
///
/// // PubSubChannel: buffered, awaitable publish
/// #[controller_event(channel_size = 8, subs = 2)]
/// #[derive(Clone, Copy)]
/// pub struct KeyEvent { pub pressed: bool }
/// ```
///
/// # Attributes
///
/// - `channel_size = N`: Use PubSubChannel with buffer size N
/// - `subs = N`: Subscriber count (default 4)
/// - `pubs = N`: Publisher count (default 1, only with channel_size)
pub trait ControllerEventTrait: Copy + Clone + Send {
    type Publisher: EventPublisher<Self>;
    type Subscriber: EventSubscriber<Self>;

    fn publisher() -> Self::Publisher;
    fn subscriber() -> Self::Subscriber;
}

/// Trait for events with awaitable publishing (PubSubChannel with backpressure)
///
/// Automatically implemented by `#[controller_event]` when `channel_size` is specified.
pub trait AwaitableControllerEventTrait: ControllerEventTrait {
    type AsyncPublisher: AsyncEventPublisher<Self>;

    fn async_publisher() -> Self::AsyncPublisher;
}

/// Publish a controller event (non-blocking, may drop if buffer full)
///
/// Example: `publish_controller_event(BatteryLevelEvent { level: 80 })`
pub fn publish_controller_event<E: ControllerEventTrait>(e: E) {
    E::publisher().publish(e);
}

/// Publish event with backpressure (waits if buffer full, requires `channel_size`)
///
/// Example: `publish_controller_event_async(KeyEvent { pressed: true }).await`
pub async fn publish_controller_event_async<E: AwaitableControllerEventTrait>(e: E) {
    E::async_publisher().async_publish(e).await;
}
