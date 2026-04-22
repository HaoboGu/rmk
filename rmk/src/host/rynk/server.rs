//! postcard-rpc server wiring for rynk.
//!
//! Bridges our byte-level `HostRx` / `HostTx` to postcard-rpc's `WireRx` /
//! `WireTx`:
//! - `RynkWireRx<R>` delegates to `R::recv` directly.
//! - `RynkWireTx<T>` wraps `T` in a `Mutex` so that postcard-rpc's
//!   `&self` `send_raw` contract can drive a `&mut self` `HostTx::send`.
//!
//! The dispatch table itself is generated from
//! `rmk_types::protocol::rmk::ENDPOINT_LIST` via
//! `postcard_rpc::define_dispatch!`; handler bodies live in `super::dispatch`.

// TODO: flesh out RynkWireRx<R: HostRx> and RynkWireTx<T: HostTx> with the
// postcard_rpc::server::{WireRx, WireTx} impls, then drive
// postcard_rpc::define_dispatch! against ENDPOINT_LIST. For now the module
// is a placeholder so feature-gated compilation can land.
