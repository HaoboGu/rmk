//! Keyboard control actions.

/// Actions for controlling the keyboard or changing the keyboard's state, for example, enable/disable a particular function
#[derive(Debug, Copy, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(postcard::experimental::max_size::MaxSize)]
#[cfg_attr(feature = "protocol", derive(postcard_schema::Schema))]
#[cfg_attr(feature = "_codegen", derive(strum::VariantNames))]
pub enum KeyboardAction {
    Bootloader,
    Reboot,
    DebugToggle,
    ClearEeprom,
    OutputAuto,
    OutputUsb,
    OutputBluetooth,
    ComboOn,
    ComboOff,
    ComboToggle,
    CapsWordToggle,
}
