// rmk-config: Configuration parsing and management for RMK keyboard firmware
//
// This crate provides types and utilities for parsing keyboard.toml configuration files
// and generating the necessary constants and settings for RMK keyboard firmware.

use std::collections::HashMap;
use std::path::Path;

use serde_derive::Deserialize;

// Module declarations
pub mod chip;
pub mod communication;
pub mod keyboard;
#[rustfmt::skip]
pub mod usb_interrupt_map;
pub mod behavior;
pub mod board;
pub mod host;
pub mod keycode_alias;
pub mod layout_parser;  // Renamed from layout
pub mod light;
pub mod storage;

// New module structure
pub mod types;
pub mod config;
pub mod api;

// Re-export commonly used types from existing modules
pub use board::{BoardConfig, UniBodyConfig};
pub use chip::{ChipModel, ChipSeries};
pub use communication::{CommunicationConfig, UsbInfo};
pub use keyboard::Basic;  // Keep for backward compatibility
pub use keycode_alias::KEYCODE_ALIAS;

// Re-export types from the new types module
pub use types::*;

/// Configurations for RMK keyboard.
#[derive(Clone, Debug, Deserialize)]
#[allow(unused)]
pub struct KeyboardTomlConfig {
    /// Basic keyboard info
    keyboard: Option<KeyboardMetadata>,  // Renamed from KeyboardInfo
    /// Matrix of the keyboard, only for non-split keyboards
    matrix: Option<MatrixConfig>,
    // Aliases for key maps
    aliases: Option<HashMap<String, String>>,
    // Layers of key maps
    layer: Option<Vec<LayerDefinition>>,  // Renamed from LayerTomlConfig
    /// Layout config.
    /// For split keyboard, the total row/col should be defined in this section
    layout: Option<LayoutDefinition>,  // Renamed from LayoutTomlConfig
    /// Behavior config
    behavior: Option<BehaviorConfig>,
    /// Light config
    light: Option<LightConfig>,
    /// Storage config
    storage: Option<StorageConfig>,
    /// Ble config
    ble: Option<BleConfig>,
    /// Chip-specific configs (e.g., [chip.nrf52840])
    chip: Option<HashMap<String, ChipConfig>>,
    /// Dependency config
    dependency: Option<DependencyConfig>,
    /// Split config
    split: Option<SplitConfig>,
    /// Input device config
    input_device: Option<InputDeviceConfig>,
    /// Output Pin config
    output: Option<Vec<OutputConfig>>,
    /// Set host configurations
    pub host: Option<HostConfig>,
    /// RMK config constants
    #[serde(default)]
    pub rmk: RmkConstantsConfig,
    /// Controller event channel configuration
    #[serde(default)]
    pub controller_event: EventConfig,
}

impl KeyboardTomlConfig {
    /// Load configuration from TOML file
    ///
    /// This is the new recommended way to load configuration.
    /// It uses a two-pass loading approach to merge chip-specific defaults.
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        config::ConfigLoader::load(path)
    }

    /// Legacy method for loading configuration
    ///
    /// This method is kept for backward compatibility.
    /// New code should use `load()` instead.
    pub fn new_from_toml_path<P: AsRef<Path>>(config_toml_path: P) -> Self {
        config::ConfigLoader::load(config_toml_path).unwrap()
    }

    /// Auto calculate some parameters in toml:
    /// - Update morse_max_num to fit all configured morses
    /// - Update max_patterns_per_key to fit the max number of configured (pattern, action) pairs per morse key
    /// - Update peripheral number based on the number of split boards
    pub fn auto_calculate_parameters(&mut self) {
        // Update the number of peripherals
        if let Some(split) = &self.split
            && split.peripheral.len() > self.rmk.split_peripherals_num
        {
            self.rmk.split_peripherals_num = split.peripheral.len();
        }

        if let Some(behavior) = &self.behavior {
            // Update the max_patterns_per_key
            if let Some(morse) = &behavior.morse
                && let Some(morses) = &morse.morses
            {
                let mut max_required_patterns = self.rmk.max_patterns_per_key;

                for morse in morses {
                    let tap_actions_len = morse.tap_actions.as_ref().map(|v| v.len()).unwrap_or(0);
                    let hold_actions_len = morse.hold_actions.as_ref().map(|v| v.len()).unwrap_or(0);

                    let n = tap_actions_len.max(hold_actions_len);
                    if n > 15 {
                        panic!("The number of taps per morse is too large, the max number of taps is 15, got {n}");
                    }

                    let morse_actions_len = morse.morse_actions.as_ref().map(|v| v.len()).unwrap_or(0);

                    max_required_patterns =
                        max_required_patterns.max(tap_actions_len + hold_actions_len + morse_actions_len);
                }
                self.rmk.max_patterns_per_key = max_required_patterns;

                // Update the morse_max_num
                self.rmk.morse_max_num = self.rmk.morse_max_num.max(morses.len());
            }
        }
    }

    pub fn get_output_config(&self) -> Result<Vec<OutputConfig>, String> {
        let output_config = self.output.clone();
        let split = self.split.clone();
        match (output_config, split) {
            (None, Some(s)) => Ok(s.central.output.unwrap_or_default()),
            (Some(c), None) => Ok(c),
            (None, None) => Ok(Default::default()),
            _ => Err("Use [[split.output]] to define outputs for split in your keyboard.toml!".to_string()),
        }
    }
}
