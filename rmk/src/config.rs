// TODO: more configs need to be added, easy configuration(from config file)
/// Configurations for RMK keyboard.
#[derive(Debug, Default)]
pub struct RmkConfig<'a> {
    pub mouse_config: MouseConfig,
    pub usb_config: KeyboardUsbConfig<'a>,
    pub vial_config: VialConfig<'a>,
}

/// Config for [vial](https://get.vial.today/).
///
/// You can generate automatically using [`build.rs`](https://github.com/HaoboGu/rmk/blob/main/boards/stm32h7/build.rs).
#[derive(Debug, Default)]
pub struct VialConfig<'a> {
    pub vial_keyboard_id: &'a [u8],
    pub vial_keyboard_def: &'a [u8],
}

impl<'a> VialConfig<'a> {
    pub fn new(vial_keyboard_id: &'a [u8], vial_keyboard_def: &'a [u8]) -> Self {
        Self {
            vial_keyboard_id,
            vial_keyboard_def,
        }
    }
}

/// Configuration for debouncing
pub struct DebounceConfig {
    /// Debounce time in ms
    pub debounce_time: u32,
}

/// Configurations for mouse functionalities
#[derive(Debug)]
pub struct MouseConfig {
    /// Time interval in ms of reporting mouse cursor states
    pub mouse_key_interval: u32,
    /// Time interval in ms of reporting mouse wheel states
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

/// Configurations for RGB light
pub struct RGBLightConfig {
    pub enabled: bool,
    pub rgb_led_num: u32,
    pub rgb_hue_step: u32,
    pub rgb_val_step: u32,
    pub rgb_sat_step: u32,
}

/// Configurations for usb
#[derive(Debug)]
pub struct KeyboardUsbConfig<'a> {
    /// Vender id
    pub vid: u16,
    /// Product id
    pub pid: u16,
    /// Manufacturer
    pub manufacturer: Option<&'a str>,
    /// Product name
    pub product_name: Option<&'a str>,
    /// Serial number
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
