use core::panic;

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::ItemMod;

use crate::{
    behavior::expand_behavior_config,
    bind_interrupt::expand_bind_interrupt,
    ble::expand_ble_config,
    chip_init::expand_chip_init,
    comm::expand_usb_init,
    config::{MatrixType, SerialConfig, SplitConfig},
    feature::{get_rmk_features, is_feature_enabled},
    flash::expand_flash_init,
    import::expand_imports,
    keyboard::gen_imports,
    keyboard_config::{read_keyboard_toml_config, BoardConfig, KeyboardConfig},
    light::expand_light_config,
    matrix::{expand_matrix_direct_pins, expand_matrix_input_output_pins},
    ChipModel, ChipSeries,
};

/// Parse split central mod and generate a valid RMK main function with all needed code
pub(crate) fn parse_split_central_mod(
    _attr: proc_macro::TokenStream,
    item_mod: ItemMod,
) -> TokenStream2 {
    let rmk_features = get_rmk_features();
    if !is_feature_enabled(&rmk_features, "split") {
        return quote! {
            compile_error!("\"split\" feature of RMK should be enabled");
        };
    }

    let async_matrix = is_feature_enabled(&rmk_features, "async_matrix");

    let toml_config = match read_keyboard_toml_config() {
        Ok(c) => c,
        Err(e) => return e,
    };

    let keyboard_config = match KeyboardConfig::new(toml_config) {
        Ok(c) => c,
        Err(e) => return e,
    };

    let imports = gen_imports(&keyboard_config);

    let main_function = expand_split_central(&keyboard_config, item_mod, async_matrix);

    quote! {
        #imports

        #main_function
    }
}

fn expand_split_central(
    keyboard_config: &KeyboardConfig,
    item_mod: ItemMod,
    async_matrix: bool,
) -> TokenStream2 {
    // Check whether keyboard.toml contains split section
    let split_config = if let BoardConfig::Split(split_config) = &keyboard_config.board {
        split_config
    } else {
        return quote! { compile_error!("No `split` field in `keyboard.toml`"); }.into();
    };

    // Expand components of main function
    let imports = expand_imports(&item_mod);
    let bind_interrupt = expand_bind_interrupt(keyboard_config, &item_mod);
    let chip_init = expand_chip_init(keyboard_config, &item_mod);
    let usb_init = expand_usb_init(keyboard_config, &item_mod);
    let flash_init = expand_flash_init(keyboard_config);
    let light_config = expand_light_config(keyboard_config);
    let behavior_config = expand_behavior_config(keyboard_config);

    let mut matrix_config = proc_macro2::TokenStream::new();
    match &split_config.central.matrix.matrix_type {
        MatrixType::normal => {
            matrix_config.extend(expand_matrix_input_output_pins(
                &keyboard_config.chip,
                split_config
                    .central
                    .matrix
                    .input_pins
                    .clone()
                    .expect("split.central.matrix.input_pins is required"),
                split_config
                    .central
                    .matrix
                    .output_pins
                    .clone()
                    .expect("split.central.matrix.output_pins is required"),
                async_matrix,
            ));
        }
        MatrixType::direct_pin => {
            matrix_config.extend(expand_matrix_direct_pins(
                &keyboard_config.chip,
                split_config
                    .central
                    .matrix
                    .direct_pins
                    .clone()
                    .expect("split.central.matrix.direct_pins is required"),
                async_matrix,
                split_config.central.matrix.direct_pin_low_active,
            ));
            // `generic_arg_infer` is a nightly feature. Const arguments cannot yet be inferred with `_` in stable now.
            // So we need to declaring them in advance.
            let rows = keyboard_config.layout.rows as usize;
            let cols = keyboard_config.layout.cols as usize;
            let size = keyboard_config.layout.rows as usize * keyboard_config.layout.cols as usize;
            let layers = keyboard_config.layout.layers as usize;
            let low_active = split_config.central.matrix.direct_pin_low_active;
            matrix_config.extend(quote! {
                pub(crate) const ROW: usize = #rows;
                pub(crate) const COL: usize = #cols;
                pub(crate) const SIZE: usize = #size;
                pub(crate) const LAYER_NUM: usize = #layers;
                let low_active = #low_active;
            });
        }
    }

    let split_communication_config =
        expand_split_communication_config(&keyboard_config.chip, split_config);
    let run_rmk = expand_split_central_entry(keyboard_config, split_config);
    let (ble_config, set_ble_config) = expand_ble_config(keyboard_config);

    let main_function_sig = if keyboard_config.chip.series == ChipSeries::Esp32 {
        quote! {
            use ::esp_idf_svc::hal::gpio::*;
            use esp_println as _;
            fn main()
        }
    } else {
        quote! {
            #[::embassy_executor::main]
            async fn main(spawner: ::embassy_executor::Spawner)
        }
    };

    quote! {
        #imports

        #bind_interrupt

        #main_function_sig {
            ::defmt::info!("RMK start!");
            // Initialize peripherals as `p`
            #chip_init

            // Initialize usb driver as `driver`
            #usb_init

            // Initialize flash driver as `flash` and storage config as `storage_config`
            #flash_init

            // Initialize light config as `light_config`
            #light_config

            // Initialize behavior config config as `behavior_config`
            #behavior_config

            // Initialize matrix config as `(input_pins, output_pins)`
            #matrix_config

            // Initialize split central ble config
            #ble_config

            // Set all keyboard config
            let rmk_config = ::rmk::config::RmkConfig {
                usb_config: KEYBOARD_USB_CONFIG,
                vial_config: VIAL_CONFIG,
                storage_config,
                behavior_config,
                #set_ble_config
                ..Default::default()
            };

            let controller_config = ::rmk::config::ControllerConfig {
                light_config,
            };

            let keyboard_config = ::rmk::config::KeyboardConfig {
                rmk_config,
                controller_config,
                ..Default::default()
            };

            #split_communication_config

            // Start
            #run_rmk
        }
    }
}

fn expand_split_central_entry(
    keyboard_config: &KeyboardConfig,
    split_config: &SplitConfig,
) -> TokenStream2 {
    let central_row = split_config.central.rows;
    let central_col = split_config.central.cols;
    let central_row_offset = split_config.central.row_offset;
    let central_col_offset = split_config.central.col_offset;
    match keyboard_config.chip.series {
        ChipSeries::Stm32 => {
            let usb_info = keyboard_config
                .communication
                .get_usb_info()
                .expect("get_usb_info returned None");
            let usb_name = format_ident!("{}", usb_info.peripheral_name);
            let low_active = split_config.central.matrix.direct_pin_low_active;
            let central_task = match split_config.central.matrix.matrix_type {
                MatrixType::normal => quote! {
                    ::rmk::split::central::run_rmk_split_central::<
                        ::embassy_stm32::gpio::Input<'_>,
                        ::embassy_stm32::gpio::Output<'_>,
                        ::embassy_stm32::usb::Driver<'_, ::embassy_stm32::peripherals::#usb_name>,
                        ::embassy_stm32::flash::Flash<'_, ::embassy_stm32::flash::Blocking>,
                        ROW,
                        COL,
                        #central_row,
                        #central_col,
                        #central_row_offset,
                        #central_col_offset,
                        NUM_LAYER,
                    >(input_pins, output_pins, driver, flash, &mut get_default_keymap(), keyboard_config, spawner)
                },
                MatrixType::direct_pin => quote! {
                    ::rmk::split::central::run_rmk_split_central_direct_pin::<
                        ::embassy_stm32::gpio::Input<'_>,
                        ::embassy_stm32::gpio::Output<'_>,
                        ::embassy_stm32::usb::Driver<'_, ::embassy_stm32::peripherals::#usb_name>,
                        ::embassy_stm32::flash::Flash<'_, ::embassy_stm32::flash::Blocking>,
                        ROW,
                        COL,
                        #central_row,
                        #central_col,
                        #central_row_offset,
                        #central_col_offset,
                        NUM_LAYER,
                        SIZE,
                    >(direct_pins, driver, flash, &mut get_default_keymap(), keyboard_config, #low_active, spawner)
                },
            };
            let mut tasks = vec![central_task];
            let central_serials = split_config
                .central
                .serial
                .clone()
                .expect("No serial defined for central");
            split_config
                .peripheral
                .iter()
                .enumerate()
                .for_each(|(idx, p)| {
                    let row = p.rows;
                    let col = p.cols;
                    let row_offset = p.row_offset;
                    let col_offset = p.col_offset;
                    let uart_instance = format_ident!("{}", central_serials.get(idx).expect("No or not enough serial defined for peripheral in central").instance.to_lowercase());

                    tasks.push(quote! {
                        ::rmk::split::central::run_peripheral_manager::<#row, #col, #row_offset, #col_offset, _>(
                            #idx,
                            #uart_instance,
                        )
                    });
                });

            join_all_tasks(tasks)
        }
        ChipSeries::Nrf52 => {
            let central_addr = split_config
                .central
                .ble_addr
                .expect("No ble_addr defined for central");
            let low_active = split_config.central.matrix.direct_pin_low_active;
            let central_task = match split_config.central.matrix.matrix_type {
                MatrixType::normal => quote! {
                    ::rmk::split::central::run_rmk_split_central::<
                        ::embassy_nrf::gpio::Input<'_>,
                        ::embassy_nrf::gpio::Output<'_>,
                        ::embassy_nrf::usb::Driver<'_, ::embassy_nrf::peripherals::USBD, &::embassy_nrf::usb::vbus_detect::SoftwareVbusDetect>,
                        ROW,
                        COL,
                        #central_row,
                        #central_col,
                        #central_row_offset,
                        #central_col_offset,
                        NUM_LAYER,
                    >(input_pins, output_pins, driver, &mut get_default_keymap(), keyboard_config, [#(#central_addr), *], spawner)
                },
                MatrixType::direct_pin => quote! {
                    ::rmk::split::central::run_rmk_split_central_direct_pin::<
                        ::embassy_nrf::gpio::Input<'_>,
                        ::embassy_nrf::gpio::Output<'_>,
                        ::embassy_nrf::usb::Driver<'_, ::embassy_nrf::peripherals::USBD, &::embassy_nrf::usb::vbus_detect::SoftwareVbusDetect>,
                        ROW,
                        COL,
                        #central_row,
                        #central_col,
                        #central_row_offset,
                        #central_col_offset,
                        NUM_LAYER,
                        SIZE,
                    >(direct_pins, driver, &mut get_default_keymap(), keyboard_config, #low_active, [#(#central_addr), *], spawner)
                },
            };
            let mut tasks = vec![central_task];
            split_config.peripheral.iter().enumerate().for_each(|(idx, p)| {
                let row = p.rows ;
                let col = p.cols ;
                let row_offset = p.row_offset ;
                let col_offset = p.col_offset ;
                let peripheral_ble_addr = p.ble_addr.expect("No ble_addr defined for peripheral");
                tasks.push(quote! {
                    ::rmk::split::central::run_peripheral_manager::<#row, #col, #row_offset, #col_offset>(
                        #idx,
                        [#(#peripheral_ble_addr), *],
                    )
                });
            });
            join_all_tasks(tasks)
        }
        ChipSeries::Rp2040 => {
            let low_active = split_config.central.matrix.direct_pin_low_active;

            let central_task = match split_config.central.matrix.matrix_type {
                MatrixType::normal => quote! {
                ::rmk::split::central::run_rmk_split_central::<
                    ::embassy_rp::gpio::Input<'_>,
                    ::embassy_rp::gpio::Output<'_>,
                    ::embassy_rp::usb::Driver<'_, ::embassy_rp::peripherals::USB>,
                    ::embassy_rp::flash::Flash<::embassy_rp::peripherals::FLASH, ::embassy_rp::flash::Async, FLASH_SIZE>,
                    ROW,
                    COL,
                    #central_row,
                    #central_col,
                    #central_row_offset,
                    #central_col_offset,
                    NUM_LAYER,
                >(input_pins, output_pins, driver, flash, &mut get_default_keymap(), keyboard_config, spawner)
                },
                MatrixType::direct_pin => quote! {
                    ::rmk::split::central::run_rmk_split_central_direct_pin::<
                        ::embassy_rp::gpio::Input<'_>,
                        ::embassy_rp::gpio::Output<'_>,
                        ::embassy_rp::usb::Driver<'_, ::embassy_rp::peripherals::USB>,
                        ::embassy_rp::flash::Flash<::embassy_rp::peripherals::FLASH, ::embassy_rp::flash::Async, FLASH_SIZE>,
                        ROW,
                        COL,
                        #central_row,
                        #central_col,
                        #central_row_offset,
                        #central_col_offset,
                        NUM_LAYER,
                        SIZE,
                    >(direct_pins, driver, flash, &mut get_default_keymap(), keyboard_config, #low_active, spawner)
                },
            };
            let mut tasks = vec![central_task];
            let central_serials = split_config
                .central
                .serial
                .clone()
                .expect("No serial defined for central");
            split_config
                .peripheral
                .iter()
                .enumerate()
                .for_each(|(idx, p)| {
                    let row = p.rows;
                    let col = p.cols;
                    let row_offset = p.row_offset;
                    let col_offset = p.col_offset;
                    let uart_instance = format_ident!("{}", central_serials.get(idx).expect("No or not enough serial defined for peripheral in central").instance.to_lowercase());

                    tasks.push(quote! {
                        ::rmk::split::central::run_peripheral_manager::<#row, #col, #row_offset, #col_offset, _>(
                            #idx,
                            #uart_instance,
                        )
                    });
                });

            join_all_tasks(tasks)
        }
        ChipSeries::Esp32 => panic!("Split for esp32 isn't implemented yet"),
    }
}

fn expand_split_communication_config(chip: &ChipModel, split_config: &SplitConfig) -> TokenStream2 {
    match &split_config.connection[..] {
        "ble" => {
            // We need to create addrs for BLE
            let central_addr = split_config
                .central
                .ble_addr
                .expect("central.ble_addr is required");
            let mut peripheral_addrs = proc_macro2::TokenStream::new();
            split_config
                .peripheral
                .iter()
                .map(|p| p.ble_addr.unwrap())
                .enumerate()
                .for_each(|(idx, addr)| {
                    let addr_name = format_ident!("peripheral_addr{}", idx);
                    peripheral_addrs.extend(quote! {let #addr_name = [#(#addr), *];})
                });

            quote! {
                let central_addr = [#(#central_addr), *];
                #peripheral_addrs
            }
        }
        "serial" => {
            // We need to initialize serial instance for serial
            let serial_config: Vec<SerialConfig> = split_config
                .central
                .serial
                .clone()
                .expect("central.serial is required");
            expand_serial_init(chip, serial_config)
        }
        _ => panic!("Invalid connection type for split"),
    }
}

pub(crate) fn expand_serial_init(chip: &ChipModel, serial: Vec<SerialConfig>) -> TokenStream2 {
    let mut uart_initializers = proc_macro2::TokenStream::new();
    serial.iter().enumerate().for_each(|(idx, s)| {
        let tx_buf_static = format_ident!("TX_BUF{}", idx);
        let rx_buf_static = format_ident!("RX_BUF{}", idx);
        let tx_buf_name = format_ident!("tx_buf{}", idx);
        let rx_buf_name = format_ident!("rx_buf{}", idx);
        let uart_buf_init = quote! {
            static #tx_buf_static: ::static_cell::StaticCell<[u8; ::rmk::split::SPLIT_MESSAGE_MAX_SIZE]> = ::static_cell::StaticCell::new();
            let #tx_buf_name = &mut #tx_buf_static.init([0_u8; ::rmk::split::SPLIT_MESSAGE_MAX_SIZE])[..];
            static #rx_buf_static: ::static_cell::StaticCell<[u8; ::rmk::split::SPLIT_MESSAGE_MAX_SIZE]> = ::static_cell::StaticCell::new();
            let #rx_buf_name = &mut #rx_buf_static.init([0_u8; ::rmk::split::SPLIT_MESSAGE_MAX_SIZE])[..];
        };
        let uart_init = match chip.series {
            ChipSeries::Rp2040 => {
                let uart_instance = format_ident!("{}", s.instance);
                let uart_name = format_ident!("{}", s.instance.to_lowercase());
                let tx_pin = format_ident!("{}", s.tx_pin);
                let rx_pin = format_ident!("{}", s.rx_pin);
                let irq_name = format_ident!("IrqsUart{}", idx);
                match &s.instance {
                    i if i.starts_with("UART") => {
                        let uart_irq = format_ident!("{}_IRQ", s.instance);
                        quote! {
                            ::embassy_rp::bind_interrupts!(struct #irq_name {
                                #uart_irq => ::embassy_rp::uart::BufferedInterruptHandler<::embassy_rp::peripherals::#uart_instance>;
                            });
                            let #uart_name = ::embassy_rp::uart::BufferedUart::new(
                                p.#uart_instance,
                                #irq_name,
                                p.#tx_pin,
                                p.#rx_pin,
                                #tx_buf_name,
                                #rx_buf_name,
                                ::embassy_rp::uart::Config::default(),
                            );
                        }
                    }
                    i if i.starts_with("PIO") => {
                        let uart_irq = format_ident!("{}_IRQ_0", s.instance);
                        let instance_init = if s.rx_pin.eq(&s.tx_pin) {
                            quote! {
                                let #uart_name = ::rmk::split::rp::uart::BufferedUart::new_half_duplex(
                                    p.#uart_instance,
                                    p.#rx_pin,
                                    #rx_buf_name,
                                    #irq_name,
                                );
                            }
                        } else {
                            quote! {
                                let #uart_name = ::rmk::split::rp::uart::BufferedUart::new_full_duplex(
                                    p.#uart_instance,
                                    p.#tx_pin,
                                    p.#rx_pin,
                                    #tx_buf_name,
                                    #rx_buf_name,
                                    #irq_name,
                                );
                            }
                        };
                        quote! {
                            ::embassy_rp::bind_interrupts!(struct #irq_name {
                                #uart_irq => ::rmk::split::rp::uart::UartInterruptHandler<::embassy_rp::peripherals::#uart_instance>;
                            });
                            #instance_init
                        }
                    }
                    _ => panic!("Serial instance {:?} is not recognised", s.instance),
                }
            }
            _ => panic!("Serial for chip {:?} isn't implemented yet", chip.series),
        };
        uart_initializers.extend(quote! {
            #uart_buf_init
            #uart_init
        });
    });
    uart_initializers
}

fn join_all_tasks(tasks: Vec<TokenStream2>) -> TokenStream2 {
    let mut current_joined = quote! {};
    tasks.iter().enumerate().for_each(|(id, task)| {
        if id == 0 {
            current_joined = quote! {#task};
        } else {
            current_joined = quote! {
                ::embassy_futures::join::join(#current_joined, #task)
            };
        }
    });

    quote! {
        #current_joined.await;
    }
}
