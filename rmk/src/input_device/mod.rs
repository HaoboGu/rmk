//! Input device module for RMK
//!
//! This module defines the `InputDevice` trait, `Runnable` trait and several macros for running input devices and processors.
//! The `InputDevice` trait provides the interface for individual input devices, and the macros facilitate their concurrent execution.

pub mod adc;
#[cfg(feature = "_ble")]
pub mod battery;
pub mod joystick;
pub mod pmw33xx;
pub mod pmw3610;
pub mod pointing;
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
/// Use the `#[input_device]` macro to automatically implement this trait.
///
/// # Example
/// ```rust
/// // For single-event devices, use the macro:
/// #[input_device(publish = BatteryEvent)]
/// struct MyInputDevice;
///
/// impl MyInputDevice {
///     // You MUST implement this read method for the published event.
///     // The method name follows the pattern: read_{event_name}_event
///     async fn read_battery_event(&mut self) -> BatteryEvent {
///         // Implementation for reading an event
///     }
/// }
///
/// // For multi-event devices, a derived enum should be used.
/// // **Note**: Wrapper enums only implement publish traits, not subscribe traits.
/// // This is because wrapper enums route events to their concrete type channels,
/// // and you should subscribe to the individual event types instead.
/// #[derive(InputEvent)]
/// enum MultiDeviceEvent {
///     Battery(BatteryEvent),
///     Pointing(PointingEvent),
/// }
///
/// #[input_device(publish = MultiDeviceEvent)]
/// struct MyInputDevice;
///
/// impl MyInputDevice {
///     // Returns the `MultiDeviceEvent`
///     async fn read_multi_device_event(&mut self) -> MultiDeviceEvent {
///         // Implementation for reading multiple types of events
///     }
/// }
/// ```
pub trait InputDevice: Runnable {
    /// The event type produced by this input device
    type Event;

    /// Read the raw input event
    async fn read_event(&mut self) -> Self::Event;
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
///
/// // Run all runnables concurrently
/// run_all!(device, processor);
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
