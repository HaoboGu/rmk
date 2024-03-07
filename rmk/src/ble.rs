pub(crate) mod advertise;
mod battery_service;
pub(crate) mod bonder;
pub(crate) mod constants;
pub(crate) mod descriptor;
mod device_information_service;
mod hid_service;
pub(crate) mod hid_service2;
pub(crate) mod server;

use self::{bonder::FlashOperationMessage, server::BleServer};
use crate::{
    ble::bonder::{BondInfo, FLASH_CHANNEL},
    keyboard::Keyboard,
};
use core::{convert::Infallible, ops::Range};
use defmt::info;
use embassy_time::Timer;
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_storage::nor_flash::NorFlash;
use nrf_softdevice::Flash;
use sequential_storage::{
    cache::NoCache,
    map::{remove_item, store_item},
};

/// Background task of nrf_softdevice
#[embassy_executor::task]
pub(crate) async fn softdevice_task(sd: &'static nrf_softdevice::Softdevice) -> ! {
    sd.run().await
}

pub(crate) const FLASH_START: u32 = 0x80000;
pub(crate) const FLASH_END: u32 = 0x82000;
pub(crate) const CONFIG_FLASH_RANGE: Range<u32> = 0x80000..0x82000;

#[embassy_executor::task]
pub(crate) async fn flash_task(f: &'static mut Flash) -> ! {
    let mut storage_data_buffer = [0_u8; 128];
    loop {
        let info: FlashOperationMessage = FLASH_CHANNEL.receive().await;
        match info {
            FlashOperationMessage::Clear(key) => {
                info!("Clearing bond info slot_num: {}", key);
                remove_item::<BondInfo, _>(
                    f,
                    CONFIG_FLASH_RANGE,
                    NoCache::new(),
                    &mut storage_data_buffer,
                    key,
                )
                .await
                .unwrap();
            }
            FlashOperationMessage::BondInfo(b) => {
                info!("Saving item: {}", info);
                store_item::<BondInfo, _>(
                    f,
                    CONFIG_FLASH_RANGE,
                    NoCache::new(),
                    &mut storage_data_buffer,
                    &b,
                )
                .await
                .unwrap();
            }
        };
    }
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
