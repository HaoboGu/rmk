#![no_std]
#![no_main]

mod logger;
mod vial;

use core::mem::MaybeUninit;
use core::panic::PanicInfo;

use ch32_hal::gpio::{Input, Level, Output, Pull, Speed};
use ch32_hal::mode::Blocking;
use ch32_hal::usb::EndpointDataBuffer;
use ch32_hal::otg_fs::{self, Driver};
use ch32_hal::peripherals::USART1;
use ch32_hal::usart::UartTx;
use ch32_hal::{self as hal, bind_interrupts, peripherals, usart, Config};
use defmt::{info, println, Display2Format};
use embassy_executor::Spawner;
use embassy_time::Timer;
use logger::set_logger;
use rmk::config::{KeyboardUsbConfig, RmkConfig, VialConfig};
use rmk::{k, run_rmk};
use static_cell::StaticCell;
use vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};
bind_interrupts!(struct Irq {
    OTG_FS => otg_fs::InterruptHandler<peripherals::OTG_FS>;
});

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    critical_section::with(|_| {
        println!("{}", Display2Format(info));

        loop {}
    })
}

static mut LOGGER_UART: MaybeUninit<UartTx<'static, USART1, Blocking>> = MaybeUninit::uninit();

#[embassy_executor::main(entry = "qingke_rt::entry")]
async fn main(spawner: Spawner) -> ! {
    // setup clocks
    let cfg = Config {
        rcc: ch32_hal::rcc::Config::SYSCLK_FREQ_144MHZ_HSI,
        ..Default::default()
    };
    let p = hal::init(cfg);
    // Setup the printer
    let uart1_config = usart::Config::default();
    unsafe {
        LOGGER_UART = MaybeUninit::new(
            UartTx::<'static, _, _>::new_blocking(p.USART1, p.PA9, uart1_config).unwrap(),
        );
    };
    set_logger(&|data| unsafe {
        #[allow(unused_must_use, static_mut_refs)]
        LOGGER_UART.assume_init_mut().blocking_write(data).ok();
    });

    // wait for serial-cat
    Timer::after_millis(300).await;

    info!("test");
    /* USB DRIVER SECION */
    static BUFFER: StaticCell<[EndpointDataBuffer; 8]> = StaticCell::new();
    // let mut buffer = ;
    let driver = Driver::new(
        p.OTG_FS,
        p.PA12,
        p.PA11,
        BUFFER.init(core::array::from_fn(|_| EndpointDataBuffer::default())),
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
        // rmk::EmptyFlashWrapper::new(),
        &mut default_keymap,
        None,
        keyboard_config,
        spawner,
    )
    .await
}
