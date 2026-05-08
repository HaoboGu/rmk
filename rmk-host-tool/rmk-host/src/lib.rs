//! Host-side client library for the RMK protocol.
//!
//! Wraps `postcard_rpc::HostClient` with the RMK protocol's handshake
//! ([`Client::connect`] performs `GetVersion` → bail on `major` mismatch /
//! unsupported `minor` → `GetCapabilities` and caches the result) and exposes
//! typed wrappers for each endpoint group.
//!
//! See [`rmk_types::protocol::rmk`] for the wire types.

use std::time::Duration;

pub mod ble;

pub use nusb;
use postcard_rpc::header::VarSeqKind;
use postcard_rpc::host_client::{HostClient, HostErr};
use rmk_types::action::{EncoderAction, KeyAction};
use rmk_types::connection::ConnectionType;
use rmk_types::protocol::rmk::{
    BehaviorConfig, DeviceCapabilities, GetEncoderRequest, KeyPosition, MatrixState, ProtocolVersion,
    RmkError, SetEncoderRequest, SetKeyRequest, StorageResetMode,
};
use thiserror::Error;

/// Highest protocol minor we know how to talk to.
pub const SUPPORTED_MINOR: u8 = 0;
/// Major version this client is built against.
pub const SUPPORTED_MAJOR: u8 = 1;

/// Outgoing queue depth (in messages) for the underlying postcard-rpc client.
const OUTGOING_DEPTH: usize = 8;

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("transport error: {0}")]
    Transport(String),
    #[error("device replied with protocol error: {0:?}")]
    Protocol(RmkError),
    #[error("device returned unsupported version: {0:?}")]
    UnsupportedVersion(ProtocolVersion),
    #[error("operation requires a feature not advertised by device: {0}")]
    MissingCapability(&'static str),
    #[error("{0}")]
    Other(String),
}

impl<E: std::fmt::Debug> From<HostErr<E>> for ClientError {
    fn from(value: HostErr<E>) -> Self {
        ClientError::Transport(format!("{value:?}"))
    }
}

pub type Result<T> = std::result::Result<T, ClientError>;

/// Convenience wrapper around `postcard_rpc::HostClient`. Caches the device
/// capabilities so feature-gated calls can fail fast.
pub struct Client {
    inner: HostClient<RmkError>,
    capabilities: DeviceCapabilities,
}

impl Client {
    /// Connect to the first matching USB device, run the handshake, and cache
    /// device capabilities.
    ///
    /// `select` is a predicate that returns `true` for the device this client
    /// should attach to. Typically matches on Vendor/Product ID.
    pub async fn connect_usb<F>(select: F) -> Result<Self>
    where
        F: FnMut(&nusb::DeviceInfo) -> bool,
    {
        let inner = HostClient::<RmkError>::try_new_raw_nusb(
            select,
            "rmk_protocol",
            OUTGOING_DEPTH,
            VarSeqKind::Seq2,
        )
        .map_err(ClientError::Transport)?;
        Self::handshake(inner).await
    }

    /// Connect over BLE.
    ///
    /// `name_filter`: optional substring of the advertised local name to match
    /// (e.g. `"RMK nRF54LM20A"`). Pass `None` to connect to the first scanned
    /// device.
    pub async fn connect_ble(
        scan_timeout: std::time::Duration,
        name_filter: Option<&str>,
    ) -> Result<Self> {
        let inner = ble::connect_ble::<RmkError>(scan_timeout, name_filter)
            .await
            .map_err(|e| ClientError::Transport(e.to_string()))?;
        Self::handshake(inner).await
    }

    async fn handshake(inner: HostClient<RmkError>) -> Result<Self> {
        use rmk_types::protocol::rmk::{GetCapabilities, GetVersion};

        let version: ProtocolVersion = send_endpoint::<GetVersion>(&inner, &()).await?;
        if version.major != SUPPORTED_MAJOR || version.minor > SUPPORTED_MINOR {
            return Err(ClientError::UnsupportedVersion(version));
        }
        let capabilities: DeviceCapabilities = send_endpoint::<GetCapabilities>(&inner, &()).await?;
        Ok(Self { inner, capabilities })
    }

    /// Cached device capabilities (from the connection handshake).
    pub fn capabilities(&self) -> &DeviceCapabilities {
        &self.capabilities
    }

    pub async fn get_version(&self) -> Result<ProtocolVersion> {
        use rmk_types::protocol::rmk::GetVersion;
        send_endpoint::<GetVersion>(&self.inner, &()).await
    }

    pub async fn get_key_action(&self, pos: KeyPosition) -> Result<KeyAction> {
        use rmk_types::protocol::rmk::GetKeyAction;
        send_endpoint::<GetKeyAction>(&self.inner, &pos).await
    }

    pub async fn set_key_action(&self, position: KeyPosition, action: KeyAction) -> Result<()> {
        use rmk_types::protocol::rmk::SetKeyAction;
        let resp: std::result::Result<(), RmkError> =
            send_endpoint::<SetKeyAction>(&self.inner, &SetKeyRequest { position, action }).await?;
        resp.map_err(ClientError::Protocol)
    }

    pub async fn get_default_layer(&self) -> Result<u8> {
        use rmk_types::protocol::rmk::GetDefaultLayer;
        send_endpoint::<GetDefaultLayer>(&self.inner, &()).await
    }

    pub async fn set_default_layer(&self, layer: u8) -> Result<()> {
        use rmk_types::protocol::rmk::SetDefaultLayer;
        let resp: std::result::Result<(), RmkError> =
            send_endpoint::<SetDefaultLayer>(&self.inner, &layer).await?;
        resp.map_err(ClientError::Protocol)
    }

    pub async fn get_current_layer(&self) -> Result<u8> {
        use rmk_types::protocol::rmk::GetCurrentLayer;
        send_endpoint::<GetCurrentLayer>(&self.inner, &()).await
    }

    pub async fn get_encoder_action(&self, encoder_id: u8, layer: u8) -> Result<EncoderAction> {
        use rmk_types::protocol::rmk::GetEncoderAction;
        send_endpoint::<GetEncoderAction>(
            &self.inner,
            &GetEncoderRequest { encoder_id, layer },
        )
        .await
    }

    pub async fn set_encoder_action(
        &self,
        encoder_id: u8,
        layer: u8,
        action: EncoderAction,
    ) -> Result<()> {
        use rmk_types::protocol::rmk::SetEncoderAction;
        let resp: std::result::Result<(), RmkError> = send_endpoint::<SetEncoderAction>(
            &self.inner,
            &SetEncoderRequest {
                encoder_id,
                layer,
                action,
            },
        )
        .await?;
        resp.map_err(ClientError::Protocol)
    }

    pub async fn get_behavior_config(&self) -> Result<BehaviorConfig> {
        use rmk_types::protocol::rmk::GetBehaviorConfig;
        send_endpoint::<GetBehaviorConfig>(&self.inner, &()).await
    }

    pub async fn set_behavior_config(&self, cfg: BehaviorConfig) -> Result<()> {
        use rmk_types::protocol::rmk::SetBehaviorConfig;
        let resp: std::result::Result<(), RmkError> =
            send_endpoint::<SetBehaviorConfig>(&self.inner, &cfg).await?;
        resp.map_err(ClientError::Protocol)
    }

    pub async fn get_connection_type(&self) -> Result<ConnectionType> {
        use rmk_types::protocol::rmk::GetConnectionType;
        send_endpoint::<GetConnectionType>(&self.inner, &()).await
    }

    pub async fn set_connection_type(&self, ty: ConnectionType) -> Result<()> {
        use rmk_types::protocol::rmk::SetConnectionType;
        let resp: std::result::Result<(), RmkError> =
            send_endpoint::<SetConnectionType>(&self.inner, &ty).await?;
        resp.map_err(ClientError::Protocol)
    }

    pub async fn get_matrix_state(&self) -> Result<MatrixState> {
        use rmk_types::protocol::rmk::GetMatrixState;
        send_endpoint::<GetMatrixState>(&self.inner, &()).await
    }

    pub async fn storage_reset(&self, mode: StorageResetMode) -> Result<()> {
        use rmk_types::protocol::rmk::StorageReset;
        send_endpoint::<StorageReset>(&self.inner, &mode).await
    }

    pub async fn reboot(&self) -> Result<()> {
        use rmk_types::protocol::rmk::Reboot;
        // The device reboots immediately; ignore transport error after reply.
        let _ = tokio::time::timeout(
            Duration::from_secs(2),
            send_endpoint::<Reboot>(&self.inner, &()),
        )
        .await;
        Ok(())
    }

    pub async fn bootloader_jump(&self) -> Result<()> {
        use rmk_types::protocol::rmk::BootloaderJump;
        let _ = tokio::time::timeout(
            Duration::from_secs(2),
            send_endpoint::<BootloaderJump>(&self.inner, &()),
        )
        .await;
        Ok(())
    }

    /// Subscribe to `LayerChange` events. Each item is the active layer.
    pub async fn subscribe_layer_changes(
        &self,
        depth: usize,
    ) -> Result<postcard_rpc::host_client::MultiSubscription<u8>> {
        use rmk_types::protocol::rmk::LayerChangeTopic;
        self.inner
            .subscribe_multi::<LayerChangeTopic>(depth)
            .await
            .map_err(|_| ClientError::Other("failed to subscribe to layer change".into()))
    }
}

async fn send_endpoint<E>(client: &HostClient<RmkError>, req: &E::Request) -> Result<E::Response>
where
    E: postcard_rpc::Endpoint,
    E::Request: serde::Serialize + postcard_rpc::postcard_schema::Schema,
    E::Response: postcard_rpc::postcard_schema::Schema + serde::de::DeserializeOwned,
{
    client
        .send_resp::<E>(req)
        .await
        .map_err(|e| match e {
            HostErr::Wire(w) => ClientError::Protocol(w),
            other => ClientError::Transport(format!("{other:?}")),
        })
}
