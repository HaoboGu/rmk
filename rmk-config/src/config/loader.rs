// Configuration loader with two-pass loading

use std::path::Path;
use config::{Config, File, FileFormat};

use crate::KeyboardTomlConfig;

pub struct ConfigLoader;

impl ConfigLoader {
    /// Load keyboard configuration with two-pass approach:
    ///
    /// **Pass 1**: Load user config to determine chip model
    /// **Pass 2**: Merge chip-specific defaults with user config
    ///
    /// This two-pass approach allows chip-specific defaults to be applied
    /// automatically based on the chip model specified in the user's config.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rmk_config::config::ConfigLoader;
    ///
    /// let config = ConfigLoader::load("keyboard.toml").unwrap();
    /// ```
    pub fn load<P: AsRef<Path>>(path: P) -> Result<KeyboardTomlConfig, String> {
        // Pass 1: Determine chip model
        let user_config = Self::load_user_config(path.as_ref())?;
        let chip = user_config.get_chip_model()?;
        let default_config_str = chip.get_default_config_str()?;

        // Pass 2: Merge defaults with user config
        let mut config = Self::merge_configs(default_config_str, path.as_ref())?;

        // Auto-calculate derived parameters
        config.auto_calculate_parameters();

        Ok(config)
    }

    /// Load user configuration file (first pass)
    fn load_user_config(path: &Path) -> Result<KeyboardTomlConfig, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read config file {:?}: {}", path, e))?;
        toml::from_str(&content)
            .map_err(|e| format!("Failed to parse config {:?}: {}", path, e.message()))
    }

    /// Merge chip defaults with user config (second pass)
    fn merge_configs(default_str: &str, user_path: &Path) -> Result<KeyboardTomlConfig, String> {
        Config::builder()
            .add_source(File::from_str(default_str, FileFormat::Toml))
            .add_source(File::with_name(user_path.to_str().unwrap()))
            .build()
            .map_err(|e| format!("Failed to merge configs: {}", e))?
            .try_deserialize()
            .map_err(|e| format!("Failed to deserialize merged config: {}", e))
    }
}
