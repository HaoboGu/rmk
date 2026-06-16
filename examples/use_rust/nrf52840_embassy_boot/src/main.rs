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
use embassy_nrf::gpio::{Input, Level, Output, OutputDrive};
use embassy_nrf::interrupt::InterruptExt;
use embassy_nrf::usb::{self, Driver};
use embassy_nrf::{bind_interrupts, peripherals};
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
use rmk::{KeymapData, initialize_keymap_and_storage, run_all};
use vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};

bind_interrupts!(struct Irqs {
    USBD => usb::InterruptHandler<peripherals::USBD>;
    CLOCK_POWER => usb::vbus_detect::InterruptHandler;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("RMK start!");
    let mut config = embassy_nrf::config::Config::default();
    config.gpiote_interrupt_priority = embassy_nrf::interrupt::Priority::P3;
    config.time_interrupt_priority = embassy_nrf::interrupt::Priority::P3;
    embassy_nrf::interrupt::USBD.set_priority(embassy_nrf::interrupt::Priority::P2);
    embassy_nrf::interrupt::CLOCK_POWER.set_priority(embassy_nrf::interrupt::Priority::P2);
    config.debug = embassy_nrf::config::Debug::NotConfigured;
    let p = embassy_nrf::init(config);
    embassy_nrf::pac::CLOCK.tasks_hfclkstart().write_value(1);
    while embassy_nrf::pac::CLOCK.events_hfclkstarted().read() != 1 {}

    let driver = Driver::new(p.USBD, Irqs, usb::vbus_detect::HardwareVbusDetect::new(Irqs));

    let (row_pins, col_pins) =
        config_matrix_pins_nrf!(peripherals: p, input: [P0_07, P0_22, P0_11, P0_12], output: [P0_13, P0_17, P0_20]);

    // Flash layout using the bootymcbootface formula:
    //   state at 0x6000 (4K), active from 0x7000 (size: (flash_size - 28K (= embassy-boot + embassy-boot state) - STORAGE_SIZE (= 64K) - page_size (= 4K)) / 2),
    //   dfu follows active (active_size + page_size (= 4K))
    //
    // All offsets (DFU_OFFSET, DFU_SIZE, STORAGE_OFFSET, etc.) are derived
    // automatically from FLASH_SIZE below — change only that constant when using
    // bootymcbootface.
    //
    // ⚠  You can define your own FLASH_SIZE and addresses, but then you must build and
    //    flash a custom embassy-boot bootloader with a matching memory.x!
    const FLASH_SIZE: u32 = 1024 * 1024; // 1 MB (nRF52840)
    const PAGE_SIZE: u32 = 4 * 1024;
    const STORAGE_SIZE: u32 = 128 * 1024; // 32 sectors × 4K after ACTIVE+DFU
    const STATE_OFFSET: u32 = 0x6000;
    const STATE_SIZE: u32 = 0x1000;
    const ACTIVE_OFFSET: u32 = 0x7000;
    let remaining: u32 = FLASH_SIZE
        - 28 * 1024 // bootloader (24K) + state (4K)
        - STORAGE_SIZE;
    let active_size: u32 = (remaining - PAGE_SIZE) / 2;
    let dfu_size: u32 = active_size + PAGE_SIZE;
    let dfu_offset: u32 = ACTIVE_OFFSET + active_size;
    let storage_offset: u32 = dfu_offset + dfu_size;
    let storage_size: u32 = STORAGE_SIZE;
    assert!(storage_offset + storage_size == FLASH_SIZE);

    info!(
        "Flash layout: state @ 0x{:04X} ({}K), active @ 0x{:04X} ({}K), dfu @ 0x{:04X} ({}K), storage @ 0x{:04X} ({}K)",
        STATE_OFFSET,
        STATE_SIZE / 1024,
        ACTIVE_OFFSET,
        active_size / 1024,
        dfu_offset,
        dfu_size / 1024,
        storage_offset,
        storage_size / 1024
    );

    let flash = async_flash_wrapper(rmk::dfu::init_flash(
        p.NVMC,
        storage_offset,
        storage_size,
        STATE_OFFSET,
        STATE_SIZE,
        dfu_offset,
        dfu_size,
    ));

    rmk::dfu::set_led(Some(Output::new(p.P0_15, Level::Low, OutputDrive::Standard)));

    let keyboard_device_config = DeviceConfig {
        vid: 0x4c4b,
        pid: 0x4643,
        manufacturer: "Haobo",
        product_name: "RMK Keyboard nRF52840 embassy-boot use_rust example",
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
        num_sectors: 16,
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

    run_all!(
        matrix,
        storage,
        usb_transport,
        wpm_processor,
        keyboard,
        host_service // , dfu_lock
    )
    .await;
}
