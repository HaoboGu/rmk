mod bind_interrupt;
mod ble;
mod chip_init;
mod comm;
mod entry;
mod flash;
mod gpio_config;
mod import;
mod keyboard;
mod keyboard_config;
mod layout;
mod light;
mod matrix;
#[rustfmt::skip]
mod usb_interrupt_map;

use crate::keyboard::parse_keyboard_mod;
use proc_macro::TokenStream;
use syn::parse_macro_input;
use usb_interrupt_map::get_usb_info;

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
            ChipSeries::Unsupported => false,
        }
    }
}

#[proc_macro_attribute]
pub fn rmk_keyboard(attr: TokenStream, item: TokenStream) -> TokenStream {
    let item_mod = parse_macro_input!(item as syn::ItemMod);
    parse_keyboard_mod(attr, item_mod).into()
}
