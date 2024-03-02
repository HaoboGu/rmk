use super::{
    battery_service::BatteryService,
    device_information_service::{DeviceInformation, DeviceInformationService, PnPID, VidSource},
    hid_service::HidService,
};
use crate::config::KeyboardUsbConfig;
use nrf_softdevice::{
    ble::{
        gatt_server::{self, RegisterError, Service, WriteOp},
        Connection,
    },
    Softdevice,
};

pub(crate) struct BleServer {
    _dis: DeviceInformationService,
    pub(crate) bas: BatteryService,
    pub(crate) hid: HidService,
}

impl BleServer {
    pub fn new(
        sd: &mut Softdevice,
        usb_config: KeyboardUsbConfig<'static>,
    ) -> Result<Self, RegisterError> {
        let dis = DeviceInformationService::new(
            sd,
            &PnPID {
                vid_source: VidSource::UsbIF,
                vendor_id: 0xDEAD,
                product_id: 0xBEEF,
                product_version: 0x0000,
            },
            DeviceInformation {
                manufacturer_name: usb_config.manufacturer,
                model_number: usb_config.product_name,
                serial_number: usb_config.serial_number,
                ..Default::default()
            },
        )?;

        let bas = BatteryService::new(sd)?;

        let hid = HidService::new(sd)?;

        Ok(Self {
            _dis: dis,
            bas,
            hid,
        })
    }
}

impl gatt_server::Server for BleServer {
    type Event = ();

    fn on_write(
        &self,
        conn: &Connection,
        handle: u16,
        _op: WriteOp,
        _offset: usize,
        data: &[u8],
    ) -> Option<Self::Event> {
        self.hid.on_write(conn, handle, data);
        self.bas.on_write(handle, data);
        None
    }
}
