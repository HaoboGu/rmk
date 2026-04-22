//! The RMK (rynk) host protocol service.
//!
//! Transport-agnostic postcard-rpc server driving the ICD declared in
//! `rmk_types::protocol::rmk`. Implements `crate::input_device::Runnable`
//! over any `(Rx: WireRx, Tx: WireTx)` pair — USB bulk today, BLE custom
//! serial tomorrow.
//!
//! The module is purely plumbing: endpoint/topic types live in `rmk-types`,
//! handler bodies live in `dispatch`, transport impls live under
//! `transport/` and implement postcard-rpc's `WireRx`/`WireTx` directly,
//! and topic fan-out from existing in-crate pubsubs lives in `topics`.

#[cfg(feature = "host_security")]
pub(crate) mod lock;

pub(crate) mod dispatch;
pub(crate) mod topics;
pub(crate) mod transport;

#[cfg(not(feature = "_no_usb"))]
use embassy_usb::Builder;
#[cfg(not(feature = "_no_usb"))]
use embassy_usb::driver::Driver;
use postcard_rpc::server::{WireRx, WireTx};
#[cfg(feature = "_ble")]
use trouble_host::prelude::{GattConnection, PacketPool};

#[cfg(feature = "_ble")]
use crate::host::rynk::transport::ble_serial::{BleSerialRx, BleSerialTx};
#[cfg(not(feature = "_no_usb"))]
use crate::host::rynk::transport::usb_bulk::{UsbBulkRx, UsbBulkTx};
use crate::input_device::Runnable;
use crate::keymap::KeyMap;

#[cfg(not(feature = "_no_usb"))]
impl<'a> RynkService<'a, UsbBulkRx, UsbBulkTx> {
    /// Allocate USB bulk endpoints on the embassy-usb builder and build the
    /// service. Must be called before `builder.build()`.
    ///
    /// TODO: actually reserve embassy-usb bulk IN/OUT endpoints from the
    /// builder; currently constructs stub halves.
    pub(crate) fn from_usb_builder<D: Driver<'static>>(
        _builder: &mut Builder<'static, D>,
        keymap: &'a KeyMap<'a>,
    ) -> Self {
        Self::new(keymap, UsbBulkRx::new(), UsbBulkTx::new())
    }
}

#[cfg(feature = "_ble")]
impl<'a> RynkService<'a, BleSerialRx, BleSerialTx> {
    /// Build a `RynkService` bound to the BLE custom-serial transport on an
    /// established GATT connection.
    pub(crate) fn from_ble<'stack, 'server, 'conn, P: PacketPool>(
        keymap: &'a KeyMap<'a>,
        _server: &crate::ble::ble_server::Server<'_>,
        _conn: &'conn GattConnection<'stack, 'server, P>,
    ) -> Self {
        Self::new(keymap, BleSerialRx::new(), BleSerialTx::new())
    }
}

pub(crate) struct RynkService<'a, Rx: WireRx, Tx: WireTx> {
    #[allow(dead_code)]
    keymap: &'a KeyMap<'a>,
    #[cfg(feature = "host_security")]
    #[allow(dead_code)]
    lock: lock::RynkLock<'a>,
    #[allow(dead_code)]
    rx: Rx,
    #[allow(dead_code)]
    tx: Tx,
}

impl<'a, Rx: WireRx, Tx: WireTx> RynkService<'a, Rx, Tx> {
    pub(crate) fn new(keymap: &'a KeyMap<'a>, rx: Rx, tx: Tx) -> Self {
        Self {
            keymap,
            #[cfg(feature = "host_security")]
            lock: lock::RynkLock::new(keymap),
            rx,
            tx,
        }
    }
}

impl<Rx: WireRx, Tx: WireTx> Runnable for RynkService<'_, Rx, Tx> {
    async fn run(&mut self) -> ! {
        // TODO: drive `postcard_rpc::define_dispatch!` against
        // `rmk_types::protocol::rmk::ENDPOINT_LIST` with `self.rx` as the
        // `WireRx` and `&self.tx` as the `WireTx`, and race it against
        // `topics::run(&self.tx)`. No adapter layer needed — the transport
        // impls `WireRx`/`WireTx` natively.
        loop {
            core::future::pending::<()>().await;
        }
    }
}
