mod behavior;
mod bind_interrupt;
mod ble;
mod chip_init;
mod comm;
mod controller;
mod entry;
mod event;
mod feature;
mod flash;
mod gpio_config;
mod import;
mod input;
mod input_device;
mod keyboard;
mod keyboard_config;
mod layout;
mod matrix;
mod processor;
mod runnable;
mod split;
mod utils;

use darling::FromMeta;
use darling::ast::NestedMeta;
use proc_macro::TokenStream;
use split::peripheral::parse_split_peripheral_mod;
use syn::parse_macro_input;

use crate::keyboard::parse_keyboard_mod;

#[proc_macro_attribute]
pub fn rmk_keyboard(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let item_mod = parse_macro_input!(item as syn::ItemMod);
    parse_keyboard_mod(item_mod).into()
}

#[proc_macro_attribute]
pub fn rmk_central(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let item_mod = parse_macro_input!(item as syn::ItemMod);
    parse_keyboard_mod(item_mod).into()
}

/// Attribute for `rmk_peripheral` macro
#[derive(Debug, FromMeta)]
struct PeripheralAttr {
    #[darling(default)]
    id: usize,
}

#[proc_macro_attribute]
pub fn rmk_peripheral(attr: TokenStream, item: TokenStream) -> TokenStream {
    let item_mod = parse_macro_input!(item as syn::ItemMod);
    let attr_args = match NestedMeta::parse_meta_list(attr.clone().into()) {
        Ok(v) => v,
        Err(e) => {
            return TokenStream::from(darling::Error::from(e).write_errors());
        }
    };

    let peripheral_id = match PeripheralAttr::from_list(&attr_args) {
        Ok(v) => v.id,
        Err(e) => {
            return TokenStream::from(e.write_errors());
        }
    };

    parse_split_peripheral_mod(peripheral_id, attr, item_mod).into()
}

/// Marker attribute for coordinating Runnable generation between macros.
/// Do not use directly.
#[doc(hidden)]
#[proc_macro_attribute]
pub fn runnable_generated(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item // Pass through unchanged
}

/// Derive macro for multi-event enums that generates automatic event dispatch.
///
/// This macro generates:
/// - `{EnumName}Publisher` struct implementing `AsyncEventPublisher` and `EventPublisher`
/// - `PublishableEvent` and `AsyncPublishableEvent` trait implementations
/// - `From<VariantType>` impls for each variant
///
/// Each variant is forwarded to its underlying event channel when published.
///
/// **Note**: You cannot subscribe to wrapper enums directly. Subscribe to the individual
/// concrete event types (e.g., `BatteryEvent`, `PointingEvent`) instead.
///
/// # Example
///
/// ```rust,ignore
/// #[derive(InputEvent)]
/// pub enum MultiSensorEvent {
///     Battery(BatteryEvent),
///     Pointing(PointingEvent),
/// }
///
/// // Publishing: events are routed to their concrete type channels
/// publish_event_async(MultiSensorEvent::Battery(event)).await;
/// ```
#[proc_macro_derive(InputEvent)]
pub fn input_event_derive(item: TokenStream) -> TokenStream {
    input::event_derive::input_event_derive_impl(item)
}

/// Macro for defining input devices that publish events.
///
/// This macro generates `InputDevice` and `Runnable` implementations for single-event devices.
/// For multi-event devices, use `#[derive(InputEvent)]` on a user-defined enum instead.
///
/// # Parameters
///
/// - `publish`: The event type to publish (single event type only)
///
/// # Example
///
/// ```rust,ignore
/// #[input_device(publish = BatteryEvent)]
/// pub struct BatteryReader { ... }
///
/// impl BatteryReader {
///     // User implements this inherent method
///     async fn read_battery_event(&mut self) -> BatteryEvent {
///         // Wait and return single event
///     }
/// }
/// ```
#[proc_macro_attribute]
pub fn input_device(attr: TokenStream, item: TokenStream) -> TokenStream {
    input::device::input_device_impl(attr, item)
}

/// Unified macro for defining events with static channels.
///
/// Generates `PublishableEvent`, `SubscribableEvent`, and `AsyncPublishableEvent` trait implementations.
///
/// # Parameters
///
/// - `channel_size`: Buffer size of the channel (default: 8 for mpsc, 1 for pubsub)
/// - `subs`: Max subscribers. If specified, a PubSub channel is used (default: 4)
/// - `pubs`: Max publishers. If specified, a PubSub channel is used (default: 1)
///
/// Channel type is automatically inferred: if `subs` or `pubs` is specified, a PubSub channel
/// (broadcast, multiple consumers) is used; otherwise an MPSC channel (single consumer) is used.
///
/// # Examples
///
/// ```rust,ignore
/// // MPSC channel (default) - single consumer
/// #[event(channel_size = 16)]
/// #[derive(Clone, Copy, Debug)]
/// pub struct KeyboardEvent { ... }
///
/// // PubSub channel - broadcast to multiple consumers
/// #[event(channel_size = 8, pubs = 1, subs = 4)]
/// #[derive(Clone, Copy, Debug)]
/// pub struct LayerChangeEvent { ... }
/// ```
#[proc_macro_attribute]
pub fn event(attr: TokenStream, item: TokenStream) -> TokenStream {
    event::event_impl(attr, item)
}

/// Unified macro for defining event processors.
///
/// Generates `Processor` trait implementation and event routing infrastructure.
/// Replaces both `#[input_processor]` and `#[controller]`.
///
/// # Parameters
///
/// - `subscribe`: Array of event types to subscribe to (required)
/// - `poll_interval`: Optional polling interval in milliseconds
///
/// # Examples
///
/// ```rust,ignore
/// // Event-driven processor
/// #[processor(subscribe = [KeyboardEvent, PointingEvent])]
/// pub struct MyProcessor { ... }
///
/// impl MyProcessor {
///     async fn on_keyboard_event(&mut self, event: KeyboardEvent) { /* ... */ }
///     async fn on_pointing_event(&mut self, event: PointingEvent) { /* ... */ }
/// }
///
/// // Polling processor
/// #[processor(subscribe = [BatteryStateEvent], poll_interval = 1000)]
/// pub struct MyPollingProcessor { ... }
///
/// impl MyPollingProcessor {
///     async fn on_battery_state_event(&mut self, event: BatteryStateEvent) { /* ... */ }
///     async fn poll(&mut self) { /* called every 1000ms */ }
/// }
/// ```
#[proc_macro_attribute]
pub fn processor(attr: TokenStream, item: TokenStream) -> TokenStream {
    processor::processor_impl(attr, item)
}
