//! Rynk wire-format message view.
//!
//! Fixed 5-byte header followed by a postcard-encoded payload:
//!
//! ```text
//! ┌──────────────┬───────────┬────────────────────┐
//! │ CMD u16 LE   │ SEQ u8    │ LEN u16 LE         │  ← 5-byte header
//! └──────────────┴───────────┴────────────────────┘
//! │                       payload bytes                       │
//! ```
//!
//! [`RynkMessage`] is implemented on `[u8]`, so any byte slice (or
//! `[u8; N]` via auto-deref) holding a Rynk message gains typed accessors
//! for the header fields and the payload sub-slice. Firmware and host
//! both operate on the bytes in place rather than through a parsed
//! `struct`.

use super::RynkError;
use super::cmd::Cmd;

/// Size in bytes of the fixed Rynk header.
pub const RYNK_HEADER_SIZE: usize = 5;

/// Header-field accessors over an in-place buffer.
///
/// `cmd()` validates the CMD discriminant against the compiled-in [`Cmd`]
/// enum and checks the buffer length, so it's the safe first call on a
/// fresh buffer. The other accessors assume `msg.len() >= RYNK_HEADER_SIZE`
/// — verify that yourself (e.g. via `cmd()`) before calling them.
pub trait RynkMessage {
    fn cmd(&self) -> Result<Cmd, RynkError>;
    fn seq(&self) -> u8;
    fn payload_len(&self) -> u16;
    fn payload(&self) -> &[u8];
    fn payload_mut(&mut self) -> &mut [u8];

    fn set_cmd(&mut self, cmd: Cmd);
    fn set_seq(&mut self, seq: u8);
    fn set_payload_len(&mut self, len: u16);
}

impl RynkMessage for [u8] {
    fn cmd(&self) -> Result<Cmd, RynkError> {
        if self.len() < RYNK_HEADER_SIZE {
            return Err(RynkError::InvalidRequest);
        }
        Cmd::from_repr(u16::from_le_bytes([self[0], self[1]])).ok_or(RynkError::InvalidRequest)
    }

    fn seq(&self) -> u8 {
        self[2]
    }

    fn payload_len(&self) -> u16 {
        u16::from_le_bytes([self[3], self[4]])
    }

    fn payload(&self) -> &[u8] {
        &self[RYNK_HEADER_SIZE..]
    }

    fn payload_mut(&mut self) -> &mut [u8] {
        &mut self[RYNK_HEADER_SIZE..]
    }

    fn set_cmd(&mut self, cmd: Cmd) {
        self[0..2].copy_from_slice(&(cmd as u16).to_le_bytes());
    }

    fn set_seq(&mut self, seq: u8) {
        self[2] = seq;
    }

    fn set_payload_len(&mut self, len: u16) {
        self[3..5].copy_from_slice(&len.to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_header_fields() {
        let mut buf = [0u8; RYNK_HEADER_SIZE + 7];
        buf.set_cmd(Cmd::GetVersion);
        buf.set_seq(0x42);
        buf.set_payload_len(7);
        buf.payload_mut()[..7].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7]);

        assert_eq!(buf.cmd().unwrap(), Cmd::GetVersion);
        assert_eq!(buf.seq(), 0x42);
        assert_eq!(buf.payload_len(), 7);
        assert_eq!(buf.payload(), &[1, 2, 3, 4, 5, 6, 7]);
    }

    #[test]
    fn cmd_rejects_short_buffer() {
        let buf = [0u8; RYNK_HEADER_SIZE - 1];
        assert_eq!(buf.cmd(), Err(RynkError::InvalidRequest));
    }

    #[test]
    fn cmd_rejects_unknown_discriminant() {
        let mut buf = [0u8; RYNK_HEADER_SIZE];
        buf[0..2].copy_from_slice(&0xFFFFu16.to_le_bytes());
        assert_eq!(buf.cmd(), Err(RynkError::InvalidRequest));
    }
}
