// Output API implementations

use crate::types::OutputConfig;
use crate::KeyboardTomlConfig;

impl KeyboardTomlConfig {
    /// Get output pin configuration
    /// This is renamed from get_output_config
    pub fn outputs(&self) -> Result<Vec<OutputConfig>, String> {
        // Delegate to the existing implementation in lib.rs
        self.get_output_config()
    }

    // Keep old method name for backward compatibility during transition
    // The actual implementation is in lib.rs
}
