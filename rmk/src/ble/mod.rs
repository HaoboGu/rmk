use core::sync::atomic::AtomicBool;

use bt_hci::cmd::le::{LeReadLocalSupportedFeatures, LeSetPhy};
use bt_hci::controller::{ControllerCmdAsync, ControllerCmdSync};
use embassy_futures::join::join;
use embassy_futures::select::{Either, Either3, select, select3};
use embassy_time::{Duration, Timer, with_timeout};
use rand_core::{CryptoRng, RngCore};
use rmk_types::ble::BleState;
use rmk_types::connection::ConnectionType;
use rmk_types::led_indicator::LedIndicator;
use trouble_host::prelude::appearance::human_interface_device::KEYBOARD;
use trouble_host::prelude::service::{BATTERY, HUMAN_INTERFACE_DEVICE};
use trouble_host::prelude::*;

use crate::ble::battery_service::BleBatteryServer;
use crate::ble::ble_server::{BleHidServer, Server};
use crate::ble::device_info::{PnPID, VidSource};
use crate::ble::led::BleLedReader;
#[cfg(feature = "passkey_entry")]
use crate::ble::passkey::{PasskeyInputState, next_gatt_event};
use crate::ble::profile::{ProfileInfo, ProfileManager, UPDATED_CCCD_TABLE, UPDATED_PROFILE};
use crate::channel::{BLE_REPORT_CHANNEL, LED_SIGNAL};
use crate::config::RmkConfig;
use crate::core_traits::Runnable;
use crate::event::SubscribableEvent;
use crate::hid::{HidWriterTrait, run_led_reader};
#[cfg(feature = "split")]
use crate::split::ble::central::CENTRAL_SLEEP;
use crate::state::set_ble_state;

pub(crate) mod battery_service;
pub(crate) mod ble_server;
pub(crate) mod device_info;
pub(crate) mod led;
#[cfg(feature = "_nrf_ble")]
pub(crate) mod nrf;
pub mod passkey;
pub(crate) mod profile;

/// Global state of sleep management
/// - `true`: Indicates central is sleeping
/// - `false`: Indicates central is awake
pub(crate) static SLEEPING_STATE: AtomicBool = AtomicBool::new(false);

/// Max number of connections
pub(crate) const CONNECTIONS_MAX: usize = crate::SPLIT_PERIPHERALS_NUM + 1;

/// Max number of L2CAP channels
pub(crate) const L2CAP_CHANNELS_MAX: usize = CONNECTIONS_MAX * 4; // Signal + att + smp + hid

/// Build the BLE stack.
pub async fn build_ble_stack<
    'a,
    C: Controller + ControllerCmdAsync<LeSetPhy>,
    P: PacketPool,
    RNG: RngCore + CryptoRng,
>(
    controller: C,
    host_address: [u8; 6],
    random_generator: &mut RNG,
    resources: &'a mut HostResources<C, P, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX>,
) -> Stack<'a, C, P> {
    // Initialize trouble host stack
    trouble_host::new(controller, resources)
        .set_random_address(Address::random(host_address))
        .set_random_generator_seed(random_generator)
        .build()
}

/// BLE transport runnable. Owns the trouble-host server and profile manager;
/// `run` joins the background `ble_task` runner with the advertise→connect→serve
/// loop and runs forever.
//
// Two lifetimes: `'b` is the borrow of the stack value, `'s` is the trouble-host
// `Stack`'s own resource lifetime. They are separated because `Stack<'s, _, _>`
// is invariant in `'s` and now has a `Drop` impl; tying them together (a single
// `'a`) forces the outer borrow to extend past `Stack`'s drop and trips dropck.
pub struct BleTransport<'b, 's, C>
where
    's: 'b,
    C: Controller + ControllerCmdAsync<LeSetPhy> + ControllerCmdSync<LeReadLocalSupportedFeatures>,
{
    stack: &'b Stack<'s, C, DefaultPacketPool>,
    server: Server<'static>,
    profile_manager: ProfileManager<'b, 's, C, DefaultPacketPool>,
    product_name: &'static str,
}

impl<'b, 's, C> BleTransport<'b, 's, C>
where
    's: 'b,
    C: Controller + ControllerCmdAsync<LeSetPhy> + ControllerCmdSync<LeReadLocalSupportedFeatures>,
{
    pub async fn new(stack: &'b Stack<'s, C, DefaultPacketPool>, rmk_config: RmkConfig<'static>) -> Self {
        #[cfg(feature = "_nrf_ble")]
        let serial_number = crate::ble::nrf::get_serial_number();
        #[cfg(not(feature = "_nrf_ble"))]
        let serial_number = rmk_config.device_config.serial_number;

        let profile_manager = ProfileManager::new(stack);

        info!("Starting advertising and GATT service");
        let server = Server::new_with_config(GapConfig::Peripheral(PeripheralConfig {
            name: rmk_config.device_config.product_name,
            appearance: &appearance::human_interface_device::KEYBOARD,
        }))
        .unwrap();

        server
            .set(
                &server.device_config_service.pnp_id,
                &PnPID {
                    vid_source: VidSource::UsbIF,
                    vendor_id: rmk_config.device_config.vid,
                    product_id: rmk_config.device_config.pid,
                    product_version: 0x0001,
                },
            )
            .unwrap();
        server
            .set(
                &server.device_config_service.serial_number,
                &heapless::String::try_from(serial_number).unwrap(),
            )
            .unwrap();
        server
            .set(
                &server.device_config_service.manufacturer_name,
                &heapless::String::try_from(rmk_config.device_config.manufacturer).unwrap(),
            )
            .unwrap();

        Self {
            stack,
            server,
            profile_manager,
            product_name: rmk_config.device_config.product_name,
        }
    }
}

impl<'b, 's, C> Runnable for BleTransport<'b, 's, C>
where
    's: 'b,
    C: Controller + ControllerCmdAsync<LeSetPhy> + ControllerCmdSync<LeReadLocalSupportedFeatures>,
{
    async fn run(&mut self) -> ! {
        // Load the preferred connection from storage
        let preferred = crate::state::load_preferred_connection().await;
        crate::state::set_preferred_connection(preferred);
        // Load the bonded devices from storage
        #[cfg(feature = "storage")]
        self.profile_manager.load_bonded_devices().await;
        self.profile_manager.update_stack_bonds();

        // Copy the &Stack reference so it doesn't tie a borrow to &mut self.
        let stack: &'b Stack<'s, C, DefaultPacketPool> = self.stack;
        let mut peripheral = stack.peripheral();
        let runner = stack.runner();

        let server = &self.server;
        let profile_manager = &mut self.profile_manager;
        let product_name = self.product_name;

        let connection_loop = async {
            loop {
                match select(
                    advertise(product_name, &mut peripheral, server),
                    profile_manager.update_profile(),
                )
                .await
                {
                    Either::First(Ok(conn)) => {
                        // Do NOT emit BleState::Connected here. gatt_events_task emits
                        // Connected when it sees GattConnectionEvent::Encrypted.
                        #[cfg(feature = "storage")]
                        let active_bond_info = profile_manager.active_bond_info();
                        if let Either::Second(_) = select(
                            run_ble_keyboard(
                                server,
                                &conn,
                                stack,
                                #[cfg(feature = "storage")]
                                active_bond_info,
                            ),
                            profile_manager.update_profile(),
                        )
                        .await
                        {
                            // When the profile changes, manually disconnect from the current host
                            if conn.raw().is_connected() {
                                conn.raw().disconnect();
                                loop {
                                    if let GattConnectionEvent::Disconnected { .. } = conn.next().await {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    Either::First(Err(BleHostError::BleHost(Error::Timeout))) => {
                        warn!("Advertising timeout, sleep and wait for any key");
                        set_ble_state(BleState::Inactive);

                        #[cfg(feature = "split")]
                        CENTRAL_SLEEP.signal(true);

                        // Wake on key or pointing activity after the advertising timeout.
                        let mut key_wake = crate::event::KeyboardEvent::subscriber();
                        let mut pointing_wake = crate::event::PointingEvent::subscriber();
                        let _ = select(key_wake.next_message_pure(), pointing_wake.next_message_pure()).await;

                        #[cfg(feature = "split")]
                        CENTRAL_SLEEP.signal(false);
                    }
                    Either::First(Err(e)) => {
                        #[cfg(feature = "defmt")]
                        let e = defmt::Debug2Format(&e);
                        error!("Advertise error: {:?}", e);
                        Timer::after_millis(200).await;
                    }
                    Either::Second(()) => {}
                };

                // Skip the Inactive transition if we never moved off Advertising
                // (e.g. an old host briefly connected, never reached the Encrypted
                // event, then dropped). Otherwise the LED would flicker
                // Advertising -> Inactive -> Advertising on every failed retry.
                if crate::state::current_ble_status().state != BleState::Advertising {
                    set_ble_state(BleState::Inactive);
                }
            }
        };

        join(ble_task(runner), connection_loop).await;
        unreachable!("BleTransport sub-tasks must run forever")
    }
}

/// This is a background task that is required to run forever alongside any other BLE tasks.
pub(crate) async fn ble_task<C: Controller + ControllerCmdAsync<LeSetPhy>, P: PacketPool>(
    mut runner: Runner<'_, C, P>,
) {
    loop {
        #[cfg(not(feature = "split"))]
        if let Err(_e) = runner.run().await {
            error!("[ble_task] runner.run() error");
            embassy_time::Timer::after_millis(100).await;
        }

        #[cfg(feature = "split")]
        {
            // Signal to indicate the stack is started
            crate::split::ble::central::STACK_STARTED.signal(true);
            if let Err(_e) = runner
                .run_with_handler(&crate::split::ble::central::ScanHandler {})
                .await
            {
                error!("[ble_task] runner.run_with_handler error");
                embassy_time::Timer::after_millis(100).await;
            }
        }
    }
}

/// Stream Events until the connection closes.
///
/// This function will handle the GATT events and process them.
/// This is how we interact with read and write requests.
async fn gatt_events_task(server: &Server<'_>, conn: &GattConnection<'_, '_, DefaultPacketPool>) -> Result<(), Error> {
    let level = server.battery_service.level;
    let output_keyboard = server.hid_service.output_keyboard;
    let hid_control_point = server.hid_service.hid_control_point;
    let input_keyboard = server.hid_service.input_keyboard;
    #[cfg(feature = "host")]
    let (output_host, input_host, host_control_point) = (
        server.host_service.output_data,
        server.host_service.input_data,
        server.host_service.hid_control_point,
    );
    let mouse = server.composite_service.mouse_report;
    let media = server.composite_service.media_report;
    let media_control_point = server.composite_service.hid_control_point;
    let system_control = server.composite_service.system_report;

    #[cfg(feature = "passkey_entry")]
    let mut passkey_state = PasskeyInputState::new();

    loop {
        #[cfg(feature = "passkey_entry")]
        let Some(event) = next_gatt_event(conn, &mut passkey_state).await else {
            continue;
        };
        #[cfg(not(feature = "passkey_entry"))]
        let event = conn.next().await;

        match event {
            GattConnectionEvent::Disconnected { reason } => {
                #[cfg(feature = "passkey_entry")]
                passkey_state.clear();
                info!("[gatt] disconnected: {:?}", reason);
                break;
            }
            GattConnectionEvent::PairingComplete { security_level, bond } => {
                #[cfg(feature = "passkey_entry")]
                passkey_state.clear();
                info!("[gatt] pairing complete: {:?}", security_level);
                let profile = crate::state::current_profile();
                if let Some(bond_info) = bond {
                    let cccd_table = server
                        .get_client_att_table(conn.raw())
                        .and_then(|t| heapless::Vec::from_slice(t.raw()).ok())
                        .unwrap_or_default();
                    let profile_info = ProfileInfo {
                        slot_num: profile,
                        info: bond_info,
                        removed: false,
                        cccd_table,
                    };
                    UPDATED_PROFILE.signal(profile_info);
                }
            }
            GattConnectionEvent::PairingFailed(err) => {
                #[cfg(feature = "passkey_entry")]
                passkey_state.clear();
                error!("[gatt] pairing error: {:?}", err);
            }
            GattConnectionEvent::Encrypted { security_level } => {
                info!("[gatt] encrypted: {:?}", security_level);
                set_ble_state(BleState::Connected);
            }
            GattConnectionEvent::Gatt { event: gatt_event } => {
                let mut cccd_updated = false;
                let result = match &gatt_event {
                    GattEvent::Read(event) => {
                        if event.handle() == level.handle {
                            let value = server.get(&level);
                            debug!("Read GATT Event to Level: {:?}", value);
                        } else {
                            debug!("Read GATT Event to Unknown: {:?}", event.handle());
                        }

                        if conn.raw().security_level()?.encrypted() {
                            None
                        } else {
                            Some(AttErrorCode::INSUFFICIENT_ENCRYPTION)
                        }
                    }
                    GattEvent::Write(event) => {
                        #[cfg(feature = "host")]
                        let host_control_point_match = event.handle() == host_control_point.handle;
                        #[cfg(not(feature = "host"))]
                        let host_control_point_match = false;

                        if event.handle() == output_keyboard.handle {
                            if event.data().len() == 1 {
                                let led_indicator = LedIndicator::from_bits(event.data()[0]);
                                debug!("Got keyboard state: {:?}", led_indicator);
                                LED_SIGNAL.signal(led_indicator);
                            } else {
                                warn!("Wrong keyboard state data: {:?}", event.data());
                            }
                        } else if event.handle() == input_keyboard.cccd_handle.expect("No CCCD for input keyboard")
                            || event.handle() == mouse.cccd_handle.expect("No CCCD for mouse report")
                            || event.handle() == media.cccd_handle.expect("No CCCD for media report")
                            || event.handle() == system_control.cccd_handle.expect("No CCCD for system report")
                            || event.handle() == level.cccd_handle.expect("No CCCD for battery level")
                        {
                            cccd_updated = true;
                        } else if event.handle() == hid_control_point.handle
                            || event.handle() == media_control_point.handle
                            || host_control_point_match
                        {
                            info!("Write GATT Event to Control Point: {:?}", event.handle());
                            #[cfg(feature = "split")]
                            {
                                // Forward an HID Control Point write to the split central's sleep signal.
                                // HID Class spec opcodes for the HID Control Point characteristic:
                                //   - 0: HID_CTRL_SUSPEND
                                //   - 1: HID_CTRL_EXIT_SUSPEND
                                let data = event.data();
                                if data.len() == 1 {
                                    match data[0] {
                                        0 => CENTRAL_SLEEP.signal(true),
                                        1 => CENTRAL_SLEEP.signal(false),
                                        _ => {}
                                    }
                                }
                            }
                        } else {
                            #[cfg(feature = "host")]
                            if event.handle() == output_host.handle {
                                debug!("Got host packet: {:?}", event.data());
                                if event.data().len() == 32 {
                                    let mut data = [0u8; 32];
                                    data.copy_from_slice(event.data());
                                    crate::channel::enqueue_host_request(ConnectionType::Ble, data).await;
                                } else {
                                    warn!("Wrong host packet data: {:?}", event.data());
                                }
                            } else if event.handle() == input_host.cccd_handle.expect("No CCCD for input host") {
                                cccd_updated = true;
                            } else {
                                debug!("Write GATT Event to Unknown: {:?}", event.handle());
                            }
                            #[cfg(not(feature = "host"))]
                            debug!("Write GATT Event to Unknown: {:?}", event.handle());
                        }

                        if conn.raw().security_level()?.encrypted() {
                            None
                        } else {
                            Some(AttErrorCode::INSUFFICIENT_ENCRYPTION)
                        }
                    }
                    GattEvent::Other(_) => None,
                    GattEvent::NotAllowed(_) => None,
                };

                // This step is also performed at drop(), but writing it explicitly is necessary
                // in order to ensure reply is sent.
                let result = if let Some(code) = result {
                    gatt_event.reject(code)
                } else {
                    gatt_event.accept()
                };
                match result {
                    Ok(reply) => reply.send().await,
                    Err(e) => warn!("[gatt] error sending response: {:?}", e),
                }

                // Update CCCD table after processing the event
                if cccd_updated {
                    // When macOS wakes up from sleep mode, it won't send EXIT SUSPEND command
                    // So we need to monitor the sleep state by using CCCD write event
                    #[cfg(feature = "split")]
                    CENTRAL_SLEEP.signal(false);

                    if let Some(table) = server.get_client_att_table(conn.raw())
                        && let Ok(bytes) = heapless::Vec::from_slice(table.raw())
                    {
                        UPDATED_CCCD_TABLE.signal(bytes);
                    }
                }
            }
            GattConnectionEvent::PhyUpdated { tx_phy, rx_phy } => {
                info!("[gatt] PhyUpdated: {:?}, {:?}", tx_phy, rx_phy)
            }
            GattConnectionEvent::ConnectionParamsUpdated {
                conn_interval,
                peripheral_latency,
                supervision_timeout,
            } => {
                info!(
                    "[gatt] ConnectionParamsUpdated: {:?}ms, {:?}, {:?}ms",
                    conn_interval.as_millis(),
                    peripheral_latency,
                    supervision_timeout.as_millis()
                );
            }
            GattConnectionEvent::RequestConnectionParams(req) => info!(
                "[gatt] RequestConnectionParams: interval: ({:?}, {:?})ms, {:?}, {:?}ms",
                req.params().min_connection_interval.as_millis(),
                req.params().max_connection_interval.as_millis(),
                req.params().max_latency,
                req.params().supervision_timeout.as_millis(),
            ),
            GattConnectionEvent::DataLengthUpdated {
                max_tx_octets,
                max_tx_time,
                max_rx_octets,
                max_rx_time,
            } => {
                info!(
                    "[gatt] DataLengthUpdated: tx/rx octets: ({:?}, {:?}), tx/rx time: ({:?}, {:?})",
                    max_tx_octets, max_rx_octets, max_tx_time, max_rx_time
                );
            }
            GattConnectionEvent::FrameSpaceUpdated {
                frame_space,
                initiator,
                phys,
                spacing_types,
            } => {
                info!(
                    "[gatt] FrameSpaceUpdated: {:?}, {:?}, {:?}, {:?}",
                    frame_space, initiator, phys, spacing_types
                );
            }
            GattConnectionEvent::ConnectionRateChanged {
                conn_interval,
                subrate_factor,
                peripheral_latency,
                continuation_number,
                supervision_timeout,
            } => {
                info!(
                    "[gatt] ConnectionRateChanged: {:?}ms, {:?}, {:?}, {:?}, {:?}ms",
                    conn_interval.as_millis(),
                    subrate_factor,
                    peripheral_latency,
                    continuation_number,
                    supervision_timeout.as_millis()
                );
            }
            GattConnectionEvent::PassKeyDisplay(pass_key) => info!("[gatt] PassKeyDisplay: {:?}", pass_key),
            GattConnectionEvent::PassKeyConfirm(pass_key) => info!("[gatt] PassKeyConfirm: {:?}", pass_key),
            GattConnectionEvent::PassKeyInput => {
                #[cfg(feature = "passkey_entry")]
                if crate::PASSKEY_ENTRY_ENABLED {
                    info!("[gatt] PassKeyInput: entering passkey entry mode");
                    passkey_state.begin();
                } else {
                    warn!("[gatt] PassKeyInput: disabled in config, cancelling pairing, this shouldn't happen");
                    if let Err(e) = conn.raw().pass_key_cancel() {
                        error!("[gatt] pass_key_cancel error: {:?}", e);
                    }
                }
                #[cfg(not(feature = "passkey_entry"))]
                warn!("[gatt] PassKeyInput event, should not happen")
            }
            GattConnectionEvent::BondLost => warn!("[gatt] BondLost"),
            GattConnectionEvent::OobRequest => warn!("[gatt] OobRequest"),
        }
    }
    info!("[gatt] task finished");
    Ok(())
}

/// Create an advertiser to use to connect to a BLE Central, and wait for it to connect.
async fn advertise<'a, 'b, C: Controller>(
    name: &'a str,
    peripheral: &mut Peripheral<'a, C, DefaultPacketPool>,
    server: &'b Server<'_>,
) -> Result<GattConnection<'a, 'b, DefaultPacketPool>, BleHostError<C::Error>> {
    // Wait for 10ms to ensure the USB is checked
    embassy_time::Timer::after_millis(10).await;
    let mut advertiser_data = [0; 31];
    AdStructure::encode_slice(
        &[
            AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
            AdStructure::CompleteServiceUuids16(&[BATTERY.to_le_bytes(), HUMAN_INTERFACE_DEVICE.to_le_bytes()]),
            AdStructure::CompleteLocalName(name.as_bytes()),
            AdStructure::Unknown {
                ty: 0x19, // Appearance
                data: &KEYBOARD.to_le_bytes(),
            },
        ],
        &mut advertiser_data[..],
    )?;

    let advertise_config = AdvertisementParameters {
        primary_phy: PhyKind::Le2M,
        secondary_phy: PhyKind::Le2M,
        tx_power: TxPower::Plus8dBm,
        interval_min: Duration::from_millis(200),
        interval_max: Duration::from_millis(200),
        ..Default::default()
    };

    info!("[adv] advertising");
    set_ble_state(BleState::Advertising);
    let advertiser = peripheral
        .advertise(
            &advertise_config,
            Advertisement::ConnectableScannableUndirected {
                adv_data: &advertiser_data[..],
                scan_data: &[],
            },
        )
        .await?;

    // Timeout for advertising is 300s
    match with_timeout(Duration::from_secs(300), advertiser.accept()).await {
        Ok(conn_res) => {
            let conn = conn_res?.with_attribute_server(server)?;
            info!("[adv] connection established");
            if let Err(e) = conn.raw().set_bondable(true) {
                error!("Set bondable error: {:?}", e);
            };
            Ok(conn)
        }
        Err(_) => Err(BleHostError::BleHost(Error::Timeout)),
    }
}

pub(crate) async fn set_conn_params<
    'a,
    'b,
    C: Controller + ControllerCmdSync<LeReadLocalSupportedFeatures>,
    P: PacketPool,
>(
    stack: &Stack<'_, C, P>,
    conn: &GattConnection<'a, 'b, P>,
) {
    // Wait for 5 seconds before setting connection parameters to avoid connection drop
    embassy_time::Timer::after_secs(5).await;

    // For macOS/iOS(aka Apple devices), both interval should be set to 15ms
    // Reference: https://developer.apple.com/accessories/Accessory-Design-Guidelines.pdf
    update_conn_params(
        stack,
        conn.raw(),
        &RequestedConnParams {
            min_connection_interval: Duration::from_millis(15),
            max_connection_interval: Duration::from_millis(15),
            max_latency: 30,
            min_event_length: Duration::from_secs(0),
            max_event_length: Duration::from_secs(0),
            supervision_timeout: Duration::from_secs(5),
        },
    )
    .await;

    embassy_time::Timer::after_secs(5).await;

    // Setting the conn param the second time ensures that we have best performance on all platforms
    update_conn_params(
        stack,
        conn.raw(),
        &RequestedConnParams {
            min_connection_interval: Duration::from_micros(7500),
            max_connection_interval: Duration::from_micros(7500),
            max_latency: 30,
            min_event_length: Duration::from_secs(0),
            max_event_length: Duration::from_secs(0),
            supervision_timeout: Duration::from_secs(5),
        },
    )
    .await;

    // Wait forever. This is because we want the conn params setting can be interrupted when the connection is lost.
    // So this task shouldn't quit after setting the conn params.
    core::future::pending::<()>().await;
}

/// Run BLE keyboard for one connection.
///
/// Returns when the GATT events task ends (i.e. the connection drops).
/// `writer_task`, `led_task`, and `host_task` are all infinite, so the outer
/// `select(communication_task, inner)` cancels them as a side-effect of
/// `communication_task` returning. `inner` itself never completes.
async fn run_ble_keyboard<
    'a,
    'b,
    C: Controller + ControllerCmdAsync<LeSetPhy> + ControllerCmdSync<LeReadLocalSupportedFeatures>,
>(
    server: &'b Server<'_>,
    conn: &GattConnection<'a, 'b, DefaultPacketPool>,
    stack: &Stack<'_, C, DefaultPacketPool>,
    #[cfg(feature = "storage")] active_bond_info: Option<crate::ble::profile::ProfileInfo>,
) {
    let mut ble_hid_server = BleHidServer::new(server, conn);
    let mut ble_led_reader = BleLedReader {};
    let mut ble_battery_server = BleBatteryServer::new(server, conn);

    // CCCD lookup uses cached bond info to avoid a cancellable flash read while
    // this future is racing other arms of an outer `select`.
    #[cfg(feature = "storage")]
    if let Some(bond_info) = active_bond_info
        && bond_info.info.identity.match_identity(&conn.raw().peer_identity())
    {
        info!("Loading CCCD table: {:?}", bond_info.cccd_table);
        match ClientAttTableView::try_from_raw(&bond_info.cccd_table) {
            Ok(view) => server.set_client_att_table(conn.raw(), &view),
            Err(e) => warn!("Invalid stored CCCD table: {:?}", e),
        }
    }

    // Use 2M Phy
    update_ble_phy(stack, conn.raw()).await;

    let communication_task = async {
        if let Either3::First(e) = select3(
            gatt_events_task(server, conn),
            set_conn_params(stack, conn),
            ble_battery_server.run(),
        )
        .await
        {
            error!("[gatt_events_task] end: {:?}", e)
        }
    };

    let writer_task = async {
        loop {
            let report = BLE_REPORT_CHANNEL.receive().await;
            if let Err(e) = ble_hid_server.write_report(&report).await {
                error!("Failed to send report: {:?}", e);
            }
        }
    };

    let led_task = run_led_reader(&mut ble_led_reader, ConnectionType::Ble);

    #[cfg(feature = "host")]
    let host_task = crate::host::ble::run_ble_host(server.host_service.input_data, conn);
    #[cfg(not(feature = "host"))]
    let host_task = core::future::pending::<()>();

    let inner = embassy_futures::join::join3(writer_task, led_task, host_task);
    select(communication_task, inner).await;
}

// Update the PHY to 2M
pub(crate) async fn update_ble_phy<P: PacketPool>(
    stack: &Stack<'_, impl Controller + ControllerCmdAsync<LeSetPhy>, P>,
    conn: &Connection<'_, P>,
) {
    loop {
        match conn.set_phy(stack, PhyKind::Le2M).await {
            Err(BleHostError::BleHost(Error::Hci(error))) => {
                if 0x2A == error.to_status().into_inner() {
                    // Busy, retry
                    info!("[update_ble_phy] HCI busy: {:?}", error);
                    continue;
                } else {
                    error!("[update_ble_phy] HCI error: {:?}", error);
                }
            }
            Err(e) => {
                #[cfg(feature = "defmt")]
                let e = defmt::Debug2Format(&e);
                error!("[update_ble_phy] error: {:?}", e);
            }
            Ok(_) => {
                info!("[update_ble_phy] PHY updated");
            }
        }
        break;
    }
}

// Update the connection parameters
pub(crate) async fn update_conn_params<
    'a,
    'b,
    C: Controller + ControllerCmdSync<LeReadLocalSupportedFeatures>,
    P: PacketPool,
>(
    stack: &Stack<'a, C, P>,
    conn: &Connection<'b, P>,
    params: &RequestedConnParams,
) {
    loop {
        match conn.update_connection_params(stack, params).await {
            Err(BleHostError::BleHost(Error::Hci(error))) => {
                if 0x3A == error.to_status().into_inner() {
                    // Busy, retry
                    info!("[update_conn_params] HCI busy: {:?}", error);
                    embassy_time::Timer::after_millis(100).await;
                    continue;
                } else {
                    error!("[update_conn_params] HCI error: {:?}", error);
                }
            }
            Err(e) => {
                #[cfg(feature = "defmt")]
                let e = defmt::Debug2Format(&e);
                error!("[update_conn_params] BLE host error: {:?}", e);
            }
            _ => (),
        }
        break;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, OnceLock};

    use embassy_futures::join::join;
    use embassy_futures::select::select;
    use embassy_time::Timer;
    use rmk_types::ble::{BleState, BleStatus};

    use crate::event::{Axis, AxisEvent, AxisValType, KeyboardEvent, PointingEvent, SubscribableEvent, publish_event};
    use crate::state::{current_ble_status, set_ble_profile, set_ble_state};
    use crate::test_support::test_block_on as block_on;

    fn ble_status_test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn set_ble_state_preserves_current_profile() {
        let _guard = ble_status_test_lock().lock().unwrap();

        set_ble_profile(2);
        set_ble_state(BleState::Advertising);

        assert_eq!(
            current_ble_status(),
            BleStatus {
                profile: 2,
                state: BleState::Advertising,
            }
        );
    }

    #[test]
    fn set_ble_profile_resets_state_when_profile_changes() {
        let _guard = ble_status_test_lock().lock().unwrap();

        set_ble_profile(1);
        set_ble_state(BleState::Connected);
        set_ble_profile(3);

        assert_eq!(
            current_ble_status(),
            BleStatus {
                profile: 3,
                state: BleState::Inactive,
            }
        );
    }

    #[test]
    fn wake_activity_includes_pointing_events() {
        let _guard = ble_status_test_lock().lock().unwrap();

        block_on(async {
            let wake = async {
                let mut key_wake = KeyboardEvent::subscriber();
                let mut pointing_wake = PointingEvent::subscriber();
                let _ = select(key_wake.next_message_pure(), pointing_wake.next_message_pure()).await;
            };
            join(wake, async {
                Timer::after_millis(1).await;
                publish_event(PointingEvent([
                    AxisEvent {
                        typ: AxisValType::Rel,
                        axis: Axis::X,
                        value: 1,
                    },
                    AxisEvent {
                        typ: AxisValType::Rel,
                        axis: Axis::Y,
                        value: 0,
                    },
                    AxisEvent {
                        typ: AxisValType::Rel,
                        axis: Axis::Z,
                        value: 0,
                    },
                ]));
            })
            .await;
        });
    }
}
