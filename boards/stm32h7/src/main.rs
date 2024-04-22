#![no_main]
#![no_std]

//! NOTE: This example compiles on latest main branch, which may be different from released version
mod keymap;
mod vial;

use crate::keymap::KEYMAP;
// use rmk::{config::RmkConfig, initialize_keyboard_with_config_and_run};
use rmk_macro::rmk_keyboard;
use vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};

// TODO: Move keymap definition to proc-macro

#[rmk_keyboard]
mod my_keyboard {
    use embassy_stm32::{
        bind_interrupts,
        peripherals::USB_OTG_HS,
        time::Hertz,
        usb_otg::{Driver, InterruptHandler},
        Config,
    };
    use static_cell::StaticCell;

    #[bind_interrupt]
    fn bind_interrupt() {
        bind_interrupts!(struct Irqs {
            OTG_HS => InterruptHandler<USB_OTG_HS>;
        });
    }

    #[Override(chip_config)]
    fn config() -> Config {
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
        config
    }

    #[Override(usb)]
    fn usb() -> Driver<'_, USB_OTG_HS> {
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
        driver
    }
}

// #[embassy_executor::main]
// async fn main(_spawner: Spawner) {
//     info!("RMK start!");
//     // RCC config
//

//     // Initialize peripherals
//     let p = embassy_stm32::init(config);

//     // Usb config
//     static EP_OUT_BUFFER: StaticCell<[u8; 1024]> = StaticCell::new();
//     let mut usb_config = embassy_stm32::usb_otg::Config::default();
//     usb_config.vbus_detection = false;
//     let driver = Driver::new_fs(
//         p.USB_OTG_HS,
//         Irqs,
//         p.PA12,
//         p.PA11,
//         &mut EP_OUT_BUFFER.init([0; 1024])[..],
//         usb_config,
//     );

//     // Use internal flash to emulate eeprom
//     let f = Flash::new_blocking(p.FLASH);

//     // Read configs from config file
//     let (input_pins, output_pins) = config_matrix!(p: p);
//     let light_config = config_light!(p: p);

//     let keyboard_config = RmkConfig {
//         usb_config: keyboard_usb_config,
//         vial_config,
//         light_config,
//         ..Default::default()
//     };

//     // Start serving
//     initialize_keyboard_with_config_and_run::<
//         Flash<'_, Blocking>,
//         Driver<'_, USB_OTG_HS>,
//         Input<'_, AnyPin>,
//         Output<'_, AnyPin>,
//         ROW,
//         COL,
//         NUM_LAYER,
//     >(
//         driver,
//         input_pins,
//         output_pins,
//         Some(f),
//         KEYMAP,
//         keyboard_config,
//     )
//     .await;
// }
