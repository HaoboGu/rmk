//! Add `bind_interrupts!` boilerplate of RMK, including USB or BLE
//!

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::ItemMod;

use crate::config::BleConfig;
use crate::keyboard_config::KeyboardConfig;

// Expand `bind_interrupt!` stuffs
pub(crate) fn expand_bind_interrupt(
    keyboard_config: &KeyboardConfig,
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
            .unwrap_or(bind_interrupt_default(keyboard_config))
    } else {
        bind_interrupt_default(keyboard_config)
    }
}

pub(crate) fn bind_interrupt_default(keyboard_config: &KeyboardConfig) -> TokenStream2 {
    if let Some(usb_info) = keyboard_config.communication.get_usb_info() {
        let interrupt_name = format_ident!("{}", usb_info.interrupt_name);
        let peripheral_name = format_ident!("{}", usb_info.peripheral_name);
        match keyboard_config.chip.series {
            crate::ChipSeries::Stm32 => {
                if !keyboard_config.chip.has_usb() {
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
                    adc_divider_measured: _,
                    adc_divider_total: _,
                }) = keyboard_config.communication.get_ble_config()
                {
                    Some(quote! {
                        SAADC => ::embassy_nrf::saadc::InterruptHandler;
                    })
                } else {
                    None
                };
                let interrupt_binding = if keyboard_config.chip.has_usb() {
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
        }
    } else {
        quote! {}
    }
}
