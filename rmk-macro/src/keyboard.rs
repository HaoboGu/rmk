use cargo_toml::Manifest;
use darling::FromMeta;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use rmk_config::toml_config::KeyboardTomlConfig;
use std::fs;
use syn::ItemMod;

use crate::{
    bind_interrupt::expand_bind_interrupt,
    ble::expand_ble_config,
    chip_init::expand_chip_init,
    comm::expand_usb_init,
    entry::expand_rmk_entry,
    flash::expand_flash_init,
    import::expand_imports,
    keyboard_config::{
        expand_keyboard_info, expand_vial_config, get_chip_model, get_communication_type,
    },
    light::expand_light_config,
    matrix::expand_matrix_config,
    usb_interrupt_map::{get_usb_info, UsbInfo},
    ChipModel, ChipSeries,
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
    Entry,
}

/// Parse keyboard mod and generate a valid RMK main function with all needed code
pub(crate) fn parse_keyboard_mod(attr: proc_macro::TokenStream, item_mod: ItemMod) -> TokenStream2 {
    // TODO: Read Cargo.toml, get feature gate
    let enabled_rmk_features = match Manifest::from_path("Cargo.toml") {
        Ok(manifest) => manifest
            .dependencies
            .iter()
            .find(|(name, _dep)| *name == "rmk")
            .map(|(_name, dep)| dep.req_features().to_vec()),
        Err(_e) => None,
    };

    eprintln!("{:?}", enabled_rmk_features);

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

    // Get communication type: USB/BLE or BOTH
    let comm_type = get_communication_type(&toml_config.keyboard, &toml_config.ble);
    if comm_type == CommunicationType::None {
        return quote! {
            compile_error!("You must enable at least one of usb or ble");
        }
        .into();
    }

    // Check if the chip has usb
    if !chip.has_usb()
        && (comm_type == CommunicationType::Usb || comm_type == CommunicationType::Both)
    {
        return quote! {
            compile_error!("The chip specified in `keyboard.toml` doesn't have a general USB peripheral, please check again. Or you can set `usb_enable = false` in `[keyboard]` section of `keyboard.toml`");
        }
        .into();
    }

    // Get usb info from generated usb_interrupt_map.rs
    let usb_info = if comm_type == CommunicationType::Usb || comm_type == CommunicationType::Both {
        if let Some(usb_info) = get_usb_info(&chip.chip.to_lowercase()) {
            usb_info
        } else {
            return quote! {
                compile_error!("Unsupported chip model, please check `chip` field in `keyboard.toml` is a valid. For stm32, it should be a feature gate of `embassy-stm32`");
            }
            .into();
        }
    } else {
        UsbInfo::new_default(&chip)
    };

    // Create keyboard info and vial struct
    let keyboard_info_static_var = expand_keyboard_info(
        toml_config.keyboard.clone(),
        toml_config.matrix.rows as usize,
        toml_config.matrix.cols as usize,
        toml_config.matrix.layers as usize,
    );
    let vial_static_var = expand_vial_config();

    // If defmt_log is disabled, add an empty defmt logger impl
    let defmt_import = if toml_config.dependency.defmt_log {
        quote! {
            use defmt_rtt as _;
        }
    } else {
        quote! {
            #[::defmt::global_logger]
            struct Logger;

            unsafe impl ::defmt::Logger for Logger {
                fn acquire() {}
                unsafe fn flush() {}
                unsafe fn release() {}
                unsafe fn write(_bytes: &[u8]) {}
            }
        }
    };

    // For ESP32s, no panic handler and defmt logger are used
    let no_std_imports = if chip.series == ChipSeries::Esp32 {
        quote!()
    } else {
        quote! {
            use panic_probe as _;
            #defmt_import
        }
    };

    // Expanded main function
    let main_function = expand_main(&chip, comm_type, usb_info, toml_config, item_mod);

    quote! {
        use defmt::*;
        #no_std_imports

        #keyboard_info_static_var
        #vial_static_var

        #main_function
    }
}

fn expand_main(
    chip: &ChipModel,
    comm_type: CommunicationType,
    usb_info: UsbInfo,
    toml_config: KeyboardTomlConfig,
    item_mod: ItemMod,
) -> TokenStream2 {
    // Expand components of main function
    let imports = expand_imports(&item_mod);
    let bind_interrupt = expand_bind_interrupt(&chip, &usb_info, &toml_config, &item_mod);
    let chip_init = expand_chip_init(&chip, &item_mod);
    let usb_init = expand_usb_init(&chip, &usb_info, comm_type, &item_mod);
    let flash_init = expand_flash_init(&chip, comm_type, toml_config.storage);
    let light_config = expand_light_config(&chip, toml_config.light);
    let matrix_config = expand_matrix_config(&chip, toml_config.matrix);
    let run_rmk = expand_rmk_entry(&chip, &usb_info, comm_type, &item_mod);
    let ble_config = expand_ble_config(&chip, toml_config.ble);

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

            #ble_config

            // Set all keyboard config
            let keyboard_config = ::rmk::config::RmkConfig {
                usb_config: keyboard_usb_config,
                vial_config,
                light_config,
                storage_config,
                ble_battery_config,
                ..Default::default()
            };

            // Start serving
            #run_rmk
        }
    }
}
