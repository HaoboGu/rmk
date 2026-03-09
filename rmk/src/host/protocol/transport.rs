use core::fmt::{Arguments, Write};

use embassy_futures::select::{Either, select};
use embassy_sync::mutex::Mutex;
use embassy_time::Timer;
use embassy_usb::driver::{Driver, Endpoint, EndpointError, EndpointIn, EndpointOut};
use embassy_usb::{Builder, msos};
use postcard_rpc::header::{VarHeader, VarKey, VarKeyKind, VarSeq};
use postcard_rpc::server::{WireRx, WireRxErrorKind, WireTx, WireTxErrorKind};
use postcard_rpc::standard_icd::LoggingTopic;
use postcard_rpc::Topic;
use serde::Serialize;

use crate::RawMutex;

pub(crate) const USB_BULK_PACKET_SIZE: usize = 64;
pub(crate) const TX_BUF_SIZE: usize = 512;
const TX_TIMEOUT_MS_PER_FRAME: usize = 10;
const RMK_WINUSB_GUIDS: &[&str] = &["{533E7A32-4C6B-49F8-8C5B-60D2D784F2C6}"];

pub(crate) struct UsbBulkTxState<'d, D: Driver<'d>> {
    ep_in: D::EndpointIn,
    log_seq: u16,
    tx_buf: [u8; TX_BUF_SIZE],
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
}

impl<'a, 'd, D: Driver<'d>> UsbBulkTx<'a, 'd, D> {
    pub(crate) fn new(inner: &'a Mutex<RawMutex, UsbBulkTxState<'d, D>>) -> Self {
        Self { inner }
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
}

impl<'a, 'd, D: Driver<'d>> UsbBulkRx<'a, 'd, D> {
    pub(crate) fn new(ep_out: &'a mut D::EndpointOut) -> Self {
        Self { ep_out }
    }
}

pub(crate) fn add_usb_bulk_interface<'d, D: Driver<'d>>(builder: &mut Builder<'d, D>) -> (D::EndpointIn, D::EndpointOut) {
    builder.msos_descriptor(msos::windows_version::WIN8_1, 0);

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
    drop(function);

    (ep_in, ep_out)
}

impl<'d, D: Driver<'d>> WireRx for UsbBulkRx<'_, 'd, D> {
    type Error = WireRxErrorKind;

    async fn wait_connection(&mut self) {
        self.ep_out.wait_enabled().await;
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

        // Buffer full — drain remaining packets without overwriting received data
        let mut drain = [0u8; USB_BULK_PACKET_SIZE];
        loop {
            match self.ep_out.read(&mut drain).await {
                Ok(n) if n == USB_BULK_PACKET_SIZE => {}
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
        // NOTE: This holds the mutex for the duration of `wait_enabled()`.
        // This is safe because `wait_connection` is only called before the
        // dispatch loop starts (no concurrent senders). If topic publishers
        // are added later, this must be revisited (e.g., use a separate Signal).
        let mut inner = self.inner.lock().await;
        inner.ep_in.wait_enabled().await;
    }

    async fn send<T: Serialize + ?Sized>(&self, hdr: VarHeader, msg: &T) -> Result<(), Self::Error> {
        let mut inner = self.inner.lock().await;
        let (hdr_used, remain) = hdr.write_to_slice(&mut inner.tx_buf).ok_or(WireTxErrorKind::Other)?;
        let bdy_used = postcard::to_slice(msg, remain).map_err(|_| WireTxErrorKind::Other)?;
        let used = hdr_used.len() + bdy_used.len();
        let state = &mut *inner;
        send_buf(&mut state.ep_in, &mut state.pending_frame, &state.tx_buf[..used]).await
    }

    async fn send_raw(&self, buf: &[u8]) -> Result<(), Self::Error> {
        let mut inner = self.inner.lock().await;
        let state = &mut *inner;
        send_buf(&mut state.ep_in, &mut state.pending_frame, buf).await
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
        send_buf(&mut state.ep_in, &mut state.pending_frame, &state.tx_buf[..used]).await
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
        writer.write_fmt(args).map_err(|_| WireTxErrorKind::Other)?;
        let body_len = writer.len();

        // Encode the varint length prefix (LEB128).
        let varint_len = encode_varint_usize(body_len, &mut remain[..MAX_VARINT]);

        // Shift body to be contiguous with varint if varint used fewer than MAX_VARINT bytes.
        let gap = MAX_VARINT - varint_len;
        if gap > 0 {
            remain.copy_within(MAX_VARINT..MAX_VARINT + body_len, varint_len);
        }

        let used = hdr_used.len() + varint_len + body_len;
        let state = &mut *inner;
        send_buf(&mut state.ep_in, &mut state.pending_frame, &state.tx_buf[..used]).await
    }
}

/// Encode a usize as a postcard varint (LEB128) into the buffer.
/// Returns the number of bytes written.
fn encode_varint_usize(mut value: usize, buf: &mut [u8]) -> usize {
    let mut i = 0;
    loop {
        if i >= buf.len() {
            return i;
        }
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

async fn send_buf(
    ep_in: &mut impl EndpointIn,
    pending_frame: &mut bool,
    out: &[u8],
) -> Result<(), WireTxErrorKind> {
    if out.is_empty() {
        return Ok(());
    }

    let frames = out.len().div_ceil(USB_BULK_PACKET_SIZE);
    let timeout_ms = frames * TX_TIMEOUT_MS_PER_FRAME;

    let send_fut = async {
        if *pending_frame && ep_in.write(&[]).await.is_err() {
            return Err(WireTxErrorKind::ConnectionClosed);
        }
        *pending_frame = true;

        for chunk in out.chunks(USB_BULK_PACKET_SIZE) {
            if ep_in.write(chunk).await.is_err() {
                return Err(WireTxErrorKind::ConnectionClosed);
            }
        }

        if out.len() % USB_BULK_PACKET_SIZE == 0 && ep_in.write(&[]).await.is_err() {
            return Err(WireTxErrorKind::ConnectionClosed);
        }

        *pending_frame = false;
        Ok(())
    };

    match select(send_fut, Timer::after_millis(timeout_ms as u64)).await {
        Either::First(res) => res,
        Either::Second(()) => {
            // Keep pending_frame=true: partial data may have been written to the
            // endpoint, so the next send_buf call will emit a ZLP first to
            // cleanly terminate the aborted transfer before starting a new frame.
            Err(WireTxErrorKind::Timeout)
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
        let end = self.used.checked_add(s.len()).ok_or(core::fmt::Error)?;
        if end > self.buf.len() {
            return Err(core::fmt::Error);
        }
        self.buf[self.used..end].copy_from_slice(s.as_bytes());
        self.used = end;
        Ok(())
    }
}
