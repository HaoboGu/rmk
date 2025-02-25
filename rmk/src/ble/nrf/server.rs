use super::{
    battery_service::{BatteryService, BatteryServiceEvent},
    device_information_service::DeviceInformationService,
    hid_service::{HidService, HidServiceEvent},
    vial_service::{BleVialService, VialServiceEvent},
};
use crate::ble::device_info::{DeviceInformation, PnPID, VidSource};
use crate::config::KeyboardUsbConfig;
use nrf_softdevice::{
    ble::{
        gatt_server::{self, RegisterError, Service, WriteOp},
        security::SecurityHandler,
        Connection,
    },
    Softdevice,
};

/// Wrapper struct for writing via BLE
pub(crate) struct BleHidWriter<'a, const N: usize> {
    conn: &'a Connection,
    handle: u16,
}

// BleServer saves all services, which have connection handles in it
pub(crate) struct BleServer {
    _dis: DeviceInformationService,
    pub(crate) bas: BatteryService,
    pub(crate) hid: HidService,
    pub(crate) vial: BleVialService,
    bonder: &'static dyn SecurityHandler,
}

impl BleServer {
    pub(crate) fn new(
        sd: &mut Softdevice,
        usb_config: KeyboardUsbConfig<'static>,
        bonder: &'static dyn SecurityHandler,
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
                manufacturer_name: Some(usb_config.manufacturer),
                model_number: Some(usb_config.product_name),
                serial_number: Some(usb_config.serial_number),
                ..Default::default()
            },
        )?;

        let bas = BatteryService::new(sd)?;

        let hid = HidService::new(sd)?;

        let vial = BleVialService::new(sd)?;

        Ok(Self {
            _dis: dis,
            bas,
            hid,
            vial,
            bonder,
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
        if let Some(event) = self.hid.on_write(handle, data) {
            match event {
                HidServiceEvent::InputKeyboardCccdWrite
                | HidServiceEvent::InputMediaKeyCccdWrite
                | HidServiceEvent::InputMouseKeyCccdWrite
                | HidServiceEvent::InputSystemKeyCccdWrite => {
                    info!("{:?}, handle: {}, data: {:?}", event, handle, data);
                    self.bonder.save_sys_attrs(conn)
                }
                HidServiceEvent::OutputKeyboard => (),
            }
        }
        if let Some(event) = self.bas.on_write(handle, data) {
            match event {
                BatteryServiceEvent::BatteryLevelCccdWrite { notifications } => {
                    info!(
                        "BatteryLevelCccdWrite, handle: {}, data: {:?}, notif: {}",
                        handle, data, notifications
                    );
                    self.bonder.save_sys_attrs(conn)
                }
            }
        }
        if let Some(event) = self.vial.on_write(handle, data) {
            match event {
                VialServiceEvent::InputVialKeyCccdWrite => {
                    info!("InputVialCccdWrite, handle: {}, data: {:?}", handle, data);
                    self.bonder.save_sys_attrs(conn)
                }
                VialServiceEvent::OutputVial => (),
            }
        }

        None
    }
}
