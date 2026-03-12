#[cfg(feature = "rmk_protocol")]
pub(crate) mod protocol;
#[cfg(feature = "storage")]
pub(crate) mod storage;
#[cfg(feature = "vial")]
pub mod via;

#[cfg(all(feature = "host", not(feature = "_no_usb"), feature = "vial"))]
pub use via::UsbVialReaderWriter as UsbHostReaderWriter;

#[cfg(all(feature = "host", feature = "_ble", not(feature = "vial")))]
use core::marker::PhantomData;

#[cfg(all(feature = "vial", feature = "rmk_protocol"))]
compile_error!("`vial` and `rmk_protocol` are mutually exclusive features");
#[cfg(all(feature = "host", not(any(feature = "vial", feature = "rmk_protocol"))))]
compile_error!("`host` requires enabling either `vial` or `rmk_protocol`.");
#[cfg(all(feature = "rmk_protocol", feature = "_ble", feature = "_no_usb"))]
compile_error!("`rmk_protocol` over BLE-only (no USB) is not yet supported.");

#[cfg(all(feature = "host", not(feature = "_no_usb")))]
use embassy_usb::{driver::Driver, Builder};
#[cfg(all(feature = "host", not(feature = "_no_usb"), feature = "rmk_protocol"))]
use embassy_sync::mutex::Mutex;
#[cfg(all(feature = "host", not(feature = "_no_usb"), feature = "vial"))]
use {crate::descriptor::ViaReport, embassy_usb::class::hid::HidReaderWriter};
#[cfg(feature = "host")]
use crate::{config::RmkConfig, keymap::KeyMap};
#[cfg(feature = "vial")]
use crate::config::VialConfig;
#[cfg(feature = "host")]
use crate::hid::{HidReaderTrait, HidWriterTrait};
#[cfg(all(feature = "host", feature = "_ble"))]
use trouble_host::prelude::{GattConnection, PacketPool};

pub(crate) trait HostService {
    async fn run(&mut self);
}

#[cfg(all(feature = "host", not(feature = "_no_usb"), feature = "vial"))]
pub(crate) struct UsbHostTransport<'d, D>
where
    D: Driver<'d>,
{
    reader_writer: HidReaderWriter<'d, D, 32, 32>,
}

#[cfg(all(feature = "host", not(feature = "_no_usb"), feature = "vial"))]
impl<D> UsbHostTransport<'static, D>
where
    D: Driver<'static>,
{
    pub(crate) fn new(builder: &mut Builder<'static, D>) -> Self {
        Self {
            reader_writer: crate::usb::add_usb_reader_writer!(builder, ViaReport, 32, 32),
        }
    }
}

#[cfg(all(feature = "host", not(feature = "_no_usb"), feature = "rmk_protocol"))]
pub(crate) struct UsbHostTransport<'d, D>
where
    D: Driver<'d>,
{
    tx_state: Mutex<crate::RawMutex, protocol::transport::UsbBulkTxState<'d, D>>,
    tx_connected: embassy_sync::signal::Signal<crate::RawMutex, ()>,
    rx: D::EndpointOut,
}

#[cfg(all(feature = "host", not(feature = "_no_usb"), feature = "rmk_protocol"))]
impl<'d, D> UsbHostTransport<'d, D>
where
    D: Driver<'d>,
{
    pub(crate) fn new(builder: &mut Builder<'d, D>) -> Self {
        let (ep_in, rx) = protocol::transport::add_usb_bulk_interface(builder);
        Self {
            tx_state: Mutex::new(protocol::transport::UsbBulkTxState::new(ep_in)),
            tx_connected: embassy_sync::signal::Signal::new(),
            rx,
        }
    }
}

#[cfg(all(feature = "host", not(feature = "_no_usb"), feature = "vial"))]
pub(crate) struct UsbHostService<'s, 'a, 'd, D>(
    via::VialService<'a, via::UsbVialReaderWriter<'s, 'd, D>>,
)
where
    D: Driver<'d>;

#[cfg(all(feature = "host", not(feature = "_no_usb"), feature = "vial"))]
impl<'s, 'a, 'd, D> UsbHostService<'s, 'a, 'd, D>
where
    D: Driver<'d>,
{
    pub(crate) fn new(
        keymap: &'a KeyMap<'a>,
        transport: &'s mut UsbHostTransport<'d, D>,
        rmk_config: &RmkConfig<'static>,
    ) -> Self {
        Self(via::VialService::new(
            keymap,
            rmk_config.vial_config,
            via::UsbVialReaderWriter::new(&mut transport.reader_writer),
        ))
    }
}

#[cfg(all(feature = "host", not(feature = "_no_usb"), feature = "vial"))]
impl<'s, 'a, 'd, D> HostService for UsbHostService<'s, 'a, 'd, D>
where
    D: Driver<'d>,
{
    async fn run(&mut self) {
        self.0.run().await;
    }
}

#[cfg(all(feature = "host", not(feature = "_no_usb"), feature = "rmk_protocol"))]
pub(crate) struct UsbHostService<'s, 'a, 'd, D>(
    protocol::ProtocolService<
        'a,
        protocol::transport::UsbBulkTx<'s, 'd, D>,
        protocol::transport::UsbBulkRx<'s, 'd, D>,
    >,
)
where
    D: Driver<'d>;

#[cfg(all(feature = "host", not(feature = "_no_usb"), feature = "rmk_protocol"))]
impl<'s, 'a, 'd, D> UsbHostService<'s, 'a, 'd, D>
where
    D: Driver<'d>,
{
    pub(crate) fn new(
        keymap: &'a KeyMap<'a>,
        transport: &'s mut UsbHostTransport<'d, D>,
        rmk_config: &RmkConfig<'static>,
    ) -> Self {
        let _ = rmk_config;
        Self(protocol::ProtocolService::new(
            keymap,
            protocol::transport::UsbBulkTx::new(&transport.tx_state, &transport.tx_connected),
            protocol::transport::UsbBulkRx::new(&mut transport.rx, &transport.tx_connected),
        ))
    }
}

#[cfg(all(feature = "host", not(feature = "_no_usb"), feature = "rmk_protocol"))]
impl<'s, 'a, 'd, D> HostService for UsbHostService<'s, 'a, 'd, D>
where
    D: Driver<'d>,
{
    async fn run(&mut self) {
        self.0.run().await;
    }
}

#[cfg(all(feature = "host", feature = "_ble", feature = "vial"))]
pub(crate) struct BleHostService<'stack, 'server, 'conn, 'a, P>(
    via::VialService<'a, crate::ble::host_service::BleHostServer<'stack, 'server, 'conn, P>>,
)
where
    P: PacketPool;

#[cfg(all(feature = "host", feature = "_ble", feature = "vial"))]
impl<'stack, 'server, 'conn, 'a, P> BleHostService<'stack, 'server, 'conn, 'a, P>
where
    P: PacketPool,
{
    pub(crate) fn new(
        keymap: &'a KeyMap<'a>,
        server: &crate::ble::ble_server::Server<'_>,
        conn: &'conn GattConnection<'stack, 'server, P>,
        rmk_config: &RmkConfig<'static>,
    ) -> Self {
        Self(via::VialService::new(
            keymap,
            rmk_config.vial_config,
            crate::ble::host_service::BleHostServer::new(server, conn),
        ))
    }
}

#[cfg(all(feature = "host", feature = "_ble", feature = "vial"))]
impl<'stack, 'server, 'conn, 'a, P> HostService for BleHostService<'stack, 'server, 'conn, 'a, P>
where
    P: PacketPool,
{
    async fn run(&mut self) {
        self.0.run().await;
    }
}

#[cfg(all(feature = "host", feature = "_ble", not(feature = "vial")))]
pub(crate) struct BleHostService<'stack, 'server, 'conn, 'a, P>(
    PhantomData<(&'a (), &'conn GattConnection<'stack, 'server, P>)>,
)
where
    P: PacketPool;

#[cfg(all(feature = "host", feature = "_ble", not(feature = "vial")))]
impl<'stack, 'server, 'conn, 'a, P> BleHostService<'stack, 'server, 'conn, 'a, P>
where
    P: PacketPool,
{
    pub(crate) fn new(
        keymap: &'a KeyMap<'a>,
        server: &crate::ble::ble_server::Server<'_>,
        conn: &'conn GattConnection<'stack, 'server, P>,
        rmk_config: &RmkConfig<'static>,
    ) -> Self {
        let _ = (keymap, server, conn, rmk_config);
        Self(PhantomData)
    }
}

#[cfg(all(feature = "host", feature = "_ble", not(feature = "vial")))]
impl<'stack, 'server, 'conn, 'a, P> HostService for BleHostService<'stack, 'server, 'conn, 'a, P>
where
    P: PacketPool,
{
    async fn run(&mut self) {
        // TODO: BLE transport for rmk_protocol is not yet implemented.
        warn!("BLE host protocol transport not yet implemented");
        core::future::pending::<()>().await;
    }
}

#[cfg(feature = "vial")]
pub(crate) async fn run_host_communicate_task<
    'a,
    Rw: HidReaderTrait<ReportType = crate::descriptor::ViaReport>
        + HidWriterTrait<ReportType = crate::descriptor::ViaReport>,
>(
    keymap: &'a KeyMap<'a>,
    reader_writer: Rw,
    vial_config: VialConfig<'static>,
) {
    let mut service = via::VialService::new(keymap, vial_config, reader_writer);
    service.run().await
}

#[cfg(not(feature = "vial"))]
pub(crate) async fn run_host_communicate_task<
    'a,
    Rw: HidReaderTrait<ReportType = crate::descriptor::ViaReport>
        + HidWriterTrait<ReportType = crate::descriptor::ViaReport>,
>(
    _keymap: &'a KeyMap<'a>,
    _reader_writer: Rw,
) {
    todo!()
}
