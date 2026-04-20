pub mod rynk;
#[cfg(feature = "storage")]
pub(crate) mod storage;
pub mod via;

// TODO: Remove those aliases
pub use via::UsbVialReaderWriter as UsbHostReaderWriter;
#[cfg(feature = "vial")]
pub(crate) use via::VialService;

#[cfg(feature = "vial")]
use crate::config::VialConfig;
use crate::descriptor::ViaReport;
use crate::hid::{HidReaderTrait, HidWriterTrait};
use crate::keymap::KeyMap;

/// The abstraction of host service.
///
/// The host service communicates to the host:
/// 1. it reads messages from host via USB Hid/USB Serial/BLE
/// 2. it updates keymaps and configurations according the message
/// 3. it can also execute some commands from host
/// 4. it syncs keyboard's state/info to the host
///
/// The host should be abstraction to two levels:
/// 1. The protocol level:
///     - Via/Vial Protocol
///     - Rynk Protocol
/// 2. The transport level:
///     - USB Hid raw packets(Vial)
///     - USB Serial/BLE(postcard-rpc)
trait HostService {
    // TODO: Finalize the trait
}

#[cfg(feature = "vial")]
pub(crate) async fn run_host_communicate_task<
    'a,
    Rw: HidReaderTrait<ReportType = ViaReport> + HidWriterTrait<ReportType = ViaReport>,
>(
    keymap: &'a KeyMap<'a>,
    reader_writer: Rw,
    vial_config: VialConfig<'static>,
) {
    let mut service = VialService::new(keymap, vial_config, reader_writer);
    service.run().await
}

#[cfg(not(feature = "vial"))]
pub(crate) async fn run_host_communicate_task<
    'a,
    Rw: HidReaderTrait<ReportType = ViaReport> + HidWriterTrait<ReportType = ViaReport>,
>(
    _keymap: &'a KeyMap<'a>,
    _reader_writer: Rw,
) {
    todo!()
}
