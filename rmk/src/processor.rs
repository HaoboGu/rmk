//! Runtime traits for event consumers in RMK.
//!
//! In RMK's event system, `crate::event` defines how event types are published
//! and subscribed, while `Processor` defines how a task consumes those events.
//! `Processor` provides the core consume loop (`subscriber` -> `next_event` ->
//! `process`), and `PollingProcessor` extends it with timer-driven `update`
//! calls interleaved with event handling.

use embassy_futures::select::{Either, select};

use crate::event::EventSubscriber;
use crate::input_device::Runnable;

/// Unified trait for event processors.
///
/// This trait provides the interface for all event-driven processors in RMK.
/// Use the `#[processor]` macro to automatically implement this trait.
///
/// ```rust,ignore
/// use rmk_macro::processor;
///
/// // Single event subscription
/// #[processor(subscribe = [LedIndicatorEvent])]
/// struct MyProcessor { /* ... */ }
///
/// impl MyProcessor {
///     // You MUST implement on_{event_name}_event handler method
///     // for each event type in `subscribe = [..]`
///     async fn on_led_indicator_event(&mut self, event: LedIndicatorEvent) {
///         // handle event
///     }
/// }
///
/// // Multiple event subscription
/// #[processor(subscribe = [EventA, EventB])]
/// struct MyMultiProcessor { /* ... */ }
///
/// impl MyMultiProcessor {
///     async fn on_event_a_event(&mut self, event: EventA) { /* ... */ }
///     async fn on_event_b_event(&mut self, event: EventB) { /* ... */ }
/// }
/// ```
pub trait Processor: Runnable {
    /// Type of the received events.
    type Event;

    /// Create a new event subscriber.
    fn subscriber() -> impl EventSubscriber<Event = Self::Event>;

    /// Process the received event.
    async fn process(&mut self, event: Self::Event);

    /// Default processing loop that continuously receives and processes events.
    async fn process_loop(&mut self) -> ! {
        let mut sub = Self::subscriber();
        loop {
            let event = sub.next_event().await;
            self.process(event).await;
        }
    }
}

/// Trait for processors with periodic updates.
///
/// This trait extends `Processor` with periodic update capability.
/// The polling loop alternates between waiting for events and calling `update()`
/// at the specified interval.
///
/// ```rust,ignore
/// use rmk_macro::processor;
///
/// #[processor(subscribe = [BatteryStateEvent], poll_interval = 1000)]
/// struct BatteryLedProcessor {
///     led_on: bool,
/// }
///
/// impl BatteryLedProcessor {
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
/// ```
pub trait PollingProcessor: Processor {
    /// Returns the interval between `update` calls.
    fn interval(&self) -> embassy_time::Duration;

    /// Update periodically, will be called according to [`Self::interval()`]
    async fn update(&mut self);

    /// Polling loop that processes events and calls `update()` at the specified interval.
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
                Either::Second(event) => self.process(event).await,
            }
        }
    }
}
