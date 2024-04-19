use std::fs;

use crate::keyboard_config::{
    expand_keyboard_info, expand_light_config, expand_matrix_config, expand_vial_config,
    get_chip_model, ChipSeries,
};
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use rmk_config::toml_config::KeyboardTomlConfig;
use syn::ItemMod;

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
    if chip == ChipSeries::Unsupported {
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
    let main_function = expand_main(&chip, toml_config);

    // TODO: 3. Insert customization code

    quote! {
        #imports

        #keyboard_info_static_var
        #vial_static_var

        #main_function
    }
}

fn get_interrupt_binding(chip: &ChipSeries, chip_name: String) -> TokenStream2 {
    // FIXME: The interrupt bindings varies for chips, it's impossible now to automatically set it
    // Leave it to users for now, until there's better solution
    match chip {
        ChipSeries::Stm32 => {

        }
        ChipSeries::Nrf52 => todo!(),
        ChipSeries::Rp2040 => todo!(),
        ChipSeries::Esp32 => todo!(),
        ChipSeries::Unsupported => todo!(),
    }
    quote! {
        use embassy_stm32::bind_interrupts;
        bind_interrupts!(struct Irqs {
            OTG_HS => InterruptHandler<USB_OTG_HS>;
        });
    }
}

fn expand_main(chip: &ChipSeries, toml_config: KeyboardTomlConfig) -> TokenStream2 {
    let light_config = expand_light_config(&chip, toml_config.light);
    let matrix_config = expand_matrix_config(&chip, toml_config.matrix);

    quote! {
        #[::embassy_executor::main]
        async fn main(_spawner: ::embassy_executor::Spawner) {
            info!("RMK start!");
            let mut config = Config::default();
            // Initialize peripherals
            let p = embassy_stm32::init(config);

            // Usb config
            // FIXME: usb initialization (with interrupt binding)
            // It needs 3 inputs from users chip,which cannot be automatically extracted:
            // 1. USB Interrupte name
            // 2. USB periphral name
            // 3. USB GPIO 
            // So, I'll leave it to users, make a stub function here
            static EP_OUT_BUFFER: StaticCell<[u8; 1024]> = StaticCell::new();
            let mut usb_config = embassy_stm32::usb_otg::Config::default();
            usb_config.vbus_detection = false;
            let driver = Driver::new_fs(
                p.USB_OTG_HS,
                Irqs,
                p.PA12,
                p.PA11,
                &mut EP_OUT_BUFFER.init([0; 1024])[..],
                usb_config,
            );

            // FIXME: if storage is enabled
            // Use internal flash to emulate eeprom
            let f = Flash::new_blocking(p.FLASH);

            // FIXME: FIX macro
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

fn get_imports(chip: &ChipSeries) -> TokenStream2 {
    let chip_specific_imports = match chip {
        ChipSeries::Stm32 => {
            quote! {
                // TODO: different imports by chip_name
                use embassy_stm32::{
                    flash::{Blocking, Flash},
                    gpio::{AnyPin, Input, Output},
                    peripherals::USB_OTG_HS,
                    time::Hertz,
                    usb_otg::{Driver, InterruptHandler},
                    Config,
                };
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
