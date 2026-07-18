//! [`RynkDevice`]: a keyboard recognized as Rynk's, before any link is opened.
//!
//! Connect (open the link + handshake) is the sole lifecycle step common to
//! every transport, so it alone forms the trait. Discovery shares no
//! signature — enumerating serial ports, scanning BLE, and a browser chooser
//! diverge, and wasm cannot enumerate at all — so each transport keeps its
//! own inherent `discover()` (or leaves discovery to JS).

#[cfg(feature = "alloc")]
use alloc::string::String;

use embassy_futures::select::{Either, select};
use embedded_io_async::{Read, Write};

use crate::driver::{Client, Driver, RynkHostError};

/// A keyboard recognized as Rynk's but not yet connected: an inert handle,
/// produced by a transport's `discover()`, that [`connect`](Self::connect)s
/// into a live [`Client`] + [`Driver`] pair. Implemented once per transport
/// (USB serial, BLE, web); discovery itself is each transport's own inherent
/// call, not part of this trait.
#[allow(async_fn_in_trait)] // concrete future `Send`-ness is fixed at each impl site
pub trait RynkDevice: Sized {
    /// The device→host half of the byte link this device opens.
    type Read: Read;
    /// The host→device half of the byte link this device opens.
    type Write: Write;

    /// Display text for a device picker (serial path / BLE name).
    #[cfg(feature = "alloc")]
    fn label(&self) -> String;

    /// Open the link without handshaking — the per-transport primitive — and
    /// hand out its reader followed by its writer. Consumes the handle: an
    /// open link is one session (a web link, once wrapped, can't be reopened).
    async fn open(self) -> Result<(Self::Read, Self::Write), RynkHostError>;

    /// Connect this recognized device into a live session: open the link and
    /// complete the Rynk handshake (version check and capability snapshot)
    /// over the normal pumps — topics arriving meanwhile queue up for
    /// `next_topic` as usual.
    ///
    /// Runtime-free, so no handshake timeout: a silent peer hangs here. Callers
    /// that need a bound wrap this in their runtime's timeout.
    async fn connect(self) -> Result<(Client, Driver<Self::Read, Self::Write>), RynkHostError> {
        let (reader, writer) = self.open().await?;
        let mut client = Client::new();
        let mut driver = Driver::new(reader, writer);
        let capabilities = match select(driver.run(&client), client.handshake()).await {
            Either::First(err) => return Err(err),
            Either::Second(caps) => caps?,
        };
        client.capabilities = capabilities;
        Ok((client, driver))
    }
}
