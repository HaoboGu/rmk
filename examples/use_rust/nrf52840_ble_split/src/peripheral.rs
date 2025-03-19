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
use rmk::{
    channel::EVENT_CHANNEL, debounce::default_debouncer::DefaultDebouncer, futures::future::join,
    matrix::Matrix, run_devices, split::peripheral::run_rmk_split_peripheral,
};

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
    interrupt::CLOCK_POWER.set_priority(interrupt::Priority::P2);
    let p = embassy_nrf::init(nrf_config);
    // Disable external HF clock by default, reduce power consumption
    // info!("Enabling ext hfosc...");
    // ::embassy_nrf::pac::CLOCK.tasks_hfclkstart().write_value(1);
    // while ::embassy_nrf::pac::CLOCK.events_hfclkstarted().read() != 1 {}

    // Initialize the ADC. We are only using one channel for detecting battery level
    let adc_pin = p.P0_05.degrade_saadc();
    let saadc = init_adc(adc_pin, p.SAADC);
    // Wait for ADC calibration.
    saadc.calibrate().await;

    let (input_pins, output_pins) = config_matrix_pins_nrf!(peripherals: p, input: [P1_09, P0_28, P0_03, P1_10], output:  [P0_30, P0_31, P0_29, P0_02, P1_13, P0_10, P0_09]);

    let central_addr = [0x18, 0xe2, 0x21, 0x80, 0xc0, 0xc7];
    let peripheral_addr = [0x7e, 0xfe, 0x73, 0x9e, 0x66, 0xe3];

    // Initialize the peripheral matrix
    let debouncer = DefaultDebouncer::<4, 7>::new();
    let mut matrix = Matrix::<_, _, _, 4, 7>::new(input_pins, output_pins, debouncer);
    // let mut matrix = rmk::matrix::TestMatrix::<4, 7>::new();

    // Start
    join(
        run_devices! (
            (matrix) => EVENT_CHANNEL, // Peripheral uses EVENT_CHANNEL to send events to central
        ),
        run_rmk_split_peripheral(central_addr, peripheral_addr, spawner),
    )
    .await;
}
