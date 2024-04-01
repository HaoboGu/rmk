use embedded_hal::digital::{InputPin, OutputPin};
use esp32_nimble::{
    enums::*, hid::*, utilities::mutex::Mutex, BLEAdvertisementData, BLECharacteristic, BLEDevice,
    BLEHIDDevice, BLEServer,
};

use crate::{action::KeyAction, config::RmkConfig};

/// Initialize and run the BLE keyboard service, with given keyboard usb config.
/// Can only be used on nrf52 series microcontrollers with `nrf-softdevice` crate.
/// This function never returns.
///
/// # Arguments
///
/// * `keymap` - default keymap definition
/// * `driver` - embassy usb driver instance
/// * `input_pins` - input gpio pins
/// * `output_pins` - output gpio pins
/// * `keyboard_config` - other configurations of the keyboard, check [RmkConfig] struct for details
/// * `spwaner` - embassy task spwaner, used to spawn nrf_softdevice background task
pub async fn initialize_esp_ble_keyboard_with_config_and_run<
    In: InputPin,
    Out: OutputPin,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    keymap: [[[KeyAction; COL]; ROW]; NUM_LAYER],
    input_pins: [In; ROW],
    output_pins: [Out; COL],
    keyboard_config: RmkConfig<'static, Out>,
) -> ! {
    let device = BLEDevice::take();
    device
        .security()
        .set_auth(AuthReq::all())
        .set_io_cap(SecurityIOCap::NoInputNoOutput);
    let server = device.get_server();
    let mut hid = BLEHIDDevice::new(server);
    let input_keyboard = hid.input_report(1);
    let output_keyboard = hid.output_report(2);
    let input_media_keys = hid.input_report(3);
    hid.manufacturer("Espressif");
    hid.pnp(0x02, 0x05ac, 0x820a, 0x0210);
    hid.hid_info(0x00, 0x01);
    // TODO: fixme
    hid.report_map(&[1,2,3,4]);

    hid.set_battery_level(100);
    loop {}
}
