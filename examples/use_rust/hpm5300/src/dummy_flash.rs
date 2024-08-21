
pub struct DummyFlash {}

impl embedded_storage_async::nor_flash::NorFlash for DummyFlash {
    const WRITE_SIZE: usize = 32;

    const ERASE_SIZE: usize = 32;

    async fn erase(&mut self, _from: u32, _to: u32) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn write(&mut self, _offset: u32, _bytes: &[u8]) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl embedded_storage_async::nor_flash::ReadNorFlash for DummyFlash {
    const READ_SIZE: usize = 32;

    async fn read(&mut self, _offset: u32, _bytes: &mut [u8]) -> Result<(), Self::Error> {
        Ok(())
    }

    fn capacity(&self) -> usize {
        0
    }
}

impl embedded_storage_async::nor_flash::ErrorType for DummyFlash {
    type Error = DummyFlashError;
}

#[derive(Debug)]
pub struct DummyFlashError {}

impl embedded_storage_async::nor_flash::NorFlashError for DummyFlashError {
    fn kind(&self) -> embedded_storage_async::nor_flash::NorFlashErrorKind {
        embedded_storage_async::nor_flash::NorFlashErrorKind::Other
    }
}