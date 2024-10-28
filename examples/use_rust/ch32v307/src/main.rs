#![no_std]
#![no_main]

mod vial;

// use defmt::*;
// use defmt_rtt as _;
use ch32_hal::gpio::{Input, Level, Output, Pull, Speed};
use ch32_hal::otg_fs::endpoint::EndpointDataBuffer;
use ch32_hal::otg_fs::{self, Driver};
use ch32_hal::{self as hal, bind_interrupts, peripherals, println, Config};
use embassy_executor::Spawner;
use panic_halt as _;
use rmk::config::{KeyboardUsbConfig, RmkConfig, VialConfig};
use rmk::{k, run_rmk};
use static_cell::StaticCell;
use vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};
bind_interrupts!(struct Irq {
    OTG_FS => otg_fs::InterruptHandler<peripherals::OTG_FS>;
});

#[defmt::global_logger]
struct Logger;

unsafe impl defmt::Logger for Logger {
    fn acquire() {}
    unsafe fn flush() {}
    unsafe fn release() {}
    unsafe fn write(_bytes: &[u8]) {
        println!("{}", core::str::from_utf8(_bytes).unwrap());
    }
}
#[embassy_executor::main(entry = "qingke_rt::entry")]
async fn main(spawner: Spawner) -> ! {
    hal::debug::SDIPrint::enable();
    println!("RMK start");
    // setup clocks
    let cfg = Config {
        rcc: ch32_hal::rcc::Config::SYSCLK_FREQ_144MHZ_HSI,
        ..Default::default()
    };
    let p = hal::init(cfg);
    hal::embassy::init();

    /* USB DRIVER SECION */
    static BUFFER: StaticCell<[EndpointDataBuffer; 1]> = StaticCell::new();
    // let mut buffer = ;
    let driver = Driver::new(
        p.OTG_FS,
        p.PA12,
        p.PA11,
        BUFFER.init([EndpointDataBuffer::default()]),
    );

    let i = [Input::new(p.PB3, Pull::Up)];
    let o = [Output::new(p.PA15, Level::Low, Speed::default())];

    let mut default_keymap = [[[k!(A)]; 1]; 1];

    let keyboard_usb_config = KeyboardUsbConfig {
        vid: 0x4c4b,
        pid: 0x4643,
        manufacturer: "Haobo",
        product_name: "Ch32 RMK Keyboard",
        serial_number: "vial:f64c2b3c:000001",
    };
    let vial_config = VialConfig::new(VIAL_KEYBOARD_ID, VIAL_KEYBOARD_DEF);

    let keyboard_config = RmkConfig {
        usb_config: keyboard_usb_config,
        vial_config,
        ..Default::default()
    };
    run_rmk(
        i,
        o,
        driver,
        rmk::EmptyFlashWrapper::new(),
        &mut default_keymap,
        keyboard_config,
        spawner,
    )
    .await
}
