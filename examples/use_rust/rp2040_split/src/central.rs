#![no_main]
#![no_std]

#[macro_use]
mod keymap;
#[macro_use]
mod macros;
mod vial;

use defmt::info;
use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::flash::{Async, Flash};
use embassy_rp::gpio::{Input, Output};
use embassy_rp::peripherals::{UART0, USB};
use embassy_rp::uart::{self, BufferedUart};
use embassy_rp::usb::{Driver, InterruptHandler};
use rmk::channel::EVENT_CHANNEL;
use rmk::config::{BehaviorConfig, ControllerConfig, KeyboardUsbConfig, RmkConfig, StorageConfig, VialConfig};
use rmk::debounce::default_debouncer::DefaultDebouncer;
use rmk::futures::future::join4;
use rmk::input_device::Runnable;
use rmk::keyboard::Keyboard;
use rmk::light::LightController;
use rmk::split::SPLIT_MESSAGE_MAX_SIZE;
use rmk::split::central::{CentralMatrix, run_peripheral_manager};
use rmk::{initialize_keymap_and_storage, run_devices, run_rmk};
use static_cell::StaticCell;
use vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
    UART0_IRQ => uart::BufferedInterruptHandler<UART0>;
});

const FLASH_SIZE: usize = 2 * 1024 * 1024;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("RMK start!");
    // Initialize peripherals
    let p = embassy_rp::init(Default::default());

    // Create the usb driver, from the HAL
    let driver = Driver::new(p.USB, Irqs);

    // Pin config
    let (input_pins, output_pins) =
        config_matrix_pins_rp!(peripherals: p, input: [PIN_9, PIN_11], output: [PIN_10, PIN_12]);

    // Use internal flash to emulate eeprom
    // Both blocking and async flash are support, use different API
    // let flash = Flash::<_, Blocking, FLASH_SIZE>::new_blocking(p.FLASH);
    let flash = Flash::<_, Async, FLASH_SIZE>::new(p.FLASH, p.DMA_CH0);

    let keyboard_usb_config = KeyboardUsbConfig {
        vid: 0x4c4b,
        pid: 0x4643,
        manufacturer: "Haobo",
        product_name: "RMK Keyboard",
        serial_number: "vial:f64c2b3c:000001",
    };

    let vial_config = VialConfig::new(VIAL_KEYBOARD_ID, VIAL_KEYBOARD_DEF);

    let rmk_config = RmkConfig {
        usb_config: keyboard_usb_config,
        vial_config,
        ..Default::default()
    };

    static TX_BUF: StaticCell<[u8; SPLIT_MESSAGE_MAX_SIZE]> = StaticCell::new();
    let tx_buf = &mut TX_BUF.init([0; SPLIT_MESSAGE_MAX_SIZE])[..];
    static RX_BUF: StaticCell<[u8; SPLIT_MESSAGE_MAX_SIZE]> = StaticCell::new();
    let rx_buf = &mut RX_BUF.init([0; SPLIT_MESSAGE_MAX_SIZE])[..];
    let uart_receiver = BufferedUart::new(p.UART0, Irqs, p.PIN_0, p.PIN_1, tx_buf, rx_buf, uart::Config::default());

    // Initialize the storage and keymap
    let mut default_keymap = keymap::get_default_keymap();
    let behavior_config = BehaviorConfig::default();
    let storage_config = StorageConfig::default();
    let (keymap, mut storage) =
        initialize_keymap_and_storage(&mut default_keymap, flash, &storage_config, behavior_config).await;

    // Initialize the matrix + keyboard
    let debouncer = DefaultDebouncer::<2, 2>::new();
    let mut matrix = CentralMatrix::<_, _, _, 0, 0, 2, 2>::new(input_pins, output_pins, debouncer);
    let mut keyboard = Keyboard::new(&keymap);

    // Initialize the light controller
    let mut light_controller: LightController<Output> = LightController::new(ControllerConfig::default().light_config);

    // Start
    join4(
        run_devices! (
            (matrix) => EVENT_CHANNEL,
        ),
        keyboard.run(),
        run_peripheral_manager::<2, 1, 2, 2, _>(0, uart_receiver),
        run_rmk(&keymap, driver, &mut storage, &mut light_controller, rmk_config),
    )
    .await;
}
