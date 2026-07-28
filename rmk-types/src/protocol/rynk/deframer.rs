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
    /// Dead bytes at the buffer front: yielded frames, skipped garbage, drained overflow.
    consumed: usize,
    /// Valid bytes in the caller buffer.
    filled: usize,
    /// Draining an oversized frame: drop everything up to its closing delimiter.
    discarding: bool,
}

impl Deframer {
    pub const fn new() -> Self {
        Self {
            consumed: 0,
            filled: 0,
            discarding: false,
        }
    }

    /// Compact consumed bytes to the front and return the free tail to read into.
    pub fn tail<'b>(&mut self, buf: &'b mut [u8]) -> &'b mut [u8] {
        if self.consumed > 0 {
            buf.copy_within(self.consumed..self.filled, 0);
            self.filled -= self.consumed;
            self.consumed = 0;
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
        !self.discarding && self.consumed < self.filled
    }

    /// Move the undecoded remainder to the end of `buf`, freeing the front so a
    /// reply can be encoded in place without clobbering a pipelined tail.
    /// Returns the parked length; pair with [`unpark_pending`](Self::unpark_pending)
    /// once the reply is written. The overflow-drain state survives the pair.
    pub fn park_pending(&mut self, buf: &mut [u8]) -> usize {
        let len = self.filled - self.consumed;
        buf.copy_within(self.consumed..self.filled, buf.len() - len);
        self.consumed = 0;
        self.filled = 0;
        len
    }

    /// Restore bytes parked by [`park_pending`](Self::park_pending) to the
    /// front of `buf` and resume deframing them.
    pub fn unpark_pending(&mut self, buf: &mut [u8], parked: usize) {
        buf.copy_within(buf.len() - parked.., 0);
        self.consumed = 0;
        self.filled = parked;
    }

    /// Decode the next logical frame to `buf[..len]` and return `len`, or return
    /// `None` when more bytes are needed. A garbled or oversized frame is
    /// skipped to the next `0x00` delimiter.
    pub fn next(&mut self, buf: &mut [u8]) -> Option<usize> {
        loop {
            let Some(i) = buf[self.consumed..self.filled].iter().position(|&b| b == 0) else {
                // A frame filling the whole buffer can never complete: drop it and
                // drain to its delimiter. While draining, bytes are dead on arrival.
                // With a consumed prefix the caller's `tail()` compacts instead.
                if self.discarding || (self.consumed == 0 && self.filled == buf.len()) {
                    self.discarding = true;
                    self.consumed = self.filled;
                }
                return None;
            };
            let (start, delim) = (self.consumed, self.consumed + i);
            self.consumed = delim + 1;
            if self.discarding {
                // The drained frame's delimiter: back in sync.
                self.discarding = false;
                continue;
            }
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
    extern crate alloc;

    use super::*;
    use crate::protocol::rynk::{Cmd, RynkHeader, encode_frame};

    const CMD: Cmd = Cmd::from_raw(0x0102);

    /// COBS-encode one frame into `buf`, returning the framed length.
    fn encode(buf: &mut [u8], cmd: Cmd, seq: u8, payload: &[u8]) -> usize {
        encode_frame(buf, RynkHeader { cmd, seq }, &payload).unwrap()
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
        // Encode once, then feed the stream one byte per commit — the frame must
        // reassemble across arbitrarily small reads.
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
    fn park_and_unpark_preserve_a_pipelined_frame() {
        let mut buf = [0u8; 128];
        let mut df = Deframer::new();
        feed(&mut df, &mut buf, CMD, 1, &[1]);
        feed(&mut df, &mut buf, CMD, 2, &[2, 2]);

        let len = df.next(&mut buf).expect("first frame");
        assert_frame(&buf[..len], CMD, 1, &[1]);

        // Serve the frame: park the tail, scribble a reply over the freed window.
        let parked = df.park_pending(&mut buf);
        assert!(parked > 0);
        let window = buf.len() - parked;
        buf[..window].iter_mut().for_each(|b| *b = 0xEE);
        df.unpark_pending(&mut buf, parked);

        let len = df.next(&mut buf).expect("second frame survives the reply");
        assert_frame(&buf[..len], CMD, 2, &[2, 2]);
        assert!(df.next(&mut buf).is_none());
    }

    #[test]
    fn park_preserves_a_partial_frame() {
        let mut src = [0u8; 64];
        let n = encode(&mut src, CMD, 9, &[7, 7, 7]);

        let mut buf = [0u8; 64];
        let mut df = Deframer::new();
        feed(&mut df, &mut buf, CMD, 8, &[5]);
        // Half of the next frame arrives in the same read.
        let split = n / 2;
        df.tail(&mut buf)[..split].copy_from_slice(&src[..split]);
        df.commit(split);

        let len = df.next(&mut buf).expect("complete frame");
        assert_frame(&buf[..len], CMD, 8, &[5]);
        let parked = df.park_pending(&mut buf);
        assert!(parked > 0);
        let window = buf.len() - parked;
        buf[..window].iter_mut().for_each(|b| *b = 0xEE);
        df.unpark_pending(&mut buf, parked);

        // The remaining bytes complete the split frame.
        df.tail(&mut buf)[..n - split].copy_from_slice(&src[split..n]);
        df.commit(n - split);
        let len = df.next(&mut buf).expect("split frame completes after the park");
        assert_frame(&buf[..len], CMD, 9, &[7, 7, 7]);
    }

    #[test]
    fn park_preserves_the_overflow_drain() {
        let mut buf = [0u8; 32];
        let mut df = Deframer::new();
        df.tail(&mut buf).iter_mut().for_each(|b| *b = 0xFF);
        df.commit(32);
        assert!(df.next(&mut buf).is_none(), "overflow: draining");

        // A park/unpark round trip must not forget the drain.
        let parked = df.park_pending(&mut buf);
        assert_eq!(parked, 0, "drained bytes are dead, nothing to park");
        df.unpark_pending(&mut buf, parked);

        // More doomed bytes, the drain delimiter, then a clean frame.
        df.tail(&mut buf)[..2].copy_from_slice(&[0xFF, 0x00]);
        df.commit(2);
        feed(&mut df, &mut buf, CMD, 3, &[9]);
        let len = df.next(&mut buf).expect("frame after the drain clears");
        assert_frame(&buf[..len], CMD, 3, &[9]);
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
    fn has_pending_tracks_partial_frames_not_garbage() {
        let mut buf = [0u8; 32];
        let mut df = Deframer::new();
        assert!(!df.has_pending(), "empty buffer: nothing pending");

        // Feed a frame minus its delimiter: pending until it completes.
        let mut src = [0u8; 32];
        let n = encode(&mut src, CMD, 1, &[1, 2, 3]);
        df.tail(&mut buf)[..n - 1].copy_from_slice(&src[..n - 1]);
        df.commit(n - 1);
        assert!(df.next(&mut buf).is_none());
        assert!(df.has_pending(), "half-received frame is pending");

        df.tail(&mut buf)[0] = 0x00;
        df.commit(1);
        let len = df.next(&mut buf).expect("frame completes");
        assert_frame(&buf[..len], CMD, 1, &[1, 2, 3]);
        assert!(df.next(&mut buf).is_none());
        assert!(!df.has_pending(), "consumed frame is not pending");

        // A drained oversized frame is not pending: the buffer is reusable.
        df.tail(&mut buf).iter_mut().for_each(|b| *b = 0xFF);
        df.commit(32);
        assert!(df.next(&mut buf).is_none());
        assert!(!df.has_pending(), "overflow drain is not pending");
    }

    #[test]
    fn discard_drain_preserves_bytes_committed_after_the_scan() {
        let mut buf = [0u8; 32];
        let mut df = Deframer::new();
        df.tail(&mut buf).iter_mut().for_each(|b| *b = 0xFF);
        df.commit(32);
        assert!(df.next(&mut buf).is_none(), "overflow: enter discard mode");

        // The doomed frame's delimiter and a real frame arrive in one read;
        // tail() must reclaim all drained garbage first.
        let mut src = [0u8; 32];
        let n = encode(&mut src, CMD, 3, &[7]);
        {
            let tail = df.tail(&mut buf);
            assert_eq!(tail.len(), 32, "drained garbage is reclaimed by tail()");
            tail[0] = 0x00;
            tail[1..=n].copy_from_slice(&src[..n]);
        }
        df.commit(1 + n);
        let len = df.next(&mut buf).expect("frame after the drain delimiter");
        assert_frame(&buf[..len], CMD, 3, &[7]);
        assert!(df.next(&mut buf).is_none());
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

    /// Deterministic xorshift64* — dependency-free randomness for the fuzz tests.
    struct Rng(u64);

    impl Rng {
        fn next_u64(&mut self) -> u64 {
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
        }

        fn below(&mut self, n: usize) -> usize {
            (self.next_u64() % n as u64) as usize
        }
    }

    #[test]
    fn fuzz_recovers_every_wellformed_frame() {
        use alloc::vec;
        use alloc::vec::Vec;

        for seed in 1..=64u64 {
            for &cap in &[16usize, 24, 48, 96] {
                let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
                // Stream = frames that fit the buffer (all must come back out) mixed
                // with junk the deframer must skip: stray delimiters, undecodable
                // garbage, and delimiter-less runs longer than the buffer.
                let mut stream: Vec<u8> = Vec::new();
                let mut expected: Vec<Vec<u8>> = Vec::new();
                for seq in 0..30u8 {
                    match rng.below(4) {
                        0 => stream.push(0x00),
                        1 => {
                            // A 0xFF code byte claims 254 data bytes; a shorter segment can't decode.
                            let glen = 1 + rng.below(cap - 2);
                            stream.push(0xFF);
                            for _ in 0..glen {
                                stream.push(1 + rng.below(255) as u8);
                            }
                            stream.push(0x00);
                        }
                        2 => {
                            // Longer than the buffer with no delimiter: dropped via the overflow drain.
                            for _ in 0..cap + 1 + rng.below(cap) {
                                stream.push(0xFF);
                            }
                            stream.push(0x00);
                        }
                        _ => {
                            let mut payload = [0u8; 96];
                            let plen = rng.below(cap);
                            payload[..plen].iter_mut().for_each(|b| *b = rng.next_u64() as u8);
                            let mut tmp = [0u8; 256];
                            let n = encode(&mut tmp, CMD, seq, &payload[..plen]);
                            if n <= cap {
                                stream.extend_from_slice(&tmp[..n]);
                                // The expected logical frame is the decode of the framed bytes.
                                let mut logical = tmp[..n - 1].to_vec();
                                let llen = cobs::decode_in_place(&mut logical).unwrap();
                                logical.truncate(llen);
                                expected.push(logical);
                            } else {
                                stream.push(0x00);
                            }
                        }
                    }
                }

                // Standard transport loop: read a chunk into the tail, cut out
                // every completed frame.
                let mut buf = vec![0u8; cap];
                let mut df = Deframer::new();
                let mut got: Vec<Vec<u8>> = Vec::new();
                let mut pos = 0;
                while pos < stream.len() {
                    let tail = df.tail(&mut buf);
                    assert!(!tail.is_empty(), "reader must never be offered an empty tail");
                    let n = tail.len().min(1 + rng.below(7)).min(stream.len() - pos);
                    tail[..n].copy_from_slice(&stream[pos..pos + n]);
                    pos += n;
                    df.commit(n);
                    while let Some(len) = df.next(&mut buf) {
                        got.push(buf[..len].to_vec());
                    }
                }
                assert_eq!(got, expected, "seed {seed} cap {cap}");
            }
        }
    }
}
