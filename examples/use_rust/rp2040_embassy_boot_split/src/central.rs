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
use embassy_rp::peripherals::{UART0, USB};
use embassy_rp::uart::{self, BufferedUart};
use embassy_rp::usb::{Driver, InterruptHandler};
use embassy_rp::{bind_interrupts, dma};
use panic_probe as _;
use rmk::config::{BehaviorConfig, DeviceConfig, PositionalConfig, RmkConfig, StorageConfig, VialConfig};
use rmk::debounce::default_debouncer::DefaultDebouncer;
use rmk::futures::future::join;
use rmk::host::HostService;
use rmk::keyboard::Keyboard;
use rmk::matrix::Matrix;
use rmk::processor::builtin::wpm::WpmProcessor;
use rmk::split::SPLIT_MESSAGE_MAX_SIZE;
use rmk::split::central::run_peripheral_manager;
use rmk::storage::async_flash_wrapper;
use rmk::usb::UsbTransport;
use rmk::watchdog::Rp2040Watchdog;
use rmk::{KeymapData, initialize_keymap_and_storage, run_all};
use static_cell::StaticCell;
use vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
    UART0_IRQ => uart::BufferedInterruptHandler<UART0>;
    DMA_IRQ_0 => dma::InterruptHandler<embassy_rp::peripherals::DMA_CH0>;
});

const PERIPHERAL1_BIN: &[u8] = include_bytes!("../rmk-rp2040-embassy-boot-split-peripheral.bin");

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("RMK start!");
    let p = embassy_rp::init(Default::default());

    let driver = Driver::new(p.USB, Irqs);

    let (row_pins, col_pins) =
        config_matrix_pins_rp!(peripherals: p, input: [PIN_6, PIN_7], output: [PIN_19, PIN_20]);

    // Flash layout using the bootymcbootface formula:
    //   state at 0x6000 (4K), active from 0x7000 (size: (flash_size - 28K (= BOOT2 size + embassy-boot + embassy-boot state) - STORAGE_SIZE (= 128K) - page_size (= 4K)) / 2),
    //   dfu follows active (active_size + page_size (= 4K))
    //
    // All offsets (DFU_OFFSET, DFU_SIZE, STORAGE_OFFSET, etc.) are derived
    // automatically from FLASH_SIZE below --- change only that constant when using
    // bootymcbootface.
    //
    // ⚠  You can define your own FLASH_SIZE and addresses, but then you must build and
    //    flash a custom embassy-boot bootloader with a matching memory.x!
    const FLASH_SIZE: u32 = 2 * 1024 * 1024; // 2 MB (default)
    // const FLASH_SIZE: u32 = 4 * 1024 * 1024;    // 4 MB
    // const FLASH_SIZE: u32 = 8 * 1024 * 1024;    // 8 MB
    // const FLASH_SIZE: u32 = 16 * 1024 * 1024;   // 16 MB
    const PAGE_SIZE: u32 = 4 * 1024;
    const STORAGE_SIZE: u32 = 128 * 1024; // 32 sectors × 4K after ACTIVE+DFU
    const STATE_OFFSET: u32 = 0x6000;
    const STATE_SIZE: u32 = 0x1000;
    const ACTIVE_OFFSET: u32 = 0x7000; // after 28K bootloader + state
    let remaining: u32 = FLASH_SIZE
        - 28 * 1024 // size of boot 2 + embassy-boot + embassy-boot state
        - STORAGE_SIZE;
    let active_size: u32 = (remaining - PAGE_SIZE) / 2; // DFU = ACTIVE + 1 page (embassy-boot requirement)
    let dfu_size: u32 = active_size + PAGE_SIZE; // embassy-boot needs that extra page for swap info
    let dfu_offset: u32 = ACTIVE_OFFSET + active_size; // dfu after active
    let storage_offset: u32 = dfu_offset + dfu_size; // storage after active + dfu
    assert!(storage_offset + STORAGE_SIZE == FLASH_SIZE); // sanity check that we fit everything in flash

    info!(
        "Flash layout: state @ 0x{:04X} ({}K), active @ 0x{:04X} ({}K), dfu @ 0x{:04X} ({}K), storage @ 0x{:04X} ({}K)",
        STATE_OFFSET,
        STATE_SIZE / 1024,
        ACTIVE_OFFSET,
        active_size / 1024,
        dfu_offset,
        dfu_size / 1024,
        storage_offset,
        STORAGE_SIZE / 1024
    );

    let flash = async_flash_wrapper(rmk::dfu::init_flash(
        p.FLASH,
        storage_offset,
        STORAGE_SIZE,
        STATE_OFFSET,
        STATE_SIZE,
        dfu_offset,
        dfu_size,
    ));

    rmk::dfu::set_led(Some(Output::new(p.PIN_25, Level::Low)));

    let keyboard_device_config = DeviceConfig {
        vid: 0x4c4b,
        pid: 0x4643,
        manufacturer: "Haobo",
        product_name: "RMK Keyboard RP2040 embassy-boot split use_rust example",
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

    // Register peripheral firmware for DFU update over split
    if rmk::dfu::set_firmware_update_data(0, PERIPHERAL1_BIN, rmk::crc32::crc32(PERIPHERAL1_BIN)).is_ok() {
        info!("registered peripheral firmware");
    }

    let debouncer = DefaultDebouncer::new();
    let mut matrix = Matrix::<_, _, _, 2, 2, true>::new(row_pins, col_pins, debouncer);
    let mut keyboard = Keyboard::new(&keymap);
    let host_ctx = rmk::host::KeyboardContext::new(&keymap);
    let mut host_service = HostService::new(&host_ctx, &rmk_config);

    let mut usb_transport = UsbTransport::new(driver, rmk_config.device_config);
    let mut wpm_processor = WpmProcessor::new();

    let mut watchdog_runner = Rp2040Watchdog::default_runner(embassy_rp::watchdog::Watchdog::new(p.WATCHDOG));

    // UART for split peripheral communication
    static TX_BUF: StaticCell<[u8; SPLIT_MESSAGE_MAX_SIZE]> = StaticCell::new();
    let tx_buf = &mut TX_BUF.init([0; SPLIT_MESSAGE_MAX_SIZE])[..];
    static RX_BUF: StaticCell<[u8; SPLIT_MESSAGE_MAX_SIZE]> = StaticCell::new();
    let rx_buf = &mut RX_BUF.init([0; SPLIT_MESSAGE_MAX_SIZE])[..];
    let uart_receiver = BufferedUart::new(p.UART0, p.PIN_0, p.PIN_1, Irqs, tx_buf, rx_buf, uart::Config::default());

    join(
        run_all!(
            matrix,
            storage,
            usb_transport,
            wpm_processor,
            keyboard,
            host_service,
            watchdog_runner
        ),
        run_peripheral_manager::<2, 1, 2, 2, _>(0, uart_receiver),
    )
    .await;
}
