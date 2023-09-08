pub mod usb_config;

use usb_config::UsbHidConfig;

pub struct KeyboardConfig<'a> {
    pub usb_config: UsbHidConfig<'a>,
}

pub static KEYBOARD_CONFIG: KeyboardConfig = KeyboardConfig {
    usb_config: UsbHidConfig {
        pid: 0x1234,
        vid: 0x1233,
        manufacturer: "RMK",
        product: "RMK product",
        serial_number: "0",
    },
};
