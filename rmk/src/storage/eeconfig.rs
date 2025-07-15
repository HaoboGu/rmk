use bitfield_struct::bitfield;

/// Keyboard configurations which should be saved in eeprom.
#[derive(Default)]
pub(crate) struct Eeconfig {
    eeprom_enable: bool,
    default_layer: u8,
    keymap_config: EeKeymapConfig,
    backlight_config: EeBacklightConfig,
    audio_config: EeAudioConfig,
    rgb_light_config: EeRgbLightConfig,
    layout_option: u32,
}

#[bitfield(u16, order = Msb, defmt = cfg(feature = "defmt"))]
#[derive(PartialEq, Eq)]

pub(crate) struct EeKeymapConfig {
    #[bits(1)]
    swap_control_capslock: bool,
    #[bits(1)]
    capslock_to_control: bool,
    #[bits(1)]
    swap_lalt_lgui: bool,
    #[bits(1)]
    swap_ralt_rgui: bool,
    #[bits(1)]
    no_gui: bool,
    #[bits(1)]
    swap_grave_esc: bool,
    #[bits(1)]
    swap_backslash_backspace: bool,
    #[bits(1)]
    nkro: bool,
    #[bits(1)]
    swap_lctl_lgui: bool,
    #[bits(1)]
    swap_rctl_rgui: bool,
    #[bits(1)]
    oneshot_enable: bool,
    #[bits(1)]
    swap_escape_capslock: bool,
    #[bits(1)]
    autocorrect_enable: bool,
    #[bits(3)]
    _reserved: u8,
}

#[bitfield(u8, order = Msb, defmt = cfg(feature = "defmt"))]
#[derive(PartialEq, Eq)]

pub(crate) struct EeBacklightConfig {
    #[bits(1)]
    enable: bool,
    #[bits(1)]
    breathing: bool,
    #[bits(1)]
    reserved: bool,
    #[bits(5)]
    level: u8,
}

#[bitfield(u8, order = Msb, defmt = cfg(feature = "defmt"))]
#[derive(PartialEq, Eq)]

pub(crate) struct EeAudioConfig {
    #[bits(1)]
    enable: bool,
    #[bits(1)]
    clicky_enable: bool,
    #[bits(6)]
    level: u8,
}

#[bitfield(u64, order = Msb, defmt = cfg(feature = "defmt"))]
#[derive(PartialEq, Eq)]
pub(crate) struct EeRgbLightConfig {
    #[bits(1)]
    enable: bool,
    #[bits(8)]
    mode: u8,
    #[bits(8)]
    hue: u8,
    #[bits(8)]
    sat: u8,
    #[bits(8)]
    val: u8,
    #[bits(8)]
    speed: u8,
    #[bits(23)]
    _reserved: u32,
}
