#[cfg(feature = "storage")]
pub(crate) mod storage;
pub mod via;

// TODO: Remove those aliases
pub use via::UsbVialReaderWriter as UsbHostReaderWriter;
#[cfg(feature = "vial")]
pub(crate) use via::VialService as HostService;

#[cfg(feature = "vial")]
use crate::config::VialConfig;
use crate::hid::{HidReaderTrait, HidWriterTrait, ViaReport};
use crate::keymap::KeyMap;

#[cfg(feature = "vial")]
pub(crate) async fn run_host_communicate_task<
    'a,
    Rw: HidReaderTrait<ReportType = ViaReport> + HidWriterTrait<ReportType = ViaReport>,
>(
    keymap: &'a KeyMap<'a>,
    reader_writer: Rw,
    vial_config: VialConfig<'static>,
) {
    let mut service = HostService::new(keymap, vial_config, reader_writer);
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
