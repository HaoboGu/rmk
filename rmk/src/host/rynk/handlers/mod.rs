//! Rynk command handlers.
//!
//! Each `Cmd` variant has one matching `handle_xxx` method on
//! [`RynkService`](super::RynkService). The handlers live in
//! `impl RynkService` blocks split across this directory by domain.
//!
//! ## Handler contract
//!
//! Every handler has signature
//!
//! ```ignore
//! async fn handle_<name>(&self, msg: &mut RynkMessage<'_>) -> Result<usize, RynkError>
//! ```
//!
//! A handler decodes its request (if any) with `msg.request::<T>()` — bounded
//! by the declared LEN, so a short frame is rejected rather than read from
//! response scratch — and writes its reply into `msg.response_payload_mut()`.
//! `Ok(n)` is the encoded byte count written there; on `Err(e)` the dispatcher
//! overwrites it with the error.

pub(crate) mod behavior;
pub(crate) mod combo;
pub(crate) mod connection;
pub(crate) mod fork;
pub(crate) mod keymap;
pub(crate) mod macro_data;
pub(crate) mod morse;
pub(crate) mod status;
pub(crate) mod system;
