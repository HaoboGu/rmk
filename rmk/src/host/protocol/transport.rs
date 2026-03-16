use core::fmt::{Arguments, Write};

use embassy_futures::select::{Either, select};
use embassy_sync::channel::Channel;
use embassy_sync::mutex::Mutex;
use embassy_sync::signal::Signal;
use embassy_time::Timer;
use embassy_usb::driver::{Driver, Endpoint, EndpointError, EndpointIn, EndpointOut};
use embassy_usb::{Builder, msos};
use postcard_rpc::Topic;
use postcard_rpc::header::{VarHeader, VarKey, VarKeyKind, VarSeq};
use postcard_rpc::server::{AsWireTxErrorKind, WireRx, WireRxErrorKind, WireTx, WireTxErrorKind};
use postcard_rpc::standard_icd::LoggingTopic;
use serde::Serialize;

use crate::RawMutex;

pub(crate) const USB_BULK_PACKET_SIZE: usize = 64;
// MAX_BULK=512 keys × ~2 bytes avg serialized = ~1040B; 1024 fits typical keymap responses.
pub(crate) const TX_BUF_SIZE: usize = 1024;
/// Depth of the TX frame queue used by `QueuingTx` to decouple dispatch from USB writes.
pub(crate) const TX_QUEUE_DEPTH: usize = 4;
// Per-packet timeout acts as a watchdog, not a hard deadline.
// 50ms per packet gives ~1.6s for a full 2048-byte frame, which is generous
// enough to accommodate scheduling jitter on resource-constrained MCUs.
const TX_TIMEOUT_MS_PER_PACKET: usize = 50;

/// A pre-serialized frame ready to be written to the USB endpoint.
pub(crate) struct TxFrame {
    pub buf: [u8; TX_BUF_SIZE],
    pub len: usize,
}

/// A `WireTx` implementation that serializes responses into [`TxFrame`]s and
/// enqueues them on a bounded channel, decoupling the dispatch loop from USB
/// write latency.
///
/// The inner `Tx` is still used directly for `send_raw`, `send_log_str`, and
/// `send_log_fmt` (these are small/infrequent and bypass the queue).
#[derive(Clone, Copy)]
pub(crate) struct QueuingTx<'a, Tx: Copy> {
    inner: Tx,
    channel: &'a Channel<RawMutex, TxFrame, TX_QUEUE_DEPTH>,
}

impl<'a, Tx: Copy> QueuingTx<'a, Tx> {
    pub(crate) fn new(inner: Tx, channel: &'a Channel<RawMutex, TxFrame, TX_QUEUE_DEPTH>) -> Self {
        Self { inner, channel }
    }
}

impl<Tx: WireTx + Copy> WireTx for QueuingTx<'_, Tx> {
    type Error = WireTxErrorKind;

    async fn send<T: Serialize + ?Sized>(&self, hdr: VarHeader, msg: &T) -> Result<(), Self::Error> {
        let mut frame = TxFrame {
            buf: [0u8; TX_BUF_SIZE],
            len: 0,
        };
        let (hdr_used, remain) = hdr.write_to_slice(&mut frame.buf).ok_or_else(|| {
            warn!("[qtx] header too large for frame buffer");
            WireTxErrorKind::Other
        })?;
        let bdy_used = postcard::to_slice(msg, remain).map_err(|_| {
            warn!("[qtx] body serialization failed (buffer too small)");
            WireTxErrorKind::Other
        })?;
        frame.len = hdr_used.len() + bdy_used.len();
        self.channel.send(frame).await;
        Ok(())
    }

    async fn wait_connection(&self) {
        self.inner.wait_connection().await
    }

    async fn send_raw(&self, buf: &[u8]) -> Result<(), Self::Error> {
        self.inner.send_raw(buf).await.map_err(|e| e.as_kind())
    }

    async fn send_log_str(&self, kkind: VarKeyKind, s: &str) -> Result<(), Self::Error> {
        self.inner.send_log_str(kkind, s).await.map_err(|e| e.as_kind())
    }

    async fn send_log_fmt<'b>(&self, kkind: VarKeyKind, args: Arguments<'b>) -> Result<(), Self::Error> {
        self.inner.send_log_fmt(kkind, args).await.map_err(|e| e.as_kind())
    }
}
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

        // After reconnection, lock the mutex and wait until the IN endpoint is
        // ready. This path also runs after TX timeouts, where the endpoint may
        // still be enabled; pending_frame must survive so the next send can
        // terminate the aborted frame with a cleanup ZLP.
        let mut inner = self.inner.lock().await;
        inner.ep_in.wait_enabled().await;
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
        warn!("[tx] cleanup ZLP for pending_frame");
        if ep_in.write(&[]).await.is_err() {
            warn!("[tx] cleanup ZLP failed (disconnected)");
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
    debug!("[tx] sending {}B ({}pkts, {}ms timeout)", out.len(), frames, timeout_ms);

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
        Either::First(Err(e)) => {
            warn!("[tx] send failed (disconnected)");
            Err(e)
        }
        Either::Second(()) => {
            // Embassy-usb endpoint writes are NOT cancel-safe. pending_frame
            // stays true so the next send emits a ZLP to terminate the aborted
            // transfer. Return Timeout (not ConnectionClosed) so the dispatch
            // loop can continue — the cleanup ZLP on the next send will
            // recover the USB state. If the endpoint is truly disconnected,
            // the ZLP write will fail with ConnectionClosed instead.
            warn!("[tx] TIMEOUT after {}ms ({}B, {}pkts)", timeout_ms, out.len(), frames);
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

#[cfg(all(test, feature = "std"))]
mod tests {
    use core::future::pending;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex as StdMutex};

    use embassy_futures::block_on;
    use embassy_usb::driver::{
        Bus, ControlPipe, Direction, EndpointAddress, EndpointAllocError, EndpointInfo, EndpointType, Event,
        Unsupported,
    };

    use super::*;
    use crate::RawMutex;

    #[derive(Clone, Copy)]
    enum WriteBehavior {
        Ok,
        PendingForever,
        Disabled,
    }

    #[derive(Default)]
    struct FakeEndpointState {
        writes: Vec<Vec<u8>>,
        wait_enabled_calls: usize,
        write_behaviors: VecDeque<WriteBehavior>,
    }

    #[derive(Clone)]
    struct FakeEndpointIn {
        info: EndpointInfo,
        state: Arc<StdMutex<FakeEndpointState>>,
    }

    impl FakeEndpointIn {
        fn new(write_behaviors: impl IntoIterator<Item = WriteBehavior>) -> Self {
            let mut state = FakeEndpointState::default();
            state.write_behaviors.extend(write_behaviors);
            Self {
                info: EndpointInfo {
                    addr: EndpointAddress::from_parts(1, Direction::In),
                    ep_type: EndpointType::Bulk,
                    max_packet_size: USB_BULK_PACKET_SIZE as u16,
                    interval_ms: 0,
                },
                state: Arc::new(StdMutex::new(state)),
            }
        }

        fn writes(&self) -> Vec<Vec<u8>> {
            self.state.lock().unwrap().writes.clone()
        }

        fn wait_enabled_calls(&self) -> usize {
            self.state.lock().unwrap().wait_enabled_calls
        }
    }

    impl Endpoint for FakeEndpointIn {
        fn info(&self) -> &EndpointInfo {
            &self.info
        }

        async fn wait_enabled(&mut self) {
            self.state.lock().unwrap().wait_enabled_calls += 1;
        }
    }

    impl EndpointIn for FakeEndpointIn {
        async fn write(&mut self, buf: &[u8]) -> Result<(), EndpointError> {
            let behavior = {
                let mut state = self.state.lock().unwrap();
                state.writes.push(buf.to_vec());
                state.write_behaviors.pop_front().unwrap_or(WriteBehavior::Ok)
            };

            match behavior {
                WriteBehavior::Ok => Ok(()),
                WriteBehavior::PendingForever => pending::<Result<(), EndpointError>>().await,
                WriteBehavior::Disabled => Err(EndpointError::Disabled),
            }
        }
    }

    struct FakeEndpointOut {
        info: EndpointInfo,
    }

    impl Default for FakeEndpointOut {
        fn default() -> Self {
            Self {
                info: EndpointInfo {
                    addr: EndpointAddress::from_parts(1, Direction::Out),
                    ep_type: EndpointType::Bulk,
                    max_packet_size: USB_BULK_PACKET_SIZE as u16,
                    interval_ms: 0,
                },
            }
        }
    }

    impl Endpoint for FakeEndpointOut {
        fn info(&self) -> &EndpointInfo {
            &self.info
        }

        async fn wait_enabled(&mut self) {
            panic!("FakeEndpointOut::wait_enabled should not be called in transport tests");
        }
    }

    impl EndpointOut for FakeEndpointOut {
        async fn read(&mut self, _buf: &mut [u8]) -> Result<usize, EndpointError> {
            panic!("FakeEndpointOut::read should not be called in transport tests");
        }
    }

    struct FakeControlPipe;

    impl ControlPipe for FakeControlPipe {
        fn max_packet_size(&self) -> usize {
            USB_BULK_PACKET_SIZE
        }

        async fn setup(&mut self) -> [u8; 8] {
            panic!("FakeControlPipe::setup should not be called in transport tests");
        }

        async fn data_out(&mut self, _buf: &mut [u8], _first: bool, _last: bool) -> Result<usize, EndpointError> {
            panic!("FakeControlPipe::data_out should not be called in transport tests");
        }

        async fn data_in(&mut self, _data: &[u8], _first: bool, _last: bool) -> Result<(), EndpointError> {
            panic!("FakeControlPipe::data_in should not be called in transport tests");
        }

        async fn accept(&mut self) {
            panic!("FakeControlPipe::accept should not be called in transport tests");
        }

        async fn reject(&mut self) {
            panic!("FakeControlPipe::reject should not be called in transport tests");
        }

        async fn accept_set_address(&mut self, _addr: u8) {
            panic!("FakeControlPipe::accept_set_address should not be called in transport tests");
        }
    }

    struct FakeBus;

    impl Bus for FakeBus {
        async fn enable(&mut self) {
            panic!("FakeBus::enable should not be called in transport tests");
        }

        async fn disable(&mut self) {
            panic!("FakeBus::disable should not be called in transport tests");
        }

        async fn poll(&mut self) -> Event {
            panic!("FakeBus::poll should not be called in transport tests");
        }

        fn endpoint_set_enabled(&mut self, _ep_addr: EndpointAddress, _enabled: bool) {
            panic!("FakeBus::endpoint_set_enabled should not be called in transport tests");
        }

        fn endpoint_set_stalled(&mut self, _ep_addr: EndpointAddress, _stalled: bool) {
            panic!("FakeBus::endpoint_set_stalled should not be called in transport tests");
        }

        fn endpoint_is_stalled(&mut self, _ep_addr: EndpointAddress) -> bool {
            panic!("FakeBus::endpoint_is_stalled should not be called in transport tests");
        }

        async fn remote_wakeup(&mut self) -> Result<(), Unsupported> {
            panic!("FakeBus::remote_wakeup should not be called in transport tests");
        }
    }

    struct FakeDriver;

    impl<'a> Driver<'a> for FakeDriver {
        type EndpointOut = FakeEndpointOut;
        type EndpointIn = FakeEndpointIn;
        type ControlPipe = FakeControlPipe;
        type Bus = FakeBus;

        fn alloc_endpoint_out(
            &mut self,
            _ep_type: EndpointType,
            _ep_addr: Option<EndpointAddress>,
            _max_packet_size: u16,
            _interval_ms: u8,
        ) -> Result<Self::EndpointOut, EndpointAllocError> {
            panic!("FakeDriver::alloc_endpoint_out should not be called in transport tests");
        }

        fn alloc_endpoint_in(
            &mut self,
            _ep_type: EndpointType,
            _ep_addr: Option<EndpointAddress>,
            _max_packet_size: u16,
            _interval_ms: u8,
        ) -> Result<Self::EndpointIn, EndpointAllocError> {
            panic!("FakeDriver::alloc_endpoint_in should not be called in transport tests");
        }

        fn start(self, _control_max_packet_size: u16) -> (Self::Bus, Self::ControlPipe) {
            panic!("FakeDriver::start should not be called in transport tests");
        }
    }

    #[test]
    fn wait_connection_preserves_pending_frame() {
        let ep_in = FakeEndpointIn::new([]);
        let inner = Mutex::<RawMutex, UsbBulkTxState<'static, FakeDriver>>::new(UsbBulkTxState::new(ep_in.clone()));
        let connected = Signal::<RawMutex, ()>::new();
        let tx = UsbBulkTx::new(&inner, &connected);

        block_on(async {
            {
                let mut state = inner.lock().await;
                state.pending_frame = true;
            }
            connected.signal(());
            tx.wait_connection().await;

            let state = inner.lock().await;
            assert!(state.pending_frame);
        });

        assert_eq!(ep_in.wait_enabled_calls(), 1);
    }

    #[test]
    fn send_buf_emits_cleanup_zlp_for_pending_frame() {
        let mut ep_in = FakeEndpointIn::new([]);
        let mut pending_frame = true;

        block_on(async {
            send_buf(&mut ep_in, &[1, 2, 3], &mut pending_frame).await.unwrap();
        });

        assert_eq!(ep_in.writes(), vec![vec![], vec![1, 2, 3]]);
        assert!(!pending_frame);
    }

    #[test]
    fn send_buf_skips_cleanup_zlp_for_clean_frame() {
        let mut ep_in = FakeEndpointIn::new([]);
        let mut pending_frame = false;

        block_on(async {
            send_buf(&mut ep_in, &[4, 5, 6], &mut pending_frame).await.unwrap();
        });

        assert_eq!(ep_in.writes(), vec![vec![4, 5, 6]]);
        assert!(!pending_frame);
    }

    #[test]
    fn timeout_keeps_pending_frame_for_next_send() {
        let mut ep_in = FakeEndpointIn::new([WriteBehavior::PendingForever, WriteBehavior::Ok, WriteBehavior::Ok]);
        let mut pending_frame = false;

        let err = block_on(async { send_buf(&mut ep_in, &[7], &mut pending_frame).await.unwrap_err() });
        assert!(matches!(err, WireTxErrorKind::Timeout));
        assert!(pending_frame);

        block_on(async {
            send_buf(&mut ep_in, &[8, 9], &mut pending_frame).await.unwrap();
        });

        assert_eq!(ep_in.writes(), vec![vec![7], vec![], vec![8, 9]]);
        assert!(!pending_frame);
    }

    #[test]
    fn cleanup_zlp_failure_surfaces_connection_closed() {
        let mut ep_in = FakeEndpointIn::new([WriteBehavior::Disabled]);
        let mut pending_frame = true;

        let err = block_on(async { send_buf(&mut ep_in, &[1], &mut pending_frame).await.unwrap_err() });

        assert!(matches!(err, WireTxErrorKind::ConnectionClosed));
        assert_eq!(ep_in.writes(), vec![vec![]]);
        assert!(pending_frame);
    }
}
