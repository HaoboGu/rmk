#![no_main]
#![no_std]

#[macro_use]
mod macros;
mod keymap;
mod vial;

use defmt::info;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_nrf::{
    bind_interrupts,
    gpio::{AnyPin, Input, Output},
    interrupt::InterruptExt,
    nvmc::Nvmc,
    peripherals,
    usb::{self, Driver, vbus_detect::HardwareVbusDetect},
};
use keymap::{COL, ROW};
use panic_probe as _;
use rmk::{
    channel::EVENT_CHANNEL,
    config::{ControllerConfig, RmkConfig, VialConfig},
    debounce::default_debouncer::DefaultDebouncer,
    futures::future::join3,
    initialize_keymap_and_storage,
    input_device::Runnable,
    keyboard::Keyboard,
    light::LightController,
    matrix::Matrix,
    run_devices, run_rmk,
    storage::async_flash_wrapper,
};
use vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};

bind_interrupts!(struct Irqs {
    USBD => usb::InterruptHandler<peripherals::USBD>;
    CLOCK_POWER => usb::vbus_detect::InterruptHandler;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("RMK start!");
    // Initialize peripherals
    let mut config = ::embassy_nrf::config::Config::default();
    config.gpiote_interrupt_priority = ::embassy_nrf::interrupt::Priority::P3;
    config.time_interrupt_priority = ::embassy_nrf::interrupt::Priority::P3;
    ::embassy_nrf::interrupt::USBD.set_priority(::embassy_nrf::interrupt::Priority::P2);
    ::embassy_nrf::interrupt::CLOCK_POWER.set_priority(::embassy_nrf::interrupt::Priority::P2);
    let p = ::embassy_nrf::init(config);
    info!("Enabling ext hfosc...");
    ::embassy_nrf::pac::CLOCK.tasks_hfclkstart().write_value(1);
    while ::embassy_nrf::pac::CLOCK.events_hfclkstarted().read() != 1 {}

    // Usb config
    let driver = Driver::new(p.USBD, Irqs, HardwareVbusDetect::new(Irqs));

    // Pin config
    let (input_pins, output_pins) = config_matrix_pins_nrf!(peripherals: p, input: [P0_07, P0_08, P0_11, P0_12], output: [P0_13, P0_14, P0_15]);

    // Use internal flash to emulate eeprom
    let flash = async_flash_wrapper(Nvmc::new(p.NVMC));

    // RMK config
    let rmk_config = RmkConfig {
        vial_config: VialConfig::new(VIAL_KEYBOARD_ID, VIAL_KEYBOARD_DEF),
        ..Default::default()
    };

    // Initialize the storage and keymap
    let mut default_keymap = keymap::get_default_keymap();
    let (keymap, storage) = initialize_keymap_and_storage(
        &mut default_keymap,
        flash,
        rmk_config.storage_config,
        rmk_config.behavior_config.clone(),
    )
    .await;

    // Initialize the matrix + keyboard
    let debouncer = DefaultDebouncer::<ROW, COL>::new();
    let mut matrix = Matrix::<_, _, _, ROW, COL>::new(input_pins, output_pins, debouncer);
    let mut keyboard = Keyboard::new(&keymap, rmk_config.behavior_config.clone());

    // Initialize the light controller
    let light_controller: LightController<Output> =
        LightController::new(ControllerConfig::default().light_config);

    // Start
    join3(
        run_devices! (
            (matrix) => EVENT_CHANNEL,
        ),
        keyboard.run(),
        run_rmk(&keymap, driver, storage, light_controller, rmk_config),
    )
    .await;
}
