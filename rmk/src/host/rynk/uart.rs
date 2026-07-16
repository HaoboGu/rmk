//! Rynk over a bare UART (or any other byte stream that implements
//! [`embedded_io_async::Read`] + [`Write`]).
//!
//! Peripheral-agnostic. The user provides pre-split read and write halves
//! (typically `embassy_*::usart::Uart::split()` or the split UART driver
//! at `rmk/src/split/rp/uart.rs`) and a `RynkService`.
//!
//! UART has no inherent "connected" notion, so the loop simply restarts
//! the session on every read/write error — effectively swallowing transient
//! errors at the cost of dropping any in-flight frame.

use embedded_io_async::{Read, Write};

use super::RynkService;

/// Drive `service` over a UART link forever, restarting the framing
/// session on every read/write error.
pub async fn run_rynk_uart<R, W>(mut rx: R, mut tx: W, service: &RynkService<'_>) -> !
where
    R: Read,
    W: Write,
{
    loop {
        service.run_session(&mut rx, &mut tx).await;
    }
}
