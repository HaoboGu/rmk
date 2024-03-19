use embedded_storage_async::nor_flash::{
    ErrorType, NorFlash, NorFlashError, NorFlashErrorKind, ReadNorFlash,
};

#[derive(Debug)]
pub struct EmptyFlashErrorWrapper {}

impl NorFlashError for EmptyFlashErrorWrapper {
    fn kind(&self) -> embedded_storage_async::nor_flash::NorFlashErrorKind {
        NorFlashErrorKind::Other
    }
}

/// An empty implementation of `NorFlash`
pub struct EmptyFlashWrapper {}

impl EmptyFlashWrapper {
    pub fn new() -> Self {
        Self {}
    }
}

impl ErrorType for EmptyFlashWrapper {
    type Error = EmptyFlashErrorWrapper;
}

impl NorFlash for EmptyFlashWrapper {
    const WRITE_SIZE: usize = 0;
    const ERASE_SIZE: usize = 0;

    async fn erase(&mut self, _from: u32, _to: u32) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn write(&mut self, _offset: u32, _bytes: &[u8]) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl ReadNorFlash for EmptyFlashWrapper {
    const READ_SIZE: usize = 1;
    async fn read(&mut self, _offset: u32, _bytes: &mut [u8]) -> Result<(), Self::Error> {
        Ok(())
    }

    fn capacity(&self) -> usize {
        0
    }
}
