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

impl<
        F: NorFlash,
        const STORAGE_START_ADDR: u32,
        const STORAGE_SIZE: u32,
        const EEPROM_SIZE: usize,
    > Eeprom<F, STORAGE_START_ADDR, STORAGE_SIZE, EEPROM_SIZE>
{
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
    pub fn get_magic(&self) -> u16 {
        BigEndian::read_u16(self.read_byte(MAGIC_ADDR, MAGIC_SIZE))
    }

    /// Set default layer
    pub fn set_default_layer(&mut self, default_layer: u8) {
        self.write_byte(DEFAULT_LAYER_START as u16, &[default_layer]);
    }

    /// Returns current default layer
    pub fn get_default_layer(&self) -> u8 {
        self.cache[DEFAULT_LAYER_START]
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

    /// Returns layout option
    pub fn get_layout_option(&self) -> u32 {
        BigEndian::read_u32(self.read_byte(LAYOUT_OPTION_ADDR, LAYOUT_OPTION_SIZE))
    }
}

#[derive(PackedStruct, Debug)]
#[packed_struct(bit_numbering = "msb0")]
pub struct EeKeymapConfig {
    /// If this magic value equals to `KEYMAP_MAGIC`, the eeprom keymap config is enabled.
    #[packed_field(bits = "0..=7")]
    keymap_enable_magic: u8,
    #[packed_field(bits = "8")]
    swap_control_capslock: bool,
    #[packed_field(bits = "9")]
    capslock_to_control: bool,
    #[packed_field(bits = "10")]
    swap_lalt_lgui: bool,
    #[packed_field(bits = "11")]
    swap_ralt_rgui: bool,
    #[packed_field(bits = "12")]
    no_gui: bool,
    #[packed_field(bits = "13")]
    swap_grave_esc: bool,
    #[packed_field(bits = "14")]
    swap_backslash_backspace: bool,
    #[packed_field(bits = "15")]
    nkro: bool,
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
