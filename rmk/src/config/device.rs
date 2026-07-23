/// Keyboard device identity for USB descriptors and BLE advertising
#[derive(Clone, Copy, Debug)]
pub struct DeviceConfig<'a> {
    /// Vender id
    pub vid: u16,
    /// Product id
    pub pid: u16,
    /// Manufacturer
    pub manufacturer: &'a str,
    /// Product name
    pub product_name: &'a str,
    /// Serial number
    pub serial_number: &'a str,
}

/// Version string embedded in the USB serial number: `rmk:<version>`.
///
/// With the `vial` feature it becomes `vial:f64c2b3c;rmk:<version>`. The `vial:` marker comes
/// first because BLE's serial characteristic is length limited.
///
/// Set `DeviceConfig::serial_number` explicitly to override with your own value.
#[cfg(feature = "vial")]
pub const RMK_BUILD_INFO: &str = concat!("vial:f64c2b3c;rmk:", env!("CARGO_PKG_VERSION"));

#[cfg(not(feature = "vial"))]
pub const RMK_BUILD_INFO: &str = concat!("rmk:", env!("CARGO_PKG_VERSION"));

impl Default for DeviceConfig<'_> {
    fn default() -> Self {
        Self {
            vid: 0x4c4b,
            pid: 0x4643,
            manufacturer: "RMK",
            product_name: "RMK Keyboard",
            serial_number: RMK_BUILD_INFO,
        }
    }
}
