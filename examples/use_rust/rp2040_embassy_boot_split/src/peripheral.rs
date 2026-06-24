#![no_main]
#![no_std]

#[macro_use]
mod macros;

use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Input, Level, Output};
use embassy_rp::peripherals::{UART0, USB};
use embassy_rp::uart::{self, BufferedUart};
use embassy_rp::usb::InterruptHandler;
use panic_probe as _;
use rmk::debounce::default_debouncer::DefaultDebouncer;
use rmk::futures::future::join;
use rmk::matrix::Matrix;
use rmk::run_all;
use rmk::split::SPLIT_MESSAGE_MAX_SIZE;
use rmk::split::peripheral::run_rmk_split_peripheral;
use rmk::watchdog::Rp2040Watchdog;
use static_cell::StaticCell;

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
    UART0_IRQ => uart::BufferedInterruptHandler<UART0>;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("RMK peripheral start!");
    let p = embassy_rp::init(Default::default());

    let (row_pins, col_pins) =
        config_matrix_pins_rp!(peripherals: p, input: [PIN_8, PIN_9], output: [PIN_10]);

    // Flash layout for embassy-boot on the peripheral,
    // matching the central's layout for consistency.
    const FLASH_SIZE: u32 = 2 * 1024 * 1024;
    const PAGE_SIZE: u32 = 4 * 1024;
    const STORAGE_SIZE: u32 = 128 * 1024;
    const STATE_OFFSET: u32 = 0x6000;
    const STATE_SIZE: u32 = 0x1000;
    const ACTIVE_OFFSET: u32 = 0x7000;
    let remaining: u32 = FLASH_SIZE - 28 * 1024 - STORAGE_SIZE;
    let active_size: u32 = (remaining - PAGE_SIZE) / 2;
    let dfu_size: u32 = active_size + PAGE_SIZE;
    let dfu_offset: u32 = ACTIVE_OFFSET + active_size;
    let storage_offset: u32 = dfu_offset + dfu_size;

    rmk::dfu::init_flash(
        p.FLASH,
        storage_offset,
        STORAGE_SIZE,
        STATE_OFFSET,
        STATE_SIZE,
        dfu_offset,
        dfu_size,
    );

    rmk::dfu::set_led(Some(Output::new(p.PIN_25, Level::Low)));

    rmk::dfu::mark_booted();

    // UART for split peripheral communication
    static TX_BUF: StaticCell<[u8; SPLIT_MESSAGE_MAX_SIZE]> = StaticCell::new();
    let tx_buf = &mut TX_BUF.init([0; SPLIT_MESSAGE_MAX_SIZE])[..];
    static RX_BUF: StaticCell<[u8; SPLIT_MESSAGE_MAX_SIZE]> = StaticCell::new();
    let rx_buf = &mut RX_BUF.init([0; SPLIT_MESSAGE_MAX_SIZE])[..];
    let uart_instance = BufferedUart::new(p.UART0, p.PIN_0, p.PIN_1, Irqs, tx_buf, rx_buf, uart::Config::default());

    let debouncer = DefaultDebouncer::new();
    let mut matrix = Matrix::<_, _, _, 2, 1, true>::new(row_pins, col_pins, debouncer);

    let mut watchdog_runner = Rp2040Watchdog::default_runner(embassy_rp::watchdog::Watchdog::new(p.WATCHDOG));

    join(
        run_all!(matrix, watchdog_runner),
        run_rmk_split_peripheral(uart_instance),
    )
    .await;
}
