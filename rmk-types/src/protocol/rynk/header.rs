//! Rynk wire-format header.
//!
//! Fixed 5-byte layout, shared between firmware and the `rynk-host`
//! library.
//!
//! ```text
//! ┌──────────────┬───────────┬────────────────────┐
//! │ CMD u16 LE   │ SEQ u8    │ LEN u16 LE         │
//! └──────────────┴───────────┴────────────────────┘
//! ```

use super::cmd::Cmd;

/// Size in bytes of the fixed Rynk header.
pub const HEADER_SIZE: usize = 5;

/// Parsed Rynk header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    pub cmd: Cmd,
    pub seq: u8,
    pub len: u16,
}

/// Reasons a header (or following payload) could not be parsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    /// Buffer is shorter than the 5-byte header or shorter than `HEADER_SIZE + len`.
    Short,
    /// CMD discriminant doesn't correspond to any compiled-in variant
    /// (host built against a newer ICD, or wire corruption).
    UnknownCmd(u16),
}

impl Header {
    /// Encode `self` into the first 5 bytes of `out`. The caller is
    /// responsible for ensuring `out.len() >= HEADER_SIZE`.
    pub fn encode_into(&self, out: &mut [u8]) {
        debug_assert!(out.len() >= HEADER_SIZE);
        let cmd = self.cmd as u16;
        out[0..2].copy_from_slice(&cmd.to_le_bytes());
        out[2] = self.seq;
        out[3..5].copy_from_slice(&self.len.to_le_bytes());
    }

    /// Parse a header from the start of `buf`, returning the header plus
    /// a slice referencing the payload bytes (length = `header.len`).
    ///
    /// Returns `Err(DecodeError::Short)` if `buf` doesn't contain the
    /// full `HEADER_SIZE + len` bytes — caller should accumulate more
    /// bytes and retry.
    pub fn decode(buf: &[u8]) -> Result<(Self, &[u8]), DecodeError> {
        if buf.len() < HEADER_SIZE {
            return Err(DecodeError::Short);
        }
        let cmd_raw = u16::from_le_bytes([buf[0], buf[1]]);
        let cmd = Cmd::from_repr(cmd_raw).ok_or(DecodeError::UnknownCmd(cmd_raw))?;
        let seq = buf[2];
        let len = u16::from_le_bytes([buf[3], buf[4]]) as usize;
        if buf.len() < HEADER_SIZE + len {
            return Err(DecodeError::Short);
        }
        Ok((
            Self {
                cmd,
                seq,
                len: len as u16,
            },
            &buf[HEADER_SIZE..HEADER_SIZE + len],
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_then_decode_round_trips() {
        let h = Header {
            cmd: Cmd::GetVersion,
            seq: 0x42,
            len: 7,
        };
        let mut buf = [0u8; HEADER_SIZE + 7];
        h.encode_into(&mut buf);
        buf[HEADER_SIZE..].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7]);
        let (parsed, payload) = Header::decode(&buf).unwrap();
        assert_eq!(parsed, h);
        assert_eq!(payload, &[1, 2, 3, 4, 5, 6, 7]);
    }

    #[test]
    fn encode_byte_layout() {
        let h = Header {
            cmd: Cmd::SetKeyAction, // 0x0102
            seq: 0xAB,
            len: 0x0304,
        };
        let mut buf = [0u8; HEADER_SIZE];
        h.encode_into(&mut buf);
        assert_eq!(buf, [0x02, 0x01, 0xAB, 0x04, 0x03]);
    }

    #[test]
    fn decode_short_header_returns_short() {
        assert_eq!(Header::decode(&[]), Err(DecodeError::Short));
        assert_eq!(Header::decode(&[0; 4]), Err(DecodeError::Short));
    }

    #[test]
    fn decode_short_payload_returns_short() {
        let mut buf = [0u8; HEADER_SIZE];
        Header {
            cmd: Cmd::GetVersion,
            seq: 0,
            len: 10,
        }
        .encode_into(&mut buf);
        assert_eq!(Header::decode(&buf), Err(DecodeError::Short));
    }

    #[test]
    fn decode_unknown_cmd_returns_unknown() {
        let buf = [0xFF, 0xFF, 0, 0, 0];
        assert_eq!(Header::decode(&buf), Err(DecodeError::UnknownCmd(0xFFFF)));
    }
}
