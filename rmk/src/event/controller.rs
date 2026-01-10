//! Controller event system
//!
//! This module provides the infrastructure for type-safe controller events.
//! Each event type has its own dedicated channel and can be subscribed to independently.

use super::{EventPublisher, EventSubscriber};

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

/// Publish a controller event
///
/// This is a convenience function that publishes an event using its associated publisher.
///
/// # Example
///
/// ```ignore
/// use rmk::event::publish_controller_event;
/// use rmk::builtin_events::BatteryEvent;
///
/// publish_controller_event(BatteryEvent(80)).await;
/// ```
pub fn publish_controller_event<E: ControllerEventTrait>(e: E) {
    E::publisher().publish(e);
}
