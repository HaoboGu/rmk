mod gpio_str;

use proc_macro::TokenStream;
use quote::quote;
use rmk_config::{
    self,
    toml_config::{KeyboardInfo, KeyboardTomlConfig, LightConfig, MatrixConfig},
};
use std::fs;
use syn::parse_macro_input;

use crate::gpio_str::{
    convert_gpio_str_to_output_pin, convert_input_pins_to_initializers,
    convert_output_pins_to_initializers,
};

enum ChipSeries {
    Stm32,
    Nrf52,
    Rp2040,
    Esp32,
    Unsupported,
}

fn get_chip_model(chip: String) -> ChipSeries {
    if chip.to_lowercase().starts_with("stm32") {
        return ChipSeries::Stm32;
    } else if chip.to_lowercase().starts_with("nrf52") {
        return ChipSeries::Nrf52;
    } else if chip.to_lowercase().starts_with("rp2040") {
        return ChipSeries::Rp2040;
    } else if chip.to_lowercase().starts_with("esp32") {
        return ChipSeries::Esp32;
    } else {
        return ChipSeries::Unsupported;
    }
}

fn expand_keyboard_info(keyboard_info: KeyboardInfo) -> proc_macro2::TokenStream {
    let pid = keyboard_info.product_id;
    let vid = keyboard_info.vendor_id;
    let product_name = keyboard_info
        .product_name
        .unwrap_or("RMK Keyboard".to_string());
    let manufacturer = keyboard_info.manufacturer.unwrap_or("RMK".to_string());
    let serial_number = keyboard_info.serial_number.unwrap_or("0000000".to_string());
    quote! {
        static keyboard_usb_config: ::rmk_config::keyboard_config::KeyboardUsbConfig = ::rmk_config::keyboard_config::KeyboardUsbConfig {
            vid: #vid,
            pid: #pid,
            manufacturer: #manufacturer,
            product_name: #product_name,
            serial_number: #serial_number,
        };
    }
}

fn expand_vial_config() -> proc_macro2::TokenStream {
    quote! {
        static vial_config: ::rmk_config::keyboard_config::VialConfig = ::rmk_config::keyboard_config::VialConfig {
            vial_keyboard_id: &VIAL_KEYBOARD_ID,
            vial_keyboard_def: &VIAL_KEYBOARD_DEF,
        };
    }
}

fn expand_light_config(chip: &ChipSeries, light_config: LightConfig) -> proc_macro2::TokenStream {
    let numslock = match light_config.numslock {
        Some(c) => {
            let p = convert_gpio_str_to_output_pin(chip, c.pin);
            quote! {Some(#p)}
        }
        None => quote! {None},
    };
    let capslock = match light_config.capslock {
        Some(c) => {
            let p = convert_gpio_str_to_output_pin(chip, c.pin);
            quote! {Some(#p)}
        }
        None => quote! {None},
    };
    let scrolllock = match light_config.scrolllock {
        Some(c) => {
            let p = convert_gpio_str_to_output_pin(chip, c.pin);
            quote! {Some(#p)}
        }
        None => quote! {None},
    };

    quote! {
        macro_rules! config_light {
            (p: $p:ident) => {{
                let numslock_pin = #numslock;
                let capslock_pin = #capslock;
                let scrolllock_pin = #scrolllock;
                (numslock_pin, capslock_pin, scrolllock_pin)
            }};
        }
    }
}

fn expand_matrix_config(
    chip: &ChipSeries,
    matrix_config: MatrixConfig,
) -> proc_macro2::TokenStream {
    let num_col = matrix_config.cols as usize;
    let num_row = matrix_config.rows as usize;
    let num_layer = matrix_config.layers as usize;
    let mut final_tokenstream = proc_macro2::TokenStream::new();
    final_tokenstream.extend(convert_input_pins_to_initializers(
        &chip,
        matrix_config.input_pins,
    ));
    final_tokenstream.extend(convert_output_pins_to_initializers(
        &chip,
        matrix_config.output_pins,
    ));

    quote! {
        pub(crate) const COL: usize = #num_col;
        pub(crate) const ROW: usize = #num_row;
        pub(crate) const NUM_LAYER: usize = #num_layer;

        macro_rules! config_matrix {
            (p: $p:ident) => {{
                #final_tokenstream
                (output_pins, input_pins)
            }};
        }
    }
}

#[proc_macro_attribute]
pub fn rmk_main(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Read keyboard config file at project root
    let s = match fs::read_to_string("keyboard.toml") {
        Ok(s) => s,
        Err(e) => {
            let msg = format!("Read keyboard config file `keyboard.toml` error: {}", e);
            return syn::Error::new_spanned::<proc_macro2::TokenStream, String>(attr.into(), msg)
                .to_compile_error()
                .into();
        }
    };
    // Parse keyboard config file content to `KeyboardTomlConfig`
    let c: KeyboardTomlConfig = match toml::from_str(&s) {
        Ok(c) => c,
        Err(e) => {
            let msg = format!("Parse `keyboard.toml` error: {}", e.message());
            return syn::Error::new_spanned::<proc_macro2::TokenStream, String>(attr.into(), msg)
                .to_compile_error()
                .into();
        }
    };

    // Generate code from toml config
    let chip = get_chip_model(c.keyboard.chip.clone());
    let keyboard_info_static_var = expand_keyboard_info(c.keyboard);
    let vial_static_var = expand_vial_config();
    let light_config_macro = expand_light_config(&chip, c.light);
    let matrix_config_macro = expand_matrix_config(&chip, c.matrix);
    let f = parse_macro_input!(item as syn::ItemFn);
    quote! {
        #keyboard_info_static_var
        #vial_static_var
        #light_config_macro
        #matrix_config_macro

        #f
    }
    .into()
}
