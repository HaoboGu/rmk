use serde_derive::Deserialize;

/// Configurations for RMK keyboard.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct KeyboardTomlConfig {
    pub keyboard: KeyboardInfo,
    pub matrix: MatrixConfig,
    pub light: LightConfig,
    pub storage: StorageConfig,
    pub ble: Option<BleConfig>,
}

/// Configurations for usb
#[derive(Clone, Debug, Deserialize)]
pub struct KeyboardInfo {
    /// Vender id
    pub vendor_id: u16,
    /// Product id
    pub product_id: u16,
    /// Manufacturer
    pub manufacturer: Option<String>,
    /// Product name
    pub product_name: Option<String>,
    /// Serial number
    pub serial_number: Option<String>,
    /// chip model
    pub chip: String,
}

impl Default for KeyboardInfo {
    fn default() -> Self {
        Self {
            vendor_id: 0x4c4b,
            product_id: 0x4643,
            manufacturer: Some("RMK".to_string()),
            product_name: Some("RMK Keyboard".to_string()),
            serial_number: Some("00000001".to_string()),
            chip: "rp2040".to_string(),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct MatrixConfig {
    pub rows: u8,
    pub cols: u8,
    pub layers: u8,
    pub input_pins: Vec<String>,
    pub output_pins: Vec<String>,
}

/// Config for storage
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct StorageConfig {
    /// Start address of local storage, MUST BE start of a sector.
    /// If start_addr is set to 0(this is the default value), the last `num_sectors` sectors will be used.
    #[serde(default)]
    pub start_addr: usize,
    // Number of sectors used for storage, >= 2.
    #[serde(default = "default_num_sectors")]
    pub num_sectors: u8,
    ///
    #[serde(default = "default_bool")]
    pub enabled: bool,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            start_addr: 0,
            num_sectors: 2,
            enabled: false,
        }
    }
}

#[derive(Clone, Default, Debug, Deserialize)]
pub struct BleConfig {
    pub enabled: bool,
    pub battery_pin: Option<String>,
    pub charge_state: Option<PinConfig>,
}

/// Config for lights
#[derive(Clone, Default, Debug, Deserialize)]
pub struct LightConfig {
    pub capslock: Option<PinConfig>,
    pub scrolllock: Option<PinConfig>,
    pub numslock: Option<PinConfig>,
}

fn default_num_sectors() -> u8 {
    2
}

fn default_bool() -> bool {
    false
}

#[derive(Clone, Default, Debug, Deserialize)]
pub struct PinConfig {
    pub pin: String,
    #[serde(default = "default_bool")]
    pub low_active: bool,
}
