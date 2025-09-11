use darling::FromMeta;
use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, quote};
use rmk_config::{BoardConfig, ChipModel, ChipSeries, CommunicationConfig, KeyboardTomlConfig};
use syn::{ItemFn, ItemMod};

use crate::keyboard::Overwritten;

/// Expand chip initialization code
///
/// If `peripheral_id` is `None`, it means that the chip initialization is for the central.
/// Otherwise, the `peripheral_id` is the index of the peripheral.
pub(crate) fn expand_chip_init(
    keyboard_config: &KeyboardTomlConfig,
    peripheral_id: Option<usize>,
    item_mod: &ItemMod,
) -> TokenStream2 {
    // If there is a function with `#[Overwritten(usb)]`, override the chip initialization
    if let Some((_, items)) = &item_mod.content {
        items
            .iter()
            .find_map(|item| {
                if let syn::Item::Fn(item_fn) = &item {
                    if item_fn.attrs.len() == 1 {
                        match Overwritten::from_meta(&item_fn.attrs[0].meta) {
                            Ok(Overwritten::ChipConfig) => {
                                return Some(override_chip_config(
                                    &keyboard_config.get_chip_model().unwrap(),
                                    item_fn,
                                ));
                            }
                            Ok(Overwritten::ChipInit) => {
                                // Override the whole chip initialization
                                let stmts = &item_fn.block.stmts;
                                return Some(quote! { #(#stmts)* });
                            }
                            _ => (),
                        }
                    }
                }
                None
            })
            .unwrap_or(chip_init_default(keyboard_config, peripheral_id))
    } else {
        chip_init_default(keyboard_config, peripheral_id)
    }
}

// Default implementations of chip initialization
pub(crate) fn chip_init_default(keyboard_config: &KeyboardTomlConfig, peripheral_id: Option<usize>) -> TokenStream2 {
    let chip = keyboard_config.get_chip_model().unwrap();
    let communication = keyboard_config.get_communication_config().unwrap();
    match chip.series {
        ChipSeries::Stm32 => quote! {
                let config = ::embassy_stm32::Config::default();
                let mut p = ::embassy_stm32::init(config);
        },
        ChipSeries::Nrf52 => {
            let dcdc_config = if chip.chip == "nrf52840" {
                quote! {
                    config.dcdc.reg0_voltage = Some(::embassy_nrf::config::Reg0Voltage::_3V3);
                    config.dcdc.reg0 = true;
                    config.dcdc.reg1 = true;
                }
            } else if chip.chip == "nrf52833" {
                quote! {
                    config.dcdc.reg0_voltage = Some(::embassy_nrf::config::Reg0Voltage::_3V3);
                    config.dcdc.reg1 = true;
                }
            } else {
                quote! {}
            };
            let ble_addr = get_ble_addr(keyboard_config, peripheral_id);
            let ble_init = match &communication {
                CommunicationConfig::Ble(_) | CommunicationConfig::Both(_, _) => quote! {
                    // Initialize nrf-sdc and ble stack
                    let mpsl_p = ::nrf_sdc::mpsl::Peripherals::new(p.RTC0, p.TIMER0, p.TEMP, p.PPI_CH19, p.PPI_CH30, p.PPI_CH31);
                    let lfclk_cfg = ::nrf_sdc::mpsl::raw::mpsl_clock_lfclk_cfg_t {
                        source: ::nrf_sdc::mpsl::raw::MPSL_CLOCK_LF_SRC_RC as u8,
                        rc_ctiv: ::nrf_sdc::mpsl::raw::MPSL_RECOMMENDED_RC_CTIV as u8,
                        rc_temp_ctiv: ::nrf_sdc::mpsl::raw::MPSL_RECOMMENDED_RC_TEMP_CTIV as u8,
                        accuracy_ppm: ::nrf_sdc::mpsl::raw::MPSL_DEFAULT_CLOCK_ACCURACY_PPM as u16,
                        skip_wait_lfclk_started: ::nrf_sdc::mpsl::raw::MPSL_DEFAULT_SKIP_WAIT_LFCLK_STARTED != 0,
                    };
                    static MPSL: ::static_cell::StaticCell<::nrf_sdc::mpsl::MultiprotocolServiceLayer> = ::static_cell::StaticCell::new();
                    static SESSION_MEM: ::static_cell::StaticCell<::nrf_sdc::mpsl::SessionMem<1>> = ::static_cell::StaticCell::new();
                    let mpsl = MPSL.init(::defmt::unwrap!(::nrf_sdc::mpsl::MultiprotocolServiceLayer::with_timeslots(
                        mpsl_p,
                        Irqs,
                        lfclk_cfg,
                        SESSION_MEM.init(::nrf_sdc::mpsl::SessionMem::new())
                    )));
                    spawner.must_spawn(mpsl_task(&*mpsl));
                    let sdc_p = ::nrf_sdc::Peripherals::new(
                        p.PPI_CH17, p.PPI_CH18, p.PPI_CH20, p.PPI_CH21, p.PPI_CH22, p.PPI_CH23, p.PPI_CH24, p.PPI_CH25, p.PPI_CH26,
                        p.PPI_CH27, p.PPI_CH28, p.PPI_CH29,
                    );
                    let mut rng = ::embassy_nrf::rng::Rng::new(p.RNG, Irqs);
                    use rand_core::SeedableRng;
                    let mut rng_gen = ::rand_chacha::ChaCha12Rng::from_rng(&mut rng).unwrap();
                    let mut sdc_mem = ::nrf_sdc::Mem::<6144>::new(); // 6KB is enough for both central and peripheral
                    let sdc = ::defmt::unwrap!(build_sdc(sdc_p, &mut rng, &*mpsl, &mut sdc_mem));
                    let ble_addr = #ble_addr;
                    let mut host_resources = ::rmk::HostResources::new();
                    let stack = ::rmk::ble::build_ble_stack(sdc, ble_addr, &mut rng_gen, &mut host_resources).await;
                },
                _ => quote! {},
            };
            quote! {
                use embassy_nrf::interrupt::InterruptExt;
                let mut config = ::embassy_nrf::config::Config::default();
                #dcdc_config
                let p = ::embassy_nrf::init(config);
                #ble_init
            }
        }
        ChipSeries::Rp2040 => {
            let ble_addr = get_ble_addr(keyboard_config, peripheral_id);
            if communication.ble_enabled() {
                quote! {
                    let config = ::embassy_rp::config::Config::default();
                    let p = ::embassy_rp::init(config);

                    #[cfg(feature = "skip-cyw43-firmware")]
                    let (fw, clm, btfw) = (&[], &[], &[]);

                    #[cfg(not(feature = "skip-cyw43-firmware"))]
                    let (fw, clm, btfw) = {
                        // IMPORTANT
                        //
                        // Download and make sure these files from https://github.com/embassy-rs/embassy/tree/main/cyw43-firmware
                        // are available in `./examples/rp-pico-w`. (should be automatic)
                        //
                        // IMPORTANT
                        let fw = include_bytes!("../cyw43-firmware/43439A0.bin");
                        let clm = include_bytes!("../cyw43-firmware/43439A0_clm.bin");
                        let btfw = include_bytes!("../cyw43-firmware/43439A0_btfw.bin");
                        (fw, clm, btfw)
                    };

                    let pwr = ::embassy_rp::gpio::Output::new(p.PIN_23, ::embassy_rp::gpio::Level::Low);
                    let cs = ::embassy_rp::gpio::Output::new(p.PIN_25, ::embassy_rp::gpio::Level::High);
                    let mut pio = ::embassy_rp::pio::Pio::new(p.PIO0, Irqs);
                    let spi = ::cyw43_pio::PioSpi::new(
                        &mut pio.common,
                        pio.sm0,
                        ::cyw43_pio::DEFAULT_CLOCK_DIVIDER,
                        pio.irq0,
                        cs,
                        p.PIN_24,
                        p.PIN_29,
                        p.DMA_CH0,
                    );

                    static STATE: ::static_cell::StaticCell<::cyw43::State> = ::static_cell::StaticCell::new();
                    let state = STATE.init(::cyw43::State::new());
                    let (_net_device, bt_device, mut control, runner) = ::cyw43::new_with_bluetooth(state, pwr, spi, fw, btfw).await;
                    spawner.spawn(cyw43_task(runner)).unwrap();
                    control.init(clm).await;

                    let controller: ::bt_hci::controller::ExternalController<_, 10> = ::bt_hci::controller::ExternalController::new(bt_device);
                    let ble_addr = #ble_addr;
                    let mut host_resources = ::rmk::HostResources::new();
                    let mut rosc_rng = ::embassy_rp::clocks::RoscRng {};
                    use rand_core::SeedableRng;
                    let mut rng = ::rand_chacha::ChaCha12Rng::from_rng(&mut rosc_rng).unwrap();
                    let stack = ::rmk::ble::build_ble_stack(controller, ble_addr, &mut rng, &mut host_resources).await;
                }
            } else {
                quote! {
                    let config = ::embassy_rp::config::Config::default();
                    let p = ::embassy_rp::init(config);
                }
            }
        }
        ChipSeries::Esp32 => {
            let ble_addr = get_ble_addr(keyboard_config, peripheral_id);
            quote! {
                ::esp_println::logger::init_logger_from_env();
                let p = ::esp_hal::init(::esp_hal::Config::default().with_cpu_clock(::esp_hal::clock::CpuClock::max()));
                ::esp_alloc::heap_allocator!(size: 72 * 1024);
                let timg0 = ::esp_hal::timer::timg::TimerGroup::new(p.TIMG0);
                let mut rng = ::esp_hal::rng::Trng::new(p.RNG, p.ADC1);
                let init = ::esp_wifi::init(timg0.timer0, rng.rng.clone()).unwrap();
                let systimer = ::esp_hal::timer::systimer::SystemTimer::new(p.SYSTIMER);
                ::esp_hal_embassy::init(systimer.alarm0);
                let bluetooth = p.BT;
                let connector = ::esp_wifi::ble::controller::BleConnector::new(&init, bluetooth);
                let controller: ::bt_hci::controller::ExternalController<_, 64> = ::bt_hci::controller::ExternalController::new(connector);
                let ble_addr = #ble_addr;
                let mut host_resources = ::rmk::HostResources::new();
                let stack = ::rmk::ble::build_ble_stack(controller, ble_addr, &mut rng, &mut host_resources).await;
            }
        }
    }
}

fn override_chip_config(chip: &ChipModel, item_fn: &ItemFn) -> TokenStream2 {
    let initialization = item_fn.block.to_token_stream();
    let mut initialization_tokens = quote! {
        let config = #initialization;
    };
    match chip.series {
        ChipSeries::Stm32 => initialization_tokens.extend(quote! {
            let mut p = ::embassy_stm32::init(config);
        }),
        ChipSeries::Nrf52 => initialization_tokens.extend(quote! {
            let mut p = ::embassy_nrf::init(config);
        }),
        ChipSeries::Rp2040 => initialization_tokens.extend(quote! {
            let mut p = ::embassy_rp::init(config);
        }),
        ChipSeries::Esp32 => initialization_tokens.extend(quote! {
            let p = ::esp_hal::init(::esp_hal::Config::default().with_cpu_clock(::esp_hal::clock::CpuClock::max()));
        }),
    }

    initialization_tokens
}

fn get_ble_addr(keyboard_config: &KeyboardTomlConfig, peripheral_id: Option<usize>) -> TokenStream2 {
    let chip = keyboard_config.get_chip_model().unwrap();
    if chip.series == ChipSeries::Nrf52 {
        quote! {
            {
                let ficr = ::embassy_nrf::pac::FICR;
                let high = u64::from(ficr.deviceid(1).read());
                let addr = high << 32 | u64::from(ficr.deviceid(0).read());
                let addr = addr | 0x0000_c000_0000_0000;
                let ble_addr = addr.to_le_bytes()[..6].try_into().expect("Failed to read BLE address from FICR");
                ble_addr
            }
        }
    } else {
        // Check whether the address is set in the keyboard.toml, if not, use the default address
        let board = keyboard_config.get_board_config().unwrap();
        let addr = match board {
            BoardConfig::Split(split) => {
                match peripheral_id {
                    Some(id) => {
                        // Split peripheral
                        // The 4th byte is the peripheral index to make sure that the BLE address for each peripheral is different
                        let default_addr = [0x7e, 0xfe, 0x73, id as u8, 0x66, 0xe3];
                        split
                            .peripheral
                            .get(id)
                            .expect(&format!("There's no config for peripheral {}", id))
                            .ble_addr
                            .unwrap_or(default_addr)
                    }
                    None => {
                        // Split central
                        let default_addr = [0x18, 0xe2, 0x21, 0x80, 0xc0, 0xc7];
                        split.central.ble_addr.unwrap_or(default_addr)
                    }
                }
            }
            // TODO: allow user to set the BLE address for uni-body keyboards
            BoardConfig::UniBody(_uni_body) => [0x18, 0xe2, 0x21, 0x80, 0xc0, 0xc7],
        };
        quote! {
            [
                #(#addr),*
            ]
        }
    }
}
