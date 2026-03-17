use crate::{BoardConfig, ChipConfig, ChipModel, CommunicationConfig, DependencyConfig, LightConfig, OutputConfig};

/// Complete hardware configuration for init code generation.
pub struct Hardware {
    pub chip: ChipModel,
    pub chip_config: ChipConfig,
    pub communication: CommunicationConfig,
    pub board: BoardConfig,
    pub storage: Option<Storage>,
    pub light: LightConfig,
    pub output: Vec<OutputConfig>,
    pub dependency: DependencyConfig,
    pub host: Host,
}

/// Resolved storage hardware config (None when storage disabled).
pub struct Storage {
    pub start_addr: usize,
    pub num_sectors: u8,
    pub clear_storage: bool,
    pub clear_layout: bool,
}

/// Resolved host tool configuration.
pub struct Host {
    pub vial_enabled: bool,
    pub unlock_keys: Vec<[u8; 2]>,
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
        let host_toml = self.get_host_config();
        let host = Host {
            vial_enabled: host_toml.vial_enabled,
            unlock_keys: host_toml.unlock_keys.unwrap_or_default(),
        };
        Ok(Hardware {
            chip,
            chip_config,
            communication,
            board,
            storage,
            light,
            output,
            dependency,
            host,
        })
    }
}
