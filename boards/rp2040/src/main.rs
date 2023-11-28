#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

mod keymap;

use defmt_rtt as _;
use panic_probe as _;

#[rtic::app(
    device = rp_pico::hal::pac,
    dispatchers = [TIMER_IRQ_1]
)]
mod app {
    use defmt::*;
    use embedded_hal::digital::v2::{OutputPin, ToggleableOutputPin};
    use rmk::{
        config::KEYBOARD_CONFIG, eeprom::EepromStorageConfig, flash::EmptyFlashWrapper,
        initialize_keyboard_and_usb_device, keyboard::Keyboard, usb::KeyboardUsbDevice,
        usb_device::class_prelude::UsbBusAllocator,
    };
    use rp_pico::{
        hal::{
            clocks::init_clocks_and_plls, gpio::*, sio, sio::Sio, usb::UsbBus, watchdog::Watchdog,
        },
        Pins, XOSC_CRYSTAL_FREQ,
    };
    use rtic_monotonics::systick::*;

    // Static usb bus instance
    static mut USB_BUS: Option<UsbBusAllocator<UsbBus>> = None;

    #[shared]
    struct Shared {
        usb_device: KeyboardUsbDevice<'static, UsbBus>,
    }

    #[local]
    struct Local {
        led: Pin<bank0::Gpio25, FunctionSio<SioOutput>, PullDown>,
        keyboard: Keyboard<
            Pin<DynPinId, FunctionSio<SioInput>, PullDown>,
            Pin<DynPinId, FunctionSio<SioOutput>, PullDown>,
            EmptyFlashWrapper,
            0,
            4,
            3,
            2,
        >,
    }

    #[init]
    fn init(c: init::Context) -> (Shared, Local) {
        // Soft-reset does not release the hardware spinlocks
        // Release them now to avoid a deadlock after debug or watchdog reset
        unsafe {
            sio::spinlock_reset();
        }

        // Initialize the systick interrupt & obtain the token to prove that we did
        let systick_mono_token = rtic_monotonics::create_systick_token!();
        // Default rp2040 clock-rate is 125MHz
        Systick::start(c.core.SYST, 125_000_000, systick_mono_token);

        let mut resets = c.device.RESETS;
        // Initialize clocks
        let mut watchdog = Watchdog::new(c.device.WATCHDOG);
        let clocks = init_clocks_and_plls(
            XOSC_CRYSTAL_FREQ,
            c.device.XOSC,
            c.device.CLOCKS,
            c.device.PLL_SYS,
            c.device.PLL_USB,
            &mut resets,
            &mut watchdog,
        )
        .ok()
        .unwrap();

        // GPIO config
        let sio = Sio::new(c.device.SIO);
        let pins = Pins::new(
            c.device.IO_BANK0,
            c.device.PADS_BANK0,
            sio.gpio_bank0,
            &mut resets,
        );
        let mut led = pins.led.into_push_pull_output();
        led.set_low().unwrap();

        // Usb config
        let usb_bus = UsbBusAllocator::new(UsbBus::new(
            c.device.USBCTRL_REGS,
            c.device.USBCTRL_DPRAM,
            clocks.usb_clock,
            true,
            &mut resets,
        ));

        unsafe {
            USB_BUS = Some(usb_bus);
        }

        // Matrix config
        let gp6 = pins.gpio6.into_pull_down_input().into_dyn_pin();
        let gp7 = pins.gpio7.into_pull_down_input().into_dyn_pin();
        let gp8 = pins.gpio8.into_pull_down_input().into_dyn_pin();
        let gp9 = pins.gpio9.into_pull_down_input().into_dyn_pin();
        let gp19 = pins
            .gpio19
            .into_push_pull_output_in_state(PinState::Low)
            .into_dyn_pin();
        let gp20 = pins
            .gpio20
            .into_push_pull_output_in_state(PinState::Low)
            .into_dyn_pin();
        let gp21 = pins
            .gpio21
            .into_push_pull_output_in_state(PinState::Low)
            .into_dyn_pin();
        let output_pins = [gp19, gp20, gp21];
        let input_pins = [gp6, gp7, gp8, gp9];

        let (keyboard, usb_device) = initialize_keyboard_and_usb_device(
            unsafe { USB_BUS.as_ref().unwrap() },
            &KEYBOARD_CONFIG,
            None,
            EepromStorageConfig::default(),
            None,
            input_pins,
            output_pins,
            crate::keymap::KEYMAP,
        );

        // Spawn heartbeat task
        scan::spawn().ok();

        (Shared { usb_device }, Local { led, keyboard })
    }

    #[task(local = [keyboard, led], priority = 1, shared = [usb_device])]
    async fn scan(mut cx: scan::Context) {
        // Keyboard scan task
        info!("Start matrix scanning");
        loop {
            cx.local.keyboard.keyboard_task().await.unwrap();
            cx.shared.usb_device.lock(|usb_device| {
                // Send keyboard report
                cx.local.keyboard.send_report(usb_device);

                // Process via report
                cx.local.keyboard.process_via_report(usb_device);
            });

            // Blink LED
            let _ = cx.local.led.toggle();

            // Scanning frequency: 10KHZ
            Systick::delay(100.micros()).await;
        }
    }

    #[task(binds = USBCTRL_IRQ, priority = 2, shared = [usb_device])]
    fn usb_poll(mut cx: usb_poll::Context) {
        cx.shared.usb_device.lock(|usb_device| {
            usb_device.usb_poll();
        });
    }
}
