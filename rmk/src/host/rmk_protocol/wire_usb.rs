//! USB bulk transport adapter for the RMK protocol.
//!
//! Templated from `postcard_rpc::server::impls::embassy_usb_v0_5` (the upstream
//! adapter) but adjusted to embassy-sync 0.8 / embassy-usb 0.6 (the upstream
//! adapter is pinned to 0.7 / 0.5). The framing model is identical: each
//! postcard-rpc frame is split into USB packets of `MAX_USB_FRAME_SIZE` bytes
//! and terminated by either a short packet or, when the body is an exact
//! multiple of the packet size, an explicit zero-length packet.
//!
//! No COBS layer is added on top — short-packet framing already delimits frames
//! cleanly on USB bulk. (The plan's "COBS-framed byte stream" applies to BLE
//! only; USB bulk uses USB's own framing.)
//!
//! Logging endpoints (`send_log_str`, `send_log_fmt`) are stubbed to return
//! `WireTxErrorKind::Other` — the RMK protocol does not use postcard-rpc
//! logging.

use core::fmt::Arguments;

use embassy_futures::select::{Either, select};
use embassy_sync::blocking_mutex::raw::RawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::Timer;
use embassy_usb::driver::{Driver, Endpoint, EndpointError, EndpointIn, EndpointOut};
use postcard_rpc::header::VarHeader;
use postcard_rpc::server::{WireRx, WireRxErrorKind, WireTx, WireTxErrorKind};
use serde::Serialize;

/// USB bulk max packet size (full-speed). Hardcoded — RMK targets full-speed
/// USB. High-speed parts may eventually want 512.
pub(crate) const USB_MAX_PACKET: usize = 64;

/// Time in milliseconds the sender is allowed per USB frame before reporting
/// timeout. Matches postcard-rpc's upstream default.
const TIMEOUT_MS_PER_FRAME: u64 = 2;

// ---------------------------------------------------------------------------
// TX
// ---------------------------------------------------------------------------

pub(crate) struct UsbWireTxInner<'b, D: Driver<'static>> {
    pub(crate) ep_in: D::EndpointIn,
    pub(crate) tx_buf: &'b mut [u8],
    pub(crate) pending_frame: bool,
}

/// `WireTx` impl over an embassy-usb bulk-IN endpoint. The wire holds a
/// shared reference to a [`Mutex`] guarding its inner state; the mutex is
/// allocated by `RmkProtocolService::run` for the lifetime of the run task.
pub(crate) struct UsbWireTx<'a, 'b, M: RawMutex + 'static, D: Driver<'static> + 'static> {
    inner: &'a Mutex<M, UsbWireTxInner<'b, D>>,
}

impl<'m, 'b, M: RawMutex + 'static, D: Driver<'static> + 'static> UsbWireTx<'m, 'b, M, D> {
    pub(crate) fn new(inner: &'m Mutex<M, UsbWireTxInner<'b, D>>) -> Self {
        Self { inner }
    }
}

impl<'m, 'b, M: RawMutex + 'static, D: Driver<'static> + 'static> Clone for UsbWireTx<'m, 'b, M, D> {
    fn clone(&self) -> Self {
        Self { inner: self.inner }
    }
}

impl<'m, 'b, M: RawMutex + 'static, D: Driver<'static> + 'static> WireTx for UsbWireTx<'m, 'b, M, D> {
    type Error = WireTxErrorKind;

    async fn wait_connection(&self) {
        let mut guard = self.inner.lock().await;
        guard.ep_in.wait_enabled().await;
        // After a reconnect any leftover `pending_frame` flag is meaningless —
        // the host's reframer is reset by re-enumeration. Clear it so the next
        // `send_all` doesn't emit a stray ZLP into the new session.
        guard.pending_frame = false;
    }

    async fn send<T: Serialize + ?Sized>(&self, hdr: VarHeader, msg: &T) -> Result<(), Self::Error> {
        let mut guard = self.inner.lock().await;
        let UsbWireTxInner {
            ep_in,
            tx_buf,
            pending_frame,
        } = &mut *guard;

        let (hdr_used, remain) = hdr.write_to_slice(tx_buf).ok_or(WireTxErrorKind::Other)?;
        let body_used = postcard::to_slice(msg, remain).map_err(|_| WireTxErrorKind::Other)?;
        let used_total = hdr_used.len() + body_used.len();
        let frame = tx_buf.get(..used_total).ok_or(WireTxErrorKind::Other)?;
        send_all::<D>(ep_in, frame, pending_frame).await
    }

    async fn send_raw(&self, buf: &[u8]) -> Result<(), Self::Error> {
        let mut guard = self.inner.lock().await;
        let UsbWireTxInner {
            ep_in, pending_frame, ..
        } = &mut *guard;
        send_all::<D>(ep_in, buf, pending_frame).await
    }

    async fn send_log_str(&self, _kkind: postcard_rpc::header::VarKeyKind, _s: &str) -> Result<(), Self::Error> {
        Err(WireTxErrorKind::Other)
    }

    async fn send_log_fmt<'a>(
        &self,
        _kkind: postcard_rpc::header::VarKeyKind,
        _a: Arguments<'a>,
    ) -> Result<(), Self::Error> {
        Err(WireTxErrorKind::Other)
    }
}

async fn send_all<D>(ep_in: &mut D::EndpointIn, out: &[u8], pending_frame: &mut bool) -> Result<(), WireTxErrorKind>
where
    D: Driver<'static>,
{
    if out.is_empty() {
        return Ok(());
    }
    let frames = out.len().div_ceil(USB_MAX_PACKET);
    let timeout_ms = (frames as u64) * TIMEOUT_MS_PER_FRAME;

    let send = async {
        // If a previous send left a pending unterminated frame, flush an empty
        // packet first so the host's reframer doesn't fuse messages.
        if *pending_frame && ep_in.write(&[]).await.is_err() {
            return Err(WireTxErrorKind::ConnectionClosed);
        }
        *pending_frame = true;
        for ch in out.chunks(USB_MAX_PACKET) {
            if ep_in.write(ch).await.is_err() {
                return Err(WireTxErrorKind::ConnectionClosed);
            }
        }
        // If exact multiple of packet size, send ZLP to terminate the bulk transfer.
        if out.len() % USB_MAX_PACKET == 0 && ep_in.write(&[]).await.is_err() {
            return Err(WireTxErrorKind::ConnectionClosed);
        }
        *pending_frame = false;
        Ok(())
    };

    match select(send, Timer::after_millis(timeout_ms)).await {
        Either::First(r) => r,
        Either::Second(()) => Err(WireTxErrorKind::Timeout),
    }
}

// ---------------------------------------------------------------------------
// RX
// ---------------------------------------------------------------------------

/// `WireRx` impl over an embassy-usb bulk-OUT endpoint. Frames are delimited by
/// short packets (any read smaller than `USB_MAX_PACKET`).
pub(crate) struct UsbWireRx<D: Driver<'static>> {
    pub(crate) ep_out: D::EndpointOut,
}

impl<D: Driver<'static>> WireRx for UsbWireRx<D> {
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
            let (_now, later) = window.split_at_mut(n);
            window = later;
            if n != USB_MAX_PACKET {
                let len = buflen - window.len();
                return Ok(&mut buf[..len]);
            }
        }
        // Buffer full and last read was a full packet — drain until end-of-frame
        // (a sub-`USB_MAX_PACKET` read), then report too-large.
        loop {
            match self.ep_out.read(buf).await {
                Ok(USB_MAX_PACKET) => {}
                Ok(_) => return Err(WireRxErrorKind::ReceivedMessageTooLarge),
                Err(EndpointError::BufferOverflow) => return Err(WireRxErrorKind::ReceivedMessageTooLarge),
                Err(EndpointError::Disabled) => return Err(WireRxErrorKind::ConnectionClosed),
            }
        }
    }
}
