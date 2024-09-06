#[repr(u8)]
#[derive(Clone, Copy)]
pub(crate) enum VidSource {
    BluetoothSIG = 1,
    UsbIF = 2,
}

/// PnP ID characteristic is a set of values used to craete an unique device ID.
/// These values are used to identify all devices of a given type/model/version using numbers.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub(crate) struct PnPID {
    pub(crate) vid_source: VidSource,
    pub(crate) vendor_id: u16,
    pub(crate) product_id: u16,
    pub(crate) product_version: u16,
}

#[derive(Debug, Default, defmt::Format)]
pub(crate) struct DeviceInformation {
    pub(crate) manufacturer_name: Option<&'static str>,
    pub(crate) model_number: Option<&'static str>,
    pub(crate) serial_number: Option<&'static str>,
    pub(crate) hw_rev: Option<&'static str>,
    pub(crate) fw_rev: Option<&'static str>,
    pub(crate) sw_rev: Option<&'static str>,
}
