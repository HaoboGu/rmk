//! Rynk protocol ICD — RMK's native host-communication protocol.
//!
//! Carries RMK's canonical types (`KeyAction`, `Combo`, `Morse`, `Fork`,
//! `EncoderAction`, `BatteryStatus`, `BleStatus`) on the wire as a 5-byte
//! fixed header + postcard-encoded payload.
//!
//! ## Wire format
//!
//! ```text
//! ┌──────────────┬───────────┬────────────────────┐
//! │ CMD u16 LE   │ SEQ u8    │ LEN u16 LE         │  ← 5-byte header
//! ├──────────────┴───────────┴────────────────────┤
//! │              postcard-encoded payload         │  ← LEN bytes
//! └───────────────────────────────────────────────┘
//! ```
//!
//! - **CMD** — `0x0000..=0x7FFF` request/response, `0x8000..=0xFFFF` topic
//!   (server→host push).
//! - **SEQ** — the sequence number of current request. Topics send SEQ = 0.
//! - **LEN** — payload byte count.
//!
//! Responses wrap the payload in postcard `Result<T, RynkError>` (`T = ()` for
//! `Set*`); requests are the bare postcard struct, unwrapped.
//!
//! ## Module layout
//!
//! - [`command`] — the [`Cmd`] ids and the table binding each command/topic to
//!   its payload types; firmware and host both compile against it, so the two
//!   ends can't disagree about a message's types.
//! - [`endpoint`] — the [`Endpoint`](endpoint::Endpoint) / [`Topic`](endpoint::Topic)
//!   traits those table entries implement.
//! - [`message`] — the header, the [`RynkMessage`] buffer view, and the envelope.
//! - `error` / `payload` (private) — [`RynkError`] and the per-domain payload
//!   types, re-exported flat at `protocol::rynk::*`.
//!
//! ## Compatibility
//!
//! `Cmd::GetVersion = 0x0001` and its `Result<ProtocolVersion, RynkError>` reply
//! are frozen across all versions.
//! Within a `major`, changes must keep old hosts working, so `minor`
//! is informational: a new `Cmd` or a new/extended topic is a `minor` bump (old
//! peers answer `UnknownCmd` or ignore trailing topic bytes), while reshaping an
//! existing request/response — *including appending a field* — is a `major` bump,
//! since hosts reject trailing response bytes. The `snapshots/*.snap` golden
//! files (`tests.rs`) fail on any accidental drift.

pub mod command;
pub mod endpoint;
pub mod message;

mod error;
mod payload;

#[cfg(test)]
pub(crate) mod tests;

pub use self::command::{Cmd, RYNK_MAX_PAYLOAD, TopicEvent};
pub use self::error::RynkError;
pub use self::message::{RYNK_HEADER_SIZE, RYNK_MIN_BUFFER_SIZE, RynkHeader, RynkMessage};
pub use self::payload::*;

/// Largest single GATT write/notification on the Rynk BLE characteristics.
pub const RYNK_BLE_CHUNK_SIZE: usize = 244;

/// Rynk GATT service UUID
pub const RYNK_SERVICE_UUID: u128 = 0x10900067_537f_4f0a_9b55_929e271f61ab;
/// Rynk `input_data` characteristic UUID.
pub const RYNK_INPUT_CHAR_UUID: u128 = 0x80f9319b_0c74_43a5_9738_c59d6dda3db9;
/// Rynk `output_data` characteristic UUID.
pub const RYNK_OUTPUT_CHAR_UUID: u128 = 0x19802524_6f90_4346_93c2_63dbc509ab55;

/// Immutable marker the `rynk` firmware prepends to its USB serial number so a
/// host can pick RMK keyboards out of all serial ports without probing every
/// device.
pub const RYNK_SERIAL_MAGIC: &str = "rynk:";
