// TODO: more configs need to be added, easy configuration
/// Advanced configurations for keyboard.
#[derive(Debug, Default)]
pub struct KeyboardAdvancedConfig<'a> {
    pub mouse_config: MouseConfig,
    pub usb_config: KeyboardUsbConfig<'a>,
}

/// Configurations for mouse functionalities
#[derive(Debug)]
pub struct MouseConfig {
    pub mouse_key_interval: u32,
    pub mouse_wheel_interval: u32,
}

impl Default for MouseConfig {
    fn default() -> Self {
        Self {
            mouse_key_interval: 20,
            mouse_wheel_interval: 80,
        }
    }
}

/// Configurations for usb
#[derive(Debug)]
pub struct KeyboardUsbConfig<'a> {
    pub vid: u16,
    pub pid: u16,
    pub manufacturer: Option<&'a str>,
    pub product_name: Option<&'a str>,
    pub serial_number: Option<&'a str>,
}

impl<'a> KeyboardUsbConfig<'a> {
    pub fn new(
        vid: u16,
        pid: u16,
        manufacturer: Option<&'a str>,
        product_name: Option<&'a str>,
        serial_number: Option<&'a str>,
    ) -> Self {
        Self {
            vid,
            pid,
            manufacturer,
            product_name,
            serial_number,
        }
    }
}

impl<'a> Default for KeyboardUsbConfig<'a> {
    fn default() -> Self {
        Self {
            vid: 0x4c4b,
            pid: 0x4643,
            manufacturer: Some("Haobo"),
            product_name: Some("RMK Keyboard"),
            serial_number: Some("00000001"),
        }
    }
}
