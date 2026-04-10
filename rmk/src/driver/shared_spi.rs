//! Shared SPI Bus wrapper using RefCell
//!
//! Allows multiple devices (e.g., shift register + sensor) to share
//! a single SPI bus safely in a single-threaded async environment.
//!
//! # Example
//! ```rust,ignore
//! use core::cell::RefCell;
//! use static_cell::StaticCell;
//!
//! // Create the shared bus
//! static SPI_BUS: StaticCell<RefCell<BitBangSpiBus<_, _>>> = StaticCell::new();
//! let spi = BitBangSpiBus::new(sck, sdio);
//! let spi_bus = SPI_BUS.init(RefCell::new(spi));
//!
//! // Create per-device handles
//! let spi_for_sr = SharedSpiBus::new(spi_bus);
//! let spi_for_sensor = SharedSpiBus::new(spi_bus);
//! ```

use core::cell::RefCell;

use embedded_hal::spi::ErrorType;
use embedded_hal_async::spi::SpiBus;

/// A shared SPI bus wrapper backed by `RefCell`.
///
/// Each device that shares the bus gets its own `SharedSpiBus` instance
/// pointing to the same `RefCell<SPI>`.  Because embassy tasks are
/// cooperatively scheduled on a single thread, the `RefCell` borrow
/// will never conflict at runtime.
pub struct SharedSpiBus<'a, SPI: SpiBus> {
    inner: &'a RefCell<SPI>,
}

impl<'a, SPI: SpiBus> SharedSpiBus<'a, SPI> {
    /// Wrap an existing `RefCell<SPI>` to create a shared handle.
    pub fn new(inner: &'a RefCell<SPI>) -> Self {
        Self { inner }
    }
}

impl<SPI: SpiBus> ErrorType for SharedSpiBus<'_, SPI> {
    type Error = SPI::Error;
}

impl<SPI: SpiBus> SpiBus for SharedSpiBus<'_, SPI> {
    async fn read(&mut self, words: &mut [u8]) -> Result<(), Self::Error> {
        self.inner.borrow_mut().read(words).await
    }

    async fn write(&mut self, words: &[u8]) -> Result<(), Self::Error> {
        self.inner.borrow_mut().write(words).await
    }

    async fn transfer(&mut self, read: &mut [u8], write: &[u8]) -> Result<(), Self::Error> {
        self.inner.borrow_mut().transfer(read, write).await
    }

    async fn transfer_in_place(&mut self, words: &mut [u8]) -> Result<(), Self::Error> {
        self.inner.borrow_mut().transfer_in_place(words).await
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        self.inner.borrow_mut().flush().await
    }
}
