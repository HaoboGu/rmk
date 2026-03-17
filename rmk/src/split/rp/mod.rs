pub mod uart;

use embedded_io_async::{ErrorType, Read, Write};

/// Wrapper that bridges `embedded-io-async` 0.6 Read/Write to 0.7.
///
/// Use this to wrap `embassy_rp::uart::BufferedUart` (which implements 0.6 traits)
/// so it can be passed to rmk split functions that require 0.7 traits.
pub struct BufferedUartWrapper<T>(pub T);

#[derive(Debug)]
pub struct WrapperError;

impl core::fmt::Display for WrapperError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "BufferedUartWrapper IO error")
    }
}

impl core::error::Error for WrapperError {}

impl embedded_io_async::Error for WrapperError {
    fn kind(&self) -> embedded_io_async::ErrorKind {
        embedded_io_async::ErrorKind::Other
    }
}

impl<T> ErrorType for BufferedUartWrapper<T> {
    type Error = WrapperError;
}

impl<T: embedded_io_async_06::Read> Read for BufferedUartWrapper<T> {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        embedded_io_async_06::Read::read(&mut self.0, buf)
            .await
            .map_err(|_| WrapperError)
    }
}

impl<T: embedded_io_async_06::Write> Write for BufferedUartWrapper<T> {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        embedded_io_async_06::Write::write(&mut self.0, buf)
            .await
            .map_err(|_| WrapperError)
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        embedded_io_async_06::Write::flush(&mut self.0)
            .await
            .map_err(|_| WrapperError)
    }
}
