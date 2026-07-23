//! COBS framing for the Rynk transport.
//!
//! A frame is COBS-encoded and terminated by a single `0x00` delimiter, so the
//! byte stream is self-synchronizing: the next `0x00` is always a real frame
//! boundary. That is what lets a desynced link (OS port-probing, stale bytes,
//! a truncated write) recover on its own instead of dying.
//!
//! - [`encode_frame`] serializes `[cmd, seq] ++ postcard(payload)` straight into
//!   the caller's buffer in one fused pass — no scratch buffer.
//! - [`Deframer`] cuts frames back out of a byte stream, decoding in place and
//!   resyncing at the next delimiter on any garbage.

use cobs::max_encoding_length;
use postcard::ser_flavors::{Cobs, Flavor, Slice};
use serde::Serialize;

#[cfg(test)]
use super::command::Cmd;
use super::error::RynkError;
use super::message::{RYNK_HEADER_SIZE, RynkHeader};

/// Physical buffer size that holds the COBS-encoded form of a `logical_len`
/// frame — the encoded body plus the trailing `0x00` delimiter. `RYNK_BUFFER_SIZE`
/// and the advertised `max_payload_size` stay the *logical* budget; only the
/// on-wire buffers grow by this overhead.
pub const fn frame_capacity(logical_len: usize) -> usize {
    max_encoding_length(logical_len) + 1
}

/// Physical buffer size for a worst-case firmware frame (`RYNK_BUFFER_SIZE`).
pub const RYNK_FRAME_BUFFER_SIZE: usize = frame_capacity(crate::constants::RYNK_BUFFER_SIZE);

/// COBS-frame `[cmd_lo, cmd_hi, seq] ++ postcard(value)` into `buf`, returning
/// the framed length (the frame is `buf[..len]`, delimiter included).
///
/// The header bytes are pushed *through* the COBS encoder along with the
/// payload, so a `0x00` in the header (e.g. cmd `0x0004`) is stuffed like any
/// other byte and never aliases the delimiter.
pub fn encode_frame<T: Serialize>(buf: &mut [u8], header: RynkHeader, value: &T) -> Result<usize, RynkError> {
    let [cmd_lo, cmd_hi] = header.cmd.to_le_bytes();
    let mut ser = postcard::Serializer {
        output: Cobs::try_new(Slice::new(buf)).map_err(|_| RynkError::Internal)?,
    };
    ser.output
        .try_extend(&[cmd_lo, cmd_hi, header.seq])
        .map_err(|_| RynkError::Internal)?;
    value.serialize(&mut ser).map_err(|_| RynkError::Internal)?;
    let framed = ser.output.finalize().map_err(|_| RynkError::Internal)?;
    Ok(framed.len())
}

/// Incremental COBS de-framer. Holds only cursors, never bytes, so the caller's
/// buffer (a firmware stack array, the host's `Vec`, or a test array) stays put
/// and is shared across all transports.
///
/// Usage: read into [`tail`](Self::tail), [`commit`](Self::commit) the count,
/// then drain frames with [`next`](Self::next):
///
/// ```ignore
/// let n = rx.read(df.tail(&mut buf)).await?;
/// df.commit(n);
/// while let Some(len) = df.next(&mut buf) {
///     handle(&mut buf[..len]); // decoded logical frame [cmd, seq, payload]
/// }
/// ```
pub struct Deframer {
    /// Start of the in-progress frame, or `None` while discarding an oversized
    /// frame until its delimiter arrives.
    frame_start: Option<usize>,
    /// Valid bytes in the caller buffer.
    filled: usize,
    /// Next index not yet inspected for a delimiter — the scan cursor, so each
    /// byte is examined once (O(n), not O(n²)).
    scan: usize,
}

impl Deframer {
    pub const fn new() -> Self {
        Self {
            frame_start: Some(0),
            filled: 0,
            scan: 0,
        }
    }

    /// Compact consumed bytes to the front and return the free tail to read into.
    pub fn tail<'b>(&mut self, buf: &'b mut [u8]) -> &'b mut [u8] {
        // During normal framing everything before `frame_start` is consumed. In
        // discard mode everything already scanned is known not to be a delimiter.
        let consumed = self.frame_start.unwrap_or(self.scan);
        if consumed > 0 {
            buf.copy_within(consumed..self.filled, 0);
            self.filled -= consumed;
            self.scan -= consumed;
            if self.frame_start.is_some() {
                self.frame_start = Some(0);
            }
        }
        &mut buf[self.filled..]
    }

    /// Mark `n` freshly read bytes as valid.
    pub fn commit(&mut self, n: usize) {
        self.filled += n;
    }

    /// Valid bytes currently buffered — for callers that grow the backing
    /// buffer before [`tail`](Self::tail).
    pub fn filled(&self) -> usize {
        self.filled
    }

    /// Whether bytes are buffered that don't yet form a complete frame. A
    /// caller that reuses the buffer for something else (e.g. encoding an
    /// outbound topic into it) must wait until this is `false`, or it would
    /// clobber a half-received frame.
    pub fn has_pending(&self) -> bool {
        self.frame_start.is_some_and(|start| start < self.filled)
    }

    /// Decode the next logical frame to `buf[..len]` and return `len`, or return
    /// `None` when more bytes are needed. A garbled or oversized frame is
    /// skipped to the next `0x00` delimiter.
    pub fn next(&mut self, buf: &mut [u8]) -> Option<usize> {
        loop {
            let Some(delim) = buf[self.scan..self.filled]
                .iter()
                .position(|&b| b == 0)
                .map(|i| self.scan + i)
            else {
                // No delimiter in the unscanned bytes; only re-examine new bytes next time.
                self.scan = self.filled;
                // Overflow only when nothing can free space: the buffer is full AND
                // there is nothing consumed at the front to reclaim by compaction. If
                // `frame_start > 0`, the caller's `tail()` compacts and the frame
                // completes; an alloc caller grows instead. Otherwise a frame that
                // fills the buffer right up to (but not including) its delimiter
                // would be wrongly dropped.
                if self.frame_start == Some(0) && self.filled == buf.len() {
                    self.frame_start = None;
                }
                return None;
            };
            let Some(start) = self.frame_start else {
                self.frame_start = Some(delim + 1);
                self.scan = delim + 1;
                continue;
            };
            self.frame_start = Some(delim + 1);
            self.scan = delim + 1;
            match cobs::decode_in_place(&mut buf[start..delim]) {
                Ok(len) if len >= RYNK_HEADER_SIZE => {
                    if start > 0 {
                        buf.copy_within(start..start + len, 0);
                    }
                    return Some(len);
                }
                // Empty/stray delimiter or a frame too short to hold a header: skip and resync.
                _ => {}
            }
        }
    }
}

impl Default for Deframer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CMD: Cmd = Cmd::from_raw(0x0102);

    /// Encode into `df`'s tail and commit, as a transport read would.
    fn feed(df: &mut Deframer, buf: &mut [u8], cmd: Cmd, seq: u8, payload: &[u8]) {
        let tail = df.tail(buf);
        let n = encode_frame(tail, RynkHeader { cmd, seq }, &payload).unwrap();
        df.commit(n);
    }

    /// Assert a frame decodes to the expected header + payload.
    fn assert_frame(frame: &[u8], cmd: Cmd, seq: u8, payload: &[u8]) {
        assert_eq!(Cmd::from_le_bytes([frame[0], frame[1]]), cmd);
        assert_eq!(frame[2], seq);
        let decoded: heapless::Vec<u8, 32> = postcard::from_bytes(&frame[RYNK_HEADER_SIZE..]).unwrap();
        assert_eq!(&decoded[..], payload);
    }

    #[test]
    fn round_trip() {
        let mut buf = [0u8; 64];
        let mut df = Deframer::new();
        feed(&mut df, &mut buf, CMD, 0x42, &[1, 2, 3]);
        let len = df.next(&mut buf).expect("one frame");
        assert_frame(&buf[..len], CMD, 0x42, &[1, 2, 3]);
        assert!(df.next(&mut buf).is_none(), "no second frame");
    }

    #[test]
    fn frame_never_contains_the_delimiter() {
        // cmd 0x0004 has a zero byte; the encoded frame must not carry a bare 0x00
        // except the trailing delimiter.
        let mut buf = [0u8; 64];
        let n = encode_frame(
            &mut buf,
            RynkHeader {
                cmd: Cmd::from_raw(0x0004),
                seq: 0,
            },
            &[0u8, 0, 0],
        )
        .unwrap();
        assert_eq!(buf[n - 1], 0, "frame is delimiter-terminated");
        assert!(buf[..n - 1].iter().all(|&b| b != 0), "no interior 0x00");
    }

    #[test]
    fn reassembles_byte_by_byte() {
        // Encode once, then feed the stream one byte per commit — the cursor scan
        // must reassemble without re-inspecting old bytes.
        let mut src = [0u8; 64];
        let payload: &[u8] = &[9, 8, 7, 6];
        let n = encode_frame(&mut src, RynkHeader { cmd: CMD, seq: 7 }, &payload).unwrap();

        let mut buf = [0u8; 64];
        let mut df = Deframer::new();
        for i in 0..n {
            df.tail(&mut buf)[0] = src[i];
            df.commit(1);
            if i + 1 < n {
                assert!(df.next(&mut buf).is_none(), "no frame before the delimiter");
            }
        }
        let len = df.next(&mut buf).expect("frame after the last byte");
        assert_frame(&buf[..len], CMD, 7, &[9, 8, 7, 6]);
    }

    #[test]
    fn two_pipelined_frames_in_one_buffer() {
        let mut buf = [0u8; 128];
        let mut df = Deframer::new();
        feed(&mut df, &mut buf, CMD, 1, &[1]);
        feed(&mut df, &mut buf, CMD, 2, &[2, 2]);

        let len = df.next(&mut buf).expect("first frame");
        assert_frame(&buf[..len], CMD, 1, &[1]);
        let len = df.next(&mut buf).expect("second frame");
        assert_frame(&buf[..len], CMD, 2, &[2, 2]);
        assert!(df.next(&mut buf).is_none());
    }

    #[test]
    fn resyncs_past_injected_garbage() {
        // [frame A][random non-zero bytes][0x00][frame B] — A decodes, the garbage
        // segment fails to decode and is skipped at its terminating 0x00, B decodes.
        let mut buf = [0u8; 128];
        let mut df = Deframer::new();
        feed(&mut df, &mut buf, CMD, 10, &[0xAA, 0xBB]);
        // Inject garbage terminated by a delimiter, straight into the tail.
        let tail = df.tail(&mut buf);
        tail[..4].copy_from_slice(&[0xDE, 0xAD, 0xBE, 0x00]);
        df.commit(4);
        feed(&mut df, &mut buf, CMD, 11, &[0xCC]);

        let len = df.next(&mut buf).expect("frame A");
        assert_frame(&buf[..len], CMD, 10, &[0xAA, 0xBB]);
        let len = df.next(&mut buf).expect("frame B survives the garbage");
        assert_frame(&buf[..len], CMD, 11, &[0xCC]);
    }

    #[test]
    fn resyncs_after_buffer_overflow() {
        // A delimiter-less run fills the buffer (overflow → drop the fragment).
        // The fragment's own terminating 0x00 clears the drain, then a clean
        // frame after it decodes.
        let mut buf = [0u8; 32];
        let mut df = Deframer::new();
        // Fill the whole buffer with non-zero bytes: no delimiter, forces overflow.
        df.tail(&mut buf).iter_mut().for_each(|b| *b = 0xFF);
        df.commit(32);
        assert!(df.next(&mut buf).is_none(), "overflow yields no frame");
        // The overflowed fragment's terminating delimiter, consumed by draining.
        df.tail(&mut buf)[0] = 0x00;
        df.commit(1);
        // Now a real frame resyncs and decodes.
        feed(&mut df, &mut buf, CMD, 12, &[5, 6]);
        let len = df.next(&mut buf).expect("frame after overflow resync");
        assert_frame(&buf[..len], CMD, 12, &[5, 6]);
    }

    #[test]
    fn compacts_instead_of_dropping_a_buffer_filling_frame() {
        // A leading empty frame followed by a frame whose bytes fill the buffer
        // right up to — but not including — its delimiter. The Deframer must
        // return None so the caller compacts (freeing the skipped byte) and the
        // frame completes, rather than mistaking a full buffer for overflow and
        // dropping a valid frame.
        let mut src = [0u8; 64];
        let payload: &[u8] = &[1, 2, 3, 4, 5];
        let n = encode_frame(&mut src, RynkHeader { cmd: CMD, seq: 9 }, &payload).unwrap();

        // Sized to hold exactly [0x00 skip] ++ [frame body without its delimiter].
        let cap = 1 + (n - 1);
        let mut store = [0u8; 64];
        let mut df = Deframer::new();
        {
            let tail = df.tail(&mut store[..cap]);
            tail[0] = 0x00; // empty frame → skipped, advancing head past it
            tail[1..].copy_from_slice(&src[..n - 1]); // frame body, delimiter withheld
            df.commit(cap);
        }
        assert!(
            df.next(&mut store[..cap]).is_none(),
            "buffer full with a consumed prefix must compact, not overflow"
        );
        {
            let tail = df.tail(&mut store[..cap]);
            assert!(!tail.is_empty(), "compaction must free the skipped byte");
            tail[0] = 0x00; // the withheld delimiter
            df.commit(1);
        }
        let len = df.next(&mut store[..cap]).expect("frame completes after compaction");
        assert_frame(&store[..len], CMD, 9, &[1, 2, 3, 4, 5]);
    }
}
