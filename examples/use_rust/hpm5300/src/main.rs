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

use embassy_executor::Spawner;
use hpm_hal::flash::Flash;
use hpm_hal::{bind_interrupts, peripherals};
use rmk::config::{KeyboardConfig, KeyboardUsbConfig, RmkConfig, VialConfig};
use rmk::run_rmk;
use vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};
use {defmt_rtt as _, riscv_rt as _};

bind_interrupts!(struct Irqs {
    USB0 => hpm_hal::usb::InterruptHandler<peripherals::USB0>;
});

const FLASH_SIZE: usize = 1 * 1024 * 1024;

#[embassy_executor::main(entry = "hpm_hal::entry")]
async fn main(spawner: Spawner) {
    let mut p = hpm_hal::init(Default::default());

    let usb_driver = hpm_hal::usb::UsbDriver::new(p.USB0, p.PA24, p.PA25);

    let flash_config = hpm_hal::flash::Config::from_rom_data(&mut p.XPI0).unwrap();
    let flash: Flash<_, FLASH_SIZE> = Flash::new(p.XPI0, flash_config).unwrap();

    // Pin config
    let (input_pins, output_pins) =
        config_matrix_pins_hpm!(peripherals: p, input: [PA31, PA28, PA29, PA27], output: [PB10, PB11, PA09]);

    let keyboard_usb_config = KeyboardUsbConfig {
        vid: 0x4c4b,
        pid: 0x4643,
        manufacturer: "Haobo",
        product_name: "RMK Keyboard",
        serial_number: "vial:f64c2b3c:000001",
    };

    let vial_config = VialConfig::new(VIAL_KEYBOARD_ID, VIAL_KEYBOARD_DEF);

    let rmk_config = RmkConfig {
        usb_config: keyboard_usb_config,
        vial_config,
        ..Default::default()
    };

    let keyboard_config = KeyboardConfig {
        rmk_config,
        ..Default::default()
    };

    // Start
    run_rmk(
        input_pins,
        output_pins,
        usb_driver,
        flash,
        &mut keymap::get_default_keymap(),
        keyboard_config,
        spawner,
    )
    .await;
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    defmt::info!("panic: {:?}", defmt::Debug2Format(&info));
    loop {}
}
