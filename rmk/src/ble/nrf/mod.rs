pub(crate) mod advertise;
mod battery_service;
pub(crate) mod bonder;
mod device_information_service;
mod hid_service;
pub(crate) mod profile;
pub(crate) mod server;
pub(crate) mod spec;
mod vial_service;

use self::server::BleServer;
use crate::ble::led::BleLedReader;
use crate::ble::nrf::hid_service::BleKeyboardWriter;
use crate::config::{BleBatteryConfig, RmkConfig, VialConfig};
use crate::hid::{DummyWriter, RunnableHidWriter};
use crate::keymap::KeyMap;
use crate::light::LightController;
use crate::run_keyboard;
use crate::storage::StorageKeys;
use crate::{
    ble::nrf::bonder::BondInfo,
    storage::{get_bond_info_key, Storage, StorageData},
    CONNECTION_STATE, CONNECTION_TYPE,
};
use advertise::{create_advertisement_data, SCAN_DATA};
use bonder::MultiBonder;
use core::sync::atomic::{AtomicU8, Ordering};
use core::{cell::RefCell, mem};
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_futures::select::{select, select4, Either4};
use embassy_time::Timer;
use embedded_hal::digital::OutputPin;
use embedded_storage_async::nor_flash::NorFlash as AsyncNorFlash;
use heapless::FnvIndexMap;
use nrf_softdevice::ble::peripheral::ConnectableAdvertisement;
use nrf_softdevice::ble::{PhySet, PhyUpdateError, TxPower};
use nrf_softdevice::raw::sd_ble_gap_conn_param_update;
use nrf_softdevice::{
    ble::{gatt_server, peripheral, security::SecurityHandler as _, Connection},
    raw, Config, Flash, Softdevice,
};
use profile::update_profile;
use sequential_storage::{cache::NoCache, map::fetch_item};
use static_cell::StaticCell;
use vial_service::BleVialReaderWriter;
#[cfg(not(feature = "_no_usb"))]
use {
    crate::light::UsbLedReader,
    crate::register_usb_writer,
    crate::usb::{
        descriptor::{CompositeReport, KeyboardReport, ViaReport},
        new_usb_builder, wait_for_usb_enabled, wait_for_usb_suspend, UsbKeyboardWriter, UsbState,
        USB_STATE,
    },
    crate::via::UsbVialReaderWriter,
    crate::{add_usb_reader_writer, run_usb_device},
    embassy_futures::select::{select3, Either3},
    embassy_nrf::usb::vbus_detect::SoftwareVbusDetect,
    embassy_usb::driver::Driver,
    once_cell::sync::OnceCell,
};

/// Maximum number of bonded devices
pub const BONDED_DEVICE_NUM: usize = 8;
/// The number of the active profile
pub static ACTIVE_PROFILE: AtomicU8 = AtomicU8::new(0);

#[cfg(not(feature = "_no_usb"))]
/// Software Vbus detect when using BLE + USB
pub static SOFTWARE_VBUS: OnceCell<SoftwareVbusDetect> = OnceCell::new();

#[cfg(not(feature = "_no_usb"))]
/// Background task of nrf_softdevice
#[embassy_executor::task]
pub(crate) async fn softdevice_task(sd: &'static nrf_softdevice::Softdevice) -> ! {
    use nrf_softdevice::SocEvent;

    use crate::usb::{UsbState, USB_STATE};

    // Enable dcdc-mode, reduce power consumption
    unsafe {
        nrf_softdevice::raw::sd_power_dcdc_mode_set(
            nrf_softdevice::raw::NRF_POWER_DCDC_MODES_NRF_POWER_DCDC_ENABLE as u8,
        );
        nrf_softdevice::raw::sd_power_dcdc0_mode_set(
            nrf_softdevice::raw::NRF_POWER_DCDC_MODES_NRF_POWER_DCDC_ENABLE as u8,
        );
    };

    // Enable USB event in softdevice
    unsafe {
        nrf_softdevice::raw::sd_power_usbpwrrdy_enable(1);
        nrf_softdevice::raw::sd_power_usbdetected_enable(1);
        nrf_softdevice::raw::sd_power_usbremoved_enable(1);
    };

    let software_vbus = SOFTWARE_VBUS.get_or_init(|| SoftwareVbusDetect::new(true, true));

    // Read the USB status at the beginning
    let mut usb_reg: u32 = 0;
    unsafe { raw::sd_power_usbregstatus_get(&mut usb_reg) };
    if usb_reg & 1 == 1 {
        software_vbus.detected(true);
        USB_STATE.store(UsbState::Enabled as u8, Ordering::Relaxed);
    }

    sd.run_with_callback(|event: SocEvent| {
        match event {
            SocEvent::PowerUsbRemoved => {
                software_vbus.detected(false);
                USB_STATE.store(UsbState::Disabled as u8, Ordering::Relaxed);
            }
            SocEvent::PowerUsbDetected => {
                software_vbus.detected(true);
                USB_STATE.store(UsbState::Enabled as u8, Ordering::Relaxed);
            }
            SocEvent::PowerUsbPowerReady => software_vbus.ready(),
            _ => {}
        };
    })
    .await
}

// Some nRF BLE chips doesn't have USB, so the softdevice_task is different
#[cfg(feature = "_no_usb")]
#[embassy_executor::task]
pub(crate) async fn softdevice_task(sd: &'static nrf_softdevice::Softdevice) -> ! {
    // Enable dcdc-mode, reduce power consumption
    unsafe {
        nrf_softdevice::raw::sd_power_dcdc_mode_set(
            nrf_softdevice::raw::NRF_POWER_DCDC_MODES_NRF_POWER_DCDC_ENABLE as u8,
        );
    };
    sd.run().await
}

/// Helper macro for reading storage config
macro_rules! read_storage {
    ($storage: ident, $key: expr, $buf: expr) => {
        fetch_item::<u32, StorageData, _>(
            &mut $storage.flash,
            $storage.storage_range.clone(),
            &mut NoCache::new(),
            &mut $buf,
            $key,
        )
        .await
    };
}

/// Create default nrf ble config
pub(crate) fn nrf_ble_config(keyboard_name: &str) -> Config {
    Config {
        clock: Some(raw::nrf_clock_lf_cfg_t {
            source: raw::NRF_CLOCK_LF_SRC_RC as u8,
            rc_ctiv: 16,
            rc_temp_ctiv: 2,
            accuracy: raw::NRF_CLOCK_LF_ACCURACY_500_PPM as u8,
            // External osc
            // source: raw::NRF_CLOCK_LF_SRC_XTAL as u8,
            // rc_ctiv: 0,
            // rc_temp_ctiv: 0,
            // accuracy: raw::NRF_CLOCK_LF_ACCURACY_20_PPM as u8,
        }),
        conn_gap: Some(raw::ble_gap_conn_cfg_t {
            conn_count: 6,
            event_length: 24,
        }),
        conn_gatt: Some(raw::ble_gatt_conn_cfg_t { att_mtu: 256 }),
        gatts_attr_tab_size: Some(raw::ble_gatts_cfg_attr_tab_size_t {
            attr_tab_size: 2048,
        }),
        gap_role_count: Some(raw::ble_gap_cfg_role_count_t {
            adv_set_count: 1,
            periph_role_count: 4,
            #[cfg(not(any(feature = "nrf52810_ble", feature = "nrf52811_ble")))]
            central_role_count: 4,
            #[cfg(not(any(feature = "nrf52810_ble", feature = "nrf52811_ble")))]
            central_sec_count: 2,
            #[cfg(not(any(feature = "nrf52810_ble", feature = "nrf52811_ble")))]
            _bitfield_1: raw::ble_gap_cfg_role_count_t::new_bitfield_1(0),
        }),
        gap_device_name: Some(raw::ble_gap_cfg_device_name_t {
            p_value: keyboard_name.as_ptr() as _,
            current_len: keyboard_name.len() as u16,
            max_len: keyboard_name.len() as u16,
            write_perm: unsafe { mem::zeroed() },
            _bitfield_1: raw::ble_gap_cfg_device_name_t::new_bitfield_1(
                raw::BLE_GATTS_VLOC_STACK as u8,
            ),
        }),
        conn_gattc: Some(raw::ble_gattc_conn_cfg_t {
            write_cmd_tx_queue_size: 4,
        }),
        conn_gatts: Some(raw::ble_gatts_conn_cfg_t {
            hvn_tx_queue_size: 4,
        }),
        ..Default::default()
    }
}

pub fn initialize_nrf_sd_and_flash(
    keyboard_name: &str,
    spawner: Spawner,
    ble_addr: Option<[u8; 6]>,
) -> (&mut Softdevice, Flash) {
    // Set ble config and enable nrf-softdevice first
    let ble_config = nrf_ble_config(keyboard_name);

    let sd = Softdevice::enable(&ble_config);

    if let Some(addr) = ble_addr {
        // This is used mainly for split
        use nrf_softdevice::ble::{set_address, Address, AddressType};
        set_address(sd, &Address::new(AddressType::RandomStatic, addr));
    };

    // Use the immutable ref of `Softdevice` to run the softdevice_task
    // It can also be used for create flash instance
    let sdv = unsafe { nrf_softdevice::Softdevice::steal() };
    spawner
        .spawn(softdevice_task(sdv))
        .expect("Failed to start softdevice task");
    (sd, Flash::take(sdv))
}

pub(crate) async fn run_nrf_ble_keyboard<
    'a,
    F: AsyncNorFlash,
    #[cfg(not(feature = "_no_usb"))] D: Driver<'static>,
    Out: OutputPin,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
    const NUM_ENCODER: usize,
>(
    keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
    storage: &mut Storage<F, ROW, COL, NUM_LAYER, NUM_ENCODER>,
    #[cfg(not(feature = "_no_usb"))] usb_driver: D,
    light_controller: &mut LightController<Out>,
    mut rmk_config: RmkConfig<'static>,
    sd: &mut Softdevice,
) -> ! {
    // Initialize usb device, ble service, etc
    #[cfg(not(feature = "_no_usb"))]
    // Initialize usb device and usb hid reader/writer
    let (
        mut usb_device,
        mut keyboard_reader,
        mut keyboard_writer,
        mut other_writer,
        mut vial_reader_writer,
    ) = {
        let mut usb_builder: embassy_usb::Builder<'_, D> =
            new_usb_builder(usb_driver, rmk_config.usb_config);
        let keyboard_reader_writer = add_usb_reader_writer!(&mut usb_builder, KeyboardReport, 1, 8);
        let other_writer = register_usb_writer!(&mut usb_builder, CompositeReport, 9);
        let vial_reader_writer = add_usb_reader_writer!(&mut usb_builder, ViaReport, 32, 32);
        let (keyboard_reader, keyboard_writer) = keyboard_reader_writer.split();
        let usb_device = usb_builder.build();
        (
            usb_device,
            keyboard_reader,
            keyboard_writer,
            other_writer,
            vial_reader_writer,
        )
    };

    // Initialize ble service
    load_keyboard_states(storage).await;
    let bond_info = load_bond_info(storage).await;
    info!("Loaded {} saved bond info", bond_info.len());
    static BONDER: StaticCell<MultiBonder> = StaticCell::new();
    let bonder = BONDER.init(MultiBonder::new(RefCell::new(bond_info)));
    let ble_server: BleServer =
        BleServer::new(sd, rmk_config.usb_config, bonder).expect("Failed to start ble server");

    // Main loop
    loop {
        // Init BLE advertising data
        let mut config = peripheral::Config::default();
        // Interval: 500ms
        config.interval = 800;
        config.tx_power = TxPower::Plus4dBm;
        let adv = ConnectableAdvertisement::ScannableUndirected {
            adv_data: &create_advertisement_data(rmk_config.usb_config.product_name),
            scan_data: &SCAN_DATA,
        };
        // If there is a USB device, things become a little bit complex because we need to enable switching between USB and BLE.
        // Remember that USB ALWAYS has higher priority than BLE.
        #[cfg(not(feature = "_no_usb"))]
        {
            debug!(
                "usb state: {}, connection type: {}",
                USB_STATE.load(Ordering::SeqCst),
                CONNECTION_TYPE.load(Ordering::Relaxed)
            );
            // Check whether the USB is connected
            if USB_STATE.load(Ordering::SeqCst) != UsbState::Disabled as u8 {
                let usb_fut = run_keyboard(
                    keymap,
                    #[cfg(any(feature = "_nrf_ble", not(feature = "_no_external_storage")))]
                    storage,
                    run_usb_device(&mut usb_device),
                    light_controller,
                    UsbLedReader::new(&mut keyboard_reader),
                    UsbVialReaderWriter::new(&mut vial_reader_writer),
                    UsbKeyboardWriter::new(&mut keyboard_writer, &mut other_writer),
                    rmk_config.vial_config,
                );
                if CONNECTION_TYPE.load(Ordering::Relaxed) == 0 {
                    info!("Running USB keyboard");
                    // USB is connected, connection_type is USB, then run USB keyboard
                    match select3(usb_fut, wait_for_usb_suspend(), update_profile(bonder)).await {
                        Either3::Third(_) => {
                            Timer::after_millis(10).await;
                            continue;
                        }
                        _ => (),
                    }
                } else {
                    // USB is connected, but connection type is BLE, try BLE while running USB keyboard
                    info!("Running USB keyboard, while advertising");
                    let adv_fut = peripheral::advertise_pairable(sd, adv, &config, bonder);
                    match select3(adv_fut, usb_fut, update_profile(bonder)).await {
                        Either3::First(Ok(conn)) => {
                            run_ble_keyboard(
                                keymap,
                                storage,
                                light_controller,
                                rmk_config.vial_config,
                                &mut rmk_config.ble_battery_config,
                                &ble_server,
                                bonder,
                                conn,
                            )
                            .await
                        }
                        _ => {
                            // Wait 10ms
                            Timer::after_millis(10).await;
                            continue;
                        }
                    }
                }
            } else {
                // USB isn't connected, wait for any of BLE/USB connection
                let dummy_task = run_dummy_keyboard(storage);
                let adv_fut = peripheral::advertise_pairable(sd, adv, &config, bonder);

                info!("BLE advertising");
                // Wait for BLE or USB connection
                match select3(adv_fut, wait_for_status_change(bonder), dummy_task).await {
                    Either3::First(Ok(conn)) => {
                        run_ble_keyboard(
                            keymap,
                            storage,
                            light_controller,
                            rmk_config.vial_config,
                            &mut rmk_config.ble_battery_config,
                            &ble_server,
                            bonder,
                            conn,
                        )
                        .await
                    }
                    _ => {
                        // Wait 10ms for usb resuming/switching profile/advertising error
                        Timer::after_millis(10).await;
                    }
                }
            }
        }

        #[cfg(feature = "_no_usb")]
        match peripheral::advertise_pairable(sd, adv, &config, bonder).await {
            Ok(conn) => {
                run_ble_keyboard(
                    keymap,
                    storage,
                    light_controller,
                    rmk_config.vial_config,
                    &mut rmk_config.ble_battery_config,
                    &ble_server,
                    bonder,
                    conn,
                )
                .await;
            }
            Err(e) => error!("Advertise error: {}", e),
        }

        // Retry after 200 ms
        Timer::after_millis(200).await;
    }
}

pub(crate) async fn load_keyboard_states<
    F: AsyncNorFlash,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
    const NUM_ENCODER: usize,
>(
    storage: &mut Storage<F, ROW, COL, NUM_LAYER, NUM_ENCODER>,
) {
    let mut buf: [u8; 128] = [0; 128];

    // Load current active profile
    if let Ok(Some(StorageData::ActiveBleProfile(profile))) =
        read_storage!(storage, &(StorageKeys::ActiveBleProfile as u32), buf)
    {
        debug!("Loaded active profile: {}", profile);
        ACTIVE_PROFILE.store(profile, Ordering::SeqCst);
    } else {
        // If no saved active profile, use 0 as default
        debug!("Loaded default active profile",);
        ACTIVE_PROFILE.store(0, Ordering::SeqCst);
    };

    // Load current connection type
    if let Ok(Some(StorageData::ConnectionType(conn_type))) =
        read_storage!(storage, &(StorageKeys::ConnectionType as u32), buf)
    {
        CONNECTION_TYPE.store(conn_type, Ordering::Relaxed);
    } else {
        // If no saved connection type, use 0 as default
        CONNECTION_TYPE.store(0, Ordering::Relaxed);
    };

    #[cfg(feature = "_no_usb")]
    CONNECTION_TYPE.store(0, Ordering::Relaxed);
}

pub(crate) async fn load_bond_info<
    F: AsyncNorFlash,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
    const NUM_ENCODER: usize,
>(
    storage: &mut Storage<F, ROW, COL, NUM_LAYER, NUM_ENCODER>,
) -> FnvIndexMap<u8, BondInfo, BONDED_DEVICE_NUM> {
    let mut buf: [u8; 128] = [0; 128];
    // Get all saved bond info, config BLE bonder
    let mut bond_info: FnvIndexMap<u8, BondInfo, BONDED_DEVICE_NUM> = FnvIndexMap::new();
    for key in 0..BONDED_DEVICE_NUM {
        if let Ok(Some(StorageData::BondInfo(info))) =
            read_storage!(storage, &get_bond_info_key(key as u8), buf)
        {
            bond_info.insert(key as u8, info).ok();
        }
    }
    bond_info
}

async fn run_ble_keyboard<
    'a,
    F: AsyncNorFlash,
    Out: OutputPin,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
    const NUM_ENCODER: usize,
>(
    keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
    storage: &mut Storage<F, ROW, COL, NUM_LAYER, NUM_ENCODER>,
    light_controller: &mut LightController<Out>,
    vial_config: VialConfig<'static>,
    ble_battery_config: &mut BleBatteryConfig<'static>,
    ble_server: &BleServer,
    bonder: &'static MultiBonder,
    mut conn: Connection,
) {
    info!("Connected to BLE");
    bonder.load_sys_attrs(&conn);
    if let Err(e) = conn.phy_update(PhySet::M2, PhySet::M2) {
        error!("Failed to update PHY");
        if let PhyUpdateError::Raw(re) = e {
            error!("Raw error code: {:?}", re);
        }
    }
    match select4(
        run_keyboard(
            keymap,
            storage,
            run_ble_server(&conn, ble_server),
            light_controller,
            BleLedReader {},
            BleVialReaderWriter::new(ble_server.vial, &conn),
            BleKeyboardWriter::new(
                &conn,
                ble_server.hid.input_keyboard,
                ble_server.hid.input_media_keys,
                ble_server.hid.input_system_keys,
                ble_server.hid.input_mouse_keys,
            ),
            vial_config,
        ),
        ble_server.bas.clone().run(ble_battery_config, &conn),
        wait_for_usb_enabled(),
        update_profile(bonder),
    )
    .await
    {
        Either4::First(_) => info!("BLE disconnected"),
        Either4::Second(_) => info!("Bas service error"),
        Either4::Third(_) => info!("Detected USB configured, quit BLE"),
        Either4::Fourth(_) => info!("Switch profile"),
    }
    bonder.save_sys_attrs(&conn);
}

pub(crate) async fn set_conn_params(conn: &Connection) {
    // Wait for 5 seconds before setting connection parameters to avoid connection drop
    embassy_time::Timer::after_secs(5).await;
    if let Some(conn_handle) = conn.handle() {
        // Update connection parameters
        unsafe {
            // For macOS/iOS(aka Apple devices), both interval should be set to 12
            let re = sd_ble_gap_conn_param_update(
                conn_handle,
                &raw::ble_gap_conn_params_t {
                    min_conn_interval: 12,
                    max_conn_interval: 12,
                    slave_latency: 99,
                    conn_sup_timeout: 500, // timeout: 5s
                },
            );
            debug!("Set conn params result: {:?}", re);

            embassy_time::Timer::after_millis(5000).await;

            // Setting the conn param the second time ensures that we have best performance on all platforms
            let re = sd_ble_gap_conn_param_update(
                conn_handle,
                &raw::ble_gap_conn_params_t {
                    min_conn_interval: 6,
                    max_conn_interval: 6,
                    slave_latency: 99,
                    conn_sup_timeout: 500, // timeout: 5s
                },
            );
            debug!("Set conn params result: {:?}", re);
        }
    }
}

// Dummy keyboard service is used to monitoring keys when there's no actual connection.
// It's useful for functions like switching active profiles when there's no connection.
pub(crate) async fn run_dummy_keyboard<
    'a,
    'b,
    F: AsyncNorFlash,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
    const NUM_ENCODER: usize,
>(
    storage: &mut Storage<F, ROW, COL, NUM_LAYER, NUM_ENCODER>,
) {
    CONNECTION_STATE.store(false, Ordering::Release);
    let storage_fut = storage.run();
    let mut dummy_writer = DummyWriter {};
    select(storage_fut, dummy_writer.run_writer()).await;
}

#[cfg(not(feature = "_no_usb"))]
// Wait for USB enabled or BLE state changed
pub(crate) async fn wait_for_status_change(bonder: &MultiBonder) {
    if CONNECTION_TYPE.load(Ordering::Relaxed) == 0 {
        // Connection type is USB, USB has higher priority
        select(wait_for_usb_enabled(), update_profile(bonder)).await;
    } else {
        // Connection type is BLE, so we don't consider USB
        update_profile(bonder).await;
    }
}

async fn run_ble_server(conn: &Connection, ble_server: &BleServer) {
    let err = join(
        set_conn_params(&conn),
        gatt_server::run(conn, ble_server, |_| {}),
    )
    .await;
    error!("BLE server exited with error: {:?}", err.1);
}

#[cfg(feature = "_no_usb")]
async fn wait_for_usb_enabled() {
    loop {
        core::future::pending::<()>().await;
    }
}
