//! Rynk protocol ICD (Interface Control Document).
//!
//! Rynk is RMK's native host-communication protocol. It carries RMK's
//! canonical types (`KeyAction`, `Combo`, `Morse`, `Fork`, `EncoderAction`,
//! `BatteryStatus`, `BleStatus`) on the wire using a 5-byte fixed header +
//! postcard-encoded payload.
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
//! - **CMD** — `0x0000..=0x7FFF` request/response, `0x8000..=0xFFFF` topic.
//!   Top bit splits dispatch with one mask. See [`Cmd`].
//! - **SEQ** — opaque echo. Firmware copies request's SEQ into response;
//!   topics always send SEQ = 0.
//! - **LEN** — payload byte count. Authoritative end-of-message indicator.
//!
//! ## Response envelope
//!
//! Every response payload is postcard-encoded `Result<T, RynkError>`.
//! `Set*` calls use `T = ()` and `Get*` calls use `T` = the requested
//! type. This costs 1 byte on success and lets reads signal
//! `InvalidParameter` / `BadState` / `InternalError` without a per-cmd
//! sentinel. Requests are *not* wrapped — they're the bare postcard
//! encoding of the request struct.
//!
//! ## Module layout
//!
//! Layering, bottom-up; each module imports only from the ones below it:
//!
//! - `payload/` — wire vocabulary: the per-domain payload types
//!   (`keymap`, `encoder`, `combo`, …), pure serde data. Private; everything
//!   is re-exported flat at `protocol::rynk::*`.
//! - `error` — [`RynkError`], the protocol error code carried in every
//!   response envelope. Private; re-exported at `protocol::rynk::RynkError`.
//! - [`command`] — the [`Cmd`] identifier *and* the ICD table that binds each
//!   command to its payload types: the named `Cmd` constants, the
//!   `GetVersion`-style marker types, and the payload-size folds.
//! - [`endpoint`] — the [`Endpoint`](endpoint::Endpoint)/[`Topic`](endpoint::Topic)
//!   trait contracts the table's marker types implement.
//! - [`message`] — frame format: the 5-byte header carrying a `Cmd`, the
//!   [`RynkMessage`] buffer view, the `Result<T, RynkError>` envelope.
//!
//! The BLE transport binding (GATT UUIDs, chunk size) is a handful of
//! consts at the bottom of this file, off to the side of the wire-format
//! stack. Only `command`, `endpoint` and `message` are public.
//!
//! ## Protocol handshake
//!
//! 1. Host connects over USB bulk or BLE GATT (length-prefixed messages).
//! 2. Host sends `Cmd::GetVersion`. The host aborts only if `major`
//!    differs from its own; any `minor` under the same major connects,
//!    in both directions (see the version bump policy below).
//! 3. Host sends `Cmd::GetCapabilities` to learn layout, feature flags,
//!    and limits.
//! 4. Host gates every subsequent call on the capability flags.
//!
//! ### Version bump policy
//!
//! Within one `major`, every wire change must keep old hosts working —
//! that is what makes `minor` purely informational at connect time.
//!
//! - `minor` bump (additive only):
//!   - New request/response `Cmd` pair. Old firmware answers
//!     `RynkError::UnknownCmd`; hosts gate new calls on capabilities.
//!   - New topic, or a field appended to an existing topic payload. Old
//!     hosts surface unknown topics verbatim and ignore trailing topic
//!     bytes — that leniency is part of this contract.
//!   - New `RynkError` variant (the enum is `#[non_exhaustive]`). It
//!     only travels on a command's error path; an old host fails to
//!     decode that one reply, but the LEN-delimited framing keeps the
//!     stream synced.
//! - `major` bump (host refuses to connect):
//!   - Any change to an existing request or response payload layout —
//!     including appending a field; hosts reject trailing response
//!     bytes as a wire/type mismatch.
//!   - A `Cmd` retyped or renumbered; an enum variant renamed,
//!     renumbered, or reordered.
//!
//! The probe is frozen across **all** majors: the 5-byte header layout,
//! `Cmd::GetVersion = 0x0001`, and its `Result<ProtocolVersion,
//! RynkError>` reply with `ProtocolVersion {major: u8, minor: u8}` may
//! never change — they are what let any host identify any firmware.
//! Two golden files enforce this policy: `snapshots/wire_values.snap`
//! (test `wire_values_locked`) pins each type's payload encoding, and
//! `snapshots/wire_frames.snap` (test `wire_frames_locked`) pins the full
//! frame — header layout, CMD numbers, the `Result<T, RynkError>` reply
//! envelope, and the frozen probe — for every protocol message.
//!
//! The `Cmd` ↔ payload-type association itself is declared once, in
//! [`command`]: firmware handlers and the host client both compile
//! against that table, so the two ends cannot disagree about a
//! command's request/response (or a topic's payload) types.

pub mod command;
pub mod endpoint;
pub mod message;

mod error;
mod payload;

#[cfg(test)]
pub(crate) mod tests;

pub use self::command::{Cmd, RYNK_MAX_PAYLOAD};
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
