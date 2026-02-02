mod behavior;
mod bind_interrupt;
mod ble;
mod chip_init;
mod comm;
mod controller;
mod entry;
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
mod split;

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

/// Macro for defining controller events.
///
/// This macro generates a static channel and implements the `ControllerEventTrait` trait.
///
/// # Examples
///
/// ```rust,ignore
/// #[controller_event(subs = 1)]
/// #[derive(Clone, Copy, Debug)]
/// pub struct BatteryEvent(pub u8);
///
/// #[controller_event(channel_size = 8, subs = 4)]
/// #[derive(Clone, Copy, Debug)]
/// pub struct KeyEvent {
///     pub keyboard_event: KeyboardEvent,
///     pub key_action: KeyAction,
/// }
/// ```
#[proc_macro_attribute]
pub fn controller_event(attr: TokenStream, item: TokenStream) -> TokenStream {
    controller::event::controller_event_impl(attr, item)
}

/// Macro for defining controllers that subscribe to multiple events.
///
/// This macro generates event routing infrastructure and implements the `Controller` trait.
///
/// # Examples
///
/// ```rust,ignore
/// #[controller(subscribe = [BatteryEvent, ChargingStateEvent])]
/// pub struct BatteryLedController<P> {
///     pin: OutputController<P>,
///     state: BatteryState,
/// }
///
/// impl<P> BatteryLedController<P> {
///     async fn on_battery_event(&mut self, event: BatteryEvent) { /* ... */ }
///     async fn on_charging_state_event(&mut self, event: ChargingStateEvent) { /* ... */ }
/// }
/// ```
#[proc_macro_attribute]
pub fn controller(attr: TokenStream, item: TokenStream) -> TokenStream {
    controller::controller_impl(attr, item)
}

/// Macro for defining input events.
///
/// This macro generates a static Channel and implements the `Event` trait.
///
/// # Parameters
///
/// - `channel_size`: Buffer size of the channel (default: 8)
///
/// # Examples
///
/// ```rust,ignore
/// #[input_event(channel_size = 8)]
/// #[derive(Clone, Copy, Debug)]
/// pub struct KeyEvent {
///     pub row: u8,
///     pub col: u8,
///     pub pressed: bool,
/// }
/// ```
#[proc_macro_attribute]
pub fn input_event(attr: TokenStream, item: TokenStream) -> TokenStream {
    input::event::input_event_impl(attr, item)
}

/// Macro for defining input processors that subscribe to multiple events.
///
/// This macro generates event routing infrastructure and implements the `InputProcessor` trait.
///
/// # Parameters
///
/// - `subscribe`: Array of event types to subscribe to (required)
///
/// # Examples
///
/// ```rust,ignore
/// #[input_processor(subscribe = [KeyEvent, ModifierEvent])]
/// pub struct MyInputProcessor {
///     // processor state
/// }
///
/// impl MyInputProcessor {
///     async fn on_key_event(&mut self, event: KeyEvent) {
///         // Handle key event
///     }
///
///     async fn on_modifier_event(&mut self, event: ModifierEvent) {
///         // Handle modifier event
///     }
/// }
/// ```
#[proc_macro_attribute]
pub fn input_processor(attr: TokenStream, item: TokenStream) -> TokenStream {
    input::processor::input_processor_impl(attr, item)
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
/// This macro generates a `publish()` method that dispatches events to the correct channel
/// based on the enum variant, and `From<EventType>` impls for convenient construction.
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
/// // Generated:
/// // - `publish()` method that dispatches to correct channel
/// // - `From<BatteryEvent>` and `From<PointingEvent>` impls
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
