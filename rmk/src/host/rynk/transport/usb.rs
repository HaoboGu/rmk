//! USB bulk transport for the Rynk service.
//!
//! Owns one BULK IN + one BULK OUT endpoint sitting on a vendor-specific
//! interface, plus the WinUSB MS OS 2.0 descriptors that let Windows bind
//! WinUSB automatically (so `rynk-cli` can talk to the device without a
//! `.inf` install).
//!
//! Framing follows [`Header`]: read packets into a buffer until
//! `5 + LEN` bytes are available, dispatch the frame, then keep any
//! remainder for the next iteration. ZLP termination is added after a
//! TX whose payload is an exact multiple of MPS so the host's URB
//! completes promptly.

use embassy_futures::select::{Either, select};
use embassy_usb::Builder;
use embassy_usb::driver::{Driver, Endpoint, EndpointError, EndpointIn, EndpointOut};
use embassy_usb::msos::{self, CompatibleIdFeatureDescriptor, PropertyData, RegistryPropertyFeatureDescriptor};
use rmk_types::protocol::rynk::header::HEADER_SIZE;

use super::super::codec::WireErr;
use super::super::topics::TopicSubscribers;
use super::super::{RYNK_BUFFER_SIZE, RynkService};

/// USB Vendor class code (per USB-IF). Combined with subclass+protocol = 0,
/// this is the magic triple Windows looks for to apply the WinUSB compat ID.
const VENDOR_CLASS: u8 = 0xFF;
const VENDOR_SUBCLASS: u8 = 0x00;
const VENDOR_PROTOCOL: u8 = 0x00;

/// MS OS 2.0 vendor code. Echoed in the `bMS_VendorCode` field of the BOS
/// descriptor; the OS uses it as the `bRequest` of a vendor control read
/// when fetching the descriptor set. Any nonzero byte works.
const MSOS_VENDOR_CODE: u8 = 0x01;

/// Stable GUID under which Windows publishes this device for user-space
/// enumeration via SetupAPI. `rynk-cli` filters on this GUID instead of
/// PID/VID so multiple keyboards can coexist.
///
/// Generated once with `uuidgen`; do not change without coordinating a
/// matching update in `rynk-host`.
const RYNK_DEVICE_GUID: &str = "{F5F5F5F5-1234-5678-9ABC-DEF012345678}";

/// USB bulk transport. Owns its bulk endpoints; built once via [`build`]
/// during USB device construction, then kept around for [`run`] to borrow.
pub struct RynkUsbTransport<'d, D: Driver<'d>> {
    bulk_in: D::EndpointIn,
    bulk_out: D::EndpointOut,
    max_packet_size: u16,
}

impl<'d, D: Driver<'d>> RynkUsbTransport<'d, D> {
    /// Add a vendor-class function with one BULK IN + one BULK OUT
    /// endpoint to `builder`, register the WinUSB MS OS 2.0 descriptor
    /// set so Windows binds WinUSB automatically, and return the
    /// transport bound to the resulting endpoints.
    ///
    /// `max_packet_size` should be 64 for FS, 512 for HS — pass whatever
    /// the underlying driver supports. Buffers in [`run`] are sized at
    /// `RYNK_BUFFER_SIZE`, independent of this.
    pub fn build(builder: &mut Builder<'d, D>, max_packet_size: u16) -> Self {
        // Register WinUSB at the device level so Windows attaches WinUSB
        // before binding the function — required for composite devices
        // because Windows otherwise asks the parent driver first.
        builder.msos_descriptor(msos::windows_version::WIN8_1, MSOS_VENDOR_CODE);

        let mut function = builder.function(VENDOR_CLASS, VENDOR_SUBCLASS, VENDOR_PROTOCOL);

        // Function-level MS OS 2.0 descriptors: declare WinUSB compat ID
        // and publish a stable interface GUID. Both must be added inside
        // the function block — embassy-usb panics otherwise.
        function.msos_feature(CompatibleIdFeatureDescriptor::new("WINUSB", ""));
        // `DeviceInterfaceGUIDs` is REG_MULTI_SZ: a list of GUIDs joined by
        // NUL terminators. embassy-usb encodes the slice into UTF-16LE and
        // adds the trailing terminator itself.
        function.msos_feature(RegistryPropertyFeatureDescriptor::new(
            "DeviceInterfaceGUIDs",
            PropertyData::RegMultiSz(&[RYNK_DEVICE_GUID]),
        ));

        let mut interface = function.interface();
        let mut alt = interface.alt_setting(VENDOR_CLASS, VENDOR_SUBCLASS, VENDOR_PROTOCOL, None);

        let bulk_in = alt.endpoint_bulk_in(None, max_packet_size);
        let bulk_out = alt.endpoint_bulk_out(None, max_packet_size);

        // `function` drops here, closing the function-level subset; this is
        // required for the MSOS writer to finalize its length prefix.
        drop(function);

        Self {
            bulk_in,
            bulk_out,
            max_packet_size,
        }
    }

    /// Drive the transport — reads frames, dispatches, writes responses,
    /// and forwards topic events. Joined into the main `run_all!()` chain
    /// by macro-generated code. Never returns under normal operation.
    pub async fn run(&mut self, service: &RynkService<'_>) -> ! {
        let mut rx_buf = [0u8; RYNK_BUFFER_SIZE];
        let mut tx_buf = [0u8; RYNK_BUFFER_SIZE];
        let mut topics = TopicSubscribers::new();

        loop {
            // wait_enabled returns once the host configures the interface;
            // a disconnect re-enters this branch via the inner `break`.
            self.bulk_out.wait_enabled().await;
            self.bulk_in.wait_enabled().await;

            let mut rx_used = 0usize;

            'session: loop {
                match select(
                    read_frame(&mut self.bulk_out, &mut rx_buf, &mut rx_used),
                    topics.next_event(),
                )
                .await
                {
                    Either::First(Ok(frame_len)) => {
                        let n = service.dispatch(&rx_buf[..frame_len], &mut tx_buf).await;
                        if n > 0
                            && write_frame(&mut self.bulk_in, &tx_buf[..n], self.max_packet_size)
                                .await
                                .is_err()
                        {
                            break 'session;
                        }
                        // Compact: keep any bytes belonging to the next frame.
                        rx_buf.copy_within(frame_len..rx_used, 0);
                        rx_used -= frame_len;
                    }
                    Either::First(Err(WireErr::ConnectionClosed)) => break 'session,
                    Either::First(Err(WireErr::Overflow)) => {
                        // Frame longer than buffer — caller's bug. Reset
                        // RX state and resync on the next host write so
                        // we don't get stuck mid-frame forever.
                        rx_used = 0;
                    }
                    Either::First(Err(WireErr::Io)) => break 'session,
                    Either::Second(event) => {
                        let n = event.encode(service, &mut tx_buf);
                        if n > 0
                            && write_frame(&mut self.bulk_in, &tx_buf[..n], self.max_packet_size)
                                .await
                                .is_err()
                        {
                            break 'session;
                        }
                    }
                }
            }
        }
    }
}

/// Read packets from `ep` into `buf`, accumulating until a full
/// `5 + LEN` frame is available, then return its length.
async fn read_frame<E: EndpointOut>(ep: &mut E, buf: &mut [u8], used: &mut usize) -> Result<usize, WireErr> {
    loop {
        // Try parsing a frame from what's already buffered. `LEN` is
        // authoritative — the buffer naturally holds at most one frame
        // in progress plus the start of the next.
        if *used >= HEADER_SIZE {
            let len = u16::from_le_bytes([buf[3], buf[4]]) as usize;
            let total = HEADER_SIZE + len;
            if total > buf.len() {
                return Err(WireErr::Overflow);
            }
            if *used >= total {
                return Ok(total);
            }
        }

        // Need more bytes — read into the unused tail.
        let tail = &mut buf[*used..];
        if tail.is_empty() {
            // Out of buffer space without having parsed a frame; the
            // caller (or a misbehaving host) is overrunning us.
            return Err(WireErr::Overflow);
        }
        let n = ep.read(tail).await.map_err(map_ep_err)?;
        *used += n;
    }
}

/// Send a frame in MPS-sized chunks. If the payload is an exact multiple
/// of `mps`, follow up with a zero-length packet so the host's URB
/// completes promptly — bulk-IN convention, transparent to the protocol.
async fn write_frame<E: EndpointIn>(ep: &mut E, frame: &[u8], mps: u16) -> Result<(), WireErr> {
    let mps = mps as usize;
    let mut offset = 0;
    while offset < frame.len() {
        let end = (offset + mps).min(frame.len());
        ep.write(&frame[offset..end]).await.map_err(map_ep_err)?;
        offset = end;
    }
    if frame.len() % mps == 0 {
        ep.write(&[]).await.map_err(map_ep_err)?;
    }
    Ok(())
}

fn map_ep_err(e: EndpointError) -> WireErr {
    match e {
        EndpointError::Disabled => WireErr::ConnectionClosed,
        EndpointError::BufferOverflow => WireErr::Overflow,
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    //! Reassembly is the only piece that's transport-shape-agnostic enough
    //! to test off-hardware. The driver / endpoint half is exercised by
    //! Phase 7's end-to-end host-tool walk.

    extern crate std;

    use embassy_futures::block_on;
    use embassy_usb::driver::{EndpointAddress, EndpointInfo, EndpointType};

    use super::*;

    /// Mock OUT endpoint that yields a fixed sequence of "packets" and
    /// then signals `Disabled`.
    struct MockOut {
        packets: std::vec::Vec<std::vec::Vec<u8>>,
        next: usize,
    }

    impl Endpoint for MockOut {
        fn info(&self) -> &EndpointInfo {
            // `read_frame` never calls `info()`. `EndpointAddress` lacks a
            // const ctor, so initialize on first call into a leaked Box.
            use std::sync::OnceLock;
            static INFO: OnceLock<&'static EndpointInfo> = OnceLock::new();
            INFO.get_or_init(|| {
                Box::leak(Box::new(EndpointInfo {
                    addr: EndpointAddress::from(0u8),
                    ep_type: EndpointType::Bulk,
                    max_packet_size: 64,
                    interval_ms: 0,
                }))
            })
        }

        async fn wait_enabled(&mut self) {}
    }

    impl EndpointOut for MockOut {
        async fn read(&mut self, buf: &mut [u8]) -> Result<usize, EndpointError> {
            if self.next >= self.packets.len() {
                return Err(EndpointError::Disabled);
            }
            let pkt = &self.packets[self.next];
            self.next += 1;
            buf[..pkt.len()].copy_from_slice(pkt);
            Ok(pkt.len())
        }
    }

    fn frame(payload: &[u8]) -> std::vec::Vec<u8> {
        let len = payload.len() as u16;
        let mut v = std::vec![0xCD, 0xAB, 0x42, len as u8, (len >> 8) as u8];
        v.extend_from_slice(payload);
        v
    }

    #[test]
    fn read_frame_assembles_single_packet() {
        let f = frame(&[1, 2, 3]);
        let mut ep = MockOut {
            packets: std::vec![f.clone()],
            next: 0,
        };
        let mut buf = [0u8; 64];
        let mut used = 0;
        let n = block_on(read_frame(&mut ep, &mut buf, &mut used)).unwrap();
        assert_eq!(n, f.len());
        assert_eq!(&buf[..n], &f[..]);
    }

    #[test]
    fn read_frame_assembles_across_packets() {
        let f = frame(&[1, 2, 3, 4, 5, 6, 7, 8]);
        let (a, b) = f.split_at(4);
        let mut ep = MockOut {
            packets: std::vec![a.to_vec(), b.to_vec()],
            next: 0,
        };
        let mut buf = [0u8; 64];
        let mut used = 0;
        let n = block_on(read_frame(&mut ep, &mut buf, &mut used)).unwrap();
        assert_eq!(n, f.len());
        assert_eq!(&buf[..n], &f[..]);
    }

    #[test]
    fn read_frame_keeps_trailing_bytes_for_next_call() {
        let f1 = frame(&[1, 2]);
        let f2 = frame(&[9]);
        let combined = [f1.as_slice(), f2.as_slice()].concat();
        let mut ep = MockOut {
            packets: std::vec![combined.clone()],
            next: 0,
        };
        let mut buf = [0u8; 64];
        let mut used = 0;
        let n = block_on(read_frame(&mut ep, &mut buf, &mut used)).unwrap();
        assert_eq!(n, f1.len());
        // Caller compacts: simulate that.
        buf.copy_within(n..used, 0);
        used -= n;
        let n2 = block_on(read_frame(&mut ep, &mut buf, &mut used)).unwrap();
        assert_eq!(n2, f2.len());
        assert_eq!(&buf[..n2], f2.as_slice());
    }

    #[test]
    fn read_frame_rejects_oversize() {
        let mut header = std::vec![0; HEADER_SIZE];
        header[3] = 0xFF;
        header[4] = 0xFF;
        let mut ep = MockOut {
            packets: std::vec![header],
            next: 0,
        };
        let mut buf = [0u8; 64];
        let mut used = 0;
        let err = block_on(read_frame(&mut ep, &mut buf, &mut used)).unwrap_err();
        assert_eq!(err, WireErr::Overflow);
    }
}
