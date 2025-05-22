use crate::KeyboardTomlConfig;

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
        Basic {
            name: self.keyboard.name.clone(),
            vendor_id: self.keyboard.vendor_id,
            product_id: self.keyboard.product_id,
            manufacturer: self.keyboard.manufacturer.clone().unwrap_or(default.manufacturer),
            product_name: self.keyboard.product_name.clone().unwrap_or(default.product_name),
            serial_number: self.keyboard.serial_number.clone().unwrap_or(default.serial_number),
        }
    }
}
