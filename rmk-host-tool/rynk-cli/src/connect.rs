//! Transport selection.
//!
//! Picks USB or BLE based on the user's `--transport` flag. The default
//! `auto` tries USB first (instant on Linux/macOS thanks to nusb) and
//! falls back to BLE if no Rynk-capable USB device is enumerated.

use anyhow::Context;
use rmk_types::protocol::rynk::Cmd;
use rynk_host::transport::{TopicFrame, Transport, TransportError};
use rynk_host::transports::{BleGattTransport, UsbBulkTransport};
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::sync::broadcast;

use crate::TransportKind;

/// Sum-type wrapper over the concrete transports so `Client<T>` can be
/// constructed once and dispatched at runtime.
pub enum AnyTransport {
    Usb(UsbBulkTransport),
    Ble(BleGattTransport),
}

impl Transport for AnyTransport {
    async fn request<Req: Serialize + Send + Sync, Resp: DeserializeOwned + Send>(
        &mut self,
        cmd: Cmd,
        req: &Req,
    ) -> Result<Resp, TransportError> {
        match self {
            AnyTransport::Usb(t) => t.request(cmd, req).await,
            AnyTransport::Ble(t) => t.request(cmd, req).await,
        }
    }

    fn topics(&self) -> broadcast::Receiver<TopicFrame> {
        match self {
            AnyTransport::Usb(t) => t.topics(),
            AnyTransport::Ble(t) => t.topics(),
        }
    }
}

pub async fn connect(kind: TransportKind) -> anyhow::Result<AnyTransport> {
    match kind {
        TransportKind::Usb => UsbBulkTransport::connect()
            .await
            .map(AnyTransport::Usb)
            .context("USB connect failed"),
        TransportKind::Ble => BleGattTransport::connect()
            .await
            .map(AnyTransport::Ble)
            .context("BLE connect failed"),
        TransportKind::Auto => match UsbBulkTransport::connect().await {
            Ok(t) => Ok(AnyTransport::Usb(t)),
            Err(usb_err) => {
                eprintln!("USB connect failed ({usb_err}); falling back to BLE.");
                BleGattTransport::connect()
                    .await
                    .map(AnyTransport::Ble)
                    .context("BLE connect failed")
            }
        },
    }
}
