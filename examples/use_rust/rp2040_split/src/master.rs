#![no_main]
#![no_std]

#[macro_use]
mod keymap;
#[macro_use]
mod macros;
mod vial;

use crate::keymap::{COL, NUM_LAYER, ROW};
use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_rp::{
    bind_interrupts,
    flash::{Async, Flash},
    gpio::{AnyPin, Input, Output},
    peripherals::{self, UART0, USB},
    uart::{self, BufferedUart},
    usb::{Driver, InterruptHandler},
};
// use embassy_rp::flash::Blocking;
use panic_probe as _;
use rmk::{
    config::{KeyboardUsbConfig, RmkConfig, VialConfig},
    split::{
        master::{initialize_split_master_and_run, run_slave_receiver},
        SPLIT_MESSAGE_MAX_SIZE,
    },
};
use static_cell::StaticCell;
use vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};

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
    // let (input_pins, output_pins) = config_matrix_pins_rp!(peripherals: p, input: [PIN_6, PIN_7, PIN_8, PIN_9], output: [PIN_19, PIN_20, PIN_21]);
    let (input_pins, output_pins) =
        config_matrix_pins_rp!(peripherals: p, input: [PIN_6, PIN_7], output: [PIN_19, PIN_20]);

    // Use internal flash to emulate eeprom
    // Both blocking and async flash are support, use different API
    // let flash = Flash::<_, Blocking, FLASH_SIZE>::new_blocking(p.FLASH);
    let flash = Flash::<_, Async, FLASH_SIZE>::new(p.FLASH, p.DMA_CH0);

    let keyboard_usb_config = KeyboardUsbConfig {
        vid: 0x4c4b,
        pid: 0x4643,
        manufacturer: "Haobo",
        product_name: "RMK Keyboard",
        serial_number: "00000000",
    };

    let vial_config = VialConfig::new(VIAL_KEYBOARD_ID, VIAL_KEYBOARD_DEF);

    let keyboard_config = RmkConfig {
        usb_config: keyboard_usb_config,
        vial_config,
        ..Default::default()
    };

    // let (uart, tx_pin, tx_dma, rx_pin, rx_dma) = (p.UART0, p.PIN_16, p.DMA_CH0, p.PIN_17, p.DMA_CH1);
    // let uart = Uart::new(uart, tx_pin, rx_pin, Irqs, tx_dma, rx_dma, config);
    static TX_BUF: StaticCell<[u8; SPLIT_MESSAGE_MAX_SIZE]> = StaticCell::new();
    let tx_buf = &mut TX_BUF.init([0; SPLIT_MESSAGE_MAX_SIZE])[..];
    static RX_BUF: StaticCell<[u8; SPLIT_MESSAGE_MAX_SIZE]> = StaticCell::new();
    let rx_buf = &mut RX_BUF.init([0; SPLIT_MESSAGE_MAX_SIZE])[..];
    let uart_receiver = BufferedUart::new(
        p.UART0,
        Irqs,
        p.PIN_0,
        p.PIN_1,
        tx_buf,
        rx_buf,
        uart::Config::default(),
    );

    // Start serving
    join(
        initialize_split_master_and_run::<
            Flash<peripherals::FLASH, Async, FLASH_SIZE>,
            Driver<'_, USB>,
            Input<'_>,
            Output<'_>,
            ROW,
            COL,
            2,
            2,
            0,
            0,
            NUM_LAYER,
        >(
            driver,
            input_pins,
            output_pins,
            Some(flash),
            crate::keymap::KEYMAP,
            keyboard_config,
        ),
        run_slave_receiver::<2, 1, 2, 2, _>(uart_receiver, 0),
    )
    .await;
}
