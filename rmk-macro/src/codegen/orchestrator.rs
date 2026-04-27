use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use rmk_config::resolved::hardware::{
    BoardConfig, ChipSeries, KeyInfo, MatrixConfig, MatrixType, UniBodyConfig,
};
use rmk_config::resolved::{Behavior, Hardware, Host, Identity, Layout};

use super::behavior::expand_behavior_config;
use super::chip::bind_interrupt::expand_bind_interrupt;
use super::chip::ble::expand_ble_config;
use super::chip::chip_init::expand_chip_init;
use super::chip::comm::expand_usb_init;
use super::chip::flash::expand_flash_init;
use super::chip::gpio::expand_output_config;
use super::display::expand_display_config;
use super::entry::expand_rmk_entry;
use super::feature::{get_rmk_features, is_feature_enabled};
use super::import::expand_custom_imports;
use super::input_device::expand_input_device_config;
use super::keyboard_config::{expand_keyboard_info, expand_vial_config, read_keyboard_toml_config};
use super::layout::expand_default_keymap;
use super::matrix::{expand_bootmagic_check, expand_matrix_config};
use super::registered_processor::expand_registered_processor_init;
use super::split::central::expand_split_central_config;

/// Parse keyboard mod and generate a valid RMK main function with all needed code
pub(crate) fn parse_keyboard_mod(item_mod: syn::ItemMod) -> TokenStream2 {
    let rmk_features = get_rmk_features();

    let keyboard_config = read_keyboard_toml_config();

    // Resolve types from keyboard.toml
    let identity = keyboard_config
        .identity()
        .expect("failed to resolve identity config");
    let host = keyboard_config.host();
    let hardware = keyboard_config
        .hardware()
        .expect("failed to resolve hardware config");
    let behavior = keyboard_config
        .behavior()
        .expect("failed to resolve behavior config");
    let layout = keyboard_config
        .layout()
        .expect("failed to resolve layout config");

    validate_feature_config_parity(
        hardware.storage.is_some(),
        is_feature_enabled(&rmk_features, "storage"),
        host.vial_enabled,
        is_feature_enabled(&rmk_features, "vial"),
    )
    .unwrap_or_else(|err| panic!("{err}"));

    // Generate imports and statics
    let imports_and_statics =
        expand_imports_and_constants(&identity, &host, &hardware, &behavior, &layout);

    // Generate main function body
    let main_function = expand_main(
        &host,
        &hardware,
        &behavior,
        &layout,
        item_mod,
        &rmk_features,
    );

    quote! {
        #imports_and_statics

        #main_function
    }
}

fn validate_feature_config_parity(
    storage_enabled_in_config: bool,
    storage_enabled_in_features: bool,
    vial_enabled_in_config: bool,
    vial_enabled_in_features: bool,
) -> Result<(), &'static str> {
    if storage_enabled_in_config != storage_enabled_in_features {
        if storage_enabled_in_config {
            return Err(
                "If the \"storage\" Cargo feature is disabled, `storage.enabled` must be set to false in keyboard.toml.",
            );
        }
        return Err(
            "`storage.enabled = false` in keyboard.toml requires disabling the \"storage\" Cargo feature for rmk in Cargo.toml (for example with `default-features = false` and explicitly re-enabling the features you need).",
        );
    }

    if vial_enabled_in_config != vial_enabled_in_features {
        if vial_enabled_in_config {
            return Err(
                "If the \"vial\" Cargo feature is disabled, `host.vial_enabled` must be set to false in keyboard.toml.",
            );
        }
        return Err(
            "`host.vial_enabled = false` in keyboard.toml requires disabling the \"vial\" Cargo feature for rmk in Cargo.toml (for example with `default-features = false` and explicitly re-enabling the features you need).",
        );
    }

    Ok(())
}

pub(crate) fn expand_imports_and_constants(
    identity: &Identity,
    host: &Host,
    hardware: &Hardware,
    behavior: &Behavior,
    layout: &Layout,
) -> TokenStream2 {
    // Generate keyboard info and number of rows/cols/layers
    let keyboard_info_static_var = expand_keyboard_info(identity, layout);
    // Generate default keymap
    let default_keymap = expand_default_keymap(layout, behavior);
    // Generate vial config
    let vial_static_var = expand_vial_config(host);

    // Generate extra imports, panic handler and logger
    let imports = match hardware.chip.series {
        ChipSeries::Esp32 => quote! {
            use esp_alloc as _;
            use esp_backtrace as _;
            ::esp_bootloader_esp_idf::esp_app_desc!();
        },
        _ => {
            // If defmt_log is disabled, add an empty defmt logger impl
            if hardware.dependency.defmt_log {
                quote! {
                    use panic_probe as _;
                    use defmt_rtt as _;
                }
            } else {
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

#[cfg(test)]
mod tests {
    use super::validate_feature_config_parity;

    #[test]
    fn accepts_matching_storage_and_vial_feature_states() {
        assert!(validate_feature_config_parity(true, true, true, true).is_ok());
        assert!(validate_feature_config_parity(false, false, false, false).is_ok());
        assert!(validate_feature_config_parity(true, true, false, false).is_ok());
    }

    #[test]
    fn rejects_storage_enabled_in_config_without_feature() {
        let err = validate_feature_config_parity(true, false, false, false).unwrap_err();
        assert_eq!(
            err,
            "If the \"storage\" Cargo feature is disabled, `storage.enabled` must be set to false in keyboard.toml."
        );
    }

    #[test]
    fn rejects_storage_feature_without_config() {
        let err = validate_feature_config_parity(false, true, false, false).unwrap_err();
        assert_eq!(
            err,
            "`storage.enabled = false` in keyboard.toml requires disabling the \"storage\" Cargo feature for rmk in Cargo.toml (for example with `default-features = false` and explicitly re-enabling the features you need)."
        );
    }

    #[test]
    fn rejects_vial_enabled_in_config_without_feature() {
        let err = validate_feature_config_parity(false, false, true, false).unwrap_err();
        assert_eq!(
            err,
            "If the \"vial\" Cargo feature is disabled, `host.vial_enabled` must be set to false in keyboard.toml."
        );
    }

    #[test]
    fn rejects_vial_feature_without_config() {
        let err = validate_feature_config_parity(false, false, false, true).unwrap_err();
        assert_eq!(
            err,
            "`host.vial_enabled = false` in keyboard.toml requires disabling the \"vial\" Cargo feature for rmk in Cargo.toml (for example with `default-features = false` and explicitly re-enabling the features you need)."
        );
    }
}

fn expand_main(
    host: &Host,
    hardware: &Hardware,
    behavior: &Behavior,
    layout: &Layout,
    item_mod: syn::ItemMod,
    rmk_features: &Option<Vec<String>>,
) -> TokenStream2 {
    // Expand components of main function
    let imports = expand_custom_imports(&item_mod);
    let bind_interrupt = expand_bind_interrupt(hardware, &item_mod);
    let chip_init = expand_chip_init(hardware, None, &item_mod);
    let usb_init = expand_usb_init(hardware, &item_mod);
    let flash_init = expand_flash_init(hardware);
    let behavior_config = expand_behavior_config(behavior);
    let matrix_config = expand_matrix_config(hardware, rmk_features);
    let output_config = expand_output_config(hardware);
    let (ble_config, set_ble_config) = expand_ble_config(hardware);
    let keymap_and_storage = expand_keymap_and_storage(hardware, layout);
    let split_central_config = expand_split_central_config(hardware);
    let (input_device_config, devices, processors) = expand_input_device_config(hardware);
    let matrix_and_keyboard = expand_matrix_and_keyboard_init(hardware);
    let (registered_processor_initializers, mut registered_processors) =
        expand_registered_processor_init(hardware, &item_mod);

    // Display configuration — for unibody use top-level, for split use central's config
    let display_config = match &hardware.board {
        BoardConfig::UniBody(_) => hardware.display.as_ref(),
        BoardConfig::Split(split_config) => split_config.central.display.as_ref(),
    };
    let display_init = if let Some(display_config) = display_config {
        let (init, processor) = expand_display_config(&hardware.chip.series, display_config);
        let processor_initializer = processor.initializer;
        let processor_var = processor.var_name;
        registered_processors.push(quote! { #processor_var.run() });
        quote! {
            #init
            #processor_initializer
        }
    } else {
        quote! {}
    };

    let host_service_init = if host.vial_enabled {
        quote! {
            let mut host_service = ::rmk::host::HostService::new(&keymap, &rmk_config);
        }
    } else {
        quote! {}
    };

    let run_rmk = expand_rmk_entry(
        hardware,
        host,
        &item_mod,
        devices,
        processors,
        registered_processors,
    );

    let vial_config = if host.vial_enabled {
        quote! { vial_config: VIAL_CONFIG,}
    } else {
        quote! {}
    };

    let rmk_config = if hardware.storage.is_some() {
        quote! {
            #[allow(clippy::needless_update)]
            let rmk_config = ::rmk::config::RmkConfig {
                device_config: KEYBOARD_DEVICE_CONFIG,
                #vial_config
                storage_config,
                #set_ble_config
                ..Default::default()
            };
        }
    } else {
        quote! {
            #[allow(clippy::needless_update)]
            let rmk_config = ::rmk::config::RmkConfig {
                device_config: KEYBOARD_DEVICE_CONFIG,
                #vial_config
                #set_ble_config
                ..Default::default()
            };
        }
    };

    let main_function_sig = if hardware.chip.series == ChipSeries::Esp32 {
        quote! {
            #[::esp_rtos::main]
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

            // Initialize behavior config config as `behavior_config`
            #behavior_config

            // Initialize matrix config as `(row_pins, col_pins)` or `direct_pins`
            #matrix_config

            // Initialize static output pins
            #output_config

            // Initialize flash driver as `flash` and storage config as `storage_config`
            #flash_init

            // Initialize ble config as `ble_battery_config`
            #ble_config

            // Set all keyboard config
            #rmk_config

            // Initialize the registered processors
            #registered_processor_initializers

            // Initialize the storage and keymap, as `storage` and `keymap`
            #keymap_and_storage

            // Initialize the matrix + keyboard, as `matrix` and `keyboard`
            #matrix_and_keyboard

            // Initialize the host (Vial) service, as `host_service`
            #host_service_init

            // Initialize input device config as `input_device_config` and processor as `processor`
            #input_device_config

            // Initialize display (if configured)
            #display_init

            // Initialize split central config(if needed)
            #split_central_config

            // Start
            #run_rmk
        }
    }
}

// TODO: move this function to a separate folder
pub(crate) fn expand_keymap_and_storage(hardware: &Hardware, layout: &Layout) -> TokenStream2 {
    let row = layout.rows as usize;
    let col = layout.cols as usize;

    let initialize_positional_config = if layout.key_info.is_empty()
        || layout.key_info.iter().all(|row| {
            row.iter().all(|key| {
                key.hand != 'L'
                    && key.hand != 'l'
                    && key.hand != 'R'
                    && key.hand != 'r'
                    && key.hand != '*'
            })
        })
        || layout.key_info.len() != row
        || layout.key_info[0].len() != col
    {
        quote! { let per_key_config = ::rmk::config::PositionalConfig::default(); }
    } else {
        let key_info_config = expand_key_info(&layout.key_info);
        quote! { let per_key_config = ::rmk::config::PositionalConfig::new(#key_info_config); }
    };

    let total_num_encoders: usize = layout.encoder_counts.iter().sum();

    let keymap_data_init = if total_num_encoders == 0 {
        quote! {
            let mut keymap_data = ::rmk::KeymapData::new(get_default_keymap());
        }
    } else {
        quote! {
            let mut keymap_data = ::rmk::KeymapData::new_with_encoder(
                get_default_keymap(),
                get_default_encoder_map(),
            );
        }
    };

    if hardware.storage.is_some() {
        quote! {
            #initialize_positional_config
            #keymap_data_init
            let (keymap, mut storage) = ::rmk::initialize_keymap_and_storage(
                &mut keymap_data,
                flash,
                &rmk_config.storage_config,
                &mut behavior_config,
                &per_key_config,
            ).await;
        }
    } else {
        quote! {
            #initialize_positional_config
            #keymap_data_init
            let keymap = ::rmk::initialize_keymap(
                &mut keymap_data,
                &mut behavior_config,
                &per_key_config,
            ).await;
        }
    }
}

pub(crate) fn expand_matrix_and_keyboard_init(hardware: &Hardware) -> TokenStream2 {
    let matrix = match &hardware.board {
        BoardConfig::UniBody(UniBodyConfig {
            matrix: matrix_config,
            input_device: _,
        }) => {
            let bootmagic = expand_bootmagic_check(matrix_config);
            let debouncer_type = get_debouncer_type(matrix_config);
            match matrix_config.matrix_type {
                MatrixType::Normal => {
                    let col2row = !matrix_config.row2col;
                    quote! {
                        #bootmagic
                        let debouncer = #debouncer_type::new();
                        let mut matrix = ::rmk::matrix::Matrix::<_, _, _, ROW, COL, #col2row>::new(row_pins, col_pins, debouncer);
                    }
                }
                MatrixType::DirectPin => {
                    let low_active = matrix_config.direct_pin_low_active;
                    quote! {
                        #bootmagic
                        let debouncer = #debouncer_type::new();
                        let mut matrix = ::rmk::matrix::direct_pin::DirectPinMatrix::<_, _, ROW, COL, SIZE>::new(direct_pins, debouncer, #low_active);
                    }
                }
            }
        }
        BoardConfig::Split(split_config) => {
            // Matrix config for split central
            let central_row = split_config.central.rows;
            let central_row_offset = split_config.central.row_offset;
            let central_col = split_config.central.cols;
            let central_col_offset = split_config.central.col_offset;
            let col2row = !split_config.central.matrix.row2col;
            let bootmagic = expand_bootmagic_check(&split_config.central.matrix);
            let debouncer_type = get_debouncer_type(&split_config.central.matrix);
            match split_config.central.matrix.matrix_type {
                MatrixType::Normal => {
                    quote! {
                        #bootmagic
                        let debouncer = #debouncer_type::new();
                        let mut matrix = ::rmk::matrix::Matrix::<_, _, _, #central_row, #central_col, #col2row, #central_row_offset, #central_col_offset>::new(row_pins, col_pins, debouncer);
                    }
                }
                MatrixType::DirectPin => {
                    let low_active = split_config.central.matrix.direct_pin_low_active;
                    let size = split_config.central.rows * split_config.central.cols;
                    quote! {
                        #bootmagic
                        let debouncer = #debouncer_type::new();
                        let mut matrix = ::rmk::matrix::direct_pin::DirectPinMatrix::<_, _, #central_row, #central_col, #size, #central_row_offset, #central_col_offset>::new(direct_pins, debouncer, #low_active);
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

/// Push rows in the key_info
fn expand_key_info(info: &Vec<Vec<KeyInfo>>) -> proc_macro2::TokenStream {
    let mut rows = vec![];
    for row in info {
        rows.push(expand_key_info_row(row));
    }
    quote! { [#(#rows), *] }
}

/// Push keys info in the row
fn expand_key_info_row(row: &Vec<KeyInfo>) -> proc_macro2::TokenStream {
    let mut key_info = vec![];
    for key in row {
        let hand = match key.hand {
            'l' | 'L' => quote! { rmk::config::Hand::Left },
            'r' | 'R' => quote! { rmk::config::Hand::Right },
            '*' => quote! { rmk::config::Hand::Bilateral },
            _ => quote! { rmk::config::Hand::Unknown },
        };
        key_info.push(hand);
    }
    quote! { [#(#key_info), *] }
}

/// Get debouncer type
pub(crate) fn get_debouncer_type(matrix_config: &MatrixConfig) -> TokenStream2 {
    match matrix_config
        .debouncer
        .clone()
        .unwrap_or("default".to_string())
    {
        s if s == "fast" => quote! { ::rmk::debounce::fast_debouncer::FastDebouncer },
        s if s == "default" => quote! { ::rmk::debounce::default_debouncer::DefaultDebouncer },
        _ => panic!("Invalid debouncer type, supported debouncer types are `default` and `fast`"),
    }
}
