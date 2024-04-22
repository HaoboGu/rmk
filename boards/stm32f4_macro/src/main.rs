#![no_main]
#![no_std]

mod keymap;
mod vial;

use crate::keymap::KEYMAP;
use rmk::macros::rmk_keyboard;
use vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};

#[rmk_keyboard]
mod keyboard {
    use embassy_stm32::{
        bind_interrupts,
        flash::{Blocking, Flash},
        gpio::{AnyPin, Input, Output},
        peripherals::USB_OTG_FS,
        time::Hertz,
        usb_otg::{Driver, InterruptHandler},
        Config,
    };
    use static_cell::StaticCell;

    #[bind_interrupt]
    fn bind_interrupt() {
        bind_interrupts!(struct Irqs {
            OTG_FS => InterruptHandler<USB_OTG_FS>;
        });
    }

    #[Override(chip_config)]
    fn config() -> Config {
        let mut config = Config::default();
        {
            use embassy_stm32::rcc::*;
            config.rcc.hse = Some(Hse {
                // freq: Hertz(25_000_000),
                freq: Hertz(8_000_000),
                mode: HseMode::Oscillator,
            });
            config.rcc.pll_src = PllSource::HSE;
            config.rcc.pll = Some(Pll {
                // prediv: PllPreDiv::DIV25,
                prediv: PllPreDiv::DIV8,
                mul: PllMul::MUL192,
                divp: Some(PllPDiv::DIV2), // 25mhz / 25 * 192 / 2 = 96Mhz.
                divq: Some(PllQDiv::DIV4), // 25mhz / 25 * 192 / 4 = 48Mhz.
                divr: None,
            });
            config.rcc.ahb_pre = AHBPrescaler::DIV1;
            // config.rcc.apb1_pre = APBPrescaler::DIV2;
            // config.rcc.apb2_pre = APBPrescaler::DIV1;
            config.rcc.apb1_pre = APBPrescaler::DIV4;
            config.rcc.apb2_pre = APBPrescaler::DIV2;
            config.rcc.sys = Sysclk::PLL1_P;
        }
        config
    }

    #[Override(usb)]
    fn usb() -> Driver<'_, USB_OTG_FS> {
        // Usb config
        static EP_OUT_BUFFER: StaticCell<[u8; 1024]> = StaticCell::new();
        let mut usb_config = embassy_stm32::usb_otg::Config::default();
        usb_config.vbus_detection = false;
        let driver = Driver::new_fs(
            p.USB_OTG_FS,
            Irqs,
            p.PA12,
            p.PA11,
            &mut EP_OUT_BUFFER.init([0; 1024])[..],
            usb_config,
        );
        driver
    }

    #[Override(entry)]
    fn entry() {
        rmk::initialize_keyboard_with_config_and_run::<
            Flash<'_, Blocking>,
            Driver<'_, USB_OTG_FS>,
            Input<'_, AnyPin>,
            Output<'_, AnyPin>,
            ROW,
            COL,
            NUM_LAYER,
        >(
            driver,
            input_pins,
            output_pins,
            Some(f),
            KEYMAP,
            keyboard_config,
        )
        .await;
    }
}

// use crate::keymap::{COL, NUM_LAYER, ROW};
// use defmt::*;
// use defmt_rtt as _;
// use embassy_executor::Spawner;
// use embassy_stm32::{
//     bind_interrupts,
//     flash::{Blocking, Flash},
//     gpio::{AnyPin, Input, Output},
//     peripherals::USB_OTG_FS,
//     time::Hertz,
//     usb_otg::{Driver, InterruptHandler},
//     Config,
// };
// use panic_probe as _;
// use rmk::initialize_keyboard_and_run;
// use static_cell::StaticCell;
// use vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};

// bind_interrupts!(struct Irqs {
//     OTG_FS => InterruptHandler<USB_OTG_FS>;
// });

// #[embassy_executor::main]
// async fn main(_spawner: Spawner) {
//     info!("RMK start!");
//     // RCC config
//     // Tested on stm32f411 with 25MHZ HSE and stm32f407 with 8MHZ HSE
//     // The commented rcc configuration code is for stm32f407
//     let mut config = Config::default();
//     {
//         use embassy_stm32::rcc::*;
//         config.rcc.hse = Some(Hse {
//             freq: Hertz(25_000_000),
//             // freq: Hertz(8_000_000),
//             mode: HseMode::Oscillator,
//         });
//         config.rcc.pll_src = PllSource::HSE;
//         config.rcc.pll = Some(Pll {
//             prediv: PllPreDiv::DIV25,
//             // prediv: PllPreDiv::DIV8,
//             mul: PllMul::MUL192,
//             divp: Some(PllPDiv::DIV2), // 25mhz / 25 * 192 / 2 = 96Mhz.
//             divq: Some(PllQDiv::DIV4), // 25mhz / 25 * 192 / 4 = 48Mhz.
//             divr: None,
//         });
//         config.rcc.ahb_pre = AHBPrescaler::DIV1;
//         config.rcc.apb1_pre = APBPrescaler::DIV2;
//         config.rcc.apb2_pre = APBPrescaler::DIV1;
//         // config.rcc.apb1_pre = APBPrescaler::DIV4;
//         // config.rcc.apb2_pre = APBPrescaler::DIV2;
//         config.rcc.sys = Sysclk::PLL1_P;
//     }

//     // Initialize peripherals
//     let p = embassy_stm32::init(config);

//     // Usb config
//     static EP_OUT_BUFFER: StaticCell<[u8; 1024]> = StaticCell::new();
//     let mut usb_config = embassy_stm32::usb_otg::Config::default();
//     usb_config.vbus_detection = false;
//     let driver = Driver::new_fs(
//         p.USB_OTG_FS,
//         Irqs,
//         p.PA12,
//         p.PA11,
//         &mut EP_OUT_BUFFER.init([0; 1024])[..],
//         usb_config,
//     );

//     // Pin config
//     let (input_pins, output_pins) = config_matrix_pins_stm32!(peripherals: p, input: [PD9, PD8, PB13, PB12], output: [PE13, PE14, PE15]);

//     // Use internal flash to emulate eeprom
//     let f = Flash::new_blocking(p.FLASH);

//     // Start serving
//     initialize_keyboard_and_run::<
//         Driver<'_, USB_OTG_FS>,
//         Input<'_, AnyPin>,
//         Output<'_, AnyPin>,
//         Flash<'_, Blocking>,
//         ROW,
//         COL,
//         NUM_LAYER,
//     >(
//         driver,
//         input_pins,
//         output_pins,
//         Some(f),
//         crate::keymap::KEYMAP,
//         &VIAL_KEYBOARD_ID,
//         &VIAL_KEYBOARD_DEF,
//     )
//     .await;
// }
