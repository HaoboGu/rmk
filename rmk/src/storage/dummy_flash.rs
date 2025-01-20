/// An empty implementation of `NorFlash`, which can be used when flash storage is not available
#[derive(Debug)]
pub struct EmptyFlashErrorWrapper {}

impl embedded_storage_async::nor_flash::NorFlashError for EmptyFlashErrorWrapper {
    fn kind(&self) -> embedded_storage_async::nor_flash::NorFlashErrorKind {
        embedded_storage_async::nor_flash::NorFlashErrorKind::Other
    }
}

/// An empty implementation of `NorFlash`
pub struct DummyFlash {}

impl Default for DummyFlash {
    fn default() -> Self {
        Self::new()
    }
}

impl DummyFlash {
    pub fn new() -> Self {
        Self {}
    }
}

impl embedded_storage::nor_flash::NorFlash for DummyFlash {
    const WRITE_SIZE: usize = 0;
    const ERASE_SIZE: usize = 0;

    fn erase(&mut self, _from: u32, _to: u32) -> Result<(), Self::Error> {
        Ok(())
    }

    fn write(&mut self, _offset: u32, _bytes: &[u8]) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl embedded_storage::nor_flash::ReadNorFlash for DummyFlash {
    const READ_SIZE: usize = 0;

    fn read(&mut self, _offset: u32, _bytes: &mut [u8]) -> Result<(), Self::Error> {
        Ok(())
    }

    fn capacity(&self) -> usize {
        0
    }
}

impl embedded_storage_async::nor_flash::ErrorType for DummyFlash {
    type Error = EmptyFlashErrorWrapper;
}

impl embedded_storage_async::nor_flash::NorFlash for DummyFlash {
    const WRITE_SIZE: usize = 0;
    const ERASE_SIZE: usize = 0;

    async fn erase(&mut self, _from: u32, _to: u32) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn write(&mut self, _offset: u32, _bytes: &[u8]) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl embedded_storage_async::nor_flash::ReadNorFlash for DummyFlash {
    const READ_SIZE: usize = 1;
    async fn read(&mut self, _offset: u32, _bytes: &mut [u8]) -> Result<(), Self::Error> {
        Ok(())
    }

    fn capacity(&self) -> usize {
        0
    }
}
