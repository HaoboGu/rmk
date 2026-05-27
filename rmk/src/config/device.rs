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

/// RMK version string embedded in the USB serial number: `rmk:<version>-<git-hash>`.
///
/// When the `vial` feature is enabled the string also includes `;vial:f64c2b3c` so that
/// vial-gui can discover the device without requiring a hardcoded serial number override.
///
/// Downstream firmware can extend this with their own build info using
/// `const_format::concatcp!(rmk::RMK_BUILD_INFO, ";my-firmware:", env!("CARGO_PKG_VERSION"), "-", env!("MY_GIT_HASH"))`.
#[cfg(feature = "vial")]
pub const RMK_BUILD_INFO: &str = concat!(
    "rmk:",
    env!("CARGO_PKG_VERSION"),
    "-",
    env!("RMK_GIT_HASH"),
    ";vial:f64c2b3c"
);

#[cfg(not(feature = "vial"))]
pub const RMK_BUILD_INFO: &str = concat!("rmk:", env!("CARGO_PKG_VERSION"), "-", env!("RMK_GIT_HASH"));

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
