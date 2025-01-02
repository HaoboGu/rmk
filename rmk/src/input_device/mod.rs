//! Input device module for RMK
//!
//! This module defines the `InputDevice` trait and the `run_devices` macro, enabling the simultaneous execution of multiple input devices.
//! The `InputDevice` trait provides the interface for individual input devices, and the `run_devices` macro facilitates their concurrent execution.
//!
//! Note: The `InputDevice` trait must be used in conjunction with the `run_devices` macro to ensure correct execution of all input devices.

use core::future::Future;

use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};

use crate::keyboard::{EVENT_CHANNEL_SIZE, REPORT_CHANNEL_SIZE};

pub mod rotary_encoder;

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
    /// Event type that input device will send
    type EventType;

    /// The number of required channel size
    // FIXME: it's not possible in stable to define an associated const and use it as the channel size:
    // `fn get_channel(..) -> &Channel<CriticalSectionRawMutex, Self::EventType, Self::EVENT_CHANNEL_SIZE >;`
    //
    // It requires #[feature(generic_const_exprs)] and adding `{}` to the const:
    // `fn get_channel(..) -> &Channel<CriticalSectionRawMutex, Self::EventType, { Self::EVENT_CHANNEL_SIZE } >;`
    //
    // const EVENT_CHANNEL_SIZE: usize = 32;

    /// Starts the input device task.
    ///
    /// This asynchronous method should contain the main logic for the input device.
    /// It will be executed concurrently with other input devices using the `run_devices` macro.
    fn run(&mut self) -> impl Future<Output = ()>;

    /// Get the event channel for the input device. All events should be send by this channel.
    fn get_channel(&self)
        -> &Channel<CriticalSectionRawMutex, Self::EventType, EVENT_CHANNEL_SIZE>;
}

/// The trait for input processors.
///
/// The input processor processes the [`Event`] from the input devices and converts it to the final HID report.
/// Take the normal keyboard as the example:
///
/// The [`Matrix`] is actually an input device and the [`Keyboard`] is actually an input processor.
pub trait InputProcessor {
    /// The event type that the input processor receives.
    type EventType;

    /// The report type that the input processor sends.
    type ReportType;

    /// Process the incoming events, convert them to HID report [`KeyboardReportMessage`],
    /// then send the report to the USB/BLE.
    ///
    /// Note there might be mulitple HID reports are generated for one event,
    /// so the "sending report" operation should be done in the `process` method.
    /// The input processor implementor should be aware of this.  
    fn process(&mut self, event: Self::EventType) -> impl Future<Output = ()>;

    /// Get the input event channel for the input processor.
    ///
    /// The input processor receives events from this channel, processes the event,
    /// then sends to the report channel.
    fn get_event_channel(
        &self,
    ) -> &Channel<CriticalSectionRawMutex, Self::EventType, EVENT_CHANNEL_SIZE>;

    /// Get the output report channel for the input processor.
    ///
    /// The input processor sends keyboard reports to this channel.
    fn get_report_channel(
        &self,
    ) -> &Channel<CriticalSectionRawMutex, Self::ReportType, REPORT_CHANNEL_SIZE>;

    /// Default implementation of the input processor. It wait for a new event from the event channel,
    /// then process the event.
    ///
    /// The report is sent to the USB/BLE in the `process` method.
    fn run(&mut self) -> impl Future<Output = ()> {
        async {
            loop {
                let event = self.get_event_channel().receive().await;
                self.process(event).await;
            }
        }
    }
}

/// Macro to run multiple input devices concurrently.
///
/// The `run_devices` macro is specifically designed to work with the `InputDevice` trait. It takes one or more instances of
/// input devices and combines their `run` methods into a single future. All futures are executed concurrently, enabling
/// efficient multitasking for multiple input devices.
///
/// # Note
/// This macro must be used with input devices that implement the `InputDevice` trait.
///
/// # Example
/// ```rust
/// // `MyInputDevice` should implement `InputDevice` trait
/// let d1 = MyInputDevice{};
/// let d2 = MyInputDevice{};
///
/// // Run all input devices concurrently
/// run_devices!(d1, d2).await;
/// ```
#[macro_export]
macro_rules! run_devices {
    // Single device case
    ($single:expr) => {
        $single.run()
    };
    // Multiple devices case
    ($first:expr, $second:expr $(, $rest:expr)*) => {
        ::embassy_futures::join::join($first.run(), run_devices!($second $(, $rest)*))
    };
}