use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use rmk_config::{
    BoardConfig, ChipModel, ChipSeries, CommunicationConfig, InputDeviceConfig, KeyboardTomlConfig, MatrixType,
    SplitBoardConfig, SplitConfig,
};
use syn::ItemMod;

use crate::chip_init::expand_chip_init;
use crate::entry::join_all_tasks;
use crate::feature::{get_rmk_features, is_feature_enabled};
use crate::flash::expand_flash_init;
use crate::import::expand_custom_imports;
use crate::input_device::adc::expand_adc_device;
use crate::input_device::encoder::expand_encoder_device;
use crate::keyboard_config::read_keyboard_toml_config;
use crate::matrix::{expand_matrix_direct_pins, expand_matrix_input_output_pins};
use crate::split::central::expand_serial_init;

/// Parse split peripheral mod and generate a valid RMK main function with all needed code
pub(crate) fn parse_split_peripheral_mod(id: usize, _attr: proc_macro::TokenStream, item_mod: ItemMod) -> TokenStream2 {
    let rmk_features = get_rmk_features();
    if !is_feature_enabled(&rmk_features, "split") {
        panic!("\"split\" feature of RMK should be enabled");
    }

    let toml_config = read_keyboard_toml_config();

    let main_function = expand_split_peripheral(id, &toml_config, item_mod, &rmk_features);
    let chip = toml_config.get_chip_model().unwrap();

    let bind_interrupts =
        expand_bind_interrupt_for_split_peripheral(&chip, &toml_config.get_communication_config().unwrap());

    let main_function_sig = if chip.series == ChipSeries::Esp32 {
        quote! {
            use {esp_alloc as _, esp_backtrace as _};
            #[esp_hal_embassy::main]
            async fn main(_s: Spawner)
        }
    } else {
        quote! {
            #bind_interrupts
            #[::embassy_executor::main]
            async fn main(spawner: ::embassy_executor::Spawner)
        }
    };

    quote! {
        use defmt_rtt as _;
        use panic_probe as _;

        #main_function_sig {
            // ::defmt::info!("RMK start!");
            #main_function
        }
    }
}

fn expand_bind_interrupt_for_split_peripheral(chip: &ChipModel, communication: &CommunicationConfig) -> TokenStream2 {
    match chip.series {
        ChipSeries::Nrf52 => {
            quote! {
                use ::embassy_nrf::bind_interrupts;
                bind_interrupts!(struct Irqs {
                    CLOCK_POWER => ::nrf_sdc::mpsl::ClockInterruptHandler;
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
                    rng: &'d mut ::embassy_nrf::rng::Rng<::embassy_nrf::peripherals::RNG, ::embassy_nrf::mode::Async>,
                    mpsl: &'d ::nrf_sdc::mpsl::MultiprotocolServiceLayer,
                    mem: &'d mut ::nrf_sdc::Mem<N>,
                ) -> Result<::nrf_sdc::SoftdeviceController<'d>, ::nrf_sdc::Error> {
                    ::nrf_sdc::Builder::new()?
                        .support_adv()?
                        .support_peripheral()?
                        .peripheral_count(1)?
                        .buffer_cfg(L2CAP_MTU as u16, L2CAP_MTU as u16, L2CAP_TXQ, L2CAP_RXQ)?
                        .build(p, rng, mpsl, mem)
                }
            }
        }
        ChipSeries::Rp2040 => {
            if communication.ble_enabled() {
                quote! {
                    use ::embassy_rp::bind_interrupts;
                    bind_interrupts!(struct Irqs {
                        PIO0_IRQ_0 => ::embassy_rp::pio::InterruptHandler<::embassy_rp::peripherals::PIO0>;
                    });
                    #[::embassy_executor::task]
                    async fn cyw43_task(runner: ::cyw43::Runner<'static, ::embassy_rp::gpio::Output<'static>, ::cyw43_pio::PioSpi<'static, ::embassy_rp::peripherals::PIO0, 0, ::embassy_rp::peripherals::DMA_CH0>>) -> ! {
                        runner.run().await
                    }
                }
            } else {
                quote! {}
            }
        }
        _ => quote! {},
    }
}

fn expand_split_peripheral(
    id: usize,
    keyboard_config: &KeyboardTomlConfig,
    item_mod: ItemMod,
    rmk_features: &Option<Vec<String>>,
) -> TokenStream2 {
    // Check whether keyboard.toml contains split section
    let board_config = keyboard_config.get_board_config().unwrap();
    let split_config = match &board_config {
        BoardConfig::Split(split) => split,
        _ => {
            panic!("No `split` field in `keyboard.toml`");
        }
    };

    let peripheral_config = split_config.peripheral.get(id).expect("Missing peripheral config");

    let imports = expand_custom_imports(&item_mod);
    let mut chip_init = expand_chip_init(keyboard_config, Some(id), &item_mod);
    if split_config.connection == "ble" {
        // Add storage when using BLE split
        let flash_init = expand_flash_init(keyboard_config);
        chip_init.extend(quote! {
            #flash_init
            let mut storage = ::rmk::storage::new_storage_for_split_peripheral(flash, storage_config).await;
        });
    }

    // Debouncer config
    let rapid_debouncer_enabled = is_feature_enabled(rmk_features, "rapid_debouncer");
    let col2row_enabled = is_feature_enabled(rmk_features, "col2row");
    let col = peripheral_config.cols;
    let row = peripheral_config.rows;
    let input_output_num = if col2row_enabled {
        quote! { #row, #col }
    } else {
        quote! { #col, #row }
    };

    let debouncer_type = if rapid_debouncer_enabled {
        quote! { ::rmk::debounce::fast_debouncer::RapidDebouncer }
    } else {
        quote! { ::rmk::debounce::default_debouncer::DefaultDebouncer }
    };

    // Matrix config
    let async_matrix = is_feature_enabled(rmk_features, "async_matrix");
    let chip = keyboard_config.get_chip_model().unwrap();
    let mut matrix_config = proc_macro2::TokenStream::new();
    match &peripheral_config.matrix.matrix_type {
        MatrixType::normal => {
            matrix_config.extend(expand_matrix_input_output_pins(
                &chip,
                peripheral_config
                    .matrix
                    .input_pins
                    .clone()
                    .expect("split.peripheral.matrix.input_pins is required"),
                peripheral_config
                    .matrix
                    .output_pins
                    .clone()
                    .expect("split.peripheral.matrix.output_pins is required"),
                async_matrix,
            ));

            matrix_config.extend(quote! {
                let debouncer = #debouncer_type::<#input_output_num>::new();
                let mut matrix = ::rmk::matrix::Matrix::<_, _, _, #input_output_num>::new(input_pins, output_pins, debouncer);
            });
        }
        MatrixType::direct_pin => {
            matrix_config.extend(expand_matrix_direct_pins(
                &chip,
                peripheral_config
                    .matrix
                    .direct_pins
                    .clone()
                    .expect("split.peripheral.matrix.direct_pins is required"),
                async_matrix,
                peripheral_config.matrix.direct_pin_low_active,
            ));
            // `generic_arg_infer` is a nightly feature. Const arguments cannot yet be inferred with `_` in stable now.
            // So we need to declaring them in advance.
            let size = row * col;
            let low_active = peripheral_config.matrix.direct_pin_low_active;

            matrix_config.extend(quote! {
                let debouncer = #debouncer_type::<#col, #row>::new();
                let mut matrix = ::rmk::direct_pin::DirectPinMatrix::<_, _, #row, #col, #size>::new(direct_pins, debouncer, #low_active);
            });
        }
    }

    // Peripherals don't need to run processors
    let (input_device_config, devices, _processors) = expand_peripheral_input_device_config(id, keyboard_config);
    let run_rmk_peripheral = expand_split_peripheral_entry(id, &chip, split_config, peripheral_config, devices);

    quote! {
        #imports
        #chip_init
        #matrix_config
        #input_device_config
        #run_rmk_peripheral
    }
}

fn expand_split_peripheral_entry(
    id: usize,
    chip: &ChipModel,
    split_config: &SplitConfig,
    peripheral_config: &SplitBoardConfig,
    devices: Vec<TokenStream2>,
) -> TokenStream2 {
    // Add matrix to devices, and run all devices
    let mut devs = devices.clone();
    devs.push(quote! {matrix});
    let device_task = quote! {
        ::rmk::run_devices! (
            (#(#devs),*) => ::rmk::channel::EVENT_CHANNEL,
        )
    };

    if split_config.connection == "ble" {
        let peripheral_run = quote! {
            ::rmk::split::peripheral::run_rmk_split_peripheral(
                #id,
                &stack,
                &mut storage,
            )
        };
        let run_rmk_peripheral = join_all_tasks(vec![device_task, peripheral_run]);
        quote! {
            #run_rmk_peripheral
        }
    } else if split_config.connection == "serial" {
        let peripheral_serial = peripheral_config
            .serial
            .clone()
            .expect("Missing peripheral serial config");
        if peripheral_serial.len() != 1 {
            panic!("Peripheral should have only one serial config");
        }
        let serial_init = expand_serial_init(chip, peripheral_serial);

        let uart_instance = format_ident!(
            "{}",
            peripheral_config
                .serial
                .as_ref()
                .expect("Missing peripheral serial config")
                .first()
                .expect("Peripheral should have only one serial config")
                .instance
                .to_lowercase()
        );
        let peripheral_run = quote! {
            ::rmk::split::peripheral::run_rmk_split_peripheral(#uart_instance)
        };
        let run_rmk_peripheral = join_all_tasks(vec![device_task, peripheral_run]);
        quote! {
            #serial_init
            #run_rmk_peripheral
        }
    } else {
        panic!("Invalid split connection type: {}", split_config.connection);
    }
}

pub(crate) fn expand_peripheral_input_device_config(
    id: usize,
    keyboard_config: &KeyboardTomlConfig,
) -> (TokenStream2, Vec<TokenStream2>, Vec<TokenStream2>) {
    let mut config = TokenStream2::new();
    let mut devices = Vec::new();
    let mut processors = Vec::new();

    let communication = keyboard_config.get_communication_config().unwrap();
    let ble_config = match &communication {
        CommunicationConfig::Ble(ble_config) | CommunicationConfig::Both(_, ble_config) => Some(ble_config.clone()),
        _ => None,
    };
    let board = keyboard_config.get_board_config().unwrap();
    let chip = keyboard_config.get_chip_model().unwrap();

    // generate ADC configuration
    let (adc_config, adc_processors) = match &board {
        BoardConfig::Split(split_config) => expand_adc_device(
            split_config.peripheral[id]
                .input_device
                .clone()
                .unwrap_or(InputDeviceConfig::default())
                .joystick
                .unwrap_or(Vec::new()),
            ble_config,
            chip.series.clone(),
        ),
        _ => (quote! {}, vec![]),
    };
    config.extend(adc_config);
    if !adc_processors.is_empty() {
        devices.push(quote! {adc_device});
    }
    processors.extend(adc_processors);

    // generate encoder configuration, processors are ignored
    let num_encoders = keyboard_config.get_board_config().unwrap().get_num_encoder();
    // The num_encoders[0] is always the number of encoders on the central, so the offset should be num_encoders[0..id + 1], where id is the index of the peripheral
    let encoder_id_offset = num_encoders[0..id + 1].iter().sum::<usize>();
    let (encoder_devices, _encoder_processors) = match &board {
        BoardConfig::Split(split_config) => expand_encoder_device(
            encoder_id_offset,
            split_config.peripheral[id]
                .input_device
                .clone()
                .unwrap_or(InputDeviceConfig::default())
                .encoder
                .unwrap_or(Vec::new()),
            &chip,
        ),
        _ => (vec![], vec![]),
    };

    for initializer in encoder_devices {
        config.extend(initializer.initializer);
        let device_name = initializer.var_name;
        devices.push(quote! { #device_name });
    }

    (config, devices, processors)
}
