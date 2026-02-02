//! Controller module for RMK
//!
//! This module defines the `Controller` trait and its variations for different modes of execution.

#[cfg(feature = "_ble")]
pub mod battery_led;
pub mod led_indicator;
pub(crate) mod wpm;

use embassy_futures::select::{Either, select};

use crate::input_device::Runnable;

/// This trait provides the interface for individual output device controllers.
pub trait Controller: Runnable {
    /// Type of the received events
    type Event;

    /// Process the received event
    async fn process_event(&mut self, event: Self::Event);

    /// Block waiting for next message
    async fn next_message(&mut self) -> Self::Event;
}

/// The trait for event-driven controllers.
///
/// This trait defines the interface for event-driven controllers in RMK.
///
/// # Example
/// ```rust
/// // Define a controller
/// struct MyController;
///
/// impl Controller for MyController {
///     async fn process_event(&mut self, event: Self::Event) {
///         // handle event
///     }
/// }
///
/// // Use the controller
/// let c = MyController;
///
/// // Run device simultaneously with RMK
/// embassy_futures::join::join(
///     c.event_loop(),
///     run_rmk(
///         // ...
///     ),
/// )
/// .await;
/// ```
pub trait EventController: Controller {
    /// Event loop
    async fn event_loop(&mut self) -> ! {
        loop {
            let event = self.next_message().await;
            self.process_event(event).await;
        }
    }
}

// Auto impl `EventController` trait for all `Controller`
impl<T: Controller> EventController for T {}

/// The trait for polling controllers.
///
/// This trait defines the interface for polling controllers in RMK.
/// The polling interval can be configured either as a fixed value or
/// dynamically at runtime through the [`Self::interval()`] method.
///
/// # Example (Fixed Interval)
/// ```rust
/// // Define a controller with a fixed interval
/// struct MyController;
///
/// impl Controller for MyController {
///     async fn process_event(&mut self, event: Self::Event) {
///         // handle event
///     }
/// }
///
/// impl PollingController for MyController {
///     fn interval(&self) -> embassy_time::Duration {
///         embassy_time::Duration::from_hz(30)
///     }
///
///     async fn update(&mut self) {
///         // update periodic
///     }
/// }
///
/// // Use the controller
/// let c = MyController;
///
/// // Run device simultaneously with RMK
/// embassy_futures::join::join(
///     c.polling_loop(),
///     run_rmk(
///         // ...
///     ),
/// )
/// .await;
/// ```
///
/// # Example (Dynamic Interval)
/// ```rust
/// // Define a controller with a configurable interval
/// struct ConfigurableController {
///     interval: embassy_time::Duration,
/// }
///
/// impl ConfigurableController {
///     /// Create a controller with a specific update frequency in Hz
///     pub fn with_hz(hz: u32) -> Self {
///         Self {
///             interval: embassy_time::Duration::from_hz(hz as u64),
///         }
///     }
/// }
///
/// impl Controller for ConfigurableController {
///     async fn process_event(&mut self, event: Self::Event) {
///         // handle event
///     }
/// }
///
/// impl PollingController for ConfigurableController {
///     fn interval(&self) -> embassy_time::Duration {
///         self.interval
///     }
///
///     async fn update(&mut self) {
///         // update periodic
///     }
/// }
///
/// // Use the controller with 60Hz update rate
/// let c = ConfigurableController::with_hz(60);
/// ```
pub trait PollingController: Controller {
    /// Returns the interval between `update` calls.
    ///
    /// This method can be overridden to provide a custom interval
    fn interval(&self) -> embassy_time::Duration;

    /// Update periodically, will be called according to [`Self::interval()`]
    async fn update(&mut self);

    /// Polling loop
    async fn polling_loop(&mut self) -> ! {
        let mut last = embassy_time::Instant::now();

        loop {
            let elapsed = last.elapsed();

            match select(
                embassy_time::Timer::after(
                    self.interval()
                        .checked_sub(elapsed)
                        .unwrap_or(embassy_time::Duration::MIN),
                ),
                self.next_message(),
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
