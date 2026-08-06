//! One dongle link: claim a slot (or a pairing candidate), connect, secure,
//! handshake over Rynk, then forward in both directions until disconnect.

use bt_hci::cmd::le::{LeReadLocalSupportedFeatures, LeSetPhy, LeSetScanParams};
use bt_hci::controller::{ControllerCmdAsync, ControllerCmdSync};
use bt_hci::param::{AddrKind, BdAddr};
use embassy_futures::select::{Either, Either3, select, select3};
use embassy_time::{Duration, Timer, with_timeout};
use rmk_types::protocol::rynk::{
    Cmd, Deframer, DeviceInfo, ProtocolVersion, RYNK_BLE_CHUNK_SIZE, RYNK_HEADER_SIZE, RYNK_INPUT_CHAR_UUID,
    RYNK_OUTPUT_CHAR_UUID, RYNK_SERVICE_UUID, RynkError, RynkHeader, encode_frame,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use trouble_host::prelude::*;

use super::{LinkState, Slot, merge, router};
use crate::ble::{update_ble_phy, update_conn_params};
use crate::channel::send_hid_report;
use crate::event::{EventSubscriber, LedIndicatorEvent, SubscribableEvent};
use crate::hid::Report;
use crate::storage::FlashOperationMessage;

/// Discovered services on this build's keyboards; HID + Rynk.
const MAX_SERVICES: usize = 8;
/// Characteristic budget for the HID service discovery.
const MAX_CHARACTERISTICS: usize = 16;

type Client<'a, C> = GattClient<'a, C, DefaultPacketPool, MAX_SERVICES>;

/// What the idle loop picked up.
enum Job {
    /// Pairing-window candidate: connect, pair, then allocate a slot.
    Pair { addr: (AddrKind, BdAddr) },
    /// A bonded slot to reconnect and serve.
    Serve { slot: u8, addr: Address },
}

pub(super) async fn link_task<C>(idx: u8, stack: &Stack<'_, C, DefaultPacketPool>) -> !
where
    C: Controller
        + ControllerCmdAsync<LeSetPhy>
        + ControllerCmdSync<LeReadLocalSupportedFeatures>
        + ControllerCmdSync<LeSetScanParams>,
{
    loop {
        // Quiet: a claim doesn't change the wire-visible snapshot, and the idle
        // poll must not spam the router's DongleSlotsChange topic.
        let Some(job) = super::update_slots_quiet(|t| claim(t, idx)) else {
            Timer::after_millis(500).await;
            continue;
        };
        run_link(idx, stack, job).await;
        Timer::after_millis(500).await;
    }
}

/// Claim the pairing candidate, else the next claimable bonded slot,
/// round-robin from this link's own index so idle links don't contend.
fn claim(t: &mut super::SlotTable, idx: u8) -> Option<Job> {
    if let Some(addr) = t.pending_pair.take() {
        return Some(Job::Pair { addr });
    }
    let n = t.slots.len();
    for k in 0..n {
        let i = (idx as usize + k) % n;
        let s = &mut t.slots[i];
        if s.link == LinkState::Free
            && !s.version_bad
            && let Some(bond) = &s.bond
        {
            let addr = bond.identity.addr;
            s.link = LinkState::Claimed(idx);
            return Some(Job::Serve { slot: i as u8, addr });
        }
    }
    None
}

async fn run_link<C>(idx: u8, stack: &Stack<'_, C, DefaultPacketPool>, job: Job)
where
    C: Controller
        + ControllerCmdAsync<LeSetPhy>
        + ControllerCmdSync<LeReadLocalSupportedFeatures>
        + ControllerCmdSync<LeSetScanParams>,
{
    let (address, bonded_slot) = match &job {
        Job::Pair { addr } => (
            Address {
                kind: addr.0,
                addr: addr.1,
            },
            None,
        ),
        Job::Serve { slot, addr } => (*addr, Some(*slot)),
    };

    let release = |t: &mut super::SlotTable| {
        if let Some(slot) = bonded_slot {
            t.slots[slot as usize].link = LinkState::Free;
        }
    };

    let conn = match connect(stack, address).await {
        Some(conn) => conn,
        None => {
            super::update_slots(release);
            return;
        }
    };

    match secure(&conn, bonded_slot.is_none()).await {
        Ok(_) => {}
        Err(SecureError::KeyRejected) => {
            // The keyboard re-bonded elsewhere; our record can never work again
            // (design §2.5). Forget the slot entirely.
            if let Some(slot) = bonded_slot {
                warn!("[dongle] link {}: encryption rejected, clearing slot {}", idx, slot);
                forget_slot(slot).await;
            }
            conn.disconnect();
            return;
        }
        Err(_) => {
            info!("[dongle] link {}: securing failed", idx);
            super::update_slots(release);
            conn.disconnect();
            return;
        }
    }

    let client = match Client::new(stack, &conn).await {
        Ok(client) => client,
        Err(_) => {
            super::update_slots(release);
            conn.disconnect();
            return;
        }
    };

    // The client task pumps notifications; everything else runs beside it and
    // wins the select when the session ends.
    let session = async {
        match connected_session(idx, stack, &conn, &client, bonded_slot).await {
            Ok(slot) => Some(slot),
            Err(e) => {
                info!("[dongle] link {}: session setup failed: {:?}", idx, e);
                None
            }
        }
    };
    let served_slot = match select(client.task(), session).await {
        Either::First(_) => None,
        Either::Second(slot) => slot,
    };

    conn.disconnect();

    // Clean up whatever state the session left behind.
    let merged = merge::clear_link(idx);
    send_hid_report(Report::KeyboardReport(merged)).await;
    send_hid_report(Report::MouseReport(merge::parse_mouse(&[0; 5]))).await;
    send_hid_report(Report::MediaKeyboardReport(merge::parse_media(&[0, 0]))).await;
    send_hid_report(Report::SystemControlReport(merge::parse_system(&[0]))).await;

    let mut meta = None;
    super::update_slots(|t| {
        release(t);
        if let Some(slot) = served_slot {
            let s = &mut t.slots[slot as usize];
            if s.link == LinkState::Connected(idx) {
                s.link = LinkState::Free;
            }
            // Recency also bumps on clean disconnect, so a long-lived
            // connection isn't the eviction victim after a replug (§4.8).
            if s.bond.is_some() {
                let seen = t.bump_last_seen(slot as usize);
                meta = Some((slot, t.slots[slot as usize].name.clone(), seen));
            }
        }
    });
    if let Some((slot, name, seen)) = meta {
        super::persist_slot_meta(slot, name, seen).await;
    }
    info!("[dongle] link {}: disconnected", idx);
}

async fn connect<'b, 's: 'b, C>(
    stack: &'b Stack<'s, C, DefaultPacketPool>,
    address: Address,
) -> Option<Connection<'b, DefaultPacketPool>>
where
    C: Controller + ControllerCmdSync<LeSetScanParams>,
{
    let mut central = stack.central();
    // Secure-phase parameters. Peripheral latency MUST be 0 here: with
    // latency 30, an esp-radio keyboard's controller loses link sync during
    // the central's ~400ms P-256 pauses in SMP and both sides hit supervision
    // timeout (bisected on hardware; the 7.5 ms interval itself is fine).
    // The generous supervision timeout rides out slow keyboard-side ECDH.
    // `connected_session` applies the typing parameters once serving starts.
    let connect_params = RequestedConnParams {
        min_connection_interval: Duration::from_micros(7500),
        max_connection_interval: Duration::from_micros(7500),
        max_latency: 0,
        supervision_timeout: Duration::from_secs(30),
        ..Default::default()
    };
    let config = ConnectConfig {
        connect_params,
        scan_config: ScanConfig {
            filter_accept_list: &[address],
            active: false,
            interval: Duration::from_millis(100),
            window: Duration::from_millis(60),
            ..Default::default()
        },
    };
    super::wait_for_stack_started().await;
    let attempt = async {
        if let Ok(_guard) = super::SCANNING_MUTEX.try_lock() {
            central.connect(&config).await
        } else {
            // The pairing window is scanning; ask it to yield the controller.
            super::STOP_SCANNING.signal(());
            let _guard = super::SCANNING_MUTEX.lock().await;
            Timer::after_millis(100).await;
            central.connect(&config).await
        }
    };
    match with_timeout(Duration::from_secs(15), attempt).await {
        Ok(Ok(conn)) => Some(conn),
        Ok(Err(e)) => {
            #[cfg(feature = "defmt")]
            let e = defmt::Debug2Format(&e);
            debug!("[dongle] connect error: {:?}", e);
            None
        }
        Err(_) => None,
    }
}

/// 7.5 ms interval — the same latency budget as a split link. The generous
/// supervision timeout trades reconnect latency after a dongle power-cycle
/// (rare) for radio-interference tolerance during normal use: the keyboard
/// only starts its directed reconnect advertising once this timer expires.
fn link_conn_params() -> RequestedConnParams {
    RequestedConnParams {
        min_connection_interval: Duration::from_micros(7500),
        max_connection_interval: Duration::from_micros(7500),
        max_latency: 30,
        supervision_timeout: Duration::from_secs(10),
        ..Default::default()
    }
}

enum SecureError {
    /// The peer refused our key or pairing: the bond is dead.
    KeyRejected,
    Disconnected,
    Timeout,
}

/// Drive the link to an encrypted state. `pairing` runs a fresh LESC Just
/// Works pairing and requires a bond in the outcome; otherwise the stored
/// bond's LTK is used.
async fn secure(
    conn: &Connection<'_, DefaultPacketPool>,
    pairing: bool,
) -> Result<Option<BondInformation>, SecureError> {
    if let Err(e) = conn.set_bondable(true) {
        warn!("[dongle] set_bondable error: {:?}", e);
    }
    if conn.request_security().is_err() {
        return Err(SecureError::Disconnected);
    }
    let wait = async {
        let mut bond = None;
        let mut encrypted = false;
        loop {
            match conn.next().await {
                ConnectionEvent::Disconnected { reason } => {
                    // 0x05 = authentication failure, 0x06 = PIN or key missing:
                    // the peer no longer holds our keys.
                    let code = reason.into_inner();
                    return Err(if code == 0x05 || code == 0x06 {
                        SecureError::KeyRejected
                    } else {
                        SecureError::Disconnected
                    });
                }
                ConnectionEvent::PairingFailed(e) => {
                    warn!("[dongle] pairing failed: {:?}", e);
                    return Err(SecureError::KeyRejected);
                }
                ConnectionEvent::PairingComplete { bond: b, .. } => bond = b,
                ConnectionEvent::Encrypted { bond: b, .. } => {
                    encrypted = true;
                    if bond.is_none() {
                        bond = b;
                    }
                }
                _ => {}
            }
            if encrypted && (!pairing || bond.is_some()) {
                return Ok(bond);
            }
        }
    };
    // Generous: LESC on a slow keyboard MCU can take many seconds of P-256 math.
    match with_timeout(Duration::from_secs(25), wait).await {
        Ok(r) => r,
        Err(_) => Err(SecureError::Timeout),
    }
}

/// Errors that end a session before serving starts.
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
enum SessionError {
    Discovery,
    VersionMismatch,
    Handshake,
    SlotsFull,
}

/// Discovered characteristic handles on the keyboard.
struct KeyboardChars {
    input_keyboard: Characteristic<[u8]>,
    output_keyboard: Characteristic<[u8]>,
    mouse: Characteristic<[u8]>,
    media: Characteristic<[u8]>,
    system: Characteristic<[u8]>,
    rynk_input: Characteristic<[u8]>,
    rynk_output: Characteristic<[u8]>,
    dongle_ctrl: Option<Characteristic<[u8]>>,
}

/// Discover → subscribe → handshake → commit the slot → serve. Returns the
/// served slot for the caller's cleanup, or the setup error.
async fn connected_session<C>(
    idx: u8,
    stack: &Stack<'_, C, DefaultPacketPool>,
    conn: &Connection<'_, DefaultPacketPool>,
    client: &Client<'_, C>,
    bonded_slot: Option<u8>,
) -> Result<u8, SessionError>
where
    C: Controller + ControllerCmdAsync<LeSetPhy> + ControllerCmdSync<LeReadLocalSupportedFeatures>,
{
    // Same latency setup as a split link: 2M PHY + 7.5 ms interval.
    update_ble_phy(stack, conn, PhyKind::Le2M).await;
    update_conn_params(stack, conn, &link_conn_params()).await;

    let chars = discover(client).await.ok_or(SessionError::Discovery)?;

    // Subscribe by writing each CCCD once, then take a single catch-all
    // listener — one queue, routed by handle.
    for ch in [
        &chars.input_keyboard,
        &chars.mouse,
        &chars.media,
        &chars.system,
        &chars.rynk_input,
    ]
    .into_iter()
    .chain(chars.dongle_ctrl.as_ref())
    {
        if let Some(cccd) = ch.cccd_handle
            && client.write_handle(cccd, &[0x01, 0x00]).await.is_err()
        {
            return Err(SessionError::Discovery);
        }
    }
    let mut listener = client.listen_all().map_err(|_| SessionError::Discovery)?;

    // Version gate + identity, over the keyboard's own Rynk service. The
    // config pass-through only opens after this, so no seq can collide.
    let chunk = rynk_chunk_size(conn);
    let version: ProtocolVersion = rynk_request(client, &mut listener, &chars, chunk, Cmd::GetVersion, 0, &())
        .await
        .map_err(|_| SessionError::Handshake)?;
    if version.major != ProtocolVersion::CURRENT.major {
        warn!(
            "[dongle] link {}: keyboard protocol {}.{} != ours",
            idx, version.major, version.minor
        );
        if let Some(slot) = bonded_slot {
            super::update_slots(|t| t.slots[slot as usize].version_bad = true);
        }
        return Err(SessionError::VersionMismatch);
    }
    let info: DeviceInfo = rynk_request(client, &mut listener, &chars, chunk, Cmd::GetDeviceInfo, 1, &())
        .await
        .map_err(|_| SessionError::Handshake)?;

    // Commit: allocate a slot for a fresh pairing, refresh a bonded one.
    let slot = commit_slot(idx, stack, conn, bonded_slot, info.product_name).await?;

    info!("[dongle] link {}: serving slot {}", idx, slot);
    serve(idx, slot, stack, conn, client, &mut listener, &chars).await;
    Ok(slot)
}

/// Write the handshake outcome into the slot table and persist it.
async fn commit_slot<C: Controller>(
    idx: u8,
    stack: &Stack<'_, C, DefaultPacketPool>,
    conn: &Connection<'_, DefaultPacketPool>,
    bonded_slot: Option<u8>,
    name: heapless::String<{ rmk_types::protocol::rynk::DEVICE_INFO_STRING_SIZE }>,
) -> Result<u8, SessionError> {
    let mut evicted = None;
    let committed = super::update_slots(|t| {
        let slot = match bonded_slot {
            Some(slot) => slot as usize,
            None => {
                let slot = t.allocate().ok_or(SessionError::SlotsFull)?;
                evicted = t.slots[slot].bond.take().map(|b| b.identity);
                // The bond was stored by the Encrypted/PairingComplete event;
                // fetch it from the stack's view via the connection.
                t.slots[slot] = Slot {
                    bond: None, // filled below
                    name: heapless::String::new(),
                    last_seen: 0,
                    link: LinkState::Free,
                    version_bad: false,
                };
                slot
            }
        };
        let s = &mut t.slots[slot];
        s.name = name.clone();
        s.link = LinkState::Connected(idx);
        let seen = t.bump_last_seen(slot);
        Ok((slot as u8, seen))
    });
    let (slot, seen) = committed?;

    if let Some(identity) = evicted {
        info!("[dongle] evicting stale bond {:?}", identity.addr);
        let _ = super::REMOVED_BONDS.try_send(identity);
    }

    if bonded_slot.is_none() {
        // Fresh pairing: persist the new bond and register it with the stack.
        let identity = conn.peer_identity();
        let bond = stack
            .with_bond_information(|bonds| bonds.iter().find(|b| b.identity == identity).cloned())
            .ok_or(SessionError::Handshake)?;
        super::update_slots(|t| t.slots[slot as usize].bond = Some(bond.clone()));
        crate::channel::FLASH_CHANNEL
            .send(FlashOperationMessage::ProfileInfo(crate::ble::profile::ProfileInfo {
                slot_num: slot,
                removed: false,
                info: bond,
                cccd_table: heapless::Vec::new(),
            }))
            .await;
    }
    super::persist_slot_meta(slot, name, seen).await;
    Ok(slot)
}

/// Discover the HID and Rynk services. The four HID input reports
/// share UUID 0x2A4D; both ends are RMK, so their declaration order is fixed:
/// input_keyboard, output_keyboard, mouse, media, system.
async fn discover<C: Controller>(client: &Client<'_, C>) -> Option<KeyboardChars> {
    let hid = client
        .services_by_uuid(&Uuid::new_short(0x1812))
        .await
        .ok()?
        .first()
        .cloned()?;
    let all: heapless::Vec<Characteristic<[u8]>, MAX_CHARACTERISTICS> =
        client.characteristics::<MAX_CHARACTERISTICS>(&hid).await.ok()?;
    let report_uuid = Uuid::new_short(0x2A4D);
    let mut reports = all.into_iter().filter(|c| c.uuid == report_uuid);
    let input_keyboard = reports.next()?;
    let output_keyboard = reports.next()?;
    let mouse = reports.next()?;
    let media = reports.next()?;
    let system = reports.next()?;

    let rynk = client
        .services_by_uuid(&Uuid::new_long(RYNK_SERVICE_UUID.to_le_bytes()))
        .await
        .ok()?
        .first()
        .cloned()?;
    let rynk_input = client
        .characteristic_by_uuid::<[u8]>(&rynk, &Uuid::new_long(RYNK_INPUT_CHAR_UUID.to_le_bytes()))
        .await
        .ok()?;
    let rynk_output = client
        .characteristic_by_uuid::<[u8]>(&rynk, &Uuid::new_long(RYNK_OUTPUT_CHAR_UUID.to_le_bytes()))
        .await
        .ok()?;
    let dongle_ctrl = client
        .characteristic_by_uuid::<[u8]>(
            &rynk,
            &Uuid::new_long(rmk_types::protocol::rynk::RYNK_DONGLE_CTRL_CHAR_UUID.to_le_bytes()),
        )
        .await
        .ok();

    Some(KeyboardChars {
        input_keyboard,
        output_keyboard,
        mouse,
        media,
        system,
        rynk_input,
        rynk_output,
        dongle_ctrl,
    })
}

/// Largest single write/notify chunk on the Rynk characteristics.
fn rynk_chunk_size(conn: &Connection<'_, DefaultPacketPool>) -> usize {
    RYNK_BLE_CHUNK_SIZE
        .min((conn.att_mtu() as usize).saturating_sub(3))
        .max(1)
}

/// One dongle-originated Rynk request over the keyboard's GATT service.
async fn rynk_request<C: Controller, T: DeserializeOwned>(
    client: &Client<'_, C>,
    listener: &mut NotificationListener<'_, 512>,
    chars: &KeyboardChars,
    chunk: usize,
    cmd: Cmd,
    seq: u8,
    request: &impl Serialize,
) -> Result<T, RynkError> {
    let mut buf = [0u8; 512];
    let n = encode_frame(&mut buf, RynkHeader { cmd, seq }, request)?;
    for part in buf[..n].chunks(chunk) {
        client
            .write_characteristic_without_response(&chars.rynk_output, part)
            .await
            .map_err(|_| RynkError::NotReady)?;
    }

    let mut rx = [0u8; 512];
    let mut df = Deframer::new();
    let read = async {
        loop {
            let notification = listener.next().await;
            if notification.handle() != chars.rynk_input.handle {
                continue; // typing may already be flowing; not ours
            }
            let data = notification.as_ref();
            let tail = df.tail(&mut rx);
            let take = data.len().min(tail.len());
            tail[..take].copy_from_slice(&data[..take]);
            df.commit(take);
            while let Some(len) = df.next(&mut rx) {
                let header = RynkHeader::parse(rx[..RYNK_HEADER_SIZE].try_into().unwrap());
                if header.cmd == cmd && header.seq == seq {
                    return postcard::from_bytes::<Result<T, RynkError>>(&rx[RYNK_HEADER_SIZE..len])
                        .map_err(|_| RynkError::Malformed)?;
                }
            }
        }
    };
    match with_timeout(Duration::from_secs(5), read).await {
        Ok(r) => r,
        Err(_) => Err(RynkError::NotReady),
    }
}

/// Forward until the link dies: notifications out to USB/router, LED state and
/// router frames back to the keyboard, plus a watchdog for disconnect/forget.
async fn serve<C: Controller>(
    idx: u8,
    slot: u8,
    stack: &Stack<'_, C, DefaultPacketPool>,
    conn: &Connection<'_, DefaultPacketPool>,
    client: &Client<'_, C>,
    listener: &mut NotificationListener<'_, 512>,
    chars: &KeyboardChars,
) {
    let chunk = rynk_chunk_size(conn);

    let rx = async {
        loop {
            let notification = listener.next().await;
            let handle = notification.handle();
            let data = notification.as_ref();
            if handle == chars.input_keyboard.handle && data.len() >= 8 {
                let merged = merge::update_keyboard(idx, data[..8].try_into().unwrap());
                send_hid_report(Report::KeyboardReport(merged)).await;
            } else if handle == chars.mouse.handle && data.len() >= 5 {
                send_hid_report(Report::MouseReport(merge::parse_mouse(data))).await;
            } else if handle == chars.media.handle && data.len() >= 2 {
                send_hid_report(Report::MediaKeyboardReport(merge::parse_media(data))).await;
            } else if handle == chars.system.handle && !data.is_empty() {
                send_hid_report(Report::SystemControlReport(merge::parse_system(data))).await;
            } else if handle == chars.rynk_input.handle {
                router::forward_to_host(slot, data);
            } else if chars.dongle_ctrl.as_ref().is_some_and(|c| c.handle == handle) {
                if data.first() == Some(&rmk_types::protocol::rynk::DONGLE_CTRL_OPEN_PAIRING_WINDOW) {
                    info!("[dongle] keyboard on slot {} authorized a pairing window", slot);
                    super::AUTH_WINDOW_SIGNAL.signal(());
                }
            }
        }
    };

    let tx = async {
        let mut led_events = LedIndicatorEvent::subscriber();
        loop {
            match select(led_events.next_event(), router::ROUTER_TX[idx as usize].receive()).await {
                Either::First(event) => {
                    let _ = client
                        .write_characteristic_without_response(&chars.output_keyboard, &[event.0.into_bits()])
                        .await;
                }
                Either::Second(frame) => {
                    for part in frame.chunks(chunk) {
                        if client
                            .write_characteristic_without_response(&chars.rynk_output, part)
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                }
            }
        }
    };

    // Consumes connection events (a peripheral's parameter request must be
    // answered or trouble drops it) and ends the serve on link loss or when
    // the router forgets this slot.
    let watch = async {
        let events = async {
            loop {
                match conn.next().await {
                    ConnectionEvent::Disconnected { .. } => return,
                    ConnectionEvent::RequestConnectionParams(req) => {
                        // The keyboard asks for its host-link parameters; they
                        // match our typing parameters, so accept.
                        if let Err(e) = req.accept(None, stack).await {
                            debug!("[dongle] conn param accept error: {:?}", e);
                        }
                    }
                    _ => {}
                }
            }
        };
        let forget = async {
            loop {
                Timer::after_millis(500).await;
                if super::read_slots(|t| t.slots[slot as usize].bond.is_none()) {
                    info!("[dongle] slot {} forgotten, dropping its link", slot);
                    conn.disconnect();
                    return;
                }
            }
        };
        select(events, forget).await;
    };

    match select3(rx, tx, watch).await {
        Either3::First(_) | Either3::Second(_) | Either3::Third(_) => {}
    }
}

/// Forget a slot completely: RAM entry, persisted bond, and the stack's copy.
async fn forget_slot(slot: u8) {
    let identity = super::update_slots(|t| {
        let s = &mut t.slots[slot as usize];
        s.name = heapless::String::new();
        s.version_bad = false;
        s.link = LinkState::Free;
        s.bond.take().map(|b| b.identity)
    });
    if let Some(identity) = identity {
        let _ = super::REMOVED_BONDS.try_send(identity);
    }
    crate::channel::FLASH_CHANNEL
        .send(FlashOperationMessage::ClearSlot(slot))
        .await;
}
