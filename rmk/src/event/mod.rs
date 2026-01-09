//! Event system for RMK
//!
//! This module provides:
//! - Input device events (keyboard, touchpad, joystick, etc.)
//! - Controller event infrastructure (publish/subscribe patterns)

use embassy_sync::{
    blocking_mutex::raw::RawMutex,
    channel,
    pubsub::{ImmediatePublisher, Subscriber},
    watch,
};

// Sub-modules
mod controller;
mod input_device;

// Re-export controller event system
pub use controller::{ControllerEventTrait, publish_controller_event};

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
    fn publish(&self, message: T);
}

/// Trait for event subscribers
///
/// This is a generic trait that can be implemented by any type that subscribes to events.
/// It's used by both controller events and potentially other event systems.
pub trait EventSubscriber<T> {
    async fn next_event(&mut self) -> T;
}

// Implementation for embassy-sync Channel
impl<'a, M: RawMutex, T: Clone, const N: usize> EventPublisher<T> for channel::Sender<'a, M, T, N> {
    fn publish(&self, message: T) {
        if let Err(_e) = self.try_send(message) {
            error!("Failed to publish event: the channel is full")
        }
    }
}

impl<'a, M: RawMutex, T: Clone, const N: usize> EventSubscriber<T> for channel::Receiver<'a, M, T, N> {
    async fn next_event(&mut self) -> T {
        self.receive().await
    }
}

// Implementations for embassy-sync PubSubChannel
impl<'a, M: RawMutex, T: Clone, const CAP: usize, const SUBS: usize, const PUBS: usize> EventPublisher<T>
    for ImmediatePublisher<'a, M, T, CAP, SUBS, PUBS>
{
    fn publish(&self, message: T) {
        self.publish_immediate(message);
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
