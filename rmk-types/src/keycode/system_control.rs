//! System control keycodes.

use postcard::experimental::max_size::MaxSize;
use serde::{Deserialize, Serialize};

use super::hid::HidKeyCode;

/// Keys in `Generic Desktop Page`, generally used for system control
/// Ref: <https://www.usb.org/sites/default/files/documents/hut1_12v2.pdf#page=26>
#[non_exhaustive]
#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord, MaxSize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum SystemControlKey {
    No = 0x00,
    PowerDown = 0x81,
    Sleep = 0x82,
    WakeUp = 0x83,
    Restart = 0x8F,
}

impl SystemControlKey {
    /// Convert SystemControlKey to the corresponding HidKeyCode
    pub fn to_hid_keycode(&self) -> Option<HidKeyCode> {
        match self {
            SystemControlKey::PowerDown => Some(HidKeyCode::SystemPower),
            SystemControlKey::Sleep => Some(HidKeyCode::SystemSleep),
            SystemControlKey::WakeUp => Some(HidKeyCode::SystemWake),
            _ => None,
        }
    }
}
