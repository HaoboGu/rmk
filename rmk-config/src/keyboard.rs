use crate::defaults;
use crate::error::{ConfigError, ConfigResult};
use crate::{DependencyConfig, KeyboardTomlConfig};

/// Device identification information
#[derive(Clone, Debug)]
pub struct DeviceInfo {
    /// Keyboard name
    pub name: String,
    /// Vendor id
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

impl Default for DeviceInfo {
    fn default() -> Self {
        Self {
            name: defaults::DEFAULT_PRODUCT_NAME.to_string(),
            vendor_id: defaults::DEFAULT_VID,
            product_id: defaults::DEFAULT_PID,
            manufacturer: defaults::DEFAULT_MANUFACTURER.to_string(),
            product_name: defaults::DEFAULT_PRODUCT_NAME.to_string(),
            serial_number: defaults::DEFAULT_SERIAL_NUMBER.to_string(),
        }
    }
}

impl KeyboardTomlConfig {
    pub fn get_device_config(&self) -> ConfigResult<DeviceInfo> {
        let default = DeviceInfo::default();
        let keyboard = self.keyboard.as_ref().ok_or(ConfigError::MissingField {
            field: "keyboard".to_string(),
        })?;
        Ok(DeviceInfo {
            name: keyboard.name.clone(),
            vendor_id: keyboard.vendor_id,
            product_id: keyboard.product_id,
            manufacturer: keyboard
                .manufacturer
                .clone()
                .unwrap_or(default.manufacturer),
            product_name: keyboard
                .product_name
                .clone()
                .unwrap_or(default.product_name),
            serial_number: keyboard
                .serial_number
                .clone()
                .unwrap_or(default.serial_number),
        })
    }

    pub fn get_dependency_config(&self) -> DependencyConfig {
        self.dependency.clone().unwrap_or_default()
    }
}
