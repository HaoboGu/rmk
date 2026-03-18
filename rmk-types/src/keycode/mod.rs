//! Keycode definitions including HID keycodes, media keys, and system control keycodes.

mod ascii;
mod consumer;
mod hid;
mod system_control;

pub use ascii::{from_ascii, to_ascii};
pub use consumer::ConsumerKey;
pub use hid::HidKeyCode;
use serde::{Deserialize, Serialize};
pub use system_control::SystemControlKey;

/// Key codes which are not in the HID spec, but still commonly used
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(postcard::experimental::max_size::MaxSize, postcard_schema::Schema)]
#[cfg_attr(feature = "_codegen", derive(strum::VariantNames))]
pub enum SpecialKey {
    // GraveEscape
    GraveEscape,
    // Repeat
    Repeat,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(postcard::experimental::max_size::MaxSize, postcard_schema::Schema)]
pub enum KeyCode {
    Hid(HidKeyCode),
    Consumer(ConsumerKey),
    SystemControl(SystemControlKey),
}
