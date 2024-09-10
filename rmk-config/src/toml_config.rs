use serde_derive::Deserialize;

/// Configurations for RMK keyboard.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct KeyboardTomlConfig {
    pub keyboard: KeyboardInfo,
    pub matrix: MatrixConfig,
    #[serde(default = "default_light_config")]
    pub light: LightConfig,
    #[serde(default = "default_storage_config")]
    pub storage: StorageConfig,
    pub ble: Option<BleConfig>,
    #[serde(default = "default_dep")]
    pub dependency: DependencyConfig,
    pub layout: Option<LayoutConfig>,
    pub split: Option<SplitConfig>
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
    /// enable usb
    #[serde(default = "default_true")]
    pub usb_enable: bool,
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
            usb_enable: true,
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
    #[serde(default = "default_true")]
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
    pub battery_adc_pin: Option<String>,
    pub charge_state: Option<PinConfig>,
    pub charge_led: Option<PinConfig>,
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

fn default_false() -> bool {
    false
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Default, Debug, Deserialize)]
pub struct PinConfig {
    pub pin: String,
    #[serde(default = "default_false")]
    pub low_active: bool,
}

/// Configurations for usb
#[derive(Clone, Debug, Default, Deserialize)]
pub struct DependencyConfig {
    /// Enable defmt log or not
    #[serde(default = "default_true")]
    pub defmt_log: bool,
}

fn default_dep() -> DependencyConfig {
    DependencyConfig { defmt_log: true }
}

fn default_light_config() -> LightConfig {
    LightConfig::default()
}

fn default_storage_config() -> StorageConfig {
    StorageConfig::default()
}

/// Configurations for usb
#[derive(Clone, Debug, Default, Deserialize)]
pub struct LayoutConfig {
    pub keymap: Vec<Vec<Vec<String>>>,
}


/// Configurations for split keyboards
#[derive(Clone, Debug, Default, Deserialize)]
pub struct SplitConfig {
    pub connection: String,
    pub central: SplitBoardConfig,
    pub peripheral: Vec<SplitBoardConfig>,
}

/// Configurations for each split board
#[derive(Clone, Debug, Default, Deserialize)]
pub struct SplitBoardConfig {
    pub rows: u8,
    pub cols: u8,
    pub row_offset: u8,
    pub col_offset: u8,
    pub ble_addr: Option<[u8; 6]>,
    pub serial: Option<Vec<SerialConfig>>,
    pub input_pins: Vec<String>,
    pub output_pins: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct SerialConfig {
    pub instance: String,
    pub tx_pin: String,
    pub rx_pin: String,
}