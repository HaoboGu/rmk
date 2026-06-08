//! End-to-end Rynk integration test crate.
//!
//! This crate has no runtime code; it exists solely to host the cross-stack
//! integration test in `tests/loopback.rs`, which drives the real
//! [`rmk_host::Client`] against the real `rmk` firmware session over an
//! in-memory duplex. Keeping it in its own crate isolates the heavy `rmk`
//! dev-dependency (and the `rmk-types/host` feature it unifies) from the shipped
//! host crates.
