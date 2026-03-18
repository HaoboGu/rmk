//! Keyboard control actions.

use postcard::experimental::max_size::MaxSize;
#[cfg(feature = "rmk_protocol")]
use postcard_schema::Schema;

/// Actions for controlling the keyboard or changing the keyboard's state, for example, enable/disable a particular function
#[derive(Debug, Copy, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(MaxSize)]
#[cfg_attr(feature = "rmk_protocol", derive(Schema))]
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
