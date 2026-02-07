//! Controller module for RMK
//!
//! This module defines the `Controller` trait and its variations for different modes of execution.

#[cfg(feature = "_ble")]
pub mod battery_led;
pub mod led_indicator;
pub(crate) mod wpm;

use embassy_futures::select::{Either, select};

use crate::event::{EventSubscriber, SubscribableControllerEvent};
use crate::input_device::Runnable;

/// The trait for controllers.
///
/// This trait provides the interface for individual output device controllers.
/// Use the `#[controller]` to define a controller struct.
/// This trait will be automatically implemented by using `#[controller]` macro.
///
/// ```rust,ignore
/// use rmk_macro::controller;
///
/// // Single event subscription
/// #[controller(subscribe = [LedIndicatorEvent])]
/// struct MyController { /* ... */ }
///
/// impl MyController {
///     // You MUST implement on_{event_name}_event handler method for each event type in `subscribe = [..]`
///     // For example, the method name is on_led_indicator_event for LedIndicatorEvent.
///     async fn on_led_indicator_event(&mut self, event: LedIndicatorEvent) {
///         // handle event
///     }
/// }
///
/// // Multiple event subscription
/// #[controller(subscribe = [EventA, EventB])]
/// struct MyMultiController { /* ... */ }
///
/// impl MyMultiController {
///     // Each subscribed event type requires a corresponding handler method
///     async fn on_event_a_event(&mut self, event: EventA) { /* ... */ }
///     async fn on_event_b_event(&mut self, event: EventB) { /* ... */ }
/// }
///
/// // With polling support
/// #[controller(subscribe = [EventA], poll_interval = 100)]
/// struct MyPollingController { /* ... */ }
///
/// impl MyPollingController {
///     async fn on_event_a_event(&mut self, event: EventA) { /* ... */ }
///
///     // When poll_interval is set, you MUST also implement poll()
///     async fn poll(&mut self) {
///         // Called periodically at the specified interval (in ms)
///     }
/// }
/// ```
pub trait Controller: Runnable {
    /// Type of the received events.
    type Event: SubscribableControllerEvent;

    /// Create a new event subscriber.
    fn subscriber() -> <Self::Event as SubscribableControllerEvent>::Subscriber {
        Self::Event::controller_subscriber()
    }

    /// Process the received event
    async fn process_event(&mut self, event: Self::Event);
}

/// The trait for event-driven controllers.
///
/// This trait is automatically implemented for all types that implement `Controller`.
/// It provides a default `event_loop()` implementation that continuously waits for
/// events and processes them.
///
/// # Example
///
/// ```rust,ignore
/// use rmk_macro::controller;
/// use rmk::controller::EventController;
///
/// #[controller(subscribe = [LedIndicatorEvent])]
/// struct MyController;
///
/// impl MyController {
///     async fn on_led_indicator_event(&mut self, event: LedIndicatorEvent) {
///         // handle event
///     }
/// }
///
/// // Run the controller using `event_loop`
/// let mut c = MyController;
/// c.event_loop().await;
/// // Or you can just use `run` from `Runnable` trait
/// c.run().await;
/// ```
pub trait EventController: Controller {
    /// Event loop that continuously processes incoming events
    async fn event_loop(&mut self) -> ! {
        let mut sub = Self::subscriber();
        loop {
            let event = sub.next_event().await;
            self.process_event(event).await;
        }
    }
}

// Auto impl `EventController` trait for all `Controller`
impl<T: Controller> EventController for T {}

/// The trait for polling controllers.
///
/// This trait extends `Controller` with periodic update capability.
/// The polling loop alternates between waiting for events and calling `update()`
/// at the specified interval.
///
/// # Example
///
/// ```rust,ignore
/// use rmk_macro::controller;
/// use rmk::controller::PollingController;
///
/// #[controller(subscribe = [BatteryStateEvent], poll_interval = 1000)]
/// struct BatteryLedController {
///     led_on: bool,
/// }
///
/// impl BatteryLedController {
///     async fn on_battery_state_event(&mut self, event: BatteryStateEvent) {
///         // Update internal state based on battery event
///     }
///
///     // Called every 1000ms (poll_interval)
///     async fn poll(&mut self) {
///         // Toggle LED based on battery state
///         self.led_on = !self.led_on;
///     }
/// }
///
/// // Run the controller with polling
/// let mut c = BatteryLedController { led_on: false };
/// c.polling_loop().await;
/// // Or you can just use `run` from `Runnable` trait.
/// // The controller macro generates polling mode `run` for polling controllers.
/// c.run().await;
/// ```
pub trait PollingController: Controller {
    /// Returns the interval between `update` calls.
    fn interval(&self) -> embassy_time::Duration;

    /// Update periodically, will be called according to [`Self::interval()`]
    async fn update(&mut self);

    /// Polling loop that processes events and calls `update()` at the specified interval
    async fn polling_loop(&mut self) -> ! {
        let mut sub = Self::subscriber();
        let mut last = embassy_time::Instant::now();

        loop {
            let elapsed = last.elapsed();

            match select(
                embassy_time::Timer::after(
                    self.interval()
                        .checked_sub(elapsed)
                        .unwrap_or(embassy_time::Duration::MIN),
                ),
                sub.next_event(),
            )
            .await
            {
                Either::First(_) => {
                    self.update().await;
                    last = embassy_time::Instant::now();
                }
                Either::Second(event) => self.process_event(event).await,
            }
        }
    }
}
