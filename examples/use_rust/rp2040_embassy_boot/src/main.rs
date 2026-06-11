#![no_main]
#![no_std]

#[macro_use]
mod keymap;
#[macro_use]
mod macros;
mod vial;

use defmt::info;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_rp::gpio::{Input, Level, Output};
use embassy_rp::peripherals::USB;
use embassy_rp::usb::{Driver, InterruptHandler};
use embassy_rp::{bind_interrupts, dma};
use keymap::{COL, ROW};
use panic_probe as _;
use rmk::config::{BehaviorConfig, DeviceConfig, PositionalConfig, RmkConfig, StorageConfig, VialConfig};
use rmk::debounce::default_debouncer::DefaultDebouncer;
use rmk::host::HostService;
use rmk::keyboard::Keyboard;
use rmk::matrix::Matrix;
use rmk::processor::builtin::wpm::WpmProcessor;
use rmk::storage::async_flash_wrapper;
use rmk::usb::UsbTransport;
use rmk::watchdog::Rp2040Watchdog;
use rmk::{KeymapData, initialize_keymap_and_storage, run_all};
use vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
    DMA_IRQ_0 => dma::InterruptHandler<embassy_rp::peripherals::DMA_CH0>;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("RMK start!");
    let p = embassy_rp::init(Default::default());

    let driver = Driver::new(p.USB, Irqs);

    let (row_pins, col_pins) =
        config_matrix_pins_rp!(peripherals: p, input: [PIN_6, PIN_7, PIN_8, PIN_9], output: [PIN_19, PIN_20, PIN_21]);

    // Flash layout using the bootymcbootface formula:
    //   state at 0x6000 (4K), active from 0x7000 (flash/4),
    //   dfu follows active (flash/4 + page_size)
    //
    // All offsets (DFU_OFFSET, DFU_SIZE, STORAGE_END, etc.) are derived
    // automatically from FLASH_SIZE below — change only that constant.
    //
    // ⚠  You can define your own FLASH_SIZE, but then you must build and
    //    flash a custom bootymcbootface with a matching memory.x!
    const FLASH_SIZE: u32 = 2 * 1024 * 1024;       // 2 MB (default)
    // const FLASH_SIZE: u32 = 4 * 1024 * 1024;    // 4 MB
    // const FLASH_SIZE: u32 = 8 * 1024 * 1024;    // 8 MB
    // const FLASH_SIZE: u32 = 16 * 1024 * 1024;   // 16 MB
    const PAGE_SIZE: u32 = 4096;
    const HALF_FLASH: u32 = FLASH_SIZE / 4;
    const STATE_OFFSET: u32 = 0x6000;
    const STATE_SIZE: u32 = 0x1000;
    const DFU_OFFSET: u32 = 0x7000 + HALF_FLASH;
    const DFU_SIZE: u32 = HALF_FLASH + PAGE_SIZE;

    const STORAGE_END: u32 = FLASH_SIZE - 32 * PAGE_SIZE;

    let flash = async_flash_wrapper(rmk::dfu::init_flash(
        p.FLASH,
        0,               // storage_start (relative, dfu_rp partitions the flash)
        STORAGE_END,
        STATE_OFFSET,
        STATE_SIZE,
        DFU_OFFSET,
        DFU_SIZE,
    ));

    rmk::dfu::set_led(Some(Output::new(p.PIN_25, Level::Low)));

    let keyboard_device_config = DeviceConfig {
        vid: 0x4c4b,
        pid: 0x4643,
        manufacturer: "Haobo",
        product_name: "RMK Keyboard RP2040 embassy-boot use_rust example",
        serial_number: "vial:f64c2b3c:000001",
    };

    let vial_config = VialConfig::new(VIAL_KEYBOARD_ID, VIAL_KEYBOARD_DEF, &[(0, 0), (1, 1)]);

    let rmk_config = RmkConfig {
        device_config: keyboard_device_config,
        vial_config,
        ..Default::default()
    };

    let mut keymap_data = KeymapData::new(keymap::get_default_keymap());
    let storage_config = StorageConfig {
        num_sectors: 32,
        start_addr: 0,
        clear_storage: false,
        clear_layout: false,
    };
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

    rmk::dfu::mark_booted();

    // Optional DFU lock — requires the `dfu_lock` Cargo feature.
    // Specify the physical keys to press simultaneously to unlock DFU firmware
    // download. The keys are (row, col) pairs matching your matrix layout.
    // The lock state is checked by the DFU USB handler on each download start.
    // To use, create a `DfuLock` and poll it periodically:
    //
    // let unlock_keys: &[(u8, u8)] = &[(0, 0), (1, 1)];
    // let mut dfu_lock = ::rmk::dfu::DfuLock::new(unlock_keys, &keymap);
    // add dfu_lock to run_all!()

    let debouncer = DefaultDebouncer::new();
    let mut matrix = Matrix::<_, _, _, ROW, COL, true>::new(row_pins, col_pins, debouncer);
    let mut keyboard = Keyboard::new(&keymap);
    let host_ctx = rmk::host::KeyboardContext::new(&keymap);
    let mut host_service = HostService::new(&host_ctx, &rmk_config);

    let mut usb_transport = UsbTransport::new(driver, rmk_config.device_config);
    let mut wpm_processor = WpmProcessor::new();

    let mut watchdog_runner = Rp2040Watchdog::default_runner(embassy_rp::watchdog::Watchdog::new(p.WATCHDOG));


    run_all!(
        matrix,
        storage,
        usb_transport,
        wpm_processor,
        keyboard,
        host_service,
        watchdog_runner
        // dfu_lock
    )
    .await;
}
