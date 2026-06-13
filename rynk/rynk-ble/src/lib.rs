//! BLE GATT transport using `bluest`.
//!
//! Reuses an already connected keyboard when possible, then falls back to a
//! short scan by device name.
//!
//! The already-connected path attaches to every OS-connected device exposing a
//! HID/Battery/Rynk service and probes it for the Rynk GATT service; a non-Rynk
//! peripheral fails service discovery and is skipped, but it is briefly attached
//! during the probe.

use std::time::Duration;

use bluest::{Adapter, Characteristic, Device, Uuid};
use futures::StreamExt;
use rmk_types::protocol::rynk::RYNK_BLE_CHUNK_SIZE;
use rynk::io::{Read, Write};
use rynk::{Client, ConnectError, TransportError};
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
    bluest::btuuid::services::HUMAN_INTERFACE_DEVICE,
    bluest::btuuid::services::BATTERY,
    RYNK_SERVICE_UUID,
];

/// GATT-level I/O failure surfaced through the embedded-io error seam.
#[derive(Debug)]
pub struct BleIoError(String);

impl core::fmt::Display for BleIoError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

impl core::error::Error for BleIoError {}

impl rynk::io::Error for BleIoError {
    fn kind(&self) -> rynk::io::ErrorKind {
        rynk::io::ErrorKind::Other
    }
}

/// Byte-stream view over the bridge's notification chunks: doles a chunk out
/// across as many `read` calls as the caller's buffer needs.
struct ChunkReader {
    chunks: mpsc::Receiver<Vec<u8>>,
    pending: Vec<u8>,
    pos: usize,
}

impl rynk::io::ErrorType for ChunkReader {
    type Error = BleIoError;
}

impl Read for ChunkReader {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        // Skip empty notification chunks: returning `Ok(0)` mid-stream would
        // read as EOF (link gone) to the client.
        while self.pos >= self.pending.len() {
            match self.chunks.recv().await {
                Some(chunk) => {
                    self.pending = chunk;
                    self.pos = 0;
                }
                None => return Ok(0), // bridge gone → EOF → Disconnected
            }
        }
        let n = buf.len().min(self.pending.len() - self.pos);
        buf[..n].copy_from_slice(&self.pending[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

/// Attached Rynk GATT link.
pub struct BleTransport {
    output_char: Characteristic,
    write_chunk: usize,
    reader: ChunkReader,
    /// Notification bridge task.
    bridge: tokio::task::JoinHandle<()>,
    /// The connected device's name, if it advertised one.
    name: Option<String>,
    // Keep the OS connection alive.
    _adapter: Adapter,
    _device: Device,
}

impl BleTransport {
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
            match attach(&adapter, d).await {
                Ok(transport) => return Ok(transport),
                Err(e) => log::debug!("rynk ble: connected candidate did not attach: {e}"),
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

impl rynk::io::ErrorType for BleTransport {
    type Error = BleIoError;
}

impl Read for BleTransport {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        self.reader.read(buf).await
    }
}

impl Write for BleTransport {
    /// One GATT write per call, capped to the characteristic capacity; the
    /// client's `write_all` loops over the rest. Acknowledged write: a
    /// silently dropped chunk would desync the firmware's stream reassembler,
    /// which has no mid-frame resync.
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        let n = buf.len().min(self.write_chunk);
        self.output_char
            .write(&buf[..n])
            .await
            .map_err(|e| BleIoError(format!("gatt write: {e}")))?;
        Ok(n)
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl Drop for BleTransport {
    fn drop(&mut self) {
        self.bridge.abort();
    }
}

/// Connect over BLE using the default `"RMK"` name hint.
pub async fn connect_ble() -> Result<Client<BleTransport>, ConnectError> {
    connect_ble_name(RMK_NAME_HINT).await
}

/// Connect over BLE to a keyboard whose name contains `name_hint`.
pub async fn connect_ble_name(name_hint: &str) -> Result<Client<BleTransport>, ConnectError> {
    connect_transport(BleTransport::connect_with_name(name_hint).await?).await
}

// Intentionally duplicated in `rynk-serial` rather than shared: `rynk`
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
            // Definitive: discovery completed and the Rynk service/characteristic
            // is absent — not a Rynk device, retrying won't change that.
            Err(e @ TransportError::DeviceNotFound(_)) => return Err(e),
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
        let bridge = tokio::spawn(notify_bridge(input, chunk_tx, sub_tx));
        if let Err(e) = sub_rx.await.unwrap_or(Err(TransportError::Disconnected)) {
            bridge.abort();
            last_err = e;
            continue;
        }

        return Ok(BleTransport {
            output_char: output,
            write_chunk,
            reader: ChunkReader {
                chunks: chunk_rx,
                pending: Vec::new(),
                pos: 0,
            },
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

/// Subscribe to GATT notifications, ack via `sub_tx`, then forward.
async fn notify_bridge(
    input: Characteristic,
    chunks: mpsc::Sender<Vec<u8>>,
    sub_tx: oneshot::Sender<Result<(), TransportError>>,
) {
    let notifications = match input.notify().await {
        Ok(n) => {
            let _ = sub_tx.send(Ok(()));
            n
        }
        Err(e) => {
            let _ = sub_tx.send(Err(TransportError::Io(e.to_string())));
            return;
        }
    };

    forward_notifications(notifications, chunks).await;
}

/// Forward notification chunks into the transport channel until the stream
/// ends/errors or the transport (the receiver) is dropped.
async fn forward_notifications<E>(
    mut notifications: impl futures::Stream<Item = Result<Vec<u8>, E>> + Unpin,
    chunks: mpsc::Sender<Vec<u8>>,
) {
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

#[cfg(test)]
mod tests {
    use futures::stream;

    use super::*;

    #[tokio::test]
    async fn forwards_chunks_until_stream_ends() {
        let (tx, mut rx) = mpsc::channel(4);
        let items: Vec<Result<Vec<u8>, ()>> = vec![Ok(vec![1, 2]), Ok(vec![3])];
        forward_notifications(stream::iter(items), tx).await;
        assert_eq!(rx.recv().await, Some(vec![1, 2]));
        assert_eq!(rx.recv().await, Some(vec![3]));
        // The sender is gone, so the transport's recv reads Disconnected.
        assert_eq!(rx.recv().await, None);
    }

    #[tokio::test]
    async fn stops_at_first_stream_error() {
        let (tx, mut rx) = mpsc::channel(4);
        let items: Vec<Result<Vec<u8>, ()>> = vec![Ok(vec![1]), Err(()), Ok(vec![2])];
        forward_notifications(stream::iter(items), tx).await;
        assert_eq!(rx.recv().await, Some(vec![1]));
        assert_eq!(rx.recv().await, None, "chunks after the error must not be forwarded");
    }

    #[tokio::test]
    async fn stops_when_transport_is_dropped() {
        let (tx, rx) = mpsc::channel(1);
        drop(rx);
        // Endless stream: returns (instead of looping) only because the
        // receiver is gone.
        forward_notifications(stream::repeat_with(|| Ok::<_, ()>(vec![0u8])), tx).await;
    }

    fn chunk_reader(capacity: usize) -> (mpsc::Sender<Vec<u8>>, ChunkReader) {
        let (tx, rx) = mpsc::channel(capacity);
        (
            tx,
            ChunkReader {
                chunks: rx,
                pending: Vec::new(),
                pos: 0,
            },
        )
    }

    #[tokio::test]
    async fn chunk_reader_doles_chunk_across_reads() {
        let (tx, mut r) = chunk_reader(2);
        tx.send(vec![1, 2, 3, 4, 5]).await.unwrap();
        drop(tx);

        let mut buf = [0u8; 2];
        assert_eq!(r.read(&mut buf).await.unwrap(), 2);
        assert_eq!(buf, [1, 2]);
        assert_eq!(r.read(&mut buf).await.unwrap(), 2);
        assert_eq!(buf, [3, 4]);
        assert_eq!(r.read(&mut buf).await.unwrap(), 1);
        assert_eq!(buf[0], 5);
        // Channel closed and drained → EOF.
        assert_eq!(r.read(&mut buf).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn chunk_reader_skips_empty_chunks() {
        let (tx, mut r) = chunk_reader(2);
        tx.send(Vec::new()).await.unwrap();
        tx.send(vec![7]).await.unwrap();
        drop(tx);

        let mut buf = [0u8; 4];
        assert_eq!(r.read(&mut buf).await.unwrap(), 1, "empty chunk must not read as EOF");
        assert_eq!(buf[0], 7);
    }
}
