//! Rynk wire-format message view.
//!
//! Fixed 5-byte header followed by a postcard-encoded payload:
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
use super::cmd::Cmd;

/// Size in bytes of the fixed Rynk header.
pub const RYNK_HEADER_SIZE: usize = 5;

/// A RynkMessage is a mutable view over the byte buffer.
pub struct RynkMessage<'a> {
    buf: &'a mut [u8],
}

impl<'a> RynkMessage<'a> {
    /// Build an outbound message: postcard-encode `value` into the payload,
    /// then write `cmd`, `seq`, and `payload_len` into the header.
    pub fn build<T: Serialize>(buf: &'a mut [u8], cmd: Cmd, seq: u8, value: &T) -> Result<Self, RynkError> {
        if buf.len() < RYNK_HEADER_SIZE {
            // Outbound encode: a buffer too small for the header is a
            // firmware-side fault, not a malformed host request.
            return Err(RynkError::Internal);
        }
        buf[0..2].copy_from_slice(&(cmd as u16).to_le_bytes());
        buf[2] = seq;
        let n = postcard::to_slice(value, &mut buf[RYNK_HEADER_SIZE..])
            .map(|s| s.len())
            .map_err(|_| RynkError::Internal)?;
        let mut msg = Self { buf };
        msg.set_payload_len(n as u16);
        Ok(msg)
    }

    // Get the cmd from buffer, this should be ALWAYS successful after RynkMessage is constructed
    pub fn cmd(&self) -> Cmd {
        Cmd::from_repr(u16::from_le_bytes([self.buf[0], self.buf[1]]))
            .expect("RynkMessage invariant: cmd valid by construction")
    }

    pub fn seq(&self) -> u8 {
        self.buf[2]
    }

    pub fn payload_len(&self) -> u16 {
        u16::from_le_bytes([self.buf[3], self.buf[4]])
    }

    /// Total frame length in bytes — header + payload.
    pub fn frame_len(&self) -> usize {
        RYNK_HEADER_SIZE + self.payload_len() as usize
    }

    pub fn payload(&self) -> &[u8] {
        &self.buf[RYNK_HEADER_SIZE..self.frame_len()]
    }

    /// Decode the request payload, bounded by the declared `LEN`. A truncated
    /// frame (`LEN` short of what `T` needs) is rejected as `Malformed` rather
    /// than reading into response scratch past the frame.
    pub fn request<T: DeserializeOwned>(&self) -> Result<T, RynkError> {
        let (value, _) = postcard::take_from_bytes::<T>(self.payload()).map_err(|_| RynkError::Malformed)?;
        Ok(value)
    }

    /// Payload-sized response encoding buffer.
    ///
    /// Inbound request payload accessors are bounded by the declared `LEN`;
    /// response encoding still needs the full backing buffer after the header
    /// because replies can be larger than requests.
    pub fn response_payload_mut(&mut self) -> &mut [u8] {
        &mut self.buf[RYNK_HEADER_SIZE..]
    }

    pub fn set_payload_len(&mut self, len: u16) {
        self.buf[3..5].copy_from_slice(&len.to_le_bytes());
    }

    /// Encode `Err(err)` into the payload and update `LEN`. Header `cmd`
    /// and `seq` bytes are left untouched so the host can correlate the
    /// error reply with the outgoing request.
    pub fn write_error(&mut self, err: RynkError) {
        let n = postcard::to_slice(&Err::<(), RynkError>(err), self.response_payload_mut())
            .map(|s| s.len())
            .unwrap_or(0);
        self.set_payload_len(n as u16);
    }

    /// Encode an `Err(err)` reply over a raw `buf`.
    /// Preserves `cmd` and `seq` from the existing header bytes.
    /// Returns the total frame length.
    pub fn encode_error_reply(buf: &mut [u8], err: RynkError) -> usize {
        debug_assert!(buf.len() >= RYNK_HEADER_SIZE);
        let mut msg = RynkMessage { buf };
        msg.write_error(err);
        msg.frame_len()
    }
}

impl<'a> TryFrom<&'a mut [u8]> for RynkMessage<'a> {
    type Error = RynkError;

    /// Validate an inbound frame: the buffer covers the header, `cmd` is a
    /// known discriminant, and the buffer is long enough to hold the
    /// declared payload (`buf.len() >= RYNK_HEADER_SIZE + payload_len`).
    ///
    /// The error distinguishes what the receiver knows: structural failures
    /// (short buffer) are [`RynkError::Malformed`]; a structurally sound
    /// frame whose CMD tag this build doesn't recognize is
    /// [`RynkError::UnknownCmd`] — a feature-gated-out or newer peer's
    /// command looks exactly like garbage here, so the name claims no more.
    fn try_from(buf: &'a mut [u8]) -> Result<Self, RynkError> {
        if buf.len() < RYNK_HEADER_SIZE {
            return Err(RynkError::Malformed);
        }
        Cmd::from_repr(u16::from_le_bytes([buf[0], buf[1]])).ok_or(RynkError::UnknownCmd)?;
        let payload_len = u16::from_le_bytes([buf[3], buf[4]]) as usize;
        if buf.len() < RYNK_HEADER_SIZE + payload_len {
            return Err(RynkError::Malformed);
        }
        Ok(Self { buf })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_round_trip() {
        let mut buf = [0u8; 32];
        let msg = RynkMessage::build(&mut buf, Cmd::GetVersion, 0x42, &[1u8, 2, 3, 4]).unwrap();
        assert_eq!(msg.cmd(), Cmd::GetVersion);
        assert_eq!(msg.seq(), 0x42);
        // postcard encodes [u8; 4] as 4 bare bytes
        assert_eq!(msg.payload_len(), 4);
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
    fn try_from_rejects_unknown_discriminant() {
        // Unknown CMD on a structurally sound frame is UnknownCmd, not
        // Malformed — sessions echo it so a host probing a feature-gated-out
        // command gets an honest answer instead of "your frame was garbage".
        let mut buf = [0u8; RYNK_HEADER_SIZE];
        buf[0..2].copy_from_slice(&0xFFFFu16.to_le_bytes());
        assert_eq!(RynkMessage::try_from(&mut buf[..]).err(), Some(RynkError::UnknownCmd),);
    }

    #[test]
    fn try_from_accepts_valid_header() {
        let mut buf = [0u8; RYNK_HEADER_SIZE];
        buf[0..2].copy_from_slice(&(Cmd::GetVersion as u16).to_le_bytes());
        let msg = RynkMessage::try_from(&mut buf[..]).unwrap();
        assert_eq!(msg.cmd(), Cmd::GetVersion);
    }

    #[test]
    fn try_from_rejects_buffer_shorter_than_payload_len() {
        // Header says payload_len = 10, but the buffer only has 4 payload bytes.
        let mut buf = [0u8; RYNK_HEADER_SIZE + 4];
        buf[0..2].copy_from_slice(&(Cmd::GetVersion as u16).to_le_bytes());
        buf[3..5].copy_from_slice(&10u16.to_le_bytes());
        assert_eq!(RynkMessage::try_from(&mut buf[..]).err(), Some(RynkError::Malformed),);
    }

    #[test]
    fn dispatch_style_set_payload_len_after_parse() {
        // Simulates the response path: parse an inbound frame, then update
        // payload_len in place after the handler writes its response.
        let mut buf = [0u8; 32];
        buf[0..2].copy_from_slice(&(Cmd::GetVersion as u16).to_le_bytes());
        let mut msg = RynkMessage::try_from(&mut buf[..]).unwrap();
        msg.response_payload_mut()[..2].copy_from_slice(&[0xAA, 0xBB]);
        msg.set_payload_len(2);
        assert_eq!(msg.payload_len(), 2);
        assert_eq!(msg.frame_len(), RYNK_HEADER_SIZE + 2);
    }

    #[test]
    fn inbound_payload_views_are_bounded_by_declared_len() {
        let mut buf = [0xCCu8; 32];
        buf[0..2].copy_from_slice(&(Cmd::SetDefaultLayer as u16).to_le_bytes());
        buf[2] = 0x34;
        buf[3..5].copy_from_slice(&0u16.to_le_bytes());

        let msg = RynkMessage::try_from(&mut buf[..]).unwrap();
        assert_eq!(msg.payload(), &[]);
    }
}
