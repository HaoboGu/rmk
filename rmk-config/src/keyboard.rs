use crate::{DependencyConfig, KeyboardTomlConfig};

/// Keyboard's basic info
#[derive(Clone, Debug)]
pub struct Basic {
    /// Keyboard name
    pub name: String,
    /// Vender id
    pub vendor_id: u16,
    /// Product id
    pub product_id: u16,
    /// Manufacturer
    pub manufacturer: String,
    /// Product name
    pub product_name: String,
    /// Serial number
    pub serial_number: String,
}

impl Default for Basic {
    fn default() -> Self {
        Self {
            name: "RMK Keyboard".to_string(),
            vendor_id: 0xE118,
            product_id: 0x0001,
            manufacturer: "RMK".to_string(),
            product_name: "RMK Keyboard".to_string(),
            serial_number: "vial:f64c2b3c:000001".to_string(),
        }
    }
}

impl KeyboardTomlConfig {
    pub fn get_basic_info(&self) -> Basic {
        let default = Basic::default();
        let keyboard = self.keyboard.as_ref().unwrap();
        Basic {
            name: keyboard.name.clone(),
            vendor_id: keyboard.vendor_id,
            product_id: keyboard.product_id,
            manufacturer: keyboard.manufacturer.clone().unwrap_or(default.manufacturer),
            product_name: keyboard.product_name.clone().unwrap_or(default.product_name),
            serial_number: keyboard.serial_number.clone().unwrap_or(default.serial_number),
        }
    }

    pub fn get_dependency_config(&self) -> DependencyConfig {
        if let Some(dependency) = &self.dependency {
            dependency.clone()
        } else {
            DependencyConfig::default()
        }
    }
}
