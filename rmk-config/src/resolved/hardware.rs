// Re-export domain types that are already in their final resolved form after
// the 3-layer merge. Consumers should import these from `resolved::hardware`
// rather than from the crate root, so the resolved module is the single public API.
pub use crate::{
    BleConfig, BoardConfig, ChipConfig, ChipModel, ChipSeries, CommunicationConfig, DependencyConfig, EncoderConfig,
    EncoderResolution, InputDeviceConfig, JoystickConfig, KeyInfo, LightConfig, MatrixConfig, MatrixType, OutputConfig,
    PinConfig, Pmw33xxConfig, Pmw33xxType, Pmw3610Config, SerialConfig, SplitBoardConfig, SplitConfig, UniBodyConfig,
};

/// Complete hardware configuration for init code generation.
///
/// Some fields (`chip_config`, `communication`, `board`, `light`, `output`, `dependency`)
/// are passed through as domain types that are already in their final form after
/// the 3-layer TOML merge — no further resolution is needed.
pub struct Hardware {
    pub chip: ChipModel,
    pub chip_config: ChipConfig,
    pub communication: CommunicationConfig,
    pub board: BoardConfig,
    pub storage: Option<Storage>,
    pub light: LightConfig,
    pub output: Vec<OutputConfig>,
    pub dependency: DependencyConfig,
}

/// Resolved storage hardware config (None when storage disabled).
pub struct Storage {
    pub start_addr: usize,
    pub num_sectors: u8,
    pub clear_storage: bool,
    pub clear_layout: bool,
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
        let output = self.get_output_config()?;
        let dependency = self.get_dependency_config();
        Ok(Hardware {
            chip,
            chip_config,
            communication,
            board,
            storage,
            light,
            output,
            dependency,
        })
    }
}
