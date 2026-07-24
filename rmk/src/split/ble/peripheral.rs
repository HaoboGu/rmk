#[cfg(feature = "dfu_ble")]
use core::sync::atomic::{AtomicBool, Ordering};

use bt_hci::cmd::le::{LeReadLocalSupportedFeatures, LeSetPhy};
use bt_hci::controller::{ControllerCmdAsync, ControllerCmdSync};
use embassy_futures::join::join;
use embassy_time::{Duration, Timer, with_timeout};
use rmk_types::connection::ConnectionStatus;
use trouble_host::prelude::*;

#[cfg(feature = "storage")]
use super::PeerAddress;
#[cfg(feature = "dfu_ble")]
use crate::ble::dfu_service::{ButtonlessDfuService, DFU_PACKET_BUF_SIZE, DfuService};
#[cfg(feature = "dfu_ble")]
use crate::dfu::ble_dfu::BleDfuHandler;
use crate::event::{CentralConnectedEvent, publish_event};
use crate::split::driver::{SplitDriverError, SplitReader, SplitWriter};
use crate::split::peripheral::SplitPeripheral;
use crate::split::{SPLIT_MESSAGE_MAX_SIZE, SplitMessage};
use crate::state::update_status;

#[cfg(all(feature = "dfu_ble", feature = "dfu_nrf"))]
type DfuFlash = crate::dfu::ble_nrf::AsyncDfuPartition;
#[cfg(all(feature = "dfu_ble", feature = "dfu_rp"))]
type DfuFlash = crate::dfu::ble_rp::AsyncDfuPartition;

/// Gatt service used in split peripheral to send split message to central
#[gatt_service(uuid = "4dd5fbaa-18e5-4b07-bf0a-353698659946")]
pub(crate) struct SplitBleService {
    #[characteristic(uuid = "0e6313e3-bd0b-45c2-8d2e-37a2e8128bc3", read, notify, indicate)]
    pub(crate) message_to_central: [u8; SPLIT_MESSAGE_MAX_SIZE],

    #[characteristic(uuid = "4b3514fb-cae4-4d38-a097-3a2a3d1c3b9c", write_without_response, read, notify)]
    pub(crate) message_to_peripheral: [u8; SPLIT_MESSAGE_MAX_SIZE],
}

/// Gatt server in split peripheral
#[gatt_server]
pub(crate) struct BleSplitPeripheralServer {
    pub(crate) service: SplitBleService,
    #[cfg(feature = "dfu_ble")]
    pub(crate) dfu_service: DfuService,
    #[cfg(feature = "dfu_ble")]
    pub(crate) buttonless_dfu_service: ButtonlessDfuService,
}

/// BLE driver for split peripheral
pub(crate) struct BleSplitPeripheralDriver<'stack_ref, 'stack_inner, 'server, 'c, C: Controller, P: PacketPool> {
    message_to_peripheral: Characteristic<[u8; SPLIT_MESSAGE_MAX_SIZE]>,
    message_to_central: Characteristic<[u8; SPLIT_MESSAGE_MAX_SIZE]>,
    conn: &'c GattConnection<'stack_ref, 'server, P>,
    stack: &'stack_ref Stack<'stack_inner, C, P>,
    #[cfg(feature = "dfu_ble")]
    dfu_control_point: Characteristic<[u8; 64]>,
    #[cfg(feature = "dfu_ble")]
    dfu_packet: Characteristic<[u8; DFU_PACKET_BUF_SIZE]>,
    #[cfg(feature = "dfu_ble")]
    dfu_handler: Option<BleDfuHandler<DfuFlash>>,
}

impl<'stack_ref, 'stack_inner, 'server, 'c, C: Controller, P: PacketPool>
    BleSplitPeripheralDriver<'stack_ref, 'stack_inner, 'server, 'c, C, P>
{
    pub(crate) fn new(
        server: &'server BleSplitPeripheralServer,
        conn: &'c GattConnection<'stack_ref, 'server, P>,
        stack: &'stack_ref Stack<'stack_inner, C, P>,
        #[cfg(feature = "dfu_ble")] dfu_handler: Option<BleDfuHandler<DfuFlash>>,
    ) -> Self {
        Self {
            message_to_central: server.service.message_to_central,
            message_to_peripheral: server.service.message_to_peripheral,
            conn,
            stack,
            #[cfg(feature = "dfu_ble")]
            dfu_control_point: server.dfu_service.dfu_control_point,
            #[cfg(feature = "dfu_ble")]
            dfu_packet: server.dfu_service.dfu_packet,
            #[cfg(feature = "dfu_ble")]
            dfu_handler,
        }
    }
}

// DFU write handling shared between SplitReader::read() and run_dfu_mode().
#[cfg(all(feature = "dfu_ble", any(feature = "dfu_nrf", feature = "dfu_rp")))]
impl<
    'stack_ref,
    'stack_inner,
    'server,
    'c,
    C: Controller + ControllerCmdSync<LeReadLocalSupportedFeatures>,
    P: PacketPool,
> BleSplitPeripheralDriver<'stack_ref, 'stack_inner, 'server, 'c, C, P>
{
    async fn handle_dfu_write(&mut self, event_handle: u16, data: &[u8]) -> bool {
        if !PERIPHERAL_DFU_LATENCY_UPDATED.load(Ordering::Relaxed) {
            PERIPHERAL_DFU_LATENCY_UPDATED.store(true, Ordering::Relaxed);
            info!("ble dfu: updating connection latency for DFU performance");
            let _ = crate::ble::update_conn_params(
                self.stack,
                self.conn.raw(),
                &RequestedConnParams {
                    min_connection_interval: Duration::from_micros(7500),
                    max_connection_interval: Duration::from_micros(7500),
                    max_latency: 0,
                    min_event_length: Duration::from_secs(0),
                    max_event_length: Duration::from_secs(0),
                    supervision_timeout: Duration::from_secs(20),
                },
            )
            .await;
        }
        if event_handle == self.dfu_control_point.handle {
            debug!("ble dfu: control point write, len={}", data.len());
            if let Some(ref mut handler) = self.dfu_handler {
                if let Some(resp) = handler.handle_control_point(data).await {
                    let mut resp_buf = [0u8; 64];
                    let resp_data = resp.response_data();
                    if resp_data.len() >= 3 {
                        debug!(
                            "ble dfu: cp resp op={} ({}), result={}",
                            resp_data[1],
                            crate::dfu::ble_dfu::dfu_op_name(resp_data[1]),
                            resp_data[2]
                        );
                    }
                    resp_buf[..resp_data.len()].copy_from_slice(resp_data);
                    let _ = self.dfu_control_point.notify(self.conn, &resp_buf, true).await;
                }
            }
            true
        } else if event_handle == self.dfu_packet.handle {
            info!("ble dfu: packet write, len={}", data.len());
            if let Some(ref mut handler) = self.dfu_handler {
                if let Some(resp) = handler.handle_packet(data).await {
                    let resp_data = resp.response_data();
                    if resp_data.len() > 3 {
                        let mut resp_buf = [0u8; 64];
                        resp_buf[..resp_data.len()].copy_from_slice(resp_data);
                        let _ = self.dfu_control_point.notify(self.conn, &resp_buf, true).await;
                    }
                }
            }
            true
        } else {
            false
        }
    }
}

#[cfg(feature = "dfu_ble")]
static PERIPHERAL_DFU_LATENCY_UPDATED: AtomicBool = AtomicBool::new(false);

impl<
    'stack_ref,
    'stack_inner,
    'server,
    'c,
    C: Controller + ControllerCmdSync<LeReadLocalSupportedFeatures>,
    P: PacketPool,
> SplitReader for BleSplitPeripheralDriver<'stack_ref, 'stack_inner, 'server, 'c, C, P>
{
    async fn read(&mut self) -> Result<SplitMessage, SplitDriverError> {
        let message = loop {
            match self.conn.next().await {
                GattConnectionEvent::Disconnected { reason } => {
                    #[cfg(all(feature = "dfu_ble", any(feature = "dfu_nrf", feature = "dfu_rp")))]
                    if let Some(ref handler) = self.dfu_handler {
                        if handler.is_complete() {
                            info!("ble dfu: finalizing on disconnect (complete={})", handler.is_complete());
                            crate::dfu::mark_updated_and_reset();
                        }
                    }
                    error!("Disconnected from central: {:?}", reason);
                    update_status(|c| *c = ConnectionStatus::new());
                    return Err(SplitDriverError::Disconnected);
                }
                GattConnectionEvent::Gatt { event: gatt_event } => {
                    match &gatt_event {
                        GattEvent::Read(event) => {
                            info!("Gatt read event: {:?}", event.handle());
                        }
                        GattEvent::Write(event) => {
                            #[cfg(not(feature = "dfu_ble"))]
                            let dfu_handled = false;
                            #[cfg(feature = "dfu_ble")]
                            let dfu_handled = {
                                let mut data_buf = [0u8; 247];
                                let data_len = event.with_data(|_, data| {
                                    let n = data.len().min(data_buf.len());
                                    data_buf[..n].copy_from_slice(&data[..n]);
                                    data.len()
                                });
                                let data = &data_buf[..data_len.min(data_buf.len())];
                                self.handle_dfu_write(event.handle(), data).await
                            };
                            // Write to peripheral (skip if DFU)
                            if !dfu_handled && event.handle() == self.message_to_peripheral.handle {
                                let parsed = event.with_data(|_, data| {
                                    trace!("Got message from central: {:?}", data);
                                    postcard::from_bytes::<SplitMessage>(data)
                                });
                                match parsed {
                                    Ok(message) => {
                                        trace!("Message from central: {:?}", message);
                                        break message;
                                    }
                                    Err(e) => error!("Postcard deserialize split message error: {}", e),
                                }
                            } else if !dfu_handled {
                                info!("Gatt write other event: {:?}", event.handle());
                            }
                        }
                        _ => debug!("Other gatt event"),
                    };
                    match gatt_event.accept() {
                        Ok(r) => r.send().await,
                        Err(e) => warn!("[gatt] error sending response: {:?}", e),
                    }
                }
                GattConnectionEvent::ConnectionParamsUpdated {
                    conn_interval,
                    peripheral_latency,
                    supervision_timeout,
                } => {
                    info!(
                        "Connection parameters updated: {:?}ms, {:?}, {:?}ms",
                        conn_interval.as_millis(),
                        peripheral_latency,
                        supervision_timeout.as_millis()
                    );
                }
                GattConnectionEvent::PhyUpdated { tx_phy, rx_phy } => {
                    info!("PHY updated: {:?}, {:?}", tx_phy, rx_phy);
                }
                _ => (),
            }
        };
        Ok(message)
    }
}

impl<'stack_ref, 'stack_inner, 'server, 'c, C: Controller, P: PacketPool> SplitWriter
    for BleSplitPeripheralDriver<'stack_ref, 'stack_inner, 'server, 'c, C, P>
{
    async fn write(&mut self, message: &SplitMessage) -> Result<usize, SplitDriverError> {
        let mut buf = [0_u8; SPLIT_MESSAGE_MAX_SIZE];
        postcard::to_slice(message, &mut buf).map_err(|e| {
            error!("Postcard serialize split message error: {}", e);
            SplitDriverError::SerializeError
        })?;
        info!("Writing split message to central: {:?}", message);
        self.message_to_central
            .notify(self.conn, &buf, true)
            .await
            .map_err(|e| {
                error!("BLE notify error: {:?}", e);
                SplitDriverError::BleError(1)
            })?;
        Ok(buf.len())
    }
}

/// Advertise for DFU mode
#[cfg(feature = "dfu_ble")]
async fn split_peripheral_advertise_dfu<'a, 'b, C: Controller>(
    name: &str,
    peripheral: &mut Peripheral<'a, C, DefaultPacketPool>,
    server: &'b BleSplitPeripheralServer<'_>,
) -> Result<GattConnection<'a, 'b, DefaultPacketPool>, BleHostError<C::Error>> {
    let mut advertiser_data = [0; 31];
    let advertisement = get_dfu_advertiser::<C>(name, &mut advertiser_data)?;
    let advertiser = peripheral
        .advertise(&AdvertisementParameters::default(), advertisement)
        .await?;
    match with_timeout(Duration::from_secs(300), advertiser.accept()).await {
        Ok(re) => Ok(re?.with_attribute_server(server)?),
        Err(_e) => Err(BleHostError::BleHost(Error::Timeout)),
    }
}

#[cfg(feature = "dfu_ble")]
fn get_dfu_advertiser<'a, C: Controller>(
    name: &str,
    advertiser_data: &'a mut [u8; 31],
) -> Result<Advertisement<'a>, BleHostError<C::Error>> {
    AdStructure::encode_slice(
        &[
            AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
            AdStructure::CompleteLocalName(name.as_bytes()),
        ],
        &mut advertiser_data[..],
    )?;
    Ok(Advertisement::ConnectableScannableUndirected {
        adv_data: &advertiser_data[..],
        scan_data: &[],
    })
}

/// Run the DFU GATT event loop. Returns when disconnected or after DFU reset.
#[cfg(feature = "dfu_ble")]
async fn run_dfu_mode<
    'stack_ref,
    'stack_inner,
    'server,
    'c,
    C: Controller + ControllerCmdSync<LeReadLocalSupportedFeatures>,
    P: PacketPool,
>(
    driver: &mut BleSplitPeripheralDriver<'stack_ref, 'stack_inner, 'server, 'c, C, P>,
    conn: &'c GattConnection<'stack_ref, 'server, P>,
) {
    loop {
        match conn.next().await {
            GattConnectionEvent::Disconnected { reason } => {
                info!("DFU mode: disconnected ({:?})", reason);
                #[cfg(all(feature = "dfu_ble", any(feature = "dfu_nrf", feature = "dfu_rp")))]
                if let Some(ref handler) = driver.dfu_handler {
                    if handler.is_complete() {
                        info!("ble dfu: finalizing on disconnect (complete={})", handler.is_complete());
                        crate::dfu::mark_updated_and_reset();
                    }
                }
                break;
            }
            GattConnectionEvent::Gatt { event: gatt_event } => {
                #[cfg(feature = "dfu_ble")]
                if let GattEvent::Write(event) = &gatt_event {
                    let mut data_buf = [0u8; 247];
                    let data_len = event.with_data(|_, data| {
                        let n = data.len().min(data_buf.len());
                        data_buf[..n].copy_from_slice(&data[..n]);
                        data.len()
                    });
                    let data = &data_buf[..data_len.min(data_buf.len())];
                    driver.handle_dfu_write(event.handle(), data).await;
                }
                match gatt_event.accept() {
                    Ok(r) => r.send().await,
                    Err(e) => warn!("[dfu] error sending response: {:?}", e),
                }
            }
            _ => {}
        }
    }
}

/// Initialize and run the nRF peripheral keyboard service via BLE.
///
/// # Arguments
///
/// * `id` - The id of the peripheral
/// * `stack` - The stack to use
/// * `name` - The device name for BLE advertisement (scan response)
fn peripheral_name(id: usize) -> &'static str {
    const NAMES: [&str; 4] = ["per0", "per1", "per2", "per3"];
    NAMES.get(id).copied().unwrap_or("per")
}

/// Initialize a BLE split peripheral, advertise, connect to the central, and
/// run the split-peripheral protocol forever.
///
/// When `dfu_ble` is enabled an additional GATT DFU service is exposed
/// alongside the split‑peripheral characteristics, so the firmware can be
/// updated over BLE using [B.O.L.T] or another Nordic DFU tool.
///
/// On first boot the peripheral scans for its central; on subsequent boots
/// it reads the central's BLE address from storage and reconnects directly.
/// If the central is unavailable the peripheral falls back to a dedicated
/// DFU advertising mode (advertising its device name) after a 10 s timeout.
///
/// # Parameters
///
/// * `id` — Peripheral index (0, 1, …).  Must match the `peripheral_addrs`
///   configured on the central.
/// * `stack` — BLE stack obtained from [`build_ble_stack`].
/// * `name` — Device name used for the dedicated DFU advertising fallback
///   (only required when `dfu_ble` is enabled).
///
/// [B.O.L.T]: https://codeberg.org/Schievel/bolt
/// [`build_ble_stack`]: crate::ble::build_ble_stack
pub async fn initialize_nrf_ble_split_peripheral_and_run<
    'b,
    's: 'b,
    C: Controller + ControllerCmdAsync<LeSetPhy> + ControllerCmdSync<LeReadLocalSupportedFeatures>,
>(
    id: usize,
    stack: &'b Stack<'s, C, DefaultPacketPool>,
    #[cfg(feature = "dfu_ble")] name: &'static str,
) {
    #[cfg(not(feature = "dfu_ble"))]
    let name = peripheral_name(id);
    publish_event(CentralConnectedEvent { connected: false });

    let mut peripheral = stack.peripheral();
    let runner = stack.runner();

    // First, read central address from storage
    let mut central_addr = crate::storage::read_peer_address(0)
        .await
        .filter(|a| a.is_valid)
        .map(|a| a.address);

    let peri_task = async {
        let server = BleSplitPeripheralServer::new_default(name).unwrap();
        loop {
            update_status(|c| *c = ConnectionStatus::new());
            publish_event(CentralConnectedEvent { connected: false });
            match split_peripheral_advertise(id, central_addr, &mut peripheral, &server, name).await {
                Ok(conn) => {
                    info!("Connected to the central / host");
                    publish_event(CentralConnectedEvent { connected: true });

                    #[cfg(feature = "dfu_ble")]
                    let dfu_handler = crate::dfu::get_manager().map(DfuFlash::make_dfu_handler);

                    let driver = BleSplitPeripheralDriver::new(
                        &server,
                        &conn,
                        stack,
                        #[cfg(feature = "dfu_ble")]
                        dfu_handler,
                    );
                    // Use low-latency connection params for DFU performance
                    crate::ble::update_conn_params(
                        stack,
                        conn.raw(),
                        &RequestedConnParams {
                            min_connection_interval: Duration::from_micros(7500),
                            max_connection_interval: Duration::from_micros(7500),
                            max_latency: 0,
                            min_event_length: Duration::from_secs(0),
                            max_event_length: Duration::from_secs(0),
                            supervision_timeout: Duration::from_secs(20),
                        },
                    )
                    .await;
                    let mut peripheral = SplitPeripheral::new(driver);
                    let new_addr = conn.raw().peer_address().addr.into_inner();
                    if central_addr != Some(new_addr) {
                        info!("Saving central address to storage");
                        if crate::storage::write_peer_address(PeerAddress {
                            peer_id: 0,
                            is_valid: true,
                            address: new_addr,
                        })
                        .await
                        {
                            central_addr = Some(new_addr);
                        }
                    }
                    peripheral.run().await;
                    info!("Disconnected");
                }
                Err(e) => {
                    #[cfg(feature = "defmt")]
                    let e = defmt::Debug2Format(&e);
                    error!("Advertise error: {:?}", e);
                    Timer::after_millis(500).await;
                    continue;
                }
            };
        }
    };

    join(ble_task(runner), peri_task).await;
}

/// Create an advertiser to use to connect to a BLE Central, and wait for it to connect.
async fn split_peripheral_advertise<'a, 'b, C: Controller>(
    id: usize,
    central_addr: Option<[u8; 6]>,
    peripheral: &mut Peripheral<'a, C, DefaultPacketPool>,
    server: &'b BleSplitPeripheralServer<'_>,
    name: &str,
) -> Result<GattConnection<'a, 'b, DefaultPacketPool>, BleHostError<C::Error>> {
    let mut advertiser_data = [0; 31];
    let mut scan_data = [0; 31];
    let advertisement = get_peri_advertiser::<C>(id, central_addr, &mut advertiser_data, &mut scan_data, name)?;

    let advertiser = peripheral
        .advertise(&AdvertisementParameters::default(), advertisement)
        .await?;

    match with_timeout(Duration::from_secs(10), advertiser.accept()).await {
        Ok(conn_res) => {
            let conn = conn_res?.with_attribute_server(server)?;
            info!("[adv] connection established");
            Ok(conn)
        }
        Err(_) => {
            warn!("[adv] Try update central_addr");
            // Advertise without central addr
            let advertisement = get_peri_advertiser::<C>(id, None, &mut advertiser_data, &mut scan_data, name)?;
            let advertiser = peripheral
                .advertise(&AdvertisementParameters::default(), advertisement)
                .await?;
            match with_timeout(Duration::from_secs(300), advertiser.accept()).await {
                Ok(re) => Ok(re?.with_attribute_server(server)?),
                Err(_e) => Err(BleHostError::BleHost(Error::Timeout)),
            }
        }
    }
}

fn get_peri_advertiser<'a, C: Controller>(
    id: usize,
    central_addr: Option<[u8; 6]>,
    advertiser_data: &'a mut [u8; 31],
    scan_data: &'a mut [u8; 31],
    name: &str,
) -> Result<Advertisement<'a>, BleHostError<C::Error>> {
    let advertisement = match central_addr {
        Some(addr) => Advertisement::ConnectableNonscannableDirected {
            peer: Address::random(addr),
        },
        None => {
            info!("No central address provided, so we advertise as undirected");
            // No central address provided, so we advertise as undirected
            AdStructure::encode_slice(
                &[
                    AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
                    AdStructure::CompleteServiceUuids128(&[
                        // uuid: 4dd5fbaa-18e5-4b07-bf0a-353698659946
                        [
                            70u8, 153u8, 101u8, 152u8, 54u8, 53u8, 10u8, 191u8, 7u8, 75u8, 229u8, 24u8, 170u8, 251u8,
                            213u8, 77u8,
                        ],
                    ]),
                    AdStructure::ManufacturerSpecificData {
                        company_identifier: 0xe118,
                        payload: &[id as u8],
                    },
                ],
                &mut advertiser_data[..],
            )?;

            // Scan response with device name for DFU discovery
            AdStructure::encode_slice(&[AdStructure::CompleteLocalName(name.as_bytes())], &mut scan_data[..])?;

            trace!("Advertising data: {:?}", advertiser_data);
            Advertisement::ConnectableScannableUndirected {
                adv_data: &advertiser_data[..],
                scan_data: &scan_data[..],
            }
        }
    };
    Ok(advertisement)
}

/// This is a background task that is required to run forever alongside any other BLE tasks.
async fn ble_task<C: Controller + ControllerCmdAsync<LeSetPhy>, P: PacketPool>(mut runner: Runner<'_, C, P>) {
    loop {
        if let Err(e) = runner.run().await {
            error!("[ble_task] runner.run() error: {:?}", e);
            embassy_time::Timer::after_millis(100).await;
        }
    }
}
