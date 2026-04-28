#![no_main]
#![no_std]

#[macro_use]
mod keymap;
#[macro_use]
mod macros;
mod vial;

use defmt::{error, info};
use defmt_rtt as _;
use embassy_executor::Spawner;
use keymap::{COL, ROW, SIZE};
use panic_probe as _;
use rand_chacha::ChaCha12Rng;
use rand_core::SeedableRng;
use rmk::ble::build_ble_stack;
use rmk::config::{BehaviorConfig, DeviceConfig, PositionalConfig, RmkConfig, StorageConfig, VialConfig};
use rmk::core_traits::Runnable;
use rmk::debounce::default_debouncer::DefaultDebouncer;
use rmk::futures::future::join4;
use rmk::host::HostService;
use rmk::input_device::rotary_encoder::RotaryEncoder;
use rmk::keyboard::Keyboard;
use rmk::matrix::direct_pin::DirectPinMatrix;
use rmk::storage::async_flash_wrapper;
use rmk::{HostResources, KeymapData, initialize_keymap_and_storage, run_all, run_rmk};
use sifli_hal::efuse::Efuse;
use sifli_hal::gpio::Input;
use sifli_hal::mpi::{BlockingNorFlash, BuiltInProfile, NorConfig, ProfileSource};
use sifli_hal::peripherals::{EFUSEC, USBC};
use sifli_hal::rng::Rng;
use sifli_hal::usb::{Driver, InterruptHandler as UsbInterruptHandler};
use sifli_hal::{bind_interrupts, ipc, pmu};
use sifli_radio::bluetooth::{BleController, BleInitConfig};
use vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};

bind_interrupts!(struct Irqs {
    MAILBOX2_CH1 => ipc::InterruptHandler;
    USBC => UsbInterruptHandler<USBC>;
});

/// Apply factory PMU trims and get a stable BLE static random address.
fn ble_addr(efusec: EFUSEC) -> [u8; 6] {
    let mut addr = [0u8; 6];
    match Efuse::new(efusec) {
        Ok(efuse) => {
            let applied = pmu::apply_calibration(efuse.calibration());
            info!("PMU calibration applied from eFUSE: {}", applied);
            // First 6 bytes of UID → BD_ADDR (little-endian byte order matches
            // the HCI Set_Random_Address command).
            addr.copy_from_slice(&efuse.uid().bytes()[..6]);
        }
        Err(e) => {
            error!("Efuse init failed, PMU trims left at defaults: {:?}", e);
            // Use fixed ble addr if Efuse init failed
            addr = [0x18, 0xe2, 0x21, 0x80, 0xc0, 0xc7]
        }
    }
    addr[5] |= 0xC0; // top two bits = 11 → Random Static
    addr
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("Hello World! RMK SF32LB52x BLE example");
    let p = sifli_hal::init(sifli_hal::Config::default());

    sifli_hal::rcc::test_print_clocks();

    // Get BLE addr
    let ble_addr = ble_addr(p.EFUSEC);
    info!(
        "Using BLE address {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        ble_addr[5], ble_addr[4], ble_addr[3], ble_addr[2], ble_addr[1], ble_addr[0],
    );

    // Give probe-rs time to attach before touching the BLE stack.
    embassy_time::Timer::after_millis(500).await;

    // Enable MPI2 NOR while running from its XIP window.
    let blocking_flash = match BlockingNorFlash::new_blocking_without_reset(
        p.MPI2,
        ProfileSource::BuiltIn(BuiltInProfile::CommonSpiNor16MiB),
        NorConfig::default(),
    ) {
        Ok(f) => f,
        Err(e) => {
            error!("MPI2 NOR flash init failed: {:?}", e);
            loop {
                embassy_time::Timer::after_secs(1).await;
            }
        }
    };

    let async_flash = async_flash_wrapper(blocking_flash);

    let storage_config = StorageConfig {
        num_sectors: 16,
        ..Default::default()
    };
    let mut keymap_data = KeymapData::new_with_encoder(keymap::get_default_keymap(), keymap::get_default_encoder_map());
    let mut behavior_config = BehaviorConfig::default();
    let per_key_config = PositionalConfig::default();
    let (keymap, mut storage) = initialize_keymap_and_storage(
        &mut keymap_data,
        async_flash,
        &storage_config,
        &mut behavior_config,
        &per_key_config,
    )
    .await;

    // Initialize BLE controller (LCPU + IPC + HCI).
    let controller: BleController = match BleController::new(
        p.LCPU,
        p.MAILBOX1_CH1,
        p.DMAC2_CH8,
        Irqs,
        &BleInitConfig::default().pm_enabled(false),
    )
    .await
    {
        Ok(c) => c,
        Err(e) => {
            error!("BLE init failed: {:?}", e);
            loop {
                embassy_time::Timer::after_secs(1).await;
            }
        }
    };
    let mut rng = Rng::new_blocking(p.TRNG);
    let mut rng_gen = ChaCha12Rng::from_rng(&mut rng).unwrap();
    let mut host_resources = HostResources::new();
    let stack = build_ble_stack(controller, ble_addr, &mut rng_gen, &mut host_resources).await;

    // Initialize USB driver (dual-mode USB + BLE). PA35/PA36 are the USB D+/D- pins.
    let usb_driver = Driver::new(p.USBC, Irqs, p.PA35, p.PA36);

    // Pin config: SF32 SuperKey macro keyboard (https://github.com/SiFliSparks/SuperKey)
    // KEY1=PA26, KEY2=PA33, KEY3=PA32, KEY4=PA40 — active-low with pull-ups.
    let direct_pins = config_direct_pins_sifli!(peripherals: p, direct_pins: [[PA26, PA33, PA32, PA40]]);

    let keyboard_device_config = DeviceConfig {
        vid: 0x4c4b,
        pid: 0x4644,
        manufacturer: "RMK & SiFli-rs",
        product_name: "RMK SF32LB52",
        serial_number: "vial:f64c2b3c:000002",
    };

    let vial_config = VialConfig::new(VIAL_KEYBOARD_ID, VIAL_KEYBOARD_DEF, &[(0, 0), (0, 3)]);

    let rmk_config = RmkConfig {
        device_config: keyboard_device_config,
        vial_config,
        storage_config,
        ..Default::default()
    };

    let debouncer = DefaultDebouncer::new();
    let mut matrix = DirectPinMatrix::<_, _, ROW, COL, SIZE>::new(direct_pins, debouncer, true);
    let mut keyboard = Keyboard::new(&keymap);
    let mut host_service = HostService::new(&keymap, &rmk_config);

    // Rotary encoder: PA43 = phase A, PA41 = phase B. Detents short to GND, so pull-ups are required.
    // Resolution 4 collapses the 4 quadrature transitions per detent into a single event;
    // reverse = true matches the SuperKey's mechanical orientation.
    let encoder_a = Input::new(p.PA43, sifli_hal::gpio::Pull::Up);
    let encoder_b = Input::new(p.PA41, sifli_hal::gpio::Pull::Up);
    let mut encoder = RotaryEncoder::with_resolution(encoder_a, encoder_b, 4, true, 0).with_debounce(2);

    info!("Starting RMK dual-mode (USB + BLE) runner...");

    join4(
        run_all!(matrix, encoder, storage),
        keyboard.run(),
        host_service.run(),
        run_rmk(usb_driver, &stack, rmk_config),
    )
    .await;
}
