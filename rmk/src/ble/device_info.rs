#[repr(u8)]
#[derive(Clone, Copy)]
pub enum VidSource {
    BluetoothSIG = 1,
    UsbIF = 2,
}

/// PnP ID characteristic is a set of values used to craete an unique device ID.
/// These values are used to identify all devices of a given type/model/version using numbers.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct PnPID {
    pub vid_source: VidSource,
    pub vendor_id: u16,
    pub product_id: u16,
    pub product_version: u16,
}

#[derive(Debug, Default, defmt::Format)]
pub struct DeviceInformation {
    pub manufacturer_name: Option<&'static str>,
    pub model_number: Option<&'static str>,
    pub serial_number: Option<&'static str>,
    pub hw_rev: Option<&'static str>,
    pub fw_rev: Option<&'static str>,
    pub sw_rev: Option<&'static str>,
}
