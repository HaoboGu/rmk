//! Rynk wire-format message.
//!
//! One frame is the 3-byte header plus a postcard-encoded payload,
//! COBS-encoded and `0x00`-terminated on the wire (layout in the
//! [module docs](super)). [`encode_frame`] serializes the logical frame a few
//! bytes into the caller's buffer, then COBS-encodes it forward in place — one
//! physical buffer, without monomorphizing a COBS serializer for every payload
//! type. A [`Deframer`](super::Deframer) cuts frames back out of the received
//! stream, and [`RynkMessage`] wraps one received frame to answer it in place.

use serde::Serialize;
use serde::de::DeserializeOwned;

use super::RynkError;
use super::command::Cmd;

/// Size in bytes of the fixed Rynk header.
pub const RYNK_HEADER_SIZE: usize = 3;

/// The fixed header of a [`RynkMessage`].
#[derive(Debug, Clone, Copy)]
pub struct RynkHeader {
    pub cmd: Cmd,
    pub seq: u8,
}

impl RynkHeader {
    /// Decode the 3 header bytes.
    pub const fn parse(bytes: &[u8; RYNK_HEADER_SIZE]) -> Self {
        Self {
            cmd: Cmd::from_le_bytes([bytes[0], bytes[1]]),
            seq: bytes[2],
        }
    }

    /// Convert the header to bytes
    pub const fn to_bytes(&self) -> [u8; RYNK_HEADER_SIZE] {
        let cmd_bytes = self.cmd.to_le_bytes();
        [cmd_bytes[0], cmd_bytes[1], self.seq]
    }
}

/// Largest logical frame (header + payload) whose streaming-COBS encoding —
/// worst case `len + len/254 + 2` for an all-nonzero frame, trailing delimiter
/// included — fits a `physical`-byte buffer; 0 if none does.
pub const fn max_logical_len(physical: usize) -> usize {
    let mut len = 0;
    while len + 1 + (len + 1) / 254 + 2 <= physical {
        len += 1;
    }
    len
}

/// Largest request/response payload either peer can carry in one frame: what
/// remains of a `RYNK_BUFFER_SIZE`-byte frame buffer after worst-case COBS
/// overhead and the 3-byte header. Advertised to hosts as `max_payload_size`.
pub const RYNK_MAX_PAYLOAD_SIZE: usize = {
    let logical = max_logical_len(crate::constants::RYNK_BUFFER_SIZE);
    // Assert before subtracting so a too-small buffer fails with this message,
    // not a const-eval underflow.
    assert!(
        logical >= RYNK_HEADER_SIZE,
        "rynk_buffer_size is too small for a COBS-framed header; increase it"
    );
    logical - RYNK_HEADER_SIZE
};

/// Start offset that leaves enough spare bytes in front of the logical frame
/// to absorb worst-case COBS growth when [`encode_shifted`] runs forward.
fn logical_start(physical_len: usize) -> Result<usize, RynkError> {
    let mut start = 1;
    while physical_len > start + 1 && cobs::max_encoding_overhead(physical_len - start - 1) > start {
        start += 1;
    }
    if physical_len <= start + 1 {
        Err(RynkError::Internal)
    } else {
        Ok(start)
    }
}

/// Write `[cmd, seq]` (plus postcard's `Result` tag: 0 = `Ok`, 1 = `Err`) at
/// the logical start; return that offset and the payload slice after it.
#[inline(never)]
fn begin_frame(buf: &mut [u8], header: RynkHeader, result_tag: Option<u8>) -> Result<(usize, &mut [u8]), RynkError> {
    let start = logical_start(buf.len())?;
    let logical_capacity = buf.len() - start - 1;
    let prefix_len = RYNK_HEADER_SIZE + usize::from(result_tag.is_some());
    if logical_capacity < prefix_len {
        return Err(RynkError::Internal);
    }
    buf[start..start + RYNK_HEADER_SIZE].copy_from_slice(&header.to_bytes());
    if let Some(tag) = result_tag {
        buf[start + RYNK_HEADER_SIZE] = tag;
    }
    Ok((start, &mut buf[start + prefix_len..start + logical_capacity]))
}

/// COBS-encode `buf[start..start + len]` forward into `buf[..]` and append the
/// delimiter, returning the framed length. The write cursor never catches the
/// read cursor because `start` covers the worst-case COBS overhead.
fn encode_shifted(buf: &mut [u8], start: usize, len: usize) -> Result<usize, RynkError> {
    let end = start.checked_add(len).ok_or(RynkError::Internal)?;
    if end > buf.len() {
        return Err(RynkError::Internal);
    }

    let mut read = start;
    let mut write = 1;
    let mut code_index = 0;
    let mut code = 1u8;
    while read < end {
        let byte = buf[read];
        read += 1;
        if byte == 0 {
            buf[code_index] = code;
            code_index = write;
            write += 1;
            code = 1;
        } else {
            if write >= buf.len() {
                return Err(RynkError::Internal);
            }
            buf[write] = byte;
            write += 1;
            code += 1;
            if code == 0xff {
                buf[code_index] = code;
                if read < end {
                    code_index = write;
                    write += 1;
                    code = 1;
                } else {
                    code_index = usize::MAX;
                }
            }
        }
    }

    if code_index != usize::MAX {
        buf[code_index] = code;
    }
    if write >= buf.len() {
        return Err(RynkError::Internal);
    }
    buf[write] = 0;
    Ok(write + 1)
}

/// COBS-encode the frame `header ++ postcard(value)` into `buf`, returning the
/// total framed length. The header is encoded with the payload, so a `0x00` in
/// it (e.g. cmd `0x0004`) never aliases the delimiter.
pub fn encode_frame<T: Serialize>(buf: &mut [u8], header: RynkHeader, value: &T) -> Result<usize, RynkError> {
    let (start, payload) = begin_frame(buf, header, None)?;
    let payload_len = postcard::to_slice(value, payload)
        .map_err(|_| RynkError::Internal)?
        .len();
    encode_shifted(buf, start, RYNK_HEADER_SIZE + payload_len)
}

/// One page of bulk-response items. Serializes as a postcard seq — the wire
/// shape of a `Vec` of the items — streaming from the iterator instead of
/// materializing one.
struct BulkItems<I>(I);

impl<I> Serialize for BulkItems<I>
where
    I: ExactSizeIterator + Clone,
    I::Item: Serialize,
{
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        let mut seq = ser.serialize_seq(Some(self.0.len()))?;
        for item in self.0.clone() {
            seq.serialize_element(&item)?;
        }
        seq.end()
    }
}

/// One received frame, answered in place: wrap the decoded logical frame
/// (`[cmd, seq, payload]`), decode the request out of it, then `encode_*` the
/// reply over it. The header is cached because the reply overwrites the buffer.
pub struct RynkMessage<'a> {
    buf: &'a mut [u8],
    header: RynkHeader,
    /// Valid prefix of `buf`: the logical frame on arrival, the COBS-framed
    /// reply after `encode_*`.
    len: usize,
}

impl<'a> RynkMessage<'a> {
    /// Wrap a decoded logical frame occupying `buf[..len]`; the rest of `buf` is
    /// scratch the `encode_*` replies may grow into. The caller guarantees
    /// `len >= RYNK_HEADER_SIZE`, as [`Deframer::next`](super::Deframer::next) does.
    pub fn from_decoded(buf: &'a mut [u8], len: usize) -> Self {
        debug_assert!(len >= RYNK_HEADER_SIZE && len <= buf.len());
        let header = RynkHeader::parse(buf.first_chunk().unwrap());
        Self { buf, header, len }
    }

    /// The decoded header.
    pub const fn header(&self) -> RynkHeader {
        self.header
    }

    /// The COBS-encoded reply frame, ready to transmit. Valid after `encode_*`.
    pub fn frame(&self) -> &[u8] {
        &self.buf[..self.len]
    }

    /// The decoded payload bytes. Valid on a received (decoded) message.
    pub fn payload(&self) -> &[u8] {
        &self.buf[RYNK_HEADER_SIZE..self.len]
    }

    /// Decode the request payload.
    pub fn decode_request<T: DeserializeOwned>(&self) -> Result<T, RynkError> {
        postcard::from_bytes(self.payload()).map_err(|_| RynkError::Malformed)
    }

    /// Encode `Ok(value)` as the response frame, replacing the buffer contents.
    pub fn encode_response<T: Serialize>(&mut self, value: &T) -> Result<(), RynkError> {
        let (start, payload) = begin_frame(self.buf, self.header, Some(0))?;
        let payload_len = postcard::to_slice(value, payload)
            .map_err(|_| RynkError::Internal)?
            .len();
        self.len = encode_shifted(self.buf, start, RYNK_HEADER_SIZE + 1 + payload_len)?;
        Ok(())
    }

    /// Encode `Err(err)` as the response frame.
    pub fn encode_error(&mut self, err: RynkError) {
        let encoded = (|| {
            let (start, payload) = begin_frame(self.buf, self.header, Some(1))?;
            let payload_len = postcard::to_slice(&err, payload)
                .map_err(|_| RynkError::Internal)?
                .len();
            encode_shifted(self.buf, start, RYNK_HEADER_SIZE + 1 + payload_len)
        })();
        self.len = encoded.unwrap_or(0);
    }

    /// Encode `Ok(items)` as a bulk response frame, serializing the page into
    /// the frame buffer and COBS-encoding it in place — no `Vec` needed.
    pub fn encode_bulk<I>(&mut self, items: I) -> Result<(), RynkError>
    where
        I: ExactSizeIterator + Clone,
        I::Item: Serialize,
    {
        let (start, payload) = begin_frame(self.buf, self.header, Some(0))?;
        let payload_len = postcard::to_slice(&BulkItems(items), payload)
            .map_err(|_| RynkError::Internal)?
            .len();
        self.len = encode_shifted(self.buf, start, RYNK_HEADER_SIZE + 1 + payload_len)?;
        Ok(())
    }
}

impl<'a> TryFrom<&'a mut [u8]> for RynkMessage<'a> {
    type Error = RynkError;

    /// Wrap already-decoded logical frame bytes (the host channel path).
    fn try_from(buf: &'a mut [u8]) -> Result<Self, RynkError> {
        if buf.len() < RYNK_HEADER_SIZE {
            return Err(RynkError::Malformed);
        }
        let len = buf.len();
        Ok(Self::from_decoded(buf, len))
    }
}

#[cfg(test)]
mod tests {
    use postcard::ser_flavors::{Cobs, Flavor, Slice};

    use super::*;

    #[test]
    fn encoded_frame_round_trips_through_cobs() {
        // GetVersion = 0x0001, so cmd_hi is 0x00 — a good test that COBS carries
        // an interior zero without it aliasing the delimiter.
        let mut buf = [0u8; 64];
        let framed_len = encode_frame(
            &mut buf,
            RynkHeader {
                cmd: Cmd::GetVersion,
                seq: 0x42,
            },
            &[1u8, 2, 3, 4],
        )
        .unwrap();
        // The wire frame is delimiter-terminated with no interior 0x00.
        assert_eq!(buf[framed_len - 1], 0);
        assert!(buf[..framed_len - 1].iter().all(|&b| b != 0));

        // Decode it back to the logical [cmd, seq, payload].
        let n = cobs::decode_in_place(&mut buf[..framed_len - 1]).unwrap();
        let header = RynkHeader::parse(buf[..RYNK_HEADER_SIZE].try_into().unwrap());
        assert_eq!(header.cmd, Cmd::GetVersion);
        assert_eq!(header.seq, 0x42);
        // postcard encodes [u8; 4] as 4 bare bytes.
        assert_eq!(&buf[RYNK_HEADER_SIZE..n], &[1, 2, 3, 4]);
    }

    #[test]
    fn encode_rejects_short_buffer() {
        // Too small to hold the COBS-framed 3-byte header.
        let mut buf = [0u8; 2];
        assert_eq!(
            encode_frame(
                &mut buf,
                RynkHeader {
                    cmd: Cmd::GetVersion,
                    seq: 0
                },
                &()
            )
            .err(),
            Some(RynkError::Internal),
        );
    }

    #[test]
    fn encode_bulk_streams_the_ok_sequence_shape() {
        // The streamed bulk reply must be byte-identical to a plain
        // `Ok(sequence)` value, or hosts can't decode it.
        let header = RynkHeader {
            cmd: Cmd::GetKeymapBulk,
            seq: 7,
        };
        let mut buf = [0u8; 64];
        buf[..RYNK_HEADER_SIZE].copy_from_slice(&header.to_bytes());
        let mut msg = RynkMessage::from_decoded(&mut buf, RYNK_HEADER_SIZE);
        msg.encode_bulk([1u8, 2, 3].into_iter().map(|b| b * 2)).unwrap();

        let mut expected = [0u8; 64];
        let n = encode_frame(&mut expected, header, &Ok::<&[u8], RynkError>(&[2, 4, 6])).unwrap();
        assert_eq!(msg.frame(), &expected[..n]);
    }

    #[test]
    fn try_from_rejects_short_buffer() {
        let mut buf = [0u8; RYNK_HEADER_SIZE - 1];
        assert_eq!(RynkMessage::try_from(&mut buf[..]).err(), Some(RynkError::Malformed));
    }

    #[test]
    fn try_from_accepts_unknown_discriminant() {
        let mut buf = [0u8; RYNK_HEADER_SIZE];
        buf[0..2].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let msg = RynkMessage::try_from(&mut buf[..]).unwrap();
        assert_eq!(msg.header().cmd, Cmd::from_raw(0xFFFF));
    }

    #[test]
    fn decoded_payload_spans_header_to_len() {
        let mut buf = [0u8; 8];
        buf[0..2].copy_from_slice(&Cmd::SetDefaultLayer.to_le_bytes());
        buf[2] = 0x34;
        buf[3..].copy_from_slice(&[0xAA, 0xBB, 0xCC, 0xDD, 0xEE]);

        let msg = RynkMessage::try_from(&mut buf[..]).unwrap();
        assert_eq!(msg.header().cmd, Cmd::SetDefaultLayer);
        assert_eq!(msg.header().seq, 0x34);
        assert_eq!(msg.payload(), &[0xAA, 0xBB, 0xCC, 0xDD, 0xEE]);
    }

    #[test]
    fn max_logical_len_is_exact_for_streaming_cobs() {
        // COBS worst case is an all-nonzero frame. For each physical size,
        // max_logical_len bytes must stream-encode into it and one byte more
        // must not — the inverse of the encoder's worst case is exact, including
        // at the 254-block boundaries where streaming COBS costs one byte over
        // one-shot encoding.
        let nonzero = [0x41u8; 600];
        let encodes_into = |logical: usize, physical: usize| {
            let mut store = [0u8; 1024];
            let mut ser = postcard::Serializer {
                output: Cobs::try_new(Slice::new(&mut store[..physical])).unwrap(),
            };
            ser.output.try_extend(&nonzero[..logical]).is_ok() && ser.output.finalize().is_ok()
        };
        for physical in [
            2usize, 3, 4, 255, 256, 257, 258, 259, 480, 488, 509, 510, 511, 512, 513, 514,
        ] {
            let len = max_logical_len(physical);
            assert!(
                encodes_into(len, physical),
                "max_logical_len({physical}) = {len} must fit"
            );
            assert!(
                !encodes_into(len + 1, physical),
                "{} must not fit in {physical}",
                len + 1
            );
        }
        // Below one code byte + delimiter, nothing fits.
        assert_eq!(max_logical_len(0), 0);
        assert_eq!(max_logical_len(1), 0);
        assert_eq!(
            RYNK_MAX_PAYLOAD_SIZE,
            max_logical_len(crate::constants::RYNK_BUFFER_SIZE) - RYNK_HEADER_SIZE
        );
    }

    #[test]
    fn overlapping_encoder_matches_reference_at_every_length() {
        const CAPACITY: usize = cobs::max_encoding_length(crate::constants::RYNK_BUFFER_SIZE) + 1;
        let mut source = [0u8; crate::constants::RYNK_BUFFER_SIZE];
        let mut overlap = [0u8; CAPACITY];
        let mut expected = [0u8; CAPACITY];

        for len in RYNK_HEADER_SIZE..=source.len() {
            let capacity = cobs::max_encoding_length(len) + 1;
            let start = logical_start(capacity).unwrap();
            for pattern in 0..3 {
                for (i, byte) in source[..len].iter_mut().enumerate() {
                    *byte = match pattern {
                        0 => 1,
                        1 => u8::from(i % 2 == 0),
                        _ => (i as u8).wrapping_mul(73),
                    };
                }

                overlap[..capacity].fill(0);
                overlap[start..start + len].copy_from_slice(&source[..len]);
                let actual_len = encode_shifted(&mut overlap[..capacity], start, len).unwrap();

                let encoded_len = cobs::try_encode(&source[..len], &mut expected[..capacity - 1]).unwrap();
                expected[encoded_len] = 0;
                let expected_len = encoded_len + 1;

                assert_eq!(actual_len, expected_len, "len={len}, pattern={pattern}");
                assert_eq!(
                    &overlap[..actual_len],
                    &expected[..expected_len],
                    "len={len}, pattern={pattern}"
                );
            }
        }
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
}
