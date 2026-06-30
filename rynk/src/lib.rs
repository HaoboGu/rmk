//! Runtime-free Rynk host protocol client.
//!
//! [`Client`] drives the Rynk protocol over any byte link implementing the
//! embedded-io-async [`Read`](io::Read) + [`Write`](io::Write) traits.
//! This crate does not depend on an async runtime; concrete transports
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
//! ## Transport contract
//!
//! Implement [`io::Read`] + [`io::Write`] via the [`io`] re-export so the
//! trait version always matches this crate. [`Client`] correctness rests on:
//!
//! - **Writes deliver without flush** — the client never calls
//!   [`flush`](io::Write::flush). A successful `write` MUST commit the returned
//!   bytes; on a lossy medium (e.g. BLE) use acknowledged writes, since a lost
//!   chunk desyncs the firmware's reassembler with no mid-frame resync.
//! - **`read` must be cancel-safe**: dropping its future must not lose
//!   delivered bytes. The client relies on this for caller-owned timeouts plus
//!   [`Client::resync`].
//! - `read` may return arbitrary chunk boundaries; the client reassembles
//!   frames. `Ok(0)` means the link is gone and surfaces as
//!   [`RynkHostError::Disconnected`].
//!
//! ## Multi-version dispatch
//!
//! [`Client::connect`] rejects only a protocol **major** mismatch. To support
//! several majors at once, link one `rynk` build per major (cargo `package`
//! renames) and probe with the newest first.
//! The probe(`GetVersion`) itself is frozen across all majors by the protocol ICD.

mod api;
mod device;
mod driver;
pub mod layout;

pub use api::IncomingTopic;
pub use device::RynkDevice;
pub use driver::{Client, RynkHostError, TopicFrame};
pub use embedded_io_async as io;
pub use layout::LayoutInfo;
pub use rmk_types;
/// The decoded topic union carried by [`IncomingTopic::Topic`]
pub use rmk_types::protocol::rynk::TopicEvent;
