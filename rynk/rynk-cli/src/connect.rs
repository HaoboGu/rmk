//! Transport selection.
//!
//! Picks USB serial or BLE based on the user's `--transport` flag. The default
//! `auto` tries USB first (discovery never opens a port, so it's instant on
//! Linux/macOS/Windows) and falls back to BLE if no Rynk-capable serial device
//! is enumerated.

use anyhow::Context;
use rynk::io::{ErrorType, Read, Write};
use rynk::{RynkDevice, TransportError};
use rynk_ble::BleDevice;
use rynk_serial::SerialDevice;

use crate::TransportKind;

/// Sum-type over the concrete byte links so one `Client<AnyTransport>` is built
/// once and dispatched at runtime. Both transports surface `std::io::Error`.
pub enum AnyTransport {
    Serial(<SerialDevice as RynkDevice>::Transport),
    Ble(<BleDevice as RynkDevice>::Transport),
}

impl ErrorType for AnyTransport {
    type Error = std::io::Error;
}

impl Read for AnyTransport {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        match self {
            AnyTransport::Serial(t) => t.read(buf).await,
            AnyTransport::Ble(t) => t.read(buf).await,
        }
    }
}

impl Write for AnyTransport {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        match self {
            AnyTransport::Serial(t) => t.write(buf).await,
            AnyTransport::Ble(t) => t.write(buf).await,
        }
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        match self {
            AnyTransport::Serial(t) => t.flush().await,
            AnyTransport::Ble(t) => t.flush().await,
        }
    }
}

pub async fn connect(kind: TransportKind) -> anyhow::Result<AnyTransport> {
    match kind {
        TransportKind::Usb => open_serial().await,
        TransportKind::Ble => open_ble().await,
        TransportKind::Auto => match open_serial().await {
            Ok(t) => Ok(t),
            Err(usb_err) => {
                eprintln!("USB serial connect failed ({usb_err:#}); falling back to BLE.");
                open_ble().await
            }
        },
    }
}

/// Discover, pick the first device, and open its link. Discovery is
/// connectable-only: a BLE device must already be connected to the OS.
async fn open_serial() -> anyhow::Result<AnyTransport> {
    let device = first_device(SerialDevice::discover().await, "USB serial")?;
    let label = device.label();
    let transport = device
        .open()
        .await
        .with_context(|| format!("opening serial device {label}"))?;
    Ok(AnyTransport::Serial(transport))
}

async fn open_ble() -> anyhow::Result<AnyTransport> {
    let device = first_device(BleDevice::discover().await, "BLE")?;
    let label = device.label();
    let transport = device
        .open()
        .await
        .with_context(|| format!("opening BLE device {label}"))?;
    Ok(AnyTransport::Ble(transport))
}

/// Take the first discovered device, turning an empty list into a clear error.
fn first_device<D>(discovered: Result<Vec<D>, TransportError>, kind: &str) -> anyhow::Result<D> {
    discovered
        .with_context(|| format!("{kind} discovery failed"))?
        .into_iter()
        .next()
        .with_context(|| format!("no Rynk {kind} device found"))
}
