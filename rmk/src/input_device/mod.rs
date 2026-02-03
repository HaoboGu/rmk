//! Input device module for RMK
//!
//! This module defines the `InputDevice` trait, `InputProcessor` trait, `Runnable` trait and several macros for running input devices and processors.
//! The `InputDevice` trait provides the interface for individual input devices, and the macros facilitate their concurrent execution.
use core::cell::RefCell;

use crate::channel::KEYBOARD_REPORT_CHANNEL;
use crate::hid::Report;
use crate::keymap::KeyMap;

pub mod adc;
pub mod battery;
pub mod joystick;
pub mod pmw3610;
pub mod rotary_encoder;

/// The trait for runnable input devices and processors.
///
/// For some input devices or processors, they should keep running in a separate task.
/// This trait is used to run them in a separate task.
pub trait Runnable {
    async fn run(&mut self) -> !;
}

/// The trait for input devices.
///
/// This trait defines the interface for input devices in RMK.
/// Use the `#[input_device]` macro to automatically implement this trait for single-event devices,
/// or implement it manually for multi-event devices using `#[derive(InputEvent)]`.
///
/// # Example
/// ```rust
/// // For single-event devices, use the macro:
/// #[input_device(publish = BatteryEvent)]
/// struct MyInputDevice;
///
/// impl MyInputDevice {
///     async fn read_battery_event(&mut self) -> BatteryEvent {
///         // Implementation for reading an event
///     }
/// }
///
/// // For multi-event devices, implement manually:
/// #[derive(InputEvent)]
/// enum MultiDeviceEvent {
///     Battery(BatteryEvent),
///     Pointing(PointingEvent),
/// }
///
/// impl InputDevice for MultiDevice {
///     type Event = MultiDeviceEvent;
///     async fn read_event(&mut self) -> Self::Event {
///         // Implementation
///     }
/// }
/// ```
pub trait InputDevice: Runnable {
    /// The event type produced by this input device
    type Event;

    /// Read the raw input event
    async fn read_event(&mut self) -> Self::Event;
}

/// The trait for input processors.
///
/// The input processor processes the [`Event`] from the input devices and converts it to the final HID report.
/// Take the normal keyboard as the example:
///
/// The [`crate::matrix::Matrix`] is actually an input device and the [`crate::keyboard::Keyboard`] is actually an input processor.
pub trait InputProcessor<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize = 0>:
    Runnable
{
    type Event;

    /// Process the incoming events, convert them to HID report [`Report`],
    /// then send the report to the USB/BLE.
    ///
    /// Note there might be multiple HID reports generated for one event,
    /// so the "sending report" operation should be done in the `process` method.
    /// The input processor implementor should be aware of this.
    async fn process(&mut self, event: Self::Event);

    /// Send the processed report.
    async fn send_report(&self, report: Report) {
        KEYBOARD_REPORT_CHANNEL.send(report).await;
    }

    /// Get the current keymap
    fn get_keymap(&self) -> &RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>;
}

/// Macro to run multiple Runnable instances concurrently.
///
/// This macro simplifies running multiple input devices, processors, or controllers
/// that implement the `Runnable` trait.
///
/// # Example
/// ```rust
/// // Define your runnables
/// let mut device = MyInputDevice::new();
/// let mut processor = MyProcessor::new();
/// let mut controller = MyController::new();
///
/// // Run all runnables concurrently
/// run_all!(device, processor, controller);
/// ```
#[macro_export]
macro_rules! run_all {
    ($( $dev:ident ),*) => {{
        use $crate::input_device::Runnable;
        $crate::join_all!(
            $(
                $dev.run()
            ),*
        )
    }};
}
