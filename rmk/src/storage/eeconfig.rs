use packed_struct::prelude::*;

/// Keyboard configurations which should be saved in eeprom.
#[derive(Default)]
pub struct Eeconfig {
    eeprom_enable: bool,
    default_layer: u8,
    keymap_config: EeKeymapConfig,
    backlight_config: EeBacklightConfig,
    audio_config: EeAudioConfig,
    rgb_light_config: EeRgbLightConfig,
    layout_option: u32,
}

#[derive(PackedStruct, Debug, Clone, Copy, Default)]
#[packed_struct(bit_numbering = "msb0", bytes = "2")]
pub(crate) struct EeKeymapConfig {
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

#[derive(PackedStruct, Debug, Default)]
#[packed_struct(bit_numbering = "msb0")]
pub(crate) struct EeBacklightConfig {
    #[packed_field(bits = "0")]
    enable: bool,
    #[packed_field(bits = "1")]
    breathing: bool,
    #[packed_field(bits = "2")]
    reserved: bool,
    #[packed_field(bits = "3..=7")]
    level: u8,
}

#[derive(PackedStruct, Debug, Default)]
#[packed_struct(bit_numbering = "msb0")]
pub(crate) struct EeAudioConfig {
    #[packed_field(bits = "0")]
    enable: bool,
    #[packed_field(bits = "1")]
    clicky_enable: bool,
    #[packed_field(bits = "2..=7")]
    level: u8,
}

#[derive(PackedStruct, Debug, Default)]
#[packed_struct(bit_numbering = "msb0")]
pub(crate) struct EeRgbLightConfig {
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
