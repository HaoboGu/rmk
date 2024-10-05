use std::fs;

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use rmk_config::toml_config::{BleConfig, KeyboardInfo, KeyboardTomlConfig};

use crate::{keyboard::CommunicationType, ChipModel, ChipSeries};

pub(crate) fn read_keyboard_config(
    attr: proc_macro::TokenStream,
) -> Result<KeyboardTomlConfig, TokenStream2> {
    // Read keyboard config file at project root
    let s = match fs::read_to_string("keyboard.toml") {
        Ok(s) => s,
        Err(e) => {
            let msg = format!("Read keyboard config file `keyboard.toml` error: {}", e);
            return Err(
                syn::Error::new_spanned::<TokenStream2, String>(attr.into(), msg)
                    .to_compile_error()
                    .into(),
            );
        }
    };

    // Parse keyboard config file content to `KeyboardTomlConfig`
    match toml::from_str(&s) {
        Ok(c) => Ok(c),
        Err(e) => {
            let msg = format!("Parse `keyboard.toml` error: {}", e.message());
            return Err(
                syn::Error::new_spanned::<TokenStream2, String>(attr.into(), msg)
                    .to_compile_error()
                    .into(),
            );
        }
    }
}

pub(crate) fn get_communication_type(
    keyboard_config: &KeyboardInfo,
    ble_config: &Option<BleConfig>,
) -> CommunicationType {
    if keyboard_config.usb_enable
        && ble_config
            .clone()
            .is_some_and(|ble_config| ble_config.enabled)
    {
        CommunicationType::Both
    } else if keyboard_config.usb_enable {
        CommunicationType::Usb
    } else if ble_config
        .clone()
        .is_some_and(|ble_config| ble_config.enabled)
    {
        CommunicationType::Ble
    } else {
        CommunicationType::None
    }
}

pub(crate) fn get_chip_model(chip: String) -> Option<ChipModel> {
    if chip.to_lowercase().starts_with("stm32") {
        Some(ChipModel {
            series: ChipSeries::Stm32,
            chip,
        })
    } else if chip.to_lowercase().starts_with("nrf52") {
        Some(ChipModel {
            series: ChipSeries::Nrf52,
            chip,
        })
    } else if chip.to_lowercase().starts_with("rp2040") {
        Some(ChipModel {
            series: ChipSeries::Rp2040,
            chip,
        })
    } else if chip.to_lowercase().starts_with("esp32") {
        Some(ChipModel {
            series: ChipSeries::Esp32,
            chip,
        })
    } else {
        None
    }
}

pub(crate) fn expand_keyboard_info(
    keyboard_info: KeyboardInfo,
    num_row: usize,
    num_col: usize,
    num_layer: usize,
) -> proc_macro2::TokenStream {
    let pid = keyboard_info.product_id;
    let vid = keyboard_info.vendor_id;
    let product_name = keyboard_info
        .product_name
        .unwrap_or("RMK Keyboard".to_string());
    let manufacturer = keyboard_info.manufacturer.unwrap_or("RMK".to_string());
    let serial_number = keyboard_info.serial_number.unwrap_or("vial:f64c2b3c:000001".to_string());
    quote! {
        pub(crate) const COL: usize = #num_col;
        pub(crate) const ROW: usize = #num_row;
        pub(crate) const NUM_LAYER: usize = #num_layer;
        static keyboard_usb_config: ::rmk::config::keyboard_config::KeyboardUsbConfig = ::rmk::config::keyboard_config::KeyboardUsbConfig {
            vid: #vid,
            pid: #pid,
            manufacturer: #manufacturer,
            product_name: #product_name,
            serial_number: #serial_number,
        };
    }
}

pub(crate) fn expand_vial_config() -> proc_macro2::TokenStream {
    quote! {
        static vial_config: ::rmk::config::keyboard_config::VialConfig = ::rmk::config::keyboard_config::VialConfig {
            vial_keyboard_id: &VIAL_KEYBOARD_ID,
            vial_keyboard_def: &VIAL_KEYBOARD_DEF,
        };
    }
}
