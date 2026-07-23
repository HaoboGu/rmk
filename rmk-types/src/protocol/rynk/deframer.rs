//! Incremental COBS de-framing for the Rynk transport.
//!
//! [`Deframer`] cuts COBS frames back out of a received byte stream, decoding
//! in place and resyncing at the next `0x00` delimiter on any garbage. The
//! encode side lives in [`message`](super::message).

use super::message::RYNK_HEADER_SIZE;

/// Incremental COBS de-framer. Holds only cursors, never bytes, so the caller's
/// buffer stays put and is shared across all transports.
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
    /// First index not yet scanned for a delimiter, so each byte is examined once.
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

    /// Whether a partial frame is buffered. A caller reusing the buffer (e.g. to
    /// encode an outbound topic) must wait until this is `false`.
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
                // Overflow only when nothing can free space: with a consumed prefix
                // the caller's `tail()` compacts and the frame may yet complete.
                if self.frame_start == Some(0) && self.filled == buf.len() {
                    self.frame_start = None;
                }
                return None;
            };
            self.scan = delim + 1;
            let Some(start) = self.frame_start.replace(delim + 1) else {
                // Was discarding an oversized frame; its delimiter ends the drain.
                continue;
            };
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
    use crate::protocol::rynk::{Cmd, RynkHeader, RynkMessage};

    const CMD: Cmd = Cmd::from_raw(0x0102);

    /// COBS-encode one frame into `buf`, returning the framed length.
    fn encode(buf: &mut [u8], cmd: Cmd, seq: u8, payload: &[u8]) -> usize {
        RynkMessage::build(buf, RynkHeader { cmd, seq }, &payload)
            .unwrap()
            .frame()
            .len()
    }

    /// Encode into `df`'s tail and commit, as a transport read would.
    fn feed(df: &mut Deframer, buf: &mut [u8], cmd: Cmd, seq: u8, payload: &[u8]) {
        let tail = df.tail(buf);
        let n = encode(tail, cmd, seq, payload);
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
    fn reassembles_byte_by_byte() {
        // Encode once, then feed the stream one byte per commit — the cursor scan
        // must reassemble without re-inspecting old bytes.
        let mut src = [0u8; 64];
        let n = encode(&mut src, CMD, 7, &[9, 8, 7, 6]);

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
    fn frames_shorter_than_a_header_are_skipped() {
        // Every yielded frame holds at least a header — that is what lets
        // `RynkMessage::from_decoded` wrap it infallibly.
        let mut buf = [0u8; 64];
        let mut df = Deframer::new();
        let short = [
            0x00, // empty frame (stray delimiter)
            0x02, 0xAA, 0x00, // decodes to 1 byte
            0x03, 0xAA, 0xBB, 0x00, // decodes to 2 bytes
        ];
        df.tail(&mut buf)[..short.len()].copy_from_slice(&short);
        df.commit(short.len());
        assert!(df.next(&mut buf).is_none(), "sub-header frames must be skipped");

        // Header-only frame: [cmd_lo, cmd_hi, seq], no payload.
        let header_only = [0x04, 0x02, 0x01, 0x42, 0x00];
        df.tail(&mut buf)[..header_only.len()].copy_from_slice(&header_only);
        df.commit(header_only.len());
        let len = df.next(&mut buf).expect("header-only frame is valid");
        assert_eq!(len, RYNK_HEADER_SIZE);
        assert_eq!(Cmd::from_le_bytes([buf[0], buf[1]]), CMD);
        assert_eq!(buf[2], 0x42);
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
        // A delimiter-less run fills the buffer and is dropped as overflow;
        // its terminating 0x00 clears the drain, then a clean frame decodes.
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
        // A full buffer with a consumed prefix is not overflow: `next` must
        // return None so the caller compacts and the frame completes.
        let mut src = [0u8; 64];
        let n = encode(&mut src, CMD, 9, &[1, 2, 3, 4, 5]);

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
