use trouble_host::prelude::*;

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

impl AsGatt for PnPID {
    const MIN_SIZE: usize = core::mem::size_of::<PnPID>();

    const MAX_SIZE: usize = core::mem::size_of::<PnPID>();

    fn as_gatt(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self as *const _ as *const u8, core::mem::size_of::<PnPID>()) }
    }
}

impl Default for PnPID {
    fn default() -> Self {
        Self {
            vid_source: VidSource::UsbIF,
            vendor_id: 0xE118,
            product_id: 0x0001,
            product_version: 0x0001,
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct DeviceInformation {
    pub(crate) manufacturer_name: Option<&'static str>,
    pub(crate) model_number: Option<&'static str>,
    pub(crate) serial_number: Option<&'static str>,
    pub(crate) hw_rev: Option<&'static str>,
    pub(crate) fw_rev: Option<&'static str>,
    pub(crate) sw_rev: Option<&'static str>,
}

#[gatt_service(uuid = service::DEVICE_INFORMATION)]
pub(crate) struct DeviceInformationService {
    #[characteristic(uuid = "2a50", read)]
    pub(crate) pnp_id: PnPID,
    #[characteristic(
        uuid = "2a25",
        read,
        value = heapless::String::try_from("vial:f64c2b3c:000001").unwrap()
    )]
    pub(crate) serial_number: heapless::String<20>,
    #[characteristic(uuid = "2a29", read)]
    pub(crate) manufacturer_name: heapless::String<20>,
}
