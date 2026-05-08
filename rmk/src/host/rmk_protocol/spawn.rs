//! Embassy `WireSpawn` impl shared between the USB and BLE transports.
//!
//! The RMK protocol does not currently use the `spawn` flavour of
//! `define_dispatch!` (every endpoint handler is `async`, run inline by the
//! per-transport `Server`), so this module provides a stub `WireSpawn` that is
//! never actually invoked. The plumbing exists because `define_dispatch!`
//! requires a `spawn_impl` even when no handler uses it.

use core::convert::Infallible;

use postcard_rpc::server::WireSpawn;

#[derive(Clone, Copy, Default)]
pub(crate) struct RmkProtocolSpawn;

impl WireSpawn for RmkProtocolSpawn {
    type Error = Infallible;
    type Info = ();

    fn info(&self) -> &Self::Info {
        &()
    }
}

/// `spawn_fn` referenced by `define_dispatch!`. Always errors out — we never
/// invoke it because no handler uses the `spawn` arm.
pub(crate) fn spawn_fn<S>(_sp: &RmkProtocolSpawn, _tok: S) -> Result<(), Infallible> {
    Ok(())
}
