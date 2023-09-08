pub struct UsbHidConfig<'a> {
    pub pid: u16,
    pub vid: u16,
    pub manufacturer: &'a str,
    pub product: &'a str,
    pub serial_number: &'a str,
}
