//! Endpoint handlers for the RMK protocol.
//!
//! ## RefCell borrow-across-await ban
//!
//! `KeyMap` exposes its inner state through a `RefCell` (`rmk/src/keymap.rs:80-82`).
//! Every public accessor borrow-mutates-drops within a single non-async call,
//! so under embassy's cooperative scheduler concurrent USB and BLE handler tasks
//! cannot panic on overlapping borrows — they only interleave at `.await`
//! suspension points.
//!
//! Handlers MUST therefore never hold a `RefCell` borrow across `.await`. In
//! practice this means: if you need to use a `with_*_mut` closure that hands out
//! a `&mut` reference to inner state, do **not** `.await` anything inside that
//! closure. Drop the borrow before any `.await`.
//!
//! When in doubt: copy the data out of the closure first, drop the borrow,
//! then `.await` against the copy.

pub(super) mod behavior;
#[cfg(feature = "_ble")]
pub(super) mod ble;
pub(super) mod combo;
pub(super) mod connection;
pub(super) mod encoder;
pub(super) mod fork;
pub(super) mod keymap;
pub(super) mod macro_data;
pub(super) mod morse;
pub(super) mod status;
pub(super) mod system;
