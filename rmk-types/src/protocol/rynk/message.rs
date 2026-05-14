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

use super::RynkError;
use super::cmd::Cmd;

/// Size in bytes of the fixed Rynk header.
pub const RYNK_HEADER_SIZE: usize = 5;

/// Message operations for Rynk
pub trait RynkMessage {
    fn cmd(&self) -> Result<Cmd, RynkError>;
    fn seq(&self) -> Result<u8, RynkError>;
    fn payload_len(&self) -> Result<u16, RynkError>;
    fn payload(&self) -> Result<&[u8], RynkError>;
    fn payload_mut(&mut self) -> Result<&mut [u8], RynkError>;

    fn set_cmd(&mut self, cmd: Cmd) -> Result<(), RynkError>;
    fn set_seq(&mut self, seq: u8) -> Result<(), RynkError>;
    fn set_payload_len(&mut self, len: u16) -> Result<(), RynkError>;
}

impl RynkMessage for [u8] {
    fn cmd(&self) -> Result<Cmd, RynkError> {
        if self.len() < RYNK_HEADER_SIZE {
            return Err(RynkError::InvalidRequest);
        }
        Cmd::from_repr(u16::from_le_bytes([self[0], self[1]])).ok_or(RynkError::InvalidRequest)
    }

    fn seq(&self) -> Result<u8, RynkError> {
        if self.len() < RYNK_HEADER_SIZE {
            return Err(RynkError::InvalidRequest);
        }
        Ok(self[2])
    }

    fn payload_len(&self) -> Result<u16, RynkError> {
        if self.len() < RYNK_HEADER_SIZE {
            return Err(RynkError::InvalidRequest);
        }
        Ok(u16::from_le_bytes([self[3], self[4]]))
    }

    fn payload(&self) -> Result<&[u8], RynkError> {
        if self.len() < RYNK_HEADER_SIZE {
            return Err(RynkError::InvalidRequest);
        }
        Ok(&self[RYNK_HEADER_SIZE..])
    }

    fn payload_mut(&mut self) -> Result<&mut [u8], RynkError> {
        if self.len() < RYNK_HEADER_SIZE {
            return Err(RynkError::InvalidRequest);
        }
        Ok(&mut self[RYNK_HEADER_SIZE..])
    }

    fn set_cmd(&mut self, cmd: Cmd) -> Result<(), RynkError> {
        if self.len() < RYNK_HEADER_SIZE {
            return Err(RynkError::InvalidRequest);
        }
        self[0..2].copy_from_slice(&(cmd as u16).to_le_bytes());
        Ok(())
    }

    fn set_seq(&mut self, seq: u8) -> Result<(), RynkError> {
        if self.len() < RYNK_HEADER_SIZE {
            return Err(RynkError::InvalidRequest);
        }
        self[2] = seq;
        Ok(())
    }

    fn set_payload_len(&mut self, len: u16) -> Result<(), RynkError> {
        if self.len() < RYNK_HEADER_SIZE {
            return Err(RynkError::InvalidRequest);
        }
        self[3..5].copy_from_slice(&len.to_le_bytes());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_header_fields() {
        let mut buf = [0u8; RYNK_HEADER_SIZE + 7];
        buf.set_cmd(Cmd::GetVersion).unwrap();
        buf.set_seq(0x42).unwrap();
        buf.set_payload_len(7).unwrap();
        buf.payload_mut().unwrap()[..7].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7]);

        assert_eq!(buf.cmd().unwrap(), Cmd::GetVersion);
        assert_eq!(buf.seq().unwrap(), 0x42);
        assert_eq!(buf.payload_len().unwrap(), 7);
        assert_eq!(buf.payload().unwrap(), &[1, 2, 3, 4, 5, 6, 7]);
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

    #[test]
    fn every_accessor_rejects_short_buffer() {
        let mut buf = [0u8; RYNK_HEADER_SIZE - 1];
        assert_eq!(buf.seq(), Err(RynkError::InvalidRequest));
        assert_eq!(buf.payload_len(), Err(RynkError::InvalidRequest));
        assert!(buf.payload().is_err());
        assert!(buf.payload_mut().is_err());
        assert_eq!(buf.set_cmd(Cmd::GetVersion), Err(RynkError::InvalidRequest));
        assert_eq!(buf.set_seq(0), Err(RynkError::InvalidRequest));
        assert_eq!(buf.set_payload_len(0), Err(RynkError::InvalidRequest));
    }
}
