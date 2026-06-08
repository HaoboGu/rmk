//! Resolved hardware types for the public API of `rmk-config`.
//!
//! Leaf types are re-exported directly from the TOML configuration types
//! Only types with genuine structural transformation are defined here.

// Re-export leaf types from TOML config (now properly named and `pub`)
pub use crate::board::{BoardConfig, UniBodyConfig};
pub use crate::chip::{ChipModel, ChipSeries};
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

/// Resolved DFU partition config
pub struct DfuConfig {
    pub state_offset: u32,
    pub state_size: u32,
    pub dfu_offset: u32,
    pub dfu_size: u32,
    pub page_size: u32,
}

/// Complete hardware configuration for init code generation.
pub struct Hardware {
    pub chip: ChipModel,
    pub chip_config: ChipConfig,
    pub communication: CommunicationConfig,
    pub board: BoardConfig,
    pub storage: Option<Storage>,
    pub dfu: Option<DfuConfig>,
    pub light: LightConfig,
    pub display: Option<DisplayConfig>,
    pub output: Vec<OutputConfig>,
    pub dependency: DependencyConfig,
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
        let dfu = self.get_dfu_config().map(|d| {
            let has_manual_overrides = self.dfu_user_set
                && (d.state_offset.is_some()
                    || d.state_size.is_some()
                    || d.dfu_offset.is_some()
                    || d.dfu_size.is_some());

            if has_manual_overrides {
                // User manually set [dfu] values → use directly
                DfuConfig {
                    state_offset: d.state_offset.unwrap_or(0x6000),
                    state_size: d.state_size.unwrap_or(0x1000),
                    dfu_offset: d.dfu_offset.unwrap_or(0x87000),
                    dfu_size: d.dfu_size.unwrap_or(516 * 1024),
                    page_size: d.page_size.unwrap_or(4096),
                }
            } else {
                // Auto-calculate using bootymcbootface formula:
                //   state at 0x6000 (4K), active from 0x7000 (flash/4),
                //   dfu follows active (flash/4 + page_size)
                let flash_size = d.flash_size.unwrap_or(2 * 1024 * 1024);
                let half_flash = flash_size / 4;
                let page_size = d.page_size.unwrap_or(4096);
                DfuConfig {
                    state_offset: 0x6000,
                    state_size: 0x1000,
                    dfu_offset: 0x7000 + half_flash,
                    dfu_size: half_flash + page_size,
                    page_size,
                }
            }
        });
        let light = self.get_light_config();
        let display = self.get_display_config();
        let output = self.get_output_config()?;
        let dependency = self.get_dependency_config();
        Ok(Hardware {
            chip,
            chip_config,
            communication,
            board,
            storage,
            dfu,
            light,
            display,
            output,
            dependency,
        })
    }
}
