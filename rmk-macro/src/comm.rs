//! Initialize communication boilerplate of RMK, including USB or BLE
//!

use darling::FromMeta;
use proc_macro2::TokenStream as TokenStream2;
use quote::{quote, ToTokens};
use syn::{ItemFn, ItemMod};

use crate::{keyboard::Overwritten, ChipModel, ChipSeries};

/// Default implementation of usb initialization
pub(crate) fn usb_config_default(chip: &ChipModel) -> TokenStream2 {
    match chip.series {
        ChipSeries::Stm32 => quote! {
            static EP_OUT_BUFFER: ::static_cell::StaticCell<[u8; 1024]> = ::static_cell::StaticCell::new();
            let mut usb_config = ::embassy_stm32::usb_otg::Config::default();
            usb_config.vbus_detection = false;
            let driver = ::embassy_stm32::usb_otg::Driver::new_fs(
                p.USB_OTG_HS,
                Irqs,
                p.PA12,
                p.PA11,
                &mut EP_OUT_BUFFER.init([0; 1024])[..],
                usb_config,
            );
        },
        ChipSeries::Nrf52 => {
            // For USB only, use
            quote! {
                let driver = :embassy_nrf::usb::Driver::new(p.USBD, Irqs, ::embassy_nrf::usb::vbus_detect::HardwareVbusDetect::new(Irqs));
            }
            // For USB + BLE, use:
            // quote! {
            // let software_vbus = ::rmk::ble::SOFTWARE_VBUS.get_or_init(|| ::embassy_nrf::usb::vbus_detect::SoftwareVbusDetect::new(true, false));
            // let driver = ::embassy_nrf::usb::Driver::new(p.USBD, Irqs, software_vbus);
            // }
        }
        ChipSeries::Rp2040 => quote! {
            let driver = ::embassy_rp::usb::Driver::new(p.USB, Irqs);
        },
        ChipSeries::Esp32 => quote! {},
        ChipSeries::Unsupported => quote! {}
    }
}

pub(crate) fn expand_usb_init(chip: &ChipModel, item_mod: &ItemMod) -> TokenStream2 {
    // If there is a function with `#[Overwritten(usb)]`, override the chip initialization
    if let Some((_, items)) = &item_mod.content {
        items
            .iter()
            .find_map(|item| {
                if let syn::Item::Fn(item_fn) = &item {
                    if item_fn.attrs.len() == 1 {
                        if let Ok(Overwritten::Usb) = Overwritten::from_meta(&item_fn.attrs[0].meta)
                        {
                            return Some(override_usb_init(item_fn));
                        }
                    }
                }
                None
            })
            .unwrap_or(usb_config_default(chip))
    } else {
        usb_config_default(chip)
    }
}

fn override_usb_init(item_fn: &ItemFn) -> TokenStream2 {
    let initialization = item_fn.block.to_token_stream();
    quote! {
        let driver = #initialization;
    }
}
