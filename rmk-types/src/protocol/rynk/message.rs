//! Rynk wire-format message.
//!
//! A [`RynkMessage`] is one frame: a 3-byte header followed by a
//! postcard-encoded payload. On the wire the whole frame is COBS-encoded and
//! terminated by a single `0x00` delimiter (see [`framing`](super::framing)),
//! which makes the byte stream self-synchronizing.
//!
//! ```text
//! ┌──────────────┬───────┐
//! │  CMD u16 LE  │SEQ u8 │  ← 3-byte header
//! ├──────────────┴───────┤
//! │   payload bytes ...  │
//! └──────────────────────┘
//! ```

use serde::Serialize;
use serde::de::DeserializeOwned;

use super::command::Cmd;
use super::endpoint::Topic;
use super::{RynkError, framing};

/// Size in bytes of the fixed Rynk header: `CMD u16 LE | SEQ u8`.
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
}

/// A view over one frame buffer. On receive it wraps a
/// [`Deframer`](super::framing::Deframer)-decoded logical frame
/// (`[cmd, seq, payload]`, read via [`header`](Self::header) /
/// [`payload`](Self::payload)); on send it holds the COBS-encoded frame built by
/// [`build`](Self::build) / [`encode_response`](Self::encode_response) (transmit
/// via [`frame`](Self::frame)). The header is cached because COBS stuffs the
/// header bytes, so a built frame can't be parsed back in place.
pub struct RynkMessage<'a> {
    buf: &'a mut [u8],
    header: RynkHeader,
    /// Valid prefix length of `buf`: logical (decoded) or framed (built).
    len: usize,
}

impl<'a> RynkMessage<'a> {
    /// Wrap a decoded logical frame whose `[cmd, seq, payload]` occupies
    /// `buf[..len]`. The rest of `buf` is free scratch that
    /// [`encode_response`](Self::encode_response) may grow the reply into.
    /// The caller guarantees `len >= RYNK_HEADER_SIZE`.
    pub fn from_decoded(buf: &'a mut [u8], len: usize) -> Self {
        let header = RynkHeader::parse(buf.first_chunk().unwrap());
        Self { buf, header, len }
    }

    /// Build an outbound frame: COBS-encode `[cmd, seq] ++ postcard(value)`.
    pub fn build<T: Serialize>(buf: &'a mut [u8], header: RynkHeader, value: &T) -> Result<Self, RynkError> {
        let len = framing::encode_frame(buf, header, value)?;
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
        self.len = framing::encode_frame(self.buf, self.header, &Ok::<&T, RynkError>(value))?;
        Ok(())
    }

    /// Encode `Err(err)` as the response frame.
    pub fn encode_error(&mut self, err: RynkError) {
        self.len = framing::encode_frame(self.buf, self.header, &Err::<(), RynkError>(err)).unwrap_or(0);
    }

    /// Encode a bulk `Ok(sequence)` response frame without allocating a `Vec`:
    /// stream the postcard `Ok` tag, element count, and each item straight
    /// through the COBS encoder. The bytes match postcard's `Ok` tag, sequence
    /// length, and element encoding.
    pub fn encode_bulk_ok<T, I>(&mut self, count: usize, items: I) -> Result<(), RynkError>
    where
        T: Serialize,
        I: IntoIterator<Item = T>,
    {
        use postcard::ser_flavors::{Cobs, Flavor, Slice};

        let [cmd_lo, cmd_hi] = self.header.cmd.to_le_bytes();
        let mut ser = postcard::Serializer {
            output: Cobs::try_new(Slice::new(self.buf)).map_err(|_| RynkError::Internal)?,
        };
        ser.output
            .try_extend(&[cmd_lo, cmd_hi, self.header.seq])
            .map_err(|_| RynkError::Internal)?;
        // postcard encodes `Result::Ok` with tag 0.
        ser.output.try_push(0).map_err(|_| RynkError::Internal)?;
        count.serialize(&mut ser).map_err(|_| RynkError::Internal)?;
        for item in items {
            item.serialize(&mut ser).map_err(|_| RynkError::Internal)?;
        }
        self.len = ser.output.finalize().map_err(|_| RynkError::Internal)?.len();
        Ok(())
    }
}

impl<'a> TryFrom<&'a mut [u8]> for RynkMessage<'a> {
    type Error = RynkError;

    /// Wrap already-decoded logical frame bytes (the host channel path, where a
    /// whole frame has been copied out of the receive stream).
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
}
