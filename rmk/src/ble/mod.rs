use core::sync::atomic::{AtomicBool, Ordering};

use bt_hci::cmd::le::{LeReadLocalSupportedFeatures, LeSetPhy};
use bt_hci::controller::{ControllerCmdAsync, ControllerCmdSync};
use embassy_futures::join::join;
#[cfg(feature = "passkey_entry")]
use embassy_futures::select::Either;
use embassy_futures::select::{Either3, select, select3};
use embassy_time::{Duration, Timer, with_timeout};
#[cfg(feature = "passkey_entry")]
use embassy_time::{Instant, with_deadline};
use rand_core::{CryptoRng, RngCore};
use rmk_types::led_indicator::LedIndicator;
use trouble_host::prelude::appearance::human_interface_device::KEYBOARD;
use trouble_host::prelude::service::{BATTERY, HUMAN_INTERFACE_DEVICE};
use trouble_host::prelude::*;

use crate::ble::battery_service::BleBatteryServer;
use crate::ble::ble_server::{BleHidServer, Server};
use crate::ble::device_info::{PnPID, VidSource};
use crate::ble::led::BleLedReader;
use crate::ble::profile::{ProfileInfo, ProfileManager, UPDATED_CCCD_TABLE, UPDATED_PROFILE};
use crate::channel::LED_SIGNAL;
use crate::config::RmkConfig;
use crate::core_traits::Runnable;
use crate::event::{ConnectionType, SubscribableEvent};
use crate::hid::{RunnableHidWriter, run_led_reader};
#[cfg(feature = "split")]
use crate::split::ble::central::CENTRAL_SLEEP;
#[cfg(feature = "storage")]
use crate::storage::StorageKey;
pub(crate) mod battery_service;
pub(crate) mod ble_server;
pub(crate) mod device_info;
pub(crate) mod led;
#[cfg(feature = "passkey_entry")]
pub mod passkey;
pub(crate) mod profile;

use rmk_types::ble::BleState;

use crate::state::set_ble_state;

pub(crate) fn get_current_profile() -> u8 {
    crate::state::connection_status().ble.profile
}

/// Global state of sleep management
/// - `true`: Indicates central is sleeping
/// - `false`: Indicates central is awake
pub(crate) static SLEEPING_STATE: AtomicBool = AtomicBool::new(false);

// TODO: Add documentation about how to define split peripheral num in Rust code
/// Max number of connections
pub(crate) const CONNECTIONS_MAX: usize = crate::SPLIT_PERIPHERALS_NUM + 1;

/// Max number of L2CAP channels
pub(crate) const L2CAP_CHANNELS_MAX: usize = CONNECTIONS_MAX * 4; // Signal + att + smp + hid

#[cfg(feature = "passkey_entry")]
struct PasskeyInputState {
    deadline: Option<Instant>,
    cleanup: Option<crate::ble::passkey::PasskeyCleanupGuard>,
}

#[cfg(feature = "passkey_entry")]
impl PasskeyInputState {
    const fn new() -> Self {
        Self {
            deadline: None,
            cleanup: None,
        }
    }

    fn clear(&mut self) {
        self.deadline = None;
        drop(self.cleanup.take());
    }

    fn begin(&mut self) {
        use crate::ble::passkey::{PasskeyCleanupGuard, begin_passkey_entry_session};

        self.clear();
        begin_passkey_entry_session();
        self.cleanup = Some(PasskeyCleanupGuard::new());
        self.deadline = Some(Instant::now() + Duration::from_secs(crate::PASSKEY_ENTRY_TIMEOUT_SECS as u64));
    }
}

#[cfg(feature = "passkey_entry")]
async fn next_gatt_event<'a, 'b>(
    conn: &GattConnection<'a, 'b, DefaultPacketPool>,
    passkey_state: &mut PasskeyInputState,
) -> Option<GattConnectionEvent<'a, 'b, DefaultPacketPool>> {
    if crate::PASSKEY_ENTRY_ENABLED
        && let Some(deadline) = passkey_state.deadline
    {
        use crate::ble::passkey::PASSKEY_RESPONSE;

        return match select(conn.next(), with_deadline(deadline, PASSKEY_RESPONSE.wait())).await {
            Either::First(event) => Some(event),
            Either::Second(Ok(Some(passkey))) => {
                passkey_state.clear();

                info!("[gatt] Passkey entered: submitting");
                if let Err(e) = conn.raw().pass_key_input(passkey) {
                    error!("[gatt] pass_key_input error: {:?}", e);
                }
                None
            }
            Either::Second(Ok(None)) => {
                passkey_state.clear();

                info!("[gatt] Passkey entry cancelled");
                if let Err(e) = conn.raw().pass_key_cancel() {
                    error!("[gatt] pass_key_cancel error: {:?}", e);
                }
                None
            }
            Either::Second(Err(_)) => {
                passkey_state.clear();

                warn!("[gatt] Passkey entry timeout");
                let _ = conn.raw().pass_key_cancel();
                None
            }
        };
    }

    Some(conn.next().await)
}

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
    resources: &'a mut HostResources<P, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX>,
) -> Stack<'a, C, P> {
    // Initialize trouble host stack
    trouble_host::new(controller, resources)
        .set_random_address(Address::random(host_address))
        .set_random_generator_seed(random_generator)
}

#[doc(hidden)]
pub fn passkey_entry_enabled() -> bool {
    #[cfg(feature = "passkey_entry")]
    {
        crate::PASSKEY_ENTRY_ENABLED
    }
    #[cfg(not(feature = "passkey_entry"))]
    {
        false
    }
}

/// Wait for local input activity that should wake the BLE transport back up
/// after an advertising timeout.
async fn wait_for_wake_activity() {
    let mut key_wake = crate::event::KeyboardEvent::subscriber();
    let mut pointing_wake = crate::event::PointingEvent::subscriber();

    match select(key_wake.next_message_pure(), pointing_wake.next_message_pure()).await {
        _ => {}
    }
}

/// BLE transport runnable. Owns the trouble-host server and profile manager;
/// `run` joins the background `ble_task` runner with the advertise→connect→serve
/// loop and runs forever.
pub struct BleTransport<'a, C>
where
    C: Controller + ControllerCmdAsync<LeSetPhy> + ControllerCmdSync<LeReadLocalSupportedFeatures>,
{
    stack: &'a Stack<'a, C, DefaultPacketPool>,
    server: Server<'static>,
    profile_manager: ProfileManager<'a, C, DefaultPacketPool>,
    product_name: &'static str,
}

impl<'a, C> BleTransport<'a, C>
where
    C: Controller + ControllerCmdAsync<LeSetPhy> + ControllerCmdSync<LeReadLocalSupportedFeatures>,
{
    pub async fn new(
        stack: &'a Stack<'a, C, DefaultPacketPool>,
        #[cfg_attr(not(feature = "_nrf_ble"), allow(unused_mut))] mut rmk_config: RmkConfig<'static>,
    ) -> Self {
        #[cfg(feature = "_nrf_ble")]
        {
            rmk_config.device_config.serial_number = crate::hid::get_serial_number();
        }

        #[cfg(feature = "storage")]
        let stored = crate::storage::read_setting(StorageKey::ConnectionType).await;
        #[cfg(not(feature = "storage"))]
        let stored: Option<u8> = None;
        let preferred = match stored {
            Some(c) => c.into(),
            #[cfg(feature = "_no_usb")]
            None => ConnectionType::Ble,
            #[cfg(not(feature = "_no_usb"))]
            None => ConnectionType::Usb,
        };
        crate::state::set_preferred(preferred);

        let mut profile_manager = ProfileManager::new(stack);
        #[cfg(feature = "storage")]
        profile_manager.load_bonded_devices().await;
        profile_manager.update_stack_bonds();

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
                &heapless::String::try_from(rmk_config.device_config.serial_number).unwrap(),
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

impl<'a, C> Runnable for BleTransport<'a, C>
where
    C: Controller + ControllerCmdAsync<LeSetPhy> + ControllerCmdSync<LeReadLocalSupportedFeatures>,
{
    async fn run(&mut self) -> ! {
        // Copy the &Stack reference so it doesn't tie a borrow to &mut self.
        let stack: &'a Stack<'a, C, DefaultPacketPool> = self.stack;
        let Host {
            mut peripheral, runner, ..
        } = stack.build();

        let server = &self.server;
        let profile_manager = &mut self.profile_manager;
        let product_name = self.product_name;

        let connection_loop = async {
            loop {
                set_ble_state(BleState::Advertising);
                match advertise(product_name, &mut peripheral, server).await {
                    Ok(conn) => {
                        // Promote BLE to connected as soon as the GATT link is
                        // established so the first post-connect key can route.
                        set_ble_state(BleState::Connected);
                        #[cfg(feature = "storage")]
                        let active_bond_info = profile_manager.active_bond_info();
                        select(
                            run_ble_keyboard(
                                server,
                                &conn,
                                stack,
                                #[cfg(feature = "storage")]
                                active_bond_info,
                            ),
                            profile_manager.update_profile(),
                        )
                        .await;
                        set_ble_state(BleState::Inactive);
                    }
                    Err(BleHostError::BleHost(Error::Timeout)) => {
                        warn!("Advertising timeout, sleep and wait for any key");
                        set_ble_state(BleState::Inactive);
                        // Once the user has typed at least one key post-sleep,
                        // keep scanning the matrix across all subsequent
                        // advertise/connect cycles so reconnect-window keys
                        // aren't dropped.
                        crate::state::MATRIX_SCAN_OVERRIDE.store(true, Ordering::Release);

                        #[cfg(feature = "split")]
                        CENTRAL_SLEEP.signal(true);

                        wait_for_wake_activity().await;

                        #[cfg(feature = "split")]
                        CENTRAL_SLEEP.signal(false);
                    }
                    Err(e) => {
                        #[cfg(feature = "defmt")]
                        let e = defmt::Debug2Format(&e);
                        error!("Advertise error: {:?}", e);
                    }
                }
                // Avoid pegging the CPU on a tight advertise-failure loop.
                Timer::after_millis(200).await;
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
    let output_host = server.host_service.output_data;
    #[cfg(feature = "host")]
    let input_host = server.host_service.input_data;
    #[cfg(feature = "host")]
    let host_control_point = server.host_service.hid_control_point;
    let battery_level = server.battery_service.level;
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
                let profile = get_current_profile();
                if let Some(bond_info) = bond {
                    let profile_info = ProfileInfo {
                        slot_num: profile,
                        info: bond_info,
                        removed: false,
                        cccd_table: server.get_cccd_table(conn.raw()).unwrap(),
                    };
                    UPDATED_PROFILE.signal(profile_info);
                }
            }
            GattConnectionEvent::PairingFailed(err) => {
                #[cfg(feature = "passkey_entry")]
                passkey_state.clear();
                error!("[gatt] pairing error: {:?}", err);
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
                            || event.handle() == battery_level.cccd_handle.expect("No CCCD for battery level")
                        {
                            // CCCD write event
                            cccd_updated = true;
                        } else if event.handle() == hid_control_point.handle
                            || event.handle() == media_control_point.handle
                        {
                            info!("Write GATT Event to Control Point: {:?}", event.handle());
                            #[cfg(feature = "split")]
                            if event.data().len() == 1 {
                                let data = event.data()[0];
                                if data == 0 {
                                    // Enter sleep mode
                                    CENTRAL_SLEEP.signal(true);
                                } else if data == 1 {
                                    // Wake up
                                    CENTRAL_SLEEP.signal(false);
                                }
                            }
                        } else {
                            #[cfg(feature = "host")]
                            if event.handle() == output_host.handle {
                                debug!("Got host packet: {:?}", event.data());
                                if event.data().len() == 32 {
                                    use crate::channel::{HOST_REQUEST_CHANNEL, HostTransport};

                                    let mut data = [0u8; 32];
                                    data.copy_from_slice(event.data());
                                    HOST_REQUEST_CHANNEL.send((HostTransport::Ble, data)).await;
                                } else {
                                    warn!("Wrong host packet data: {:?}", event.data());
                                }
                            } else if event.handle() == input_host.cccd_handle.expect("No CCCD for input host") {
                                // CCCD write event
                                cccd_updated = true;
                            } else if event.handle() == host_control_point.handle {
                                info!("Write GATT Event to Control Point: {:?}", event.handle());
                                #[cfg(feature = "split")]
                                if event.data().len() == 1 {
                                    let data = event.data()[0];
                                    if data == 0 {
                                        // Enter sleep mode
                                        CENTRAL_SLEEP.signal(true);
                                    } else if data == 1 {
                                        // Wake up
                                        CENTRAL_SLEEP.signal(false);
                                    }
                                }
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

                    if let Some(table) = server.get_cccd_table(conn.raw()) {
                        UPDATED_CCCD_TABLE.signal(table);
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

/// Run BLE keyboard with connected device. Returns when any inner task ends —
/// typically the GATT events task on disconnect.
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
        server.set_cccd_table(conn.raw(), bond_info.cccd_table);
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

    let writer_task = ble_hid_server.run_writer();

    let led_task = run_led_reader(&mut ble_led_reader, ConnectionType::Ble);

    #[cfg(feature = "host")]
    let host_task = crate::host::run_ble_host(server.host_service.input_data, conn);
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
    use embassy_time::Timer;
    use rmk_types::ble::{BleState, BleStatus};

    use crate::event::{publish_event, Axis, AxisEvent, AxisValType, PointingEvent};
    use crate::state::{connection_status, set_ble_state, set_ble_status};
    use crate::test_support::test_block_on as block_on;

    fn ble_status_test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn set_ble_state_preserves_current_profile() {
        let _guard = ble_status_test_lock().lock().unwrap();

        set_ble_status(BleStatus {
            profile: 2,
            state: BleState::Inactive,
        });
        set_ble_state(BleState::Advertising);

        assert_eq!(
            connection_status().ble,
            BleStatus {
                profile: 2,
                state: BleState::Advertising,
            }
        );
    }

    #[test]
    fn set_ble_status_can_reset_state_when_profile_changes() {
        let _guard = ble_status_test_lock().lock().unwrap();

        set_ble_status(BleStatus {
            profile: 1,
            state: BleState::Connected,
        });
        set_ble_status(BleStatus {
            profile: 3,
            state: BleState::Inactive,
        });

        assert_eq!(
            connection_status().ble,
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
            join(super::wait_for_wake_activity(), async {
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
