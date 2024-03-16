use super::{
    battery_service::BatteryService,
    device_information_service::{DeviceInformation, DeviceInformationService, PnPID, VidSource},
    hid_service::HidService,
};
use crate::{
    config::KeyboardUsbConfig,
    hid::{ConnectionType, ConnectionTypeWrapper, HidError, HidWriterWrapper},
};
use defmt::error;
use nrf_softdevice::{
    ble::{
        gatt_server::{self, RegisterError, Service, WriteOp},
        Connection,
    },
    Softdevice,
};
use usbd_hid::descriptor::AsInputReport;

/// Wrapper struct for writing via BLE
pub(crate) struct BleHidWriter<'a, const N: usize> {
    conn: &'a Connection,
    handle: u16,
}

impl<'a, const N: usize> ConnectionTypeWrapper for BleHidWriter<'a, N> {
    fn get_conn_type(&self) -> crate::hid::ConnectionType {
        ConnectionType::BLE
    }
}

impl<'a, const N: usize> HidWriterWrapper for BleHidWriter<'a, N> {
    async fn write_serialize<IR: AsInputReport>(&mut self, r: &IR) -> Result<(), HidError> {
        use ssmarshal::serialize;
        let mut buf: [u8; N] = [0; N];
        match serialize(&mut buf, &r) {
            Ok(_) => self.write(&buf).await,
            Err(_) => Err(HidError::ReportSerializeError),
        }
    }

    async fn write(&mut self, report: &[u8]) -> Result<(), HidError> {
        gatt_server::notify_value(self.conn, self.handle, report).map_err(|e| {
            error!("Send ble report error: {}", e);
            match e {
                gatt_server::NotifyValueError::Disconnected => HidError::BleDisconnected,
                gatt_server::NotifyValueError::Raw(_) => HidError::BleRawError,
            }
        })
    }
}

impl<'a, const N: usize> BleHidWriter<'a, N> {
    pub(crate) fn new(conn: &'a Connection, handle: u16) -> Self {
        Self { conn, handle }
    }
}

// BleServer
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
                vendor_id: 0x4C4B,
                product_id: 0x4643,
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
