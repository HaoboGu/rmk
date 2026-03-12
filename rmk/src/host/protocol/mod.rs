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
use postcard_rpc::header::VarHeader;
use postcard_rpc::server::{
    AsWireRxErrorKind, AsWireTxErrorKind, Dispatch, Sender, WireRx, WireRxErrorKind, WireSpawn,
    WireTx, WireTxErrorKind,
};
use rmk_types::protocol::rmk::*;

use handlers::*;
use crate::keymap::KeyMap;

// RX buffer must fit the largest possible incoming frame:
// SetKeymapBulkRequest = BulkRequest(4 bytes) + up to MAX_BULK(32) KeyAction values.
// Each KeyAction can be up to ~10 bytes postcard-serialized, so worst case is
// ~4 + 32*10 + VarHeader(~6) ≈ 330 bytes. 512 provides comfortable headroom.
const RX_BUF_SIZE: usize = 512;

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

impl<'a, Tx: WireTx + Clone, Rx: WireRx> ProtocolService<'a, Tx, Rx> {
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
    /// On any fatal transport error the inner loop breaks, a fresh locked
    /// session is created, and the service waits for a new connection.
    pub(crate) async fn run(&mut self) {
        loop {
            let ctx = ProtocolContext {
                keymap: self.keymap,
                locked: true,
            };
            let mut dispatch = impls::RmkDispatcher::<'_, _, { sizer::NEEDED_SZ }>::new(ctx, NoSpawn);
            let vkk = dispatch.min_key_len();
            let sender = Sender::new(self.tx.clone(), vkk);

            // Wait for connection on both sides.
            self.rx.wait_connection().await;
            self.tx.wait_connection().await;

            // Dispatch loop — any fatal error reconnects with a fresh locked session.
            loop {
                let used = match self.rx.receive(&mut self.rx_buf).await {
                    Ok(u) => u,
                    Err(e) => {
                        match e.as_kind() {
                            WireRxErrorKind::ConnectionClosed => break,
                            _ => continue,
                        }
                    }
                };
                let Some((hdr, body)) = VarHeader::take_from_slice(used) else {
                    continue;
                };
                if let Err(e) = dispatch.handle(&sender, &hdr, body).await {
                    match e.as_kind() {
                        WireTxErrorKind::ConnectionClosed | WireTxErrorKind::Timeout => break,
                        _ => {}
                    }
                }
            }
        }
    }
}

impl<'a, Tx: WireTx + Clone, Rx: WireRx> crate::host::HostService for ProtocolService<'a, Tx, Rx> {
    async fn run(&mut self) {
        ProtocolService::run(self).await;
    }
}
