#![no_main]
#![no_std]

#[macro_use]
mod keymap;
#[macro_use]
mod macros;

use defmt::info;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_rp::flash::{Async, Flash};
use embassy_rp::gpio::{Input, Output};
use embassy_rp::peripherals::{DMA_CH0, USB};
use embassy_rp::usb::{Driver, InterruptHandler};
use embassy_rp::{bind_interrupts, dma};
use keymap::{COL, ROW};
use panic_probe as _;
use rmk::config::{BehaviorConfig, DeviceConfig, PositionalConfig, RmkConfig, StorageConfig};
use rmk::debounce::default_debouncer::DefaultDebouncer;
use rmk::host::HostService;
use rmk::keyboard::Keyboard;
use rmk::matrix::Matrix;
use rmk::processor::builtin::wpm::WpmProcessor;
use rmk::usb::UsbTransport;
use rmk::{KeymapData, initialize_keymap_and_storage, run_all};

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
    DMA_IRQ_0 => dma::InterruptHandler<DMA_CH0>;
});

const FLASH_SIZE: usize = 2 * 1024 * 1024;

/// Concrete USB driver type used by this example. Surfaces the same alias the
/// orchestrator macro emits in the `keyboard.toml`-driven path so the
/// `HostService<RmkUsbDriverTy>` turbofish at construction has one fixed name.
type RmkUsbDriverTy = Driver<'static, USB>;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("RMK start!");
    // Initialize peripherals
    let p = embassy_rp::init(Default::default());

    // Create the usb driver, from the HAL
    let driver: RmkUsbDriverTy = Driver::new(p.USB, Irqs);

    // Pin config
    let (row_pins, col_pins) =
        config_matrix_pins_rp!(peripherals: p, input: [PIN_6, PIN_7, PIN_8, PIN_9], output: [PIN_19, PIN_20, PIN_21]);

    // Use internal flash to emulate eeprom
    let flash = Flash::<_, Async, FLASH_SIZE>::new(p.FLASH, p.DMA_CH0, Irqs);

    let keyboard_device_config = DeviceConfig {
        vid: 0xc0de,
        pid: 0xcafe,
        manufacturer: "Haobo",
        product_name: "RMK Keyboard",
        serial_number: "rmk:rp2040:000001",
    };

    let rmk_config = RmkConfig {
        device_config: keyboard_device_config,
        ..Default::default()
    };

    // Initialize the storage and keymap
    let mut keymap_data = KeymapData::new(keymap::get_default_keymap());
    let storage_config = StorageConfig::default();
    let mut behavior_config = BehaviorConfig::default();
    let per_key_config = PositionalConfig::default();
    let (keymap, mut storage) = initialize_keymap_and_storage(
        &mut keymap_data,
        flash,
        &storage_config,
        &mut behavior_config,
        &per_key_config,
    )
    .await;

    // Initialize the matrix + keyboard
    let debouncer = DefaultDebouncer::new();
    let mut matrix = Matrix::<_, _, _, ROW, COL, true>::new(row_pins, col_pins, debouncer);
    let mut keyboard = Keyboard::new(&keymap);

    let mut usb_transport = UsbTransport::new(driver, rmk_config.device_config);
    let mut wpm_processor = WpmProcessor::new();

    // The rmk_protocol USB bulk endpoints have to be taken out of the
    // UsbTransport once it has been built; `HostService` drives the protocol
    // dispatch while `UsbTransport::run` drives the underlying USB device.
    let mut host_service = HostService::<RmkUsbDriverTy>::new(&keymap, usb_transport.take_rmk_protocol_endpoints());

    run_all!(matrix, storage, usb_transport, wpm_processor, keyboard, host_service).await;
}
