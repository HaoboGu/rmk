#![no_std]
#![no_main]

#[macro_use]
mod macros;
mod keymap;
mod vial;

use crate::keymap::{COL, NUM_LAYER, ROW};
use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_nrf::{
    self as _, bind_interrupts,
    gpio::{AnyPin, Input, Output},
    interrupt::{self, InterruptExt, Priority},
    peripherals::{self, SAADC, USBD},
    saadc::{self, AnyInput, Input as _, Saadc},
    usb::{self, vbus_detect::SoftwareVbusDetect, Driver},
};
use panic_probe as _;
use rmk::{
    ble::SOFTWARE_VBUS,
    config::{BleBatteryConfig, KeyboardUsbConfig, RmkConfig, StorageConfig, VialConfig},
    split::central::{run_peripheral_monitor, run_rmk_split_central},
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
    // let (input_pins, output_pins) = config_matrix_pins_nrf!(peripherals: p, input: [P1_11, P1_10, P0_03, P0_28, P1_13], output:  [P0_30, P0_31, P0_29, P0_02, P0_05, P1_09, P0_13, P0_24, P0_09, P0_10, P1_00, P1_02, P1_04, P1_06]);

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

    let (input_pins, output_pins) =
        config_matrix_pins_nrf!(peripherals: p, input: [P0_12, P0_13], output:  [P0_14, P0_15]);

    let central_addr = [0x18, 0xe2, 0x21, 0x80, 0xc0, 0xc7];
    let peripheral_addr = [0x7e, 0xfe, 0x73, 0x9e, 0x66, 0xe3];

    join(
        run_rmk_split_central::<
            Input<'_>,
            Output<'_>,
            Driver<'_, USBD, &SoftwareVbusDetect>,
            ROW,
            COL,
            2,
            2,
            0,
            0,
            NUM_LAYER,
        >(
            input_pins,
            output_pins,
            driver,
            &mut keymap::get_default_keymap(),
            keyboard_config,
            central_addr,
            spawner,
        ),
        run_peripheral_monitor::<2, 1, 2, 2>(0, peripheral_addr),
    )
    .await;
}
