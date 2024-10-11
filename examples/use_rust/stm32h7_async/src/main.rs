#![no_main]
#![no_std]

//! NOTE: This example compiles on latest main branch, which may be different from released version

#[macro_use]
mod macros;
mod keymap;
mod vial;

use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_stm32::{
    bind_interrupts,
    exti::{Channel, ExtiInput},
    flash::Flash,
    gpio::{Input, Output, Pull},
    peripherals::USB_OTG_HS,
    time::Hertz,
    usb::{Driver, InterruptHandler},
    Config,
};
use panic_probe as _;
use rmk::{
    config::{RmkConfig, VialConfig},
    run_rmk,
};
use static_cell::StaticCell;
use vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};

bind_interrupts!(struct Irqs {
    OTG_HS => InterruptHandler<USB_OTG_HS>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("RMK start!");
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
    let mut usb_config = embassy_stm32::usb::Config::default();
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
    // let (input_pins, output_pins) = config_matrix_pins_stm32!(peripherals: p, input: [PD9, PD8, PB13, PB12], output: [PE13, PE14, PE15]);
    let output_pins = config_output_pins_stm32!(peripherals: p, output: [PE13, PE14, PE15]);
    
    let pd9 = ExtiInput::new(p.PD9,  p.EXTI9, Pull::Down);
    let pd8 = ExtiInput::new(p.PD8,  p.EXTI8, Pull::Down);
    let pb13 = ExtiInput::new(p.PB13, p.EXTI13, Pull::Down);
    let pb12 = ExtiInput::new(p.PB12, p.EXTI12, Pull::Down);
    let input_pins = [pd9, pd8, pb13, pb12];

    // Use internal flash to emulate eeprom
    let f = Flash::new_blocking(p.FLASH);

    let vial_config = VialConfig::new(VIAL_KEYBOARD_ID, VIAL_KEYBOARD_DEF);

    let keyboard_config = RmkConfig {
        vial_config,
        ..Default::default()
    };

    // Start serving
    run_rmk(
        input_pins,
        output_pins,
        driver,
        f,
        crate::keymap::KEYMAP,
        keyboard_config,
        spawner,
    )
    .await;
}
