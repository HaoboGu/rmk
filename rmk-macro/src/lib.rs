mod behavior;
mod bind_interrupt;
mod ble;
mod chip_init;
mod comm;
mod entry;
mod feature;
mod flash;
mod gpio_config;
mod import;
mod input_device;
mod keyboard;
mod keyboard_config;
mod layout;
mod light;
mod matrix;
mod split;

use darling::ast::NestedMeta;
use darling::FromMeta;
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
