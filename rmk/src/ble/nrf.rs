pub(crate) mod advertise;
mod battery_service;
pub(crate) mod bonder;
mod device_information_service;
mod hid_service;
pub(crate) mod server;
pub(crate) mod spec;

// TODO: Conditional imports should be compatible with more nRF chip models
use self::server::BleServer;
use crate::{
    ble::{
        ble_task,
        nrf::{
            advertise::{create_advertisement_data, SCAN_DATA},
            bonder::{BondInfo, Bonder},
            server::BleHidWriter,
        },
    },
    keyboard::{keyboard_task, Keyboard, KeyboardReportMessage},
    storage::{get_bond_info_key, Storage, StorageData},
    KeyAction, KeyMap, RmkConfig,
};
use core::{cell::RefCell, mem};
use defmt::*;
use embassy_executor::Spawner;
use embassy_futures::select::{select, select4, Either4};
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::{Channel, Receiver, Sender},
};
use embassy_time::Timer;
use embedded_hal::digital::{InputPin, OutputPin};
#[cfg(feature = "async_matrix")]
use embedded_hal_async::digital::Wait;
use embedded_storage_async::nor_flash::NorFlash as AsyncNorFlash;
use heapless::FnvIndexMap;
use nrf_softdevice::{
    ble::{gatt_server, peripheral, security::SecurityHandler as _, Connection},
    raw, Config, Flash, Softdevice,
};
use rmk_config::BleBatteryConfig;
use sequential_storage::{cache::NoCache, map::fetch_item};
use static_cell::StaticCell;
#[cfg(any(feature = "nrf52840_ble", feature = "nrf52833_ble"))]
use {
    crate::{
        run_usb_keyboard,
        usb::{wait_for_usb_configured, wait_for_usb_suspend, USB_DEVICE_ENABLED},
        KeyboardUsbDevice, LightService, VialService,
    },
    core::sync::atomic::Ordering,
    embassy_futures::select::Either,
    embassy_nrf::usb::vbus_detect::SoftwareVbusDetect,
    embassy_usb::driver::Driver,
    once_cell::sync::OnceCell,
};

/// Maximum number of bonded devices
pub const BONDED_DEVICE_NUM: usize = 8;

#[cfg(any(feature = "nrf52840_ble", feature = "nrf52833_ble"))]
/// Software Vbus detect when using BLE + USB
pub static SOFTWARE_VBUS: OnceCell<SoftwareVbusDetect> = OnceCell::new();

#[cfg(any(feature = "nrf52840_ble", feature = "nrf52833_ble"))]
/// Background task of nrf_softdevice
#[embassy_executor::task]
pub(crate) async fn softdevice_task(sd: &'static nrf_softdevice::Softdevice) -> ! {
    use nrf_softdevice::SocEvent;

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
            SocEvent::PowerUsbRemoved => software_vbus.detected(false),
            SocEvent::PowerUsbDetected => software_vbus.detected(true),
            SocEvent::PowerUsbPowerReady => software_vbus.ready(),
            _ => {}
        };
    })
    .await
}

// Some nRF BLE chips doesn't have USB, so the softdevice_task is different
#[cfg(any(
    feature = "nrf52832_ble",
    feature = "nrf52811_ble",
    feature = "nrf52810_ble"
))]
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
            attr_tab_size: raw::BLE_GATTS_ATTR_TAB_SIZE_DEFAULT,
        }),
        gap_role_count: Some(raw::ble_gap_cfg_role_count_t {
            adv_set_count: 1,
            periph_role_count: 3,
            central_role_count: 0,
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
pub async fn initialize_nrf_ble_keyboard_with_config_and_run<
    #[cfg(any(feature = "nrf52840_ble", feature = "nrf52833_ble"))] D: Driver<'static>,
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    Out: OutputPin,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    keymap: [[[KeyAction; COL]; ROW]; NUM_LAYER],
    #[cfg(feature = "col2row")] input_pins: [In; ROW],
    #[cfg(not(feature = "col2row"))] input_pins: [In; COL],
    #[cfg(feature = "col2row")] output_pins: [Out; COL],
    #[cfg(not(feature = "col2row"))] output_pins: [Out; ROW],
    #[cfg(any(feature = "nrf52840_ble", feature = "nrf52833_ble"))] usb_driver: Option<D>,
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
    let mut storage = Storage::new(flash, &keymap, keyboard_config.storage_config).await;
    let keymap = RefCell::new(
        KeyMap::<ROW, COL, NUM_LAYER>::new_from_storage(keymap, Some(&mut storage)).await,
    );

    // Get all saved bond info, config BLE bonder
    let mut buf: [u8; 128] = [0; 128];
    let mut bond_info: FnvIndexMap<u8, BondInfo, BONDED_DEVICE_NUM> = FnvIndexMap::new();
    for key in 0..BONDED_DEVICE_NUM {
        if let Ok(Some(StorageData::BondInfo(info))) =
            fetch_item::<u32, StorageData<ROW, COL, NUM_LAYER>, _>(
                &mut storage.flash,
                storage.storage_range.clone(),
                &mut NoCache::new(),
                &mut buf,
                get_bond_info_key(key as u8),
            )
            .await
        {
            bond_info.insert(key as u8, info).ok();
        }
    }
    info!("Loaded {} saved bond info", bond_info.len());
    static BONDER: StaticCell<Bonder> = StaticCell::new();
    let bonder = BONDER.init(Bonder::new(RefCell::new(bond_info)));

    let ble_server = unwrap!(BleServer::new(sd, keyboard_config.usb_config, bonder));

    // Keyboard services
    let mut keyboard = Keyboard::new(input_pins, output_pins, &keymap);
    #[cfg(any(feature = "nrf52840_ble", feature = "nrf52833_ble"))]
    let (mut usb_device, mut vial_service, mut light_service) = (
        usb_driver.map(|u| KeyboardUsbDevice::new(u, keyboard_config.usb_config)),
        VialService::new(&keymap, keyboard_config.vial_config),
        LightService::from_config(keyboard_config.light_config),
    );

    // BLE only, test power usage
    // usb_device = None;

    static keyboard_channel: Channel<CriticalSectionRawMutex, KeyboardReportMessage, 8> =
        Channel::new();
    let mut keyboard_report_sender = keyboard_channel.sender();
    let mut keyboard_report_receiver = keyboard_channel.receiver();

    // Main loop
    loop {
        // Init BLE advertising data
        let mut config = peripheral::Config::default();
        // Interval: 500ms
        config.interval = 800;
        let adv_data = create_advertisement_data(keyboard_name);
        let adv = peripheral::ConnectableAdvertisement::ScannableUndirected {
            adv_data: &adv_data,
            scan_data: &SCAN_DATA,
        };
        let adv_fut = peripheral::advertise_pairable(sd, adv, &config, bonder);

        // If there is a USB device, things become a little bit complex because we need to enable switching between USB and BLE.
        // Remember that USB ALWAYS has higher priority than BLE.
        #[cfg(any(feature = "nrf52840_ble", feature = "nrf52833_ble"))]
        if let Some(ref mut usb_device) = usb_device {
            // Check and run via USB first
            if USB_DEVICE_ENABLED.load(Ordering::SeqCst) {
                let usb_fut = run_usb_keyboard(
                    usb_device,
                    &mut keyboard,
                    &mut storage,
                    &mut light_service,
                    &mut vial_service,
                    &mut keyboard_report_receiver,
                    &mut keyboard_report_sender,
                );
                info!("Running USB keyboard!");
                select(usb_fut, wait_for_usb_suspend()).await;
            }

            // Usb device have to be started to check if usb is configured
            let usb_fut = usb_device.device.run();
            let usb_configured = wait_for_usb_configured();
            info!("USB suspended, BLE Advertising");

            // Wait for BLE or USB connection
            match select(adv_fut, select(usb_fut, usb_configured)).await {
                Either::First(re) => match re {
                    Ok(conn) => {
                        info!("Connected to BLE");
                        bonder.load_sys_attrs(&conn);
                        let usb_configured = wait_for_usb_configured();
                        let usb_fut = usb_device.device.run();
                        // TODO: enable light service(and vial service) in ble mode
                        match select(
                            run_ble_keyboard(
                                &conn,
                                &ble_server,
                                &mut keyboard,
                                &mut storage,
                                &mut keyboard_config.ble_battery_config,
                                &mut keyboard_report_receiver,
                                &mut keyboard_report_sender,
                            ),
                            select(usb_fut, usb_configured),
                        )
                        .await
                        {
                            Either::First(_) => (),
                            Either::Second(_) => {
                                info!("Detected USB configured, quit BLE");
                                continue;
                            }
                        }
                    }
                    Err(e) => error!("Advertise error: {}", e),
                },
                Either::Second(_) => {
                    // Wait 10ms for usb resuming
                    Timer::after_millis(10).await;
                    continue;
                }
            }
        } else {
            // If no USB device, just start BLE advertising
            info!("No USB, Start BLE advertising!");
            match adv_fut.await {
                Ok(conn) => {
                    bonder.load_sys_attrs(&conn);
                    run_ble_keyboard(
                        &conn,
                        &ble_server,
                        &mut keyboard,
                        &mut storage,
                        &mut keyboard_config.ble_battery_config,
                        &mut keyboard_report_receiver,
                        &mut keyboard_report_sender,
                    )
                    .await
                }
                Err(e) => error!("Advertise error: {}", e),
            }
        }

        #[cfg(any(
            feature = "nrf52832_ble",
            feature = "nrf52811_ble",
            feature = "nrf52810_ble"
        ))]
        match adv_fut.await {
            Ok(conn) => {
                bonder.load_sys_attrs(&conn);
                run_ble_keyboard(
                    &conn,
                    &ble_server,
                    &mut keyboard,
                    &mut storage,
                    &mut keyboard_config.ble_battery_config,
                    &mut keyboard_report_receiver,
                    &mut keyboard_report_sender,
                )
                .await
            }
            Err(e) => error!("Advertise error: {}", e),
        }
        // Retry after 3 second
        Timer::after_millis(100).await;
    }
}

// Run ble keyboard task for once
async fn run_ble_keyboard<
    'a,
    'b,
    F: AsyncNorFlash,
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    Out: OutputPin,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    conn: &Connection,
    ble_server: &BleServer,
    keyboard: &mut Keyboard<'a, In, Out, ROW, COL, NUM_LAYER>,
    storage: &mut Storage<F>,
    battery_config: &mut BleBatteryConfig<'b>,
    keyboard_report_receiver: &mut Receiver<'a, CriticalSectionRawMutex, KeyboardReportMessage, 8>,
    keyboard_report_sender: &mut Sender<'a, CriticalSectionRawMutex, KeyboardReportMessage, 8>,
) {
    info!("Starting GATT server 20 ms later");
    Timer::after_millis(20).await;
    let mut ble_keyboard_writer = BleHidWriter::<'_, 8>::new(&conn, ble_server.hid.input_keyboard);
    let mut ble_media_writer = BleHidWriter::<'_, 2>::new(&conn, ble_server.hid.input_media_keys);
    let mut ble_system_control_writer =
        BleHidWriter::<'_, 1>::new(&conn, ble_server.hid.input_system_keys);
    let mut ble_mouse_writer = BleHidWriter::<'_, 5>::new(&conn, ble_server.hid.input_mouse_keys);
    let mut bas = ble_server.bas;
    let battery_fut = bas.run(battery_config, &conn);

    // Run the GATT server on the connection. This returns when the connection gets disconnected.
    let ble_fut = gatt_server::run(&conn, ble_server, |_| {});
    let keyboard_fut = keyboard_task(keyboard, keyboard_report_sender);
    let ble_task = ble_task(
        keyboard_report_receiver,
        &mut ble_keyboard_writer,
        &mut ble_media_writer,
        &mut ble_system_control_writer,
        &mut ble_mouse_writer,
    );
    let storage_fut = storage.run::<ROW, COL, NUM_LAYER>();

    // Exit if anyone of three futures exits
    match select4(
        ble_fut,
        select(ble_task, keyboard_fut),
        battery_fut,
        storage_fut,
    )
    .await
    {
        Either4::First(disconnected_error) => error!(
            "BLE gatt_server run exited with error: {:?}",
            disconnected_error
        ),
        Either4::Second(_) => error!("Keyboard task exited"),
        Either4::Third(_) => error!("Battery task exited"),
        Either4::Fourth(_) => error!("Storage task exited"),
    }
}
