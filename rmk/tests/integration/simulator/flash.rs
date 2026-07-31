//! An in-memory NOR flash part for storage-round-trip tests.
//!
//! Cloning shares the same bytes, so a second `build_with_flash` over a clone
//! reads back what the first keyboard persisted — the harness's stand-in for a
//! power cycle.

use std::cell::RefCell;
use std::rc::Rc;

use embedded_storage::nor_flash::{
    ErrorType, NorFlash, NorFlashErrorKind, ReadNorFlash, check_erase, check_read, check_write,
};

/// Geometry of the simulated part: 4 KiB in 256-byte sectors, written 4 bytes
/// at a time.
const SIZE: usize = 4096;
const ERASE: usize = 256;
const WRITE: usize = 4;

#[derive(Clone)]
pub struct InMemoryFlash {
    data: Rc<RefCell<[u8; SIZE]>>,
}

impl InMemoryFlash {
    pub fn new() -> Self {
        Self {
            data: Rc::new(RefCell::new([0xFF; SIZE])),
        }
    }
}

impl ErrorType for InMemoryFlash {
    type Error = NorFlashErrorKind;
}

impl ReadNorFlash for InMemoryFlash {
    const READ_SIZE: usize = 1;

    fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        check_read(self, offset, bytes.len())?;
        let offset = offset as usize;
        bytes.copy_from_slice(&self.data.borrow()[offset..offset + bytes.len()]);
        Ok(())
    }

    fn capacity(&self) -> usize {
        SIZE
    }
}

impl NorFlash for InMemoryFlash {
    const WRITE_SIZE: usize = WRITE;
    const ERASE_SIZE: usize = ERASE;

    fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> {
        check_erase(self, from, to)?;
        self.data.borrow_mut()[from as usize..to as usize].fill(0xFF);
        Ok(())
    }

    fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
        check_write(self, offset, bytes.len())?;
        let mut data = self.data.borrow_mut();
        let offset = offset as usize;
        for (current, byte) in data[offset..offset + bytes.len()].iter_mut().zip(bytes) {
            // Real NOR only clears bits; writing a 1 over a 0 needs an erase first.
            if *current & *byte != *byte {
                return Err(NorFlashErrorKind::Other);
            }
            *current &= *byte;
        }
        Ok(())
    }
}
