pub(crate) mod advertise;
mod battery_service;
pub(crate) mod bonder;
pub(crate) mod descriptor;
mod device_information_service;
mod hid_service;
pub(crate) mod server;
pub(crate) mod spec;
#[cfg(feature = "esp32_ble")]
pub mod esp;

use self::server::BleServer;
use crate::{hid::HidWriterWrapper, keyboard::Keyboard};
use core::{convert::Infallible, mem};
use embassy_time::Timer;
use embedded_hal::digital::{InputPin, OutputPin};
use nrf_softdevice::{ble::Connection, raw, Config};

/// Maximum number of bonded devices
pub const BONDED_DEVICE_NUM: usize = 8;

// TODO: Be compatible with more nRF chip models
#[cfg(feature = "nrf52840_ble")]
use embassy_nrf::usb::vbus_detect::SoftwareVbusDetect;
#[cfg(feature = "nrf52840_ble")]
use once_cell::sync::OnceCell;
#[cfg(feature = "nrf52840_ble")]
/// Software Vbus detect when using BLE + USB
pub static SOFTWARE_VBUS: OnceCell<SoftwareVbusDetect> = OnceCell::new();

#[cfg(feature = "nrf52840_ble")]
/// Background task of nrf_softdevice
#[embassy_executor::task]
pub(crate) async fn softdevice_task(sd: &'static nrf_softdevice::Softdevice) -> ! {
    use nrf_softdevice::SocEvent;

    unsafe {
        nrf_softdevice::raw::sd_power_usbpwrrdy_enable(1);
        nrf_softdevice::raw::sd_power_usbdetected_enable(1);
        nrf_softdevice::raw::sd_power_usbremoved_enable(1);
        // nrf_softdevice::raw::sd_clock_hfclk_request();
    };

    let software_vbus = SOFTWARE_VBUS.get_or_init(|| SoftwareVbusDetect::new(true, true));

    sd.run_with_callback(|event: SocEvent| {
        match event {
            SocEvent::PowerUsbRemoved => software_vbus.detected(false),
            SocEvent::PowerUsbDetected => software_vbus.detected(true),
            SocEvent::PowerUsbPowerReady => software_vbus.ready(),
            _ => {}
        };
    })
    .await
}

// nRF52832 doesn't have USB, so the softdevice_task is different
#[cfg(feature = "nrf52832_ble")]
#[embassy_executor::task]
pub(crate) async fn softdevice_task(sd: &'static nrf_softdevice::Softdevice) -> ! {
    sd.run().await
}

/// Create default nrf ble config
pub(crate) fn nrf_ble_config(keyboard_name: &str) -> Config {
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

/// BLE keyboard task, run the keyboard with the ble server
pub(crate) async fn keyboard_ble_task<
    'a,
    W: HidWriterWrapper,
    W2: HidWriterWrapper,
    W3: HidWriterWrapper,
    W4: HidWriterWrapper,
    In: InputPin<Error = Infallible>,
    Out: OutputPin<Error = Infallible>,
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
    // Wait 2 seconds, ensure that gatt server has been started
    Timer::after_secs(2).await;
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

/// BLE keyboard task, run the keyboard with the ble server
pub(crate) async fn ble_battery_task(ble_server: &BleServer, conn: &Connection) {
    // Wait 2 seconds, ensure that gatt server has been started
    Timer::after_secs(2).await;
    ble_server.set_battery_value(conn, &80);
    loop {
        // TODO: A real battery service
        Timer::after_secs(10).await
    }
}
