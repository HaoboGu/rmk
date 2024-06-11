//! Add `bind_interrupts!` boilerplate of RMK, including USB or BLE
//!

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use rmk_config::toml_config::{BleConfig, KeyboardTomlConfig};
use syn::ItemMod;

use crate::{usb_interrupt_map::UsbInfo, ChipModel};

// Expand `bind_interrupt!` stuffs
pub(crate) fn expand_bind_interrupt(
    chip: &ChipModel,
    usb_info: &UsbInfo,
    toml_config: &KeyboardTomlConfig,
    item_mod: &ItemMod,
) -> TokenStream2 {
    // If there is a function with `#[Overwritten(bind_interrupt)]`, override it
    if let Some((_, items)) = &item_mod.content {
        items
            .iter()
            .find_map(|item| {
                if let syn::Item::Fn(item_fn) = &item {
                    if item_fn.attrs.len() == 1 {
                        if let Some(i) = &item_fn.attrs[0].meta.path().get_ident() {
                            if i.to_string() == "bind_interrupt" {
                                let content = &item_fn.block.stmts;
                                return Some(quote! {
                                    #(#content)*
                                });
                            }
                        }
                    }
                }
                None
            })
            .unwrap_or(bind_interrupt_default(chip, usb_info, toml_config))
    } else {
        bind_interrupt_default(chip, usb_info, toml_config)
    }
}

pub(crate) fn bind_interrupt_default(
    chip: &ChipModel,
    usb_info: &UsbInfo,
    toml_config: &KeyboardTomlConfig,
) -> TokenStream2 {
    let interrupt_name = format_ident!("{}", usb_info.interrupt_name);
    let peripheral_name = format_ident!("{}", usb_info.peripheral_name);
    match chip.series {
        crate::ChipSeries::Stm32 => {
            if !chip.has_usb() {
                return quote! {};
            }
            let usb_mod_name = if usb_info.peripheral_name.contains("OTG") {
                format_ident!("{}", "usb_otg")
            } else {
                format_ident!("{}", "usb")
            };
            quote! {
                use ::embassy_stm32::bind_interrupts;
                bind_interrupts!(struct Irqs {
                    #interrupt_name => ::embassy_stm32::#usb_mod_name::InterruptHandler<::embassy_stm32::peripherals::#peripheral_name>;
                });
            }
        }
        crate::ChipSeries::Nrf52 => {
            let saadc_interrupt = if let Some(BleConfig {
                enabled: true,
                battery_adc_pin: Some(_adc_pin),
                charge_state: _,
                charge_led: _,
            }) = &toml_config.ble
            {
                Some(quote! {
                    SAADC => ::embassy_nrf::saadc::InterruptHandler;
                })
            } else {
                None
            };
            let interrupt_binding = if chip.has_usb() {
                quote! {
                    #interrupt_name => ::embassy_nrf::usb::InterruptHandler<::embassy_nrf::peripherals::#peripheral_name>;
                    #saadc_interrupt
                    POWER_CLOCK => ::embassy_nrf::usb::vbus_detect::InterruptHandler;
                }
            } else {
                quote! { #saadc_interrupt }
            };
            quote! {
                use ::embassy_nrf::bind_interrupts;
                bind_interrupts!(struct Irqs {
                #interrupt_binding
                });
            }
        }
        crate::ChipSeries::Rp2040 => {
            quote! {
                use ::embassy_rp::bind_interrupts;
                bind_interrupts!(struct Irqs {
                    #interrupt_name => ::embassy_rp::usb::InterruptHandler<::embassy_rp::peripherals::#peripheral_name>;
                });
            }
        }
        crate::ChipSeries::Esp32 => quote! {},
        crate::ChipSeries::Unsupported => quote! {},
    }
}
