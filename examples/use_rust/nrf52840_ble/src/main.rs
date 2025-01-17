#![no_std]
#![no_main]

#[macro_use]
mod macros;
mod keymap;
mod vial;

use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_nrf::{
    self as _, bind_interrupts,
    gpio::{AnyPin, Input, Output},
    interrupt::{self, InterruptExt, Priority},
    peripherals::{self, SAADC},
    saadc::{self, AnyInput, Input as _, Saadc},
    usb::{self, vbus_detect::SoftwareVbusDetect, Driver},
};
use panic_probe as _;
use rmk::{
    ble::SOFTWARE_VBUS,
    config::{BleBatteryConfig, KeyboardUsbConfig, RmkConfig, StorageConfig, VialConfig},
    event::Event,
    input_device::{rotary_encoder::RotaryEncoder, InputDevice},
    keyboard::{EVENT_CHANNEL, EVENT_CHANNEL_SIZE},
    run_devices, run_rmk, CriticalSectionRawMutex, Sender,
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
    let keyboard_config = RmkConfig {
        usb_config: keyboard_usb_config,
        vial_config,
        ble_battery_config,
        storage_config,
        ..Default::default()
    };

    let mut my_device = MyDevice {};
    let pin_a = Input::new(AnyPin::from(p.P0_06), embassy_nrf::gpio::Pull::Up);
    let pin_b = Input::new(AnyPin::from(p.P0_11), embassy_nrf::gpio::Pull::Up);
    let mut encoder = RotaryEncoder::new(pin_a, pin_b, 0);

    embassy_futures::join::join(
        run_rmk(
            input_pins,
            output_pins,
            driver,
            &mut keymap::get_default_keymap(),
            keyboard_config,
            spawner,
        ),
        run_devices!(my_device, encoder),
    )
    .await;
}

struct MyDevice {}
impl InputDevice for MyDevice {
    async fn run(&mut self) {
        loop {
            embassy_time::Timer::after_secs(1).await;
        }
    }

    type EventType = Event;

    fn event_sender(&self) -> Sender<CriticalSectionRawMutex, Self::EventType, EVENT_CHANNEL_SIZE> {
        EVENT_CHANNEL.sender()
    }
}
