#[cfg(feature = "rmk_protocol")]
pub(crate) mod protocol;
#[cfg(feature = "storage")]
pub(crate) mod storage;
pub mod via;

pub use via::UsbVialReaderWriter as UsbHostReaderWriter;

/// Unified trait for host communication services (Vial, RMK Protocol, etc.).
///
/// Both `VialService` and `ProtocolService` implement this trait, allowing
/// `run_keyboard` to accept any host service without protocol-specific parameters.
pub(crate) trait HostService {
    async fn run(&mut self);
}

/// No-op host service for when no protocol is enabled.
pub(crate) struct PendingHostService;

impl HostService for PendingHostService {
    async fn run(&mut self) {
        core::future::pending::<()>().await
    }
}
