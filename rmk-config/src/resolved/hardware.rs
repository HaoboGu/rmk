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
    pub led: Option<PinConfig>,
    pub unlock_keys: Vec<[u8; 2]>,
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
        let (dfu, dfu_auto_calc) = match self.get_dfu_config() {
            Some(d) => {
                let vals = [d.state_offset, d.state_size, d.dfu_offset, d.dfu_size];
                let any_set = vals.iter().any(|v| v.is_some());
                let all_set = vals.iter().all(|v| v.is_some());

                if any_set && !all_set {
                    return Err(
                        "If you set one of state_offset/state_size/dfu_offset/dfu_size, you must set all four"
                            .to_string(),
                    );
                }

                if all_set {
                    (
                        Some(DfuConfig {
                            state_offset: d.state_offset.unwrap(),
                            state_size: d.state_size.unwrap(),
                            dfu_offset: d.dfu_offset.unwrap(),
                            dfu_size: d.dfu_size.unwrap(),
                            page_size: d.page_size.unwrap_or(4096),
                            led: d.led
                                .clone()
                                .and_then(|pin| {
                                    if pin.eq_ignore_ascii_case("none") || pin.is_empty() {
                                        None
                                    } else {
                                        Some(PinConfig { pin, low_active: false })
                                    }
                                }),
                            unlock_keys: d.unlock_keys.clone().unwrap_or_default(),
                        }),
                        false,
                    )
                } else {
                    // Auto-calculate: use ALL remaining flash for ACTIVE+DFU+storage
                    // layout: [28K bootloader+state][ACTIVE][DFU(ACTIVE+1page)][storage]
                    let flash_size = d.flash_size.unwrap_or(2 * 1024 * 1024);
                    let page_size = d.page_size.unwrap_or(4096);
                    let bootloader_state_end = 0x7000u32; // 28K
                    // Reserve 128 KB for storage behind DFU, DFU auto-calc always assumes this
                    let storage_size = 32u32 * page_size;
                    let remaining = flash_size - bootloader_state_end - storage_size;
                    let active_size = (remaining - page_size) / 2;
                    (
                        Some(DfuConfig {
                            state_offset: 0x6000,
                            state_size: 0x1000,
                            dfu_offset: bootloader_state_end + active_size,
                            dfu_size: active_size + page_size,
                            page_size,
                            led: d.led
                                .clone()
                                .and_then(|pin| {
                                    if pin.eq_ignore_ascii_case("none") || pin.is_empty() {
                                        None
                                    } else {
                                        Some(PinConfig { pin, low_active: false })
                                    }
                                }),
                            unlock_keys: d.unlock_keys.clone().unwrap_or_default(),
                        }),
                        true,
                    )
                }
            }
            None => (None, false),
        };
        if self.storage_user_set
            && dfu_auto_calc
            && let Some(storage_cfg) = &self.storage
        {
            if let Some(num_sectors) = storage_cfg.num_sectors
                && num_sectors != 32
            {
                eprintln!(
                    "warning: `[storage].num_sectors = {}` is ignored with DFU auto-calculation. The DFU layout always reserves 128 KB (32 sectors) at the end of flash. Values < 32 waste reserved space, values > 32 risk flash overflow.",
                    num_sectors
                );
            }
            if let Some(start_addr) = storage_cfg.start_addr
                && start_addr != 0
            {
                eprintln!(
                    "warning: `[storage].start_addr = {:#x}` has no effect with DFU. The storage partition is automatically placed after the DFU partition.",
                    start_addr
                );
            }
        }
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
