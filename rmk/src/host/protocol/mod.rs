//! RMK protocol service.
//!
//! This module hosts the implemented subset of the RMK ICD on top of
//! postcard-rpc's `Server`/`Dispatch` abstractions.
//!
//! The USB transport uses raw bulk transfer boundaries (short-packet
//! termination) for framing, not COBS.

mod dispatch_macro;
mod handlers;
pub(crate) mod transport;

use crate::define_dispatch;
use embassy_futures::select::select;
use embassy_sync::channel::Channel;
use postcard_rpc::header::VarHeader;
use postcard_rpc::server::{
    AsWireRxErrorKind, AsWireTxErrorKind, Dispatch, Sender, WireRx, WireRxErrorKind, WireSpawn, WireTx, WireTxErrorKind,
};
use rmk_types::protocol::rmk::*;
use transport::{QueuingTx, TX_QUEUE_DEPTH, TxFrame};

use crate::RawMutex;
use crate::keymap::KeyMap;
use handlers::*;

// RX buffer must fit the largest possible incoming frame.
// MAX_BULK=512, worst-case ~10 bytes/key → 512*10 + VarHeader ≈ 5130B.
// 2048 covers the realistic average (~2 bytes/key → ~1040B) with room for complex keys.
const RX_BUF_SIZE: usize = 2048;

// ---------------------------------------------------------------------------
// NoSpawn: no-op WireSpawn for dispatchers that only use blocking/async handlers.
// The published postcard-rpc 0.12.x does not ship this type, so we provide it.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub(crate) struct NoSpawn;

impl WireSpawn for NoSpawn {
    type Error = core::convert::Infallible;
    type Info = ();
    fn info(&self) -> &() {
        &()
    }
}

fn no_spawn<S, F>(_: &S, _: F) -> Result<(), core::convert::Infallible>
where
    F: core::future::Future<Output = ()> + 'static,
{
    Ok(())
}

// ---------------------------------------------------------------------------
// Dispatch via define_dispatch! macro
// ---------------------------------------------------------------------------

define_dispatch! {
    app: RmkDispatcher[
        'a,
        Tx: WireTx
    ];
    tx_impl: Tx;
    context: ProtocolContext<'a>;

    endpoints: {
        list: ENDPOINT_LIST;

        // Only implemented endpoints need handlers here.
        // Unimplemented ones (encoder, macro, combo, morse, fork, behavior,
        // BLE control, battery, split) are still in ENDPOINT_LIST for key-width
        // calculation but get the macro's default UnknownKey response.
        | EndpointTy        | kind  | handler              |
        | ----------         | ----  | -------              |
        | GetVersion         | async | get_version          |
        | GetCapabilities    | async | get_capabilities     |
        | GetLockStatus      | async | get_lock_status      |
        | UnlockRequest      | async | unlock_request       |
        | LockRequest        | async | lock_request         |
        | Reboot             | async | reboot               |
        | BootloaderJump     | async | bootloader_jump      |
        | StorageReset       | async | storage_reset        |
        | GetKeyAction       | async | get_key_action       |
        | SetKeyAction       | async | set_key_action       |
        | GetKeymapBulk      | async | get_keymap_bulk      |
        | SetKeymapBulk      | async | set_keymap_bulk      |
        | GetLayerCount      | async | get_layer_count      |
        | GetDefaultLayer    | async | get_default_layer    |
        | SetDefaultLayer    | async | set_default_layer    |
        | ResetKeymap        | async | reset_keymap         |
        | GetConnectionInfo  | async | get_connection_info  |
        | GetCurrentLayer    | async | get_current_layer    |
        | GetMatrixState     | async | get_matrix_state     |
    };
    topics_in: {
        list: TOPICS_IN_LIST;
    };
    topics_out: {
        list: TOPICS_OUT_LIST;
    };
}

// ---------------------------------------------------------------------------
// Protocol service
// ---------------------------------------------------------------------------

pub(crate) struct ProtocolService<'a, Tx: WireTx, Rx: WireRx> {
    keymap: &'a KeyMap<'a>,
    tx: Tx,
    rx: Rx,
    rx_buf: [u8; RX_BUF_SIZE],
}

impl<'a, Tx: WireTx + Copy, Rx: WireRx> ProtocolService<'a, Tx, Rx> {
    pub(crate) fn new(keymap: &'a KeyMap<'a>, tx: Tx, rx: Rx) -> Self {
        Self {
            keymap,
            tx,
            rx,
            rx_buf: [0u8; RX_BUF_SIZE],
        }
    }

    /// Run the protocol dispatch loop.
    ///
    /// Dispatch and USB TX are decoupled via a bounded channel: the dispatch
    /// future serializes responses into `TxFrame`s and enqueues them, while the
    /// drain future independently flushes frames to the USB endpoint.  Both run
    /// concurrently under `select`; when either side hits a fatal error the
    /// other is dropped and the outer loop reconnects with a fresh session.
    pub(crate) async fn run(&mut self) {
        let tx_channel: Channel<RawMutex, TxFrame, TX_QUEUE_DEPTH> = Channel::new();
        loop {
            let ctx = ProtocolContext {
                keymap: self.keymap,
                locked: true,
            };
            let mut dispatch = impls::RmkDispatcher::<'_, _, { sizer::NEEDED_SZ }>::new(ctx, NoSpawn);
            let vkk = dispatch.min_key_len();

            // Wait for connection on both sides.
            self.rx.wait_connection().await;
            self.tx.wait_connection().await;

            let queuing_tx = QueuingTx::new(self.tx, &tx_channel);
            let sender = Sender::new(queuing_tx, vkk);
            let rx = &mut self.rx;
            let rbuf = &mut self.rx_buf;
            let tx = self.tx; // Copy — used by drain_fut to flush queued frames

            let dispatch_fut = async {
                let mut req_idx: u32 = 0;
                loop {
                    let used = match rx.receive(rbuf).await {
                        Ok(u) => u,
                        Err(e) => match e.as_kind() {
                            WireRxErrorKind::ConnectionClosed => {
                                warn!("[proto] rx closed after {} requests", req_idx);
                                break;
                            }
                            other => {
                                warn!("[proto] rx error: {:?}, continuing", other);
                                continue;
                            }
                        },
                    };
                    let Some((hdr, body)) = VarHeader::take_from_slice(used) else {
                        warn!("[proto] #{}: bad header ({} bytes)", req_idx, used.len());
                        continue;
                    };
                    req_idx += 1;
                    debug!("[proto] #{} rx body={}B", req_idx, body.len());
                    if let Err(e) = dispatch.handle(&sender, &hdr, body).await {
                        match e.as_kind() {
                            WireTxErrorKind::ConnectionClosed => {
                                warn!("[proto] #{} tx ConnectionClosed, breaking", req_idx);
                                break;
                            }
                            kind => {
                                warn!("[proto] #{} tx error: {:?}, continuing", req_idx, kind);
                            }
                        }
                    }
                }
            };

            let drain_fut = async {
                loop {
                    let frame = tx_channel.receive().await;
                    if tx.send_raw(&frame.buf[..frame.len]).await.is_err() {
                        break;
                    }
                }
            };

            select(dispatch_fut, drain_fut).await;
            // Flush any frames that were queued but not yet drained before reconnecting.
            while tx_channel.try_receive().is_ok() {}
        }
    }
}

impl<'a, Tx: WireTx + Copy, Rx: WireRx> crate::host::HostService for ProtocolService<'a, Tx, Rx> {
    async fn run(&mut self) {
        ProtocolService::run(self).await;
    }
}
