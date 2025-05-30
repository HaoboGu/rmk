use darling::FromMeta;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use rmk_config::{BoardConfig, ChipSeries, KeyboardTomlConfig, MatrixType, UniBodyConfig};
use syn::ItemMod;

use crate::behavior::expand_behavior_config;
use crate::bind_interrupt::expand_bind_interrupt;
use crate::ble::expand_ble_config;
use crate::chip_init::expand_chip_init;
use crate::comm::expand_usb_init;
use crate::entry::expand_rmk_entry;
use crate::feature::{get_rmk_features, is_feature_enabled};
use crate::flash::expand_flash_init;
use crate::import::expand_custom_imports;
use crate::input_device::expand_input_device_config;
use crate::keyboard_config::{expand_keyboard_info, expand_vial_config, read_keyboard_toml_config};
use crate::layout::expand_default_keymap;
use crate::light::expand_light_config;
use crate::matrix::expand_matrix_config;
use crate::split::central::expand_split_central_config;

/// List of functions that can be overwritten
#[derive(Debug, Clone, Copy, FromMeta)]
pub enum Overwritten {
    Usb,
    ChipConfig,
    Entry,
}

/// Parse keyboard mod and generate a valid RMK main function with all needed code
pub(crate) fn parse_keyboard_mod(item_mod: ItemMod) -> TokenStream2 {
    let rmk_features = get_rmk_features();

    let keyboard_config = read_keyboard_toml_config();

    if keyboard_config.get_storage_config().enabled != is_feature_enabled(&rmk_features, "storage") {
        if keyboard_config.get_storage_config().enabled {
            panic!("If the \"storage\" cargo feature is disabled, `storage.enabled` must be set to false in the keyboard.toml.")
        } else {
            panic!("Storage is disabled. The \"storage\" cargo feature must also be disabled, by disabling default features for rmk in your Cargo.toml (and potentially re-adding col2row and defmt, as desired)")
        }
    }

    // Generate imports and statics
    let imports_and_statics = expand_imports_and_constants(&keyboard_config);

    // Generate main function body
    let main_function = expand_main(&keyboard_config, item_mod, &rmk_features);

    quote! {
        #imports_and_statics

        #main_function
    }
}

pub(crate) fn expand_imports_and_constants(config: &KeyboardTomlConfig) -> TokenStream2 {
    // Generate keyboard info and number of rows/cols/layers
    let keyboard_info_static_var = expand_keyboard_info(config);
    // Generate default keymap
    let default_keymap = expand_default_keymap(config);
    // Generate vial config
    let vial_static_var = expand_vial_config();

    // Generate extra imports, panic handler and logger
    let imports = match config.get_chip_model().unwrap().series {
        ChipSeries::Esp32 => quote! {
            use {esp_alloc as _, esp_backtrace as _};
        },
        _ => {
            // If defmt_log is disabled, add an empty defmt logger impl
            if config.get_dependency_config().defmt_log {
                quote! {
                    use panic_probe as _;
                    use defmt_rtt as _;
                }
            } else {
                // TODO: use panic_halt when defmt_log is disabled
                quote! {
                    use panic_probe as _;

                    #[::defmt::global_logger]
                    struct Logger;

                    unsafe impl ::defmt::Logger for Logger {
                        fn acquire() {}
                        unsafe fn flush() {}
                        unsafe fn release() {}
                        unsafe fn write(_bytes: &[u8]) {}
                    }
                }
            }
        }
    };

    quote! {
        #imports

        #keyboard_info_static_var
        #vial_static_var
        #default_keymap
    }
}

fn expand_main(
    keyboard_config: &KeyboardTomlConfig,
    item_mod: ItemMod,
    rmk_features: &Option<Vec<String>>,
) -> TokenStream2 {
    // Expand components of main function
    let imports = expand_custom_imports(&item_mod);
    let bind_interrupt = expand_bind_interrupt(keyboard_config, &item_mod);
    let chip_init = expand_chip_init(keyboard_config, None, &item_mod);
    let usb_init = expand_usb_init(keyboard_config, &item_mod);
    let flash_init = expand_flash_init(keyboard_config);
    let light_config = expand_light_config(keyboard_config);
    let behavior_config = expand_behavior_config(keyboard_config);
    let matrix_config = expand_matrix_config(keyboard_config, rmk_features);
    let (ble_config, set_ble_config) = expand_ble_config(keyboard_config);
    let keymap_and_storage = expand_keymap_and_storage(keyboard_config);
    let split_central_config = expand_split_central_config(keyboard_config);
    let (input_device_config, devices, processors) = expand_input_device_config(keyboard_config);
    let matrix_and_keyboard = expand_matrix_and_keyboard_init(keyboard_config, rmk_features);
    let controller = expand_controller_init(keyboard_config);
    let run_rmk = expand_rmk_entry(keyboard_config, &item_mod, devices, processors);

    let rmk_config = if keyboard_config.get_storage_config().enabled {
        quote! {
            let rmk_config = ::rmk::config::RmkConfig {
                usb_config: KEYBOARD_USB_CONFIG,
                vial_config: VIAL_CONFIG,
                storage_config,
                #set_ble_config
                ..Default::default()
            };
        }
    } else {
        quote! {
            let rmk_config = ::rmk::config::RmkConfig {
                usb_config: KEYBOARD_USB_CONFIG,
                vial_config: VIAL_CONFIG,
                #set_ble_config
                ..Default::default()
            };
        }
    };

    let main_function_sig = if keyboard_config.get_chip_model().unwrap().series == ChipSeries::Esp32 {
        quote! {
            #[::esp_hal_embassy::main]
            async fn main(_s: ::embassy_executor::Spawner)
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
            // Initialize peripherals as `p`
            #chip_init

            // Initialize usb driver as `driver`
            #usb_init

            // Initialize light config as `light_config`
            #light_config

            // Initialize behavior config config as `behavior_config`
            #behavior_config

            // Initialize matrix config as `(input_pins, output_pins)` or `direct_pins`
            #matrix_config

            // Initialize flash driver as `flash` and storage config as `storage_config`
            #flash_init

            // Initialize ble config as `ble_battery_config`
            #ble_config

            // Set all keyboard config
            #rmk_config

            // Initialize the controller, as `controller`
            #controller

            // Initialize the storage and keymap, as `storage` and `keymap`
            #keymap_and_storage

            // Initialize the matrix + keyboard, as `matrix` and `keyboard`
            #matrix_and_keyboard

            // Initialize input device config as `input_device_config` and processor as `processor`
            #input_device_config

            // Initialize split central config(if needed)
            #split_central_config

            // Start
            #run_rmk
        }
    }
}

pub(crate) fn expand_keymap_and_storage(keyboard_config: &KeyboardTomlConfig) -> TokenStream2 {
    if keyboard_config.get_storage_config().enabled {
        let num_encoders = keyboard_config.get_board_config().unwrap().get_num_encoder();
        let total_num_encoders = num_encoders.iter().sum::<usize>();
        let keymap_storage_init = if total_num_encoders == 0 {
            // No encoder
            quote! {
                ::rmk::initialize_keymap_and_storage(
                    &mut default_keymap,
                    flash,
                    &rmk_config.storage_config,
                    behavior_config,
                )
            }
        } else {
            // Encoder exists
            quote! {
                ::rmk::initialize_encoder_keymap_and_storage(
                    &mut default_keymap,
                    &mut encoder_keymap,
                    flash,
                    &rmk_config.storage_config,
                    behavior_config,
                )
            }
        };
        let default_encoder_keymap = if total_num_encoders == 0 {
            quote! {}
        } else {
            quote! {
                let mut encoder_keymap = get_default_encoder_map();
            }
        };
        // Return the keymap and storage initialization code
        quote! {
            let mut default_keymap = get_default_keymap();
            #default_encoder_keymap
            let (keymap, mut storage) =  #keymap_storage_init.await;
        }
    } else {
        // Return the keymap initialization code
        quote! {
            let mut default_keymap = get_default_keymap();
            let keymap =  ::rmk::initialize_keymap(
                &mut default_keymap,
                behavior_config,
            ).await;
        }
    }
}

pub(crate) fn expand_matrix_and_keyboard_init(
    keyboard_config: &KeyboardTomlConfig,
    rmk_features: &Option<Vec<String>>,
) -> TokenStream2 {
    let rapid_debouncer_enabled = is_feature_enabled(rmk_features, "rapid_debouncer");
    let col2row_enabled = is_feature_enabled(rmk_features, "col2row");
    let input_output_num = if col2row_enabled {
        quote! { ROW, COL }
    } else {
        quote! { COL, ROW }
    };

    let debouncer_type = if rapid_debouncer_enabled {
        quote! { ::rmk::debounce::fast_debouncer::RapidDebouncer }
    } else {
        quote! { ::rmk::debounce::default_debouncer::DefaultDebouncer }
    };

    let matrix = match keyboard_config.get_board_config().unwrap() {
        BoardConfig::UniBody(UniBodyConfig {
            matrix: matrix_config,
            input_device: _,
        }) => match matrix_config.matrix_type {
            MatrixType::normal => {
                if matrix_config.row2col {
                    eprintln!("row2col is enabled, please ensure that you have updated your Cargo.toml, disabled default features(col2row is enabled as default feature)");
                }
                quote! {
                    let debouncer = #debouncer_type::<#input_output_num>::new();
                    let mut matrix = ::rmk::matrix::Matrix::<_, _, _, #input_output_num>::new(input_pins, output_pins, debouncer);
                }
            }
            MatrixType::direct_pin => {
                let low_active = matrix_config.direct_pin_low_active;
                quote! {
                    let debouncer = #debouncer_type::<COL, ROW>::new();
                    let mut matrix = ::rmk::direct_pin::DirectPinMatrix::<_, _, #input_output_num, SIZE>::new(direct_pins, debouncer, #low_active);
                }
            }
        },
        BoardConfig::Split(split_config) => {
            // Matrix config for split central
            let central_row = split_config.central.rows;
            let central_row_offset = split_config.central.row_offset;
            let central_col = split_config.central.cols;
            let central_col_offset = split_config.central.col_offset;
            let input_output_pin_num = if split_config.central.matrix.row2col {
                eprintln!("row2col is enabled, please ensure that you have updated your Cargo.toml, disabled default features(col2row is enabled as default feature)");
                quote! { #central_row_offset, #central_col_offset, #central_col, #central_row }
            } else {
                quote! { #central_row_offset, #central_col_offset, #central_row, #central_col }
            };
            match split_config.central.matrix.matrix_type {
                MatrixType::normal => quote! {
                    let debouncer = #debouncer_type::<#input_output_num>::new();
                    let mut matrix = ::rmk::split::central::CentralMatrix::<_, _, _, #input_output_pin_num>::new(input_pins, output_pins, debouncer);
                },
                MatrixType::direct_pin => {
                    let low_active = split_config.central.matrix.direct_pin_low_active;
                    let size = split_config.central.rows * split_config.central.cols;
                    quote! {
                        let debouncer = #debouncer_type::<COL, ROW>::new();
                        let mut matrix = ::rmk::split::central::CentralDirectPinMatrix::<_, _, #central_row_offset, #central_col_offset, #central_row, #central_col, #size>::new(direct_pins, debouncer, #low_active);
                    }
                }
            }
        }
    };
    quote! {
        let mut keyboard = ::rmk::keyboard::Keyboard::new(&keymap);
        #matrix
    }
}

fn expand_controller_init(keyboard_config: &KeyboardTomlConfig) -> TokenStream2 {
    // TODO: Initialization for other controllers
    let output_pin_type = match keyboard_config.get_chip_model().unwrap().series {
        ChipSeries::Esp32 => quote! { ::esp_hal::gpio::Output },
        ChipSeries::Stm32 => quote! { ::embassy_stm32::gpio::Output },
        ChipSeries::Nrf52 => quote! { ::embassy_nrf::gpio::Output },
        ChipSeries::Rp2040 => quote! { ::embassy_rp::gpio::Output },
    };

    quote! {
        let mut light_controller: ::rmk::light::LightController<#output_pin_type>  = ::rmk::light::LightController::new(light_config);
    }
}
