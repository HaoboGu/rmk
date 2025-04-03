use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use serde::Deserialize;
use std::fs;

use crate::config::{
    BehaviorConfig, BleConfig, DependencyConfig, InputDeviceConfig, KeyboardInfo,
    KeyboardTomlConfig, LayoutConfig, LightConfig, MatrixConfig, MatrixType, SplitConfig,
    StorageConfig,
};
use crate::{
    default_config::{
        esp32::default_esp32, nrf52810::default_nrf52810, nrf52832::default_nrf52832,
        nrf52840::default_nrf52840, rp2040::default_rp2040, stm32::default_stm32,
    },
    usb_interrupt_map::{get_usb_info, UsbInfo},
    ChipModel, ChipSeries,
};

macro_rules! rmk_compile_error {
    ($msg:expr) => {
        Err(syn::Error::new_spanned(quote! {}, $msg).to_compile_error())
    };
}

// Max number of combos
pub const COMBO_MAX_NUM: usize = 8;
// Max size of combos
pub const COMBO_MAX_LENGTH: usize = 4;

// Max number of forks
pub const FORK_MAX_NUM: usize = 16;

/// Keyboard's basic info
#[allow(unused)]
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Basic {
    /// Keyboard name
    pub name: String,
    /// Vender id
    pub vendor_id: u16,
    /// Product id
    pub product_id: u16,
    /// Manufacturer
    pub manufacturer: String,
    /// Product name
    pub product_name: String,
    /// Serial number
    pub serial_number: String,
}

impl Default for Basic {
    fn default() -> Self {
        Self {
            name: "RMK Keyboard".to_string(),
            vendor_id: 0x4c4b,
            product_id: 0x4643,
            manufacturer: "RMK".to_string(),
            product_name: "RMK Keyboard".to_string(),
            serial_number: "vial:f64c2b3c:000001".to_string(),
        }
    }
}

/// Keyboard config is a bridge representation between toml_config and generated keyboard cofiguration
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

#[derive(Clone, Debug)]
pub(crate) enum BoardConfig {
    Split(SplitConfig),
    UniBody(UniBodyConfig),
}

#[derive(Clone, Debug, Default)]
pub(crate) struct UniBodyConfig {
    pub(crate) matrix: MatrixConfig,
    pub(crate) input_device: InputDeviceConfig,
}

impl Default for BoardConfig {
    fn default() -> Self {
        BoardConfig::UniBody(UniBodyConfig::default())
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) enum CommunicationConfig {
    // USB only, no need for specific config
    Usb(UsbInfo),
    // BLE only
    Ble(BleConfig),
    // Both USB and BLE
    Both(UsbInfo, BleConfig),
    #[default]
    None,
}

impl CommunicationConfig {
    pub(crate) fn ble_enabled(&self) -> bool {
        match self {
            CommunicationConfig::Ble(_) | CommunicationConfig::Both(_, _) => true,
            _ => false,
        }
    }

    pub(crate) fn usb_enabled(&self) -> bool {
        match self {
            CommunicationConfig::Usb(_) | CommunicationConfig::Both(_, _) => true,
            _ => false,
        }
    }

    pub(crate) fn get_ble_config(&self) -> Option<BleConfig> {
        match self {
            CommunicationConfig::Ble(ble_config) | CommunicationConfig::Both(_, ble_config) => {
                Some(ble_config.clone())
            }
            _ => None,
        }
    }

    pub(crate) fn get_usb_info(&self) -> Option<UsbInfo> {
        match self {
            CommunicationConfig::Usb(usb_info) | CommunicationConfig::Both(usb_info, _) => {
                Some(usb_info.clone())
            }
            _ => None,
        }
    }
}

impl KeyboardConfig {
    pub(crate) fn new(toml_config: KeyboardTomlConfig) -> Result<Self, TokenStream2> {
        let chip = Self::get_chip_model(&toml_config)?;

        // Get per-chip configuration
        let mut config = Self::get_default_config(chip)?;

        // Then update all the configurations from toml

        // Update communication config
        config.communication = Self::get_communication_config(
            config.communication,
            toml_config.ble,
            toml_config.keyboard.usb_enable.clone(),
            &config.chip,
        )?;

        // Update basic info
        config.basic = Self::get_basic_info(config.basic, toml_config.keyboard);

        // Board config
        config.board = Self::get_board_config(
            toml_config.matrix,
            toml_config.split,
            toml_config.input_device,
        )?;

        // Layout config
        config.layout = Self::get_layout_from_toml(toml_config.layout)?;

        // Behavior config
        config.behavior =
            Self::get_behavior_from_toml(config.behavior, toml_config.behavior, &config.layout)?;

        // Light config
        config.light = Self::get_light_from_toml(config.light, toml_config.light);

        // Storage config
        config.storage = Self::get_storage_from_toml(config.storage, toml_config.storage);

        // Dependency config
        config.dependency = toml_config.dependency.unwrap_or_default();

        Ok(config)
    }

    /// Read chip model from toml config.
    ///
    /// The chip model can be either configured to a board or a microcontroller chip.
    pub(crate) fn get_chip_model(config: &KeyboardTomlConfig) -> Result<ChipModel, TokenStream2> {
        if config.keyboard.board.is_none() == config.keyboard.chip.is_none() {
            let message = format!(
                "Either \"board\" or \"chip\" should be set in keyboard.toml, but not both"
            );
            return rmk_compile_error!(message);
        }

        // Check board first
        let chip = if let Some(board) = config.keyboard.board.clone() {
            match board.as_str() {
                "nice!nano" | "nice!nano_v2" | "XIAO BLE" => Some(ChipModel {
                    series: ChipSeries::Nrf52,
                    chip: "nrf52840".to_string(),
                    board: Some(board.clone()),
                }),
                _ => None,
            }
        } else if let Some(chip) = config.keyboard.chip.clone() {
            if chip.to_lowercase().starts_with("stm32") {
                Some(ChipModel {
                    series: ChipSeries::Stm32,
                    chip,
                    board: None,
                })
            } else if chip.to_lowercase().starts_with("nrf52") {
                Some(ChipModel {
                    series: ChipSeries::Nrf52,
                    chip,
                    board: None,
                })
            } else if chip.to_lowercase().starts_with("rp2040") {
                Some(ChipModel {
                    series: ChipSeries::Rp2040,
                    chip,
                    board: None,
                })
            } else if chip.to_lowercase().starts_with("esp32") {
                Some(ChipModel {
                    series: ChipSeries::Esp32,
                    chip,
                    board: None,
                })
            } else {
                None
            }
        } else {
            None
        };

        chip.ok_or(
            syn::Error::new_spanned::<TokenStream2, String>(
                "".parse().unwrap(),
                "Given \"board\" or \"chip\" is not supported".to_string(),
            )
            .to_compile_error(),
        )
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
                let message = format!("No default chip config for {}, please report at https://github.com/HaoboGu/rmk/issues", chip.chip);
                return rmk_compile_error!(message);
            }
        };

        Ok(config)
    }

    fn get_basic_info(default: Basic, toml: KeyboardInfo) -> Basic {
        Basic {
            name: toml.name,
            vendor_id: toml.vendor_id,
            product_id: toml.product_id,
            manufacturer: toml.manufacturer.unwrap_or(default.manufacturer),
            product_name: toml.product_name.unwrap_or(default.product_name),
            serial_number: toml.serial_number.unwrap_or(default.serial_number),
        }
    }

    fn get_communication_config(
        default_setting: CommunicationConfig,
        ble_config: Option<BleConfig>,
        usb_enabled: Option<bool>,
        chip: &ChipModel,
    ) -> Result<CommunicationConfig, TokenStream2> {
        // Get usb config
        let usb_enabled = { usb_enabled.unwrap_or(default_setting.usb_enabled()) };
        let usb_info = if usb_enabled {
            get_usb_info(&chip.chip)
        } else {
            None
        };

        // Get ble config
        let ble_config = match (default_setting, ble_config) {
            (CommunicationConfig::Ble(default), None)
            | (CommunicationConfig::Both(_, default), None) => Some(default),
            (CommunicationConfig::Ble(default), Some(mut config))
            | (CommunicationConfig::Both(_, default), Some(mut config)) => {
                // Use default setting if the corresponding field is not set
                config.battery_adc_pin = config.battery_adc_pin.or(default.battery_adc_pin);
                config.charge_state = config.charge_state.or(default.charge_state);
                config.charge_led = config.charge_led.or(default.charge_led);
                config.adc_divider_measured =
                    config.adc_divider_measured.or(default.adc_divider_measured);
                config.adc_divider_total = config.adc_divider_total.or(default.adc_divider_total);
                Some(config)
            }
            (_, c) => c,
        };

        match (usb_info, ble_config) {
            (Some(usb_info), None) => Ok(CommunicationConfig::Usb(usb_info)),
            (Some(usb_info), Some(ble_config)) => {
                if !ble_config.enabled {
                    Ok(CommunicationConfig::Usb(usb_info))
                } else {
                    Ok(CommunicationConfig::Both(usb_info, ble_config))
                }
            }
            (None, Some(c)) => {
                if !c.enabled {
                    rmk_compile_error!("You must enable at least one of usb or ble".to_string())
                } else {
                    Ok(CommunicationConfig::Ble(c))
                }
            }
            _ => rmk_compile_error!("You must enable at least one of usb or ble".to_string()),
        }
    }

    fn get_board_config(
        matrix: Option<MatrixConfig>,
        split: Option<SplitConfig>,
        input_device: Option<InputDeviceConfig>,
    ) -> Result<BoardConfig, TokenStream2> {
        match (matrix, split) {
            (None, Some(s)) => {
                Ok(BoardConfig::Split(s))
            },
            (Some(m), None) => {
                match m.matrix_type {
                    MatrixType::normal => {
                        if m.input_pins == None || m.output_pins == None {
                            rmk_compile_error!("`input_pins` and `output_pins` is required for normal matrix".to_string())
                        } else {
                            Ok(())
                        }
                    },
                    MatrixType::direct_pin => {
                        if m.direct_pins == None {
                            rmk_compile_error!("`direct_pins` is required for direct pin matrix".to_string())
                        } else {
                            Ok(())
                        }
                    },
                }?;
                Ok(BoardConfig::UniBody(UniBodyConfig{matrix: m, input_device: input_device.unwrap_or(InputDeviceConfig::default())}))
            },
            (None, None) => rmk_compile_error!("[matrix] section in keyboard.toml is required for non-split keyboard".to_string()),
            _ => rmk_compile_error!("Use at most one of [matrix] or [split] in your keyboard.toml!\n-> [matrix] is used to define a normal matrix of non-split keyboard\n-> [split] is used to define a split keyboard\n".to_string()),
        }
    }

    // Layout is a mandatory field in toml, so we mainly check the sizes
    fn get_layout_from_toml(mut layout: LayoutConfig) -> Result<LayoutConfig, TokenStream2> {
        if layout.keymap.len() <= layout.layers as usize {
            // The required number of layers is less than what's set in keymap
            // Fill the rest with empty keys
            for _ in layout.keymap.len()..layout.layers as usize {
                // Add 2D vector of empty keys
                layout.keymap.push(vec![
                    vec!["_".to_string(); layout.cols as usize];
                    layout.rows as usize
                ]);
            }
        } else {
            return rmk_compile_error!(
                "keyboard.toml: Layer number in keymap is larger than [layout.layers]".to_string()
            );
        }

        // Row
        if let Some(_) = layout
            .keymap
            .iter()
            .map(|r| r.len())
            .find(|l| *l as u8 != layout.rows)
        {
            return rmk_compile_error!(
                "keyboard.toml: Row number in keymap doesn't match with [layout.row]".to_string()
            );
        }
        // Col
        if let Some(_) = layout
            .keymap
            .iter()
            .filter_map(|r| r.iter().map(|c| c.len()).find(|l| *l as u8 != layout.cols))
            .next()
        {
            // Find a row whose col num is wrong
            return rmk_compile_error!(
                "keyboard.toml: Col number in keymap doesn't match with [layout.col]".to_string()
            );
        }

        Ok(layout)
    }

    fn get_behavior_from_toml(
        default: BehaviorConfig,
        toml: Option<BehaviorConfig>,
        layout: &LayoutConfig,
    ) -> Result<BehaviorConfig, TokenStream2> {
        match toml {
            Some(mut behavior) => {
                // Use default setting if the corresponding field is not set
                behavior.tri_layer = match behavior.tri_layer {
                    Some(tri_layer) => {
                        if tri_layer.upper >= layout.layers {
                            return rmk_compile_error!(
                                "keyboard.toml: Tri layer upper is larger than [layout.layers]"
                            );
                        } else if tri_layer.lower >= layout.layers {
                            return rmk_compile_error!(
                                "keyboard.toml: Tri layer lower is larger than [layout.layers]"
                            );
                        } else if tri_layer.adjust >= layout.layers {
                            return rmk_compile_error!(
                                "keyboard.toml: Tri layer adjust is larger than [layout.layers]"
                            );
                        }
                        Some(tri_layer)
                    }
                    None => default.tri_layer,
                };

                behavior.tap_hold = behavior.tap_hold.or(default.tap_hold);
                behavior.one_shot = behavior.one_shot.or(default.one_shot);

                behavior.combo = behavior.combo.or(default.combo);
                if let Some(combo) = &behavior.combo {
                    if combo.combos.len() > COMBO_MAX_NUM {
                        return rmk_compile_error!(format!("keyboard.toml: number of combos is greater than [behavior.combo.max_num]"));
                    }

                    for (i, c) in combo.combos.iter().enumerate() {
                        if c.actions.len() > COMBO_MAX_LENGTH {
                            return rmk_compile_error!(format!("keyboard.toml: number of keys in combo #{i} is greater than [behavior.combo.max_length]"));
                        }

                        if let Some(layer) = c.layer {
                            if layer >= layout.layers {
                                return rmk_compile_error!(format!("keyboard.toml: layer in combo #{i} is greater than [layout.layers]"));
                            }
                        }
                    }
                }

                behavior.fork = behavior.fork.or(default.fork);
                if let Some(fork) = &behavior.fork {
                    if fork.forks.len() > FORK_MAX_NUM {
                        return rmk_compile_error!(format!("keyboard.toml: number of forks is greater than [behavior.fork.max_num]"));
                    }
                }

                Ok(behavior)
            }
            None => Ok(default),
        }
    }

    fn get_light_from_toml(default: LightConfig, toml: Option<LightConfig>) -> LightConfig {
        match toml {
            Some(mut light_config) => {
                // Use default setting if the corresponding field is not set
                light_config.capslock = light_config.capslock.or(default.capslock);
                light_config.numslock = light_config.numslock.or(default.numslock);
                light_config.scrolllock = light_config.scrolllock.or(default.scrolllock);
                light_config
            }
            None => default,
        }
    }

    fn get_storage_from_toml(default: StorageConfig, toml: Option<StorageConfig>) -> StorageConfig {
        if let Some(mut storage) = toml {
            // Use default setting if the corresponding field is not set
            storage.start_addr = storage.start_addr.or(default.start_addr);
            storage.num_sectors = storage.num_sectors.or(default.num_sectors);
            storage.clear_storage = storage.clear_storage.or(default.clear_storage);
            storage
        } else {
            default
        }
    }
}

pub(crate) fn read_keyboard_toml_config() -> Result<KeyboardTomlConfig, TokenStream2> {
    // Read keyboard config file at project root
    let s = match fs::read_to_string("keyboard.toml") {
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
            return rmk_compile_error!(msg);
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

    quote! {
        pub(crate) const COL: usize = #num_col;
        pub(crate) const ROW: usize = #num_row;
        pub(crate) const NUM_LAYER: usize = #num_layer;
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
