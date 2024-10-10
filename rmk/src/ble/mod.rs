pub(crate) mod descriptor;
pub(crate) mod device_info;

#[cfg(feature = "_esp_ble")]
pub mod esp;
#[cfg(feature = "_nrf_ble")]
pub mod nrf;

use defmt::{debug, error};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Receiver};
use embassy_time::Timer;
#[cfg(any(feature = "nrf52840_ble", feature = "nrf52833_ble"))]
pub use nrf::SOFTWARE_VBUS;

use crate::{
    hid::HidWriterWrapper,
    keyboard::{write_other_report_to_host, KeyboardReportMessage},
    usb::descriptor::CompositeReportType,
};

/// BLE communication task, send reports to host via BLE.
/// TODO: Merge `ble_communication_task` and `communication_task` into one task.
pub(crate) async fn ble_communication_task<
    'a,
    W: HidWriterWrapper,
    W2: HidWriterWrapper,
    W3: HidWriterWrapper,
    W4: HidWriterWrapper,
>(
    keyboard_report_receiver: &Receiver<'a, CriticalSectionRawMutex, KeyboardReportMessage, 8>,
    ble_keyboard_writer: &mut W,
    ble_media_writer: &mut W2,
    ble_system_control_writer: &mut W3,
    ble_mouse_writer: &mut W4,
) {
    // Wait 1 seconds, ensure that gatt server has been started
    Timer::after_secs(1).await;
    loop {
        match keyboard_report_receiver.receive().await {
            KeyboardReportMessage::KeyboardReport(report) => {
                debug!(
                    "Send keyboard report via BLE: {=[u8]:#X}, modifier: {:b}",
                    report.keycodes, report.modifier
                );
                match ble_keyboard_writer.write_serialize(&report).await {
                    Ok(()) => {}
                    Err(e) => error!("Send keyboard report error: {}", e),
                };
            }
            KeyboardReportMessage::CompositeReport(report, report_type) => {
                match report_type {
                    CompositeReportType::Media => {
                        write_other_report_to_host(report, report_type, ble_media_writer).await
                    }
                    CompositeReportType::Mouse => {
                        write_other_report_to_host(report, report_type, ble_mouse_writer).await
                    }
                    CompositeReportType::System => {
                        write_other_report_to_host(report, report_type, ble_system_control_writer)
                            .await
                    }
                    CompositeReportType::None => (),
                };
            }
        }
    }
}

pub(crate) fn as_bytes<T: Sized>(p: &T) -> &[u8] {
    unsafe {
        ::core::slice::from_raw_parts((p as *const T) as *const u8, ::core::mem::size_of::<T>())
    }
}
