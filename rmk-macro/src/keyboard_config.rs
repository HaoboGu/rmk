use std::fs;

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use rmk_config::{BoardConfig, KeyboardTomlConfig};
macro_rules! rmk_compile_error {
    ($msg:expr) => {
        Err(syn::Error::new_spanned(quote! {}, $msg).to_compile_error())
    };
}

pub(crate) fn read_keyboard_toml_config() -> Result<KeyboardTomlConfig, TokenStream2> {
    // Read keyboard config file at project root
    let config_toml_path = std::env::var("KEYBOARD_TOML_PATH")
        .expect("\x1b[1;31mERROR\x1b[0m: KEYBOARD_TOML_PATH should be set in `.cargo/config.toml`\n");

    let s = match fs::read_to_string(config_toml_path) {
        Ok(s) => s,
        Err(e) => {
            let msg = format!("Read keyboard config file `keyboard.toml` error: {}", e);
            return rmk_compile_error!(msg);
        }
    };

    // Parse keyboard config file content to `KeyboardTomlConfig`
    match toml::from_str(&s) {
        Ok(c) => Ok(c),
        Err(e) => {
            let msg = format!("Parse `keyboard.toml` error: {}", e.message());
            rmk_compile_error!(msg)
        }
    }
}

pub(crate) fn expand_keyboard_info(keyboard_config: &KeyboardTomlConfig) -> proc_macro2::TokenStream {
    let basic = keyboard_config.get_basic_info();
    let layout = keyboard_config.get_layout_config().unwrap();
    let board = keyboard_config.get_board_config().unwrap();
    let pid = basic.product_id;
    let vid = basic.vendor_id;
    let product_name = basic.product_name.clone();
    let manufacturer = basic.manufacturer.clone();
    let serial_number = basic.serial_number.clone();

    let num_col = layout.cols as usize;
    let num_row = layout.rows as usize;
    let num_layer = layout.layers as usize;
    let num_encoder = match &board {
        BoardConfig::Split(_split_config) => {
            // TODO: encoder config for split keyboard
            0
        }
        BoardConfig::UniBody(uni_body_config) => {
            uni_body_config.input_device.encoder.clone().unwrap_or(Vec::new()).len()
        }
    };
    quote! {
        pub(crate) const COL: usize = #num_col;
        pub(crate) const ROW: usize = #num_row;
        pub(crate) const NUM_LAYER: usize = #num_layer;
        pub(crate) const NUM_ENCODER: usize = #num_encoder;
        static KEYBOARD_USB_CONFIG: ::rmk::config::KeyboardUsbConfig = ::rmk::config::KeyboardUsbConfig {
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
        include!(concat!(env!("OUT_DIR"), "/config_generated.rs"));
        static VIAL_CONFIG: ::rmk::config::VialConfig = ::rmk::config::VialConfig {
            vial_keyboard_id: &VIAL_KEYBOARD_ID,
            vial_keyboard_def: &VIAL_KEYBOARD_DEF,
        };
    }
}
