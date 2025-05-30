//! Controller module for RMK
//!
//! This module defines the `Controller` trait and several macros for running output device controllers.
//! The `Controller` trait provides the interface for individual output device controllers, and the macros facilitate their concurrent execution.

use embassy_futures::select::{select, Either};

/// Common trait for controllers.
pub trait Controller {
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
/// impl EventController for MyController { }
///
/// // Use the input device
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
    async fn event_loop(&mut self) {
        loop {
            let event = self.next_message().await;
            self.process_event(event).await;
        }
    }
}

/// The trait for polling controllers.
///
/// This trait defines the interface for polling controllers in RMK.
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
/// impl PollingController for MyController {
///     type INTERVAL: embassy_time::Duration = embassy_time::Duration::from_hz(60);
///
///     async fn update(&mut self) {
///         // update periodic
///     }
/// }
///
/// // Use the input device
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
pub trait PollingController: Controller {
    /// Interval between `update` calls
    const INTERVAL: embassy_time::Duration;

    /// Update periodically
    async fn update(&mut self);

    /// Polling loop
    async fn polling_loop(&mut self) {
        let mut last = embassy_time::Instant::now();
        let mut elapsed;

        loop {
            let now = embassy_time::Instant::now();
            elapsed = now - last;
            last = now;
            match select(
                embassy_time::Timer::after(Self::INTERVAL - elapsed),
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
