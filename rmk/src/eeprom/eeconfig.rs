use packed_struct::prelude::*;

const EEPROM_MAGIC: u16 = 0xFEE6;
const KEYMAP_MAGIC: u16 = 0x1400;

/// Keyboard configurations stored in EEPROM.
/// TODO: Read `EeConfig` values from EEPROM in-place: `EeConfig::unpack_from_slice(&eeprom.cache[0..16]);`
#[derive(PackedStruct, Debug)]
pub struct EeConfig {
    /// If this magic value equals to `EEPROM_MAGIC`, the EEPROM is enabled.
    #[packed_field(endian = "msb")]
    magic: u16,
    /// Default layer of the keyboard
    default_layer: u8,
    /// Keymap configs
    #[packed_field(size_bytes = "2")]
    keymap_config: EeKeymapConfig,
    /// Backlight level
    #[packed_field(size_bytes = "1")]
    backlight: EeBacklightConfig,
    /// Audio configs     
    #[packed_field(size_bytes = "1")]
    audio: EeAudioConfig,
    /// RGB light configs
    #[packed_field(size_bytes = "5")]
    rgb_light: EeRgbLightConfig,
    /// Via layout option 
    #[packed_field(endian="msb")]
    layout_option: u32,
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
