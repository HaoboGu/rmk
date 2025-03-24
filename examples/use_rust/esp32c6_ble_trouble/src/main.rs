#![no_std]
#![no_main]

mod keymap;
mod macros;
mod vial;

use crate::keymap::*;
use crate::vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};
use bt_hci::controller::ExternalController;
use embassy_executor::Spawner;
use esp_hal::{clock::CpuClock, timer::timg::TimerGroup};
use esp_storage::FlashStorage;
use esp_wifi::ble::controller::BleConnector;
use rmk::channel::EVENT_CHANNEL;
use rmk::config::StorageConfig;
use rmk::futures::future::join3;
use rmk::input_device::Runnable;
use rmk::run_devices;
use rmk::{
    config::{ControllerConfig, RmkConfig, VialConfig},
    initialize_keymap_and_storage,
    keyboard::Keyboard,
    light::LightController,
    matrix::TestMatrix,
    storage::{async_flash_wrapper, Storage},
};
use {esp_alloc as _, esp_backtrace as _};

#[esp_hal_embassy::main]
async fn main(_s: Spawner) {
    esp_println::logger::init_logger_from_env();
    let peripherals = esp_hal::init({
        let mut config = esp_hal::Config::default();
        config.cpu_clock = CpuClock::max();
        config
    });

    let flash = FlashStorage::new();
    let flash = async_flash_wrapper(flash);

    esp_alloc::heap_allocator!(64 * 1024);
    let timg0 = TimerGroup::new(peripherals.TIMG0);

    let mut rng = esp_hal::rng::Trng::new(peripherals.RNG, peripherals.ADC1);

    let init = esp_wifi::init(timg0.timer0, rng.rng.clone(), peripherals.RADIO_CLK).unwrap();

    let systimer = esp_hal::timer::systimer::SystemTimer::new(peripherals.SYSTIMER);
    esp_hal_embassy::init(systimer.alarm0);

    let bluetooth = peripherals.BT;
    let connector = BleConnector::new(&init, bluetooth);
    let controller: ExternalController<_, 64> = ExternalController::new(connector);

    // RMK config
    let vial_config = VialConfig::new(VIAL_KEYBOARD_ID, VIAL_KEYBOARD_DEF);
    let storage_config = StorageConfig {
        start_addr: 0x3f0000,
        num_sectors: 16,
        ..Default::default()
    };
    let rmk_config = RmkConfig {
        vial_config,
        storage_config,
        ..Default::default()
    };

    let mut matrix: TestMatrix<ROW, COL> = TestMatrix::new();
    // Initialize the storage and keymap
    let mut default_keymap = keymap::get_default_keymap();
    let (keymap, mut storage) = initialize_keymap_and_storage(
        &mut default_keymap,
        flash,
        rmk_config.storage_config,
        rmk_config.behavior_config.clone(),
    )
    .await;

    use esp_hal::gpio::Output;
    let mut keyboard = Keyboard::new(&keymap, rmk_config.behavior_config.clone());
    // Initialize the light controller
    let mut light_controller: LightController<Output> =
        LightController::new(ControllerConfig::default().light_config);

    join3(
        run_devices! (
            (matrix) => EVENT_CHANNEL,
        ),
        keyboard.run(),
        rmk::ble::trouble::run::<_, _, _, _, ROW, COL, NUM_LAYER>(
            &keymap,
            &mut storage,
            controller,
            &mut rng,
            &mut light_controller,
            rmk_config,
        ),
    )
    .await;
}
