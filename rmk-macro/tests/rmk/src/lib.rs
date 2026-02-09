//! Mock rmk crate for macro expansion tests.
//! Provides minimal types and macros for macro expansion.

#![no_std]
#![allow(dead_code)]
#![allow(async_fn_in_trait)]

// Re-export futures for tests.
pub use futures;

// Re-export select_biased! macro.
#[macro_export]
macro_rules! select_biased {
    ($($tokens:tt)*) => {
        $crate::futures::select_biased!($($tokens)*)
    };
}

pub mod event {
    pub trait EventPublisher {
        type Event;
        fn publish(&self, event: Self::Event);
    }

    pub trait AsyncEventPublisher {
        type Event;
        fn publish_async(&self, event: Self::Event)
            -> impl core::future::Future<Output = ()>;
    }

    pub trait EventSubscriber {
        type Event;
        fn next_event(&mut self)
            -> impl core::future::Future<Output = Self::Event>;
    }

    pub trait PublishableEvent: Clone + Send {
        type Publisher;
        fn publisher() -> Self::Publisher;
    }

    pub trait SubscribableEvent: Clone + Send {
        type Subscriber;
        fn subscriber() -> Self::Subscriber;
    }

    pub trait AsyncPublishableEvent: PublishableEvent {
        type AsyncPublisher;
        fn publisher_async() -> Self::AsyncPublisher;
    }

    pub fn publish_event<E: PublishableEvent>(_e: E) {
        // No-op mock.
    }

    pub async fn publish_event_async<E: AsyncPublishableEvent>(_e: E) {
        // No-op mock.
    }
}

pub mod input_device {
    pub trait Runnable {
        async fn run(&mut self) -> !;
    }

    pub trait InputDevice: Runnable {
        type Event;
        async fn read_event(&mut self) -> Self::Event;
    }
}

pub struct KeyMap<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize> {
    _phantom: core::marker::PhantomData<&'a ()>,
}
