//! USB bulk transport using `nusb`.
//!
//! The firmware exposes one BULK IN + one BULK OUT endpoint on a vendor-
//! specific interface tagged with the WinUSB MS OS 2.0 descriptor set
//! (`CompatibleId = "WINUSB"`, `DeviceInterfaceGUIDs =
//! {F5F5F5F5-1234-5678-9ABC-DEF012345678}`). On Windows that triggers
//! automatic WinUSB binding. On Linux/macOS the same GUID is the
//! `udev`/`IORegistry` discriminator the host uses to filter from other
//! USB devices.
//!
//! Match-by-VID/PID is intentionally **not** the primary filter — many
//! keyboards share VID/PID with their bootloader, and downstream users
//! often re-use VID/PIDs across firmwares.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use nusb::transfer::{Buffer, Bulk, In, Out};
use nusb::{Endpoint, MaybeFuture};
use rmk_types::protocol::rynk::Cmd;
use rmk_types::protocol::rynk::header::HEADER_SIZE;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::sync::{Mutex, broadcast, oneshot};
use tokio::task::JoinHandle;

use crate::framing::{MAX_FRAME_SIZE, encode_frame, parse_header};
use crate::transport::{TopicFrame, Transport, TransportError};

/// Vendor-specific class code the firmware claims. Combined with
/// subclass+protocol = 0, this is the magic triple Windows looks for to
/// apply the WinUSB compat ID.
const VENDOR_CLASS: u8 = 0xFF;

/// Inbox map: each in-flight request registers a oneshot keyed by its
/// SEQ. The RX worker pops the matching entry and delivers the payload.
type Inbox = Arc<Mutex<HashMap<u8, oneshot::Sender<Vec<u8>>>>>;

/// Owns the bulk-OUT endpoint, the next SEQ counter, and a topic-broadcast
/// sender. The RX worker runs in the background on its own tokio task
/// holding the bulk-IN endpoint; dropping `UsbBulkTransport` aborts it.
pub struct UsbBulkTransport {
    bulk_out: Endpoint<Bulk, Out>,
    next_seq: u8,
    inbox: Inbox,
    topic_tx: broadcast::Sender<TopicFrame>,
    rx_handle: JoinHandle<()>,
}

impl UsbBulkTransport {
    /// Find the first Rynk-capable USB device and connect.
    ///
    /// Filters on `bInterfaceClass == 0xFF`. The first vendor-class
    /// interface with one BULK IN + one BULK OUT endpoint is claimed.
    /// For multi-device setups, fetch the [`nusb::DeviceInfo`] list
    /// yourself and pass the chosen entry to [`Self::open`].
    pub async fn connect() -> Result<Self, TransportError> {
        let devices = nusb::list_devices()
            .wait()
            .map_err(|e| TransportError::Io(e.to_string()))?;
        for di in devices {
            if let Ok(t) = Self::open(&di).await {
                return Ok(t);
            }
        }
        Err(TransportError::DeviceNotFound(
            "no Rynk-capable USB device found (vendor class 0xFF)".into(),
        ))
    }

    /// Connect to a specific USB device. Probes its first vendor-class
    /// interface for the BULK IN/OUT pair.
    pub async fn open(di: &nusb::DeviceInfo) -> Result<Self, TransportError> {
        let device = di.open().wait().map_err(|e| TransportError::Io(e.to_string()))?;
        let config = device
            .active_configuration()
            .map_err(|e| TransportError::Io(e.to_string()))?;

        // Walk interfaces looking for vendor-class + matching endpoint pair.
        // The first BULK IN / BULK OUT pair under a vendor-class alt setting
        // wins. Rynk advertises only one such interface per device.
        let mut iface_picked: Option<(u8, u8, u8)> = None;
        'outer: for interface in config.interfaces() {
            for alt in interface.alt_settings() {
                if alt.class() != VENDOR_CLASS {
                    continue;
                }
                let mut bulk_in_addr: Option<u8> = None;
                let mut bulk_out_addr: Option<u8> = None;
                for ep in alt.endpoints() {
                    let dir = ep.address() & 0x80;
                    // 0x02 = bulk transfer type per USB spec; nusb's typed
                    // endpoint constructor verifies this for us, so just
                    // confirm the kind here when scanning descriptors.
                    if ep.transfer_type() as u8 != 0x02 {
                        continue;
                    }
                    if dir == 0x80 {
                        bulk_in_addr = Some(ep.address());
                    } else {
                        bulk_out_addr = Some(ep.address());
                    }
                }
                if let (Some(bi), Some(bo)) = (bulk_in_addr, bulk_out_addr) {
                    iface_picked = Some((interface.interface_number(), bi, bo));
                    break 'outer;
                }
            }
        }
        let (iface_num, bulk_in_addr, bulk_out_addr) = iface_picked.ok_or_else(|| {
            TransportError::DeviceNotFound("device has no vendor-class interface with bulk IN+OUT".into())
        })?;

        let iface = device
            .claim_interface(iface_num)
            .wait()
            .map_err(|e| TransportError::Io(e.to_string()))?;

        let bulk_out: Endpoint<Bulk, Out> = iface
            .endpoint::<Bulk, Out>(bulk_out_addr)
            .map_err(|e| TransportError::Io(e.to_string()))?;
        let bulk_in: Endpoint<Bulk, In> = iface
            .endpoint::<Bulk, In>(bulk_in_addr)
            .map_err(|e| TransportError::Io(e.to_string()))?;

        Ok(Self::start(bulk_out, bulk_in))
    }

    fn start(bulk_out: Endpoint<Bulk, Out>, bulk_in: Endpoint<Bulk, In>) -> Self {
        let inbox: Inbox = Arc::new(Mutex::new(HashMap::new()));
        let (topic_tx, _) = broadcast::channel::<TopicFrame>(64);

        let rx_handle = tokio::spawn(rx_worker(bulk_in, inbox.clone(), topic_tx.clone()));

        Self {
            bulk_out,
            next_seq: 1,
            inbox,
            topic_tx,
            rx_handle,
        }
    }
}

impl Drop for UsbBulkTransport {
    fn drop(&mut self) {
        self.rx_handle.abort();
    }
}

impl Transport for UsbBulkTransport {
    async fn request<Req: Serialize + Send + Sync, Resp: DeserializeOwned + Send>(
        &mut self,
        cmd: Cmd,
        req: &Req,
    ) -> Result<Resp, TransportError> {
        let seq = self.next_seq();
        let frame = encode_frame(cmd, seq, req)?;

        let (resp_tx, resp_rx) = oneshot::channel();
        self.inbox.lock().await.insert(seq, resp_tx);

        // Submit the OUT transfer and wait for it to complete. `nusb`
        // schedules the URB; the driver chunks across MPS internally.
        self.bulk_out.submit(frame.into());
        self.bulk_out
            .next_complete()
            .await
            .into_result()
            .map_err(|e| TransportError::Io(format!("{e:?}")))?;

        // Wait for the matching response, bounded so a misbehaving
        // firmware can't hang the host indefinitely.
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

impl UsbBulkTransport {
    fn next_seq(&mut self) -> u8 {
        let s = self.next_seq;
        // SEQ 0 is reserved for topics, so skip it on wrap.
        self.next_seq = self.next_seq.wrapping_add(1);
        if self.next_seq == 0 {
            self.next_seq = 1;
        }
        s
    }
}

/// Background RX loop: continuously submit IN buffers, decode each
/// completed transfer into one or more frames (split by `LEN`), and
/// dispatch them to either an inbox entry (for SEQ != 0 responses) or
/// the topic broadcast (for topic-CMD frames).
async fn rx_worker(mut bulk_in: Endpoint<Bulk, In>, inbox: Inbox, topic_tx: broadcast::Sender<TopicFrame>) {
    // Keep two transfers in flight to mask USB latency on bursts.
    // 1024 B per buffer is comfortably larger than any expected frame
    // for non-bulk Cmds.
    for _ in 0..2 {
        bulk_in.submit(Buffer::new(1024));
    }

    let mut buf: Vec<u8> = Vec::with_capacity(4096);
    loop {
        let chunk = match bulk_in.next_complete().await.into_result() {
            Ok(c) => c,
            Err(_) => break,
        };
        buf.extend_from_slice(&chunk);
        // Resubmit so the driver always has a buffer to fill.
        bulk_in.submit(Buffer::new(1024));

        // Drain whatever full frames are now present. `LEN` decides.
        while buf.len() >= HEADER_SIZE {
            let Ok((cmd_raw, seq, len)) = parse_header(&buf) else {
                buf.clear();
                break;
            };
            let total = HEADER_SIZE + len;
            if total > MAX_FRAME_SIZE {
                buf.clear();
                break;
            }
            if buf.len() < total {
                break;
            }
            let payload = buf[HEADER_SIZE..total].to_vec();
            buf.drain(..total);

            // Topic frames have the high bit set in CMD. Topics carry
            // SEQ = 0 by spec but dispatch on the CMD mask, not SEQ, so
            // a misbehaving topic publisher with nonzero SEQ still
            // routes correctly.
            let is_topic = cmd_raw & 0x8000 != 0;
            if is_topic {
                let Some(cmd) = Cmd::from_repr(cmd_raw) else {
                    continue;
                };
                let _ = topic_tx.send(TopicFrame { cmd, payload });
            } else if let Some(tx) = inbox.lock().await.remove(&seq) {
                let _ = tx.send(payload);
            }
            // Unknown CMD or unmatched SEQ → drop silently.
        }
    }
}
