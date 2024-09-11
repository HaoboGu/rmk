use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use rmk_config::toml_config::{KeyboardTomlConfig, SerialConfig, SplitConfig};
use syn::ItemMod;

use crate::{
    bind_interrupt::expand_bind_interrupt,
    ble::expand_ble_config,
    chip_init::expand_chip_init,
    comm::expand_usb_init,
    feature::{get_rmk_features, is_feature_enabled},
    flash::expand_flash_init,
    import::expand_imports,
    keyboard::{gen_imports, get_chip_info, CommunicationType},
    keyboard_config::read_keyboard_config,
    light::expand_light_config,
    matrix::expand_matrix_input_output_pins,
    usb_interrupt_map::UsbInfo,
    ChipModel, ChipSeries,
};

/// Parse split central mod and generate a valid RMK main function with all needed code
pub(crate) fn parse_split_central_mod(
    attr: proc_macro::TokenStream,
    item_mod: ItemMod,
) -> TokenStream2 {
    let rmk_features = get_rmk_features();
    if !is_feature_enabled(&rmk_features, "split") {
        return quote! {
            compile_error!("\"split\" feature of RMK should be enabled");
        };
    }

    let async_matrix = is_feature_enabled(&rmk_features, "async_matrix");

    let toml_config = match read_keyboard_config(attr) {
        Ok(c) => c,
        Err(e) => return e,
    };

    let (chip, comm_type, usb_info) = match get_chip_info(&toml_config) {
        Ok(value) => value,
        Err(e) => return e,
    };

    let imports = gen_imports(&toml_config, &chip);

    let main_function = expand_split_central(
        &chip,
        comm_type,
        usb_info,
        toml_config,
        item_mod,
        async_matrix,
    );

    quote! {
        #imports

        #main_function
    }
}

fn expand_split_central(
    chip: &ChipModel,
    comm_type: CommunicationType,
    usb_info: UsbInfo,
    toml_config: KeyboardTomlConfig,
    item_mod: ItemMod,
    async_matrix: bool,
) -> TokenStream2 {
    // Check whether keyboard.toml contains split section
    let split_config = match &toml_config.split {
        Some(c) => c,
        None => return quote! { compile_error!("No `split` field in `keyboard.toml`"); }.into(),
    };

    // Expand components of main function
    let imports = expand_imports(&item_mod);
    let bind_interrupt = expand_bind_interrupt(&chip, &usb_info, &toml_config, &item_mod);
    let chip_init = expand_chip_init(&chip, &item_mod);
    let usb_init = expand_usb_init(&chip, &usb_info, comm_type, &item_mod);
    let flash_init = expand_flash_init(&chip, comm_type, toml_config.storage);
    let light_config = expand_light_config(&chip, toml_config.light);
    let matrix_config = expand_matrix_input_output_pins(
        &chip,
        split_config.central.input_pins.clone(),
        split_config.central.output_pins.clone(),
        async_matrix,
    );
    let split_communicate = expand_split_communicate(&chip, split_config);
    let run_rmk = expand_split_central_entry(&chip, split_config);
    let (ble_config, set_ble_config) = expand_ble_config(&chip, comm_type, toml_config.ble);

    let main_function_sig = if chip.series == ChipSeries::Esp32 {
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
            #light_config;

            // Initialize matrix config as `(input_pins, output_pins)`
            #matrix_config;

            #ble_config

            // Set all keyboard config
            let keyboard_config = ::rmk::config::RmkConfig {
                usb_config: keyboard_usb_config,
                vial_config,
                light_config,
                storage_config,
                #set_ble_config
                ..Default::default()
            };

            #split_communicate

            // Start serving
            #run_rmk
        }
    }
}

fn expand_split_central_entry(chip: &ChipModel, split_config: &SplitConfig) -> TokenStream2 {
    match chip.series {
        ChipSeries::Stm32 => todo!(),
        ChipSeries::Nrf52 => {
            let central_addr = split_config
                .central
                .ble_addr
                .expect("No ble_addr defined for central");

            let row = split_config.central.rows;
            let col = split_config.central.cols;
            let row_offset = split_config.central.row_offset;
            let col_offset = split_config.central.col_offset;
            let central_task = quote! {
                ::rmk::split::central::run_rmk_split_central::<
                    ::embassy_nrf::gpio::Input<'_>,
                    ::embassy_nrf::gpio::Output<'_>,
                    ::embassy_nrf::usb::Driver<'_, ::embassy_nrf::peripherals::USBD, &::embassy_nrf::usb::vbus_detect::SoftwareVbusDetect>,
                    ROW,
                    COL,
                    #row,
                    #col,
                    #row_offset,
                    #col_offset,
                    NUM_LAYER,
                >(input_pins, output_pins, driver, KEYMAP, keyboard_config, [#(#central_addr), *], spawner)
            };
            let mut tasks = vec![central_task];
            split_config.peripheral.iter().enumerate().for_each(|(idx, p)| {
                let row = p.rows ;
                let col = p.cols ;
                let row_offset = p.row_offset ;
                let col_offset = p.col_offset ;
                let peripheral_ble_addr = p.ble_addr.expect("No ble_addr defined for peripheral");
                tasks.push(quote! {
                    ::rmk::split::central::run_peripheral_monitor::<#row, #col, #row_offset, #col_offset>(
                        #idx,
                        [#(#peripheral_ble_addr), *],
                    )
                });
            });
            join_all_tasks(tasks)
        }
        ChipSeries::Rp2040 => {
            let row = split_config.central.rows as usize;
            let col = split_config.central.cols as usize;
            let row_offset = split_config.central.row_offset as usize;
            let col_offset = split_config.central.col_offset as usize;
            let central_task = quote! {
                ::rmk::split::central::run_rmk_split_central::<
                    ::embassy_rp::gpio::Input<'_>,
                    ::embassy_rp::gpio::Output<'_>,
                    ::embassy_rp::usb::Driver<'_, ::embassy_rp::peripherals::USB>,
                    ::embassy_rp::flash::Flash<::embassy_rp::peripherals::FLASH, ::embassy_rp::flash::Async, FLASH_SIZE>,
                    ROW,
                    COL,
                    #row,
                    #col,
                    #row_offset,
                    #col_offset,
                    NUM_LAYER,
                >(input_pins, output_pins, driver, flash, KEYMAP, keyboard_config, spawner)
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
                .for_each(|(idx, peripheral_config)| {
                    let row = peripheral_config.rows as usize;
                    let col = peripheral_config.cols as usize;
                    let row_offset = peripheral_config.row_offset as usize;
                    let col_offset = peripheral_config.col_offset as usize;
                    let uart_instance = format_ident!("{}", central_serials.get(idx).expect("No or not enough serial defined for peripheral in central").instance.to_lowercase());

                    tasks.push(quote! {
                        ::rmk::split::central::run_peripheral_monitor::<#row, #col, #row_offset, #col_offset, _>(
                            #idx,
                            #uart_instance,
                        )
                    });
                });

            join_all_tasks(tasks)
        }
        ChipSeries::Esp32 => todo!(),
    }
}

fn expand_split_communicate(chip: &ChipModel, split_config: &SplitConfig) -> TokenStream2 {
    match &split_config.connection[..] {
        "ble" => {
            // We need to create addrs for BLE
            let central_addr = split_config.central.ble_addr.unwrap();
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
            let serial_config: Vec<SerialConfig> = split_config.central.serial.clone().unwrap();
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
            ChipSeries::Stm32 => todo!(),
            ChipSeries::Nrf52 => todo!(),
            ChipSeries::Rp2040 => {
                let uart_instance = format_ident!("{}", s.instance);
                let uart_name = format_ident!("{}", s.instance.to_lowercase());
                let uart_irq = format_ident!("{}_IRQ", s.instance);
                let tx_pin = format_ident!("{}", s.tx_pin);
                let rx_pin = format_ident!("{}", s.rx_pin);
                let irq_name = format_ident!("IrqsUart{}", idx);
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
            ChipSeries::Esp32 => todo!(),
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
