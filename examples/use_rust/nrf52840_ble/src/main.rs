#![no_std]
#![no_main]

#[macro_use]
mod macros;
mod keymap;
mod vial;

use core::cell::RefCell;

use defmt::info;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_nrf::{
    self as _, bind_interrupts,
    gpio::{AnyPin, Input, Output},
    interrupt::{self, InterruptExt, Priority},
    peripherals::{self, SAADC},
    saadc::{self, AnyInput, Input as _, Saadc},
    usb::{self, vbus_detect::SoftwareVbusDetect, Driver},
};
use keymap::{get_default_keymap, COL, NUM_LAYER, ROW};
use panic_probe as _;
use rmk::{
    action::KeyAction,
    bind_device_and_processor_and_run,
    ble::SOFTWARE_VBUS,
    config::{
        BleBatteryConfig, ControllerConfig, KeyboardConfig, KeyboardUsbConfig, RmkConfig,
        StorageConfig, VialConfig,
    },
    debounce::{default_bouncer::DefaultDebouncer, DebouncerTrait},
    event::{Event, KeyEvent},
    hid::Report,
    initialize_nrf_sd_and_flash,
    input_device::{rotary_encoder::RotaryEncoder, InputDevice, InputProcessor},
    keyboard::Keyboard,
    keymap::KeyMap,
    light::LightController,
    matrix::{Matrix, TestMatrix},
    run_rmk,
    storage::Storage,
};

use vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};

bind_interrupts!(struct Irqs {
    USBD => usb::InterruptHandler<peripherals::USBD>;
    SAADC => saadc::InterruptHandler;
});

/// Initializes the SAADC peripheral in single-ended mode on the given pin.
fn init_adc(adc_pin: AnyInput, adc: SAADC) -> Saadc<'static, 1> {
    // Then we initialize the ADC. We are only using one channel in this example.
    let config = saadc::Config::default();
    let channel_cfg = saadc::ChannelConfig::single_ended(adc_pin.degrade_saadc());
    interrupt::SAADC.set_priority(interrupt::Priority::P3);
    let saadc = saadc::Saadc::new(adc, Irqs, config, [channel_cfg]);
    saadc
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Hello NRF BLE!");
    let mut nrf_config = embassy_nrf::config::Config::default();
    nrf_config.gpiote_interrupt_priority = Priority::P3;
    nrf_config.time_interrupt_priority = Priority::P3;
    interrupt::USBD.set_priority(interrupt::Priority::P2);
    interrupt::CLOCK_POWER.set_priority(interrupt::Priority::P2);
    let p = embassy_nrf::init(nrf_config);
    // Disable external HF clock by default, reduce power consumption
    // info!("Enabling ext hfosc...");
    // ::embassy_nrf::pac::CLOCK.tasks_hfclkstart().write_value(1);
    // while ::embassy_nrf::pac::CLOCK.events_hfclkstarted().read() != 1 {}

    // Pin config
    let (input_pins, output_pins) = config_matrix_pins_nrf!(peripherals: p, input: [P1_11, P1_10, P0_03, P0_28, P1_13], output:  [P0_30, P0_31, P0_29, P0_02, P0_05, P1_09, P0_13, P0_24, P0_09, P0_10, P1_00, P1_02, P1_04, P1_06]);

    // Usb config
    let software_vbus = SOFTWARE_VBUS.get_or_init(|| SoftwareVbusDetect::new(true, false));
    let driver = Driver::new(p.USBD, Irqs, software_vbus);

    // Initialize the ADC. We are only using one channel for detecting battery level
    let adc_pin = p.P0_04.degrade_saadc();
    let is_charging_pin = Input::new(AnyPin::from(p.P0_07), embassy_nrf::gpio::Pull::Up);
    let charging_led = Output::new(
        AnyPin::from(p.P0_08),
        embassy_nrf::gpio::Level::Low,
        embassy_nrf::gpio::OutputDrive::Standard,
    );
    let saadc = init_adc(adc_pin, p.SAADC);
    // Wait for ADC calibration.
    saadc.calibrate().await;

    // Keyboard config
    let keyboard_usb_config = KeyboardUsbConfig {
        vid: 0x4c4b,
        pid: 0x4643,
        manufacturer: "Haobo",
        product_name: "RMK Keyboard",
        serial_number: "vial:f64c2b3c:000001",
    };
    let vial_config = VialConfig::new(VIAL_KEYBOARD_ID, VIAL_KEYBOARD_DEF);
    let ble_battery_config = BleBatteryConfig::new(
        Some(is_charging_pin),
        true,
        Some(charging_led),
        false,
        Some(saadc),
        2000,
        2806,
    );
    let storage_config = StorageConfig {
        start_addr: 0,
        num_sectors: 6,
        ..Default::default()
    };
    let rmk_config = RmkConfig {
        usb_config: keyboard_usb_config,
        vial_config,
        ble_battery_config,
        storage_config,
        ..Default::default()
    };
    // Keyboard config
    // let keyboard_config = KeyboardConfig {
    //     rmk_config,
    //     ..Default::default()
    // };

    let mut my_device = MyDevice {};
    let mut my_device2 = MyDevice {};
    let mut processor = MyProcessor {};
    let pin_a = Input::new(AnyPin::from(p.P0_06), embassy_nrf::gpio::Pull::Up);
    let pin_b = Input::new(AnyPin::from(p.P0_11), embassy_nrf::gpio::Pull::Up);
    let mut encoder = RotaryEncoder::new(pin_a, pin_b, 0);

    // Create the debouncer, use COL2ROW by default
    let debouncer = DefaultDebouncer::<ROW, COL>::new();

    // Keyboard matrix, use COL2ROW by default
    // let mut matrix = Matrix::<_, _, _, ROW, COL>::new(input_pins, output_pins, debouncer);
    let mut matrix = TestMatrix::<ROW, COL>::new();

    let (sd, flash) =
        initialize_nrf_sd_and_flash(rmk_config.usb_config.product_name, spawner, None);
    let mut storage = Storage::new(
        flash,
        &mut keymap::get_default_keymap(),
        rmk_config.storage_config,
    )
    .await;
    let mut km = get_default_keymap();
    let keymap = RefCell::new(
        KeyMap::new_from_storage(
            &mut km,
            Some(&mut storage),
            rmk_config.behavior_config.clone(),
        )
        .await,
    );
    let mut keyboard = Keyboard::new(&keymap, rmk_config.behavior_config.clone());

    let light_controller: LightController<Output> =
        LightController::new(ControllerConfig::default().light_config);

    join(
        bind_device_and_processor_and_run!((matrix) => keyboard),
        run_rmk(&keymap, driver, storage, light_controller, rmk_config, sd),
    )
    .await;

    // bind_device_and_processor!(device_task = (matrix: Matrix<Input<'static>, Output<'static>, DefaultDebouncer<ROW, COL>, ROW, COL>, my_device2: MyDevice) => processor: Keyboard<'static, ROW, COL, NUM_LAYER>);
    // spawner
    //     .spawn(device_task(keyboard, matrix, my_device2))
    //     .unwrap();

    // loop {}

    // embassy_futures::join::join(
    //     run_rmk(
    //         input_pins,
    //         output_pins,
    //         driver,
    //         &mut keymap::get_default_keymap(),
    //         keyboard_config,
    //         spawner,
    //     ),
    //     // Option 1
    //     f,
    // )
    // .await;
}

struct MyDevice {}
// impl InputDevice for MyDevice {
//     async fn run(&mut self) {
//         loop {
//             embassy_time::Timer::after_secs(1).await;
//             self.send_event(Event::Key(KeyEvent {
//                 row: 0,
//                 col: 0,
//                 pressed: true,
//             }))
//             .await;
//         }
//     }

//     type EventType = Event;

//     async fn send_event(&mut self, event: Self::EventType) {
//         EVENT_CHANNEL.sender().send(event).await
//     }
// }

impl InputDevice for MyDevice {
    async fn read_event(&mut self) -> Event {
        embassy_time::Timer::after_secs(1).await;
        Event::Key(KeyEvent {
            row: 0,
            col: 0,
            pressed: true,
        })
    }
}

struct MyProcessor {}
// impl InputProcessor for MyProcessor {
//     async fn process(&mut self, event: Event) {
//         match event {
//             Event::Key(key) => {
//                 // Process key event
//                 info!("Hey received key")
//             }
//             _ => {}
//         }
//     }
//     type EventType = Event;

//     async fn read_event(&self) -> Self::EventType {
//         EVENT_CHANNEL.receive().await
//     }

//     async fn send_report(&self, report: Report) {
//         // Send report
//         info!("Sending report");
//         KEYBOARD_REPORT_CHANNEL.send(report).await
//     }
// }
impl InputProcessor for MyProcessor {
    async fn process(&mut self, event: Event) {
        match event {
            Event::Key(key) => {
                // Process key event
                info!("Hey received key")
            }
            _ => {}
        }
    }
    // type EventType = Event;
    // type ReportType = rmk::usb::descriptor::KeyboardReport;

    // async fn read_event(&self) -> Self::EventType {
    //     EVENT_CHANNEL.receive().await
    // }

    // fn send_report(&self, report: Self::ReportType) -> impl Future<Output = ()> {
    //     async {
    //         // Send report
    //         info!("Sending report");
    //         KEYBOARD_REPORT_CHANNEL.send(Report)
    //     }
    // }
}
