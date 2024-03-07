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
use core::{convert::Infallible, mem, ops::Range};
use defmt::info;
use embassy_time::Timer;
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_storage::nor_flash::NorFlash;
use nrf_softdevice::{raw, Config, Flash};
use sequential_storage::{
    cache::NoCache,
    map::{remove_item, store_item},
};


/// Flash range which used to save bonding info
pub(crate) const CONFIG_FLASH_RANGE: Range<u32> = 0x80000..0x82000;
/// Maximum number of bonded devices
pub const BONDED_DEVICE_NUM: usize = 2;

/// Create default nrf ble config
pub fn nrf_ble_config(keyboard_name: &str) -> Config {
    Config {
        clock: Some(raw::nrf_clock_lf_cfg_t {
            source: raw::NRF_CLOCK_LF_SRC_RC as u8,
            rc_ctiv: 16,
            rc_temp_ctiv: 2,
            accuracy: raw::NRF_CLOCK_LF_ACCURACY_500_PPM as u8,
        }),
        conn_gap: Some(raw::ble_gap_conn_cfg_t {
            conn_count: 6,
            event_length: 24,
        }),
        conn_gatt: Some(raw::ble_gatt_conn_cfg_t { att_mtu: 256 }),
        gatts_attr_tab_size: Some(raw::ble_gatts_cfg_attr_tab_size_t {
            attr_tab_size: raw::BLE_GATTS_ATTR_TAB_SIZE_DEFAULT,
        }),
        gap_role_count: Some(raw::ble_gap_cfg_role_count_t {
            adv_set_count: 1,
            periph_role_count: 3,
            central_role_count: 3,
            central_sec_count: 0,
            _bitfield_1: raw::ble_gap_cfg_role_count_t::new_bitfield_1(0),
        }),
        gap_device_name: Some(raw::ble_gap_cfg_device_name_t {
            p_value: keyboard_name.as_ptr() as _,
            current_len: keyboard_name.len() as u16,
            max_len: keyboard_name.len() as u16,
            write_perm: unsafe { mem::zeroed() },
            _bitfield_1: raw::ble_gap_cfg_device_name_t::new_bitfield_1(
                raw::BLE_GATTS_VLOC_STACK as u8,
            ),
        }),
        ..Default::default()
    }
}

/// Background task of nrf_softdevice
#[embassy_executor::task]
pub(crate) async fn softdevice_task(sd: &'static nrf_softdevice::Softdevice) -> ! {
    sd.run().await
}

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
