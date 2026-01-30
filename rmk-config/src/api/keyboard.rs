// Keyboard API implementations

use crate::types::{DependencyConfig, KeyboardInfo, KeyboardMetadata};
use crate::KeyboardTomlConfig;

impl KeyboardTomlConfig {
    /// Get keyboard basic information
    /// This is renamed from get_device_config
    pub fn keyboard_info(&self) -> KeyboardInfo {
        let default = KeyboardInfo::default();
        let keyboard = self.keyboard.as_ref().unwrap();
        KeyboardInfo {
            name: keyboard.name.clone(),
            vendor_id: keyboard.vendor_id,
            product_id: keyboard.product_id,
            manufacturer: keyboard.manufacturer.clone().unwrap_or(default.manufacturer),
            product_name: keyboard.product_name.clone().unwrap_or(default.product_name),
            serial_number: keyboard.serial_number.clone().unwrap_or(default.serial_number),
        }
    }

    /// Get dependency configuration
    /// This is renamed from get_dependency_config
    pub fn dependencies(&self) -> DependencyConfig {
        if let Some(dependency) = &self.dependency {
            dependency.clone()
        } else {
            DependencyConfig::default()
        }
    }

    // Keep old method names for backward compatibility during transition
    #[deprecated(since = "0.3.0", note = "Use `keyboard_info()` instead")]
    pub fn get_device_config(&self) -> crate::keyboard::Basic {
        let info = self.keyboard_info();
        crate::keyboard::Basic {
            name: info.name,
            vendor_id: info.vendor_id,
            product_id: info.product_id,
            manufacturer: info.manufacturer,
            product_name: info.product_name,
            serial_number: info.serial_number,
        }
    }

    #[deprecated(since = "0.3.0", note = "Use `dependencies()` instead")]
    pub fn get_dependency_config(&self) -> DependencyConfig {
        self.dependencies()
    }
}
