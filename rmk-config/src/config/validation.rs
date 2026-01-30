// Centralized validation logic

use crate::types::*;

pub struct Validator;

impl Validator {
    /// Validate all configuration
    pub fn validate_all(config: &crate::KeyboardTomlConfig) -> Result<(), String> {
        Self::validate_constants(&config.rmk)?;
        // Additional validations can be added here
        Ok(())
    }

    /// Validate RMK constants configuration
    pub fn validate_constants(config: &RmkConstantsConfig) -> Result<(), String> {
        if config.combo_max_num > 256 {
            return Err("combo_max_num must be between 0 and 256".into());
        }
        if config.morse_max_num > 256 {
            return Err("morse_max_num must be between 0 and 256".into());
        }
        if !(4..=65536).contains(&config.max_patterns_per_key) {
            return Err("max_patterns_per_key must be between 4 and 65536".into());
        }
        if config.fork_max_num > 256 {
            return Err("fork_max_num must be between 0 and 256".into());
        }
        Ok(())
    }

    /// Validate behavior configuration
    pub fn validate_behavior(
        behavior: &BehaviorConfig,
        layout: &Layout,
        constants: &RmkConstantsConfig,
    ) -> Result<(), String> {
        // Behavior validation logic can be added here
        // This is a placeholder for future validation
        Ok(())
    }

    /// Validate board configuration
    pub fn validate_board(board: &crate::board::BoardConfig) -> Result<(), String> {
        // Board validation logic can be added here
        Ok(())
    }
}
