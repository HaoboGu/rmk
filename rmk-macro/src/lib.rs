mod gpio_str;

use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use rmk_config::{
    self,
    toml_config::{KeyboardInfo, KeyboardTomlConfig, LightConfig},
};
use std::fs;
use syn::parse_macro_input;

use crate::gpio_str::convert_gpio_str_to_pin;

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
    let serial_number = keyboard_info.serial_number.unwrap_or("0".to_string());
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

fn expand_light_config(chip: ChipSeries, light_config: LightConfig) -> proc_macro2::TokenStream {
    let numslock = match light_config.numslock {
        Some(c) => convert_gpio_str_to_pin(&chip, c.pin),
        None => quote! {None},
    };
    let capslock = match light_config.capslock {
        Some(c) => convert_gpio_str_to_pin(&chip, c.pin),
        None => quote! {None},
    };
    let scrolllock = match light_config.scrolllock {
        Some(c) => convert_gpio_str_to_pin(&chip, c.pin),
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

#[proc_macro_attribute]
pub fn rmk_main(attr: TokenStream, item: TokenStream) -> TokenStream {
    let s = match fs::read_to_string("keyboard.toml") {
        Ok(s) => s,
        Err(e) => {
            let msg = format!("Read keyboard config file `keyboard.toml` error: {}", e);
            return syn::Error::new_spanned::<proc_macro2::TokenStream, String>(attr.into(), msg)
                .to_compile_error()
                .into();
        }
    };
    let c: KeyboardTomlConfig = match toml::from_str(&s) {
        Ok(c) => c,
        Err(e) => {
            let msg = format!("Parse `keyboard.toml` error: {}", e.message());
            return syn::Error::new_spanned::<proc_macro2::TokenStream, String>(attr.into(), msg)
                .to_compile_error()
                .into();
        }
    };

    let chip = get_chip_model(c.keyboard.chip.clone());
    let keyboard_info_static_var = expand_keyboard_info(c.keyboard);
    let vial_static_var = expand_vial_config();
    let light_config_var = expand_light_config(chip, c.light);
    eprintln!("{}", light_config_var.to_string());
    let f = parse_macro_input!(item as syn::ItemFn);
    quote! {
        #keyboard_info_static_var
        #vial_static_var
        #light_config_var

        #f
    }
    .into()
}
