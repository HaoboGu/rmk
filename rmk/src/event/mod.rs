//! Event system for RMK
//!
//! This module provides:
//! - Event infrastructure (traits, publish/subscribe patterns, implementations)
//! - Built-in controller events (battery, connection, input, etc.)
//! - Built-in input device events (keyboard, touchpad, joystick, etc.)

use embassy_sync::blocking_mutex::raw::RawMutex;
use embassy_sync::pubsub::{ImmediatePublisher, Publisher, Subscriber};
use embassy_sync::{channel, watch};

mod controller;
mod input;

pub use controller::*;
pub use input::*;

/// Trait for event publishers
///
/// This is a trait that can be implemented by any type that publishes events.
/// It's used by both controller events and potentially other event systems.
pub trait EventPublisher {
    type Event;
    fn publish(&self, message: Self::Event);
}

/// Async version of event publisher trait
pub trait AsyncEventPublisher {
    type Event;
    async fn publish_async(&self, message: Self::Event);
}

/// Trait for event subscribers
///
/// This is a generic trait that can be implemented by any type that subscribes to events.
/// It's used by both controller events and potentially other event systems.
pub trait EventSubscriber {
    type Event;
    async fn next_event(&mut self) -> Self::Event;
}

/// Base trait for all events
pub trait Event: Clone + Send {}
impl<T: Clone + Send> Event for T {}

// ============================================================================
// Input Event Traits
// ============================================================================

/// Trait for input events that can be published.
///
/// This is the "publish" side of input events. Types implementing this trait
/// can send events to a channel.
pub trait PublishableInputEvent: Event {
    type Publisher: EventPublisher<Event = Self>;

    fn input_publisher() -> Self::Publisher;
}

/// Async version of input publish event trait.
pub trait AsyncPublishableInputEvent: PublishableInputEvent {
    type AsyncPublisher: AsyncEventPublisher<Event = Self>;

    fn input_publisher_async() -> Self::AsyncPublisher;
}

/// Trait for input events that can be subscribed to.
///
/// This is the "subscribe" side of input events. Types implementing this trait
/// can receive events from a channel.
pub trait SubscribableInputEvent: Event {
    type Subscriber: EventSubscriber<Event = Self>;

    fn input_subscriber() -> Self::Subscriber;
}

/// Combined trait for input events that support both publish and subscribe.
///
/// Most concrete input event types implement this trait.
/// Wrapper enums (for routing) only implement `PublishableInputEvent`.
pub trait InputEvent: PublishableInputEvent + SubscribableInputEvent {}

// Auto-implement InputEvent for types that implement both publish and subscribe
impl<T: PublishableInputEvent + SubscribableInputEvent> InputEvent for T {}

/// Async version of input event trait (for backward compatibility)
pub trait AsyncInputEvent: InputEvent + AsyncPublishableInputEvent {}

impl<T: InputEvent + AsyncPublishableInputEvent> AsyncInputEvent for T {}

// ============================================================================
// Controller Event Traits
// ============================================================================

/// Trait for controller events that can be published.
///
/// This is the "publish" side of controller events.
pub trait PublishableControllerEvent: Event {
    type Publisher: EventPublisher<Event = Self>;

    fn controller_publisher() -> Self::Publisher;
}

/// Async version of controller publish event trait.
pub trait AsyncPublishableControllerEvent: PublishableControllerEvent {
    type AsyncPublisher: AsyncEventPublisher<Event = Self>;

    fn controller_publisher_async() -> Self::AsyncPublisher;
}

/// Trait for controller events that can be subscribed to.
///
/// This is the "subscribe" side of controller events.
pub trait SubscribableControllerEvent: Event {
    type Subscriber: EventSubscriber<Event = Self>;

    fn controller_subscriber() -> Self::Subscriber;
}

/// Combined trait for controller events that support both publish and subscribe.
///
/// Most concrete controller event types implement this trait.
/// Aggregated event enums (for multi-event subscription) also implement this trait.
pub trait ControllerEvent: PublishableControllerEvent + SubscribableControllerEvent {}

// Auto-implement ControllerEvent for types that implement both publish and subscribe
impl<T: PublishableControllerEvent + SubscribableControllerEvent> ControllerEvent for T {}

/// Async version of controller event trait (for backward compatibility)
pub trait AsyncControllerEvent: ControllerEvent + AsyncPublishableControllerEvent {}

impl<T: ControllerEvent + AsyncPublishableControllerEvent> AsyncControllerEvent for T {}

// Implementations for embassy-sync PubSubChannel
impl<'a, M: RawMutex, T: Clone, const CAP: usize, const SUBS: usize, const PUBS: usize> EventPublisher
    for ImmediatePublisher<'a, M, T, CAP, SUBS, PUBS>
{
    type Event = T;
    fn publish(&self, message: T) {
        self.publish_immediate(message);
    }
}

impl<'a, M: RawMutex, T: Clone, const CAP: usize, const SUBS: usize, const PUBS: usize> AsyncEventPublisher
    for Publisher<'a, M, T, CAP, SUBS, PUBS>
{
    type Event = T;
    async fn publish_async(&self, message: T) {
        self.publish(message).await
    }
}

impl<'a, M: RawMutex, T: Clone, const CAP: usize, const SUBS: usize, const PUBS: usize> EventSubscriber
    for Subscriber<'a, M, T, CAP, SUBS, PUBS>
{
    type Event = T;
    async fn next_event(&mut self) -> Self::Event {
        self.next_message_pure().await
    }
}

// Implementations for embassy-sync Watch
impl<'a, M: RawMutex, T: Clone, const N: usize> EventPublisher for watch::Sender<'a, M, T, N> {
    type Event = T;
    fn publish(&self, message: T) {
        self.send(message);
    }
}

impl<'a, M: RawMutex, T: Clone, const N: usize> EventSubscriber for watch::Receiver<'a, M, T, N> {
    type Event = T;
    // WARNING: it won't work when using `XEvent::subscriber().next_event().await`,
    // because `subscriber()` creates a new subscriber, which will immediately return when `changed()` is called.
    // A possible solution is to call `changed()` twice in `next_event()`, but it looks ugly.
    async fn next_event(&mut self) -> Self::Event {
        self.changed().await
    }
}

// Implementation for embassy-sync Channel
impl<'a, M: RawMutex, T: Clone, const N: usize> EventPublisher for channel::Sender<'a, M, T, N> {
    type Event = T;
    fn publish(&self, message: T) {
        if self.try_send(message).is_err() {
            error!("Send event to Channel error, channel is full");
        }
    }
}

impl<'a, M: RawMutex, T: Clone, const N: usize> AsyncEventPublisher for channel::Sender<'a, M, T, N> {
    type Event = T;
    async fn publish_async(&self, message: T) {
        self.send(message).await
    }
}

impl<'a, M: RawMutex, T: Clone, const N: usize> EventSubscriber for channel::Receiver<'a, M, T, N> {
    type Event = T;
    async fn next_event(&mut self) -> Self::Event {
        self.receive().await
    }
}
