extern crate rmk;

/// Convert a key `k!(key)` to the u8 representation in hid report.
/// For example, `HidKeyCode::A` will be converted to `0x04`.
#[macro_export]
macro_rules! kc_to_u8 {
    ($key: ident) => {
        rmk::types::keycode::HidKeyCode::$key as u8
    };
}
