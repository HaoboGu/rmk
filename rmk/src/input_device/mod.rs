//! Input device module for RMK
//!
//! This module defines the `InputDevice` trait, `InputProcessor` trait and the `bind_device_and_processor` macro, enabling the simultaneous execution of multiple input devices.
//! The `InputDevice` trait provides the interface for individual input devices, and the `bind_device_and_processor` macro facilitates their concurrent execution.
use crate::{channel::KEYBOARD_REPORT_CHANNEL, event::Event, hid::Report};

pub mod rotary_encoder;

// TODO: Runnable device and processor

/// The trait for input devices.
///
/// This trait defines the interface for input devices in RMK.
/// The `bind_device_and_processor` macro is required to run tasks associated with input devices concurrently.
///
/// # Example
/// ```rust
/// // Define an input device
/// struct MyInputDevice;
///
/// impl InputDevice for MyInputDevice {
///     async fn run(&mut self) {
///         // Input device implementation
///     }
/// }
///
/// // Use the input device
/// let d1 = MyInputDevice{};
/// let d2 = MyInputDevice{};
///
/// // Run all devices simultaneously with RMK
/// embassy_futures::join::join(
///     run_devices!(d1, d2),
///     run_rmk(
///         // .. arguments
///     ),
/// )
/// .await;
/// ```
pub trait InputDevice {
    /// Read the raw input event
    async fn read_event(&mut self) -> Event;
}

/// The trait for input processors.
///
/// The input processor processes the [`Event`] from the input devices and converts it to the final HID report.
/// Take the normal keyboard as the example:
///
/// The [`crate::matrix::Matrix`] is actually an input device and the [`crate::keyboard::Keyboard`] is actually an input processor.
pub trait InputProcessor {
    /// Process the incoming events, convert them to HID report [`Report`],
    /// then send the report to the USB/BLE.
    ///
    /// Note there might be mulitple HID reports are generated for one event,
    /// so the "sending report" operation should be done in the `process` method.
    /// The input processor implementor should be aware of this.  
    async fn process(&mut self, event: Event);

    /// Send the processed report.
    async fn send_report(&self, report: Report) {
        KEYBOARD_REPORT_CHANNEL.send(report).await;
    }
}

/// Macro to bind input devices to event channels and run all of them.
///
/// This macro simplifies the creation of a task that reads events from multiple input devices
/// and send all of them to. It allows for efficient handling of
/// input events in a concurrent manner.
///
/// # Arguments
///
/// * `dev`: A list of input devices.
/// * `channel`: The channel that devices send the events to.
///
/// # Example
/// ```rust
/// use rmk::channel::{blocking_mutex::raw::NoopRawMutex, channel::Channel, EVENT_CHANNEL};
/// // Initialize channel
/// let local_channel: Channel<NoopRawMutex, Event, 16> = Channel::new();
///
/// // Define your input devices, both MyInputDevice and MyInputDevice2 should implement `InputDevice] trait
/// struct MyInputDevice;
/// struct MyInputDevice2;
///
/// let d1 = MyInputDevice{};
/// let d2 = MyInputDevice2{};
/// // Bind devices to channels and run, RMK also provides EVENT_CHANNEL for general use
/// let device_future = run_device! {
///     (d1, d2) => local_channel,
///     (matrix) => rmk::EVENT_CHANNEL,
/// };
///
/// ```

#[macro_export]
macro_rules! run_devices {
    ( $( ( $( $dev:ident ),* ) => $channel:ident ),+ $(,)? ) => {{
        use $crate::futures::{self, future::FutureExt, select_biased};
        $crate::join_all!(
            $(
                async {
                    loop {
                        let e = select_biased! {
                            $(
                                e = $dev.read_event().fuse() => e,
                            )*
                        };
                        $channel.send(e).await;
                    }
                }
            ),+
        )
    }};
}

#[macro_export]
macro_rules! run_processors {
    ( $( $channel:ident => $proc:ident ),+ $(,)? ) => {{
        use $crate::futures::{self, future::FutureExt, select_biased};
        $crate::join_all!(
            $(
                async {
                    loop {
                        let e = $channel.receive().await;
                        $proc.process(e).await;
                    }
                }
            ),+
        )
    }};
}

/// Macro to bind input devices and an input processor directly.
///
/// This macro simplifies the creation of a task that reads events from multiple input devices
/// and processes them using a specified input processor. It allows for efficient handling of
/// input events in a concurrent manner.
///
/// # Arguments
///
/// * `dev`: A list of input devices.
/// * `proc`: The input processor that will handle the events from the devices.
///
/// # Example
/// ```rust
/// // Define your input devices and processor
/// struct MyInputDevice;
/// struct MyInputDevice2;
/// struct MyInputProcessor;
///
/// impl InputDevice for MyInputDevice {
///     async fn read_event(&mut self) -> Event {
///         // Implementation for reading an event
///     }
/// }
///
/// impl InputProcessor for MyInputProcessor {
///     async fn process(&mut self, event: Event) {
///         // Implementation for processing an event
///     }
/// }
///
/// // Bind devices and processor into a task, aka use `processor` to process input events from `device1` and `device2`
/// let device_future = bind_device_and_processor!((device1, device2) => processor);
///
/// ```
#[macro_export]
macro_rules! bind_device_and_processor_and_run {
    (($( $dev:ident),*) => $proc:ident) => {
        async {
            use $crate::futures::{self, FutureExt, select_biased};
            use $crate::input_device::{InputDevice, InputProcessor};
            loop {
                let e = select_biased! {
                    $(
                        e = $dev.read_event().fuse() => e,
                    )*
                };
                $proc.process(e).await;
            }
        }
    };
}

/// Helper macro for joining all futures
#[macro_export]
macro_rules! join_all {
    ($first:expr, $second:expr, $($rest:expr),*) => {
        $crate::futures::future::join(
            $first,
            $crate::join_all!($second, $($rest),*)
        )
    };
    ($a:expr, $b:expr) => {
        $crate::futures::future::join($a, $b)
    };
    ($single:expr) => { $single };
}
