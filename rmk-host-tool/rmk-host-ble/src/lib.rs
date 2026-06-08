//! BLE GATT transport using `bluest`.
//!
//! Reuses an already connected keyboard when possible, then falls back to a
//! short scan by device name.
//!
//! The already-connected path attaches to every OS-connected device exposing a
//! HID/Battery/Rynk service and probes it for the Rynk GATT service; a non-Rynk
//! peripheral fails service discovery and is skipped, but it is briefly attached
//! during the probe.

use std::sync::Arc;
use std::time::Duration;

use bluest::{Adapter, Characteristic, Device, Uuid};
use futures::StreamExt;
use rmk_host::{Client, ConnectError, Transport, TransportError};
use rmk_types::protocol::rynk::RYNK_BLE_CHUNK_SIZE;
use tokio::sync::{mpsc, oneshot};

const RYNK_SERVICE_UUID: Uuid = Uuid::from_u128(rmk_types::protocol::rynk::RYNK_SERVICE_UUID);
const RYNK_INPUT_CHAR_UUID: Uuid = Uuid::from_u128(rmk_types::protocol::rynk::RYNK_INPUT_CHAR_UUID);
const RYNK_OUTPUT_CHAR_UUID: Uuid = Uuid::from_u128(rmk_types::protocol::rynk::RYNK_OUTPUT_CHAR_UUID);

/// Default BLE name hint.
const RMK_NAME_HINT: &str = "RMK";

/// Protocol handshake timeout after the BLE link is attached.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(1);

/// ATT-minimum MTU payload.
const BLE_SAFE_WRITE: usize = 20;

/// Notify bridge channel depth.
const BRIDGE_CHANNEL_CAPACITY: usize = 32;

/// Services used when querying already connected devices.
const CONNECTED_LOOKUP_SERVICES: &[Uuid] = &[
    Uuid::from_u128(0x00001812_0000_1000_8000_00805f9b34fb), // Human Interface Device
    Uuid::from_u128(0x0000180f_0000_1000_8000_00805f9b34fb), // Battery
    RYNK_SERVICE_UUID,
];

/// Attached Rynk GATT link.
pub struct BleTransport {
    output_char: Characteristic,
    write_chunk: usize,
    chunks: mpsc::Receiver<Vec<u8>>,
    /// Notification bridge task.
    bridge: tokio::task::JoinHandle<()>,
    /// The connected device's name, if it advertised one.
    name: Option<String>,
    // Keep the OS connection alive.
    _adapter: Adapter,
    _device: Device,
}

impl BleTransport {
    /// Connect using the default `"RMK"` name hint.
    pub async fn connect() -> Result<Self, TransportError> {
        Self::connect_with_name(RMK_NAME_HINT).await
    }

    /// The connected keyboard's BLE name, if any.
    pub fn device_name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Connect to a keyboard whose BLE name contains `name_hint`.
    pub async fn connect_with_name(name_hint: &str) -> Result<Self, TransportError> {
        let adapter = Adapter::default()
            .await
            .ok_or_else(|| TransportError::DeviceNotFound("no BLE adapter".into()))?;
        adapter
            .wait_available()
            .await
            .map_err(|e| TransportError::Io(e.to_string()))?;

        // Prefer an already-connected device. A connected device is radio-silent
        // — it won't reappear in the scan below — so we must not skip a
        // service-matched one just because its GAP name is momentarily unreadable
        // or doesn't carry the hint. Try every candidate, best name-match first,
        // and only fall through to the scan once they've all failed to attach.
        let mut connected = adapter
            .connected_devices_with_services(CONNECTED_LOOKUP_SERVICES)
            .await
            .map_err(|e| TransportError::Io(e.to_string()))?;
        // Stable sort: name-matches (key `false`) first, the rest in discovery order.
        connected.sort_by_key(|d| !d.name().is_ok_and(|n| n.contains(name_hint)));
        for d in connected {
            if let Ok(transport) = attach(&adapter, d).await {
                return Ok(transport);
            }
        }

        // Fallback: scan for an advertising keyboard.
        let mut scan = adapter.scan(&[]).await.map_err(|e| TransportError::Io(e.to_string()))?;
        let found = tokio::time::timeout(Duration::from_secs(10), async {
            while let Some(adv) = scan.next().await {
                let name = adv.adv_data.local_name.as_deref().unwrap_or("");
                if name.contains(name_hint) {
                    return Some(adv.device);
                }
            }
            None
        })
        .await
        .ok()
        .flatten();
        drop(scan);

        match found {
            Some(device) => attach(&adapter, device).await,
            None => Err(TransportError::DeviceNotFound(format!(
                "no connected or advertising BLE device whose name contains {name_hint:?}"
            ))),
        }
    }
}

impl Transport for BleTransport {
    async fn send(&mut self, frame: &[u8]) -> Result<(), TransportError> {
        for chunk in frame.chunks(self.write_chunk) {
            self.output_char
                .write_without_response(chunk)
                .await
                .map_err(|e| TransportError::Io(e.to_string()))?;
        }
        Ok(())
    }

    async fn recv(&mut self) -> Result<Vec<u8>, TransportError> {
        self.chunks.recv().await.ok_or(TransportError::Disconnected)
    }
}

impl Drop for BleTransport {
    fn drop(&mut self) {
        self.bridge.abort();
    }
}

/// Connect over BLE using the default `"RMK"` name hint.
pub async fn connect_ble() -> Result<Client<BleTransport>, ConnectError> {
    connect_transport(BleTransport::connect().await?).await
}

/// Connect over BLE to a keyboard whose name contains `name_hint`.
pub async fn connect_ble_name(name_hint: &str) -> Result<Client<BleTransport>, ConnectError> {
    connect_transport(BleTransport::connect_with_name(name_hint).await?).await
}

// Intentionally duplicated in `rmk-host-serial` rather than shared: `rmk-host`
// is deliberately runtime-free (no `tokio`, builds for `wasm32`), so the
// timeout wrapper can't live there. Each transport crate owns its own runtime.
async fn connect_transport(transport: BleTransport) -> Result<Client<BleTransport>, ConnectError> {
    tokio::time::timeout(HANDSHAKE_TIMEOUT, Client::connect(transport))
        .await
        .map_err(|_| ConnectError::Transport(TransportError::DeviceNotFound("handshake timed out".into())))?
}

/// Attach, discover characteristics, and subscribe to notifications.
async fn attach(adapter: &Adapter, device: Device) -> Result<BleTransport, TransportError> {
    // GATT can briefly fail after reconnect.
    let mut last_err = TransportError::Disconnected;
    for attempt in 0..6 {
        if attempt > 0 {
            log::debug!("rynk ble: attach retry {attempt}/5 after {last_err}");
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        if let Err(e) = adapter.connect_device(&device).await {
            last_err = TransportError::Io(format!("connect_device: {e}"));
            continue;
        }
        let (input, output) = match discover_chars(&device).await {
            Ok(pair) => pair,
            Err(e) => {
                last_err = e;
                continue;
            }
        };

        // Clamp to the firmware's characteristic capacity.
        let write_chunk = output
            .max_write_len()
            .unwrap_or(BLE_SAFE_WRITE)
            .clamp(BLE_SAFE_WRITE, RYNK_BLE_CHUNK_SIZE);

        // The bridge owns the characteristic because `notify()` borrows it.
        let (chunk_tx, chunk_rx) = mpsc::channel(BRIDGE_CHANNEL_CAPACITY);
        let (sub_tx, sub_rx) = oneshot::channel();
        let bridge = tokio::spawn(notify_bridge(Arc::new(input), chunk_tx, sub_tx));
        match sub_rx.await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                bridge.abort();
                last_err = e;
                continue;
            }
            Err(_) => {
                bridge.abort();
                last_err = TransportError::Disconnected;
                continue;
            }
        }

        return Ok(BleTransport {
            output_char: output,
            write_chunk,
            chunks: chunk_rx,
            bridge,
            name: device.name().ok(),
            _adapter: adapter.clone(),
            _device: device.clone(),
        });
    }
    Err(last_err)
}

/// Discover the Rynk service and its input/output characteristics.
async fn discover_chars(device: &Device) -> Result<(Characteristic, Characteristic), TransportError> {
    let service = device
        .discover_services_with_uuid(RYNK_SERVICE_UUID)
        .await
        .map_err(|e| TransportError::Io(e.to_string()))?
        .into_iter()
        .next()
        .ok_or_else(|| TransportError::DeviceNotFound("Rynk GATT service not found".into()))?;

    let mut input_char = None;
    let mut output_char = None;
    for c in service
        .discover_characteristics()
        .await
        .map_err(|e| TransportError::Io(e.to_string()))?
    {
        match c.uuid() {
            u if u == RYNK_INPUT_CHAR_UUID => input_char = Some(c),
            u if u == RYNK_OUTPUT_CHAR_UUID => output_char = Some(c),
            _ => {}
        }
    }
    let input = input_char.ok_or_else(|| TransportError::DeviceNotFound("input characteristic missing".into()))?;
    let output = output_char.ok_or_else(|| TransportError::DeviceNotFound("output characteristic missing".into()))?;
    Ok((input, output))
}

/// Forward GATT notifications into the transport channel.
async fn notify_bridge(
    input: Arc<Characteristic>,
    chunks: mpsc::Sender<Vec<u8>>,
    sub_tx: oneshot::Sender<Result<(), TransportError>>,
) {
    let mut notifications = match input.notify().await {
        Ok(n) => {
            let _ = sub_tx.send(Ok(()));
            n
        }
        Err(e) => {
            let _ = sub_tx.send(Err(TransportError::Io(e.to_string())));
            return;
        }
    };

    while let Some(item) = notifications.next().await {
        let chunk = match item {
            Ok(c) => c,
            Err(_) => break,
        };
        if chunks.send(chunk).await.is_err() {
            break;
        }
    }
}
