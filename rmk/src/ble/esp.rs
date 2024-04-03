pub(crate) mod server;

use self::server::BleServer;
use crate::{
    action::KeyAction, config::RmkConfig, flash::EmptyFlashWrapper, hid::HidWriterWrapper as _,
    keyboard::Keyboard, keymap::KeyMap, storage::Storage,
};
use core::cell::RefCell;
use defmt::{info, warn};
use embassy_embedded_hal::adapter::BlockingAsync;
use embassy_time::Timer;
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_storage::nor_flash::NorFlash;
use usbd_hid::descriptor::KeyboardReport;

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
    // F: NorFlash,
    In: InputPin,
    Out: OutputPin,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    keymap: [[[KeyAction; COL]; ROW]; NUM_LAYER],
    input_pins: [In; ROW],
    output_pins: [Out; COL],
    // flash: Option<F>,
    keyboard_config: RmkConfig<'static, Out>,
) -> ! {
    // Wrap `embedded-storage` to `embedded-storage-async`
    // let async_flash = flash.map(|f| embassy_embedded_hal::adapter::BlockingAsync::new(f));
    let (mut storage, keymap) = (
        None::<EmptyFlashWrapper>,
        RefCell::new(
            KeyMap::<ROW, COL, NUM_LAYER>::new_from_storage::<EmptyFlashWrapper>(keymap, None)
                .await,
        ),
    );
    // Initialize storage and keymap
    // let (mut storage, keymap) = match async_flash {
    //     Some(f) => {
    //         let mut s = Storage::new(f, &keymap, keyboard_config.storage_config).await;
    //         let keymap = RefCell::new(
    //             KeyMap::<ROW, COL, NUM_LAYER>::new_from_storage(keymap, Some(&mut s)).await,
    //         );
    //         (Some(s), keymap)
    //     }
    //     None => {

    //         (None, keymap)
    //     }
    // };

    let mut keyboard = Keyboard::new(input_pins, output_pins, &keymap);

    loop {
        info!("Advertising..");
        let mut ble_server = BleServer::new(keyboard_config.usb_config);
        info!("Waitting for connection..");
        ble_server.wait_for_connection().await;
        info!("BLE connected!");
        let keyboard_fut = keyboard_ble_task(&mut keyboard, &mut ble_server);
        keyboard_fut.await;

        warn!("BLE disconnected!")
    }
}

pub(crate) async fn keyboard_ble_task<
    'a,
    In: InputPin,
    Out: OutputPin,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    keyboard: &mut Keyboard<'a, In, Out, ROW, COL, NUM_LAYER>,
    ble_server: &mut BleServer,
) {
    loop {
        // let _ = keyboard.scan_matrix().await;
        info!("Writing");
        let report: KeyboardReport = KeyboardReport {
            modifier: 0,
            reserved: 0,
            leds: 0,
            keycodes: [0x04, 0, 0, 0, 0, 0],
        };
        ble_server
            .write_serialize(&report)
            .await
            .unwrap();
        Timer::after_secs(3).await;

        // keyboard.send_keyboard_report(ble_server).await;
    }
}
