//! The device-lifecycle abstraction shared by every transport.
//!
//! Each transport crate implements [`RynkDevice`] for its discovered-device
//! handle, so a consumer writes the discover → connect → use lifecycle **once**,
//! generic over `D: RynkDevice`, and names the transport only at a single
//! instantiation site. Discovery stays per-transport and typed (`Vec<Self>`) so
//! an app can list USB and BLE keyboards separately.

use embedded_io_async::{Read, Write};

use crate::driver::{Client, ConnectError, TransportError};

/// A discoverable Rynk keyboard on one transport (USB serial, BLE, …).
#[allow(async_fn_in_trait)] // concrete future `Send`-ness is fixed at each impl site
pub trait RynkDevice: Sized {
    /// The byte link this device opens, driven by [`Client`].
    type Transport: Read + Write;

    /// Enumerate this transport's Rynk keyboards.
    async fn discover() -> Result<Vec<Self>, TransportError>;

    /// Display text for a device picker (serial path / BLE name).
    fn label(&self) -> String;

    /// Open the link without handshaking — the per-transport primitive.
    async fn open(&self) -> Result<Self::Transport, TransportError>;

    /// Open the link and complete the Rynk handshake.
    ///
    /// Runtime-free, so it carries no handshake timeout: a silent peer would hang
    /// here. Callers that need a bound wrap this in their runtime's timeout (the
    /// BLE attach inside [`open`](Self::open) is already internally bounded).
    async fn connect(&self) -> Result<Client<Self::Transport>, ConnectError> {
        Client::connect(self.open().await?).await
    }
}
