// Layout API implementations

use crate::types::{KeyInfo, Layout};
use crate::KeyboardTomlConfig;

impl KeyboardTomlConfig {
    /// Get layout configuration
    /// This is renamed from get_layout_config
    pub fn layout(&self) -> Result<(Layout, Vec<Vec<KeyInfo>>), String> {
        // Delegate to the existing implementation in layout_parser
        self.get_layout_config()
    }

    // Keep old method name for backward compatibility during transition
    // The actual implementation is in layout_parser.rs
}
