pub(crate) mod descriptor;
pub(crate) mod device_info;

#[cfg(feature = "_esp_ble")]
pub mod esp;
#[cfg(feature = "_nrf_ble")]
pub mod nrf;

use defmt::error;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Receiver};
#[cfg(any(feature = "nrf52840_ble", feature = "nrf52833_ble"))]
pub use nrf::SOFTWARE_VBUS;
use nrf_softdevice::ble::{gatt_server, Connection};
use ssmarshal::serialize;

use crate::{
    hid::HidError,
    keyboard::{KEYBOARD_REPORT_CHANNEL, REPORT_CHANNEL_SIZE},
    reporter::{Report, Reporter},
};

pub(crate) struct BleKeyboardReporter<'a> {
    conn: &'a Connection,
    keyboard_handle: u16,
    media_handle: u16,
    system_control_handle: u16,
    mouse_handle: u16,
}

impl<'a> BleKeyboardReporter<'a> {
    pub(crate) fn new(
        conn: &'a Connection,
        keyboard_handle: u16,
        media_handle: u16,
        system_control_handle: u16,
        mouse_handle: u16,
    ) -> Self {
        Self {
            conn,
            keyboard_handle,
            media_handle,
            system_control_handle,
            mouse_handle,
        }
    }
    async fn write(&mut self, handle: u16, report: &[u8]) -> Result<(), HidError> {
        gatt_server::notify_value(self.conn, handle, report).map_err(|e| {
            error!("Send ble report error: {}", e);
            match e {
                gatt_server::NotifyValueError::Disconnected => HidError::BleDisconnected,
                gatt_server::NotifyValueError::Raw(_) => HidError::BleRawError,
            }
        })
    }
}

impl Reporter for BleKeyboardReporter<'_> {
    type ReportType = Report;

    fn report_receiver(
        &self,
    ) -> Receiver<'_, CriticalSectionRawMutex, Self::ReportType, REPORT_CHANNEL_SIZE> {
        KEYBOARD_REPORT_CHANNEL.receiver()
    }

    async fn write_report(&mut self, report: Self::ReportType) {
        match report {
            Report::KeyboardReport(keyboard_report) => {
                let mut buf = [0u8; 8];
                match serialize(&mut buf, &keyboard_report) {
                    Ok(_) => self.write(self.keyboard_handle, &buf).await,
                    Err(_) => Err(HidError::ReportSerializeError),
                }
            }
            Report::MouseReport(mouse_report) => {
                let mut buf = [0u8; 5];
                match serialize(&mut buf, &mouse_report) {
                    Ok(_) => self.write(self.mouse_handle, &buf).await,
                    Err(_) => Err(HidError::ReportSerializeError),
                }
            }
            Report::MediaKeyboardReport(media_keyboard_report) => {
                let mut buf = [0u8; 2];
                match serialize(&mut buf, &media_keyboard_report) {
                    Ok(_) => self.write(self.media_handle, &buf).await,
                    Err(_) => Err(HidError::ReportSerializeError),
                }
            }
            Report::SystemControlReport(system_control_report) => {
                let mut buf = [0u8; 1];
                match serialize(&mut buf, &system_control_report) {
                    Ok(_) => self.write(self.system_control_handle, &buf).await,
                    Err(_) => Err(HidError::ReportSerializeError),
                }
            }
        };
    }
}

pub(crate) fn as_bytes<T: Sized>(p: &T) -> &[u8] {
    unsafe {
        ::core::slice::from_raw_parts((p as *const T) as *const u8, ::core::mem::size_of::<T>())
    }
}
