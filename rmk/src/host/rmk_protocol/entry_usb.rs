//! USB rmk_protocol server entry point.
//!
//! Constructed by the orchestrator-generated entry code. Wraps the
//! `Server::run` reentry loop and the topic publisher behind a single
//! `run_usb_server` future the entry-task list can `.run()` alongside the
//! existing `UsbTransport::run()` call.

use embassy_futures::join::join;
use embassy_sync::mutex::Mutex;
use embassy_usb::driver::Driver;
use postcard_rpc::header::VarKeyKind;
use postcard_rpc::server::Server;
use static_cell::StaticCell;

use super::topics::run_topic_publisher;
use super::wire_usb::{UsbWireRx, UsbWireTx, UsbWireTxInner};
use super::{Ctx, RmkProtocolApp};
use crate::RawMutex;
use crate::keymap::KeyMap;

/// Postcard-rpc dispatch RX buffer size. Must hold the largest decoded request
/// payload + header. Aligned with the BLE side's frame ceiling.
const USB_RX_BUF_LEN: usize = 512;
/// WireTx scratch buffer size (post-header + serialized response).
const USB_TX_BUF_LEN: usize = 512;

/// Static storage required to run the USB rmk_protocol server. The
/// orchestrator declares one of these as a `static` at the call site (where
/// the concrete `D` is known) and passes a `&'static` reference to
/// [`run_usb_server`].
pub struct UsbServerStorage<D: Driver<'static> + 'static> {
    pub mutex: StaticCell<Mutex<RawMutex, UsbWireTxInner<D>>>,
    pub tx_buf: StaticCell<[u8; USB_TX_BUF_LEN]>,
    pub rx_buf: StaticCell<[u8; USB_RX_BUF_LEN]>,
}

impl<D: Driver<'static> + 'static> UsbServerStorage<D> {
    pub const fn new() -> Self {
        Self {
            mutex: StaticCell::new(),
            tx_buf: StaticCell::new(),
            rx_buf: StaticCell::new(),
        }
    }
}

impl<D: Driver<'static> + 'static> Default for UsbServerStorage<D> {
    fn default() -> Self {
        Self::new()
    }
}

/// Run the USB rmk_protocol Server forever. Joined alongside `UsbTransport::run`
/// in the macro-generated entry task list.
///
/// `ep_in` / `ep_out` come from `UsbTransport::take_rmk_protocol_endpoints`.
pub async fn run_usb_server<'a, D: Driver<'static> + 'static>(
    storage: &'static UsbServerStorage<D>,
    keymap: &'a KeyMap<'a>,
    ep_in: D::EndpointIn,
    ep_out: D::EndpointOut,
) -> ! {
    let tx_buf: &'static mut [u8] = storage.tx_buf.init([0u8; USB_TX_BUF_LEN]);
    let rx_buf: &'static mut [u8] = storage.rx_buf.init([0u8; USB_RX_BUF_LEN]);
    let mutex_ref: &'static Mutex<RawMutex, UsbWireTxInner<D>> = storage.mutex.init(Mutex::new(UsbWireTxInner {
        ep_in,
        tx_buf,
        pending_frame: false,
    }));

    let tx = UsbWireTx::<RawMutex, D>::new(mutex_ref);
    let rx = UsbWireRx::<D> { ep_out };

    let app = RmkProtocolApp::<'a, UsbWireTx<RawMutex, D>>::new(Ctx::new(keymap));
    let mut server = Server::new(tx.clone(), rx, &mut rx_buf[..], app, VarKeyKind::Key8);

    let publisher = run_topic_publisher(server.sender());
    let serve = async {
        loop {
            // `Server::run` returns on fatal Tx/Rx errors. Wire impls re-await
            // connection on each iter, so we just respin.
            let _ = server.run().await;
        }
    };

    join(serve, publisher).await;
    unreachable!("USB rmk_protocol server tasks must run forever");
}
