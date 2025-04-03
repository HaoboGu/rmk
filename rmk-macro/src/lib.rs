mod behavior;
mod bind_interrupt;
mod ble;
mod chip_init;
mod comm;
mod config;
mod default_config;
mod entry;
mod feature;
mod flash;
mod gpio_config;
mod import;
mod input_device;
mod keyboard;
mod keyboard_config;
mod keycode_alias;
mod layout;
mod light;
mod matrix;
mod split;
#[rustfmt::skip]
mod usb_interrupt_map;

use crate::keyboard::parse_keyboard_mod;
use darling::{ast::NestedMeta, FromMeta};
use proc_macro::TokenStream;
use split::peripheral::parse_split_peripheral_mod;
use syn::parse_macro_input;
use usb_interrupt_map::get_usb_info;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ChipSeries {
    Stm32,
    Nrf52,
    #[default]
    Rp2040,
    Esp32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ChipModel {
    pub(crate) series: ChipSeries,
    pub(crate) chip: String,
    pub(crate) board: Option<String>,
}

impl ChipModel {
    pub(crate) fn has_usb(&self) -> bool {
        match self.series {
            ChipSeries::Stm32 => get_usb_info(&self.chip).is_some(),
            ChipSeries::Nrf52 => {
                if self.chip == "nrf52833" || self.chip == "nrf52840" || self.chip == "nrf52820" {
                    true
                } else {
                    false
                }
            }
            ChipSeries::Rp2040 => true,
            ChipSeries::Esp32 => {
                if self.chip == "esp32s3" || self.chip == "esp32s2" {
                    true
                } else {
                    false
                }
            }
        }
    }
}

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
