pub(crate) mod advertise;
mod battery_service;
pub(crate) mod bonder;
pub(crate) mod constants;
mod descriptor;
mod device_information_service;
mod hid_service;
pub(crate) mod server;

use self::server::BleServer;
use crate::keyboard::Keyboard;
use core::convert::Infallible;
use embassy_time::Timer;
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_storage::nor_flash::NorFlash;

/// Background task of nrf_softdevice
#[embassy_executor::task]
pub(crate) async fn softdevice_task(sd: &'static nrf_softdevice::Softdevice) -> ! {
    sd.run().await
}

/// BLE keyboard task, run the keyboard with the ble server
pub(crate) async fn keyboard_ble_task<
    'a,
    In: InputPin<Error = Infallible>,
    Out: OutputPin<Error = Infallible>,
    F: NorFlash,
    const EEPROM_SIZE: usize,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    keyboard: &mut Keyboard<'a, In, Out, F, EEPROM_SIZE, ROW, COL, NUM_LAYER>,
    ble_server: &BleServer,
    conn: &nrf_softdevice::ble::Connection,
) {
    // Wait 2 seconds, ensure that gatt server has been started
    Timer::after_secs(2).await;
    // TODO: A real battery service
    ble_server.set_battery_value(conn, &50);
    loop {
        let _ = keyboard.keyboard_task().await;
        keyboard.send_ble_report(ble_server, conn).await;
    }
}
