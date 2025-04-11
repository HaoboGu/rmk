//! Input device module for RMK
//!
//! This module defines the `InputDevice` trait, `InputProcessor` trait, `Runnable` trait and several macros for running input devices and processors.
//! The `InputDevice` trait provides the interface for individual input devices, and the macros facilitate their concurrent execution.
use core::cell::RefCell;

use crate::channel::KEYBOARD_REPORT_CHANNEL;
use crate::event::Event;
use crate::hid::Report;
use crate::keymap::KeyMap;

pub mod adc;
pub mod battery;
pub mod joystick;
pub mod rotary_encoder;

/// The trait for runnable input devices and processors.
///
/// For some input devices or processors, they should keep running in a separate task.
/// This trait is used to run them in a separate task.
pub trait Runnable {
    async fn run(&mut self);
}

/// The trait for input devices.
///
/// This trait defines the interface for input devices in RMK.
/// The `run_devices` macro is required to run tasks associated with input devices concurrently.
///
/// # Example
/// ```rust
/// // Define an input device
/// struct MyInputDevice;
///
/// impl InputDevice for MyInputDevice {
///     async fn read_event(&mut self) -> Event {
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
///     run_devices!((d1, d2) => EVENT_CHANNEL),
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

/// Processing result of the processor chain
pub enum ProcessResult {
    /// Continue processing the event
    Continue(Event),
    /// Stop processing
    Stop,
}

/// The trait for input processors.
///
/// The input processor processes the [`Event`] from the input devices and converts it to the final HID report.
/// Take the normal keyboard as the example:
///
/// The [`crate::matrix::Matrix`] is actually an input device and the [`crate::keyboard::Keyboard`] is actually an input processor.
pub trait InputProcessor<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize = 0> {
    /// Process the incoming events, convert them to HID report [`Report`],
    /// then send the report to the USB/BLE.
    ///
    /// Note there might be mulitple HID reports are generated for one event,
    /// so the "sending report" operation should be done in the `process` method.
    /// The input processor implementor should be aware of this.
    async fn process(&mut self, event: Event) -> ProcessResult;

    /// Send the processed report.
    async fn send_report(&self, report: Report) {
        KEYBOARD_REPORT_CHANNEL.send(report).await;
    }

    /// Get the current keymap
    fn get_keymap(&self) -> &RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>;
}

/// Macro to bind input devices to event channels and run all of them.
///
/// This macro simplifies the creation of a task that reads events from multiple input devices
/// and sends them to specified channels. It allows for efficient handling of
/// input events in a concurrent manner.
///
/// # Arguments
///
/// * `dev`: A list of input devices grouped in parentheses.
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
/// let device_future = run_devices! {
///     (d1, d2) => local_channel,
///     (matrix) => rmk::EVENT_CHANNEL,
/// };
///
/// ```
#[macro_export]
macro_rules! run_devices {
    ( $( ( $( $dev:ident ),* ) => $channel:expr),+ $(,)? ) => {{
        use $crate::input_device::InputDevice;
        $crate::join_all!(
            $(
                $crate::join_all!(
                    $(
                        async {
                            loop {
                                let e = $dev.read_event().await;
                                // For KeyEvent, send it to KEY_EVENT_CHANNEL
                                match e {
                                    $crate::event::Event::Key(key_event) => {
                                        $crate::channel::KEY_EVENT_CHANNEL.send(key_event).await;
                                    }
                                    _ => {
                                        // Drop the oldest event if the channel is full
                                        if $channel.is_full() {
                                           let _ = $channel.receive().await;
                                        }
                                        $channel.send(e).await;
                                    }
                                }
                            }
                        }
                    ),*
                )
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
/// * `dev`: A list of input devices grouped in parentheses.
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
/// let device_future = bind_device_and_processor_and_run!((device1, device2) => processor);
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

/// Macro for binding input processor chain to event channel and running them.
///
/// FIXME: For split keyboard, `EVENT_CHANNEL` is REQUIRED as it's the default channel for receiving events from peripherals.
///
/// This macro creates tasks that receive events from channels and process them using specified processor chains.
/// It calls processors in order and decides whether to continue the chain based on the result of each processor.
///
/// # Arguments
///
/// * `channel`: The channel to receive events from
/// * `procs`: The processor list that will handle the events
///
/// # Example
///
/// ```rust
/// use rmk::channel::{blocking_mutex::raw::NoopRawMutex, channel::Channel, EVENT_CHANNEL};
/// // Create a local channel for processor chain
/// let local_channel: Channel<NoopRawMutex, Event, 16> = Channel::new();
/// // Two chains, one use local channel, the other use the built-in channel
/// let processor_future = run_processor_chain! {
///     local_channel => [processor1, processor2, processor3]
///     EVENT_CHANNEL => [processor4, processor5, processor6]
/// };
/// ```
#[macro_export]
macro_rules! run_processor_chain {
    ( $( $channel:expr => [ $first:expr $(, $rest:expr )* ] ),+ $(,)? ) => {{
        use rmk::input_device::InputProcessor;
        $crate::join_all!(
            $(
                async {
                    loop {
                        let event = $channel.receive().await;

                        // Process the event with the first processor
                        match $first.process(event).await {
                            $crate::input_device::ProcessResult::Stop => {
                                // If the first processor returns Stop, continue to wait for the next event
                                continue;
                            },
                            $crate::input_device::ProcessResult::Continue(next_event) => {
                                // Pass the result to the next processor in the chain
                                let mut current_event = next_event;
                                $(
                                    match $rest.process(current_event).await {
                                        $crate::input_device::ProcessResult::Stop => {
                                            // If any processor returns Stop, continue to wait for the next event
                                            continue;
                                        },
                                        $crate::input_device::ProcessResult::Continue(next_event) => {
                                            // Update the current event and continue processing
                                            current_event = next_event;
                                        }
                                    }
                                )*
                            }
                        }
                    }
                }
            ),+
        )
    }};
}
