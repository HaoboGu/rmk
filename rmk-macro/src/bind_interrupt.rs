//! Add `bind_interrupts!` boilerplate of RMK, including USB or BLE
//!

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::ItemMod;

use crate::config::BleConfig;
use crate::keyboard_config::{BoardConfig, KeyboardConfig};

// Expand `bind_interrupt!` stuffs
pub(crate) fn expand_bind_interrupt(keyboard_config: &KeyboardConfig, item_mod: &ItemMod) -> TokenStream2 {
    // If there is a function with `#[Overwritten(bind_interrupt)]`, override it
    if let Some((_, items)) = &item_mod.content {
        items
            .iter()
            .find_map(|item| {
                if let syn::Item::Fn(item_fn) = &item {
                    if item_fn.attrs.len() == 1 {
                        if let Some(i) = item_fn.attrs[0].meta.path().get_ident() {
                            if i == "bind_interrupt" {
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
    match keyboard_config.chip.series {
        crate::ChipSeries::Stm32 => {
            if let Some(usb_info) = keyboard_config.communication.get_usb_info() {
                let interrupt_name = format_ident!("{}", usb_info.interrupt_name);
                let peripheral_name = format_ident!("{}", usb_info.peripheral_name);
                quote! {
                    use ::embassy_stm32::bind_interrupts;
                    bind_interrupts!(struct Irqs {
                        #interrupt_name => ::embassy_stm32::usb::InterruptHandler<::embassy_stm32::peripherals::#peripheral_name>;
                    });
                }
            } else {
                quote! {}
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
                quote! { SAADC => ::embassy_nrf::saadc::InterruptHandler; }
            } else {
                quote! {}
            };
            let usb_and_clock_interrupt = if let Some(usb_info) = keyboard_config.communication.get_usb_info() {
                let interrupt_name = format_ident!("{}", usb_info.interrupt_name);
                let peripheral_name = format_ident!("{}", usb_info.peripheral_name);
                quote! {
                    #interrupt_name => ::embassy_nrf::usb::InterruptHandler<::embassy_nrf::peripherals::#peripheral_name>;
                    CLOCK_POWER => ::nrf_sdc::mpsl::ClockInterruptHandler, ::embassy_nrf::usb::vbus_detect::InterruptHandler;
                }
            } else {
                quote! { CLOCK_POWER => ::nrf_sdc::mpsl::ClockInterruptHandler; }
            };
            let nrf_sdc_config = match keyboard_config.board {
                BoardConfig::Split(_) => quote! {
                    ::nrf_sdc::Builder::new()?
                    .support_scan()?
                    .support_central()?
                    .support_adv()?
                    .support_peripheral()?
                    .central_count(1)?
                    .peripheral_count(1)?
                    .buffer_cfg(L2CAP_MTU as u8, L2CAP_MTU as u8, L2CAP_TXQ, L2CAP_RXQ)?
                    .build(p, rng, mpsl, mem)
                },
                BoardConfig::UniBody(_) => quote! {
                    ::nrf_sdc::Builder::new()?
                    .support_adv()?
                    .support_peripheral()?
                    .peripheral_count(1)?
                    .buffer_cfg(L2CAP_MTU as u8, L2CAP_MTU as u8, L2CAP_TXQ, L2CAP_RXQ)?
                    .build(p, rng, mpsl, mem)
                },
            };
            quote! {
                use ::embassy_nrf::bind_interrupts;
                bind_interrupts!(struct Irqs {
                    #saadc_interrupt
                    #usb_and_clock_interrupt
                    RNG => ::embassy_nrf::rng::InterruptHandler<::embassy_nrf::peripherals::RNG>;
                    EGU0_SWI0 => ::nrf_sdc::mpsl::LowPrioInterruptHandler;
                    RADIO => ::nrf_sdc::mpsl::HighPrioInterruptHandler;
                    TIMER0 => ::nrf_sdc::mpsl::HighPrioInterruptHandler;
                    RTC0 => ::nrf_sdc::mpsl::HighPrioInterruptHandler;
                });

                #[::embassy_executor::task]
                async fn mpsl_task(mpsl: &'static ::nrf_sdc::mpsl::MultiprotocolServiceLayer<'static>) -> ! {
                    mpsl.run().await
                }
                /// How many outgoing L2CAP buffers per link
                const L2CAP_TXQ: u8 = 3;

                /// How many incoming L2CAP buffers per link
                const L2CAP_RXQ: u8 = 3;

                /// Size of L2CAP packets
                const L2CAP_MTU: usize = 72;
                fn build_sdc<'d, const N: usize>(
                    p: ::nrf_sdc::Peripherals<'d>,
                    rng: &'d mut ::embassy_nrf::rng::Rng<::embassy_nrf::peripherals::RNG>,
                    mpsl: &'d ::nrf_sdc::mpsl::MultiprotocolServiceLayer,
                    mem: &'d mut ::nrf_sdc::Mem<N>,
                ) -> Result<::nrf_sdc::SoftdeviceController<'d>, ::nrf_sdc::Error> {
                    #nrf_sdc_config
                }
            }
        }
        crate::ChipSeries::Rp2040 => {
            let usb_info = keyboard_config.communication.get_usb_info().unwrap();
            let interrupt_name = format_ident!("{}", usb_info.interrupt_name);
            let peripheral_name = format_ident!("{}", usb_info.peripheral_name);
            quote! {
                use ::embassy_rp::bind_interrupts;
                bind_interrupts!(struct Irqs {
                    #interrupt_name => ::embassy_rp::usb::InterruptHandler<::embassy_rp::peripherals::#peripheral_name>;
                });
            }
        }
        crate::ChipSeries::Esp32 => quote! {},
    }
}
