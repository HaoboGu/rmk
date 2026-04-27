use embassy_sync::signal::Signal;
use embassy_time::Timer;

use crate::RawMutex;
use crate::channel::HOST_REQUEST_CHANNEL;
use crate::core_traits::Runnable;
use crate::hid::{HidReaderTrait, HidWriterTrait, ViaReport};

/// Per-connection bridge between a real host transport (USB HID or BLE GATT)
/// and the global `HOST_REQUEST_CHANNEL`. Reads a Vial request from the transport,
/// hands it to `HostService` via the channel together with a private reply slot,
/// awaits the reply, then writes it back to the transport.
pub(crate) struct HostBridge<'a, RW> {
    transport: &'a mut RW,
}

impl<'a, RW> HostBridge<'a, RW> {
    pub(crate) fn new(transport: &'a mut RW) -> Self {
        Self { transport }
    }
}

impl<RW> Runnable for HostBridge<'_, RW>
where
    RW: HidReaderTrait<ReportType = ViaReport> + HidWriterTrait<ReportType = ViaReport>,
{
    async fn run(&mut self) -> ! {
        // The static is monomorphized per `RW`, so concurrent USB+BLE bridges have
        // independent reply slots. Multiple sites that share the same `RW` (e.g. the
        // three USB bridge sites) share the same slot, which is fine: at most one is
        // active at a time, and `reset()` before each send clears any stale value.
        static REPLY: Signal<RawMutex, ViaReport> = Signal::new();
        loop {
            let req = match self.transport.read_report().await {
                Ok(r) => r,
                Err(e) => {
                    error!("Host bridge read error: {:?}", e);
                    Timer::after_millis(100).await;
                    continue;
                }
            };
            REPLY.reset();
            HOST_REQUEST_CHANNEL.send((req, &REPLY)).await;
            let resp = REPLY.wait().await;
            if let Err(e) = self.transport.write_report(resp).await {
                error!("Host bridge write error: {:?}", e);
            }
        }
    }
}
