#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]
#![feature(async_closure)]

#[macro_use]
mod macros;
mod keymap;
mod vial;

use bleps::{
    ad_structure::{
        create_advertising_data, AdStructure, BR_EDR_NOT_SUPPORTED, LE_GENERAL_DISCOVERABLE,
    },
    async_attribute_server::AttributeServer,
    asynch::Ble,
    attribute_server::NotificationData,
    gatt,
};
use core::cell::RefCell;
use defmt::info;
use embassy_executor::Spawner;
use embedded_hal_async::digital::Wait;
use esp_backtrace as _;
pub use esp_hal as hal;
use esp_hal::gpio::{AnyPin, Output, PushPull};
use esp_println as _;
use esp_println::println;
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
    let init = initialize(
        EspWifiInitFor::Ble,
        timer,
        Rng::new(peripherals.RNG),
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
    println!("Connector created");
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
    )
    .await;

    }
    // let pin_ref = RefCell::new(button);
    // let pin_ref = &pin_ref;

    // loop {
    //     println!("{:?}", ble.init().await);
    //     println!("{:?}", ble.cmd_set_le_advertising_parameters().await);
    //     println!(
    //         "{:?}",
    //         ble.cmd_set_le_advertising_data(
    //             create_advertising_data(&[
    //                 AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
    //                 AdStructure::ServiceUuids16(&[Uuid::Uuid16(0x1809)]),
    //                 AdStructure::CompleteLocalName(SOC_NAME),
    //             ])
    //             .unwrap()
    //         )
    //         .await
    //     );
    //     println!("{:?}", ble.cmd_set_le_advertise_enable(true).await);

    //     info!("demft enabled!");
    //     println!("started advertising");

    //     let mut rf = |_offset: usize, data: &mut [u8]| {
    //         data[..20].copy_from_slice(&b"Hello Bare-Metal BLE"[..]);
    //         17
    //     };
    //     let mut wf = |offset: usize, data: &[u8]| {
    //         println!("RECEIVED: {} {:?}", offset, data);
    //     };

    //     let mut wf2 = |offset: usize, data: &[u8]| {
    //         println!("RECEIVED: {} {:?}", offset, data);
    //     };

    //     let mut rf3 = |_offset: usize, data: &mut [u8]| {
    //         data[..5].copy_from_slice(&b"Hola!"[..]);
    //         5
    //     };
    //     let mut wf3 = |offset: usize, data: &[u8]| {
    //         println!("RECEIVED: Offset {}, data {:?}", offset, data);
    //     };

    //     gatt!([service {
    //         uuid: "937312e0-2354-11eb-9f10-fbc30a62cf38",
    //         characteristics: [
    //             characteristic {
    //                 uuid: "937312e0-2354-11eb-9f10-fbc30a62cf38",
    //                 read: rf,
    //                 write: wf,
    //             },
    //             characteristic {
    //                 uuid: "957312e0-2354-11eb-9f10-fbc30a62cf38",
    //                 write: wf2,
    //             },
    //             characteristic {
    //                 name: "my_characteristic",
    //                 uuid: "987312e0-2354-11eb-9f10-fbc30a62cf38",
    //                 notify: true,
    //                 read: rf3,
    //                 write: wf3,
    //             },
    //         ],
    //     },]);

    //     let mut rng = bleps::no_rng::NoRng;
    //     let mut srv = AttributeServer::new(&mut ble, &mut gatt_attributes, &mut rng);

    //     let counter = RefCell::new(0u8);
    //     let counter = &counter;

    //     let mut notifier = || {
    //         // TODO how to check if notifications are enabled for the characteristic?
    //         // maybe pass something into the closure which just can query the characteristic value
    //         // probably passing in the attribute server won't work?

    //         async {
    //             pin_ref.borrow_mut().wait_for_rising_edge().await.unwrap();
    //             let mut data = [0u8; 13];
    //             data.copy_from_slice(b"Notification0");
    //             {
    //                 let mut counter = counter.borrow_mut();
    //                 data[data.len() - 1] += *counter;
    //                 *counter = (*counter + 1) % 10;
    //             }
    //             NotificationData::new(my_characteristic_handle, &data)
    //         }
    //     };

    //     srv.run(&mut notifier).await.unwrap();
    // }
}
