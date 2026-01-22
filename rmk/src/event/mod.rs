//! Event system for RMK
//!
//! This module provides:
//! - Built-in controller events (battery, connection, input, etc.)
//! - Input device events (keyboard, touchpad, joystick, etc.)
//! - Controller event infrastructure (publish/subscribe patterns)

use embassy_sync::blocking_mutex::raw::RawMutex;
use embassy_sync::pubsub::{ImmediatePublisher, Publisher, Subscriber};
use embassy_sync::{channel, watch};

mod builtin;
mod controller;
mod input_device;

pub use builtin::*;
pub use controller::*;
pub use input_device::*;

/// Trait for event publishers
///
/// This is a generic trait that can be implemented by any type that publishes events.
/// It's used by both controller events and potentially other event systems.
pub trait EventPublisher<T> {
    fn publish(&self, message: T);
}

/// Async version of event publisher trait
pub trait AsyncEventPublisher<T> {
    async fn publish_async(&self, message: T);
}

/// Trait for event subscribers
///
/// This is a generic trait that can be implemented by any type that subscribes to events.
/// It's used by both controller events and potentially other event systems.
pub trait EventSubscriber<T> {
    async fn next_event(&mut self) -> T;
}

pub trait Event: Clone + Send {
    type Publisher: EventPublisher<Self>;
    type Subscriber: EventSubscriber<Self>;

    fn publisher() -> Self::Publisher;
    fn subscriber() -> Self::Subscriber;
}

pub trait AsyncEvent: Event {
    type AsyncPublisher: AsyncEventPublisher<Self>;

    fn publisher_async() -> Self::AsyncPublisher;
}

// Implementations for embassy-sync PubSubChannel
impl<'a, M: RawMutex, T: Clone, const CAP: usize, const SUBS: usize, const PUBS: usize> EventPublisher<T>
    for ImmediatePublisher<'a, M, T, CAP, SUBS, PUBS>
{
    fn publish(&self, message: T) {
        self.publish_immediate(message);
    }
}

impl<'a, M: RawMutex, T: Clone, const CAP: usize, const SUBS: usize, const PUBS: usize> AsyncEventPublisher<T>
    for Publisher<'a, M, T, CAP, SUBS, PUBS>
{
    async fn publish_async(&self, message: T) {
        self.publish(message).await
    }
}

impl<'a, M: RawMutex, T: Clone, const CAP: usize, const SUBS: usize, const PUBS: usize> EventSubscriber<T>
    for Subscriber<'a, M, T, CAP, SUBS, PUBS>
{
    async fn next_event(&mut self) -> T {
        self.next_message_pure().await
    }
}

// Implementations for embassy-sync Watch
impl<'a, M: RawMutex, T: Clone, const N: usize> EventPublisher<T> for watch::Sender<'a, M, T, N> {
    fn publish(&self, message: T) {
        self.send(message);
    }
}

impl<'a, M: RawMutex, T: Clone, const N: usize> EventSubscriber<T> for watch::Receiver<'a, M, T, N> {
    // WARNING: it won't work when using `XEvent::subscriber().next_event().await`,
    // because `subscriber()` creates a new subscriber, which will immediately return when `changed()` is called.
    // A possible solution is to call `changed()` twice in `next_event()`, but it looks ugly.
    async fn next_event(&mut self) -> T {
        self.changed().await
    }
}

// Implementation for embassy-sync Channel
impl<'a, M: RawMutex, T: Clone, const N: usize> EventPublisher<T> for channel::Sender<'a, M, T, N> {
    fn publish(&self, message: T) {
        if let Err(_) = self.try_send(message) {
            error!("Send event to Channel error, channel is full");
        }
    }
}

impl<'a, M: RawMutex, T: Clone, const N: usize> AsyncEventPublisher<T> for channel::Sender<'a, M, T, N> {
    async fn publish_async(&self, message: T) {
        self.send(message).await
    }
}

impl<'a, M: RawMutex, T: Clone, const N: usize> EventSubscriber<T> for channel::Receiver<'a, M, T, N> {
    async fn next_event(&mut self) -> T {
        self.receive().await
    }
}
