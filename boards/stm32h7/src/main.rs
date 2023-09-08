#![no_main]
#![no_std]
#![feature(type_alias_impl_trait)]
#![allow(dead_code)]

#[macro_use]
mod macros;
mod keymap;

use panic_rtt_target as _;
use rtic::app;

#[app(device = stm32h7xx_hal::pac, peripherals = true)]
mod app {
    use log::info;
    use rmk::config::KEYBOARD_CONFIG;
    use rmk::keyboard::Keyboard;
    use rmk::rtt_logger;
    use rmk::usb::create_usb_device_and_hid_class;
    use rtic_monotonics::systick::*;
    use stm32h7xx_hal::{
        gpio::{ErasedPin, Input, Output, PE3},
        pac::rcc::cdccip2r::USBSEL_A::Hsi48,
        prelude::*,
        usb_hs::{UsbBus, USB1},
    };
    use usb_device::prelude::*;
    use usbd_hid::hid_class::HIDClass;

    static mut EP_MEMORY: [u32; 1024] = [0; 1024];

    #[shared]
    struct Shared {
        usb: (
            HIDClass<'static, UsbBus<USB1>>,
            UsbDevice<'static, UsbBus<USB1>>,
        ),
        led: PE3<Output>,
    }

    #[local]
    struct Local {
        keyboard: Keyboard<ErasedPin<Input>, ErasedPin<Output>, 4, 3, 2>,
    }

    #[init]
    fn init(cx: init::Context) -> (Shared, Local) {
        rtt_logger::init();
        let cp = cx.core;
        let dp = cx.device;

        // Initialize the systick interrupt & obtain the token to prove that we did
        let systick_mono_token = rtic_monotonics::create_systick_token!();
        // Default clock rate is 225MHz
        Systick::start(cp.SYST, 225_000_000, systick_mono_token);

        // Power config
        let pwr = dp.PWR.constrain();
        let pwrcfg = pwr.freeze();

        // Clock config
        let rcc = dp.RCC.constrain();
        let mut ccdr = rcc
            .use_hse(25.MHz())
            .sys_ck(225.MHz())
            .hclk(225.MHz())
            .per_ck(225.MHz())
            .freeze(pwrcfg, &dp.SYSCFG);
        // Check HSI 48MHZ
        let _ = ccdr.clocks.hsi48_ck().expect("HSI48 must run");
        // Config HSI
        ccdr.peripheral.kernel_usb_clk_mux(Hsi48);

        // GPIO config
        let gpioa = dp.GPIOA.split(ccdr.peripheral.GPIOA);
        let gpioe = dp.GPIOE.split(ccdr.peripheral.GPIOE);
        let gpiod = dp.GPIOD.split(ccdr.peripheral.GPIOD);
        let gpiob = dp.GPIOB.split(ccdr.peripheral.GPIOB);

        // USB config
        let usb_dm = gpioa.pa11.into_alternate();
        let usb_dp = gpioa.pa12.into_alternate();
        let usb: USB1 = USB1::new(
            dp.OTG1_HS_GLOBAL,
            dp.OTG1_HS_DEVICE,
            dp.OTG1_HS_PWRCLK,
            usb_dm,
            usb_dp,
            ccdr.peripheral.USB1OTG,
            &ccdr.clocks,
        );
        let usb_bus = cortex_m::singleton!(
            : usb_device::class_prelude::UsbBusAllocator<UsbBus<USB1>> =
                UsbBus::new(usb, unsafe { &mut EP_MEMORY })
        )
        .unwrap();
        let (hid, usb_dev) = create_usb_device_and_hid_class(usb_bus, &KEYBOARD_CONFIG);

        // Led config
        let mut led = gpioe.pe3.into_push_pull_output();
        led.set_high();

        // Initialize keyboard matrix pins
        let (input_pins, output_pins) = config_matrix_pins!(input: [gpiod.pd9, gpiod.pd8, gpiob.pb13, gpiob.pb12], output: [gpioe.pe13,gpioe.pe14,gpioe.pe15]);
        // Initialize keyboard
        let keyboard = Keyboard::new(input_pins, output_pins, crate::keymap::KEYMAP);

        // Spawn keyboard task
        scan::spawn().ok();

        // RTIC resources
        (
            Shared {
                usb: (hid, usb_dev),
                led,
            },
            Local { keyboard },
        )
    }

    #[task(local = [keyboard], shared = [usb])]
    async fn scan(mut cx: scan::Context) {
        // Keyboard scan task
        info!("Start matrix scanning");
        loop {
            cx.local.keyboard.keyboard_task().await.unwrap();
            cx.shared.usb.lock(|(hid, _usb_device)| {
                cx.local.keyboard.send_report(hid);
            })
        }
    }

    #[task(binds = OTG_HS, shared = [usb])]
    fn usb_poll(mut cx: usb_poll::Context) {
        cx.shared.usb.lock(|(hid, usb_device)| {
            usb_device.poll(&mut [hid]);
        });
    }
}
