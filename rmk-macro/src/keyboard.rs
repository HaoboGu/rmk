use darling::FromMeta;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use rmk_config::toml_config::KeyboardTomlConfig;
use syn::ItemMod;

use crate::{
    bind_interrupt::expand_bind_interrupt,
    ble::expand_ble_config,
    chip_init::expand_chip_init,
    comm::expand_usb_init,
    entry::expand_rmk_entry,
    feature::{get_rmk_features, is_feature_enabled},
    flash::expand_flash_init,
    import::expand_imports,
    keyboard_config::{
        expand_keyboard_info, expand_vial_config, get_chip_model, get_communication_type,
        read_keyboard_config,
    },
    layout::expand_layout_init,
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

impl CommunicationType {
    pub(crate) fn usb_enabled(&self) -> bool {
        match self {
            CommunicationType::Both | CommunicationType::Usb => true,
            _ => false,
        }
    }

    pub(crate) fn ble_enabled(&self) -> bool {
        match self {
            CommunicationType::Both | CommunicationType::Ble => true,
            _ => false,
        }
    }
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
    let rmk_features = get_rmk_features();
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

    // Expanded main function
    let main_function = expand_main(
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

pub(crate) fn gen_imports(toml_config: &KeyboardTomlConfig, chip: &ChipModel) -> TokenStream2 {
    // Create layout info
    let layout = expand_layout_init(toml_config.layout.clone(), toml_config.matrix.clone());
    // Create keyboard info and vial struct
    let keyboard_info_static_var = expand_keyboard_info(
        toml_config.keyboard.clone(),
        toml_config.matrix.rows as usize,
        toml_config.matrix.cols as usize,
        toml_config.matrix.layers as usize,
    );

    // Create vial config
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

    quote! {
        use defmt::*;
        #no_std_imports

        #keyboard_info_static_var
        #vial_static_var
        #layout
    }
}

pub(crate) fn get_chip_info(
    toml_config: &KeyboardTomlConfig,
) -> Result<(ChipModel, CommunicationType, UsbInfo), TokenStream2> {
    let chip = get_chip_model(toml_config.keyboard.chip.clone());
    let chip = match chip {
        Some(c) => c,
        None => return Err(quote! {
            compile_error!("Unsupported chip series, please check `chip` field in `keyboard.toml`");
        }
        .into()),
    };

    let comm_type = get_communication_type(&toml_config.keyboard, &toml_config.ble);
    if comm_type == CommunicationType::None {
        return Err(quote! {
            compile_error!("You must enable at least one of usb or ble");
        }
        .into());
    }
    if !chip.has_usb() && comm_type.usb_enabled() {
        return Err(quote! {
            compile_error!("The chip specified in `keyboard.toml` doesn't have a general USB peripheral, please check again. Or you can set `usb_enable = false` in `[keyboard]` section of `keyboard.toml`");
        }
        .into());
    }
    let usb_info = if comm_type.usb_enabled() {
        if let Some(usb_info) = get_usb_info(&chip.chip.to_lowercase()) {
            usb_info
        } else {
            return Err(quote! {
                compile_error!("Unsupported chip model, please check `chip` field in `keyboard.toml` is a valid. For stm32, it should be a feature gate of `embassy-stm32`");
            }
            .into());
        }
    } else {
        UsbInfo::new_default(&chip)
    };
    Ok((chip, comm_type, usb_info))
}

fn expand_main(
    chip: &ChipModel,
    comm_type: CommunicationType,
    usb_info: UsbInfo,
    toml_config: KeyboardTomlConfig,
    item_mod: ItemMod,
    async_matrix: bool,
) -> TokenStream2 {
    // Expand components of main function
    let imports = expand_imports(&item_mod);
    let bind_interrupt = expand_bind_interrupt(&chip, &usb_info, &toml_config, &item_mod);
    let chip_init = expand_chip_init(&chip, &item_mod);
    let usb_init = expand_usb_init(&chip, &usb_info, comm_type, &item_mod);
    let flash_init = expand_flash_init(&chip, comm_type, toml_config.storage);
    let light_config = expand_light_config(&chip, toml_config.light);
    let matrix_config = expand_matrix_config(&chip, toml_config.matrix, async_matrix);
    let run_rmk = expand_rmk_entry(&chip, comm_type, &item_mod);
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

            // Start serving
            #run_rmk
        }
    }
}
