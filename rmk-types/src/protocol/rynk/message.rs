//! Rynk wire-format message.
//!
//! A [`RynkMessage`] contains a 5-byte header and a
//! postcard-encoded payload:
//!
//! ```text
//! ┌──────────────┬───────┬──────────────┐
//! │  CMD u16 LE  │SEQ u8 │  LEN u16 LE  │  ← 5-byte header
//! ├──────────────┴───────┴──────────────┤
//! │          payload bytes ...          │
//! └─────────────────────────────────────┘
//! ```
//!

use serde::Serialize;
use serde::de::DeserializeOwned;

use super::RynkError;
use super::command::{Cmd, RYNK_MAX_PAYLOAD};
use super::endpoint::Topic;

/// Size in bytes of the fixed Rynk header.
pub const RYNK_HEADER_SIZE: usize = 5;

/// Minimum buffer size required to hold any single Rynk message (header + max-payload).
pub const RYNK_MIN_BUFFER_SIZE: usize = RYNK_HEADER_SIZE + RYNK_MAX_PAYLOAD;

/// The fixed header of a [`RynkMessage`].
#[derive(Debug, Clone, Copy)]
pub struct RynkHeader {
    pub cmd: Cmd,
    pub seq: u8,
    pub payload_len: u16,
}

impl RynkHeader {
    /// Decode the 5 header bytes.
    pub const fn parse(bytes: &[u8; RYNK_HEADER_SIZE]) -> Self {
        Self {
            cmd: Cmd::from_le_bytes([bytes[0], bytes[1]]),
            seq: bytes[2],
            payload_len: u16::from_le_bytes([bytes[3], bytes[4]]),
        }
    }

    /// Total frame length in bytes — header + declared payload.
    pub fn frame_len(&self) -> usize {
        RYNK_HEADER_SIZE + self.payload_len as usize
    }
}

/// A RynkMessage is a mutable view over the byte buffer, split at
/// construction into the fixed header bytes and the body after them.
pub struct RynkMessage<'a> {
    header: &'a mut [u8; RYNK_HEADER_SIZE],
    body: &'a mut [u8],
}

impl<'a> RynkMessage<'a> {
    /// Build an outbound message: postcard-encode `value` into the payload,
    /// then write `cmd`, `seq`, and `payload_len` into the header.
    pub fn build<T: Serialize>(buf: &'a mut [u8], cmd: Cmd, seq: u8, value: &T) -> Result<Self, RynkError> {
        let Some((header, body)) = buf.split_first_chunk_mut() else {
            // Outbound encode: a buffer too small for the header is a
            // firmware-side fault, not a malformed host request.
            return Err(RynkError::Internal);
        };
        let n = postcard::to_slice(value, body)
            .map(|s| s.len())
            .map_err(|_| RynkError::Internal)?;
        let mut msg = Self { header, body };
        msg.header[0..2].copy_from_slice(&cmd.to_le_bytes());
        msg.header[2] = seq;
        msg.set_payload_len(n as u16);
        Ok(msg)
    }

    /// Build a topic push frame.
    pub fn build_topic<T: Topic>(buf: &'a mut [u8], value: &T::Payload) -> Result<Self, RynkError> {
        Self::build(buf, T::CMD, 0, value)
    }

    /// The decoded header.
    pub const fn header(&self) -> RynkHeader {
        RynkHeader::parse(self.header)
    }

    /// Total frame length in bytes — header + payload.
    pub fn frame_len(&self) -> usize {
        self.header().frame_len()
    }

    fn payload(&self) -> &[u8] {
        &self.body[..self.header().payload_len as usize]
    }

    fn set_payload_len(&mut self, len: u16) {
        self.header[3..5].copy_from_slice(&len.to_le_bytes());
    }

    /// Decode the request payload.
    /// A short frame is rejected as `Malformed` instead of reading response scratch.
    pub fn decode_request<T: DeserializeOwned>(&self) -> Result<T, RynkError> {
        postcard::from_bytes(self.payload()).map_err(|_| RynkError::Malformed)
    }

    /// Encode `Ok(value)` into the payload and update `LEN`.
    pub fn encode_response<T: Serialize>(&mut self, value: &T) -> Result<(), RynkError> {
        let n = postcard::to_slice(&Ok::<&T, RynkError>(value), self.body)
            .map(|s| s.len())
            .map_err(|_| RynkError::Internal)?;
        self.set_payload_len(n as u16);
        Ok(())
    }

    /// Encode `Err(err)` into the payload and update `LEN`.
    pub fn encode_error(&mut self, err: RynkError) {
        let n = postcard::to_slice(&Err::<(), RynkError>(err), self.body)
            .map(|s| s.len())
            .unwrap_or(0);
        self.set_payload_len(n as u16);
    }
}

impl<'a> TryFrom<&'a mut [u8]> for RynkMessage<'a> {
    type Error = RynkError;

    /// Build [`RynkMessage`] from buffer.
    fn try_from(buf: &'a mut [u8]) -> Result<Self, RynkError> {
        let Some((header, body)) = buf.split_first_chunk_mut() else {
            return Err(RynkError::Malformed);
        };
        if body.len() < RynkHeader::parse(header).payload_len as usize {
            return Err(RynkError::Malformed);
        }
        Ok(Self { header, body })
    }
}

#[cfg(test)]
mod tests {
    use postcard::experimental::max_size::MaxSize;

    use super::super::DeviceCapabilities;
    use super::*;

    #[test]
    fn rynk_min_buffer_size_covers_largest_known_response() {
        // `DeviceCapabilities` is one of the largest single-message responses
        // when bulk is disabled; the min buffer must hold its wrapped form
        // plus header.
        let wrapped = <Result<DeviceCapabilities, RynkError> as MaxSize>::POSTCARD_MAX_SIZE;
        assert!(RYNK_MAX_PAYLOAD >= wrapped);
        assert!(RYNK_MIN_BUFFER_SIZE >= wrapped + RYNK_HEADER_SIZE);
    }

    #[test]
    fn build_round_trip() {
        let mut buf = [0u8; 32];
        let msg = RynkMessage::build(&mut buf, Cmd::GetVersion, 0x42, &[1u8, 2, 3, 4]).unwrap();
        assert_eq!(msg.header().cmd, Cmd::GetVersion);
        assert_eq!(msg.header().seq, 0x42);
        // postcard encodes [u8; 4] as 4 bare bytes
        assert_eq!(msg.header().payload_len, 4);
        assert_eq!(&msg.payload()[..4], &[1, 2, 3, 4]);
        assert_eq!(msg.frame_len(), RYNK_HEADER_SIZE + 4);
    }

    #[test]
    fn build_rejects_short_buffer() {
        let mut buf = [0u8; RYNK_HEADER_SIZE - 1];
        assert_eq!(
            RynkMessage::build(&mut buf, Cmd::GetVersion, 0, &()).err(),
            Some(RynkError::Internal),
        );
    }

    #[test]
    fn try_from_rejects_short_buffer() {
        let mut buf = [0u8; RYNK_HEADER_SIZE - 1];
        assert_eq!(RynkMessage::try_from(&mut buf[..]).err(), Some(RynkError::Malformed),);
    }

    #[test]
    fn try_from_accepts_unknown_discriminant() {
        let mut buf = [0u8; RYNK_HEADER_SIZE];
        buf[0..2].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let msg = RynkMessage::try_from(&mut buf[..]).unwrap();
        assert_eq!(msg.header().cmd, Cmd::from_raw(0xFFFF));
    }

    #[test]
    fn try_from_accepts_valid_header() {
        let mut buf = [0u8; RYNK_HEADER_SIZE];
        buf[0..2].copy_from_slice(&Cmd::GetVersion.to_le_bytes());
        let msg = RynkMessage::try_from(&mut buf[..]).unwrap();
        assert_eq!(msg.header().cmd, Cmd::GetVersion);
    }

    #[test]
    fn try_from_rejects_buffer_shorter_than_payload_len() {
        // Header says payload_len = 10, but the buffer only has 4 payload bytes.
        let mut buf = [0u8; RYNK_HEADER_SIZE + 4];
        buf[0..2].copy_from_slice(&Cmd::GetVersion.to_le_bytes());
        buf[3..5].copy_from_slice(&10u16.to_le_bytes());
        assert_eq!(RynkMessage::try_from(&mut buf[..]).err(), Some(RynkError::Malformed),);
    }

    #[test]
    fn inbound_payload_views_are_bounded_by_declared_len() {
        let mut buf = [0xCCu8; 32];
        buf[0..2].copy_from_slice(&Cmd::SetDefaultLayer.to_le_bytes());
        buf[2] = 0x34;
        buf[3..5].copy_from_slice(&0u16.to_le_bytes());

        let msg = RynkMessage::try_from(&mut buf[..]).unwrap();
        // `&[] as &[u8]`: serde_json (a dev-dep) adds `impl PartialEq<Value> for
        // u8`, so an untyped empty slice would be ambiguous here.
        assert_eq!(msg.payload(), &[] as &[u8]);
    }
}
