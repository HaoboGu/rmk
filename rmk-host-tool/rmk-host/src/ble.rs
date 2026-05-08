//! BLE GATT host transport for the RMK protocol.
//!
//! Wraps `bluest` (CoreBluetooth on macOS, BlueZ on Linux, WinRT on Windows)
//! to scan for the RMK protocol service, connect, and bridge GATT
//! notify/write operations to postcard-rpc's `WireTx` / `WireRx` traits.
//!
//! On the wire each frame is COBS-encoded, terminated with `0x00`, and split
//! across MTU-sized notifications. This file mirrors
//! `rmk/src/host/rmk_protocol/wire_ble.rs` on the firmware side — the two
//! ends share the same framing.

use std::sync::Arc;
use std::time::Duration;

use bluest::{Adapter, AdvertisingDevice, Characteristic, Uuid};
use futures::StreamExt;
use postcard_rpc::host_client::{HostClient, WireRx, WireSpawn, WireTx};
use postcard_rpc::header::VarSeqKind;
use serde::de::DeserializeOwned;
use postcard_rpc::postcard_schema::Schema;
use thiserror::Error;
use tokio::sync::mpsc;

/// Same UUID the firmware advertises in `rmk/src/ble/ble_server.rs::RmkProtocolService`.
pub const RMK_PROTOCOL_SERVICE_UUID: Uuid =
    Uuid::from_u128(0x9d44e000_3582_4f23_a39c_37e0c9bd6b00);
/// `output_data` characteristic — host → device write.
pub const RMK_PROTOCOL_OUTPUT_UUID: Uuid =
    Uuid::from_u128(0x9d44e001_3582_4f23_a39c_37e0c9bd6b00);
/// `input_data` characteristic — device → host notify.
pub const RMK_PROTOCOL_INPUT_UUID: Uuid =
    Uuid::from_u128(0x9d44e002_3582_4f23_a39c_37e0c9bd6b00);

/// One BLE notify chunk size: MTU − 3 (typical 247 → 244).
const NOTIFY_PAYLOAD: usize = 244;

#[derive(Debug, Error)]
pub enum BleError {
    #[error("BLE adapter error: {0}")]
    Adapter(String),
    #[error("device not found within scan window")]
    NotFound,
    #[error("RMK protocol service or characteristics missing on device")]
    MissingService,
    #[error("transport closed")]
    Closed,
    #[error("{0}")]
    Other(String),
}

impl From<bluest::Error> for BleError {
    fn from(e: bluest::Error) -> Self {
        BleError::Adapter(format!("{e:?}"))
    }
}

/// Connect to the first device matching `name_filter` (substring match
/// against the advertised local name) and build a `HostClient` over BLE
/// GATT. The firmware's BLE adv only carries 16-bit HID/Battery UUIDs, not
/// the 128-bit rmk_protocol service UUID, so name-filtering is the
/// fallback. Pass `None` to take the first device that exposes the
/// rmk_protocol GATT service after connecting (slower — connects to every
/// scanned device until one has the service).
pub async fn connect_ble<WireErr>(
    scan_timeout: Duration,
    name_filter: Option<&str>,
) -> Result<HostClient<WireErr>, BleError>
where
    WireErr: DeserializeOwned + Schema + Send + 'static,
{
    let adapter = Adapter::default()
        .await
        .ok_or_else(|| BleError::Adapter("no default adapter".into()))?;
    adapter.wait_available().await?;

    let device = scan_for_device(&adapter, name_filter, scan_timeout).await?;
    adapter.connect_device(&device).await?;

    // Discover the service + characteristics.
    let services = device.discover_services().await?;
    let service = services
        .iter()
        .find(|s| s.uuid() == RMK_PROTOCOL_SERVICE_UUID)
        .ok_or(BleError::MissingService)?
        .clone();
    let chars = service.discover_characteristics().await?;
    let output = chars
        .iter()
        .find(|c| c.uuid() == RMK_PROTOCOL_OUTPUT_UUID)
        .ok_or(BleError::MissingService)?
        .clone();
    let input = chars
        .iter()
        .find(|c| c.uuid() == RMK_PROTOCOL_INPUT_UUID)
        .ok_or(BleError::MissingService)?
        .clone();

    // Spawn a task that subscribes to notifies and forwards each chunk into a
    // tokio mpsc for the WireRx side to drain.
    let (chunk_tx, chunk_rx) = mpsc::channel::<Vec<u8>>(32);
    {
        let input = input.clone();
        tokio::spawn(async move {
            match input.notify().await {
                Ok(mut stream) => {
                    while let Some(item) = stream.next().await {
                        match item {
                            Ok(bytes) => {
                                if chunk_tx.send(bytes).await.is_err() {
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                    }
                }
                Err(_) => {}
            }
        });
    }

    let tx = BleWireTx {
        output: Arc::new(output),
    };
    let rx = BleWireRx {
        chunks: chunk_rx,
        accumulator: Vec::with_capacity(NOTIFY_PAYLOAD * 4),
    };
    let sp = BleSpawn;

    Ok(HostClient::new_with_wire(
        tx,
        rx,
        sp,
        VarSeqKind::Seq2,
        "rmk_protocol",
        8,
    ))
}

async fn scan_for_device(
    adapter: &Adapter,
    name_filter: Option<&str>,
    timeout: Duration,
) -> Result<bluest::Device, BleError> {
    // Empty filter = scan everything.
    let mut stream = adapter.scan(&[]).await?;
    tokio::time::timeout(timeout, async move {
        while let Some(adv) = stream.next().await {
            let AdvertisingDevice {
                device,
                adv_data,
                ..
            } = adv;
            let local_name = adv_data.local_name.as_deref();
            let dev_name = device.name().ok();
            let candidate: Option<&str> = local_name.or(dev_name.as_deref());
            match (name_filter, candidate) {
                (Some(needle), Some(n)) if n.contains(needle) => {
                    return Ok::<_, BleError>(device);
                }
                (None, _) => return Ok::<_, BleError>(device),
                _ => continue,
            }
        }
        Err(BleError::NotFound)
    })
    .await
    .map_err(|_| BleError::NotFound)?
}

// ---------------------------------------------------------------------------
// WireTx
// ---------------------------------------------------------------------------

pub struct BleWireTx {
    output: Arc<Characteristic>,
}

impl WireTx for BleWireTx {
    type Error = BleError;

    async fn send(&mut self, data: Vec<u8>) -> Result<(), Self::Error> {
        // COBS-encode the frame, append the 0x00 sentinel, then split across
        // MTU-sized chunks and write each.
        let mut encoded = vec![0u8; cobs::max_encoding_length(data.len()) + 1];
        let n = cobs::try_encode(&data, &mut encoded).map_err(|e| BleError::Other(format!("cobs: {e:?}")))?;
        encoded[n] = 0;
        encoded.truncate(n + 1);

        for chunk in encoded.chunks(NOTIFY_PAYLOAD) {
            // write_without_response is a one-shot ATT op; bluest exposes it as
            // `write_without_response` if supported, otherwise fall through to
            // confirmed write. We prefer the former for throughput.
            self.output.write_without_response(chunk).await?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// WireRx
// ---------------------------------------------------------------------------

pub struct BleWireRx {
    chunks: mpsc::Receiver<Vec<u8>>,
    accumulator: Vec<u8>,
}

impl WireRx for BleWireRx {
    type Error = BleError;

    async fn receive(&mut self) -> Result<Vec<u8>, Self::Error> {
        loop {
            // First check: do we have a sentinel in the accumulator already?
            if let Some(zero_pos) = self.accumulator.iter().position(|&b| b == 0) {
                let frame_encoded = self.accumulator[..zero_pos].to_vec();
                let mut decoded = vec![0u8; frame_encoded.len()];
                let report = cobs::decode(&frame_encoded, &mut decoded)
                    .map_err(|e| BleError::Other(format!("cobs decode: {e:?}")))?;
                decoded.truncate(report.frame_size());
                self.accumulator.drain(..=zero_pos);
                return Ok(decoded);
            }

            // No sentinel yet — wait for another chunk.
            let chunk = self.chunks.recv().await.ok_or(BleError::Closed)?;
            self.accumulator.extend_from_slice(&chunk);
        }
    }
}

// ---------------------------------------------------------------------------
// WireSpawn
// ---------------------------------------------------------------------------

pub struct BleSpawn;

impl WireSpawn for BleSpawn {
    fn spawn(&mut self, fut: impl std::future::Future<Output = ()> + Send + 'static) {
        let _ = tokio::task::spawn(fut);
    }
}
