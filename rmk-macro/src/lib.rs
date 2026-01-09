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
/// ```ignore
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
/// ```ignore
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
    controller::controller::controller_impl(attr, item)
}
