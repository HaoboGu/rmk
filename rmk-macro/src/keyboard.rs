use darling::FromMeta;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::ItemMod;

use crate::{
    behavior::expand_behavior_config,
    bind_interrupt::expand_bind_interrupt,
    ble::expand_ble_config,
    chip_init::expand_chip_init,
    comm::expand_usb_init,
    entry::expand_rmk_entry,
    feature::{get_rmk_features, is_feature_enabled},
    flash::expand_flash_init,
    import::expand_imports,
    keyboard_config::{
        expand_keyboard_info, expand_vial_config, read_keyboard_toml_config, KeyboardConfig,
    },
    layout::expand_layout_init,
    light::expand_light_config,
    matrix::expand_matrix_config,
    ChipSeries,
};

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
    let async_matrix = is_feature_enabled(&rmk_features, "async_matrix");

    let toml_config = match read_keyboard_toml_config() {
        Ok(c) => c,
        Err(e) => return e,
    };

    if let Some(m) = toml_config.clone().matrix {
        if m.row2col {
            eprintln!("row2col is enabled, please ensure that you have updated your Cargo.toml, disabled default features(col2row is enabled as default feature)");
        }
    }

    let keyboard_config = match KeyboardConfig::new(toml_config) {
        Ok(c) => c,
        Err(e) => return e,
    };

    let imports = gen_imports(&keyboard_config);

    // Expanded main function
    let main_function = expand_main(&keyboard_config, item_mod, async_matrix);

    quote! {
        #imports

        #main_function
    }
}

pub(crate) fn gen_imports(config: &KeyboardConfig) -> TokenStream2 {
    // Create layout info
    let layout = expand_layout_init(config);
    // Create keyboard info and vial struct
    let keyboard_info_static_var = expand_keyboard_info(config);

    // Create vial config
    let vial_static_var = expand_vial_config();

    let imports = match config.chip.series {
        ChipSeries::Esp32 => quote! {}, // For ESP32s, no panic handler and defmt logger are used
        _ => {
            // If defmt_log is disabled, add an empty defmt logger impl
            if config.dependency.defmt_log {
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

    // TODO: remove static var?
    quote! {
        use defmt::*;
        #imports

        #keyboard_info_static_var
        #vial_static_var
        #layout
    }
}

fn expand_main(
    keyboard_config: &KeyboardConfig,
    item_mod: ItemMod,
    async_matrix: bool,
) -> TokenStream2 {
    // Expand components of main function
    let imports = expand_imports(&item_mod);
    let bind_interrupt = expand_bind_interrupt(keyboard_config, &item_mod);
    let chip_init = expand_chip_init(keyboard_config, &item_mod);
    let usb_init = expand_usb_init(keyboard_config, &item_mod);
    let flash_init = expand_flash_init(keyboard_config);
    let light_config = expand_light_config(keyboard_config);
    let behavior_config = expand_behavior_config(keyboard_config);
    let matrix_config = expand_matrix_config(keyboard_config, async_matrix);
    let run_rmk = expand_rmk_entry(keyboard_config, &item_mod);
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

            // Initialize matrix config as `(input_pins, output_pins)` or `direct_pins`
            #matrix_config

            #ble_config

            // Set all keyboard config
            let keyboard_config = ::rmk::config::RmkConfig {
                usb_config: KEYBOARD_USB_CONFIG,
                vial_config: VIAL_CONFIG,
                light_config,
                storage_config,
                behavior_config,
                #set_ble_config
                ..Default::default()
            };

            // Start serving
            #run_rmk
        }
    }
}
