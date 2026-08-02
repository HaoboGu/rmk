//! The keycode tables, handed to JS whole.
//!
//! A configurator has to offer every keycode the firmware understands, but a
//! TypeScript union is erased at runtime — it cannot be iterated. Without these,
//! a host keeps its own copy of the table and silently stops offering whatever
//! the firmware gains next. The lists come from the same enums the wire uses, so
//! `rmk-types` stays the only place a keycode is declared.

use rynk::rmk_types::keycode::{ConsumerKey, HidKeyCode, SystemControlKey};
use wasm_bindgen::prelude::*;

/// Every HID keycode, in wire order.
#[wasm_bindgen]
pub fn all_hid_keycodes() -> Vec<HidKeyCode> {
    HidKeyCode::all().collect()
}

/// Every consumer-page key, in wire order.
#[wasm_bindgen]
pub fn all_consumer_keys() -> Vec<ConsumerKey> {
    ConsumerKey::all().collect()
}

/// Every system-control key, in wire order.
#[wasm_bindgen]
pub fn all_system_control_keys() -> Vec<SystemControlKey> {
    SystemControlKey::all().collect()
}
