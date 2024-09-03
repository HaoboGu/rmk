#![no_std]
#![no_main]

#[macro_use]
mod macros;

use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_nrf::{
    self as _, bind_interrupts,
    gpio::{AnyPin, Input, Output},
    interrupt::{self, InterruptExt, Priority},
    peripherals::SAADC,
    saadc::{self, AnyInput, Input as _, Saadc},
};
use panic_probe as _;
use rmk::split::slave::initialize_nrf_ble_split_slave_and_run;

bind_interrupts!(struct Irqs {
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
    interrupt::POWER_CLOCK.set_priority(interrupt::Priority::P2);
    let p = embassy_nrf::init(nrf_config);
    // Disable external HF clock by default, reduce power consumption
    // let clock: embassy_nrf::pac::CLOCK = unsafe { core::mem::transmute(()) };
    // info!("Enabling ext hfosc...");
    // clock.tasks_hfclkstart.write(|w| unsafe { w.bits(1) });
    // while clock.events_hfclkstarted.read().bits() != 1 {}

    // Initialize the ADC. We are only using one channel for detecting battery level
    let adc_pin = p.P0_04.degrade_saadc();
    // TODO: Slave's charging state and battery level
    let _is_charging_pin = Input::new(AnyPin::from(p.P0_07), embassy_nrf::gpio::Pull::Up);
    let _charging_led = Output::new(
        AnyPin::from(p.P0_08),
        embassy_nrf::gpio::Level::Low,
        embassy_nrf::gpio::OutputDrive::Standard,
    );
    let saadc = init_adc(adc_pin, p.SAADC);
    // Wait for ADC calibration.
    saadc.calibrate().await;

    let (input_pins, output_pins) =
        config_matrix_pins_nrf!(peripherals: p, input: [P1_11, P1_10], output:  [P0_30, P0_31]);

    let master_addr = [0x18, 0xe2, 0x21, 0x80, 0xc0, 0xc7];
    let slave_addr = [0x7e, 0xfe, 0x73, 0x9e, 0x66, 0xe3];

    initialize_nrf_ble_split_slave_and_run::<Input<'_>, Output<'_>, 2, 2>(
        input_pins,
        output_pins,
        master_addr,
        slave_addr,
        spawner,
    )
    .await;
}