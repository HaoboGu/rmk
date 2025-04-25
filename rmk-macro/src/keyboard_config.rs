use pest::Parser;
use pest_derive::Parser;

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;

use crate::config::{
    BehaviorConfig, BleConfig, DependencyConfig, InputDeviceConfig, KeyboardInfo, KeyboardTomlConfig, LayerTomlConfig,
    LayoutConfig, LayoutTomlConfig, LightConfig, MatrixConfig, MatrixType, SplitConfig, StorageConfig,
};
use crate::default_config::esp32::default_esp32;
use crate::default_config::nrf52810::default_nrf52810;
use crate::default_config::nrf52832::default_nrf52832;
use crate::default_config::nrf52840::default_nrf52840;
use crate::default_config::rp2040::default_rp2040;
use crate::default_config::stm32::default_stm32;
use crate::usb_interrupt_map::{get_usb_info, UsbInfo};
use crate::{ChipModel, ChipSeries};

// Pest parser using the grammar files
#[derive(Parser)]
#[grammar = "keymap.pest"]
struct ConfigParser;

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

// Max alias resolution depth to prevent infinite loops
const MAX_ALIAS_RESOLUTION_DEPTH: usize = 10;

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
        matches!(self, CommunicationConfig::Ble(_) | CommunicationConfig::Both(_, _))
    }

    pub(crate) fn usb_enabled(&self) -> bool {
        matches!(self, CommunicationConfig::Usb(_) | CommunicationConfig::Both(_, _))
    }

    pub(crate) fn get_ble_config(&self) -> Option<BleConfig> {
        match self {
            CommunicationConfig::Ble(ble_config) | CommunicationConfig::Both(_, ble_config) => Some(ble_config.clone()),
            _ => None,
        }
    }

    pub(crate) fn get_usb_info(&self) -> Option<UsbInfo> {
        match self {
            CommunicationConfig::Usb(usb_info) | CommunicationConfig::Both(usb_info, _) => Some(usb_info.clone()),
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
            toml_config.keyboard.usb_enable,
            &config.chip,
        )?;

        // Update basic info
        config.basic = Self::get_basic_info(config.basic, toml_config.keyboard);

        // Board config
        config.board = Self::get_board_config(toml_config.matrix, toml_config.split, toml_config.input_device)?;

        // Layout config
        config.layout = Self::get_layout_from_toml(
            toml_config.layout,
            toml_config.layer.unwrap_or_default(),
            toml_config.aliases.unwrap_or_default(),
        )?;

        // Behavior config
        config.behavior = Self::get_behavior_from_toml(config.behavior, toml_config.behavior, &config.layout)?;

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
            let message = "Either \"board\" or \"chip\" should be set in keyboard.toml, but not both";
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
                let message = format!(
                    "No default chip config for {}, please report at https://github.com/HaoboGu/rmk/issues",
                    chip.chip
                );
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
        let usb_info = if usb_enabled { get_usb_info(&chip.chip) } else { None };

        // Get ble config
        let ble_config = match (default_setting, ble_config) {
            (CommunicationConfig::Ble(default), None) | (CommunicationConfig::Both(_, default), None) => Some(default),
            (CommunicationConfig::Ble(default), Some(mut config))
            | (CommunicationConfig::Both(_, default), Some(mut config)) => {
                // Use default setting if the corresponding field is not set
                config.battery_adc_pin = config.battery_adc_pin.or(default.battery_adc_pin);
                config.charge_state = config.charge_state.or(default.charge_state);
                config.charge_led = config.charge_led.or(default.charge_led);
                config.adc_divider_measured = config.adc_divider_measured.or(default.adc_divider_measured);
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
                        if m.input_pins.is_none() |m.output_pins.is_none() {
                            rmk_compile_error!("`input_pins` and `output_pins` is required for normal matrix".to_string())
                        } else {
                            Ok(())
                        }
                    },
                    MatrixType::direct_pin => {
                        if m.direct_pins.is_none() {
                            rmk_compile_error!("`direct_pins` is required for direct pin matrix".to_string())
                        } else {
                            Ok(())
                        }
                    },
                }?;
                // FIXME: input device for split keyboard is not supported yet
                Ok(BoardConfig::UniBody(UniBodyConfig{matrix: m, input_device: input_device.unwrap_or_default()}))
            },
            (None, None) => rmk_compile_error!("[matrix] section in keyboard.toml is required for non-split keyboard".to_string()),
            _ => rmk_compile_error!("Use at most one of [matrix] or [split] in your keyboard.toml!\n-> [matrix] is used to define a normal matrix of non-split keyboard\n-> [split] is used to define a split keyboard\n".to_string()),
        }
    }

    /// Parses and validates a matrix_map string using Pest.
    /// Ensures the string contains only valid coordinates and whitespace.
    fn parse_matrix_map(matrix_map: &str) -> Result<Vec<(u8, u8)>, String> {
        match ConfigParser::parse(Rule::matrix_map, matrix_map) {
            Ok(pairs) => {
                let mut coordinates = Vec::new();
                // The top-level pair is 'matrix_map'. We need to iterate its inner content.
                for pair in pairs {
                    // Should only be one pair matching Rule::matrix_map
                    if pair.as_rule() == Rule::matrix_map {
                        for inner_pair in pair.into_inner() {
                            match inner_pair.as_rule() {
                                Rule::coordinate => {
                                    let mut coord_parts = inner_pair.into_inner(); // Should contain two 'number' pairs

                                    let row_str = coord_parts.next().ok_or("Missing row coordinate")?.as_str();
                                    let col_str = coord_parts.next().ok_or("Missing col coordinate")?.as_str();

                                    let row = row_str
                                        .parse::<u8>()
                                        .map_err(|e| format!("Failed to parse row '{}': {}", row_str, e))?;
                                    let col = col_str
                                        .parse::<u8>()
                                        .map_err(|e| format!("Failed to parse col '{}': {}", col_str, e))?;

                                    coordinates.push((row, col));
                                }
                                Rule::EOI | Rule::WHITESPACE => {
                                    // Ignore End Of Input marker
                                }
                                _ => {
                                    // This case should not be reached
                                    return Err(format!(
                                        "Unexpected rule encountered during layout.matrix_map processing: {:?}",
                                        inner_pair.as_rule()
                                    ));
                                }
                            }
                        }
                    }
                }
                Ok(coordinates)
            }
            Err(e) => Err(format!("Invalid layout.matrix_map format: {}", e)),
        }
    }

    fn alias_resolver(keys: &str, aliases: &HashMap<String, String>) -> Result<String, String> {
        let mut current_keys = keys.to_string();

        let mut iterations = 0;

        loop {
            let mut next_keys = String::with_capacity(current_keys.capacity());
            let mut made_replacement = false;
            let mut last_index = 0; // Keep track of where we are in current_keys

            while let Some(at_index) = current_keys[last_index..].find('@') {
                let start_index = last_index + at_index;

                // Append the text before the '@'
                next_keys.push_str(&current_keys[last_index..start_index]);

                // Check if it's a valid alias start (@ followed by a non whitespace)
                if let Some(first_char) = current_keys.as_bytes().get(start_index + 1) {
                    if !first_char.is_ascii_whitespace() {
                        // Find the end of the alias identifier
                        let mut end_index = start_index + 2;
                        while let Some(c) = current_keys.as_bytes().get(end_index) {
                            if c.is_ascii_whitespace() {
                                break;
                            } else {
                                end_index += 1;
                            }
                        }

                        // Extract the alias key (except the starting '@')
                        let alias_key = &current_keys[start_index + 1..end_index];

                        // Look up and replace
                        match aliases.get(alias_key) {
                            Some(value) => {
                                next_keys.push_str(value);
                                made_replacement = true;
                            }
                            None => return Err(format!("Undefined alias: {}", alias_key)),
                        }
                        last_index = end_index; // Move past the processed alias
                    } else {
                        // Not a valid alias start, treat '@' literally
                        next_keys.push('@');
                        last_index = start_index + 1;
                    }
                } else {
                    // '@' was the last character, treat it literally
                    next_keys.push('@');
                    last_index = start_index + 1;
                    break; // No more characters after '@'
                }
            }

            // Append any remaining part of the string after the last '@' or if no '@' was found
            next_keys.push_str(&current_keys[last_index..]);

            // Check for termination conditions
            iterations += 1;
            if iterations >= MAX_ALIAS_RESOLUTION_DEPTH {
                return Err(format!(
                    "Alias resolution exceeded maximum depth ({}), potential infinite loop detected in '{}'",
                    MAX_ALIAS_RESOLUTION_DEPTH, keys
                )); // Show original keys for context
            }

            if !made_replacement {
                break; // No more replacements needed
            }

            // Prepare for the next iteration
            current_keys = next_keys;
        }

        Ok(current_keys)
    }

    fn layer_name_resolver(
        prefix: &str,
        pair: pest::iterators::Pair<Rule>,
        layer_names: &HashMap<String, u32>,
    ) -> Result<String, String> {
        let mut action = prefix.to_string() + "(";

        for inner_pair in pair.into_inner() {
            match inner_pair.as_rule() {
                //the first argument is the layer name or layer number
                Rule::layer_name => {
                    // Check if the layer name is valid
                    let layer_name = inner_pair.as_str().to_string();
                    if let Some(layer_number) = layer_names.get(&layer_name) {
                        action += layer_number.to_string().as_str();
                    } else {
                        return Err(format!("Invalid layer name: {}", layer_name));
                    }
                }
                Rule::layer_number => {
                    action += inner_pair.as_str();
                }
                _ => {
                    // the second argument is not processed, just forwarded
                    action += ", ";
                    action += inner_pair.as_str();
                }
            }
        }
        action += ")";

        Ok(action)
    }

    fn keymap_parser(
        layer_keys: &str,
        aliases: &HashMap<String, String>,
        layer_names: &HashMap<String, u32>,
    ) -> Result<Vec<String>, String> {
        //resolve aliases first
        let layer_keys = Self::alias_resolver(layer_keys, aliases)?;

        let mut key_action_sequence = Vec::new();

        // Parse the keymap using Pest
        match ConfigParser::parse(Rule::key_map, &layer_keys) {
            Ok(pairs) => {
                // The top-level pair is 'key_map'. We need to iterate its inner content.
                for pair in pairs {
                    // Should only be one pair matching Rule::key_map
                    if pair.as_rule() == Rule::key_map {
                        for inner_pair in pair.into_inner() {
                            match inner_pair.as_rule() {
                                Rule::no_action => {
                                    let action = inner_pair.as_str().to_string();
                                    key_action_sequence.push(action);
                                }

                                Rule::transparent_action => {
                                    let action = inner_pair.as_str().to_string();
                                    key_action_sequence.push(action);
                                }

                                Rule::simple_keycode => {
                                    let action = inner_pair.as_str().to_string();
                                    key_action_sequence.push(action);
                                }

                                Rule::shifted_action => {
                                    let action = inner_pair.as_str().to_string();
                                    key_action_sequence.push(action);
                                }

                                Rule::osm_action => {
                                    let action = inner_pair.as_str().to_string();
                                    key_action_sequence.push(action);
                                }

                                Rule::wm_action => {
                                    let action = inner_pair.as_str().to_string();
                                    key_action_sequence.push(action);
                                }

                                //layer actions:
                                Rule::df_action => {
                                    key_action_sequence.push(Self::layer_name_resolver("DF", inner_pair, layer_names)?);
                                }
                                Rule::mo_action => {
                                    key_action_sequence.push(Self::layer_name_resolver("MO", inner_pair, layer_names)?);
                                }
                                Rule::lm_action => {
                                    key_action_sequence.push(Self::layer_name_resolver("LM", inner_pair, layer_names)?);
                                }
                                Rule::lt_action => {
                                    key_action_sequence.push(Self::layer_name_resolver("LT", inner_pair, layer_names)?);
                                    //"LT(".to_owned() + &Self::layer_name_resolver(inner_pair, layer_names)? + ")");
                                }
                                Rule::osl_action => {
                                    key_action_sequence.push(Self::layer_name_resolver(
                                        "OSL",
                                        inner_pair,
                                        layer_names,
                                    )?);
                                }
                                Rule::tt_action => {
                                    key_action_sequence.push(Self::layer_name_resolver("TT", inner_pair, layer_names)?);
                                }
                                Rule::tg_action => {
                                    key_action_sequence.push(Self::layer_name_resolver("TG", inner_pair, layer_names)?);
                                }
                                Rule::to_action => {
                                    key_action_sequence.push(Self::layer_name_resolver("TO", inner_pair, layer_names)?);
                                }

                                //tap-hold actions:
                                Rule::mt_action => {
                                    let action = inner_pair.as_str().to_string();
                                    key_action_sequence.push(action);
                                }
                                Rule::th_action => {
                                    let action = inner_pair.as_str().to_string();
                                    key_action_sequence.push(action);
                                }

                                Rule::EOI | Rule::WHITESPACE => {
                                    // Ignore End of input marker
                                }
                                _ => {
                                    // This case should not be reached
                                    return Err(format!(
                                        "Unexpected rule encountered during layer.keys processing:{:?}",
                                        inner_pair.as_rule()
                                    ));
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                return Err(format!("Invalid keymap format: {}", e));
            }
        }

        Ok(key_action_sequence)
    }

    // Layout is a mandatory field in toml, so we mainly check the sizes
    fn get_layout_from_toml(
        mut layout: LayoutTomlConfig,
        layers: Vec<LayerTomlConfig>,
        aliases: HashMap<String, String>,
    ) -> Result<LayoutConfig, TokenStream2> {
        //temporarily allow both matrix_map and keymap to be set and append the obsolete layout.keymap based layer configurations
        //to the new [[layer]] based layer configurations in the resulting LayoutConfig

        // Check alias keys for whitespace
        for key in aliases.keys() {
            if key.chars().any(char::is_whitespace) {
                let error_message = format!(
                    "keyboard.toml: Alias key '{}' must not contain whitespace characters",
                    key
                );
                return rmk_compile_error!(error_message);
            }
        }

        let mut final_layers = Vec::<Vec<Vec<String>>>::new();
        let mut sequence_to_grid: Option<Vec<(u8, u8)>> = None;

        if let Some(matrix_map) = &layout.matrix_map {
            //process matrix_map first to build mapping between the electronic grid and the configuration sequence of keys
            let mut sequence_number = 0u32;
            let mut grid_to_sequence: Vec<Vec<Option<u32>>> =
                vec![vec![None; layout.cols as usize]; layout.rows as usize];

            match Self::parse_matrix_map(matrix_map) {
                Ok(coords) => {
                    for (row, col) in &coords {
                        if *row >= layout.rows || *col >= layout.cols {
                            let error_message = format!(
                                "keyboard.toml: Coordinate ({},{}) in `layout.matrix_map` is out of bounds: ([0..{}], [0..{}]) is the expected range",
                                row, col, layout.rows-1, layout.cols-1
                            );
                            return rmk_compile_error!(error_message);
                        }
                        if grid_to_sequence[*row as usize][*col as usize].is_some() {
                            let error_message = format!(
                                "keyboard.toml: Duplicate coordinate ({},{}) found in `layout.matrix_map`",
                                row, col
                            );
                            return rmk_compile_error!(error_message);
                        } else {
                            grid_to_sequence[*row as usize][*col as usize] = Some(sequence_number);
                        }
                        sequence_number += 1;
                    }
                    sequence_to_grid = Some(coords);
                }
                Err(parse_err) => {
                    // Pest error already includes details about the invalid format
                    let error_message = format!("keyboard.toml: Error in `layout.matrix_map`: {}", parse_err);
                    return rmk_compile_error!(error_message);
                }
            }
        } else if !layers.is_empty() {
            return rmk_compile_error!(
                "layout.matrix_map is need to be defined to process [[layer]] based key maps".to_string()
            );
        }

        if let Some(sequence_to_grid) = &sequence_to_grid {
            // collect layer names first
            let mut layer_names = HashMap::<String, u32>::new();
            for (layer_number, layer) in layers.iter().enumerate() {
                if let Some(name) = &layer.name {
                    if layer_names.contains_key(name) {
                        let error_message = format!(
                            "keyboard.toml: Duplicate layer name '{}' found in `layout.keymap`",
                            name
                        );
                        return rmk_compile_error!(error_message);
                    }
                    layer_names.insert(name.clone(), layer_number as u32);
                }
            }
            if layers.len() > layout.layers as usize {
                return rmk_compile_error!(
                    "keyboard.toml: Number of [[layer]] entries is larger than layout.layers".to_string()
                );
            }

            // Parse each explicitly defined [[layer]] with pest into the final_layers vector
            // using the previously defined sequence_to_grid mapping to fill in the
            // grid shaped classic keymaps
            let layer_names = layer_names; //make it immutable
            for (layer_number, layer) in layers.iter().enumerate() {
                // each layer should contain a sequence of keymap entries
                // their number and order should match the number and order of the above parsed matrix map
                match Self::keymap_parser(&layer.keys, &aliases, &layer_names) {
                    Ok(key_action_sequence) => {
                        let mut legacy_keymap =
                            vec![vec!["No".to_string(); layout.cols as usize]; layout.rows as usize];

                        for (sequence_number, key_action) in key_action_sequence.into_iter().enumerate() {
                            if sequence_number >= sequence_to_grid.len() {
                                let error_message = format!(
                                    "keyboard.toml: {} layer #{} contains too many entries (must match layout.matrix_map)", &layer.name.clone().unwrap_or_default(), layer_number);
                                return rmk_compile_error!(error_message);
                            }
                            let (row, col) = sequence_to_grid[sequence_number];
                            legacy_keymap[row as usize][col as usize] = key_action.clone();
                        }

                        final_layers.push(legacy_keymap);
                    }

                    Err(parse_err) => {
                        // Pest error already includes details about the invalid format
                        let error_message = format!("keyboard.toml: Error in `layout.keymap`: {}", parse_err);
                        return rmk_compile_error!(error_message);
                    }
                }
            }
        }

        // Handle the deprecated `keymap` field if present
        if let Some(keymap) = &mut layout.keymap {
            final_layers.append(keymap);
        }

        // The required number of layers is less than what's set in keymap
        // Fill the rest with empty keys
        if final_layers.len() <= layout.layers as usize {
            for _ in final_layers.len()..layout.layers as usize {
                // Add 2D vector of empty keys
                final_layers.push(vec![vec!["_".to_string(); layout.cols as usize]; layout.rows as usize]);
            }
        } else {
            let error_message = format!(
                "keyboard.toml: The actual number of layers is larger than {} [layout.layers]: {} [[Layer]] entries + {} layers in layout.keymap",
                layout.layers, layers.len(), layout.keymap.map(|keymap| keymap.len()).unwrap_or_default()
            );
            return rmk_compile_error!(error_message);
        }

        // Row
        if final_layers.iter().any(|r| r.len() as u8 != layout.rows) {
            return rmk_compile_error!(
                "keyboard.toml: Row number in keymap doesn't match with [layout.row]".to_string()
            );
        }
        // Col
        if final_layers
            .iter()
            .any(|r| r.iter().any(|c| c.len() as u8 != layout.cols))
        {
            // Find a row whose col num is wrong
            return rmk_compile_error!(
                "keyboard.toml: Col number in keymap doesn't match with [layout.col]".to_string()
            );
        }

        Ok(LayoutConfig {
            rows: layout.rows,
            cols: layout.cols,
            layers: layout.layers,
            keymap: final_layers,
        })
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
                            return rmk_compile_error!("keyboard.toml: Tri layer upper is larger than [layout.layers]");
                        } else if tri_layer.lower >= layout.layers {
                            return rmk_compile_error!("keyboard.toml: Tri layer lower is larger than [layout.layers]");
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
                        return rmk_compile_error!(format!(
                            "keyboard.toml: number of combos is greater than [behavior.combo.max_num]"
                        ));
                    }

                    for (i, c) in combo.combos.iter().enumerate() {
                        if c.actions.len() > COMBO_MAX_LENGTH {
                            return rmk_compile_error!(format!("keyboard.toml: number of keys in combo #{i} is greater than [behavior.combo.max_length]"));
                        }

                        if let Some(layer) = c.layer {
                            if layer >= layout.layers {
                                return rmk_compile_error!(format!(
                                    "keyboard.toml: layer in combo #{i} is greater than [layout.layers]"
                                ));
                            }
                        }
                    }
                }

                behavior.fork = behavior.fork.or(default.fork);
                if let Some(fork) = &behavior.fork {
                    if fork.forks.len() > FORK_MAX_NUM {
                        return rmk_compile_error!(format!(
                            "keyboard.toml: number of forks is greater than [behavior.fork.max_num]"
                        ));
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
