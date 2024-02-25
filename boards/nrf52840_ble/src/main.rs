#![no_std]
#![no_main]

use core::mem;
use defmt::{info, *};
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_nrf as _;
use panic_probe as _;
use rmk::{
    ble::{HidSecurityHandler, Server},
    nrf_softdevice::{
        self,
        ble::{
            advertisement_builder::{
                AdvertisementDataType, Flag, LegacyAdvertisementBuilder,
                LegacyAdvertisementPayload, ServiceList, ServiceUuid16,
            },
            gatt_server, peripheral,
        },
        raw, Softdevice,
    },
};
#[embassy_executor::task]
async fn softdevice_task(sd: &'static Softdevice) -> ! {
    sd.run().await
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Hello NRF BLE!");

    let config = nrf_softdevice::Config {
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
            p_value: b"HelloRust" as *const u8 as _,
            current_len: 9,
            max_len: 9,
            write_perm: unsafe { mem::zeroed() },
            _bitfield_1: raw::ble_gap_cfg_device_name_t::new_bitfield_1(
                raw::BLE_GATTS_VLOC_STACK as u8,
            ),
        }),
        ..Default::default()
    };

    let sd = Softdevice::enable(&config);
    let server = unwrap!(Server::new(sd, "12345678"));
    unwrap!(spawner.spawn(softdevice_task(sd)));

    static ADV_DATA: LegacyAdvertisementPayload = LegacyAdvertisementBuilder::new()
        .flags(&[Flag::GeneralDiscovery, Flag::LE_Only])
        .services_16(
            ServiceList::Incomplete,
            &[
                ServiceUuid16::BATTERY,
                ServiceUuid16::HUMAN_INTERFACE_DEVICE,
            ],
        )
        .full_name("HelloRust")
        // Change the appearance (icon of the bluetooth device) to a keyboard
        .raw(AdvertisementDataType::APPEARANCE, &[0xC1, 0x03])
        .build();

    static SCAN_DATA: LegacyAdvertisementPayload = LegacyAdvertisementBuilder::new()
        .services_16(
            ServiceList::Complete,
            &[
                ServiceUuid16::DEVICE_INFORMATION,
                ServiceUuid16::BATTERY,
                ServiceUuid16::HUMAN_INTERFACE_DEVICE,
            ],
        )
        .build();

    static SEC: HidSecurityHandler = HidSecurityHandler {};

    loop {
        let config = peripheral::Config::default();
        let adv = peripheral::ConnectableAdvertisement::ScannableUndirected {
            adv_data: &ADV_DATA,
            scan_data: &SCAN_DATA,
        };
        let conn = peripheral::advertise_pairable(sd, adv, &config, &SEC)
            .await
            .unwrap();

        info!("advertising done!");

        // Run the GATT server on the connection. This returns when the connection gets disconnected.
        let e = gatt_server::run(&conn, &server, |_| {}).await;

        info!("gatt_server run exited with error: {:?}", e);
    }
}
