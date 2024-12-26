//! Input device module for RMK
//!
//! This module defines the `InputDevice` trait and the `run_devices` macro, enabling the simultaneous execution of multiple input devices.
//! The `InputDevice` trait provides the interface for individual input devices, and the `run_devices` macro facilitates their concurrent execution.
//!
//! Note: The `InputDevice` trait must be used in conjunction with the `run_devices` macro to ensure correct execution of all input devices.

use core::future::Future;

use crate::keyboard::key_event_channel;

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
    /// Starts the input device task.
    ///
    /// This asynchronous method should contain the main logic for the input device.
    /// It will be executed concurrently with other input devices using the `run_devices` macro.
    fn run(&mut self) -> impl Future<Output = ()>;
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
