use crate::keyboard::Basic;

/// Keyboard identity for USB descriptors and BLE advertising.
pub struct Identity {
    pub name: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub manufacturer: String,
    pub product_name: String,
    pub serial_number: String,
}

impl crate::KeyboardTomlConfig {
    /// Resolve keyboard identity from TOML config.
    pub fn identity(&self) -> Result<Identity, String> {
        let default = Basic::default();
        let keyboard = self
            .keyboard
            .as_ref()
            .ok_or_else(|| "keyboard.toml: [keyboard] section is required".to_string())?;
        Ok(Identity {
            name: keyboard.name.clone(),
            vendor_id: keyboard.vendor_id,
            product_id: keyboard.product_id,
            manufacturer: keyboard.manufacturer.clone().unwrap_or(default.manufacturer),
            product_name: keyboard.product_name.clone().unwrap_or(default.product_name),
            serial_number: keyboard.serial_number.clone().unwrap_or(default.serial_number),
        })
    }
}
