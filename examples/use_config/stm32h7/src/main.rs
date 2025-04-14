#![no_main]
#![no_std]

//! NOTE: This example compiles on latest main branch, which may be different from released version

use rmk::macros::rmk_keyboard;

/// There is an example of full customization of the keyboard with `rmk_keyboard` macro
#[rmk_keyboard]
mod my_keyboard {
    use embassy_stm32::time::Hertz;
    use embassy_stm32::usb::Driver;
    use embassy_stm32::Config;
    use rmk::channel::EVENT_CHANNEL;
    use rmk::futures::future::join3;
    use rmk::input_device::Runnable;
    use rmk::{run_devices, run_rmk};
    use static_cell::StaticCell;

    // If you want customize interrupte binding , use `#[Override(bind_interrupt)]` to override default interrupt binding
    #[Override(bind_interrupt)]
    fn bind_interrupt() {
        bind_interrupts!(struct Irqs {
            OTG_HS => InterruptHandler<USB_OTG_HS>;
        });
    }

    // If you're using custom chip config, use `#[Override(chip_config)]` to override embassy's default config
    #[Override(chip_config)]
    fn config() -> Config {
        let mut config = Config::default();
        {
            use embassy_stm32::rcc::*;
            config.rcc.hsi = Some(HSIPrescaler::DIV1);
            config.rcc.csi = true;
            // Needed for USB
            config.rcc.hsi48 = Some(Hsi48Config { sync_from_usb: true });
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

    // If you're using custom usb config, use `#[Override(usb)]` to override default usb config
    #[Override(usb)]
    fn usb() -> Driver<'_, USB_OTG_HS> {
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
        driver
    }

    // Use `#[Override(entry)]` to override default rmk keyboard runner
    #[Override(entry)]
    fn run() {
        // Start
        join3(
            run_devices!((matrix) => EVENT_CHANNEL),
            keyboard.run(),
            run_rmk(&keymap, driver, &mut storage, &mut light_controller, rmk_config),
        )
        .await;
    }
}
