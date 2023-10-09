pub mod usb_config;

use usb_config::UsbHidConfig;

pub struct KeyboardConfig<'a> {
    pub usb_config: UsbHidConfig<'a>,
}

pub static KEYBOARD_CONFIG: KeyboardConfig = KeyboardConfig {
    usb_config: UsbHidConfig {
        pid: 0x4643,
        vid: 0x4C4B,
        manufacturer: "RMK",
        product: "RMK product",
        serial_number: "vial:f64c2b3c:000001",
    },
};
