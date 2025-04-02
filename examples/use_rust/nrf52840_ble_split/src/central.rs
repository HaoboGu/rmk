#![no_std]
#![no_main]

#[macro_use]
mod macros;
mod keymap;
mod vial;

use defmt::info;
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
    channel::EVENT_CHANNEL,
    config::{
        BleBatteryConfig, ControllerConfig, KeyboardUsbConfig, RmkConfig, StorageConfig, VialConfig,
    },
    debounce::default_debouncer::DefaultDebouncer,
    futures::future::{join, join4},
    initialize_keymap_and_storage, initialize_nrf_sd_and_flash,
    input_device::{
        adc::{AnalogEventType, NrfAdc},
        battery::BatteryProcessor,
        rotary_encoder::{DefaultPhase, RotaryEncoder, RotaryEncoderProcessor},
        Runnable,
    },
    keyboard::Keyboard,
    light::LightController,
    run_devices, run_processor_chain, run_rmk,
    split::central::{run_peripheral_manager, CentralMatrix},
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

    // Usb config
    let software_vbus = SOFTWARE_VBUS.get_or_init(|| SoftwareVbusDetect::new(true, false));
    let driver = Driver::new(p.USBD, Irqs, software_vbus);

    // Initialize the ADC. We are only using one channel for detecting battery level
    let adc_pin = p.P0_05.degrade_saadc();
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
    let ble_battery_config =
        BleBatteryConfig::new(Some(is_charging_pin), true, Some(charging_led), false);
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

    let (input_pins, output_pins) = config_matrix_pins_nrf!(peripherals: p, input: [P0_30, P0_31, P0_29, P0_02], output:  [P0_28, P0_03, P1_10, P1_11, P1_13, P0_09, P0_10]);

    // Initialize the Softdevice and flash
    let central_addr = [0x18, 0xe2, 0x21, 0x80, 0xc0, 0xc7];
    let peripheral_addr = [0x7e, 0xfe, 0x73, 0x9e, 0x66, 0xe3];
    let (sd, flash) = initialize_nrf_sd_and_flash(
        rmk_config.usb_config.product_name,
        spawner,
        Some(central_addr),
    );

    // Initialize the storage and keymap
    let mut default_keymap = keymap::get_default_keymap();
    let (keymap, storage) = initialize_keymap_and_storage(
        &mut default_keymap,
        flash,
        rmk_config.storage_config,
        rmk_config.behavior_config.clone(),
    )
    .await;

    let pin_a = Input::new(AnyPin::from(p.P1_06), embassy_nrf::gpio::Pull::None);
    let pin_b = Input::new(AnyPin::from(p.P1_04), embassy_nrf::gpio::Pull::None);
    let mut encoder = RotaryEncoder::with_phase(pin_a, pin_b, DefaultPhase, 0);

    // Initialize the matrix + keyboard
    let debouncer = DefaultDebouncer::<4, 7>::new();
    let mut matrix = CentralMatrix::<_, _, _, 0, 0, 4, 7>::new(input_pins, output_pins, debouncer);
    let mut keyboard = Keyboard::new(&keymap, rmk_config.behavior_config.clone());

    // Initialize the light controller
    let light_controller: LightController<Output> =
        LightController::new(ControllerConfig::default().light_config);

    let mut encoder_processor = RotaryEncoderProcessor::new(&keymap);

    let mut adc_device = NrfAdc::new(saadc, [AnalogEventType::Battery], 12000, None);
    let mut batt_proc = BatteryProcessor::new(2000, 2806, &keymap);

    // Start
    join4(
        run_devices! (
            (matrix, encoder, adc_device) => EVENT_CHANNEL,
        ),
        run_processor_chain! {
            EVENT_CHANNEL => [encoder_processor, batt_proc],
        },
        keyboard.run(),
        join(
            run_peripheral_manager::<4, 7, 4, 0>(0, peripheral_addr),
            run_rmk(&keymap, driver, storage, light_controller, rmk_config, sd),
        ),
    )
    .await;
}
