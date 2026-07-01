//! USB CDC-ACM serial transport using `tokio-serial`.
//!
//! [`SerialDevice::discover`] returns one [`SerialDevice`] per Rynk keyboard,
//! recognized by the [`RYNK_SERIAL_MAGIC`] marker in its USB serial number — an
//! immutable tag the `rynk` firmware prepends regardless of the user-configured
//! VID/serial. The OS caches the serial string at enumeration, so discovery reads
//! it on Windows/macOS/Linux *without opening the port*; the app then picks a
//! device and calls [`RynkDevice::connect`], which opens it and completes the Rynk
//! handshake — the authoritative confirmation.
//!
//! Discovery deliberately never opens a port: opening a CDC port toggles DTR
//! (resetting some MCUs), so only the chosen device is opened, exactly once. The
//! marker is to BLE's service UUID what identifies a device before connecting.

use embedded_io_adapters::tokio_1::FromTokio;
use rmk_types::protocol::rynk::RYNK_SERIAL_MAGIC;
use rynk::io::{Read, Write};
use rynk::{RynkDevice, RynkHostError};
use tokio_serial::{ClearBuffer, SerialPort as _, SerialPortBuilderExt, SerialPortInfo, SerialPortType, SerialStream};

/// Required by serial APIs; ignored by USB CDC-ACM devices.
const CDC_BAUD_RATE: u32 = 115_200;

/// Open CDC-ACM serial port.
///
/// Dropping this (with the owning `Client`) ends the Rynk **session** only:
/// the keyboard stays connected and usable.
pub struct SerialTransport {
    io: FromTokio<SerialStream>,
}

impl rynk::io::ErrorType for SerialTransport {
    type Error = std::io::Error;
}

impl Read for SerialTransport {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        self.io.read(buf).await
    }
}

impl Write for SerialTransport {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        self.io.write(buf).await
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// A Rynk keyboard found by [`SerialDevice::discover`], for building a device
/// picker. Carries the port path and the USB product name (the display
/// [`label`](RynkDevice::label)); version and capabilities are read by
/// [`connect`](RynkDevice::connect), the first time the port is opened.
pub struct SerialDevice {
    pub path: String,
    /// USB product string from the device descriptor, if it carried one.
    pub name: Option<String>,
}

impl SerialDevice {
    /// List the marked USB CDC ports — one [`SerialDevice`] per Rynk keyboard,
    /// recognized by [`RYNK_SERIAL_MAGIC`] without opening any port.
    pub async fn discover() -> Result<Vec<Self>, RynkHostError> {
        Ok(Self::rynk_serial_ports()?
            .into_iter()
            .map(|port| {
                let name = match port.port_type {
                    SerialPortType::UsbPort(info) => info.product,
                    _ => None,
                };
                SerialDevice {
                    path: port.port_name,
                    name,
                }
            })
            .collect())
    }

    /// List the USB CDC ports whose serial number carries the Rynk marker.
    fn rynk_serial_ports() -> Result<Vec<SerialPortInfo>, RynkHostError> {
        let ports = tokio_serial::available_ports().map_err(|e| RynkHostError::Io(e.to_string()))?;
        let mut ports: Vec<SerialPortInfo> = ports.into_iter().filter(Self::serial_is_rynk).collect();
        // macOS exposes one USB CDC device as both `/dev/cu.*` and `/dev/tty.*`:
        // keep only the `cu.*` node. Other platforms have no `cu.*` sibling.
        let cu_nodes: std::collections::HashSet<String> = ports
            .iter()
            .map(|p| p.port_name.clone())
            .filter(|p| p.starts_with("/dev/cu."))
            .collect();
        ports.retain(|p| match p.port_name.strip_prefix("/dev/tty.") {
            Some(suffix) => !cu_nodes.contains(&format!("/dev/cu.{suffix}")),
            None => true,
        });
        Ok(ports)
    }

    /// Helper function for checking whether a serial port has Rynk marker
    fn serial_is_rynk(port: &SerialPortInfo) -> bool {
        match &port.port_type {
            SerialPortType::UsbPort(info) => info
                .serial_number
                .as_deref()
                .is_some_and(|s| s.to_ascii_lowercase().contains(RYNK_SERIAL_MAGIC)),
            _ => false,
        }
    }
}

impl RynkDevice for SerialDevice {
    type Transport = SerialTransport;

    /// The USB product name, falling back to the port path when the descriptor
    /// carried none.
    fn label(&self) -> String {
        self.name.clone().unwrap_or_else(|| self.path.clone())
    }

    /// Open the port. A device unplugged since discovery surfaces as a normal
    /// [`RynkHostError`].
    async fn open(self) -> Result<SerialTransport, RynkHostError> {
        let stream = tokio_serial::new(&self.path, CDC_BAUD_RATE)
            .open_native_async()
            .map_err(|e| RynkHostError::Io(format!("open {}: {}", &self.path, e)))?;
        // Best-effort cleanup of stale bytes from an old session.
        let _ = stream.clear(ClearBuffer::Input);
        Ok(SerialTransport {
            io: FromTokio::new(stream),
        })
    }
}

// PTY-backed tests: `SerialStream::pair()` is a real serial byte stream with no
// hardware, so transport, timeout, and probe all run against a scripted peer.
// Unix only, like the pair.
#[cfg(all(test, unix))]
mod tests {
    use std::os::fd::AsRawFd;
    use std::time::Duration;

    use rmk_types::protocol::rynk::{
        Cmd, DeviceCapabilities, ProtocolVersion, RYNK_HEADER_SIZE, RYNK_MIN_BUFFER_SIZE, RynkError, RynkHeader,
        RynkMessage,
    };
    use rynk::Client;
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
            io: FromTokio::new(stream),
        }
    }

    /// Header + postcard payload, framed as the firmware sends it.
    fn frame<T: Serialize>(cmd: Cmd, seq: u8, value: &T) -> Vec<u8> {
        let mut buf = vec![0u8; RYNK_MIN_BUFFER_SIZE];
        let len = RynkMessage::build(&mut buf, cmd, seq, value).unwrap().frame_len();
        buf.truncate(len);
        buf
    }

    // Representative device; omitted fields take their zero/false default.
    fn caps() -> DeviceCapabilities {
        DeviceCapabilities {
            num_layers: 4,
            num_rows: 6,
            num_cols: 14,
            max_combos: 8,
            max_combo_keys: 4,
            max_macros: 8,
            macro_space_size: 1024,
            max_morse: 4,
            max_patterns_per_key: 4,
            max_forks: 4,
            storage_enabled: true,
            max_payload_size: 256,
            macro_chunk_size: 64,
            ..Default::default()
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

    #[test]
    fn rynk_serial_ports_enumerates() {
        // Returns marked ports the host has (maybe none in CI); must not error.
        SerialDevice::rynk_serial_ports().expect("enumeration must not error");
    }

    #[tokio::test]
    async fn connect_handshakes_against_scripted_peer() {
        let (peer, ours) = pty_pair();
        let device = scripted_firmware(peer, ProtocolVersion::CURRENT);

        // The serial transport carries the GetVersion+GetCapabilities handshake;
        // connect succeeding proves the full round trip. The negotiated values are
        // asserted against the real firmware in the core driver's loopback test.
        Client::connect(transport(ours)).await.unwrap();
        device.await.unwrap();
    }

    #[tokio::test]
    async fn connect_times_out_on_silent_peer() {
        // The peer end stays open but never answers, so `Client::connect` would
        // hang forever; consumers bound it with their own timeout (the lifecycle's
        // `connect` is runtime-free and carries none). Runs ~1s.
        let (_peer, ours) = pty_pair();
        let timed_out = tokio::time::timeout(Duration::from_secs(1), Client::connect(transport(ours))).await;
        assert!(timed_out.is_err(), "connect must not resolve against a silent peer");
    }
}
