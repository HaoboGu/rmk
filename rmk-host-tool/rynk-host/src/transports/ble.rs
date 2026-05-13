//! BLE GATT transport using `btleplug`.
//!
//! The firmware exposes a Rynk GATT service (UUID `F5F50001-…`) with two
//! characteristics:
//!
//! | Characteristic | UUID | Direction | Properties |
//! |---|---|---|---|
//! | `input_data`  | `F5F50002-…` | server → host  | `read | notify` |
//! | `output_data` | `F5F50003-…` | host → server | `read | write | wwr` |
//!
//! Both carry up to (MTU − 3) bytes per write/notify. The host reassembles
//! by `LEN` in the 5-byte Rynk header, exactly like USB.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use btleplug::api::{Central, CharPropFlags, Manager as _, Peripheral as _, ScanFilter, WriteType};
use btleplug::platform::{Manager, Peripheral};
use futures::StreamExt;
use rmk_types::protocol::rynk::Cmd;
use rmk_types::protocol::rynk::RYNK_HEADER_SIZE;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::sync::{Mutex, broadcast, oneshot};
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::framing::{MAX_FRAME_SIZE, encode_frame, parse_header};
use crate::transport::{TopicFrame, Transport, TransportError};

/// Rynk GATT service UUID. Hand-picked; do not change without coordinating
/// with `rmk/src/ble/ble_server.rs`.
pub const RYNK_SERVICE_UUID: Uuid = Uuid::from_u128(0xF5F50001_0000_0000_0000_000000000000);
pub const RYNK_INPUT_CHAR_UUID: Uuid = Uuid::from_u128(0xF5F50002_0000_0000_0000_000000000000);
pub const RYNK_OUTPUT_CHAR_UUID: Uuid = Uuid::from_u128(0xF5F50003_0000_0000_0000_000000000000);

type Inbox = Arc<Mutex<HashMap<u8, oneshot::Sender<Vec<u8>>>>>;

/// BLE GATT transport.
pub struct BleGattTransport {
    peripheral: Peripheral,
    output_char: btleplug::api::Characteristic,
    next_seq: u8,
    inbox: Inbox,
    topic_tx: broadcast::Sender<TopicFrame>,
    rx_handle: JoinHandle<()>,
}

impl BleGattTransport {
    /// Scan for + connect to the first peripheral advertising the Rynk
    /// service UUID.
    pub async fn connect() -> Result<Self, TransportError> {
        let manager = Manager::new().await.map_err(|e| TransportError::Io(e.to_string()))?;
        let adapters = manager
            .adapters()
            .await
            .map_err(|e| TransportError::Io(e.to_string()))?;
        let central = adapters
            .into_iter()
            .next()
            .ok_or_else(|| TransportError::DeviceNotFound("no BLE adapter".into()))?;

        central
            .start_scan(ScanFilter {
                services: vec![RYNK_SERVICE_UUID],
            })
            .await
            .map_err(|e| TransportError::Io(e.to_string()))?;

        // Poll for up to 10 s before giving up — typical scan windows
        // catch a beaconing keyboard in well under a second.
        for _ in 0..20 {
            tokio::time::sleep(Duration::from_millis(500)).await;
            let peripherals = central
                .peripherals()
                .await
                .map_err(|e| TransportError::Io(e.to_string()))?;
            for p in peripherals {
                let props = match p.properties().await {
                    Ok(Some(p)) => p,
                    _ => continue,
                };
                if props.services.contains(&RYNK_SERVICE_UUID) {
                    let _ = central.stop_scan().await;
                    return Self::connect_peripheral(p).await;
                }
            }
        }

        let _ = central.stop_scan().await;
        Err(TransportError::DeviceNotFound(
            "no peripheral advertising Rynk service".into(),
        ))
    }

    /// Connect to an already-discovered peripheral and subscribe to the
    /// Rynk input characteristic. Public so callers can drive selection
    /// themselves (multi-device setups, named-device lookups, etc.).
    pub async fn connect_peripheral(peripheral: Peripheral) -> Result<Self, TransportError> {
        peripheral
            .connect()
            .await
            .map_err(|e| TransportError::Io(e.to_string()))?;
        peripheral
            .discover_services()
            .await
            .map_err(|e| TransportError::Io(e.to_string()))?;

        let mut input_char = None;
        let mut output_char = None;
        for c in peripheral.characteristics() {
            if c.uuid == RYNK_INPUT_CHAR_UUID && c.properties.contains(CharPropFlags::NOTIFY) {
                input_char = Some(c.clone());
            } else if c.uuid == RYNK_OUTPUT_CHAR_UUID {
                output_char = Some(c.clone());
            }
        }
        let input = input_char.ok_or_else(|| TransportError::DeviceNotFound("input characteristic missing".into()))?;
        let output =
            output_char.ok_or_else(|| TransportError::DeviceNotFound("output characteristic missing".into()))?;

        peripheral
            .subscribe(&input)
            .await
            .map_err(|e| TransportError::Io(e.to_string()))?;

        let inbox: Inbox = Arc::new(Mutex::new(HashMap::new()));
        let (topic_tx, _) = broadcast::channel::<TopicFrame>(64);

        let notifications = peripheral
            .notifications()
            .await
            .map_err(|e| TransportError::Io(e.to_string()))?;

        let rx_handle = tokio::spawn(rx_worker(notifications, input.uuid, inbox.clone(), topic_tx.clone()));

        Ok(Self {
            peripheral,
            output_char: output,
            next_seq: 1,
            inbox,
            topic_tx,
            rx_handle,
        })
    }
}

impl Drop for BleGattTransport {
    fn drop(&mut self) {
        self.rx_handle.abort();
    }
}

impl Transport for BleGattTransport {
    async fn request<Req: Serialize + Send + Sync, Resp: DeserializeOwned + Send>(
        &mut self,
        cmd: Cmd,
        req: &Req,
    ) -> Result<Resp, TransportError> {
        let seq = self.next_seq();
        let frame = encode_frame(cmd, seq, req)?;

        let (resp_tx, resp_rx) = oneshot::channel();
        self.inbox.lock().await.insert(seq, resp_tx);

        // Chunk by 244 B (MTU − 3 for the typical 247 B MTU). For larger
        // MTUs btleplug will negotiate higher and a single write covers
        // small frames; for ≤244 B writes a single Write completes.
        for chunk in frame.chunks(244) {
            self.peripheral
                .write(&self.output_char, chunk, WriteType::WithoutResponse)
                .await
                .map_err(|e| TransportError::Io(e.to_string()))?;
        }

        let payload = tokio::time::timeout(Duration::from_secs(5), resp_rx)
            .await
            .map_err(|_| TransportError::Timeout)?
            .map_err(|_| TransportError::Disconnected)?;

        postcard::from_bytes::<Resp>(&payload).map_err(TransportError::Deserialize)
    }

    fn topics(&self) -> broadcast::Receiver<TopicFrame> {
        self.topic_tx.subscribe()
    }
}

impl BleGattTransport {
    fn next_seq(&mut self) -> u8 {
        let s = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1);
        if self.next_seq == 0 {
            self.next_seq = 1;
        }
        s
    }
}

async fn rx_worker(
    mut notifications: std::pin::Pin<Box<dyn futures::Stream<Item = btleplug::api::ValueNotification> + Send>>,
    input_uuid: Uuid,
    inbox: Inbox,
    topic_tx: broadcast::Sender<TopicFrame>,
) {
    let mut buf: Vec<u8> = Vec::with_capacity(1024);
    while let Some(n) = notifications.next().await {
        if n.uuid != input_uuid {
            continue;
        }
        buf.extend_from_slice(&n.value);

        while buf.len() >= RYNK_HEADER_SIZE {
            let Ok((cmd_raw, seq, len)) = parse_header(&buf) else {
                buf.clear();
                break;
            };
            let total = RYNK_HEADER_SIZE + len;
            if total > MAX_FRAME_SIZE {
                buf.clear();
                break;
            }
            if buf.len() < total {
                break;
            }
            let payload = buf[RYNK_HEADER_SIZE..total].to_vec();
            buf.drain(..total);

            let is_topic = cmd_raw & 0x8000 != 0;
            if is_topic {
                let Some(cmd) = Cmd::from_repr(cmd_raw) else {
                    continue;
                };
                let _ = topic_tx.send(TopicFrame { cmd, payload });
            } else if let Some(tx) = inbox.lock().await.remove(&seq) {
                let _ = tx.send(payload);
            }
        }
    }
}
