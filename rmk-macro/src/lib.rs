mod bind_interrupt;
mod chip_init;
mod comm;
mod gpio_config;
mod import;
mod keyboard;
mod keyboard_config;
mod light;
mod matrix;
mod storage;

use crate::keyboard::parse_keyboard_mod;
use proc_macro::TokenStream;
use syn::parse_macro_input;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ChipSeries {
    Stm32,
    Nrf52,
    Rp2040,
    Esp32,
    Unsupported,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ChipModel {
    pub(crate) series: ChipSeries,
    pub(crate) chip: String,
}

#[proc_macro_attribute]
pub fn rmk_keyboard(attr: TokenStream, item: TokenStream) -> TokenStream {
    let item_mod = parse_macro_input!(item as syn::ItemMod);
    parse_keyboard_mod(attr, item_mod).into()
}
