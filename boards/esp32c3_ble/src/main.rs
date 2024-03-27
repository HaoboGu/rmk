#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]
#![feature(async_closure)]

#[macro_use]
mod macros;
mod keymap;
mod vial;

use bleps::asynch::Ble;
use defmt::*;
use embassy_executor::Spawner;
use esp_backtrace as _;
pub use esp_hal as hal;
use esp_hal::gpio::{AnyPin, Output, PushPull};
use esp_println as _;
use esp_wifi::{ble::controller::asynch::BleConnector, initialize, EspWifiInitFor};
use hal::{
    clock::ClockControl,
    embassy,
    gpio::{Input, PullDown},
    peripherals::*,
    prelude::*,
    timer::TimerGroup,
    Rng, IO,
};
use rmk::{
    config::{KeyboardUsbConfig, RmkConfig, VialConfig},
    initialize_esp_ble_keyboard_with_config_and_run,
};

use crate::{
    keymap::{COL, NUM_LAYER, ROW},
    vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID},
};

pub type _BootButton = crate::hal::gpio::Gpio9<crate::hal::gpio::Input<crate::hal::gpio::PullDown>>;
pub const SOC_NAME: &str = "ESP32-C3";
#[main]
async fn main(spawner: Spawner) {
    info!("Hello ESP BLE!");

    // Device config
    let peripherals = Peripherals::take();
    let system = peripherals.SYSTEM.split();
    let clocks = ClockControl::max(system.clock_control).freeze();

    let timer = hal::systimer::SystemTimer::new(peripherals.SYSTIMER).alarm0;
    let rng = Rng::new(peripherals.RNG);
    let init = initialize(
        EspWifiInitFor::Ble,
        timer,
        rng.clone(),
        system.radio_clock_control,
        &clocks,
    )
    .unwrap();

    // Pin config
    let io = IO::new(peripherals.GPIO, peripherals.IO_MUX);
    // let button = io.pins.gpio9.into_pull_down_input();
    let (input_pins, output_pins) = config_matrix_pins_esp!(io: io, input: [gpio6, gpio7, gpio8, gpio9], output: [gpio10, gpio11, gpio12]);

    // Async requires the GPIO interrupt to wake futures
    hal::interrupt::enable(
        hal::peripherals::Interrupt::GPIO,
        hal::interrupt::Priority::Priority1,
    )
    .unwrap();

    // Keyboard config
    let keyboard_usb_config = KeyboardUsbConfig::new(
        0x4c4b,
        0x4643,
        Some("Haobo"),
        Some("RMK Keyboard"),
        Some("00000001"),
    );
    let vial_config = VialConfig::new(VIAL_KEYBOARD_ID, VIAL_KEYBOARD_DEF);
    let keyboard_config = RmkConfig {
        usb_config: keyboard_usb_config,
        vial_config,
        ..Default::default()
    };

    let timer_group0 = TimerGroup::new(peripherals.TIMG0, &clocks);
    embassy::init(&clocks, timer_group0);

    let mut bluetooth = peripherals.BT;

    loop {
        let connector = BleConnector::new(&init, &mut bluetooth);
        let mut ble = Ble::new(connector, esp_wifi::current_millis);
        debug!("Connector created");
        initialize_esp_ble_keyboard_with_config_and_run::<
            BleConnector<'_>,
            AnyPin<Input<PullDown>, _>,
            AnyPin<Output<PushPull>>,
            ROW,
            COL,
            NUM_LAYER,
        >(
            crate::keymap::KEYMAP,
            input_pins,
            output_pins,
            keyboard_config,
            &mut ble,
            rng,
        )
        .await;
    }
}
