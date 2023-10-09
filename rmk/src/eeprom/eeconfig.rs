use super::Eeprom;
use byteorder::{BigEndian, ByteOrder};
use embedded_storage::nor_flash::NorFlash;
use log::error;
use packed_struct::prelude::*;

/// EEPROM magic value.
/// If the first 2 bytes of eeprom equals it, eeprom is enabled.
pub(crate) const EEPROM_MAGIC: u16 = 0xFEE6;
const EEPROM_DISABLED_MAGIC: u16 = 0xFFFF;

/// Start index of eeprom magic value
const MAGIC_ADDR: u16 = 0;
/// Size of eeprom magic value: 2 bytes
const MAGIC_SIZE: usize = 2;
/// Index of default layer in eeprom
const DEFAULT_LAYER_START: usize = 2;
/// Size of default layer: 1 byte
const DEFAULT_LAYER_SIZE: usize = 1;
/// Start index of keymap config
const KEYMAP_CONFIG_ADDR: u16 = 3;
/// Size of keymap config: 2 bytes
const KEYMAP_CONFIG_SIZE: usize = 2;
/// Start index of backlight config
const BACKLIGHT_CONFIG_ADDR: u16 = 5;
/// Size of backlight config: 1 byte
const BACKLIGHT_CONFIG_SIZE: usize = 1;
/// Start index of audio config
const AUDIO_CONFIG_ADDR: u16 = 6;
/// Size of audio config: 1 byte
const AUDIO_CONFIG_SIZE: usize = 1;
/// Start index of rgb config
const RGB_CONFIG_ADDR: u16 = 7;
/// Size of rgb config: 5 bytes
const RGB_CONFIG_SIZE: usize = 5;
/// Start index of layout option in eeprom
const LAYOUT_OPTION_ADDR: u16 = 12;
/// Size of layout option: 4 bytes
const LAYOUT_OPTION_SIZE: usize = 4;

/// Start index of dynamic keymap
pub(crate) const DYNAMIC_KEYMAP_ADDR: u16 = 15;

impl<
        F: NorFlash,
        const STORAGE_START_ADDR: u32,
        const STORAGE_SIZE: u32,
        const EEPROM_SIZE: usize,
    > Eeprom<F, STORAGE_START_ADDR, STORAGE_SIZE, EEPROM_SIZE>
{
    /// Initialize eeprom with default eeconfig
    pub fn init_with_default_config(&mut self) {
        self.set_enable(true);
        self.set_default_layer(0);
        // TODO: move all default configs to a single place
        self.set_keymap_config(EeKeymapConfig {
            swap_control_capslock: false,
            capslock_to_control: false,
            swap_lalt_lgui: false,
            swap_ralt_rgui: false,
            no_gui: false,
            swap_grave_esc: false,
            swap_backslash_backspace: false,
            nkro: false,
            swap_lctl_lgui: false,
            swap_rctl_rgui: false,
            oneshot_enable: false,
            swap_escape_capslock: false,
            autocorrect_enable: false,
            ..EeKeymapConfig::default()
        });
        self.set_backlight_config(EeBacklightConfig {
            enable: false,
            breathing: false,
            reserved: false,
            level: 0,
        });
        self.set_audio_config(EeAudioConfig {
            enable: false,
            clicky_enable: false,
            level: 0,
        });
        self.set_rgb_light_config(EeRgbLightConfig {
            enable: false,
            mode: 0,
            hue: 0,
            sat: 0,
            val: 0,
            speed: 0,
        });
        self.set_layout_option(0);
    }

    /// Enable or disable eeprom by writing magic value
    pub fn set_enable(&mut self, enabled: bool) {
        let magic = if enabled {
            EEPROM_MAGIC
        } else {
            EEPROM_DISABLED_MAGIC
        };
        // Write eeprom
        let mut buf = [0xFF; 2];
        BigEndian::write_u16(&mut buf, magic);
        self.write_byte(0, &mut buf);
    }

    /// Returns eeprom magic value stored in EEPROM
    pub fn get_magic(&mut self) -> u16 {
        // ALWAYS read magic from backend store
        if let Some(record) = self.read_record(0) {
            record.data
        } else {
            EEPROM_DISABLED_MAGIC
        }
    }

    /// Set default layer
    pub fn set_default_layer(&mut self, default_layer: u8) {
        self.write_byte(DEFAULT_LAYER_START as u16, &[default_layer]);
    }

    /// Returns current default layer
    pub fn get_default_layer(&self) -> u8 {
        self.cache[DEFAULT_LAYER_START]
    }

    /// Set keymap config
    pub fn set_keymap_config(&mut self, config: EeKeymapConfig) {
        let mut buf = match config.pack() {
            Ok(b) => b,
            Err(e) => {
                error!("Pack keymap config error: {:?}", e);
                [0xFF; 2]
            }
        };
        self.write_byte(KEYMAP_CONFIG_ADDR, &mut buf);
    }

    /// Returns keymap config as `EeKeymapConfig`
    pub fn get_keymap_config(&self) -> Option<EeKeymapConfig> {
        match EeKeymapConfig::unpack_from_slice(
            self.read_byte(KEYMAP_CONFIG_ADDR, KEYMAP_CONFIG_SIZE),
        ) {
            Ok(config) => Some(config),
            Err(e) => {
                error!("Unpack keymap config error: {:?}", e);
                None
            }
        }
    }

    /// Set backlight config
    pub fn set_backlight_config(&mut self, config: EeBacklightConfig) {
        let mut buf = match config.pack() {
            Ok(b) => b,
            Err(e) => {
                error!("Pack backlight config error: {:?}", e);
                [0xFF; 1]
            }
        };
        self.write_byte(BACKLIGHT_CONFIG_ADDR, &mut buf);
    }

    /// Returns backlight config as `EeBacklightConfig`
    pub fn get_backlight_config(&self) -> Option<EeBacklightConfig> {
        match EeBacklightConfig::unpack_from_slice(
            self.read_byte(BACKLIGHT_CONFIG_ADDR, BACKLIGHT_CONFIG_SIZE),
        ) {
            Ok(config) => Some(config),
            Err(e) => {
                error!("Unpack backlight config error: {:?}", e);
                None
            }
        }
    }

    /// Set audio config
    pub fn set_audio_config(&mut self, config: EeAudioConfig) {
        let mut buf = match config.pack() {
            Ok(b) => b,
            Err(e) => {
                error!("Pack audio config error: {:?}", e);
                [0xFF; 1]
            }
        };
        self.write_byte(AUDIO_CONFIG_ADDR, &mut buf);
    }

    /// Returns audio config as `EeAudioConfig`
    pub fn get_audio_config(&self) -> Option<EeAudioConfig> {
        match EeAudioConfig::unpack_from_slice(self.read_byte(AUDIO_CONFIG_ADDR, AUDIO_CONFIG_SIZE))
        {
            Ok(config) => Some(config),
            Err(e) => {
                error!("Unpack audio config error: {:?}", e);
                None
            }
        }
    }

    /// Set rgb light config
    pub fn set_rgb_light_config(&mut self, config: EeRgbLightConfig) {
        let mut buf = match config.pack() {
            Ok(b) => b,
            Err(e) => {
                error!("Pack rgb light config error: {:?}", e);
                [0xFF; 5]
            }
        };
        self.write_byte(RGB_CONFIG_ADDR, &mut buf);
    }

    /// Returns rgb light config as `EeRgbLightConfig`
    pub fn get_rgb_light_config(&self) -> Option<EeRgbLightConfig> {
        match EeRgbLightConfig::unpack_from_slice(self.read_byte(RGB_CONFIG_ADDR, RGB_CONFIG_SIZE))
        {
            Ok(config) => Some(config),
            Err(e) => {
                error!("Unpack rgb light config error: {:?}", e);
                None
            }
        }
    }

    /// Set layout option
    pub fn set_layout_option(&mut self, option: u32) {
        let mut buf = [0xFF; 4];
        BigEndian::write_u32(&mut buf, option);
        self.write_byte(LAYOUT_OPTION_ADDR, &mut buf);
    }

    /// Returns layout option
    pub fn get_layout_option(&self) -> u32 {
        BigEndian::read_u32(self.read_byte(LAYOUT_OPTION_ADDR, LAYOUT_OPTION_SIZE))
    }
}

#[derive(PackedStruct, Debug, Default)]
#[packed_struct(bit_numbering = "msb0", bytes = "2")]
pub struct EeKeymapConfig {
    #[packed_field(bits = "0")]
    swap_control_capslock: bool,
    #[packed_field(bits = "1")]
    capslock_to_control: bool,
    #[packed_field(bits = "2")]
    swap_lalt_lgui: bool,
    #[packed_field(bits = "3")]
    swap_ralt_rgui: bool,
    #[packed_field(bits = "4")]
    no_gui: bool,
    #[packed_field(bits = "5")]
    swap_grave_esc: bool,
    #[packed_field(bits = "6")]
    swap_backslash_backspace: bool,
    #[packed_field(bits = "7")]
    nkro: bool,
    #[packed_field(bits = "8")]
    swap_lctl_lgui: bool,
    #[packed_field(bits = "9")]
    swap_rctl_rgui: bool,
    #[packed_field(bits = "10")]
    oneshot_enable: bool,
    #[packed_field(bits = "11")]
    swap_escape_capslock: bool,
    #[packed_field(bits = "12")]
    autocorrect_enable: bool,
    _reserved: ReservedOne<packed_bits::Bits<3>>,
}

#[derive(PackedStruct, Debug)]
#[packed_struct(bit_numbering = "msb0")]
pub struct EeBacklightConfig {
    #[packed_field(bits = "0")]
    enable: bool,
    #[packed_field(bits = "1")]
    breathing: bool,
    #[packed_field(bits = "2")]
    reserved: bool,
    #[packed_field(bits = "3..=7")]
    level: u8,
}

#[derive(PackedStruct, Debug)]
#[packed_struct(bit_numbering = "msb0")]
pub struct EeAudioConfig {
    #[packed_field(bits = "0")]
    enable: bool,
    #[packed_field(bits = "1")]
    clicky_enable: bool,
    #[packed_field(bits = "2..=7")]
    level: u8,
}

#[derive(PackedStruct, Debug)]
#[packed_struct(bit_numbering = "msb0")]
pub struct EeRgbLightConfig {
    #[packed_field(bits = "0")]
    enable: bool,
    #[packed_field(bits = "1..=7")]
    mode: u8,
    #[packed_field(bits = "8..=15")]
    hue: u8,
    #[packed_field(bits = "16..=23")]
    sat: u8,
    #[packed_field(bits = "24..=31")]
    val: u8,
    #[packed_field(bits = "32..=39")]
    speed: u8,
}
