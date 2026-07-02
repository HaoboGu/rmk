//! Resolved hardware types for the public API of `rmk-config`.
//!
//! Leaf types are re-exported directly from the TOML configuration types
//! Only types with genuine structural transformation are defined here.

use std::collections::HashMap;

// Re-export leaf types from TOML config (now properly named and `pub`)
pub use crate::board::{BoardConfig, UniBodyConfig};
pub use crate::chip::{ChipModel, ChipSeries, parse_chip_model};
pub use crate::communication::{CommunicationConfig, UsbInfo};
pub use crate::{
    BleConfig, ChipConfig, CommunicationProtocol, DependencyConfig, DisplayConfig, DisplayDriver, EncoderConfig,
    EncoderResolution, I2cConfig, InputDeviceConfig, Iqs5xxConfig, Iqs5xxI2cConfig, JoystickConfig, KeyInfo,
    LightConfig, MatrixConfig, MatrixType, OutputConfig, PinConfig, Pmw33xxConfig, Pmw33xxType, Pmw3610Config,
    PointingDeviceConfig, SerialConfig, SpiConfig, SplitBoardConfig, SplitConfig,
};

/// Resolved storage hardware config
pub struct Storage {
    pub start_addr: usize,
    pub num_sectors: u8,
    pub clear_storage: bool,
    pub clear_layout: bool,
}

/// Complete hardware configuration for init code generation.
pub struct Hardware {
    pub chip: ChipModel,
    pub chip_config: ChipConfig,
    /// User-supplied `[chip.<name>]` overrides for all chips used in the build.
    pub chip_configs: HashMap<String, ChipConfig>,
    pub communication: CommunicationConfig,
    pub board: BoardConfig,
    pub storage: Option<Storage>,
    pub light: LightConfig,
    pub display: Option<DisplayConfig>,
    pub output: Vec<OutputConfig>,
    pub dependency: DependencyConfig,
}

impl Hardware {
    /// Resolve the chip model and chip-specific config for a split board.
    ///
    /// If the board defines its own `chip`, that chip is used; otherwise the
    /// top-level keyboard chip is used.
    pub fn chip_for_split_board(&self, board_config: &SplitBoardConfig) -> (ChipModel, ChipConfig) {
        let chip_model = board_config
            .chip
            .as_ref()
            .map(|chip| parse_chip_model(chip).expect("Invalid split board chip"))
            .unwrap_or_else(|| self.chip.clone());
        let chip_config = self
            .chip_configs
            .get(&chip_model.chip)
            .cloned()
            .unwrap_or_default();
        (chip_model, chip_config)
    }
}

impl crate::KeyboardTomlConfig {
    /// Resolve hardware configuration from TOML config.
    pub fn hardware(&self) -> Result<Hardware, String> {
        let chip = self.get_chip_model()?;
        let chip_config = self.get_chip_config();
        let communication = self.get_communication_config()?;
        let board = self.get_board_config()?;
        let storage_toml = self.get_storage_config();
        let storage = if storage_toml.enabled {
            Some(Storage {
                start_addr: storage_toml.start_addr.unwrap_or(0),
                num_sectors: storage_toml.num_sectors.unwrap_or(2),
                clear_storage: storage_toml.clear_storage.unwrap_or(false),
                clear_layout: storage_toml.clear_layout.unwrap_or(false),
            })
        } else {
            None
        };
        let light = self.get_light_config();
        let display = self.get_display_config();
        let output = self.get_output_config()?;
        let dependency = self.get_dependency_config();
        Ok(Hardware {
            chip,
            chip_config,
            chip_configs: self.chip.clone().unwrap_or_default(),
            communication,
            board,
            storage,
            light,
            display,
            output,
            dependency,
        })
    }
}
