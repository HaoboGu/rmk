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
use rmk::debounce::default_debouncer::DefaultDebouncer;
use rmk::direct_pin::DirectPinMatrix;
use rmk::futures::future::join3;
use rmk::input_device::Runnable;
use rmk::keyboard::Keyboard;
use rmk::storage::async_flash_wrapper;
use rmk::{HostResources, KeymapData, initialize_keymap_and_storage, run_all, run_rmk};
use sifli_hal::bind_interrupts;
use sifli_hal::efuse::Efuse;
use sifli_hal::gpio::Input;
use sifli_hal::ipc;
use sifli_hal::mpi::{BlockingNorFlash, BuiltInProfile, NorConfig, ProfileSource};
use sifli_hal::pmu;
use sifli_hal::rng::Rng;
use sifli_radio::bluetooth::{BleController, BleInitConfig};
use vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};

bind_interrupts!(struct Irqs {
    MAILBOX2_CH1 => ipc::InterruptHandler;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("Hello World! RMK SF32LB52x BLE example");
    let p = sifli_hal::init(sifli_hal::Config::default());

    sifli_hal::rcc::test_print_clocks();

    // Load factory-programmed PMU trim values from eFUSE Bank1. Without this,
    // BUCK / LPSYS_LDO / VRET / PERI_LDO / AON_BG sit on chip defaults; the RF
    // front-end supplies then run slightly off-spec and every BLE HCI command
    // reports success while *zero* packets actually reach the air. This is the
    // Rust equivalent of the SDK's `BSP_System_Config` → `HAL_PMU_LoadCalData`
    // chain; sifli-hal exposes the write path (`pmu::apply_calibration`) but
    // does not auto-call it, so we do it here before any BLE bring-up.
    //
    // We also keep the Efuse handle so we can derive a *stable* BLE static
    // random address from the factory UID — required for reconnects to
    // succeed with the saved bond info. BLE hosts index bonds by the
    // remote identity address (our static random addr); if we randomized
    // it each boot the host would treat us as a fresh device every time
    // and ignore the bond / force re-pairing.
    let efuse_opt = match Efuse::new(p.EFUSEC) {
        Ok(efuse) => {
            let applied = pmu::apply_calibration(efuse.calibration());
            info!("PMU calibration applied from eFUSE: {}", applied);
            Some(efuse)
        }
        Err(e) => {
            error!("Efuse init failed, PMU trims left at defaults: {:?}", e);
            None
        }
    };

    // Match the sifli-rs ble_advertise example: give probe-rs time to attach
    // before touching the BLE stack.
    embassy_time::Timer::after_secs(1).await;

    // Bring up MPI2 NOR while running from its XIP window. `new_blocking_without_reset`
    // keeps the preconfigured timing/command set the bootloader left behind —
    // touching those would stall the code fetches we're doing right now. Async
    // flash is rejected on the same-instance XIP path (`AsyncForbiddenInXip`),
    // so we get a blocking handle and wrap it with `BlockingAsync` for the
    // sequential-storage API.
    //
    // IMPORTANT: do this *before* the BLE controller starts, so the LCPU isn't
    // flooding the HCPU with mailbox IRQs while the XIP-safe program path has
    // PRIMASK disabled. With LCPU idle, interrupt-latency budgets are loose
    // and the WIP-status polling completes reliably; once the LCPU is
    // heartbeating the very first PAGE_PROGRAM sometimes stalls forever.
    let mut blocking_flash = match BlockingNorFlash::new_blocking_without_reset(
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

    // Auto-seed: if the storage tail still holds factory/garbage bytes, wipe
    // it so `sequential-storage` sees a clean 0xFF range. Skip the wipe when
    // the region is already either erased (`0xFF`) or holds a valid RMK
    // PartialOpen page marker (`0x00` at offset 0) — that way user data
    // survives resets.
    const STORAGE_SECTOR_SIZE: u32 = 4096;
    const STORAGE_NUM_SECTORS: u32 = 2;
    const STORAGE_BYTES: u32 = STORAGE_SECTOR_SIZE * STORAGE_NUM_SECTORS;
    // 16 MiB flash, last `STORAGE_BYTES` reserved for RMK storage.
    const STORAGE_START: u32 = (16 * 1024 * 1024) - STORAGE_BYTES;
    const STORAGE_END: u32 = STORAGE_START + STORAGE_BYTES;
    let mut probe = [0u8; 1];
    if let Err(e) = blocking_flash.read(STORAGE_START, &mut probe) {
        error!("Probe read of storage head failed: {:?}", e);
    } else if probe[0] != 0x00 && probe[0] != 0xFF {
        info!(
            "Storage head byte = 0x{:02X}, pre-erasing 0x{:06X}..0x{:06X}",
            probe[0], STORAGE_START, STORAGE_END
        );
        if let Err(e) = blocking_flash.erase(STORAGE_START, STORAGE_END) {
            error!("Pre-erase of storage range failed: {:?}", e);
        } else {
            info!("Pre-erase of storage range OK");
        }
    }

    let async_flash = async_flash_wrapper(blocking_flash);

    let storage_config = StorageConfig {
        num_sectors: 16,
        ..Default::default()
    };
    let mut keymap_data = KeymapData::new(keymap::get_default_keymap());
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

    // 1. Initialize BLE controller (LCPU + IPC + HCI).
    // pm_enabled(false) keeps LCPU awake between events — required for reliable
    // connections when we cannot guarantee the LXT (32 kHz external crystal) is
    // present / accurate on this dev board. The SF32LB52-MOD-1-N16R8 module
    // does have an LXT stuffed, so leave enable_lxt at its default (true).
    let controller: BleController = match BleController::new(
        p.LCPU,
        p.MAILBOX1_CH1,
        p.DMAC2_CH8,
        Irqs,
        &BleInitConfig::default().pm_enabled(false),
    )
    .await
    {
        Ok(c) => {
            info!("BLE controller initialized");
            c
        }
        Err(e) => {
            error!("BLE init failed: {:?}", e);
            loop {
                embassy_time::Timer::after_secs(1).await;
            }
        }
    };

    // 2. Build BLE stack (trouble-host) with a static random address.
    // Bond-based reconnects require a *stable* identity address, so we derive
    // it from the factory eFUSE UID (guaranteed-unique per chip, never
    // changes). The top two bits must be `11` for BLE "Random Static".
    //
    // If eFUSE init failed earlier we fall back to TRNG — reconnects
    // won't work in that degraded case, but the device still advertises
    // as a fresh peripheral on every boot (same behaviour as before).
    let mut rng = Rng::new_blocking(p.TRNG);
    let mut rng_gen = ChaCha12Rng::from_rng(&mut rng).unwrap();
    let mut ble_addr = [0u8; 6];
    match &efuse_opt {
        Some(efuse) => {
            let uid = efuse.uid();
            let uid_bytes = uid.bytes();
            // First 6 bytes of UID → BD_ADDR (little-endian byte order
            // matches the HCI Set_Random_Address command).
            ble_addr.copy_from_slice(&uid_bytes[..6]);
        }
        None => {
            rand_core::RngCore::fill_bytes(&mut rng_gen, &mut ble_addr);
        }
    }
    ble_addr[5] |= 0xC0; // top two bits = 11 → Random Static
    info!(
        "Using BLE address {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        ble_addr[5], ble_addr[4], ble_addr[3], ble_addr[2], ble_addr[1], ble_addr[0],
    );
    let mut host_resources = HostResources::new();
    let stack = build_ble_stack(controller, ble_addr, &mut rng_gen, &mut host_resources).await;

    // Pin config: single direct-pin key on PA11 (KEY2 on the SF32LB52-DevKit-LCD
    // "黄山派"). The board has an on-board pull-down on this line, so we use
    // `Pull::None` internally and treat the key as active-high (press drives
    // PA11 to VCC).
    let direct_pins = config_direct_pins_sifli!(peripherals: p, direct_pins: [[PA11]]);

    // Keyboard config
    let keyboard_device_config = DeviceConfig {
        vid: 0x4c4b,
        pid: 0x4644,
        manufacturer: "RMK & SiFli-rs",
        // Keep product name short: 31-byte BLE legacy advertising limit has to
        // fit Flags (3B) + ServiceUUIDs 2x2B (6B) + Appearance (4B) + local name,
        // so name max ≈ 16 chars (including the 2B AD header).
        product_name: "RMK SF32 KB",
        serial_number: "vial:f64c2b3c:000002",
    };

    let vial_config = VialConfig::new(VIAL_KEYBOARD_ID, VIAL_KEYBOARD_DEF, &[(0, 0), (1, 1)]);

    let rmk_config = RmkConfig {
        device_config: keyboard_device_config,
        vial_config,
        storage_config,
        ..Default::default()
    };

    // Initialize the matrix + keyboard
    let debouncer = DefaultDebouncer::new();
    let mut matrix = DirectPinMatrix::<_, _, ROW, COL, SIZE>::new(direct_pins, debouncer, false);
    let mut keyboard = Keyboard::new(&keymap);

    info!("Starting RMK BLE runner...");

    // Start
    join3(
        run_all!(matrix),
        keyboard.run(),
        run_rmk(&keymap, &stack, &mut storage, rmk_config),
    )
    .await;
}
