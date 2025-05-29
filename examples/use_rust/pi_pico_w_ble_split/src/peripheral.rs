#![no_std]
#![no_main]

#[macro_use]
mod macros;

use bt_hci::controller::ExternalController;
use cyw43_pio::PioSpi;
use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::clocks::RoscRng;
use embassy_rp::flash::{Async, Flash};
use embassy_rp::gpio::{Input, Level, Output};
use embassy_rp::peripherals::{DMA_CH0, PIO0};
use embassy_rp::pio::{self, Pio};
use rand::SeedableRng;
use rmk::ble::trouble::build_ble_stack;
use rmk::channel::EVENT_CHANNEL;
use rmk::config::StorageConfig;
use rmk::debounce::default_debouncer::DefaultDebouncer;
use rmk::futures::future::join;
use rmk::matrix::Matrix;
use rmk::split::peripheral::run_rmk_split_peripheral;
use rmk::storage::new_storage_for_split_peripheral;
use rmk::{HostResources, run_devices};
use static_cell::StaticCell;
use {defmt_rtt as _, embassy_time as _, panic_probe as _};

bind_interrupts!(struct Irqs {
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

    // Storage config
    let flash = Flash::<_, Async, FLASH_SIZE>::new(p.FLASH, p.DMA_CH1);
    let storage_config = StorageConfig {
        start_addr: 0x100000, // Start from 1M
        num_sectors: 32,
        ..Default::default()
    };
    let mut storage = new_storage_for_split_peripheral(flash, storage_config).await;

    // Pin config
    let (input_pins, output_pins) =
        config_matrix_pins_rp!(peripherals: p, input: [PIN_6, PIN_7], output: [PIN_19, PIN_20]);
    let debouncer = DefaultDebouncer::<2, 2>::new();
    let mut matrix = Matrix::<_, _, _, 2, 2>::new(input_pins, output_pins, debouncer);

    let ble_addr = [0x7e, 0xfe, 0x73, 0x9e, 0x66, 0xe3];

    let mut host_resources = HostResources::new();
    let mut rosc_rng = RoscRng {};
    let mut rng = rand_chacha::ChaCha12Rng::from_rng(&mut rosc_rng).unwrap();

    let stack = build_ble_stack(controller, ble_addr, &mut rng, &mut host_resources).await;
    // Start
    join(
        run_devices! (
            (matrix) => EVENT_CHANNEL, // Peripheral uses EVENT_CHANNEL to send events to central
        ),
        run_rmk_split_peripheral(0, &stack, &mut storage),
    )
    .await;
}
