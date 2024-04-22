use darling::FromMeta;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use rmk_config::toml_config::KeyboardTomlConfig;
use std::fs;
use syn::ItemMod;

use crate::{
    bind_interrupt::expand_bind_interrupt, chip_init::expand_chip_init, comm::expand_usb_init, import::expand_imports, keyboard_config::{expand_keyboard_info, expand_vial_config, get_chip_model}, light::expand_light_config, matrix::expand_matrix_config, ChipModel, ChipSeries
};

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

    // Create keyboard info and vial struct
    let keyboard_info_static_var = expand_keyboard_info(
        toml_config.keyboard.clone(),
        toml_config.matrix.rows as usize,
        toml_config.matrix.cols as usize,
        toml_config.matrix.layers as usize,
    );
    let vial_static_var = expand_vial_config();

    // TODO: 2. Generate main function
    let imports = get_imports(&chip);
    let main_function = expand_main(&chip, toml_config, item_mod);

    // TODO: 3. Insert customization code

    quote! {
        #imports
        #keyboard_info_static_var
        #vial_static_var

        #main_function
    }
}


fn expand_main(
    chip: &ChipModel,
    toml_config: KeyboardTomlConfig,
    item_mod: ItemMod,
) -> TokenStream2 {
    // Expand components of main function
    let light_config = expand_light_config(&chip, toml_config.light);
    let matrix_config = expand_matrix_config(&chip, toml_config.matrix);
    let usb_init = expand_usb_init(&chip, &item_mod);
    let chip_init = expand_chip_init(&chip, &item_mod);
    let imports = expand_imports(&item_mod);
    let bind_interrupt = expand_bind_interrupt(&item_mod);
    quote! {
        #imports

        #bind_interrupt

        #[::embassy_executor::main]
        async fn main(_spawner: ::embassy_executor::Spawner) {
            info!("RMK start!");
            // Initialize peripherals
            #chip_init

            // Usb config
            // FIXME: usb initialization (with interrupt binding)
            // It needs 3 inputs from users chip,which cannot be automatically extracted:
            // 1. USB Interrupte name
            // 2. USB periphral name
            // 3. USB GPIO
            // So, I'll leave it to users, make a stub function here
            #usb_init

            // FIXME: if storage is enabled
            // Use internal flash to emulate eeprom
            let f = Flash::new_blocking(p.FLASH);

            let light_config = #light_config;
            let (input_pins, output_pins) = #matrix_config;

            let keyboard_config = RmkConfig {
                usb_config: keyboard_usb_config,
                vial_config,
                light_config,
                ..Default::default()
            };

            // Start serving
            initialize_keyboard_with_config_and_run::<
                Flash<'_, Blocking>,
                Driver<'_, USB_OTG_HS>,
                Input<'_, AnyPin>,
                Output<'_, AnyPin>,
                ROW,
                COL,
                NUM_LAYER,
            >(
                driver,
                input_pins,
                output_pins,
                Some(f),
                KEYMAP,
                keyboard_config,
            )
            .await;
        }
    }
}

fn get_imports(chip: &ChipModel) -> TokenStream2 {
    let chip_specific_imports = match chip.series {
        ChipSeries::Stm32 => {
            quote! {
                // TODO: different imports by chip_name

            }
        }
        ChipSeries::Nrf52 => todo!(),
        ChipSeries::Rp2040 => todo!(),
        ChipSeries::Esp32 => todo!(),
        ChipSeries::Unsupported => todo!(),
    };

    quote! {
        use defmt::*;
        use defmt_rtt as _;
        use panic_probe as _;
        use embassy_executor::Spawner;
        #chip_specific_imports
    }
}
