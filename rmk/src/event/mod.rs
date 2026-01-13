//! Event system for RMK
//!
//! This module provides:
//! - Built-in controller events (battery, connection, input, etc.)
//! - Input device events (keyboard, touchpad, joystick, etc.)
//! - Controller event infrastructure (publish/subscribe patterns)

use embassy_sync::blocking_mutex::raw::RawMutex;
use embassy_sync::pubsub::{ImmediatePublisher, Publisher, Subscriber};
use embassy_sync::watch;

// Sub-modules
mod builtin;
mod controller;
mod input_device;

// Re-export all built-in events at top level
pub use builtin::*;
// Re-export controller event system
pub use controller::{
    AwaitableControllerEventTrait, ControllerEventTrait, publish_controller_event, publish_controller_event_async,
};
// Re-export input device events
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
    async fn async_publish(&self, message: T);
}

/// Trait for event subscribers
///
/// This is a generic trait that can be implemented by any type that subscribes to events.
/// It's used by both controller events and potentially other event systems.
pub trait EventSubscriber<T> {
    async fn next_event(&mut self) -> T;
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
    async fn async_publish(&self, message: T) {
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
    async fn next_event(&mut self) -> T {
        self.changed().await
    }
}
