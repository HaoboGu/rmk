//! BLE GATT transport adapter for the RMK protocol.
//!
//! Wire model: COBS-framed byte stream over a dedicated GATT primary service
//! (NOT under the HID UUID `0x1812`). Two characteristics:
//! * `output_data` (write / write-without-response) — host → device
//! * `input_data`  (notify) — device → host
//!
//! Both characteristics are sized to MTU − 3 (`[u8; 244]` for the typical
//! 247-byte MTU). Frames are COBS-encoded with `0x00` as the sentinel and may
//! span multiple notifies; the host reframer accumulates until it sees the
//! sentinel.
//!
//! The transport is queue-backed:
//! * `gatt_events_task` (in `ble/mod.rs`) pushes incoming write payloads onto
//!   `RMK_PROTOCOL_REQUEST_CHANNEL`. `BleWireRx::receive` drains that channel,
//!   reframes by COBS, and decodes.
//! * `BleWireTx::send` COBS-encodes the frame into a heapless buffer and pushes
//!   it onto `RMK_PROTOCOL_REPLY_CHANNEL`. A small notify task in the BLE
//!   transport drains the reply channel and fans the frame out across
//!   `input_data` notifies on the active connection.
//!
//! Sidesteps the trouble-host `Characteristic`/`GattConnection` lifetime
//! problem: the WireTx itself holds no connection reference; that lives only in
//! the per-connection notify task.

use core::fmt::Arguments;

use embassy_sync::blocking_mutex::raw::RawMutex;
use embassy_sync::mutex::Mutex;
use heapless::Vec;
use postcard_rpc::header::{VarHeader, VarKeyKind};
use postcard_rpc::server::{WireRx, WireRxErrorKind, WireTx, WireTxErrorKind};
use serde::Serialize;

/// One-MTU notify size minus 3 ATT-header bytes.
pub(crate) const BLE_NOTIFY_PAYLOAD: usize = 244;

/// Maximum size of a single COBS-encoded RPC frame on BLE. Sized to cover the
/// largest expected response (capabilities, bulk reads) plus COBS overhead
/// (~1 byte per 254 source bytes + 1 sentinel).
pub(crate) const BLE_FRAME_MAX: usize = 512;

/// Sized by `Server::run`'s buffer requirement: the largest decoded request
/// payload plus header. Aligns with `BLE_FRAME_MAX` to keep buffer sizing simple.
pub(crate) const BLE_RX_BUF: usize = BLE_FRAME_MAX;

/// Inbound chunk: one BLE write payload, fixed at MTU − 3.
pub(crate) type BleRequestChunk = Vec<u8, BLE_NOTIFY_PAYLOAD>;

/// Outbound frame: a single complete COBS-encoded RPC frame.
pub(crate) type BleReplyFrame = Vec<u8, BLE_FRAME_MAX>;

// ---------------------------------------------------------------------------
// TX
// ---------------------------------------------------------------------------

pub(crate) struct BleWireTxInner<'b> {
    /// Scratch buffer used to serialize header + body before COBS-encoding.
    pub(crate) tx_buf: &'b mut [u8],
    /// Scratch buffer used to hold the COBS-encoded frame.
    pub(crate) cobs_buf: &'b mut [u8],
    /// Reply channel sender (heapless `Channel` provides `try_send`/`send`).
    pub(crate) replies: &'static embassy_sync::channel::Channel<crate::RawMutex, BleReplyFrame, 4>,
}

pub(crate) struct BleWireTx<'m, 'b, M: RawMutex + 'static> {
    inner: &'m Mutex<M, BleWireTxInner<'b>>,
}

impl<'m, 'b, M: RawMutex + 'static> BleWireTx<'m, 'b, M> {
    pub(crate) fn new(inner: &'m Mutex<M, BleWireTxInner<'b>>) -> Self {
        Self { inner }
    }
}

impl<'m, 'b, M: RawMutex + 'static> Clone for BleWireTx<'m, 'b, M> {
    fn clone(&self) -> Self {
        Self { inner: self.inner }
    }
}

impl<'m, 'b, M: RawMutex + 'static> WireTx for BleWireTx<'m, 'b, M> {
    type Error = WireTxErrorKind;

    async fn send<T: Serialize + ?Sized>(&self, hdr: VarHeader, msg: &T) -> Result<(), Self::Error> {
        // Build the encoded frame inside the lock, then drop the guard before
        // awaiting the replies channel — otherwise a slow notify task would
        // hold the Tx mutex across `replies.send().await` and stall the topic
        // publisher and any other reply behind it.
        let (owned, replies) = {
            let mut guard = self.inner.lock().await;
            let BleWireTxInner {
                tx_buf,
                cobs_buf,
                replies,
            } = &mut *guard;

            let (hdr_used, remain) = hdr.write_to_slice(tx_buf).ok_or(WireTxErrorKind::Other)?;
            let hdr_len = hdr_used.len();
            let body_used = postcard::to_slice(msg, remain).map_err(|_| WireTxErrorKind::Other)?;
            let total = hdr_len + body_used.len();

            let encoded = cobs::try_encode(&tx_buf[..total], cobs_buf).map_err(|_| WireTxErrorKind::Other)?;
            if encoded + 1 > cobs_buf.len() {
                return Err(WireTxErrorKind::Other);
            }
            cobs_buf[encoded] = 0;

            let mut owned: BleReplyFrame = Vec::new();
            owned
                .extend_from_slice(&cobs_buf[..encoded + 1])
                .map_err(|_| WireTxErrorKind::Other)?;
            (owned, *replies)
        };
        replies.send(owned).await;
        Ok(())
    }

    async fn send_raw(&self, buf: &[u8]) -> Result<(), Self::Error> {
        let (owned, replies) = {
            let mut guard = self.inner.lock().await;
            let BleWireTxInner { cobs_buf, replies, .. } = &mut *guard;
            let encoded = cobs::try_encode(buf, cobs_buf).map_err(|_| WireTxErrorKind::Other)?;
            if encoded + 1 > cobs_buf.len() {
                return Err(WireTxErrorKind::Other);
            }
            cobs_buf[encoded] = 0;
            let mut owned: BleReplyFrame = Vec::new();
            owned
                .extend_from_slice(&cobs_buf[..encoded + 1])
                .map_err(|_| WireTxErrorKind::Other)?;
            (owned, *replies)
        };
        replies.send(owned).await;
        Ok(())
    }

    async fn send_log_str(&self, _kkind: VarKeyKind, _s: &str) -> Result<(), Self::Error> {
        Err(WireTxErrorKind::Other)
    }

    async fn send_log_fmt<'a>(&self, _kkind: VarKeyKind, _a: Arguments<'a>) -> Result<(), Self::Error> {
        Err(WireTxErrorKind::Other)
    }
}

// ---------------------------------------------------------------------------
// RX
// ---------------------------------------------------------------------------

/// `WireRx` impl over an MTU-sized inbound request channel. Each channel item
/// is one BLE write payload; we accumulate until a `0x00` sentinel is found,
/// then COBS-decode the bytes up to the sentinel into the dispatcher's buffer.
pub(crate) struct BleWireRx<'b> {
    /// Source channel filled by `gatt_events_task` on every write to
    /// `output_data`.
    pub(crate) requests: &'static embassy_sync::channel::Channel<crate::RawMutex, BleRequestChunk, 4>,
    /// Scratch accumulator: holds the unconsumed tail of the most recent chunk
    /// plus any bytes carried over from the previous frame's leftover.
    pub(crate) scratch: &'b mut [u8],
    /// Number of valid bytes currently in `scratch`.
    pub(crate) scratch_len: usize,
    /// True after an oversize frame: ignore inbound bytes until we've seen a
    /// `0x00` sentinel and resynchronized to the next frame boundary.
    pub(crate) draining: bool,
}

impl<'b> BleWireRx<'b> {
    fn copy_back(&mut self, start: usize) {
        if start >= self.scratch_len {
            self.scratch_len = 0;
            return;
        }
        let count = self.scratch_len - start;
        self.scratch.copy_within(start..self.scratch_len, 0);
        self.scratch_len = count;
    }
}

impl<'b> WireRx for BleWireRx<'b> {
    type Error = WireRxErrorKind;

    async fn wait_connection(&mut self) {}

    async fn receive<'a>(&mut self, buf: &'a mut [u8]) -> Result<&'a mut [u8], Self::Error> {
        loop {
            // While draining, swallow inbound chunks until one contains a
            // sentinel; the bytes after that sentinel start a fresh frame.
            if self.draining {
                let chunk = self.requests.receive().await;
                if let Some(pos) = chunk.iter().position(|&b| b == 0) {
                    self.draining = false;
                    let tail = &chunk[pos + 1..];
                    // Tail can never overflow: chunk size ≤ BLE_NOTIFY_PAYLOAD
                    // and that's strictly less than the scratch capacity.
                    debug_assert!(tail.len() <= self.scratch.len());
                    let take = tail.len().min(self.scratch.len());
                    self.scratch[..take].copy_from_slice(&tail[..take]);
                    self.scratch_len = take;
                }
                continue;
            }

            // First check: do we already have a full frame in scratch?
            if let Some(zero_pos) = self.scratch[..self.scratch_len].iter().position(|&b| b == 0) {
                let frame = &self.scratch[..zero_pos]; // COBS-encoded body, sentinel excluded
                let report = cobs::decode(frame, buf).map_err(|e| match e {
                    cobs::DecodeError::TargetBufTooSmall => WireRxErrorKind::ReceivedMessageTooLarge,
                    _ => WireRxErrorKind::Other,
                })?;
                let decoded = report.frame_size();
                self.copy_back(zero_pos + 1);
                return Ok(&mut buf[..decoded]);
            }

            // No sentinel yet; wait for another chunk.
            let chunk = self.requests.receive().await;
            let needed = self.scratch_len + chunk.len();
            if needed > self.scratch.len() {
                // Frame exceeds scratch capacity. Drop the accumulated state
                // and enter draining mode; we'll resync to the next sentinel.
                self.scratch_len = 0;
                if let Some(pos) = chunk.iter().position(|&b| b == 0) {
                    let tail = &chunk[pos + 1..];
                    debug_assert!(tail.len() <= self.scratch.len());
                    let take = tail.len().min(self.scratch.len());
                    self.scratch[..take].copy_from_slice(&tail[..take]);
                    self.scratch_len = take;
                } else {
                    self.draining = true;
                }
                return Err(WireRxErrorKind::ReceivedMessageTooLarge);
            }
            self.scratch[self.scratch_len..self.scratch_len + chunk.len()].copy_from_slice(&chunk);
            self.scratch_len = needed;
        }
    }
}

// ---------------------------------------------------------------------------
// COBS reframer unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::vec;
    use std::vec::Vec as StdVec;

    use super::*;

    fn run_receive(chunks: &[&[u8]], frame_buf_len: usize) -> StdVec<Result<StdVec<u8>, WireRxErrorKind>> {
        let scratch: &'static mut [u8] = Box::leak(vec![0u8; BLE_RX_BUF].into_boxed_slice());
        let req_ch: &'static embassy_sync::channel::Channel<crate::RawMutex, BleRequestChunk, 4> =
            Box::leak(Box::new(embassy_sync::channel::Channel::new()));

        for chunk in chunks {
            let mut v: BleRequestChunk = heapless::Vec::new();
            v.extend_from_slice(chunk).unwrap();
            req_ch.try_send(v).expect("test channel full");
        }

        let mut rx = BleWireRx {
            requests: req_ch,
            scratch,
            scratch_len: 0,
            draining: false,
        };

        let mut buf: StdVec<u8> = vec![0u8; frame_buf_len];
        let mut results: StdVec<Result<StdVec<u8>, WireRxErrorKind>> = StdVec::new();
        for _ in 0..chunks.len() + 1 {
            let mut fut = Box::pin(rx.receive(&mut buf));
            let waker = noop_waker();
            let mut cx = core::task::Context::from_waker(&waker);
            match fut.as_mut().poll(&mut cx) {
                core::task::Poll::Ready(Ok(slice)) => {
                    let owned = slice.to_vec();
                    drop(fut);
                    results.push(Ok(owned));
                }
                core::task::Poll::Ready(Err(e)) => {
                    drop(fut);
                    results.push(Err(e));
                }
                core::task::Poll::Pending => break,
            }
        }
        results
    }

    fn noop_waker() -> core::task::Waker {
        const VTABLE: core::task::RawWakerVTable = core::task::RawWakerVTable::new(
            |_| core::task::RawWaker::new(core::ptr::null(), &VTABLE),
            |_| {},
            |_| {},
            |_| {},
        );
        unsafe { core::task::Waker::from_raw(core::task::RawWaker::new(core::ptr::null(), &VTABLE)) }
    }

    fn cobs_encode_with_sentinel(src: &[u8]) -> StdVec<u8> {
        let mut buf: StdVec<u8> = vec![0u8; cobs::max_encoding_length(src.len()) + 1];
        let n = cobs::try_encode(src, &mut buf).unwrap();
        buf[n] = 0;
        buf.truncate(n + 1);
        buf
    }

    #[test]
    fn single_frame_in_single_chunk_decodes_back_to_payload() {
        let payload = b"hello world";
        let encoded = cobs_encode_with_sentinel(payload);
        let res = run_receive(&[&encoded], 256);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].as_ref().unwrap(), payload);
    }

    #[test]
    fn frame_split_across_chunk_boundary_is_reassembled() {
        let payload = b"split-across-boundary-payload-bytes";
        let encoded = cobs_encode_with_sentinel(payload);
        let mid = encoded.len() / 2;
        let res = run_receive(&[&encoded[..mid], &encoded[mid..]], 256);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].as_ref().unwrap(), payload);
    }

    #[test]
    fn zero_byte_at_chunk_boundary_terminates_frame_correctly() {
        let payload = b"abc\x00def";
        let encoded = cobs_encode_with_sentinel(payload);
        let split = encoded.len() - 1;
        let res = run_receive(&[&encoded[..split], &encoded[split..]], 256);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].as_ref().unwrap(), payload);
    }

    #[test]
    fn two_back_to_back_frames_decode_independently() {
        let p1 = b"first";
        let p2 = b"second-one";
        let mut combined = cobs_encode_with_sentinel(p1);
        combined.extend_from_slice(&cobs_encode_with_sentinel(p2));
        let res = run_receive(&[&combined], 256);
        assert_eq!(res.len(), 2);
        assert_eq!(res[0].as_ref().unwrap(), p1);
        assert_eq!(res[1].as_ref().unwrap(), p2);
    }

    /// An oversize frame whose terminating sentinel is in the *same* chunk as
    /// the overflow point: the receiver returns `MessageTooLarge`, then on the
    /// next call decodes the frame that follows the sentinel.
    #[test]
    fn oversize_frame_with_inline_sentinel_recovers_to_next_frame() {
        // Build a > scratch-capacity COBS payload, then a sentinel, then a
        // valid following frame. Send the whole thing in one chunk.
        let oversize = vec![0xAAu8; BLE_RX_BUF + 8];
        let recovery = b"after-overflow";

        let mut chunk: StdVec<u8> = StdVec::new();
        chunk.extend_from_slice(&oversize);
        chunk.push(0); // sentinel terminating the oversize frame
        chunk.extend_from_slice(&cobs_encode_with_sentinel(recovery));

        // Chunks are bounded by `BLE_NOTIFY_PAYLOAD`; split the oversize
        // payload across multiple chunks. The split boundary doesn't matter
        // for the test as long as the eventual chunk crossing scratch_len
        // exceeds capacity.
        let chunks: StdVec<&[u8]> = chunk.chunks(BLE_NOTIFY_PAYLOAD).collect();
        let res = run_receive(&chunks, 256);

        // Expect: at least one Err(MessageTooLarge) followed by Ok(recovery).
        let mut saw_err = false;
        let mut saw_recovery = false;
        for r in &res {
            match r {
                Err(WireRxErrorKind::ReceivedMessageTooLarge) => saw_err = true,
                Ok(b) if b.as_slice() == recovery => saw_recovery = true,
                _ => {}
            }
        }
        assert!(saw_err, "expected a MessageTooLarge for the oversize frame");
        assert!(saw_recovery, "expected the next frame to decode after recovery");
    }
}
