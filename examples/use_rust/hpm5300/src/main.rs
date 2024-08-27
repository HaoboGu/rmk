#![no_main]
#![no_std]
#![feature(type_alias_impl_trait)]
#![feature(abi_riscv_interrupt)]
#![feature(impl_trait_in_assoc_type)]

#[macro_use]
mod keymap;
#[macro_use]
mod macros;
mod dummy_flash;
mod vial;

use defmt_rtt as _;
use embassy_executor::Spawner;
use hpm_hal::gpio::{Input, Output};
use hpm_hal::usb::UsbDriver;
use hpm_hal::flash::Flash;
use hpm_hal::{bind_interrupts, peripherals};
use riscv_rt as _;
use rmk::{
    config::{KeyboardUsbConfig, RmkConfig, VialConfig},
    initialize_keyboard_and_run,
};
use vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};

use crate::keymap::{COL, NUM_LAYER, ROW};

bind_interrupts!(struct Irqs {
    USB0 => hpm_hal::usb::InterruptHandler<peripherals::USB0>;
});

const FLASH_SIZE: usize = 1 * 1024 * 1024;

#[embassy_executor::main(entry = "hpm_hal::entry")]
async fn main(_spawner: Spawner) {
    let mut p = hpm_hal::init(Default::default());

    let usb_driver = hpm_hal::usb::UsbDriver::new(p.USB0, p.PA24, p.PA25);

    let flash_config = hpm_hal::flash::Config::from_rom_data(&mut p.XPI0).unwrap();
    let flash: Flash<_, FLASH_SIZE> = Flash::new(p.XPI0, flash_config).unwrap();

    // Pin config
    let (input_pins, output_pins) = config_matrix_pins_hpm!(peripherals: p, input: [PA31, PA28, PA29, PA27], output: [PB10, PB11, PA09]);

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

    // Start serving
    initialize_keyboard_and_run::<
        Flash<_, FLASH_SIZE>,
        UsbDriver<'_, peripherals::USB0>,
        Input<'_>,
        Output<'_>,
        ROW,
        COL,
        NUM_LAYER,
    >(
        usb_driver,
        input_pins,
        output_pins,
        Some(flash),
        crate::keymap::KEYMAP,
        keyboard_config,
    )
    .await;
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    defmt::info!("panic: {:?}", defmt::Debug2Format(&info));
    loop {}
}
