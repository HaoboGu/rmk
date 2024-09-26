#![no_main]
#![no_std]

#[macro_use]
mod macros;

use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_rp::{
    bind_interrupts,
    gpio::{AnyPin, Input, Output},
    peripherals::{UART0, USB},
    uart::{self, BufferedUart},
    usb::InterruptHandler,
};
use panic_probe as _;
use rmk::split::{peripheral::run_rmk_split_peripheral, SPLIT_MESSAGE_MAX_SIZE};
use static_cell::StaticCell;

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
    UART0_IRQ => uart::BufferedInterruptHandler<UART0>;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("RMK start!");
    // Initialize peripherals
    let p = embassy_rp::init(Default::default());

    // Pin config
    let (input_pins, output_pins) =
        config_matrix_pins_rp!(peripherals: p, input: [PIN_9, PIN_11], output: [PIN_10, PIN_12]);

    static TX_BUF: StaticCell<[u8; SPLIT_MESSAGE_MAX_SIZE]> = StaticCell::new();
    let tx_buf = &mut TX_BUF.init([0; SPLIT_MESSAGE_MAX_SIZE])[..];
    static RX_BUF: StaticCell<[u8; SPLIT_MESSAGE_MAX_SIZE]> = StaticCell::new();
    let rx_buf = &mut RX_BUF.init([0; SPLIT_MESSAGE_MAX_SIZE])[..];
    let uart_instance = BufferedUart::new(
        p.UART0,
        Irqs,
        p.PIN_0,
        p.PIN_1,
        tx_buf,
        rx_buf,
        uart::Config::default(),
    );

    // Start serving
    run_rmk_split_peripheral::<Input<'_>, Output<'_>, _, 2, 2>(
        input_pins,
        output_pins,
        uart_instance,
    )
    .await;
}
