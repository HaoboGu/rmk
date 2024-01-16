use core::convert::Infallible;

use embedded_storage::nor_flash::{ErrorType, NorFlash, ReadNorFlash};
// use stm32h7xx_hal::flash::{Error, LockedFlashBank, UnlockedFlashBank};

/// rmk::Keyboard requires `NorFlash` trait to emulate the eeprom, but `stm32h7xx-hal` doesn't implement `NorFlash` for it's internal flash.
// pub struct FlashWrapper {
//     locked_flash: LockedFlashBank,
// }

// impl FlashWrapper {
//     pub fn new(locked: LockedFlashBank) -> Self {
//         Self {
//             locked_flash: locked,
//         }
//     }
// }

// impl ErrorType for FlashWrapper {
//     type Error = Error;
// }

// impl NorFlash for FlashWrapper {
//     const WRITE_SIZE: usize = UnlockedFlashBank::WRITE_SIZE;
//     const ERASE_SIZE: usize = UnlockedFlashBank::ERASE_SIZE;

//     fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> {
//         let mut unlocked = self.locked_flash.unlocked();
//         unlocked.erase(from, to)
//     }

//     fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
//         let mut unlocked = self.locked_flash.unlocked();
//         unlocked.write(offset, bytes)
//     }
// }

// impl ReadNorFlash for FlashWrapper {
//     const READ_SIZE: usize = 1;
//     fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
//         self.locked_flash.read(offset, bytes)
//     }

//     fn capacity(&self) -> usize {
//         self.locked_flash.capacity()
//     }
// }

pub struct DummyFlash {}

impl ErrorType for DummyFlash {
    type Error = Infallible;
}

impl NorFlash for DummyFlash {
    const WRITE_SIZE: usize = 0;
    const ERASE_SIZE: usize = 0;

    fn erase(&mut self, _from: u32, _to: u32) -> Result<(), Self::Error> {
        Ok(())
    }

    fn write(&mut self, _offset: u32, _bytes: &[u8]) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl ReadNorFlash for DummyFlash {
    const READ_SIZE: usize = 1;
    fn read(&mut self, _offset: u32, _bytes: &mut [u8]) -> Result<(), Self::Error> {
        Ok(())
    }

    fn capacity(&self) -> usize {
        0
    }
}
