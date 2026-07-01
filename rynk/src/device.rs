//! [`RynkDevice`]: a keyboard recognized as Rynk's, before any link is opened.
//!
//! Discovery yields a `RynkDevice` for each keyboard a transport identifies — a
//! serial port bearing the Rynk magic, a BLE peripheral exposing the Rynk
//! service, a link the browser has already opened. It is an inert handle,
//! carrying only what a picker needs ([`label`](RynkDevice::label)) and the
//! means to open the link; it has negotiated nothing with the firmware.
//!
//! [`connect`](RynkDevice::connect) turns that handle into a live [`Client`]: it
//! opens the link and completes the handshake (version check and capability
//! snapshot). All request and topic traffic then runs through the `Client` — a
//! `RynkDevice` exists only to produce one. This connect step is the sole part
//! of the lifecycle common to every transport, and so it alone forms the trait:
//! native impls open a real link (serial port, BLE GATT) then handshake, while
//! the web impl wraps an already-open JS byte link. A consumer drives connect →
//! use once, generic over `D: RynkDevice`, naming the transport at a single site.
//!
//! Discovery is deliberately not part of the trait. Enumerating USB ports,
//! listing BLE services, and driving a browser chooser share no signature, so
//! each transport exposes its own inherent `discover()` (native) or leaves
//! discovery to JS (web). That divergence is what lets the web transport, which
//! cannot enumerate from wasm, still implement `RynkDevice`.

use embedded_io_async::{Read, Write};

use crate::driver::{Client, RynkHostError};

/// A keyboard recognized as Rynk's but not yet connected: an inert handle,
/// produced by a transport's `discover()`, that [`connect`](Self::connect)s into
/// a live [`Client`]. Implemented once per transport (USB serial, BLE, web);
/// discovery itself is each transport's own inherent call, not part of this
/// trait.
#[allow(async_fn_in_trait)] // concrete future `Send`-ness is fixed at each impl site
pub trait RynkDevice: Sized {
    /// The byte link this device opens, driven by [`Client`].
    type Transport: Read + Write;

    /// Display text for a device picker (serial path / BLE name).
    fn label(&self) -> String;

    /// Open the link without handshaking — the per-transport primitive. Consumes
    /// the handle: an open link is one session (a web link, once wrapped, can't
    /// be reopened).
    async fn open(self) -> Result<Self::Transport, RynkHostError>;

    /// Connect this recognized device into a live [`Client`]: open the link and
    /// complete the Rynk handshake.
    ///
    /// Runtime-free, so it carries no handshake timeout: a silent peer would hang
    /// here. Callers that need a bound wrap this in their runtime's timeout (the
    /// BLE attach inside [`open`](Self::open) is already internally bounded).
    async fn connect(self) -> Result<Client<Self::Transport>, RynkHostError> {
        Client::connect(self.open().await?).await
    }
}
