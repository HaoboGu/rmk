#![no_std]
#![no_main]

#[macro_use]
mod keymap;
#[macro_use]
mod macros;
mod vial;

use bt_hci::controller::ExternalController;
use cyw43_pio::PioSpi;
use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::clocks::RoscRng;
use embassy_rp::flash::{Async, Flash};
use embassy_rp::gpio::{Input, Level, Output};
use embassy_rp::peripherals::{DMA_CH0, PIO0, USB};
use embassy_rp::pio::{self, Pio};
use embassy_rp::usb::{self, Driver};
use keymap::{COL, ROW};
use rand::SeedableRng;
use rmk::ble::trouble::build_ble_stack;
use rmk::channel::EVENT_CHANNEL;
use rmk::config::{BehaviorConfig, ControllerConfig, KeyboardUsbConfig, RmkConfig, StorageConfig, VialConfig};
use rmk::debounce::default_debouncer::DefaultDebouncer;
use rmk::futures::future::join3;
use rmk::input_device::Runnable;
use rmk::keyboard::Keyboard;
use rmk::light::LightController;
use rmk::matrix::Matrix;
use rmk::{initialize_keymap_and_storage, run_devices, run_rmk, HostResources};
use static_cell::StaticCell;
use vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};
use {defmt_rtt as _, embassy_time as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => usb::InterruptHandler<USB>;
    PIO0_IRQ_0 => pio::InterruptHandler<PIO0>;
});

#[embassy_executor::task]
async fn cyw43_task(runner: cyw43::Runner<'static, Output<'static>, PioSpi<'static, PIO0, 0, DMA_CH0>>) -> ! {
    runner.run().await
}

const FLASH_SIZE: usize = 2 * 1024 * 1024;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    #[cfg(feature = "skip-cyw43-firmware")]
    let (fw, clm, btfw) = (&[], &[], &[]);

    #[cfg(not(feature = "skip-cyw43-firmware"))]
    let (fw, clm, btfw) = {
        // IMPORTANT
        //
        // Download and make sure these files from https://github.com/embassy-rs/embassy/tree/main/cyw43-firmware
        // are available in `./examples/rp-pico-w`. (should be automatic)
        //
        // IMPORTANT
        let fw = include_bytes!("../cyw43-firmware/43439A0.bin");
        let clm = include_bytes!("../cyw43-firmware/43439A0_clm.bin");
        let btfw = include_bytes!("../cyw43-firmware/43439A0_btfw.bin");
        (fw, clm, btfw)
    };

    let pwr = Output::new(p.PIN_23, Level::Low);
    let cs = Output::new(p.PIN_25, Level::High);
    let mut pio = Pio::new(p.PIO0, Irqs);
    let spi = PioSpi::new(
        &mut pio.common,
        pio.sm0,
        cyw43_pio::DEFAULT_CLOCK_DIVIDER,
        pio.irq0,
        cs,
        p.PIN_24,
        p.PIN_29,
        p.DMA_CH0,
    );

    static STATE: StaticCell<cyw43::State> = StaticCell::new();
    let state = STATE.init(cyw43::State::new());
    let (_net_device, bt_device, mut control, runner) = cyw43::new_with_bluetooth(state, pwr, spi, fw, btfw).await;
    defmt::unwrap!(spawner.spawn(cyw43_task(runner)));
    control.init(clm).await;

    let controller: ExternalController<_, 10> = ExternalController::new(bt_device);

    // Create the usb driver, from the HAL
    let driver = Driver::new(p.USB, Irqs);

    // Pin config
    let (input_pins, output_pins) =
        config_matrix_pins_rp!(peripherals: p, input: [PIN_6, PIN_7, PIN_8, PIN_9], output: [PIN_19, PIN_20, PIN_21]);

    // Use internal flash to emulate eeprom
    // Both blocking and async flash are support, use different API
    // let flash = Flash::<_, Blocking, FLASH_SIZE>::new_blocking(p.FLASH);
    let flash = Flash::<_, Async, FLASH_SIZE>::new(p.FLASH, p.DMA_CH1);

    let keyboard_usb_config = KeyboardUsbConfig {
        vid: 0x4c4c,
        pid: 0x464c,
        manufacturer: "Haobo",
        product_name: "RMK PicoW",
        serial_number: "vial:f64c2b3c:000001",
    };

    let vial_config = VialConfig::new(VIAL_KEYBOARD_ID, VIAL_KEYBOARD_DEF);

    let storage_config = StorageConfig {
        start_addr: 0x100000, // Start from 1M
        num_sectors: 32,
        ..Default::default()
    };

    let rmk_config = RmkConfig {
        usb_config: keyboard_usb_config,
        vial_config,
        storage_config,
        ..Default::default()
    };

    // Initialize the storage and keymap
    let mut default_keymap = keymap::get_default_keymap();
    let behavior_config = BehaviorConfig::default();
    let (keymap, mut storage) =
        initialize_keymap_and_storage(&mut default_keymap, flash, &storage_config, behavior_config).await;

    // Initialize the matrix + keyboard
    let debouncer = DefaultDebouncer::<ROW, COL>::new();
    let mut matrix = Matrix::<_, _, _, ROW, COL>::new(input_pins, output_pins, debouncer);
    let mut keyboard = Keyboard::new(&keymap);

    // Initialize the light controller
    let mut light_controller: LightController<Output> = LightController::new(ControllerConfig::default().light_config);

    let ble_addr = [0x18, 0xe2, 0x21, 0x88, 0xc0, 0xc7];

    let mut host_resources = HostResources::new();
    let mut rosc_rng = RoscRng {};
    let mut rng = rand_chacha::ChaCha12Rng::from_rng(&mut rosc_rng).unwrap();

    let stack = build_ble_stack(controller, ble_addr, &mut rng, &mut host_resources).await;
    // Start
    join3(
        run_devices! (
            (matrix) => EVENT_CHANNEL,
        ),
        keyboard.run(),
        run_rmk(&keymap, driver, &stack, &mut storage, &mut light_controller, rmk_config),
    )
    .await;
}
