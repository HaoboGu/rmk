//! Input device module for RMK
//!
//! This module defines the `InputDevice` trait, `InputProcessor` trait, `Runnable` trait and several macros for running input devices and processors.
//! The `InputDevice` trait provides the interface for individual input devices, and the macros facilitate their concurrent execution.
use crate::channel::KEYBOARD_REPORT_CHANNEL;
use crate::event::{EventSubscriber, InputSubscribeEvent};
use crate::hid::Report;

pub mod adc;
#[cfg(feature = "_ble")]
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
/// #[input_device(publish = MultiDeviceEvent)]
/// struct MyInputDevice;
///
/// impl MyInputDevice {
///     async fn read_multi_device_event(&mut self) -> MultiDeviceEvent {
///         // Implementation for reading multiple events
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
/// The input processor processes events from input devices and converts them to HID reports.
/// For example, the [`crate::matrix::Matrix`] is an input device and the [`crate::keyboard::Keyboard`]
/// is an input processor.
///
/// # Usage
///
/// There are two ways to implement an InputProcessor:
///
/// ## 1. Using the `#[input_processor]` macro (recommended)
///
/// For most use cases, use the `#[input_processor]` macro which automatically generates
/// the `EventSubscriber` type and all necessary boilerplate:
///
/// ```rust,ignore
/// use rmk_macro::input_processor;
///
/// // Subscribe to multiple input events
/// #[input_processor(subscribe = [KeyEvent, EncoderEvent])]
/// struct KeyProcessor;
///
/// impl KeyProcessor {
///     async fn on_key_event(&mut self, event: KeyEvent) {
///         // Process key event
///     }
///
///     async fn on_encoder_event(&mut self, event: EncoderEvent) {
///         // Process encoder event
///     }
/// }
/// ```
///
/// ## 2. Manual implementation (for single event types)
///
/// For simple processors that subscribe to a single event type, you can manually
/// implement the trait:
///
/// ```rust,ignore
/// use rmk::event::InputSubscribeEvent;
///
/// impl InputProcessor for MyProcessor {
///     type Event = KeyboardEvent;
///
///     // subscriber() has a default implementation, no need to override
///
///     async fn process(&mut self, event: Self::Event) {
///         // Process the event
///     }
/// }
/// ```
///
/// **Note**: For multiple event subscriptions, you must use the `#[input_processor]` macro
/// as it generates the necessary aggregated event type that implements `InputSubscribeEvent`.
pub trait InputProcessor: Runnable {
    /// The event type processed by this input processor.
    ///
    /// Must implement `InputSubscribeEvent`, which provides the `Subscriber` type
    /// and the `input_subscriber()` method.
    type Event: InputSubscribeEvent;

    /// Create a new event subscriber.
    ///
    /// Default implementation uses the event's `input_subscriber()` method.
    fn subscriber() -> <Self::Event as InputSubscribeEvent>::Subscriber {
        Self::Event::input_subscriber()
    }

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

    /// Default processing loop that continuously receives and processes events
    async fn process_loop(&mut self) -> ! {
        let mut sub = Self::subscriber();
        loop {
            let event = sub.next_event().await;
            self.process(event).await;
        }
    }
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
