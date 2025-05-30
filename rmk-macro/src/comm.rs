//! Initialize communication boilerplate of RMK, including USB or BLE
//!

use darling::FromMeta;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote, ToTokens};
use rmk_config::{ChipSeries, KeyboardTomlConfig};
use syn::{ItemFn, ItemMod};

use crate::keyboard::Overwritten;

pub(crate) fn expand_usb_init(keyboard_config: &KeyboardTomlConfig, item_mod: &ItemMod) -> TokenStream2 {
    // If there is a function with `#[Overwritten(usb)]`, override the chip initialization
    if let Some((_, items)) = &item_mod.content {
        items
            .iter()
            .find_map(|item| {
                if let syn::Item::Fn(item_fn) = &item {
                    if item_fn.attrs.len() == 1 {
                        if let Ok(Overwritten::Usb) = Overwritten::from_meta(&item_fn.attrs[0].meta) {
                            return Some(override_usb_init(item_fn));
                        }
                    }
                }
                None
            })
            .unwrap_or(usb_config_default(keyboard_config))
    } else {
        usb_config_default(keyboard_config)
    }
}

/// Default implementation of usb initialization
pub(crate) fn usb_config_default(keyboard_config: &KeyboardTomlConfig) -> TokenStream2 {
    if let Some(usb_info) = keyboard_config.get_communication_config().unwrap().get_usb_info() {
        let peripheral_name = format_ident!("{}", usb_info.peripheral_name);
        match keyboard_config.get_chip_model().unwrap().series {
            ChipSeries::Stm32 => {
                let dp = format_ident!("{}", usb_info.dp);
                let dm = format_ident!("{}", usb_info.dm);
                if usb_info.peripheral_name.contains("OTG") {
                    quote! {
                        static EP_OUT_BUFFER: ::static_cell::StaticCell<[u8; 1024]> = ::static_cell::StaticCell::new();
                        let mut usb_config = ::embassy_stm32::usb::Config::default();
                        usb_config.vbus_detection = false;
                        let driver = ::embassy_stm32::usb::Driver::new_fs(
                            p.#peripheral_name,
                            Irqs,
                            p.#dp,
                            p.#dm,
                            &mut EP_OUT_BUFFER.init([0; 1024])[..],
                            usb_config,
                        );
                    }
                } else {
                    quote! {
                        {
                            let _dp = ::embassy_stm32::gpio::Output::new(&mut p.#dp, ::embassy_stm32::gpio::Level::Low, ::embassy_stm32::gpio::Speed::Low);
                            ::embassy_time::Timer::after_millis(10).await;
                        }
                        // Usb driver
                        let driver = ::embassy_stm32::usb::Driver::new(p.#peripheral_name, Irqs, p.#dp, p.#dm);
                    }
                }
            }
            ChipSeries::Nrf52 => quote! {
                // use hardware vbus
                let driver = ::embassy_nrf::usb::Driver::new(p.#peripheral_name, Irqs, ::embassy_nrf::usb::vbus_detect::HardwareVbusDetect::new(Irqs));
            },
            ChipSeries::Rp2040 => quote! {
                let driver = ::embassy_rp::usb::Driver::new(p.#peripheral_name, Irqs);
            },
            ChipSeries::Esp32 => {
                let dp = format_ident!("{}", usb_info.dp);
                let dm = format_ident!("{}", usb_info.dm);
                quote! {
                    static mut EP_MEMORY: [u8; 1024] = [0; 1024];
                    let usb = ::esp_hal::otg_fs::Usb::new(p.#peripheral_name, p.#dp, p.#dm);
                    let usb_config = ::esp_hal::otg_fs::asynch::Config::default();
                    let driver = ::esp_hal::otg_fs::asynch::Driver::new(usb, unsafe { &mut *core::ptr::addr_of_mut!(EP_MEMORY) }, usb_config);
                }
            }
        }
    } else {
        quote! {}
    }
}

fn override_usb_init(item_fn: &ItemFn) -> TokenStream2 {
    let initialization = item_fn.block.to_token_stream();
    quote! {
        let driver = #initialization;
    }
}
