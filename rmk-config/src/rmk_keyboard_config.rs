/// Configurations for usb
#[derive(Clone, Copy, Debug)]
pub struct KeyboardUsbConfig<'a> {
    /// Vender id
    pub vid: u16,
    /// Product id
    pub pid: u16,
    /// Manufacturer
    pub manufacturer: Option<&'a str>,
    /// Product name
    pub product_name: Option<&'a str>,
    /// Serial number
    pub serial_number: Option<&'a str>,
}