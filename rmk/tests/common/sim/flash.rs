use std::sync::{Arc, Mutex};
use std::vec::Vec;

use embedded_storage::nor_flash::{
    ErrorType, NorFlash, NorFlashError, NorFlashErrorKind, ReadNorFlash, check_erase, check_read, check_write,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InMemoryFlashError {
    OutOfBounds,
    NotAligned,
    WriteRequiresErase,
    Poisoned,
}

impl NorFlashError for InMemoryFlashError {
    fn kind(&self) -> NorFlashErrorKind {
        match self {
            Self::OutOfBounds => NorFlashErrorKind::OutOfBounds,
            Self::NotAligned => NorFlashErrorKind::NotAligned,
            Self::WriteRequiresErase | Self::Poisoned => NorFlashErrorKind::Other,
        }
    }
}

#[derive(Debug, Clone)]
pub struct InMemoryFlash<const SIZE: usize, const ERASE: usize = 4096, const WRITE: usize = 4> {
    data: Arc<Mutex<Vec<u8>>>,
}

impl<const SIZE: usize, const ERASE: usize, const WRITE: usize> Default for InMemoryFlash<SIZE, ERASE, WRITE> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const SIZE: usize, const ERASE: usize, const WRITE: usize> InMemoryFlash<SIZE, ERASE, WRITE> {
    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(std::vec![0xFF; SIZE])),
        }
    }

    fn map_error(kind: NorFlashErrorKind) -> InMemoryFlashError {
        match kind {
            NorFlashErrorKind::OutOfBounds => InMemoryFlashError::OutOfBounds,
            NorFlashErrorKind::NotAligned => InMemoryFlashError::NotAligned,
            _ => InMemoryFlashError::Poisoned,
        }
    }
}

impl<const SIZE: usize, const ERASE: usize, const WRITE: usize> ErrorType for InMemoryFlash<SIZE, ERASE, WRITE> {
    type Error = InMemoryFlashError;
}

impl<const SIZE: usize, const ERASE: usize, const WRITE: usize> ReadNorFlash for InMemoryFlash<SIZE, ERASE, WRITE> {
    const READ_SIZE: usize = 1;

    fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        check_read(self, offset, bytes.len()).map_err(Self::map_error)?;
        let data = self.data.lock().map_err(|_| InMemoryFlashError::Poisoned)?;
        let offset = offset as usize;
        bytes.copy_from_slice(&data[offset..offset + bytes.len()]);
        Ok(())
    }

    fn capacity(&self) -> usize {
        SIZE
    }
}

impl<const SIZE: usize, const ERASE: usize, const WRITE: usize> NorFlash for InMemoryFlash<SIZE, ERASE, WRITE> {
    const WRITE_SIZE: usize = WRITE;
    const ERASE_SIZE: usize = ERASE;

    fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> {
        check_erase(self, from, to).map_err(Self::map_error)?;
        let mut data = self.data.lock().map_err(|_| InMemoryFlashError::Poisoned)?;
        data[from as usize..to as usize].fill(0xFF);
        Ok(())
    }

    fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
        check_write(self, offset, bytes.len()).map_err(Self::map_error)?;
        let mut data = self.data.lock().map_err(|_| InMemoryFlashError::Poisoned)?;
        let offset = offset as usize;
        for (idx, byte) in bytes.iter().enumerate() {
            let current = data[offset + idx];
            if current & byte != *byte {
                return Err(InMemoryFlashError::WriteRequiresErase);
            }
            data[offset + idx] = current & byte;
        }
        Ok(())
    }
}

impl<const SIZE: usize, const ERASE: usize, const WRITE: usize> embedded_storage_async::nor_flash::ReadNorFlash
    for InMemoryFlash<SIZE, ERASE, WRITE>
{
    const READ_SIZE: usize = 1;

    async fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        ReadNorFlash::read(self, offset, bytes)
    }

    fn capacity(&self) -> usize {
        SIZE
    }
}

impl<const SIZE: usize, const ERASE: usize, const WRITE: usize> embedded_storage_async::nor_flash::NorFlash
    for InMemoryFlash<SIZE, ERASE, WRITE>
{
    const WRITE_SIZE: usize = WRITE;
    const ERASE_SIZE: usize = ERASE;

    async fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> {
        NorFlash::erase(self, from, to)
    }

    async fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
        NorFlash::write(self, offset, bytes)
    }
}
