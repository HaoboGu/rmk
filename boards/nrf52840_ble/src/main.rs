#![no_std]
#![no_main]

#[macro_use]
mod macros;
mod keymap;
mod vial;

use crate::keymap::{COL, NUM_LAYER, ROW};
use core::{cell::RefCell, mem};
use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_nrf::{
    self as _,
    gpio::{AnyPin, Input, Output},
    nvmc::Nvmc,
};
use panic_probe as _;
use rmk::{
    config::{KeyboardUsbConfig, RmkConfig, VialConfig},
    keymap::KeyMap,
    nrf_softdevice::{self, raw},
};
use static_cell::StaticCell;
use vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};

const EEPROM_SIZE: usize = 128;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
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

    let p = embassy_nrf::init(Default::default());

    // Pin config
    let (input_pins, output_pins) = config_matrix_pins_nrf!(peripherals: p, input: [P0_07, P0_08, P0_11, P0_12], output: [P0_13, P0_14, P0_15]);
    // Use internal flash to emulate eeprom
    let f = Nvmc::new(p.NVMC);
    // Keymap + eeprom config
    static MY_KEYMAP: StaticCell<RefCell<KeyMap<Nvmc, EEPROM_SIZE, ROW, COL, NUM_LAYER>>> =
        StaticCell::new();
    let keymap = MY_KEYMAP.init(RefCell::new(KeyMap::new(
        crate::keymap::KEYMAP,
        Some(f),
        None,
    )));

    let keyboard_usb_config = KeyboardUsbConfig::new(
        0x4c4b,
        0x4643,
        Some("Haobo"),
        Some("RMK Keyboard"),
        Some("00000001"),
    );
    let vial_config = VialConfig::new(VIAL_KEYBOARD_ID, VIAL_KEYBOARD_DEF);
    let keyboard_config = RmkConfig {
        usb_config: keyboard_usb_config,
        vial_config,
        ..Default::default()
    };

    rmk::initialize_ble_keyboard_with_config_and_run(
        keymap,
        input_pins,
        output_pins,
        config,
        keyboard_config,
    )
    .await;
}
