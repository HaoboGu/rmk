//! BLE rmk_protocol server entry point.
//!
//! Owns the dispatch `Server::run` loop, the topic publisher, and the
//! per-connection notify-out task that drains `RMK_PROTOCOL_REPLY_CHANNEL`
//! and writes COBS-encoded frames to the `input_data` characteristic.
//!
//! The `Server` itself is connection-agnostic — it pulls inbound chunks from
//! `RMK_PROTOCOL_REQUEST_CHANNEL` (filled by `gatt_events_task`) and pushes
//! outbound frames to `RMK_PROTOCOL_REPLY_CHANNEL`. The only piece that needs
//! a live `GattConnection` is the notify task, which is owned per-connection
//! by `ble/mod.rs` and started/stopped on connect/disconnect.

use embassy_futures::join::join;
use embassy_sync::mutex::Mutex;
use postcard_rpc::header::VarKeyKind;
use postcard_rpc::server::Server;
use static_cell::StaticCell;

use super::topics::run_topic_publisher;
use super::wire_ble::{BLE_FRAME_MAX, BLE_RX_BUF, BleWireRx, BleWireTx, BleWireTxInner};
use super::{Ctx, RmkProtocolApp};
use crate::RawMutex;
use crate::channel::{BLE_RMK_PROTOCOL_READY, RMK_PROTOCOL_REPLY_CHANNEL, RMK_PROTOCOL_REQUEST_CHANNEL};
use crate::keymap::KeyMap;

/// Static storage for the BLE rmk_protocol server. Declared once as a
/// `static` by the orchestrator-generated code and passed by `&'static` ref
/// to [`run_ble_server`].
pub struct BleServerStorage {
    pub mutex: StaticCell<Mutex<RawMutex, BleWireTxInner>>,
    pub tx_buf: StaticCell<[u8; BLE_FRAME_MAX]>,
    pub cobs_buf: StaticCell<[u8; BLE_FRAME_MAX]>,
    pub rx_buf: StaticCell<[u8; BLE_RX_BUF]>,
    pub scratch: StaticCell<[u8; BLE_RX_BUF]>,
}

impl BleServerStorage {
    pub const fn new() -> Self {
        Self {
            mutex: StaticCell::new(),
            tx_buf: StaticCell::new(),
            cobs_buf: StaticCell::new(),
            rx_buf: StaticCell::new(),
            scratch: StaticCell::new(),
        }
    }
}

impl Default for BleServerStorage {
    fn default() -> Self {
        Self::new()
    }
}

/// Run the BLE rmk_protocol Server forever. Joined alongside `BleTransport::run`
/// in the macro-generated entry task list.
pub async fn run_ble_server<'a>(storage: &'static BleServerStorage, keymap: &'a KeyMap<'a>) -> ! {
    let tx_buf: &'static mut [u8] = storage.tx_buf.init([0u8; BLE_FRAME_MAX]);
    let cobs_buf: &'static mut [u8] = storage.cobs_buf.init([0u8; BLE_FRAME_MAX]);
    let rx_buf: &'static mut [u8] = storage.rx_buf.init([0u8; BLE_RX_BUF]);
    let scratch: &'static mut [u8] = storage.scratch.init([0u8; BLE_RX_BUF]);

    let mutex_ref: &'static Mutex<RawMutex, BleWireTxInner> = storage.mutex.init(Mutex::new(BleWireTxInner {
        tx_buf,
        cobs_buf,
        replies: &RMK_PROTOCOL_REPLY_CHANNEL,
    }));
    let tx = BleWireTx::<RawMutex>::new(mutex_ref);
    let rx = BleWireRx {
        requests: &RMK_PROTOCOL_REQUEST_CHANNEL,
        scratch,
        scratch_len: 0,
    };

    let app = RmkProtocolApp::<'a, BleWireTx<RawMutex>>::new(Ctx::new(keymap));
    let mut server = Server::new(tx.clone(), rx, &mut rx_buf[..], app, VarKeyKind::Key8);

    let publisher = run_topic_publisher(server.sender());
    let serve = async {
        // Outer loop: gate dispatch on a CCCD subscribe so we don't push
        // notifies into a connection that hasn't enabled them yet.
        loop {
            BLE_RMK_PROTOCOL_READY.wait().await;
            let _ = server.run().await;
        }
    };

    join(serve, publisher).await;
    unreachable!("BLE rmk_protocol server tasks must run forever");
}
