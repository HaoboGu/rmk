//! Controller module for RMK
//!
//! This module defines the `Controller` trait and several macros for running output device controllers.
//! The `Controller` trait provides the interface for individual output device controllers, and the macros facilitate their concurrent execution.

/// The trait for controllers.
///
/// This trait defines the interface for event-driven controllers in RMK.
///
/// # Example
/// ```rust
/// // Define a controller
/// struct MyController {
///     // ...
/// }
///
/// impl EventController for MyController {
///     type Event = rmk::event::ControllerEvent;
///
///     async fn process_event(&mut self, event: Self::Event) {
///         // handle event
///     }
///     async fn next_message(&mut self) -> Self::Event {
///         // self.sub.next_message_pure();
///     }
/// }
///
/// // Use the input device
/// let c = MyController {
///     // ...
/// };
///
/// // Run device simultaneously with RMK
/// embassy_futures::join::join(
///     c.run_controller(),
///     run_rmk(
///         // ...
///     ),
/// )
/// .await;
/// ```
pub trait EventController {
    /// Type of the received events
    type Event;

    /// Process the received event
    async fn process_event(&mut self, event: Self::Event);

    /// Block waiting for next message
    async fn next_message(&mut self) -> Self::Event;

    /// Event loop
    async fn run_controller(&mut self) {
        loop {
            let event = self.next_message().await;
            self.process_event(event).await;
        }
    }
}

/// The trait for controllers.
///
/// This trait defines the interface for polling controllers in RMK.
///
/// # Example
/// ```rust
/// // Define a controller
/// struct MyController {
///     // ...
/// }
///
/// impl PollingController for MyController {
///     type INTERVAL: embassy_time::Duration = embassy_time::Duration::from_hz(60);
///     type Event = rmk::event::ControllerEvent;
///
///     async fn process_event(&mut self, event: Self::Event) {
///         // handle event
///     }
///     async fn update(&mut self) {
///         // update periodic
///     }
///     fn poll(&mut self) -> Self::Event {
///         // self.sub.try_next_message_pure();
///     }
/// }
///
/// // Use the input device
/// let c = MyController {
///     // ...
/// };
///
/// // Run device simultaneously with RMK
/// embassy_futures::join::join(
///     c.run_polling(),
///     run_rmk(
///         // ...
///     ),
/// )
/// .await;
/// ```
pub trait PollingController {
    /// Interval in which `poll` and `update` will be called
    const INTERVAL: embassy_time::Duration;

    /// Type of the received events
    type Event;

    /// Process the received event
    async fn process_event(&mut self, event: Self::Event);

    /// Update periodically
    async fn update(&mut self);

    /// Wait for next message without blocking
    fn poll(&mut self) -> Option<Self::Event>;

    /// Polling loop
    async fn run_polling(&mut self) {
        loop {
            if let Some(event) = self.poll() {
                self.process_event(event).await;
            }
            self.update().await;

            embassy_time::Timer::after(Self::INTERVAL).await;
        }
    }
}

/// Macro to bind controllers to event channels and run all of them.
///
/// This macro simplifies the creation of a task that reads events from a specified channel and
/// sens them to specified controllers. It allows for controllers to listen on events in a
/// concurrent manner.
///
/// # Arguments
///
/// * `dev`: A list of controllers grouped in parentheses.
/// * `channel`: The channel that events are received from.
///
/// # Example
/// ```rust
/// // Define your controllers, MyController should implement Controller trait
/// struct MyController;
///
/// let c1 = MyController{};
/// let c2 = MyController{};
/// // Bind devices to channels
/// let controller_future = run_controllers! {
///     (c1, c2) => rmk::channel::CONTROLLER_CHANNEL,
/// };
///
/// ```
#[macro_export]
macro_rules! run_controllers {
    ( $( ( $( $dev:ident ),* ) => $channel:expr),+ $(,)? ) => {{
        use $crate::controller::Controller;
        $crate::join_all!(
            $(
                $crate::join_all!(
                    $(
                        async {
                            let sub = unwrap!($channel.subscriber());
                            loop {
                                let e = sub.next_message_pure().await;
                                $dev.process_event(e).await;
                            }
                        }
                    ),*
                )
            ),+
        )
    }};
}
