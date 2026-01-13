//! Controller event system
//!
//! This module provides the infrastructure for type-safe controller events.
//! Each event type has its own dedicated channel and can be subscribed to independently.

use super::{EventPublisher, EventSubscriber};
use crate::event::AsyncEventPublisher;

/// Trait for controller event types
///
/// This trait is automatically implemented by the `#[controller_event]` macro.
/// It provides type-safe access to event publishers and subscribers.
///
/// # Example
///
/// ```ignore
/// use rmk_macro::controller_event;
///
/// #[controller_event(subs = 1)]
/// #[derive(Clone, Copy)]
/// pub struct BatteryEvent(pub u8);
///
/// // Publishing an event
/// rmk::event::publish_controller_event(BatteryEvent(80));
///
/// // Subscribing to an event
/// let mut subscriber = BatteryEvent::subscriber();
/// let event = subscriber.next_event().await;
/// ```
pub trait ControllerEventTrait: Copy + Clone + Send {
    type Publisher: EventPublisher<Self>;
    type Subscriber: EventSubscriber<Self>;

    fn publisher() -> Self::Publisher;
    fn subscriber() -> Self::Subscriber;
}

/// Trait for controller events that support awaitable publishing
///
/// This trait extends `ControllerEventTrait` for events that use `PubSubChannel` with buffering.
/// It provides an async publisher that will wait for space in the channel instead of dropping events.
///
/// This trait is automatically implemented by the `#[controller_event]` macro when `channel_size` is specified.
///
/// # Example
///
/// ```ignore
/// use rmk_macro::controller_event;
///
/// #[controller_event(channel_size = 8, subs = 1, pubs = 1)]
/// #[derive(Clone, Copy)]
/// pub struct ImportantEvent(pub u8);
///
/// // This will wait if the channel is full instead of dropping the event
/// rmk::event::publish_controller_event_async(ImportantEvent(42)).await;
/// ```
pub trait AwaitableControllerEventTrait: ControllerEventTrait {
    type AsyncPublisher: AsyncEventPublisher<Self>;

    fn async_publisher() -> Self::AsyncPublisher;
}

/// Publish a controller event
///
/// This is a convenience function that publishes an event using its associated publisher.
///
/// # Example
///
/// ```ignore
/// use rmk::event::{publish_controller_event, BatteryLevelEvent};
///
/// publish_controller_event(BatteryLevelEvent { level: 80 });
/// ```
pub fn publish_controller_event<E: ControllerEventTrait>(e: E) {
    E::publisher().publish(e);
}

/// Publish a controller event asynchronously with backpressure
///
/// This function waits for space in the channel if it's full, ensuring the event is not dropped.
/// Only available for events that implement `AwaitableControllerEventTrait` (events with `channel_size`).
///
/// # Example
///
/// ```ignore
/// use rmk::event::{publish_controller_event_async, KeyEvent};
///
/// // This will wait if the channel is full
/// publish_controller_event_async(KeyEvent { /* fields */ }).await;
/// ```
pub async fn publish_controller_event_async<E: AwaitableControllerEventTrait>(e: E) {
    E::async_publisher().async_publish(e).await;
}
