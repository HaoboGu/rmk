//! `rynk-host` — host-side library for the Rynk protocol.
//!
//! The crate is layered:
//!
//! - [`transport::Transport`] — low-level frame round-trip.
//! - [`transports`] — concrete USB / BLE implementations.
//! - [`client::Client`] — typed handshake + capability snapshot.
//! - [`api`] — typed wrappers per `Cmd` group.
//!
//! Re-exports `rmk_types` so downstream callers don't need to depend on
//! it directly.

pub mod api;
pub mod client;
pub mod framing;
pub mod transport;
pub mod transports;

pub use client::{Client, ConnectError};
pub use rmk_types;
pub use transport::{TopicFrame, Transport, TransportError};

/// Convenience alias for the host-side response envelope. Every firmware
/// response decodes to `Result<T, RynkError>` (defaulting to `()`); the
/// host-side API functions surface this directly so callers can pattern-
/// match on protocol errors without unwrapping a bare value.
pub type RynkResult<T = ()> = Result<T, rmk_types::protocol::rynk::RynkError>;
