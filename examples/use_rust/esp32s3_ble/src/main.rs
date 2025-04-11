#![no_std]
#![no_main]

mod keymap;
#[macro_use]
mod macros;
mod vial;

use core::ptr::addr_of_mut;

use bt_hci::controller::ExternalController;
use embassy_executor::Spawner;
use esp_hal::clock::CpuClock;
use esp_hal::gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull};
use esp_hal::otg_fs::asynch::{Config, Driver};
use esp_hal::otg_fs::Usb;
use esp_hal::timer::timg::TimerGroup;
use esp_storage::FlashStorage;
use esp_wifi::ble::controller::BleConnector;
use rmk::ble::trouble::build_ble_stack;
use rmk::channel::EVENT_CHANNEL;
use rmk::config::{ControllerConfig, RmkConfig, StorageConfig, VialConfig};
use rmk::debounce::default_debouncer::DefaultDebouncer;
use rmk::futures::future::join3;
use rmk::input_device::Runnable;
use rmk::keyboard::Keyboard;
use rmk::light::LightController;
use rmk::matrix::Matrix;
use rmk::storage::async_flash_wrapper;
use rmk::{initialize_keymap_and_storage, run_devices, run_rmk, HostResources};
use {esp_alloc as _, esp_backtrace as _};

use crate::keymap::*;
use crate::vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};

#[esp_hal_embassy::main]
async fn main(_s: Spawner) {
    // Initialize the peripherals and bluetooth controller
    esp_println::logger::init_logger_from_env();
    let peripherals = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()));
    esp_alloc::heap_allocator!(size: 72 * 1024);
    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let mut rng = esp_hal::rng::Trng::new(peripherals.RNG, peripherals.ADC1);
    let init = esp_wifi::init(timg0.timer0, rng.rng.clone(), peripherals.RADIO_CLK).unwrap();
    let systimer = esp_hal::timer::systimer::SystemTimer::new(peripherals.SYSTIMER);
    esp_hal_embassy::init(systimer.alarm0);
    let bluetooth = peripherals.BT;
    let connector = BleConnector::new(&init, bluetooth);
    let controller: ExternalController<_, 20> = ExternalController::new(connector);
    let central_addr = [0x18, 0xe2, 0x21, 0x80, 0xc0, 0xc7];
    let mut host_resources = HostResources::new();
    let stack = build_ble_stack(controller, central_addr, &mut rng, &mut host_resources).await;

    // Initialize USB
    static mut EP_MEMORY: [u8; 1024] = [0; 1024];
    let usb = Usb::new(peripherals.USB0, peripherals.GPIO20, peripherals.GPIO19);
    // Create the driver, from the HAL.
    let config = Config::default();
    let usb_driver = Driver::new(usb, unsafe { &mut *addr_of_mut!(EP_MEMORY) }, config);

    // Initialize the flash
    let flash = FlashStorage::new();
    let flash = async_flash_wrapper(flash);

    // Initialize the IO pins
    let (input_pins, output_pins) = config_matrix_pins_esp!(peripherals: peripherals, input: [GPIO6, GPIO7, GPIO21, GPIO35], output: [GPIO3, GPIO4, GPIO5]);

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

    // Initialze keyboard stuffs
    // Initialize the storage and keymap
    let mut default_keymap = keymap::get_default_keymap();
    let (keymap, mut storage) = initialize_keymap_and_storage(
        &mut default_keymap,
        flash,
        rmk_config.storage_config,
        rmk_config.behavior_config.clone(),
    )
    .await;

    // Initialize the matrix and keyboard
    let debouncer = DefaultDebouncer::<ROW, COL>::new();
    let mut matrix = Matrix::<_, _, _, ROW, COL>::new(input_pins, output_pins, debouncer);
    // let mut matrix = rmk::matrix::TestMatrix::<ROW, COL>::new();
    let mut keyboard = Keyboard::new(&keymap, rmk_config.behavior_config.clone()); // Initialize the light controller

    // Initialize the light controller
    let mut light_controller: LightController<Output> = LightController::new(ControllerConfig::default().light_config);

    join3(
        run_devices! (
            (matrix) => EVENT_CHANNEL,
        ),
        keyboard.run(), // Keyboard is special
        run_rmk(
            &keymap,
            usb_driver,
            &stack,
            &mut storage,
            &mut light_controller,
            rmk_config,
        ),
    )
    .await;
}
