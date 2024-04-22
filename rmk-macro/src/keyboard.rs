use darling::FromMeta;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use rmk_config::toml_config::KeyboardTomlConfig;
use std::fs;
use syn::ItemMod;

use crate::{
    bind_interrupt::expand_bind_interrupt, chip_init::expand_chip_init, comm::expand_usb_init, entry::expand_rmk_entry, flash::expand_flash_init, import::expand_imports, keyboard_config::{
        expand_keyboard_info, expand_vial_config, get_chip_model, get_communication_type,
    }, light::expand_light_config, matrix::expand_matrix_config, ChipModel, ChipSeries
};

/// List of functions that can be overwritten
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommunicationType {
    Usb,
    Ble,
    Both,
    None,
}

/// List of functions that can be overwritten
#[derive(Debug, Clone, Copy, FromMeta)]
pub enum Overwritten {
    Usb,
    ChipConfig,
}

/// Parse keyboard mod and generate a valid RMK main function with all needed code
pub(crate) fn parse_keyboard_mod(attr: proc_macro::TokenStream, item_mod: ItemMod) -> TokenStream2 {
    // Read keyboard config file at project root
    let s = match fs::read_to_string("keyboard.toml") {
        Ok(s) => s,
        Err(e) => {
            let msg = format!("Read keyboard config file `keyboard.toml` error: {}", e);
            return syn::Error::new_spanned::<TokenStream2, String>(attr.into(), msg)
                .to_compile_error()
                .into();
        }
    };
    // Parse keyboard config file content to `KeyboardTomlConfig`
    let toml_config: KeyboardTomlConfig = match toml::from_str(&s) {
        Ok(c) => c,
        Err(e) => {
            let msg = format!("Parse `keyboard.toml` error: {}", e.message());
            return syn::Error::new_spanned::<TokenStream2, String>(attr.into(), msg)
                .to_compile_error()
                .into();
        }
    };

    // Generate code from toml config
    let chip = get_chip_model(toml_config.keyboard.chip.clone());
    if chip.series == ChipSeries::Unsupported {
        return quote! {
            compile_error!("Unsupported chip series, please check `chip` field in `keyboard.toml`");
        }
        .into();
    }

    let comm_type = get_communication_type(&toml_config.keyboard, &toml_config.ble);
    if comm_type == CommunicationType::None {
        return quote! {
            compile_error!("You must enable at least one of usb or ble");
        }
        .into();
    }

    // Create keyboard info and vial struct
    let keyboard_info_static_var = expand_keyboard_info(
        toml_config.keyboard.clone(),
        toml_config.matrix.rows as usize,
        toml_config.matrix.cols as usize,
        toml_config.matrix.layers as usize,
    );
    let vial_static_var = expand_vial_config();

    // TODO: 2. Generate main function
    let main_function = expand_main(&chip, comm_type, toml_config, item_mod);

    // TODO: 3. Insert customization code

    quote! {
        use defmt::*;
        use defmt_rtt as _;
        use panic_probe as _;

        #keyboard_info_static_var
        #vial_static_var

        #main_function
    }
}

fn expand_main(
    chip: &ChipModel,
    comm_type: CommunicationType,
    toml_config: KeyboardTomlConfig,
    item_mod: ItemMod,
) -> TokenStream2 {
    // Expand components of main function
    let imports = expand_imports(&item_mod);
    let bind_interrupt = expand_bind_interrupt(&item_mod);
    let chip_init = expand_chip_init(&chip, &item_mod);
    let usb_init = expand_usb_init(&chip, &item_mod);
    let flash_init = expand_flash_init(&chip, comm_type, toml_config.storage);
    let light_config = expand_light_config(&chip, toml_config.light);
    let matrix_config = expand_matrix_config(&chip, toml_config.matrix);
    let run_rmk = expand_rmk_entry(&chip, comm_type);
    // TODO: Add ble battery config

    quote! {
        #imports

        #bind_interrupt

        #[::embassy_executor::main]
        async fn main(_spawner: ::embassy_executor::Spawner) {
            ::defmt::info!("RMK start!");
            // Initialize peripherals as `p`
            #chip_init

            // Usb config needs at most 3 inputs from users chip, which cannot be automatically extracted:
            // 1. USB Interrupte name
            // 2. USB periphral name
            // 3. USB GPIO
            // Users have to implement the usb initialization function if the built-in func cannot 
            // Initialize usb driver as `driver`
            #usb_init

            // FIXME: if storage is enabled
            // Initialize flash driver as `flash` and storage config as `storage_config`
            #flash_init

            // Initialize light config as `light_config`
            #light_config;

            // Initialize matrix config as `(input_pins, output_pins)`
            #matrix_config;

            // Set all keyboard config
            let keyboard_config = ::rmk::config::RmkConfig {
                usb_config: keyboard_usb_config,
                vial_config,
                light_config,
                storage_config,
                ..Default::default()
            };

            // Start serving
            #run_rmk
        }
    }
}
