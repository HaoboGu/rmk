//! Controller module for RMK
//!
//! This module defines the `Controller` trait and several macros for running output device controllers.
//! The `Controller` trait provides the interface for individual output device controllers, and the macros facilitate their concurrent execution.

use crate::event::ControllerEvent;

/// The trait for controllers.
///
/// This trait defines the interface for controllers in RMK.
/// The `run_controllers` macro is required to run tasks associated with controllers concurrently.
///
/// # Example
/// ```rust
/// // Define an input device
/// struct MyController;
///
/// impl Controller for MyController {
///     async fn process_event(&mut self, event: ControllerEvent) {
///         // Controller implementation
///     }
/// }
///
/// // Use the input device
/// let c1 = MyController{};
/// let c2 = MyController{};
///
/// // Run all devices simultaneously with RMK
/// embassy_futures::join::join(
///     run_controllers!((c1, c2) => CONTROLLER_CHANNEL),
///     run_rmk(
///         // .. arguments
///     ),
/// )
/// .await;
/// ```
pub trait Controller {
    /// Process the received event
    async fn process_event(&mut self, event: ControllerEvent);
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
