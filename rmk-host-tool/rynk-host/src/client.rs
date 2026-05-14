//! Higher-level [`Client`] facade — wraps a [`Transport`] with the handshake
//! and aggregate operations.

use rmk_types::protocol::rynk::{DeviceCapabilities, ProtocolVersion, RynkError};
use thiserror::Error;

use crate::api;
use crate::transport::{Transport, TransportError};

/// The major protocol version this build of `rynk-host` was compiled
/// against. Hosts bail when the firmware reports `version.major !=
/// CLIENT_MAJOR_VERSION` because postcard wire encodings aren't
/// forward-compatible across major bumps.
pub const CLIENT_MAJOR_VERSION: u8 = 1;

/// The largest `minor` this build of `rynk-host` knows. Newer firmware
/// (higher `minor`) is rejected with a "regenerate rynk-host" error so
/// the host doesn't silently mis-decode a wire shape it doesn't know yet.
pub const CLIENT_MAX_MINOR_VERSION: u8 = 0;

/// Errors that can happen during [`Client::connect`].
#[derive(Debug, Error)]
pub enum ConnectError {
    #[error("transport error: {0}")]
    Transport(#[from] TransportError),
    #[error("firmware rejected handshake request: {0:?}")]
    FirmwareError(RynkError),
    #[error(
        "protocol version mismatch — firmware reports v{firmware_major}.{firmware_minor}, host supports up to v{host_major}.{host_max_minor}. \
         Regenerate `rynk-host` from a matching `rmk-types` checkout."
    )]
    VersionMismatch {
        firmware_major: u8,
        firmware_minor: u8,
        host_major: u8,
        host_max_minor: u8,
    },
}

/// High-level Rynk client.
///
/// `Client` owns the transport and the capability snapshot. Construct it
/// via [`Client::connect`]; on success, every subsequent API call should
/// see a consistent firmware view.
pub struct Client<T: Transport> {
    pub(crate) transport: T,
    capabilities: DeviceCapabilities,
}

impl<T: Transport> Client<T> {
    /// Perform the handshake: read the protocol version + capabilities,
    /// then bail on version mismatch. Schema-drift across same-version
    /// firmware/host combos is detected by the wire-format snapshot in
    /// `rmk-types`, so `ProtocolVersion::major`/`minor` is the only
    /// runtime gate.
    pub async fn connect(mut transport: T) -> Result<Self, ConnectError> {
        let version = api::system::get_version(&mut transport)
            .await?
            .map_err(ConnectError::FirmwareError)?;
        Self::check_version(version)?;

        let caps = api::system::get_capabilities(&mut transport)
            .await?
            .map_err(ConnectError::FirmwareError)?;

        Ok(Self {
            transport,
            capabilities: caps,
        })
    }

    fn check_version(v: ProtocolVersion) -> Result<(), ConnectError> {
        if v.major != CLIENT_MAJOR_VERSION || v.minor > CLIENT_MAX_MINOR_VERSION {
            return Err(ConnectError::VersionMismatch {
                firmware_major: v.major,
                firmware_minor: v.minor,
                host_major: CLIENT_MAJOR_VERSION,
                host_max_minor: CLIENT_MAX_MINOR_VERSION,
            });
        }
        Ok(())
    }

    /// Cached capability snapshot from [`Client::connect`]. The firmware
    /// can't change these mid-session, so reading once on connect is safe.
    pub fn capabilities(&self) -> &DeviceCapabilities {
        &self.capabilities
    }

    /// Direct access to the underlying transport — useful for ad-hoc Cmd
    /// calls that don't have a typed wrapper yet, and for subscribing to
    /// topics via [`Transport::topics`].
    pub fn transport(&mut self) -> &mut T {
        &mut self.transport
    }
}
