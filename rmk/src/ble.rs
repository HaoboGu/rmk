pub(crate) mod advertise;
mod battery_service;
pub(crate) mod bonder;
pub(crate) mod constants;
pub(crate) mod descriptor;
mod device_information_service;
mod hid_service;
pub(crate) mod hid_service2;
pub(crate) mod server;

use self::{bonder::StoredBondInfo, hid_service2::BleServer2, server::BleServer};
use crate::{ble::bonder::FLASH_CHANNEL, keyboard::Keyboard};
use core::convert::Infallible;
use defmt::info;
use embassy_time::Timer;
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_storage::nor_flash::NorFlash;
use embedded_storage_async::nor_flash::NorFlash as _;
use nrf_softdevice::Flash;

/// Background task of nrf_softdevice
#[embassy_executor::task]
pub(crate) async fn softdevice_task(sd: &'static nrf_softdevice::Softdevice) -> ! {
    sd.run().await
}

#[embassy_executor::task]
pub(crate) async fn flash_task(f: &'static mut Flash) -> ! {
    loop {
        let info: StoredBondInfo = FLASH_CHANNEL.receive().await;
        match info {
            StoredBondInfo::Peer(p) => {
                info!("Received Peer {}", p);
                // Write data should be aligned
                let mut s = [0_u8; 52];
                s[0..50].copy_from_slice(&p.to_slice());
                f.erase(0x80000, 0x81000).await.unwrap();
                f.write(0x80000, &s).await.unwrap();
            }
            StoredBondInfo::SystemAttribute(sys_attr) => {
                info!(
                    "Received SystemAttr {:#X}",
                    sys_attr.data[0..sys_attr.length]
                );
                let s = sys_attr.to_slice();
                info!("SysAttr Slice: {:#X}", s);
                f.erase(0x81000, 0x82000).await.unwrap();
                f.write(0x81000, &s).await.unwrap();
            }
            StoredBondInfo::Clear => {
                info!("Clearing bond info");
                f.erase(0x80000, 0x82000).await.unwrap();
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
    ble_server: &BleServer2,
    conn: &nrf_softdevice::ble::Connection,
) {
    // Wait 2 seconds, ensure that gatt server has been started
    Timer::after_secs(2).await;
    // TODO: A real battery service
    // ble_server.battery_service.battery_level_notify(conn, &50).unwrap();
    // ble_server.set_battery_value(conn, &50);
    loop {
        let _ = keyboard.keyboard_task().await;
        keyboard.send_ble_report(ble_server, conn).await;
    }
}
