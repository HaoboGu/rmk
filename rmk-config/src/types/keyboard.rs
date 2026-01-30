// Keyboard metadata and info types

use serde::Deserialize;

/// Configurations for keyboard info (internal TOML representation)
/// This is renamed from KeyboardInfo to KeyboardMetadata
#[derive(Clone, Debug, Default, Deserialize)]
pub struct KeyboardMetadata {
    /// Keyboard name
    pub name: String,
    /// Vender id
    pub vendor_id: u16,
    /// Product id
    pub product_id: u16,
    /// Manufacturer
    pub manufacturer: Option<String>,
    /// Product name, if not set, it will use `name` as default
    pub product_name: Option<String>,
    /// Serial number
    pub serial_number: Option<String>,
    /// Board name(if a supported board is used)
    pub board: Option<String>,
    /// Chip model
    pub chip: Option<String>,
    /// enable usb
    pub usb_enable: Option<bool>,
}

/// Keyboard's basic info (public API type)
/// This is renamed from Basic to KeyboardInfo
#[derive(Clone, Debug)]
pub struct KeyboardInfo {
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

impl Default for KeyboardInfo {
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
