//! Input device module for RMK
//!
//! This module defines the `InputDevice` trait, `InputProcessor` trait and the `bind_device_and_processor` macro, enabling the simultaneous execution of multiple input devices.
//! The `InputDevice` trait provides the interface for individual input devices, and the `bind_device_and_processor` macro facilitates their concurrent execution.
use crate::{channel::KEYBOARD_REPORT_CHANNEL, event::Event, hid::Report};

pub mod rotary_encoder;

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
/// The [`Matrix`] is actually an input device and the [`Keyboard`] is actually an input processor.
pub trait InputProcessor {
    /// Process the incoming events, convert them to HID report [`KeyboardReportMessage`],
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

// TODO: Runnable device and processor
// For example, devices like keymatrix emits multiple events during one pass, so the `read_event` is not good for thoese devices
// Adding runnable devices makes it possible to

/// Macro to bind input devices and an input processor into a single asynchronous task.
///
/// This macro simplifies the creation of a task that reads events from multiple input devices
/// and processes them using a specified input processor. It allows for efficient handling of
/// input events in a concurrent manner.
///
/// # Arguments
///
/// * `task_name`: The name of the asynchronous task to be created.
/// * `dev`: A list of input devices, each with a specified type.
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
/// // Bind devices and processor into a task
/// bind_device_and_processor!(my_task = (device1: MyInputDevice, device2: MyInputDevice2) => processor: MyInputProcessor);
///
/// // In main function, you need to spawn `my_task`
/// #[embassy_executor::main]
/// async fn main(spawner: Spawner) {
///     // ...
///     spawner.spawn(my_task(processor, my_device, my_device2)).unwrap();
/// }
/// ```
///
/// This macro will create an asynchronous task named `my_task` that continuously reads events
/// from `device1` and `device2`, and processes them using `processor`.
/// The task will run indefinitely in a loop, handling events as they come in.
#[macro_export]
macro_rules! bind_device_and_processor_and_run {
    (($( $dev:ident),*) => $proc:ident) => {
        async {
            use $crate::futures::{self, FutureExt, select_biased};
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
