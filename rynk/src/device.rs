//! The connect lifecycle shared by every transport — native USB/BLE *and* web.
//!
//! [`RynkDevice`] is the one step that is universal across platforms: turn a
//! device handle into a live [`Client`]. Native impls open a real link (serial
//! port, BLE GATT) then handshake; the web impl wraps an already-open JS byte
//! link. A consumer writes the connect → use half of the lifecycle **once**,
//! generic over `D: RynkDevice`, naming the transport only at a single site.
//!
//! Discovery is deliberately *not* on this trait: enumerating USB ports, listing
//! BLE services, and driving a browser chooser share no signature, so each
//! transport exposes its own inherent `discover()` (native) or leaves discovery
//! to JS (web). That divergence is exactly why the web transport — which can't
//! enumerate from wasm — can still implement `RynkDevice`.

use embedded_io_async::{Read, Write};

use crate::driver::{Client, ConnectError, TransportError};

/// A Rynk keyboard handle that opens into a [`Client`], on one transport (USB
/// serial, BLE, web). Discovery is each transport's own inherent call, not part
/// of this trait.
#[allow(async_fn_in_trait)] // concrete future `Send`-ness is fixed at each impl site
pub trait RynkDevice: Sized {
    /// The byte link this device opens, driven by [`Client`].
    type Transport: Read + Write;

    /// Display text for a device picker (serial path / BLE name).
    fn label(&self) -> String;

    /// Open the link without handshaking — the per-transport primitive. Consumes
    /// the handle: an open link is one session (a web link, once wrapped, can't
    /// be reopened).
    async fn open(self) -> Result<Self::Transport, TransportError>;

    /// Open the link and complete the Rynk handshake.
    ///
    /// Runtime-free, so it carries no handshake timeout: a silent peer would hang
    /// here. Callers that need a bound wrap this in their runtime's timeout (the
    /// BLE attach inside [`open`](Self::open) is already internally bounded).
    async fn connect(self) -> Result<Client<Self::Transport>, ConnectError> {
        Client::connect(self.open().await?).await
    }
}
