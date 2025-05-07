use std::fs;

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use rmk_config::{
    Basic, BehaviorConfig, BoardConfig, CommunicationConfig, DependencyConfig, KeyboardTomlConfig, LayoutConfig,
    LightConfig, StorageConfig,
};

use crate::default_config::esp32::default_esp32;
use crate::default_config::nrf52810::default_nrf52810;
use crate::default_config::nrf52832::default_nrf52832;
use crate::default_config::nrf52840::default_nrf52840;
use crate::default_config::rp2040::default_rp2040;
use crate::default_config::stm32::default_stm32;
use rmk_config::ChipModel;

macro_rules! rmk_compile_error {
    ($msg:expr) => {
        Err(syn::Error::new_spanned(quote! {}, $msg).to_compile_error())
    };
}

/// Keyboard config is a bridge representation between toml_config and generated keyboard configuration
/// This struct is added mainly for the following reasons:
/// 1. To make it easier to set per-chip default configuration
/// 2. To do pre-checking for all configs before generating code
///
/// This struct is constructed as the generated code, not toml_config
#[derive(Clone, Debug, Default)]
pub(crate) struct KeyboardConfig {
    // Keyboard basic info
    pub(crate) basic: Basic,
    // Communication config
    pub(crate) communication: CommunicationConfig,
    // Chip model
    pub(crate) chip: ChipModel,
    // Board config, normal or split
    pub(crate) board: BoardConfig,
    // Layout config
    pub(crate) layout: LayoutConfig,
    // Behavior Config
    pub(crate) behavior: BehaviorConfig,
    // Light config
    pub(crate) light: LightConfig,
    // Storage config
    pub(crate) storage: StorageConfig,
    // Dependency config
    pub(crate) dependency: DependencyConfig,
}

impl KeyboardConfig {
    pub(crate) fn new(toml_config: KeyboardTomlConfig) -> Result<Self, TokenStream2> {
        let chip = toml_config
            .get_chip_model()
            .map_err(|e| syn::Error::new_spanned(quote! {}, e).to_compile_error())?;

        // Get per-chip configuration
        let mut config = Self::get_default_config(chip.clone())?;

        // Then update all the configurations from toml

        // Update communication config
        config.communication = toml_config
            .get_communication_config(config.communication, &chip)
            .map_err(|e| syn::Error::new_spanned(quote! {}, e).to_compile_error())?;

        // Update basic info
        config.basic = toml_config.get_basic_info();

        // Board config
        config.board = toml_config
            .get_board_config()
            .map_err(|e| syn::Error::new_spanned(quote! {}, e).to_compile_error())?;

        // Layout config
        config.layout = toml_config
            .get_layout_from_toml()
            .map_err(|e| syn::Error::new_spanned(quote! {}, e).to_compile_error())?;

        // Behavior config
        config.behavior = toml_config
            .get_behavior_from_toml(config.behavior, &config.layout)
            .map_err(|e| syn::Error::new_spanned(quote! {}, e).to_compile_error())?;

        // Light config
        config.light = toml_config.get_light_from_toml(config.light);

        // Storage config
        config.storage = toml_config.get_storage_from_toml(config.storage);

        // Dependency config
        config.dependency = toml_config.dependency.unwrap_or_default();

        Ok(config)
    }

    // Get per-chip/board default configuration
    fn get_default_config(chip: ChipModel) -> Result<KeyboardConfig, TokenStream2> {
        if let Some(board) = chip.board.clone() {
            match board.as_str() {
                "nice!nano" | "nice!nano_v2" | "XIAO BLE" => {
                    return Ok(default_nrf52840(chip));
                }
                _ => (),
            }
        }

        let config = match chip.chip.as_str() {
            "nrf52840" | "nrf52833" => default_nrf52840(chip),
            "nrf52832" => default_nrf52832(chip),
            "nrf52810" | "nrf52811" => default_nrf52810(chip),
            "rp2040" => default_rp2040(chip),
            s if s.starts_with("stm32") => default_stm32(chip),
            s if s.starts_with("esp32") => default_esp32(chip),
            _ => {
                let message = format!(
                    "No default chip config for {}, please report at https://github.com/HaoboGu/rmk/issues",
                    chip.chip
                );
                return rmk_compile_error!(message);
            }
        };

        Ok(config)
    }
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

pub(crate) fn expand_keyboard_info(keyboard_config: &KeyboardConfig) -> proc_macro2::TokenStream {
    let pid = keyboard_config.basic.product_id;
    let vid = keyboard_config.basic.vendor_id;
    let product_name = keyboard_config.basic.product_name.clone();
    let manufacturer = keyboard_config.basic.manufacturer.clone();
    let serial_number = keyboard_config.basic.serial_number.clone();

    let num_col = keyboard_config.layout.cols as usize;
    let num_row = keyboard_config.layout.rows as usize;
    let num_layer = keyboard_config.layout.layers as usize;
    let num_encoder = match &keyboard_config.board {
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
