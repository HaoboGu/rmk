#![no_std]
#![no_main]

#[macro_use]
mod macros;

use bt_hci::controller::ExternalController;
use cyw43::aligned_bytes;
use cyw43_pio::PioSpi;
use defmt::info;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Input, Level, Output};
use embassy_rp::peripherals::{DMA_CH0, DMA_CH2, PIO0};
use embassy_rp::pio::{self, Pio};
use embassy_time as _;
use panic_probe as _;
use rmk::ble::build_ble_stack;
use rmk::config::{BehaviorConfig, PositionalConfig, StorageConfig};
use rmk::debounce::default_debouncer::DefaultDebouncer;
use rmk::dfu::{DfuLock, DfuLockKeyUpdater};
use rmk::matrix::Matrix;
use rmk::processor::builtin::dfu_led::DfuLedProcessor;
use rmk::split::peripheral::run_rmk_split_peripheral;
use rmk::storage::{async_flash_wrapper, new_storage_for_split_peripheral};
use rmk::types::action::KeyAction;
use rmk::watchdog::Rp2040Watchdog;
use rmk::{HostResources, KeymapData, initialize_keymap, run_all};
use static_cell::StaticCell;

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => pio::InterruptHandler<PIO0>;
    DMA_IRQ_0 => embassy_rp::dma::InterruptHandler<DMA_CH0>, embassy_rp::dma::InterruptHandler<DMA_CH2>;
});

#[embassy_executor::task]
async fn cyw43_task(
    runner: cyw43::Runner<'static, cyw43::SpiBus<Output<'static>, PioSpi<'static, PIO0, 0>>, cyw43::Cyw43439>,
) -> ! {
    runner.run().await
}

const FLASH_SIZE: u32 = 2 * 1024 * 1024;
const PAGE_SIZE: u32 = 4 * 1024;
const STORAGE_SIZE: u32 = 128 * 1024;
const STATE_OFFSET: u32 = 0x6000;
const STATE_SIZE: u32 = 0x1000;
const ACTIVE_OFFSET: u32 = 0x7000;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("RMK peripheral start!");
    let p = embassy_rp::init(Default::default());

    let remaining = FLASH_SIZE - 28 * 1024 - STORAGE_SIZE;
    let active_size = (remaining - PAGE_SIZE) / 2;
    let dfu_size = active_size + PAGE_SIZE;
    let dfu_offset = ACTIVE_OFFSET + active_size;
    let storage_offset = dfu_offset + dfu_size;

    let flash = async_flash_wrapper(rmk::dfu::init_flash(
        p.FLASH,
        storage_offset,
        STORAGE_SIZE,
        STATE_OFFSET,
        STATE_SIZE,
        dfu_offset,
        dfu_size,
    ));

    rmk::dfu::mark_booted();

    #[cfg(feature = "skip-cyw43-firmware")]
    let (fw, clm, btfw, nvram) = {
        static EMPTY: &cyw43::Aligned<cyw43::A4, [u8]> = &cyw43::Aligned([0u8; 0]);
        (EMPTY, &[] as &[u8], EMPTY, EMPTY)
    };

    #[cfg(not(feature = "skip-cyw43-firmware"))]
    let (fw, clm, btfw, nvram) = {
        let fw = aligned_bytes!("../cyw43-firmware/43439A0.bin");
        let clm = aligned_bytes!("../cyw43-firmware/43439A0_clm.bin");
        let btfw = aligned_bytes!("../cyw43-firmware/43439A0_btfw.bin");
        let nvram = aligned_bytes!("../cyw43-firmware/nvram_rp2040.bin");
        (fw, clm, btfw, nvram)
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
        embassy_rp::dma::Channel::new(p.DMA_CH0, Irqs),
        embassy_rp::dma::Channel::new(p.DMA_CH2, Irqs),
    );

    static STATE: StaticCell<cyw43::State> = StaticCell::new();
    let state = STATE.init(cyw43::State::new());
    let (_net_device, bt_device, mut control, runner) =
        cyw43::new_with_bluetooth(state, pwr, spi, fw, btfw, nvram).await;
    spawner.spawn(cyw43_task(runner).unwrap());
    control.init(clm).await;
    let controller: ExternalController<_, 10> = ExternalController::new(bt_device);

    let storage_config = StorageConfig {
        start_addr: 0,
        num_sectors: 32,
        ..Default::default()
    };
    let mut storage = new_storage_for_split_peripheral(flash, storage_config).await;

    let (row_pins, col_pins) = config_matrix_pins_rp!(peripherals: p, input: [PIN_6, PIN_7], output: [PIN_19, PIN_20]);
    let debouncer = DefaultDebouncer::new();
    let mut matrix = Matrix::<_, _, _, 2, 2, true>::new(row_pins, col_pins, debouncer);

    let ble_addr = [0x7e, 0xfe, 0x73, 0x9e, 0x66, 0xe3];

    let mut host_resources = HostResources::new();
    let stack = build_ble_stack(controller, ble_addr, &mut host_resources).await;
    let mut watchdog_runner = Rp2040Watchdog::default_runner(embassy_rp::watchdog::Watchdog::new(p.WATCHDOG));

    let core_task = run_all!(matrix, storage, watchdog_runner);
    let ble_task = run_rmk_split_peripheral(0, &stack, "per0");

    let mut keymap_data = KeymapData::new([[[KeyAction::No; 1]; 1]; 1]);
    let mut behavior_config = BehaviorConfig::default();
    let per_key_config = PositionalConfig::default();
    let keymap = initialize_keymap(&mut keymap_data, &mut behavior_config, &per_key_config).await;
    let unlock_keys: &[(u8, u8)] = &[(0, 0)];
    let mut dfu_lock = DfuLock::new(unlock_keys, &keymap);
    let mut dfu_lock_key_updater = DfuLockKeyUpdater { keymap: &keymap };
    let mut dfu_led_processor = DfuLedProcessor::new(Output::new(p.PIN_16, Level::Low), false);

    rmk::embassy_futures::join::join3(
        core_task,
        run_all!(dfu_lock, dfu_lock_key_updater, dfu_led_processor),
        ble_task,
    )
    .await;
}
