//! Host configurator support (keymap editing, firmware introspection, etc.).
//!
//! Organized along two axes:
//! - **Protocol** — Via/Vial (`via/`) or RMK/rynk (`rynk/`). One picks a
//!   protocol via the `vial` or `rmk_protocol` Cargo feature (mutually
//!   exclusive; enforced in `crate::lib`).
//! - **Transport** — owned by the protocol. Vial has fixed 32-byte HID
//!   reports (USB + BLE); rynk has COBS-framed postcard bytes (USB bulk +
//!   BLE custom-serial).
//!
//! Each protocol owns its own transport traits — Vial uses `via::transport::{ViaRx, ViaTx}`,
//! rynk uses `postcard_rpc::server::{WireRx, WireTx}` directly. The two
//! never share a transport struct, so a cross-protocol byte-level trait
//! would add abstraction without sharing. Call sites use the
//! [`HostServiceImpl`] alias; the active service type implements
//! [`crate::input_device::Runnable`], which is the bound `run_keyboard` takes.

// The `vial` / `rmk_protocol` mutual-exclusivity guard lives in `crate::lib.rs`.
#[cfg(all(feature = "host", not(any(feature = "vial", feature = "rmk_protocol"))))]
compile_error!(
    "Enabling the `host` feature requires selecting a protocol: enable either `vial` or `rmk_protocol`."
);

#[cfg(feature = "rmk_protocol")]
pub(crate) mod rynk;
#[cfg(feature = "storage")]
pub(crate) mod storage;
#[cfg(feature = "vial")]
pub(crate) mod via;

// The active-protocol service type is re-exported as `HostServiceImpl`.
// Call sites import one name and get the correct service type for whichever
// protocol is enabled. Construction is still per-protocol — Vial's factories
// take a `VialConfig`, rynk's take none — so call sites cfg-gate the
// constructor arguments. The two services are never in scope simultaneously
// (mutually exclusive features).

#[cfg(feature = "vial")]
pub(crate) use via::VialService as HostServiceImpl;

#[cfg(feature = "rmk_protocol")]
pub(crate) use rynk::RynkService as HostServiceImpl;
