#![no_main]
#![no_std]
#![feature(type_alias_impl_trait)]
#![allow(dead_code)]

#[macro_use]
mod macros;
mod keymap;
#[macro_use]
pub mod rtt_logger;

use core::{cell::RefCell, sync::atomic::AtomicBool};
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_stm32::{
    bind_interrupts,
    flash::{Blocking, Flash},
    gpio::{AnyPin, Input, Output},
    peripherals::USB_OTG_HS,
    time::Hertz,
    usb_otg::{Driver, InterruptHandler},
    Config,
};
use embassy_time::Timer;
use log::info;
use panic_rtt_target as _;
use rmk::{eeprom::EepromStorageConfig, initialize_keyboard_and_usb_device, keymap::KeyMap};
use static_cell::StaticCell;

use crate::keymap::{COL, NUM_LAYER, ROW};

bind_interrupts!(struct Irqs {
    OTG_HS => InterruptHandler<USB_OTG_HS>;
});

static SUSPENDED: AtomicBool = AtomicBool::new(false);
const FLASH_SECTOR_15_ADDR: u32 = 15 * 8192;
const EEPROM_SIZE: usize = 128;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    if cfg!(debug_assertions) {
        rtt_logger::init(log::LevelFilter::Info);
    }
    info!("Rmk start!");
    // RCC config
    let mut config = Config::default();
    {
        use embassy_stm32::rcc::*;
        config.rcc.hsi = Some(HSIPrescaler::DIV1);
        config.rcc.csi = true;
        // Needed for USB
        config.rcc.hsi48 = Some(Hsi48Config {
            sync_from_usb: true,
        });
        // External oscillator 25MHZ
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
        config.rcc.sys = Sysclk::PLL1_P;
        config.rcc.ahb_pre = AHBPrescaler::DIV2;
        config.rcc.apb1_pre = APBPrescaler::DIV2;
        config.rcc.apb2_pre = APBPrescaler::DIV2;
        config.rcc.apb3_pre = APBPrescaler::DIV2;
        config.rcc.apb4_pre = APBPrescaler::DIV2;
        config.rcc.voltage_scale = VoltageScale::Scale0;
    }

    // Initialize peripherals
    let p = embassy_stm32::init(config);

    // Usb config
    static EP_OUT_BUFFER: StaticCell<[u8; 1024]> = StaticCell::new();
    let mut usb_config = embassy_stm32::usb_otg::Config::default();
    usb_config.vbus_detection = false;
    let driver = Driver::new_fs(
        p.USB_OTG_HS,
        Irqs,
        p.PA12,
        p.PA11,
        &mut EP_OUT_BUFFER.init([0; 1024])[..],
        usb_config,
    );

    // Pin config
    let (input_pins, output_pins) = config_matrix_pins_stm32!(peripherals: p, input: [PD9, PD8, PB13, PB12], output: [PE13, PE14, PE15]);

    // Keymap + eeprom config
    static MY_KEYMAP: StaticCell<
        RefCell<KeyMap<Flash<'_, Blocking>, EEPROM_SIZE, ROW, COL, NUM_LAYER>>,
    > = StaticCell::new();
    let eeprom_storage_config = EepromStorageConfig {
        start_addr: FLASH_SECTOR_15_ADDR,
        storage_size: 8192, // uses 8KB for eeprom
        page_size: 32,
    };
    // Use internal flash to emulate eeprom
    let f = Flash::new_blocking(p.FLASH);
    let keymap = MY_KEYMAP.init(RefCell::new(KeyMap::new(
        crate::keymap::KEYMAP,
        Some(f),
        eeprom_storage_config,
        None,
    )));

    // Initialize all utilities: keyboard, usb and keymap
    let (mut keyboard, mut usb_device, vial) = initialize_keyboard_and_usb_device::<
        Driver<'_, USB_OTG_HS>,
        Input<'_, AnyPin>,
        Output<'_, AnyPin>,
        Flash<'_, Blocking>,
        EEPROM_SIZE,
        ROW,
        COL,
        NUM_LAYER,
    >(driver, input_pins, output_pins, keymap);

    let usb_fut = usb_device.device.run();
    let keyboard_fut = async {
        loop {
            let _ = keyboard.keyboard_task().await;
            keyboard.send_report(&mut usb_device.keyboard_hid).await;
            keyboard.send_media_report(&mut usb_device.other_hid).await;
        }
    };

    let via_fut = async {
        loop {
            vial.process_via_report(&mut usb_device.via_hid).await;
            Timer::after_millis(1).await;
        }
    };
    join(usb_fut, join(keyboard_fut, via_fut)).await;
}
