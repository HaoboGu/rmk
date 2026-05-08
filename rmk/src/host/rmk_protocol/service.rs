//! Transport-agnostic `Runnable` host service for the RMK protocol.
//!
//! `RmkProtocolService` joins the per-transport postcard-rpc `Server`s (USB
//! and/or BLE depending on cargo features) plus their topic publishers in a
//! single `Runnable::run` future. The user constructs one instance and runs
//! it alongside `UsbTransport` / `BleTransport` via the existing `run_all!`
//! macro.
//!
//! ## Why the wire mutex lives on the `run` stack
//!
//! `Mutex<RawMutex, UsbWireTxInner<D>>` is generic in `D`, and Rust statics
//! cannot reference generic parameters — so the per-transport mutex has to
//! be a function-local. The byte buffers it borrows (`tx_buf`, `rx_buf`,
//! `cobs_buf`, `scratch`) are concrete arrays, so they sit in
//! function-local `static_cell::StaticCell`s; the wire types pick the
//! shorter `'b` borrow lifetime up from the mutex it's stored in. That
//! collapses the public API to just `RmkProtocolService` plus a `Runnable`
//! impl, with no user-declared storage statics.

#[cfg(any(not(feature = "_no_usb"), feature = "_ble"))]
use embassy_sync::mutex::Mutex;
#[cfg(not(feature = "_no_usb"))]
use embassy_usb::driver::Driver;
#[cfg(any(not(feature = "_no_usb"), feature = "_ble"))]
use postcard_rpc::header::VarKeyKind;
#[cfg(any(not(feature = "_no_usb"), feature = "_ble"))]
use postcard_rpc::server::Server;
#[cfg(any(not(feature = "_no_usb"), feature = "_ble"))]
use static_cell::StaticCell;

#[cfg(feature = "_ble")]
use super::topics::run_ble_topic_publisher;
#[cfg(not(feature = "_no_usb"))]
use super::topics::run_usb_topic_publisher;
#[cfg(feature = "_ble")]
use super::wire_ble::{BLE_FRAME_MAX, BLE_RX_BUF, BleWireRx, BleWireTx, BleWireTxInner};
#[cfg(not(feature = "_no_usb"))]
use super::wire_usb::{UsbWireRx, UsbWireTx, UsbWireTxInner};
#[cfg(any(not(feature = "_no_usb"), feature = "_ble"))]
use super::{Ctx, RmkProtocolApp};
#[cfg(any(not(feature = "_no_usb"), feature = "_ble"))]
use crate::RawMutex;
#[cfg(feature = "_ble")]
use crate::channel::{BLE_RMK_PROTOCOL_READY, RMK_PROTOCOL_REPLY_CHANNEL, RMK_PROTOCOL_REQUEST_CHANNEL};
use crate::core_traits::Runnable;
use crate::keymap::KeyMap;

/// Postcard-rpc dispatch RX buffer size for USB. Must hold the largest decoded
/// request payload + header. Aligned with the BLE side's frame ceiling.
#[cfg(not(feature = "_no_usb"))]
const USB_RX_BUF_LEN: usize = 512;
/// USB WireTx scratch buffer size (post-header + serialized response).
#[cfg(not(feature = "_no_usb"))]
const USB_TX_BUF_LEN: usize = 512;

/// Transport-agnostic host service for the RMK protocol.
///
/// Owns the per-transport postcard-rpc state and runs both servers (USB and
/// BLE, gated on cargo features) plus their topic publishers under a single
/// `Runnable`. Construct one instance per program and run it with
/// `run_all!`/`join_all!` next to `UsbTransport` / `BleTransport`.
#[cfg(not(feature = "_no_usb"))]
pub struct RmkProtocolService<'a, D: Driver<'static> + 'static> {
    keymap: &'a KeyMap<'a>,
    usb_ep_in: Option<D::EndpointIn>,
    usb_ep_out: Option<D::EndpointOut>,
}

#[cfg(feature = "_no_usb")]
pub struct RmkProtocolService<'a> {
    keymap: &'a KeyMap<'a>,
}

#[cfg(not(feature = "_no_usb"))]
impl<'a, D: Driver<'static> + 'static> RmkProtocolService<'a, D> {
    /// Build the service. The USB endpoints come from
    /// [`UsbTransport::take_rmk_protocol_endpoints`](crate::usb::UsbTransport::take_rmk_protocol_endpoints);
    /// they must be passed in here so the user-facing `UsbTransport` keeps no
    /// hidden coupling to this module.
    pub fn new(keymap: &'a KeyMap<'a>, usb_endpoints: (D::EndpointIn, D::EndpointOut)) -> Self {
        let (ep_in, ep_out) = usb_endpoints;
        Self {
            keymap,
            usb_ep_in: Some(ep_in),
            usb_ep_out: Some(ep_out),
        }
    }
}

#[cfg(feature = "_no_usb")]
impl<'a> RmkProtocolService<'a> {
    pub fn new(keymap: &'a KeyMap<'a>) -> Self {
        Self { keymap }
    }
}

#[cfg(all(not(feature = "_no_usb"), not(feature = "_ble")))]
impl<'a, D: Driver<'static> + 'static> Runnable for RmkProtocolService<'a, D> {
    async fn run(&mut self) -> ! {
        run_usb::<D>(self.keymap, self.usb_ep_in.take(), self.usb_ep_out.take()).await
    }
}

#[cfg(all(not(feature = "_no_usb"), feature = "_ble"))]
impl<'a, D: Driver<'static> + 'static> Runnable for RmkProtocolService<'a, D> {
    async fn run(&mut self) -> ! {
        let usb = run_usb::<D>(self.keymap, self.usb_ep_in.take(), self.usb_ep_out.take());
        let ble = run_ble(self.keymap);
        embassy_futures::join::join(usb, ble).await;
        unreachable!("rmk_protocol service tasks must run forever")
    }
}

#[cfg(all(feature = "_no_usb", feature = "_ble"))]
impl<'a> Runnable for RmkProtocolService<'a> {
    async fn run(&mut self) -> ! {
        run_ble(self.keymap).await
    }
}

/// USB postcard-rpc server + topic publisher loop. Joined for the lifetime
/// of the surrounding `run` task; allocates its mutex on the local stack
/// (because the `Mutex<RawMutex, UsbWireTxInner<D>>` type is generic in `D`
/// and can't appear in a `static`).
#[cfg(not(feature = "_no_usb"))]
async fn run_usb<'a, D: Driver<'static> + 'static>(
    keymap: &'a KeyMap<'a>,
    ep_in: Option<D::EndpointIn>,
    ep_out: Option<D::EndpointOut>,
) -> ! {
    static USB_TX_BUF: StaticCell<[u8; USB_TX_BUF_LEN]> = StaticCell::new();
    static USB_RX_BUF: StaticCell<[u8; USB_RX_BUF_LEN]> = StaticCell::new();
    let tx_buf: &'static mut [u8] = USB_TX_BUF.init([0u8; USB_TX_BUF_LEN]);
    let rx_buf: &'static mut [u8] = USB_RX_BUF.init([0u8; USB_RX_BUF_LEN]);

    let ep_in = ep_in.expect("RmkProtocolService::run called twice");
    let ep_out = ep_out.expect("RmkProtocolService::run called twice");

    let mutex: Mutex<RawMutex, UsbWireTxInner<'_, D>> = Mutex::new(UsbWireTxInner {
        ep_in,
        tx_buf,
        pending_frame: false,
    });
    let tx: UsbWireTx<'_, '_, RawMutex, D> = UsbWireTx::new(&mutex);
    let rx = UsbWireRx::<D> { ep_out };

    let app = RmkProtocolApp::<UsbWireTx<'_, '_, RawMutex, D>>::new(Ctx::new(keymap));
    let mut server = Server::new(tx.clone(), rx, &mut rx_buf[..], app, VarKeyKind::Key8);

    let publisher = run_usb_topic_publisher(server.sender());
    let serve = async {
        loop {
            // `Server::run` returns on fatal Tx/Rx errors. Wire impls re-await
            // connection on each iter, so we just respin.
            let _ = server.run().await;
        }
    };

    embassy_futures::join::join(serve, publisher).await;
    unreachable!("USB rmk_protocol server tasks must run forever");
}

/// BLE postcard-rpc server + topic publisher loop. Outer dispatch waits on
/// `BLE_RMK_PROTOCOL_READY` so we don't push notifies into a connection that
/// hasn't enabled them yet.
#[cfg(feature = "_ble")]
async fn run_ble<'a>(keymap: &'a KeyMap<'a>) -> ! {
    static BLE_TX_BUF: StaticCell<[u8; BLE_FRAME_MAX]> = StaticCell::new();
    static BLE_COBS_BUF: StaticCell<[u8; BLE_FRAME_MAX]> = StaticCell::new();
    static BLE_RX_BUF_S: StaticCell<[u8; BLE_RX_BUF]> = StaticCell::new();
    static BLE_SCRATCH: StaticCell<[u8; BLE_RX_BUF]> = StaticCell::new();
    let tx_buf: &'static mut [u8] = BLE_TX_BUF.init([0u8; BLE_FRAME_MAX]);
    let cobs_buf: &'static mut [u8] = BLE_COBS_BUF.init([0u8; BLE_FRAME_MAX]);
    let rx_buf: &'static mut [u8] = BLE_RX_BUF_S.init([0u8; BLE_RX_BUF]);
    let scratch: &'static mut [u8] = BLE_SCRATCH.init([0u8; BLE_RX_BUF]);

    let mutex: Mutex<RawMutex, BleWireTxInner<'_>> = Mutex::new(BleWireTxInner {
        tx_buf,
        cobs_buf,
        replies: &RMK_PROTOCOL_REPLY_CHANNEL,
    });
    let tx: BleWireTx<'_, '_, RawMutex> = BleWireTx::new(&mutex);
    let rx = BleWireRx {
        requests: &RMK_PROTOCOL_REQUEST_CHANNEL,
        scratch,
        scratch_len: 0,
        draining: false,
    };

    let app = RmkProtocolApp::<BleWireTx<'_, '_, RawMutex>>::new(Ctx::new(keymap));
    let mut server = Server::new(tx.clone(), rx, &mut rx_buf[..], app, VarKeyKind::Key8);

    let publisher = run_ble_topic_publisher(server.sender());
    let serve = async {
        loop {
            BLE_RMK_PROTOCOL_READY.wait().await;
            let _ = server.run().await;
        }
    };

    embassy_futures::join::join(serve, publisher).await;
    unreachable!("BLE rmk_protocol server tasks must run forever");
}
