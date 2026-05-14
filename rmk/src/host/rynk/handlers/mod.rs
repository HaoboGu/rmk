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
//! async fn handle_<name>(&self, payload: &mut [u8]) -> Result<usize, RynkError>
//! ```
//!
//! `Ok(n)` is the byte count of the postcard-encoded `Ok::<T, RynkError>(value)`
//! the handler wrote into `payload`. On `Err(e)` the dispatcher overwrites
//! the payload with the postcard encoding of `Err::<(), RynkError>(e)` and
//! sets `payload_len = 2`. Handlers propagate `RynkError` with `?`; there are
//! no per-call decode/encode helpers — every postcard call is inlined at its
//! site so the error mapping stays local and visible.
//!
//! ## Borrow-across-await rule
//!
//! `KeyMap` is a `RefCell<KeyMapInner>`. Its public API is sync-only —
//! every method borrows, mutates, and drops within a single call. **Do
//! not** introduce code that holds a `RefCell` borrow across an
//! `.await`; under embassy's cooperative scheduler that lets a second
//! handler observe a still-borrowed cell and panic. Stick to
//! [`KeyboardContext`](crate::host::context::KeyboardContext)
//! accessors, which all uphold this rule by construction.

pub(crate) mod behavior;
pub(crate) mod combo;
pub(crate) mod connection;
pub(crate) mod fork;
pub(crate) mod keymap;
pub(crate) mod macro_data;
pub(crate) mod morse;
pub(crate) mod status;
pub(crate) mod system;
