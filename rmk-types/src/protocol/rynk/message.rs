//! Rynk wire-format message.
//!
//! A [`RynkMessage`] is one frame: the 3-byte header plus a postcard-encoded
//! payload, COBS-encoded and `0x00`-terminated on the wire (layout in the
//! [module docs](super)). [`RynkMessage::build`] encodes straight into the
//! caller's buffer; a [`Deframer`](super::Deframer) cuts frames back out of
//! the received stream.

use postcard::ser_flavors::{Cobs, Flavor, Slice};
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::RynkError;
use super::command::Cmd;
use super::endpoint::Topic;

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

/// Physical buffer size for the COBS-encoded form of a worst-case firmware
/// frame, trailing delimiter included (`RYNK_BUFFER_SIZE` stays the *logical*
/// budget). One byte over `cobs::max_encoding_length`: the streaming `Cobs`
/// flavor emits a placeholder after each full 254-byte block that one-shot
/// encoding elides.
pub const RYNK_FRAME_BUFFER_SIZE: usize =
    crate::constants::RYNK_BUFFER_SIZE + crate::constants::RYNK_BUFFER_SIZE / 254 + 2;

/// COBS-frame `RynkHeader ++ body` into `buf`, returning the total framed
/// length. The header goes through the COBS encoder too, so a `0x00` in it
/// never aliases the delimiter.
fn encode_frame_with<'a>(
    buf: &'a mut [u8],
    header: RynkHeader,
    body: impl FnOnce(&mut postcard::Serializer<Cobs<Slice<'a>>>) -> postcard::Result<()>,
) -> Result<usize, RynkError> {
    let mut ser = postcard::Serializer {
        output: Cobs::try_new(Slice::new(buf)).map_err(|_| RynkError::Internal)?,
    };
    ser.output
        .try_extend(&header.to_bytes())
        .map_err(|_| RynkError::Internal)?;
    body(&mut ser).map_err(|_| RynkError::Internal)?;
    Ok(ser.output.finalize().map_err(|_| RynkError::Internal)?.len())
}

fn encode_frame<T: Serialize>(buf: &mut [u8], header: RynkHeader, value: &T) -> Result<usize, RynkError> {
    encode_frame_with(buf, header, |ser| value.serialize(ser))
}

/// A view over one frame buffer: a decoded logical frame (`[cmd, seq, payload]`)
/// on receive, a COBS-encoded frame on send. The header is cached because COBS
/// stuffs the header bytes — a built frame can't be parsed back in place.
pub struct RynkMessage<'a> {
    buf: &'a mut [u8],
    header: RynkHeader,
    /// Valid prefix length of `buf`: logical (decoded) or framed (built).
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

    /// Build an outbound frame: COBS-encode `[cmd, seq] ++ postcard(value)`.
    pub fn build<T: Serialize>(buf: &'a mut [u8], header: RynkHeader, value: &T) -> Result<Self, RynkError> {
        let len = encode_frame(buf, header, value)?;
        Ok(Self { buf, header, len })
    }

    /// Build a topic push frame (SEQ = 0).
    pub fn build_topic<T: Topic>(buf: &'a mut [u8], value: &T::Payload) -> Result<Self, RynkError> {
        Self::build(buf, RynkHeader { cmd: T::CMD, seq: 0 }, value)
    }

    /// The decoded header.
    pub const fn header(&self) -> RynkHeader {
        self.header
    }

    /// The COBS-encoded frame, ready to transmit. Valid after `build`/`encode_*`.
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
        self.len = encode_frame(self.buf, self.header, &Ok::<&T, RynkError>(value))?;
        Ok(())
    }

    /// Encode `Err(err)` as the response frame.
    pub fn encode_error(&mut self, err: RynkError) {
        self.len = encode_frame(self.buf, self.header, &Err::<(), RynkError>(err)).unwrap_or(0);
    }

    /// Encode a bulk `Ok(sequence)` response frame, streaming the postcard `Ok`
    /// tag, element count, and items through the COBS encoder — no `Vec` needed.
    pub fn encode_bulk<T, I>(&mut self, count: usize, items: I) -> Result<(), RynkError>
    where
        T: Serialize,
        I: IntoIterator<Item = T>,
    {
        self.len = encode_frame_with(self.buf, self.header, |ser| {
            // `Ok(())` emits just the `Ok` tag; the count + items streamed after it
            // sit where `Ok`'s payload would, so the frame decodes as `Ok(sequence)`.
            Ok::<(), RynkError>(()).serialize(&mut *ser)?;
            count.serialize(&mut *ser)?;
            for item in items {
                item.serialize(&mut *ser)?;
            }
            Ok(())
        })?;
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
    use super::*;

    #[test]
    fn build_frame_round_trips_through_cobs() {
        // GetVersion = 0x0001, so cmd_hi is 0x00 — a good test that COBS carries
        // an interior zero without it aliasing the delimiter.
        let mut buf = [0u8; 64];
        let framed_len;
        {
            let msg = RynkMessage::build(
                &mut buf,
                RynkHeader {
                    cmd: Cmd::GetVersion,
                    seq: 0x42,
                },
                &[1u8, 2, 3, 4],
            )
            .unwrap();
            assert_eq!(msg.header().cmd, Cmd::GetVersion);
            assert_eq!(msg.header().seq, 0x42);
            framed_len = msg.frame().len();
        }
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
    fn build_rejects_short_buffer() {
        // Too small to hold the COBS-framed 3-byte header.
        let mut buf = [0u8; 2];
        assert_eq!(
            RynkMessage::build(
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
    fn frame_buffer_sizing_fits_worst_case_cobs() {
        // COBS worst case is an all-nonzero frame; a buffer of exactly
        // n + n / 254 + 2 must hold it, including at multiples of 254.
        let capacity = |n: usize| n + n / 254 + 2;
        assert_eq!(RYNK_FRAME_BUFFER_SIZE, capacity(crate::constants::RYNK_BUFFER_SIZE));
        let nonzero = [0x41u8; 512];
        for n in [1usize, 253, 254, 255, 508, 509, 512] {
            let mut store = [0u8; 1024];
            let buf = &mut store[..capacity(n)];
            let mut ser = postcard::Serializer {
                output: Cobs::try_new(Slice::new(buf)).unwrap(),
            };
            ser.output.try_extend(&nonzero[..n]).unwrap();
            let framed = ser
                .output
                .finalize()
                .unwrap_or_else(|_| panic!("capacity({n}) too small for the worst-case COBS frame"));
            assert!(framed.len() <= capacity(n), "n={n}: {} > {}", framed.len(), capacity(n));
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
