pub(crate) mod descriptor;
pub(crate) mod device_info;

#[cfg(feature = "esp_ble")]
pub mod esp;
#[cfg(feature = "nrf_ble")]
pub mod nrf;

use embassy_time::Timer;
use embedded_hal::digital::{InputPin, OutputPin};
#[cfg(feature = "nrf52840_ble")]
pub use nrf::SOFTWARE_VBUS;

use crate::{hid::HidWriterWrapper, Keyboard};

/// BLE keyboard task, run the keyboard with the ble server
pub(crate) async fn keyboard_ble_task<
    'a,
    W: HidWriterWrapper,
    W2: HidWriterWrapper,
    W3: HidWriterWrapper,
    W4: HidWriterWrapper,
    In: InputPin,
    Out: OutputPin,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    keyboard: &mut Keyboard<'a, In, Out, ROW, COL, NUM_LAYER>,
    ble_keyboard_writer: &mut W,
    ble_media_writer: &mut W2,
    ble_system_control_writer: &mut W3,
    ble_mouse_writer: &mut W4,
) {
    // Wait 1 seconds, ensure that gatt server has been started
    Timer::after_secs(1).await;
    loop {
        let _ = keyboard.scan_matrix().await;

        keyboard.send_keyboard_report(ble_keyboard_writer).await;
        keyboard.send_media_report(ble_media_writer).await;
        keyboard
            .send_system_control_report(ble_system_control_writer)
            .await;
        keyboard.send_mouse_report(ble_mouse_writer).await;
    }
}
