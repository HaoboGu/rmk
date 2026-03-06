use core::fmt::Arguments;

use postcard_rpc::header::{VarHeader, VarKeyKind};
use postcard_rpc::server::{WireRx, WireRxErrorKind, WireTx, WireTxErrorKind};
use serde::Serialize;

// ---------------------------------------------------------------------------
// USB bulk endpoint transport (only available when USB hardware is present)
// ---------------------------------------------------------------------------

#[cfg(not(feature = "_no_usb"))]
mod usb_bulk {
    use core::cell::RefCell;
    use core::fmt::Arguments;

    use embassy_usb::driver::{Driver, Endpoint, EndpointError, EndpointIn, EndpointOut};
    use postcard_rpc::header::{VarHeader, VarKey, VarKeyKind, VarSeq};
    use postcard_rpc::server::{WireRx, WireRxErrorKind, WireTx, WireTxErrorKind};
    use postcard_rpc::standard_icd::LoggingTopic;
    use postcard_rpc::Topic;
    use serde::Serialize;

    const TX_BUF_SIZE: usize = 256;
    const MAX_PACKET_SIZE: usize = 64;

    /// USB bulk IN transport implementing postcard-rpc `WireTx`.
    ///
    /// Borrows the endpoint so the caller can re-create this struct across loop
    /// iterations (matching the `UsbHostReaderWriter` pattern used by Vial).
    pub(crate) struct UsbBulkTx<'a, D: Driver<'static>> {
        inner: RefCell<UsbBulkTxInner<'a, D>>,
    }

    struct UsbBulkTxInner<'a, D: Driver<'static>> {
        ep_in: &'a mut D::EndpointIn,
        tx_buf: [u8; TX_BUF_SIZE],
    }

    impl<'a, D: Driver<'static>> UsbBulkTx<'a, D> {
        pub(crate) fn new(ep_in: &'a mut D::EndpointIn) -> Self {
            Self {
                inner: RefCell::new(UsbBulkTxInner {
                    ep_in,
                    tx_buf: [0u8; TX_BUF_SIZE],
                }),
            }
        }
    }

    async fn send_all<E: EndpointIn>(ep_in: &mut E, data: &[u8]) -> Result<(), WireTxErrorKind> {
        if data.is_empty() {
            return Ok(());
        }

        for chunk in data.chunks(MAX_PACKET_SIZE) {
            ep_in
                .write(chunk)
                .await
                .map_err(|_| WireTxErrorKind::ConnectionClosed)?;
        }

        // ZLP to terminate transfer when data is an exact multiple of max packet size
        if data.len() % MAX_PACKET_SIZE == 0 {
            ep_in
                .write(&[])
                .await
                .map_err(|_| WireTxErrorKind::ConnectionClosed)?;
        }

        Ok(())
    }

    impl<D: Driver<'static>> WireTx for UsbBulkTx<'_, D> {
        type Error = WireTxErrorKind;

        async fn wait_connection(&self) {
            let mut inner = self.inner.borrow_mut();
            inner.ep_in.wait_enabled().await;
        }

        async fn send<T: Serialize + ?Sized>(
            &self,
            hdr: VarHeader,
            msg: &T,
        ) -> Result<(), Self::Error> {
            let mut inner = self.inner.borrow_mut();
            let UsbBulkTxInner { ep_in, tx_buf } = &mut *inner;

            let (hdr_used, remain) =
                hdr.write_to_slice(tx_buf).ok_or(WireTxErrorKind::Other)?;
            let body_used =
                postcard::to_slice(msg, remain).map_err(|_| WireTxErrorKind::Other)?;
            let total = hdr_used.len() + body_used.len();

            send_all(*ep_in, &tx_buf[..total]).await
        }

        async fn send_raw(&self, buf: &[u8]) -> Result<(), Self::Error> {
            let mut inner = self.inner.borrow_mut();
            send_all(inner.ep_in, buf).await
        }

        async fn send_log_str(&self, kkind: VarKeyKind, s: &str) -> Result<(), Self::Error> {
            let mut inner = self.inner.borrow_mut();
            let UsbBulkTxInner { ep_in, tx_buf } = &mut *inner;

            let key = match kkind {
                VarKeyKind::Key1 => VarKey::Key1(LoggingTopic::TOPIC_KEY1),
                VarKeyKind::Key2 => VarKey::Key2(LoggingTopic::TOPIC_KEY2),
                VarKeyKind::Key4 => VarKey::Key4(LoggingTopic::TOPIC_KEY4),
                VarKeyKind::Key8 => VarKey::Key8(LoggingTopic::TOPIC_KEY),
            };
            let hdr = VarHeader {
                key,
                seq_no: VarSeq::Seq1(0),
            };
            let (hdr_used, remain) =
                hdr.write_to_slice(tx_buf).ok_or(WireTxErrorKind::Other)?;
            let body_used =
                postcard::to_slice::<str>(s, remain).map_err(|_| WireTxErrorKind::Other)?;
            let total = hdr_used.len() + body_used.len();

            send_all(*ep_in, &tx_buf[..total]).await
        }

        async fn send_log_fmt<'a>(
            &self,
            kkind: VarKeyKind,
            _a: Arguments<'a>,
        ) -> Result<(), Self::Error> {
            self.send_log_str(kkind, "<fmt>").await
        }
    }

    /// USB bulk OUT transport implementing postcard-rpc `WireRx`.
    pub(crate) struct UsbBulkRx<'a, D: Driver<'static>> {
        ep_out: &'a mut D::EndpointOut,
    }

    impl<'a, D: Driver<'static>> UsbBulkRx<'a, D> {
        pub(crate) fn new(ep_out: &'a mut D::EndpointOut) -> Self {
            Self { ep_out }
        }
    }

    impl<D: Driver<'static>> WireRx for UsbBulkRx<'_, D> {
        type Error = WireRxErrorKind;

        async fn wait_connection(&mut self) {
            self.ep_out.wait_enabled().await;
        }

        async fn receive<'a>(
            &mut self,
            buf: &'a mut [u8],
        ) -> Result<&'a mut [u8], Self::Error> {
            let buf_len = buf.len();
            let mut window = &mut buf[..];

            while !window.is_empty() {
                let n = match self.ep_out.read(window).await {
                    Ok(n) => n,
                    Err(EndpointError::BufferOverflow) => {
                        return Err(WireRxErrorKind::ReceivedMessageTooLarge);
                    }
                    Err(EndpointError::Disabled) => {
                        return Err(WireRxErrorKind::ConnectionClosed);
                    }
                };

                let (_filled, rest) = window.split_at_mut(n);
                window = rest;

                // Short packet signals end of USB bulk transfer
                if n != MAX_PACKET_SIZE {
                    let remaining = window.len();
                    let len = buf_len - remaining;
                    return Ok(&mut buf[..len]);
                }
            }

            // Buffer full — drain remaining USB packets to resync
            loop {
                match self.ep_out.read(buf).await {
                    Ok(n) if n == MAX_PACKET_SIZE => continue,
                    Ok(_) => return Err(WireRxErrorKind::ReceivedMessageTooLarge),
                    Err(EndpointError::BufferOverflow) => {
                        return Err(WireRxErrorKind::ReceivedMessageTooLarge);
                    }
                    Err(EndpointError::Disabled) => {
                        return Err(WireRxErrorKind::ConnectionClosed);
                    }
                }
            }
        }
    }
}

#[cfg(not(feature = "_no_usb"))]
pub(crate) use usb_bulk::{UsbBulkRx, UsbBulkTx};

// ---------------------------------------------------------------------------
// Pending (no-op) transport for contexts where no transport is available yet
// (e.g., BLE-connected keyboard, or _no_usb boards before Phase 8)
// ---------------------------------------------------------------------------

/// A `WireTx` that pends forever. Used when no transport is available.
pub(crate) struct PendingTx;

impl WireTx for PendingTx {
    type Error = WireTxErrorKind;

    async fn send<T: Serialize + ?Sized>(
        &self,
        _hdr: VarHeader,
        _msg: &T,
    ) -> Result<(), Self::Error> {
        core::future::pending().await
    }

    async fn send_raw(&self, _buf: &[u8]) -> Result<(), Self::Error> {
        core::future::pending().await
    }

    async fn send_log_str(&self, _kkind: VarKeyKind, _s: &str) -> Result<(), Self::Error> {
        core::future::pending().await
    }

    async fn send_log_fmt<'a>(
        &self,
        _kkind: VarKeyKind,
        _a: Arguments<'a>,
    ) -> Result<(), Self::Error> {
        core::future::pending().await
    }
}

/// A `WireRx` that pends forever. Used when no transport is available.
pub(crate) struct PendingRx;

impl WireRx for PendingRx {
    type Error = WireRxErrorKind;

    async fn receive<'a>(&mut self, _buf: &'a mut [u8]) -> Result<&'a mut [u8], Self::Error> {
        core::future::pending().await
    }
}
