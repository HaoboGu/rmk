//! USB CDC-ACM serial transport using `tokio-serial`.
//!
//! [`connect_serial`] filters by RMK's USB VID, then probes candidates with
//! the Rynk handshake. [`discover_serial`] returns every responsive port for a
//! device picker.
//!
//! Candidates are filtered by USB **VID only** (the Rynk service isn't otherwise
//! discoverable on a raw CDC port), so probing opens every same-VID port and
//! writes a `GetVersion`/`GetCapabilities` handshake to it — including a
//! non-Rynk CDC device that happens to share RMK's VID. A non-Rynk device just
//! fails the handshake, but it does receive those unsolicited bytes. Narrowing
//! by USB product string / interface is a possible future refinement.

use std::time::Duration;

use rmk_host::{Client, ConnectError, RequestError, Transport, TransportError};
use rmk_types::protocol::rynk::{DeviceCapabilities, ProtocolVersion};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::task::JoinSet;
use tokio_serial::{ClearBuffer, SerialPort as _, SerialPortBuilderExt, SerialPortType, SerialStream};

/// Required by serial APIs; ignored by USB CDC-ACM devices.
const CDC_BAUD_RATE: u32 = 115_200;

/// Per-port handshake timeout used by serial discovery/connect helpers.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(1);

/// RMK's default USB VID (overridable in firmware via `keyboard.toml`).
pub const RMK_USB_VID: u16 = 0x4c4b;

/// Open CDC-ACM serial port.
pub struct SerialTransport {
    stream: SerialStream,
    path: String,
    read_buf: Box<[u8; 1024]>,
}

impl SerialTransport {
    /// Serial ports whose USB VID matches RMK's default.
    pub fn candidates() -> Result<Vec<String>, TransportError> {
        Self::candidates_with_vid(RMK_USB_VID)
    }

    /// Serial ports whose USB VID matches `vid`.
    pub fn candidates_with_vid(vid: u16) -> Result<Vec<String>, TransportError> {
        let ports = tokio_serial::available_ports().map_err(|e| TransportError::Io(e.to_string()))?;
        Ok(ports
            .into_iter()
            .filter(|p| matches!(&p.port_type, SerialPortType::UsbPort(info) if info.vid == vid))
            .map(|p| p.port_name)
            .collect())
    }

    /// Open a specific serial port path.
    pub async fn open(path: &str) -> Result<Self, TransportError> {
        let stream = tokio_serial::new(path, CDC_BAUD_RATE)
            .open_native_async()
            .map_err(|e| TransportError::Io(format!("open {path}: {e}")))?;
        // Best-effort cleanup of stale bytes from an old session.
        let _ = stream.clear(ClearBuffer::Input);
        Ok(Self {
            stream,
            path: path.to_string(),
            read_buf: Box::new([0u8; 1024]),
        })
    }

    /// The port path this transport is connected to.
    pub fn path(&self) -> &str {
        &self.path
    }
}

impl Transport for SerialTransport {
    async fn send(&mut self, frame: &[u8]) -> Result<(), TransportError> {
        self.stream
            .write_all(frame)
            .await
            .map_err(|e| TransportError::Io(e.to_string()))?;
        self.stream.flush().await.map_err(|e| TransportError::Io(e.to_string()))
    }

    async fn recv(&mut self) -> Result<Vec<u8>, TransportError> {
        match self.stream.read(&mut self.read_buf[..]).await {
            Ok(0) => Err(TransportError::Disconnected),
            Ok(n) => Ok(self.read_buf[..n].to_vec()),
            Err(e) => Err(TransportError::Io(e.to_string())),
        }
    }
}

/// A responsive Rynk serial device, for building a device picker.
pub struct SerialDevice {
    pub path: String,
    pub version: ProtocolVersion,
    pub capabilities: DeviceCapabilities,
}

/// Connect to the first VID-matching serial port that passes the handshake.
pub async fn connect_serial() -> Result<Client<SerialTransport>, ConnectError> {
    connect_serial_vid(RMK_USB_VID).await
}

/// [`connect_serial`] with a custom USB VID.
pub async fn connect_serial_vid(vid: u16) -> Result<Client<SerialTransport>, ConnectError> {
    let candidates = SerialTransport::candidates_with_vid(vid).map_err(ConnectError::Transport)?;
    if candidates.is_empty() {
        return Err(ConnectError::Transport(TransportError::DeviceNotFound(format!(
            "no USB serial port with vendor id {vid:#06x} found"
        ))));
    }
    let total = candidates.len();
    let mut probes = JoinSet::new();
    for path in candidates {
        probes.spawn(async move { connect_transport(SerialTransport::open(&path).await?).await });
    }
    let mut last_err = ConnectError::Transport(TransportError::DeviceNotFound("handshake timed out".into()));
    // A real Rynk reply (wrong version / rejection) is the most informative error,
    // but it must not abort the probe: another matching-VID port may host a fully
    // compatible keyboard. Keep probing, prefer any working client, and surface
    // the definitive error only when no port answered successfully.
    let mut definitive_err: Option<ConnectError> = None;
    while let Some(joined) = probes.join_next().await {
        match joined {
            Ok(Ok(client)) => return Ok(client),
            Ok(Err(e @ (ConnectError::VersionMismatch { .. } | ConnectError::Request(RequestError::Rejected(_))))) => {
                definitive_err.get_or_insert(e);
            }
            Ok(Err(e)) => last_err = e,
            Err(_) => {}
        }
    }
    Err(definitive_err.unwrap_or(ConnectError::NoResponsiveDevice {
        probed: total,
        last: Box::new(last_err),
    }))
}

/// Connect to a specific serial port.
pub async fn connect_serial_path(path: &str) -> Result<Client<SerialTransport>, ConnectError> {
    connect_transport(SerialTransport::open(path).await?).await
}

/// Probe every VID-matching port concurrently and return the responsive ones.
/// Unlike [`connect_serial`], this waits for all candidates so a picker can
/// list them — use it for `list`, then [`connect_serial_path`] to attach.
pub async fn discover_serial() -> Result<Vec<SerialDevice>, TransportError> {
    discover_serial_vid(RMK_USB_VID).await
}

/// [`discover_serial`] with a custom USB VID.
pub async fn discover_serial_vid(vid: u16) -> Result<Vec<SerialDevice>, TransportError> {
    let candidates = SerialTransport::candidates_with_vid(vid)?;
    let mut probes = JoinSet::new();
    for path in candidates {
        probes.spawn(async move {
            let client = connect_transport(SerialTransport::open(&path).await.ok()?).await.ok()?;
            Some(SerialDevice {
                path,
                version: client.protocol_version(),
                capabilities: *client.capabilities(),
            })
        });
    }
    let mut found = Vec::new();
    while let Some(joined) = probes.join_next().await {
        if let Ok(Some(dev)) = joined {
            found.push(dev);
        }
    }
    Ok(found)
}

// Intentionally duplicated in `rmk-host-ble` rather than shared: `rmk-host` is
// deliberately runtime-free (no `tokio`, builds for `wasm32`), so the timeout
// wrapper can't live there. Each transport crate owns its own runtime.
async fn connect_transport(transport: SerialTransport) -> Result<Client<SerialTransport>, ConnectError> {
    tokio::time::timeout(HANDSHAKE_TIMEOUT, Client::connect(transport))
        .await
        .map_err(|_| ConnectError::Transport(TransportError::DeviceNotFound("handshake timed out".into())))?
}
