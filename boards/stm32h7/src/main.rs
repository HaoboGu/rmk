#![no_main]
#![no_std]
#![feature(type_alias_impl_trait)]
#![allow(dead_code)]

#[macro_use]
mod macros;
mod keymap;
#[macro_use]
pub mod rtt_logger;
mod flash;

use core::sync::atomic::AtomicBool;
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_stm32::{
    bind_interrupts,
    gpio::{AnyPin, Input, Output},
    peripherals::{self, USB_OTG_HS},
    time::Hertz,
    usb_otg::{Driver, InterruptHandler},
    Config,
};
use log::info;
use panic_rtt_target as _;
use rmk::{eeprom::EepromStorageConfig, initialize_keyboard_and_usb_device};
use static_cell::StaticCell;

use crate::flash::DummyFlash;

bind_interrupts!(struct Irqs {
    OTG_HS => InterruptHandler<peripherals::USB_OTG_HS>;
});

static SUSPENDED: AtomicBool = AtomicBool::new(false);

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    rtt_logger::init(log::LevelFilter::Info);
    info!("Rmk start!");
    let mut config = Config::default();
    {
        use embassy_stm32::rcc::*;
        config.rcc.hsi = Some(HSIPrescaler::DIV1);
        config.rcc.csi = true;
        config.rcc.hsi48 = Some(Hsi48Config {
            sync_from_usb: true,
        }); // needed for USB
        config.rcc.hse = Some(Hse {
            freq: Hertz(25_000_000),
            mode: HseMode::Oscillator,
        });
        config.rcc.pll1 = Some(Pll {
            source: PllSource::HSE,
            prediv: PllPreDiv::DIV5,
            mul: PllMul::MUL112,
            divp: Some(PllDiv::DIV2),
            divq: Some(PllDiv::DIV2),
            divr: Some(PllDiv::DIV2),
        });
        config.rcc.sys = Sysclk::PLL1_P; // 400 Mhz
        config.rcc.ahb_pre = AHBPrescaler::DIV2; // 200 Mhz
        config.rcc.apb1_pre = APBPrescaler::DIV2; // 100 Mhz
        config.rcc.apb2_pre = APBPrescaler::DIV2; // 100 Mhz
        config.rcc.apb3_pre = APBPrescaler::DIV2; // 100 Mhz
        config.rcc.apb4_pre = APBPrescaler::DIV2; // 100 Mhz
        config.rcc.voltage_scale = VoltageScale::Scale0;
    }

    let p = embassy_stm32::init(config);

    static EP_OUT_BUFFER: StaticCell<[u8; 1024]> = StaticCell::new();
    let mut config = embassy_stm32::usb_otg::Config::default();
    config.vbus_detection = false;
    let driver = Driver::new_fs(
        p.USB_OTG_HS,
        Irqs,
        p.PA12,
        p.PA11,
        &mut EP_OUT_BUFFER.init([0; 1024])[..],
        config,
    );

    const FLASH_SECTOR_15_ADDR: u32 = 15 * 8192;
    let storage_config = EepromStorageConfig {
        start_addr: FLASH_SECTOR_15_ADDR,
        storage_size: 8192,
        page_size: 16,
    };

    let (input_pins, output_pins) = config_matrix_pins_stm32!(peripherals: p, input: [PD9, PD8, PB13, PB12], output: [PE13, PE14, PE15]);

    let (mut keyboard, mut device) = initialize_keyboard_and_usb_device::<
        Driver<'_, USB_OTG_HS>,
        Input<'_, AnyPin>,
        Output<'_, AnyPin>,
        DummyFlash,
        0,
        4,
        3,
        2,
    >(
        driver,
        None,
        storage_config,
        None,
        input_pins,
        output_pins,
        crate::keymap::KEYMAP,
    );

    let usb_fut = device.device.run();
    let keyboard_fut = async {
        loop {
            let _ = keyboard.keyboard_task().await;
            keyboard.send_report(&mut device.keyboard_hid).await;
        }
        // TODO: sleep
    };

    join(usb_fut, keyboard_fut).await;
}
