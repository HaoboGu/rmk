// Chip API implementations

use crate::chip::ChipModel;
use crate::types::ChipConfig;
use crate::KeyboardTomlConfig;

impl KeyboardTomlConfig {
    /// Get chip model
    /// This is renamed from get_chip_model
    pub fn chip(&self) -> Result<ChipModel, String> {
        // Delegate to the existing implementation in chip.rs
        self.get_chip_model()
    }

    /// Get chip-specific settings
    /// This is renamed from get_chip_config
    pub fn chip_settings(&self) -> ChipConfig {
        // Delegate to the existing implementation in chip.rs
        self.get_chip_config()
    }

    // Keep old method names for backward compatibility during transition
    // The actual implementations are in chip.rs
}
