//! Shared frame encode/decode helpers.
//!
//! Both transports do the same thing on the wire — they only differ on
//! how packets are delivered (USB BULK chunks vs. BLE GATT writes /
//! notifications). Pull the common code here so the transport modules
//! focus on their I/O loops.

use rmk_types::protocol::rynk::Cmd;
use rmk_types::protocol::rynk::RYNK_HEADER_SIZE;
use serde::Serialize;

use crate::transport::TransportError;

/// Largest frame the host accepts in a single response. Mirrors firmware's
/// `RYNK_BUFFER_SIZE` ceiling; oversize replies indicate either a firmware
/// bug or a corrupt stream. 64 KiB is comfortably above any realistic
/// payload (full keymap dumps, etc.).
pub const MAX_FRAME_SIZE: usize = 64 * 1024;

/// Encode a request payload into a complete frame (5-byte header + payload).
pub fn encode_frame<T: Serialize>(cmd: Cmd, seq: u8, value: &T) -> Result<Vec<u8>, TransportError> {
    // Worst-case host payload size — see `MAX_FRAME_SIZE` above. Allocate
    // up-front so postcard's slice serializer doesn't have to grow.
    let mut buf = vec![0u8; MAX_FRAME_SIZE];
    let payload = postcard::to_slice(value, &mut buf[RYNK_HEADER_SIZE..]).map_err(TransportError::Serialize)?;
    let len = payload.len();
    let total = RYNK_HEADER_SIZE + len;

    let cmd_raw = cmd as u16;
    buf[0] = cmd_raw as u8;
    buf[1] = (cmd_raw >> 8) as u8;
    buf[2] = seq;
    buf[3] = len as u8;
    buf[4] = (len >> 8) as u8;

    buf.truncate(total);
    Ok(buf)
}

/// Parse the 5-byte header at the front of `bytes`. Returns
/// `(cmd_raw, seq, payload_len)`. Caller verifies `bytes.len() >= 5 +
/// payload_len`.
pub fn parse_header(bytes: &[u8]) -> Result<(u16, u8, usize), TransportError> {
    if bytes.len() < RYNK_HEADER_SIZE {
        return Err(TransportError::FrameTooLong {
            len: bytes.len(),
            max: RYNK_HEADER_SIZE,
        });
    }
    let cmd = u16::from_le_bytes([bytes[0], bytes[1]]);
    let seq = bytes[2];
    let len = u16::from_le_bytes([bytes[3], bytes[4]]) as usize;
    Ok((cmd, seq, len))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_unit_payload_emits_header_only() {
        let bytes = encode_frame(Cmd::GetVersion, 0x42, &()).unwrap();
        assert_eq!(bytes.len(), RYNK_HEADER_SIZE);
        // CMD 0x0001 little-endian.
        assert_eq!(bytes[0], 0x01);
        assert_eq!(bytes[1], 0x00);
        // SEQ
        assert_eq!(bytes[2], 0x42);
        // LEN = 0
        assert_eq!(bytes[3], 0x00);
        assert_eq!(bytes[4], 0x00);
    }

    #[test]
    fn parse_header_extracts_fields() {
        let frame = [0x05, 0x01, 0x07, 0x03, 0x00, 0xAA, 0xBB, 0xCC];
        let (cmd, seq, len) = parse_header(&frame).unwrap();
        // 0x0105 = GetEncoderAction, but we check the raw u16 here.
        assert_eq!(cmd, 0x0105);
        assert_eq!(seq, 0x07);
        assert_eq!(len, 3);
    }

    #[test]
    fn parse_header_rejects_short_input() {
        assert!(parse_header(&[]).is_err());
        assert!(parse_header(&[0, 0, 0, 0]).is_err());
    }

    #[test]
    fn encode_round_trips_through_parse_header() {
        let bytes = encode_frame(Cmd::GetKeyAction, 0x10, &(0u8, 1u8, 2u8)).unwrap();
        let (cmd, seq, len) = parse_header(&bytes).unwrap();
        assert_eq!(cmd, Cmd::GetKeyAction as u16);
        assert_eq!(seq, 0x10);
        assert_eq!(len, bytes.len() - RYNK_HEADER_SIZE);
    }
}
