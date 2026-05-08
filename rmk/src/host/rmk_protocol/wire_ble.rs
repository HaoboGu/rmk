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

pub struct BleWireTxInner {
    /// Scratch buffer used to serialize header + body before COBS-encoding.
    pub(crate) tx_buf: &'static mut [u8],
    /// Scratch buffer used to hold the COBS-encoded frame.
    pub(crate) cobs_buf: &'static mut [u8],
    /// Reply channel sender (heapless `Channel` provides `try_send`/`send`).
    pub(crate) replies: &'static embassy_sync::channel::Channel<crate::RawMutex, BleReplyFrame, 4>,
}

pub(crate) struct BleWireTx<M: RawMutex + 'static> {
    inner: &'static Mutex<M, BleWireTxInner>,
}

impl<M: RawMutex + 'static> BleWireTx<M> {
    pub(crate) fn new(inner: &'static Mutex<M, BleWireTxInner>) -> Self {
        Self { inner }
    }
}

impl<M: RawMutex + 'static> Clone for BleWireTx<M> {
    fn clone(&self) -> Self {
        Self { inner: self.inner }
    }
}

impl<M: RawMutex + 'static> WireTx for BleWireTx<M> {
    type Error = WireTxErrorKind;

    async fn send<T: Serialize + ?Sized>(&self, hdr: VarHeader, msg: &T) -> Result<(), Self::Error> {
        let mut guard = self.inner.lock().await;
        let BleWireTxInner {
            tx_buf,
            cobs_buf,
            replies,
        } = &mut *guard;

        // 1. Serialize header + body into tx_buf.
        let (hdr_used, remain) = hdr.write_to_slice(tx_buf).ok_or(WireTxErrorKind::Other)?;
        let hdr_len = hdr_used.len();
        let body_used = postcard::to_slice(msg, remain).map_err(|_| WireTxErrorKind::Other)?;
        let total = hdr_len + body_used.len();

        // 2. COBS-encode in place into cobs_buf and append the 0x00 sentinel.
        let encoded = cobs::try_encode(&tx_buf[..total], cobs_buf).map_err(|_| WireTxErrorKind::Other)?;
        if encoded + 1 > cobs_buf.len() {
            return Err(WireTxErrorKind::Other);
        }
        cobs_buf[encoded] = 0;
        let frame = &cobs_buf[..encoded + 1];

        // 3. Build a single owned frame and enqueue it for the notify task.
        let mut owned: BleReplyFrame = Vec::new();
        owned.extend_from_slice(frame).map_err(|_| WireTxErrorKind::Other)?;
        replies.send(owned).await;
        Ok(())
    }

    async fn send_raw(&self, buf: &[u8]) -> Result<(), Self::Error> {
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
pub(crate) struct BleWireRx {
    /// Source channel filled by `gatt_events_task` on every write to
    /// `output_data`.
    pub(crate) requests: &'static embassy_sync::channel::Channel<crate::RawMutex, BleRequestChunk, 4>,
    /// Scratch accumulator: holds the unconsumed tail of the most recent chunk
    /// plus any bytes carried over from the previous frame's leftover.
    pub(crate) scratch: &'static mut [u8],
    /// Number of valid bytes currently in `scratch`.
    pub(crate) scratch_len: usize,
}

impl BleWireRx {
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

impl WireRx for BleWireRx {
    type Error = WireRxErrorKind;

    async fn wait_connection(&mut self) {}

    async fn receive<'a>(&mut self, buf: &'a mut [u8]) -> Result<&'a mut [u8], Self::Error> {
        loop {
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
                // Frame exceeds scratch capacity; drop accumulated state and
                // continue from the next sentinel boundary.
                self.scratch_len = 0;
                if let Some(pos) = chunk.iter().position(|&b| b == 0) {
                    let tail = &chunk[pos + 1..];
                    if tail.len() <= self.scratch.len() {
                        self.scratch[..tail.len()].copy_from_slice(tail);
                        self.scratch_len = tail.len();
                    }
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
}
