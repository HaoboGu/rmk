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
use rmk_types::protocol::rynk::{ProtocolVersion, RynkError, command};

use crate::driver::{Client, Driver, Peer, RynkHostError};

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
    /// complete the Rynk handshake (version check, capability snapshot, and
    /// dongle probe) over the normal pumps — topics arriving meanwhile queue up
    /// for `next_topic` as usual.
    ///
    /// Runtime-free, so no handshake timeout: a silent peer hangs here. Callers
    /// that need a bound wrap this in their runtime's timeout.
    async fn connect(self) -> Result<(Client, Driver<Self::Read, Self::Write>), RynkHostError> {
        let (reader, writer) = self.open().await?;
        let mut client = Client::new();
        let mut driver = Driver::new(reader, writer);
        let peer = match select(driver.run(&client), handshake(&client)).await {
            Either::First(err) => return Err(err),
            Either::Second(peer) => peer?,
        };
        client.peer = peer;
        Ok((client, driver))
    }
}

/// Negotiate the version, fetch device capabilities, and identify what answered.
///
/// Rejects only major-version mismatches; same-major minors connect.
async fn handshake(client: &Client) -> Result<Peer, RynkHostError> {
    // All three ride one round trip. `GetDongleSlots` is the dongle probe: the
    // `0x09xx` segment is answered by a dongle and never forwarded, so only a
    // keyboard rejects it as unknown.
    let (version, capabilities, slots) = embassy_futures::join::join3(
        client.request::<command::GetVersion>(&()),
        client.request::<command::GetCapabilities>(&()),
        client.request::<command::GetDongleSlots>(&()),
    )
    .await;

    let is_dongle = match slots {
        Ok(_) => true,
        Err(RynkHostError::Rejected(RynkError::UnknownCmd)) => false,
        Err(err) => return Err(err),
    };

    // A dongle answers its own segment while the target keyboard is unreachable,
    // so the forwarded pair failing is a state to report, not a failed connect.
    // The version gate runs once a target exists — the dongle parses the frames
    // either way, and its own segment is what stays usable here.
    if is_dongle
        && matches!(
            version,
            Err(RynkHostError::Rejected(RynkError::NoTarget | RynkError::NotReady))
        )
    {
        return Ok(Peer::DongleUnselected);
    }

    let version = version?;
    let supported = ProtocolVersion::CURRENT;
    if version.major != supported.major {
        return Err(RynkHostError::VersionMismatch {
            firmware_major: version.major,
            firmware_minor: version.minor,
            host_major: supported.major,
            host_max_minor: supported.minor,
        });
    }
    if version.minor > supported.minor {
        log::info!(
            "rynk: firmware protocol v{}.{} is newer than this client's v{}.{}; new commands/topics may be unavailable",
            version.major,
            version.minor,
            supported.major,
            supported.minor
        );
    }
    // The version gate runs before the capabilities are released.
    let capabilities = capabilities?;
    Ok(if is_dongle {
        Peer::Dongle(capabilities)
    } else {
        Peer::Keyboard(capabilities)
    })
}
