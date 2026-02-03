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
    pub trait EventSubscriber<T> {
        fn next_event(&mut self) -> impl core::future::Future<Output = T>;
    }

    pub trait AsyncEventPublisher<T> {
        fn publish_async(&self, event: T) -> impl core::future::Future<Output = ()>;
    }

    pub trait InputEvent {
        type Publisher;
        type Subscriber;

        fn input_publisher() -> Self::Publisher;
        fn input_subscriber() -> Self::Subscriber;
    }

    pub trait AsyncInputEvent: InputEvent {
        type AsyncPublisher;

        fn input_publisher_async() -> Self::AsyncPublisher;
    }

    pub trait ControllerEvent {
        fn controller_subscriber() -> impl EventSubscriber<Self>
        where
            Self: Sized;
    }

    pub async fn publish_input_event_async<E: AsyncInputEvent>(_e: E) {
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

    pub trait InputProcessor<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize> {
        type Event;
        fn process(&mut self, event: Self::Event) -> impl core::future::Future<Output = ()>;
        fn get_keymap(&self) -> &core::cell::RefCell<crate::KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>;
    }
}

pub mod controller {
    pub trait Controller {
        type Event;
        fn process_event(&mut self, event: Self::Event) -> impl core::future::Future<Output = ()>;
        fn next_message(&mut self) -> impl core::future::Future<Output = Self::Event>;
    }

    pub trait PollingController: Controller {
        fn update(&mut self) -> impl core::future::Future<Output = ()>;
    }
}

pub struct KeyMap<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize> {
    _phantom: core::marker::PhantomData<&'a ()>,
}
