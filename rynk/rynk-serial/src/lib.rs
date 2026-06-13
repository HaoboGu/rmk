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

use embedded_io_adapters::tokio_1::FromTokio;
use rmk_types::protocol::rynk::{DeviceCapabilities, ProtocolVersion};
use rynk::io::{Read, Write};
use rynk::{Client, ConnectError, RequestError, TransportError};
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
    stream: FromTokio<SerialStream>,
    path: String,
}

impl SerialTransport {
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
            stream: FromTokio::new(stream),
            path: path.to_string(),
        })
    }

    /// The port path this transport is connected to.
    pub fn path(&self) -> &str {
        &self.path
    }
}

impl rynk::io::ErrorType for SerialTransport {
    type Error = std::io::Error;
}

impl Read for SerialTransport {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        self.stream.read(buf).await
    }
}

impl Write for SerialTransport {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        self.stream.write(buf).await
    }

    /// No tcdrain: tokio-serial's flush blocks the thread until the peer
    /// drains, which can stall the whole runtime on a wedged device; `write`
    /// already handed the bytes to the kernel.
    async fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
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

/// Enumerate VID-matching ports off the reactor — `available_ports` walks OS
/// device registries (IOKit/SetupAPI/sysfs) and can block for tens of ms.
async fn candidates_blocking(vid: u16) -> Result<Vec<String>, TransportError> {
    tokio::task::spawn_blocking(move || SerialTransport::candidates_with_vid(vid))
        .await
        .map_err(|e| TransportError::Io(e.to_string()))?
}

/// [`connect_serial`] with a custom USB VID.
pub async fn connect_serial_vid(vid: u16) -> Result<Client<SerialTransport>, ConnectError> {
    let candidates = candidates_blocking(vid).await?;
    if candidates.is_empty() {
        return Err(ConnectError::Transport(TransportError::DeviceNotFound(format!(
            "no USB serial port with vendor id {vid:#06x} found"
        ))));
    }
    probe_candidates(candidates).await
}

/// Probe candidate port paths concurrently; the first passing handshake wins.
async fn probe_candidates(candidates: Vec<String>) -> Result<Client<SerialTransport>, ConnectError> {
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
    let candidates = candidates_blocking(vid).await?;
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

// Intentionally duplicated in `rynk-ble` rather than shared: `rynk` is
// deliberately runtime-free (no `tokio`, builds for `wasm32`), so the timeout
// wrapper can't live there. Each transport crate owns its own runtime.
async fn connect_transport(transport: SerialTransport) -> Result<Client<SerialTransport>, ConnectError> {
    tokio::time::timeout(HANDSHAKE_TIMEOUT, Client::connect(transport))
        .await
        .map_err(|_| ConnectError::Transport(TransportError::DeviceNotFound("handshake timed out".into())))?
}

// PTY-backed tests: `SerialStream::pair()` gives a real byte stream with serial
// semantics but no hardware, so the transport, the handshake timeout, and the
// concurrent probe all run against a scripted peer. Unix only, like the pair.
#[cfg(all(test, unix))]
mod tests {
    use std::os::fd::AsRawFd;

    use rmk_types::protocol::rynk::{Cmd, RYNK_HEADER_SIZE, RynkError, RynkHeader, RynkMessage};
    use serde::Serialize;
    use tokio::io::AsyncReadExt as _;

    use super::*;

    /// A raw-mode PTY pair. `pair()` leaves the pty's line discipline as-is,
    /// so without `cfmakeraw` reads would be line-buffered and echoed.
    fn pty_pair() -> (SerialStream, SerialStream) {
        let (master, slave) = SerialStream::pair().unwrap();
        for fd in [master.as_raw_fd(), slave.as_raw_fd()] {
            unsafe {
                let mut t: libc::termios = std::mem::zeroed();
                assert_eq!(libc::tcgetattr(fd, &mut t), 0);
                libc::cfmakeraw(&mut t);
                assert_eq!(libc::tcsetattr(fd, libc::TCSANOW, &t), 0);
            }
        }
        (master, slave)
    }

    fn transport(stream: SerialStream) -> SerialTransport {
        SerialTransport {
            stream: FromTokio::new(stream),
            path: "<pty>".into(),
        }
    }

    /// Header + postcard payload, framed as the firmware sends it.
    fn frame<T: Serialize>(cmd: Cmd, seq: u8, value: &T) -> Vec<u8> {
        let mut buf = vec![0u8; 1024];
        let len = RynkMessage::build(&mut buf, cmd, seq, value).unwrap().frame_len();
        buf.truncate(len);
        buf
    }

    fn caps() -> DeviceCapabilities {
        DeviceCapabilities {
            num_layers: 4,
            num_rows: 6,
            num_cols: 14,
            num_encoders: 0,
            max_combos: 8,
            max_combo_keys: 4,
            max_macros: 8,
            macro_space_size: 1024,
            max_morse: 4,
            max_patterns_per_key: 4,
            max_forks: 4,
            storage_enabled: true,
            lighting_enabled: false,
            is_split: false,
            num_split_peripherals: 0,
            ble_enabled: false,
            num_ble_profiles: 0,
            max_payload_size: 256,
            max_bulk_keys: 0,
            macro_chunk_size: 64,
            bulk_transfer_supported: false,
        }
    }

    /// Read one request frame off the peer end; returns its cmd + seq.
    async fn read_request(peer: &mut SerialStream) -> (Cmd, u8) {
        let mut bytes = [0u8; RYNK_HEADER_SIZE];
        peer.read_exact(&mut bytes).await.unwrap();
        let header = RynkHeader::parse(&bytes);
        let mut payload = vec![0u8; header.payload_len as usize];
        if !payload.is_empty() {
            peer.read_exact(&mut payload).await.unwrap();
        }
        (header.cmd, header.seq)
    }

    /// Script a Rynk firmware on `peer`: answer the GetVersion/GetCapabilities
    /// handshake with `version`, then keep the line open until dropped.
    fn scripted_firmware(mut peer: SerialStream, version: ProtocolVersion) -> tokio::task::JoinHandle<SerialStream> {
        tokio::spawn(async move {
            let (cmd, seq) = read_request(&mut peer).await;
            assert_eq!(cmd, Cmd::GetVersion);
            tokio::io::AsyncWriteExt::write_all(&mut peer, &frame(cmd, seq, &Ok::<_, RynkError>(version)))
                .await
                .unwrap();
            // A mismatched major never gets the capabilities request.
            if version.major == ProtocolVersion::CURRENT.major {
                let (cmd, seq) = read_request(&mut peer).await;
                assert_eq!(cmd, Cmd::GetCapabilities);
                tokio::io::AsyncWriteExt::write_all(&mut peer, &frame(cmd, seq, &Ok::<_, RynkError>(caps())))
                    .await
                    .unwrap();
            }
            peer
        })
    }

    #[tokio::test]
    async fn transport_round_trips_bytes() {
        let (mut peer, ours) = pty_pair();
        let mut t = transport(ours);

        t.write_all(&[1, 2, 3]).await.unwrap();
        let mut buf = [0u8; 3];
        peer.read_exact(&mut buf).await.unwrap();
        assert_eq!(buf, [1, 2, 3]);

        tokio::io::AsyncWriteExt::write_all(&mut peer, &[9, 8]).await.unwrap();
        let mut got = [0u8; 2];
        t.read_exact(&mut got).await.unwrap();
        assert_eq!(got, [9, 8]);
    }

    #[tokio::test]
    async fn connect_handshakes_against_scripted_peer() {
        let (peer, ours) = pty_pair();
        let device = scripted_firmware(peer, ProtocolVersion::CURRENT);

        let client = connect_transport(transport(ours)).await.unwrap();
        assert_eq!(client.protocol_version(), ProtocolVersion::CURRENT);
        assert_eq!(client.capabilities().num_cols, 14);
        device.await.unwrap();
    }

    #[tokio::test]
    async fn connect_times_out_on_silent_peer() {
        // The peer end stays open but never answers; runs ~HANDSHAKE_TIMEOUT.
        let (_peer, ours) = pty_pair();
        let err = connect_transport(transport(ours)).await.err().expect("must time out");
        assert!(
            matches!(&err, ConnectError::Transport(TransportError::DeviceNotFound(m)) if m.contains("timed out")),
            "expected handshake timeout, got {err:?}"
        );
    }

    /// The probe must keep a silent same-VID port from masking a responsive one.
    /// Linux-only: macOS cannot open a pty through the serialport builder (the
    /// baud ioctl returns ENOTTY), and the probe opens ports by path.
    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn probe_prefers_responsive_port_over_silent_one() {
        let (_silent_peer, silent) = pty_pair();
        let (good_peer, good) = pty_pair();
        let silent_path = silent.name().expect("pty has a path");
        let good_path = good.name().expect("pty has a path");
        // Keep `silent`/`good` alive: the probe opens a second fd on each
        // path, and macOS refuses to re-open a fully closed pty slave.
        let device = scripted_firmware(good_peer, ProtocolVersion::CURRENT);

        let client = probe_candidates(vec![silent_path, good_path])
            .await
            .expect("responsive port wins");
        assert_eq!(client.protocol_version(), ProtocolVersion::CURRENT);
        device.await.unwrap();
    }

    /// A real-but-incompatible Rynk reply is the definitive error: it must
    /// surface over the generic timeout of other silent candidates.
    /// Linux-only, same pty limitation as above.
    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn probe_surfaces_version_mismatch_over_timeouts() {
        let (_silent_peer, silent) = pty_pair();
        let (old_peer, old) = pty_pair();
        let silent_path = silent.name().expect("pty has a path");
        let old_path = old.name().expect("pty has a path");
        let newer_major = ProtocolVersion {
            major: ProtocolVersion::CURRENT.major + 1,
            minor: 0,
        };
        let device = scripted_firmware(old_peer, newer_major);

        let err = probe_candidates(vec![silent_path, old_path])
            .await
            .err()
            .expect("no compatible port");
        assert!(
            matches!(err, ConnectError::VersionMismatch { .. }),
            "expected the definitive VersionMismatch, got {err:?}"
        );
        device.await.unwrap();
    }
}
