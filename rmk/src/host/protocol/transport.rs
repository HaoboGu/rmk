use core::fmt::{Arguments, Write};

use embassy_futures::select::{Either, select};
use embassy_sync::mutex::Mutex;
use embassy_sync::signal::Signal;
use embassy_time::Timer;
use embassy_usb::driver::{Driver, Endpoint, EndpointError, EndpointIn, EndpointOut};
use embassy_usb::{Builder, msos};
use postcard_rpc::Topic;
use postcard_rpc::header::{VarHeader, VarKey, VarKeyKind, VarSeq};
use postcard_rpc::server::{WireRx, WireRxErrorKind, WireTx, WireTxErrorKind};
use postcard_rpc::standard_icd::LoggingTopic;
use serde::Serialize;

use crate::RawMutex;

pub(crate) const USB_BULK_PACKET_SIZE: usize = 64;
// Largest non-schema response is BulkKeyActions (~362 bytes).
// 512 provides comfortable headroom for all normal endpoint responses.
pub(crate) const TX_BUF_SIZE: usize = 512;
// Per-packet timeout acts as a watchdog, not a hard deadline.
// 50ms per packet gives ~1.6s for a full 2048-byte frame, which is generous
// enough to accommodate scheduling jitter on resource-constrained MCUs.
const TX_TIMEOUT_MS_PER_PACKET: usize = 50;
const RMK_WINUSB_GUIDS: &[&str] = &["{533E7A32-4C6B-49F8-8C5B-60D2D784F2C6}"];

pub(crate) struct UsbBulkTxState<'d, D: Driver<'d>> {
    ep_in: D::EndpointIn,
    log_seq: u16,
    tx_buf: [u8; TX_BUF_SIZE],
    /// True when a previous send was interrupted mid-frame (e.g. by timeout).
    /// The next send must emit a ZLP first to cleanly terminate the aborted
    /// transfer so the host can detect the frame boundary.
    pending_frame: bool,
}

impl<'d, D: Driver<'d>> UsbBulkTxState<'d, D> {
    pub(crate) fn new(ep_in: D::EndpointIn) -> Self {
        Self {
            ep_in,
            log_seq: 0,
            tx_buf: [0; TX_BUF_SIZE],
            pending_frame: false,
        }
    }
}

pub(crate) struct UsbBulkTx<'a, 'd, D: Driver<'d>> {
    inner: &'a Mutex<RawMutex, UsbBulkTxState<'d, D>>,
    connected: &'a Signal<RawMutex, ()>,
}

impl<'a, 'd, D: Driver<'d>> UsbBulkTx<'a, 'd, D> {
    pub(crate) fn new(inner: &'a Mutex<RawMutex, UsbBulkTxState<'d, D>>, connected: &'a Signal<RawMutex, ()>) -> Self {
        Self { inner, connected }
    }
}

impl<'d, D: Driver<'d>> Clone for UsbBulkTx<'_, 'd, D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'d, D: Driver<'d>> Copy for UsbBulkTx<'_, 'd, D> {}

pub(crate) struct UsbBulkRx<'a, 'd, D: Driver<'d>> {
    ep_out: &'a mut D::EndpointOut,
    tx_connected: &'a Signal<RawMutex, ()>,
}

impl<'a, 'd, D: Driver<'d>> UsbBulkRx<'a, 'd, D> {
    pub(crate) fn new(ep_out: &'a mut D::EndpointOut, tx_connected: &'a Signal<RawMutex, ()>) -> Self {
        Self { ep_out, tx_connected }
    }
}

pub(crate) fn add_usb_bulk_interface<'d, D: Driver<'d>>(
    builder: &mut Builder<'d, D>,
) -> (D::EndpointIn, D::EndpointOut) {
    // Vendor code 1 (non-zero to avoid conflicts with standard USB requests on older Windows)
    builder.msos_descriptor(msos::windows_version::WIN8_1, 1);

    let mut function = builder.function(0xFF, 0, 0);
    function.msos_feature(msos::CompatibleIdFeatureDescriptor::new("WINUSB", ""));
    function.msos_feature(msos::RegistryPropertyFeatureDescriptor::new(
        "DeviceInterfaceGUIDs",
        msos::PropertyData::RegMultiSz(RMK_WINUSB_GUIDS),
    ));

    let mut interface = function.interface();
    let mut alt = interface.alt_setting(0xFF, 0, 0, None);
    let ep_out = alt.endpoint_bulk_out(None, USB_BULK_PACKET_SIZE as u16);
    let ep_in = alt.endpoint_bulk_in(None, USB_BULK_PACKET_SIZE as u16);
    (ep_in, ep_out)
}

impl<'d, D: Driver<'d>> WireRx for UsbBulkRx<'_, 'd, D> {
    type Error = WireRxErrorKind;

    async fn wait_connection(&mut self) {
        // Clear any stale signal from a previous connection cycle
        self.tx_connected.reset();
        self.ep_out.wait_enabled().await;
        // Signal the TX side that the connection is ready
        self.tx_connected.signal(());
    }

    async fn receive<'a>(&mut self, buf: &'a mut [u8]) -> Result<&'a mut [u8], Self::Error> {
        let buflen = buf.len();
        let mut window = &mut buf[..];

        while !window.is_empty() {
            let n = match self.ep_out.read(window).await {
                Ok(n) => n,
                Err(EndpointError::BufferOverflow) => return Err(WireRxErrorKind::ReceivedMessageTooLarge),
                Err(EndpointError::Disabled) => return Err(WireRxErrorKind::ConnectionClosed),
            };

            let (_, later) = window.split_at_mut(n);
            window = later;

            if n != USB_BULK_PACKET_SIZE {
                let len = buflen - window.len();
                return Ok(&mut buf[..len]);
            }
        }

        // Buffer full — drain remaining packets without overwriting received data.
        // If the next read returns a ZLP (0 bytes) or a short packet, the transfer
        // is complete. The frame is valid ONLY if no extra full packets were drained
        // (i.e., the frame was exactly buffer-sized and terminated by a ZLP).
        let mut drain = [0u8; USB_BULK_PACKET_SIZE];
        let mut drained_excess = false;
        loop {
            match self.ep_out.read(&mut drain).await {
                Ok(0) => {
                    // ZLP terminates the transfer. If we drained extra full
                    // packets before this ZLP, the frame exceeded the buffer.
                    if drained_excess {
                        return Err(WireRxErrorKind::ReceivedMessageTooLarge);
                    }
                    return Ok(buf);
                }
                Ok(n) if n == USB_BULK_PACKET_SIZE => {
                    drained_excess = true; // extra full packet beyond buffer
                }
                Ok(_) => return Err(WireRxErrorKind::ReceivedMessageTooLarge),
                Err(EndpointError::BufferOverflow) => return Err(WireRxErrorKind::ReceivedMessageTooLarge),
                Err(EndpointError::Disabled) => return Err(WireRxErrorKind::ConnectionClosed),
            }
        }
    }
}

impl<'d, D: Driver<'d>> WireTx for UsbBulkTx<'_, 'd, D> {
    type Error = WireTxErrorKind;

    async fn wait_connection(&self) {
        // Wait for the connection signal from the RX side without holding the
        // TX mutex. The signal is set by UsbBulkRx::wait_connection after
        // ep_out becomes enabled, so topic publishers won't deadlock.
        self.connected.wait().await;

        // After reconnection, lock the mutex to reset endpoint state.
        // ep_in.wait_enabled() ensures the hardware endpoint is ready
        // (it returns immediately if already enabled — the signal guarantees
        // USB is up). Clearing pending_frame prevents a stale ZLP.
        let mut inner = self.inner.lock().await;
        inner.ep_in.wait_enabled().await;
        inner.pending_frame = false;
    }

    async fn send<T: Serialize + ?Sized>(&self, hdr: VarHeader, msg: &T) -> Result<(), Self::Error> {
        let mut inner = self.inner.lock().await;
        let (hdr_used, remain) = match hdr.write_to_slice(&mut inner.tx_buf) {
            Some(v) => v,
            None => {
                warn!("TX header serialization failed (buffer too small)");
                return Err(WireTxErrorKind::Other);
            }
        };
        let bdy_used = match postcard::to_slice(msg, remain) {
            Ok(v) => v,
            Err(_) => {
                warn!("TX body serialization failed (buffer too small)");
                return Err(WireTxErrorKind::Other);
            }
        };
        let used = hdr_used.len() + bdy_used.len();
        let state = &mut *inner;
        send_buf(&mut state.ep_in, &state.tx_buf[..used], &mut state.pending_frame).await
    }

    async fn send_raw(&self, buf: &[u8]) -> Result<(), Self::Error> {
        let mut inner = self.inner.lock().await;
        let state = &mut *inner;
        send_buf(&mut state.ep_in, buf, &mut state.pending_frame).await
    }

    async fn send_log_str(&self, kkind: VarKeyKind, s: &str) -> Result<(), Self::Error> {
        let mut inner = self.inner.lock().await;
        let key = logging_key(kkind);
        let seq = inner.log_seq;
        inner.log_seq = inner.log_seq.wrapping_add(1);
        let hdr = VarHeader {
            key,
            seq_no: VarSeq::Seq2(seq),
        };
        let (hdr_used, remain) = hdr.write_to_slice(&mut inner.tx_buf).ok_or(WireTxErrorKind::Other)?;
        let bdy_used = postcard::to_slice::<str>(s, remain).map_err(|_| WireTxErrorKind::Other)?;
        let used = hdr_used.len() + bdy_used.len();
        let state = &mut *inner;
        send_buf(&mut state.ep_in, &state.tx_buf[..used], &mut state.pending_frame).await
    }

    async fn send_log_fmt<'a>(&self, kkind: VarKeyKind, args: Arguments<'a>) -> Result<(), Self::Error> {
        let mut inner = self.inner.lock().await;
        let key = logging_key(kkind);
        let seq = inner.log_seq;
        inner.log_seq = inner.log_seq.wrapping_add(1);
        let hdr = VarHeader {
            key,
            seq_no: VarSeq::Seq2(seq),
        };
        let (hdr_used, remain) = hdr.write_to_slice(&mut inner.tx_buf).ok_or(WireTxErrorKind::Other)?;

        // Reserve max varint space (5 bytes), then format the string body after it.
        // Postcard serializes `str` as varint-length + raw UTF-8 bytes.
        const MAX_VARINT: usize = 5;
        if remain.len() <= MAX_VARINT {
            return Err(WireTxErrorKind::Other);
        }
        let mut writer = SliceWriter::new(&mut remain[MAX_VARINT..]);
        let overflow = writer.write_fmt(args).is_err();
        let mut body_len = writer.len();

        // If truncated, append "..." at a UTF-8 char boundary (scan backwards
        // to avoid splitting multi-byte characters).
        if overflow && body_len >= 3 {
            let body = &remain[MAX_VARINT..MAX_VARINT + body_len];
            let mut trunc = body_len - 3;
            while trunc > 0 && (body[trunc] & 0xC0) == 0x80 {
                trunc -= 1;
            }
            if trunc == 0 && !body.is_empty() && (body[0] & 0xC0) == 0x80 {
                // No valid char boundary found; replace entire body with "..."
                remain[MAX_VARINT..MAX_VARINT + 3].copy_from_slice(b"...");
                body_len = 3;
            } else {
                remain[MAX_VARINT + trunc..MAX_VARINT + trunc + 3].copy_from_slice(b"...");
                body_len = trunc + 3;
            }
        }

        // Encode the varint length prefix (LEB128).
        let varint_len = encode_varint_usize(body_len, &mut remain[..MAX_VARINT]);

        // Shift body to be contiguous with varint if varint used fewer than MAX_VARINT bytes.
        let gap = MAX_VARINT - varint_len;
        if gap > 0 {
            remain.copy_within(MAX_VARINT..MAX_VARINT + body_len, varint_len);
        }

        let used = hdr_used.len() + varint_len + body_len;
        let state = &mut *inner;
        send_buf(&mut state.ep_in, &state.tx_buf[..used], &mut state.pending_frame).await
    }
}

/// Encode a usize as a postcard varint (LEB128) into `buf`.
/// Returns the number of bytes written. Caller must provide at least 5 bytes.
fn encode_varint_usize(mut value: usize, buf: &mut [u8]) -> usize {
    let mut i = 0;
    loop {
        debug_assert!(
            i < buf.len(),
            "varint buffer overflow: value needs more than {} bytes",
            buf.len()
        );
        if value < 0x80 {
            buf[i] = value as u8;
            return i + 1;
        }
        buf[i] = (value as u8) | 0x80;
        value >>= 7;
        i += 1;
    }
}

fn logging_key(kkind: VarKeyKind) -> VarKey {
    match kkind {
        VarKeyKind::Key1 => VarKey::Key1(LoggingTopic::TOPIC_KEY1),
        VarKeyKind::Key2 => VarKey::Key2(LoggingTopic::TOPIC_KEY2),
        VarKeyKind::Key4 => VarKey::Key4(LoggingTopic::TOPIC_KEY4),
        VarKeyKind::Key8 => VarKey::Key8(LoggingTopic::TOPIC_KEY),
    }
}

async fn send_buf(ep_in: &mut impl EndpointIn, out: &[u8], pending_frame: &mut bool) -> Result<(), WireTxErrorKind> {
    // If a previous send was interrupted mid-frame, send a ZLP to cleanly
    // terminate it so the host can detect the frame boundary.
    if *pending_frame {
        if ep_in.write(&[]).await.is_err() {
            return Err(WireTxErrorKind::ConnectionClosed);
        }
        *pending_frame = false;
    }

    if out.is_empty() {
        return Ok(());
    }

    *pending_frame = true;

    let frames = out.len().div_ceil(USB_BULK_PACKET_SIZE);
    // Minimum 100ms timeout to account for scheduling jitter + possible ZLP
    let timeout_ms = (frames * TX_TIMEOUT_MS_PER_PACKET).max(100);

    let send_fut = async {
        for chunk in out.chunks(USB_BULK_PACKET_SIZE) {
            if ep_in.write(chunk).await.is_err() {
                return Err(WireTxErrorKind::ConnectionClosed);
            }
        }

        if out.len() % USB_BULK_PACKET_SIZE == 0 && ep_in.write(&[]).await.is_err() {
            return Err(WireTxErrorKind::ConnectionClosed);
        }

        Ok(())
    };

    match select(send_fut, Timer::after_millis(timeout_ms as u64)).await {
        Either::First(Ok(())) => {
            *pending_frame = false;
            Ok(())
        }
        Either::First(Err(e)) => Err(e),
        Either::Second(()) => {
            // Embassy-usb endpoint writes are NOT cancel-safe. pending_frame
            // stays true so the next send emits a ZLP to terminate the aborted
            // transfer. Returning ConnectionClosed breaks the dispatch loop,
            // which re-enters wait_connection() and waits for a USB bus reset.
            Err(WireTxErrorKind::ConnectionClosed)
        }
    }
}

struct SliceWriter<'a> {
    buf: &'a mut [u8],
    used: usize,
}

impl<'a> SliceWriter<'a> {
    fn new(buf: &'a mut [u8]) -> Self {
        Self { buf, used: 0 }
    }

    fn len(&self) -> usize {
        self.used
    }
}

impl Write for SliceWriter<'_> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let remaining = self.buf.len() - self.used;
        let mut to_write = s.len().min(remaining);
        // Don't split a multi-byte UTF-8 character at the buffer boundary.
        while to_write > 0 && !s.is_char_boundary(to_write) {
            to_write -= 1;
        }
        if to_write > 0 {
            self.buf[self.used..self.used + to_write].copy_from_slice(&s.as_bytes()[..to_write]);
            self.used += to_write;
        }
        if to_write < s.len() {
            Err(core::fmt::Error)
        } else {
            Ok(())
        }
    }
}
