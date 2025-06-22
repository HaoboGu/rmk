//! Add `bind_interrupts!` boilerplate of RMK, including USB or BLE
//!

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use rmk_config::{BoardConfig, KeyboardTomlConfig};
use syn::ItemMod;

/// Expand `bind_interrupt!` stuffs, and other code before `main` function
pub(crate) fn expand_bind_interrupt(keyboard_config: &KeyboardTomlConfig, item_mod: &ItemMod) -> TokenStream2 {
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

/// Expand default `bind_interrupt!` for different chips and nrf-sdc config for nRF52
pub(crate) fn bind_interrupt_default(keyboard_config: &KeyboardTomlConfig) -> TokenStream2 {
    let chip = keyboard_config.get_chip_model().unwrap();
    let board = keyboard_config.get_board_config().unwrap();
    let communication = keyboard_config.get_communication_config().unwrap();
    match chip.series {
        rmk_config::ChipSeries::Stm32 => {
            // For stm32, bind only USB interrupt by default
            if let Some(usb_info) = communication.get_usb_info() {
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
        rmk_config::ChipSeries::Nrf52 => {
            // Usb and clock interrupt
            let usb_and_clock_interrupt = if let Some(usb_info) = communication.get_usb_info() {
                let interrupt_name = format_ident!("{}", usb_info.interrupt_name);
                let peripheral_name = format_ident!("{}", usb_info.peripheral_name);
                quote! {
                    #interrupt_name => ::embassy_nrf::usb::InterruptHandler<::embassy_nrf::peripherals::#peripheral_name>;
                    CLOCK_POWER => ::nrf_sdc::mpsl::ClockInterruptHandler, ::embassy_nrf::usb::vbus_detect::InterruptHandler;
                }
            } else {
                quote! { CLOCK_POWER => ::nrf_sdc::mpsl::ClockInterruptHandler; }
            };
            // nrf-sdc interrupt config
            let nrf_sdc_config = match board {
                BoardConfig::Split(_) => quote! {
                    ::nrf_sdc::Builder::new()?
                    .support_scan()?
                    .support_central()?
                    .support_adv()?
                    .support_peripheral()?
                    .support_dle_peripheral()?
                    .support_dle_central()?
                    .support_phy_update_central()?
                    .support_phy_update_peripheral()?
                    .support_le_2m_phy()?
                    .central_count(1)?
                    .peripheral_count(1)?
                    .buffer_cfg(L2CAP_MTU as u16, L2CAP_MTU as u16, L2CAP_TXQ, L2CAP_RXQ)?
                    .build(p, rng, mpsl, mem)
                },
                BoardConfig::UniBody(_) => quote! {
                    ::nrf_sdc::Builder::new()?
                    .support_adv()?
                    .support_peripheral()?
                    .support_dle_peripheral()?
                    .support_phy_update_peripheral()?
                    .support_le_2m_phy()?
                    .peripheral_count(1)?
                    .buffer_cfg(L2CAP_MTU as u16, L2CAP_MTU as u16, L2CAP_TXQ, L2CAP_RXQ)?
                    .build(p, rng, mpsl, mem)
                },
            };
            quote! {
                use ::embassy_nrf::bind_interrupts;
                bind_interrupts!(struct Irqs {
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
                const L2CAP_MTU: usize = 251;
                fn build_sdc<'d, const N: usize>(
                    p: ::nrf_sdc::Peripherals<'d>,
                    rng: &'d mut ::embassy_nrf::rng::Rng<::embassy_nrf::peripherals::RNG, ::embassy_nrf::mode::Async>,
                    mpsl: &'d ::nrf_sdc::mpsl::MultiprotocolServiceLayer,
                    mem: &'d mut ::nrf_sdc::Mem<N>,
                ) -> Result<::nrf_sdc::SoftdeviceController<'d>, ::nrf_sdc::Error> {
                    #nrf_sdc_config
                }
            }
        }
        rmk_config::ChipSeries::Rp2040 => {
            let usb_info = communication.get_usb_info().expect("no usb info for the chip");
            let interrupt_name = format_ident!("{}", usb_info.interrupt_name);
            let peripheral_name = format_ident!("{}", usb_info.peripheral_name);
            // For Pico W, enabled PIO0_IRQ_0 interrupt
            let (pio0_irq_0, ble_task) = if communication.ble_enabled() {
                (
                    quote! {
                        PIO0_IRQ_0 => ::embassy_rp::pio::InterruptHandler<::embassy_rp::peripherals::PIO0>;
                    },
                    quote! {
                        #[::embassy_executor::task]
                        async fn cyw43_task(runner: ::cyw43::Runner<'static, ::embassy_rp::gpio::Output<'static>, ::cyw43_pio::PioSpi<'static, ::embassy_rp::peripherals::PIO0, 0, ::embassy_rp::peripherals::DMA_CH0>>) -> ! {
                            runner.run().await
                        }
                    },
                )
            } else {
                (quote! {}, quote! {})
            };
            quote! {
                use ::embassy_rp::bind_interrupts;
                bind_interrupts!(struct Irqs {
                    #interrupt_name => ::embassy_rp::usb::InterruptHandler<::embassy_rp::peripherals::#peripheral_name>;
                    #pio0_irq_0
                });
                #ble_task
            }
        }
        rmk_config::ChipSeries::Esp32 => quote! {},
    }
}
