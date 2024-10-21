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
use crate::keyboard::keyboard_report_channel;
use crate::matrix::MatrixTrait;
use crate::storage::StorageKeys;
use crate::KEYBOARD_STATE;
use crate::BackgroundTask;
use crate::{
    ble::{
        ble_communication_task,
        nrf::{
            advertise::{create_advertisement_data, SCAN_DATA},
            bonder::BondInfo,
            server::BleHidWriter,
        },
    },
    keyboard::{Keyboard, KeyboardReportMessage},
    light::led_service_task,
    storage::{get_bond_info_key, Storage, StorageData},
    vial_task, KeyAction, KeyMap, LightService, RmkConfig, VialService,
};
use bonder::MultiBonder;
use core::sync::atomic::{AtomicU8, Ordering};
use core::{cell::RefCell, mem};
use defmt::*;
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_futures::select::{select, select4, Either4};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Receiver};
use embassy_time::Timer;
use embedded_hal::digital::OutputPin;
use embedded_storage_async::nor_flash::NorFlash as AsyncNorFlash;
use heapless::FnvIndexMap;
use nrf_softdevice::ble::peripheral::ConnectableAdvertisement;
use nrf_softdevice::raw::sd_ble_gap_conn_param_update;
use nrf_softdevice::{
    ble::{gatt_server, peripheral, security::SecurityHandler as _, Connection},
    raw, Config, Flash, Softdevice,
};
use profile::update_profile;
use rmk_config::BleBatteryConfig;
use sequential_storage::{cache::NoCache, map::fetch_item};
use static_cell::StaticCell;
use vial_service::VialReaderWriter;
#[cfg(not(feature = "_no_usb"))]
use {
    crate::{
        run_usb_keyboard,
        usb::{wait_for_usb_enabled, wait_for_usb_suspend, USB_DEVICE_ENABLED},
        KeyboardUsbDevice,
    },
    embassy_futures::select::{select3, Either3},
    embassy_nrf::usb::vbus_detect::SoftwareVbusDetect,
    embassy_usb::driver::Driver,
    once_cell::sync::OnceCell,
};

/// Maximum number of bonded devices
// TODO: make it configurable
pub const BONDED_DEVICE_NUM: usize = 8;
pub static ACTIVE_PROFILE: AtomicU8 = AtomicU8::new(0);

#[cfg(not(feature = "_no_usb"))]
/// Software Vbus detect when using BLE + USB
pub static SOFTWARE_VBUS: OnceCell<SoftwareVbusDetect> = OnceCell::new();

#[cfg(not(feature = "_no_usb"))]
/// Background task of nrf_softdevice
#[embassy_executor::task]
pub(crate) async fn softdevice_task(sd: &'static nrf_softdevice::Softdevice) -> ! {
    use nrf_softdevice::SocEvent;

    use crate::usb::USB_SUSPENDED;

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

    sd.run_with_callback(|event: SocEvent| {
        match event {
            SocEvent::PowerUsbRemoved => {
                software_vbus.detected(false);
                USB_SUSPENDED.store(true, Ordering::Release);
                USB_DEVICE_ENABLED.store(false, Ordering::Release);
            }
            SocEvent::PowerUsbDetected => {
                software_vbus.detected(true);
                USB_SUSPENDED.store(false, Ordering::Release);
                USB_DEVICE_ENABLED.store(true, Ordering::Release);
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
            central_role_count: 4,
            central_sec_count: 0,
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
        ..Default::default()
    }
}

/// Initialize and run the BLE keyboard service, with given keyboard usb config.
/// Can only be used on nrf52 series microcontrollers with `nrf-softdevice` crate.
/// This function never returns.
///
/// # Arguments
///
/// * `keymap` - default keymap definition
/// * `driver` - embassy usb driver instance
/// * `input_pins` - input gpio pins
/// * `output_pins` - output gpio pins
/// * `keyboard_config` - other configurations of the keyboard, check [RmkConfig] struct for details
/// * `spwaner` - embassy task spwaner, used to spawn nrf_softdevice background task
/// * `saadc` - nRF's [saadc](https://infocenter.nordicsemi.com/index.jsp?topic=%2Fcom.nordic.infocenter.nrf52832.ps.v1.1%2Fsaadc.html) instance for battery level detection, if you don't need it, pass `None`
pub(crate) async fn initialize_nrf_ble_keyboard_with_config_and_run<
    M: MatrixTrait,
    Out: OutputPin,
    #[cfg(not(feature = "_no_usb"))] D: Driver<'static>,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    mut matrix: M,
    #[cfg(not(feature = "_no_usb"))] usb_driver: D,
    default_keymap: &mut [[[KeyAction; COL]; ROW]; NUM_LAYER],
    mut keyboard_config: RmkConfig<'static, Out>,
    spawner: Spawner,
) -> ! {
    // Set ble config and start nrf-softdevice background task first
    let keyboard_name = keyboard_config.usb_config.product_name;
    let ble_config = nrf_ble_config(keyboard_name);

    let sd = Softdevice::enable(&ble_config);
    {
        // Use the immutable ref of `Softdevice` to run the softdevice_task
        // The mumtable ref is used for configuring Flash and BleServer
        let sdv = unsafe { nrf_softdevice::Softdevice::steal() };
        unwrap!(spawner.spawn(softdevice_task(sdv)))
    };

    // Flash and keymap configuration
    let flash = Flash::take(sd);
    let mut storage = Storage::new(flash, &default_keymap, keyboard_config.storage_config).await;
    let keymap = RefCell::new(
        KeyMap::<ROW, COL, NUM_LAYER>::new_from_storage(default_keymap, Some(&mut storage)).await,
    );

    let mut buf: [u8; 128] = [0; 128];
    // Read current active profil
    if let Ok(Some(StorageData::ActiveBleProfile(profile))) =
        fetch_item::<u32, StorageData<ROW, COL, NUM_LAYER>, _>(
            &mut storage.flash,
            storage.storage_range.clone(),
            &mut NoCache::new(),
            &mut buf,
            &(StorageKeys::ActiveBleProfile as u32),
        )
        .await
    {
        ACTIVE_PROFILE.store(profile, Ordering::SeqCst);
    } else {
        // If no saved active profile, use 0 as default
        ACTIVE_PROFILE.store(0, Ordering::SeqCst);
    };

    // Get all saved bond info, config BLE bonder
    let mut bond_info: FnvIndexMap<u8, BondInfo, BONDED_DEVICE_NUM> = FnvIndexMap::new();
    for key in 0..BONDED_DEVICE_NUM {
        if let Ok(Some(StorageData::BondInfo(info))) =
            fetch_item::<u32, StorageData<ROW, COL, NUM_LAYER>, _>(
                &mut storage.flash,
                storage.storage_range.clone(),
                &mut NoCache::new(),
                &mut buf,
                &get_bond_info_key(key as u8),
            )
            .await
        {
            bond_info.insert(key as u8, info).ok();
        }
    }

    info!("Loaded {} saved bond info", bond_info.len());
    // static BONDER: StaticCell<Bonder> = StaticCell::new();
    // let bonder = BONDER.init(Bonder::new(RefCell::new(bond_info)));
    static BONDER: StaticCell<MultiBonder> = StaticCell::new();
    let bonder = BONDER.init(MultiBonder::new(RefCell::new(bond_info)));

    let ble_server = unwrap!(BleServer::new(sd, keyboard_config.usb_config, bonder));

    let keyboard_report_sender = keyboard_report_channel.sender();
    let keyboard_report_receiver = keyboard_report_channel.receiver();

    // Keyboard services
    let mut keyboard = Keyboard::new(&keymap, &keyboard_report_sender);
    #[cfg(not(feature = "_no_usb"))]
    let mut usb_device = KeyboardUsbDevice::new(usb_driver, keyboard_config.usb_config);
    let mut vial_service = VialService::new(&keymap, keyboard_config.vial_config);
    let mut light_service = LightService::from_config(keyboard_config.light_config);

    // Main loop
    loop {
        KEYBOARD_STATE.store(false, core::sync::atomic::Ordering::Release);
        // Init BLE advertising data
        let mut config = peripheral::Config::default();
        // Interval: 500ms
        config.interval = 800;
        let adv = ConnectableAdvertisement::ScannableUndirected {
            adv_data: &create_advertisement_data(keyboard_name),
            scan_data: &SCAN_DATA,
        };
        let adv_fut = peripheral::advertise_pairable(sd, adv, &config, bonder);

        // If there is a USB device, things become a little bit complex because we need to enable switching between USB and BLE.
        // Remember that USB ALWAYS has higher priority than BLE.
        #[cfg(not(feature = "_no_usb"))]
        {
            // Check and run via USB first
            if USB_DEVICE_ENABLED.load(Ordering::SeqCst) {
                // Run usb keyboard
                let usb_fut = run_usb_keyboard(
                    &mut usb_device,
                    &mut keyboard,
                    &mut matrix,
                    &mut storage,
                    &mut light_service,
                    &mut vial_service,
                    &keyboard_report_receiver,
                );
                info!("Running USB keyboard!");
                select3(usb_fut, wait_for_usb_suspend(), update_profile(bonder)).await;
            }

            // Usb device have to be started to check if usb is configured
            let active_profile = ACTIVE_PROFILE.load(Ordering::SeqCst);
            info!(
                "USB suspended, BLE Advertising on profile: {}",
                active_profile
            );

            // Wait for BLE or USB connection
            let dummy_task = run_dummy_keyboard(
                &mut keyboard,
                &mut matrix,
                &mut storage,
                &keyboard_report_receiver,
            );
            match select3(
                adv_fut,
                wait_for_usb_enabled(),
                select(dummy_task, update_profile(bonder)),
            )
            .await
            {
                Either3::First(re) => match re {
                    Ok(conn) => {
                        info!("Connected to BLE");
                        // Check whether the peer address is matched with current profile
                        if !bonder.check_connection(&conn) {
                            error!("Bonded peer address doesn't match active profile, disconnect");
                            continue;
                        }
                        bonder.load_sys_attrs(&conn);
                        match select3(
                            run_ble_keyboard(
                                &conn,
                                &ble_server,
                                &mut keyboard,
                                &mut matrix,
                                &mut storage,
                                &mut light_service,
                                &mut vial_service,
                                &mut keyboard_config.ble_battery_config,
                                &keyboard_report_receiver,
                            ),
                            wait_for_usb_enabled(),
                            update_profile(bonder),
                        )
                        .await
                        {
                            Either3::First(_) => (),
                            Either3::Second(_) => {
                                info!("Detected USB configured, quit BLE");
                                continue;
                            }
                            Either3::Third(_) => {
                                info!("Switch profile");
                                continue;
                            }
                        }
                    }
                    Err(e) => error!("Advertise error: {}", e),
                },
                Either3::Second(_) => {
                    // Wait 10ms for usb resuming
                    Timer::after_millis(10).await;
                    continue;
                }
                Either3::Third(_) => {
                    // Switch profile
                    Timer::after_millis(10).await;
                    continue;
                }
            }
        }

        #[cfg(feature = "_no_usb")]
        match adv_fut.await {
            Ok(conn) => {
                bonder.load_sys_attrs(&conn);
                select(
                    run_ble_keyboard(
                        &conn,
                        &ble_server,
                        &mut keyboard,
                        &mut matrix,
                        &mut storage,
                        &mut light_service,
                        &mut vial_service,
                        &mut keyboard_config.ble_battery_config,
                        &keyboard_report_receiver,
                    ),
                    update_profile(bonder),
                )
                .await;
            }
            Err(e) => error!("Advertise error: {}", e),
        }

        // Retry after 1 second
        Timer::after_secs(1).await;
    }
}

async fn set_conn_params(conn: &Connection) {
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

            embassy_time::Timer::after_millis(50).await;

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
// TODO: make matrix + keyboard + storage task running in the background ALWAYS,
// add a dummy receiver preventing everything blocks
pub(crate) async fn run_dummy_keyboard<
    'a,
    'b,
    M: MatrixTrait,
    F: AsyncNorFlash,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    keyboard: &mut Keyboard<'a, ROW, COL, NUM_LAYER>,
    matrix: &mut M,
    storage: &mut Storage<F, ROW, COL, NUM_LAYER>,
    keyboard_report_receiver: &Receiver<'a, CriticalSectionRawMutex, KeyboardReportMessage, 8>,
) {
    let matrix_fut = matrix.scan();
    let keyboard_fut = keyboard.run();
    let storage_fut = storage.run();
    let dummy_communication = async {
        loop {
            keyboard_report_receiver.receive().await;
            warn!("Dummy service receives")
        }
    };

    match select4(matrix_fut, keyboard_fut, storage_fut, dummy_communication).await {
        Either4::First(_) => (),
        Either4::Second(_) => (),
        Either4::Third(_) => (),
        Either4::Fourth(_) => (),
    }
}

// Run ble keyboard task for once
pub(crate) async fn run_ble_keyboard<
    'a,
    'b,
    M: MatrixTrait,
    F: AsyncNorFlash,
    Out: OutputPin,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    conn: &Connection,
    ble_server: &BleServer,
    keyboard: &mut Keyboard<'a, ROW, COL, NUM_LAYER>,
    matrix: &mut M,
    storage: &mut Storage<F, ROW, COL, NUM_LAYER>,
    light_service: &mut LightService<Out>,
    vial_service: &mut VialService<'a, ROW, COL, NUM_LAYER>,
    battery_config: &mut BleBatteryConfig<'b>,
    keyboard_report_receiver: &Receiver<'a, CriticalSectionRawMutex, KeyboardReportMessage, 8>,
) {
    info!("Starting GATT server 20 ms later");
    Timer::after_millis(20).await;
    let mut ble_keyboard_writer = BleHidWriter::<'_, 8>::new(&conn, ble_server.hid.input_keyboard);
    let mut ble_media_writer = BleHidWriter::<'_, 2>::new(&conn, ble_server.hid.input_media_keys);
    let mut ble_system_control_writer =
        BleHidWriter::<'_, 1>::new(&conn, ble_server.hid.input_system_keys);
    let mut ble_mouse_writer = BleHidWriter::<'_, 5>::new(&conn, ble_server.hid.input_mouse_keys);
    let mut bas = ble_server.bas;
    let mut vial_rw = VialReaderWriter::new(ble_server.vial, &conn);
    let vial_task = vial_task(&mut vial_rw, vial_service);

    // Tasks
    let battery_fut = bas.run(battery_config, &conn);
    let led_fut = led_service_task(light_service);
    let matrix_fut = matrix.scan();
    // Run the GATT server on the connection. This returns when the connection gets disconnected.
    let ble_fut = gatt_server::run(&conn, ble_server, |_| {});
    let keyboard_fut = keyboard.run();
    let ble_communication_task = ble_communication_task(
        keyboard_report_receiver,
        &mut ble_keyboard_writer,
        &mut ble_media_writer,
        &mut ble_system_control_writer,
        &mut ble_mouse_writer,
    );
    let storage_fut = storage.run();
    let set_conn_param = set_conn_params(&conn);

    // Exit if anyone of three futures exits
    match select4(
        select(matrix_fut, join(ble_fut, set_conn_param)),
        select(ble_communication_task, keyboard_fut),
        select(battery_fut, led_fut),
        select(vial_task, storage_fut),
    )
    .await
    {
        Either4::First(disconnected_error) => error!(
            "BLE gatt_server run exited with error: {:?}",
            disconnected_error
        ),
        Either4::Second(_) => error!("Keyboard task or ble task exited"),
        Either4::Third(_) => error!("Battery task or led task exited"),
        Either4::Fourth(_) => error!("Storage task exited"),
    }
}
