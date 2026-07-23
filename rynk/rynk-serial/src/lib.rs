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

use std::collections::HashSet;

use embedded_io_adapters::tokio_1::FromTokio;
use rynk::rmk_types::protocol::rynk::RYNK_SERIAL_MAGIC;
use rynk::{RynkDevice, RynkHostError};
use tokio::io::{ReadHalf, WriteHalf};
use tokio_serial::{SerialPortBuilderExt, SerialPortInfo, SerialPortType, SerialStream};

/// Required by serial APIs; ignored by USB CDC-ACM devices.
const CDC_BAUD_RATE: u32 = 115_200;

/// The open port's halves for [`RynkDevice::open`]. Dropping them (with the
/// owning session) ends the Rynk **session** only: the keyboard stays
/// connected and usable.
type SerialReader = FromTokio<ReadHalf<SerialStream>>;
type SerialWriter = FromTokio<WriteHalf<SerialStream>>;

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
    pub fn discover() -> Result<Vec<Self>, RynkHostError> {
        let ports =
            tokio_serial::available_ports().map_err(|e| RynkHostError::Transport("available_ports", e.to_string()))?;
        Ok(Self::from_ports(ports))
    }

    fn from_ports(ports: Vec<SerialPortInfo>) -> Vec<Self> {
        let mut devices: Vec<Self> = ports
            .into_iter()
            .filter_map(|port| {
                let SerialPortType::UsbPort(info) = port.port_type else {
                    return None;
                };

                let is_rynk = info
                    .serial_number
                    .as_deref()
                    .and_then(|serial| serial.get(..RYNK_SERIAL_MAGIC.len()))
                    .is_some_and(|prefix| prefix.eq_ignore_ascii_case(RYNK_SERIAL_MAGIC));
                if !is_rynk {
                    return None;
                }

                Some(Self {
                    path: port.port_name,
                    name: info.product,
                })
            })
            .collect();

        let cu_suffixes: HashSet<_> = devices
            .iter()
            .filter_map(|device| device.path.strip_prefix("/dev/cu."))
            .map(str::to_owned)
            .collect();
        devices.retain(|device| {
            device
                .path
                .strip_prefix("/dev/tty.")
                .is_none_or(|suffix| !cu_suffixes.contains(suffix))
        });

        devices.sort_unstable_by(|a, b| a.path.cmp(&b.path).then_with(|| a.name.cmp(&b.name)));
        devices
    }
}

impl RynkDevice for SerialDevice {
    type Read = SerialReader;
    type Write = SerialWriter;

    /// The USB product name, falling back to the port path when the descriptor
    /// carried none.
    fn label(&self) -> String {
        self.name.clone().unwrap_or_else(|| self.path.clone())
    }

    /// Open the port. A device unplugged since discovery surfaces as a normal
    /// [`RynkHostError`].
    async fn open(self) -> Result<(SerialReader, SerialWriter), RynkHostError> {
        let stream = tokio_serial::new(&self.path, CDC_BAUD_RATE)
            .open_native_async()
            .map_err(|e| RynkHostError::Transport("open", e.to_string()))?;
        let (reader, writer) = tokio::io::split(stream);
        Ok((FromTokio::new(reader), FromTokio::new(writer)))
    }
}

#[cfg(test)]
mod discovery_tests {
    use tokio_serial::UsbPortInfo;

    use super::*;

    fn usb(path: &str, serial: Option<&str>, product: Option<&str>) -> SerialPortInfo {
        SerialPortInfo {
            port_name: path.into(),
            port_type: SerialPortType::UsbPort(UsbPortInfo {
                vid: 0x1209,
                pid: 0x0001,
                serial_number: serial.map(Into::into),
                manufacturer: None,
                product: product.map(Into::into),
            }),
        }
    }

    #[test]
    fn filters_marker_prefix_case_insensitively() {
        let ports = vec![
            usb("/dev/third", Some("prefix-rynk:123"), Some("embedded marker")),
            usb("/dev/second", Some("RYNK:456"), Some("second")),
            usb("/dev/first", Some("rynk:123"), Some("first")),
            usb("/dev/missing", None, None),
            SerialPortInfo {
                port_name: "/dev/not-usb".into(),
                port_type: SerialPortType::Unknown,
            },
        ];

        let devices = SerialDevice::from_ports(ports);
        let found: Vec<_> = devices
            .iter()
            .map(|device| (device.path.as_str(), device.name.as_deref()))
            .collect();
        assert_eq!(found, [("/dev/first", Some("first")), ("/dev/second", Some("second"))]);
    }

    #[test]
    fn prefers_mac_cu_node() {
        let ports = vec![
            usb("/dev/tty.keyboard", Some("rynk:123"), Some("tty")),
            usb("/dev/cu.keyboard", Some("RYNK:123"), Some("cu")),
            usb("/dev/tty.other", Some("rynk:456"), Some("other")),
        ];

        let devices = SerialDevice::from_ports(ports);
        let found: Vec<_> = devices
            .iter()
            .map(|device| (device.path.as_str(), device.name.as_deref()))
            .collect();
        assert_eq!(
            found,
            [("/dev/cu.keyboard", Some("cu")), ("/dev/tty.other", Some("other"))]
        );
    }
}

// PTY-backed tests run the serial transport without hardware.
#[cfg(all(test, unix))]
mod tests {
    use std::os::fd::AsRawFd;
    use std::time::Duration;

    use rynk::io::{Read as _, Write as _};
    use rynk::rmk_types::protocol::rynk::{
        Cmd, DeviceCapabilities, ProtocolVersion, RYNK_HEADER_SIZE, RynkError, RynkHeader, RynkMessage,
    };
    use serde::Serialize;
    use tokio::io::AsyncReadExt as _;

    use super::*;

    /// A PTY end as a device, so tests connect through [`RynkDevice::connect`].
    struct PtyDevice(SerialStream);

    impl RynkDevice for PtyDevice {
        type Read = SerialReader;
        type Write = SerialWriter;

        fn label(&self) -> String {
            "pty".into()
        }

        async fn open(self) -> Result<(SerialReader, SerialWriter), RynkHostError> {
            let (reader, writer) = tokio::io::split(self.0);
            Ok((FromTokio::new(reader), FromTokio::new(writer)))
        }
    }

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

    /// Header + postcard payload, framed as the firmware sends it.
    fn frame<T: Serialize>(cmd: Cmd, seq: u8, value: &T) -> Vec<u8> {
        // Scratch large enough for any test frame.
        let mut buf = vec![0u8; 4096];
        let msg = RynkMessage::build(&mut buf, cmd, seq, value).unwrap();
        msg.frame().to_vec()
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
                tokio::io::AsyncWriteExt::write_all(
                    &mut peer,
                    &frame(cmd, seq, &Ok::<_, RynkError>(DeviceCapabilities::default())),
                )
                .await
                .unwrap();
            }
            peer
        })
    }

    #[tokio::test]
    async fn transport_round_trips_bytes() {
        let (mut peer, ours) = pty_pair();
        let (reader, writer) = tokio::io::split(ours);
        let (mut writer, mut reader) = (FromTokio::new(writer), FromTokio::new(reader));

        writer.write_all(&[1, 2, 3]).await.unwrap();
        let mut buf = [0u8; 3];
        peer.read_exact(&mut buf).await.unwrap();
        assert_eq!(buf, [1, 2, 3]);

        tokio::io::AsyncWriteExt::write_all(&mut peer, &[9, 8]).await.unwrap();
        let mut got = [0u8; 2];
        reader.read_exact(&mut got).await.unwrap();
        assert_eq!(got, [9, 8]);
    }

    #[tokio::test]
    async fn connect_handshakes_against_scripted_peer() {
        let (peer, ours) = pty_pair();
        let device = scripted_firmware(peer, ProtocolVersion::CURRENT);

        // Success proves the serial handshake round trip.
        PtyDevice(ours).connect().await.unwrap();
        device.await.unwrap();
    }

    #[tokio::test]
    async fn connect_times_out_on_silent_peer() {
        // A silent peer must remain pending; callers own the timeout.
        let (_peer, ours) = pty_pair();
        let timed_out = tokio::time::timeout(Duration::from_secs(1), PtyDevice(ours).connect()).await;
        assert!(timed_out.is_err(), "connect must not resolve against a silent peer");
    }
}
