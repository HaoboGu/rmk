//! Runtime-free Rynk host protocol client.
//!
//! [`Client`] drives the Rynk protocol over any byte link implementing the
//! embedded-io-async [`Read`](io::Read) + [`Write`](io::Write) traits — the
//! same seam the firmware session loop reads, so both ends of the wire share
//! one transport vocabulary, and anything with an embedded-io adapter (tokio
//! streams, embassy pipes, …) is already a transport. This crate does not
//! open devices and does not depend on an async runtime; concrete transports
//! live in separate crates and hand the link to [`Client::connect`].
//!
//! ```no_run
//! # use rynk::Client;
//! # async fn run<T: rynk::io::Read + rynk::io::Write>(transport: T) -> Result<(), Box<dyn std::error::Error>> {
//! let mut client = Client::connect(transport).await?;
//! let layer = client.get_current_layer().await?;
//! println!("active layer: {layer}");
//! # Ok(()) }
//! ```
//!
//! Each method returns the response value directly; a device rejection is
//! [`RequestError::Rejected`], so `?` carries both transport and firmware
//! failures.
//!
//! ## Transport contract
//!
//! Implement [`io::Read`] + [`io::Write`] via the [`io`] re-export so the
//! trait version always matches this crate. [`Client`] correctness rests on:
//!
//! - **Writes deliver without flush** — the client never calls
//!   [`flush`](io::Write::flush). Chunk internally to the medium and use
//!   acknowledged writes where it can drop (e.g. BLE); a silently lost chunk
//!   desyncs the firmware's stream reassembler.
//! - **`read` must be cancel-safe**: dropping its future must not lose
//!   delivered bytes. The client relies on this for caller-owned timeouts plus
//!   [`Client::resync`].
//! - `read` may return arbitrary chunk boundaries; the client reassembles
//!   frames. `Ok(0)` means the link is gone and surfaces as
//!   [`TransportError::Disconnected`].
//!
//! ## Multi-version dispatch
//!
//! [`Client::connect`] rejects only a protocol **major** mismatch. To support
//! several majors at once, link one `rynk` build per major (cargo `package`
//! renames) and probe with the newest first: `&mut T` also implements the I/O
//! traits (embedded-io blanket impls), so `Client::connect(&mut transport)`
//! borrows the link, and on [`ConnectError::VersionMismatch`] the handshake
//! round trip has already completed — the same transport retries cleanly with
//! the next client. The probe itself (`GetVersion`, the 5-byte header, and the
//! version reply) is frozen across all majors by the protocol ICD, and every
//! linked major must pin the **same `embedded-io-async` version** so the trait
//! identity transports implement is shared.

mod api;
mod driver;

pub use api::Event;
pub use driver::{Client, ConnectError, RequestError, TopicFrame, TransportError};
/// The byte-link traits [`Client`] is generic over, re-exported so transports
/// implement exactly the version this crate consumes.
pub use embedded_io_async as io;
/// The protocol/wire types appearing in [`Client`] method signatures,
/// re-exported so downstream crates import them from the matching version.
pub use rmk_types;
