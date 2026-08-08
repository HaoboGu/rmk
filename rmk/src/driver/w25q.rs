use embedded_hal::digital::OutputPin;
use embedded_hal::spi::SpiBus;
use embedded_storage::nor_flash::{
    ErrorType, MultiwriteNorFlash, NorFlash, NorFlashError, NorFlashErrorKind, ReadNorFlash,
};

const CMD_READ: u8 = 0x03;
const CMD_PAGE_PROGRAM: u8 = 0x02;
const CMD_WRITE_ENABLE: u8 = 0x06;
const CMD_READ_STATUS: u8 = 0x05;
const CMD_SECTOR_ERASE: u8 = 0x20;
const CMD_BLOCK_ERASE_64K: u8 = 0xD8;

const PAGE_SIZE: u32 = 256;
const SECTOR_SIZE: u32 = 4096;
const BLOCK64_SIZE: u32 = 65536;

/// [`NorFlash`] implementation for W25Q and compatible 25-series SPI NOR flash
/// chips. Uses standard JEDEC commands valid across Winbond, Macronix, ISSI,
/// and similar families.
pub struct W25qNorFlash<BUS: SpiBus, CS: OutputPin> {
    bus: BUS,
    cs: CS,
    flash_size: u32,
}

impl<BUS: SpiBus, CS: OutputPin> W25qNorFlash<BUS, CS> {
    pub fn new(bus: BUS, mut cs: CS, flash_size: u32) -> Self {
        cs.set_high().ok();
        Self { bus, cs, flash_size }
    }

    fn wait_wip(&mut self) -> Result<(), W25qError<BUS::Error>> {
        loop {
            self.cs.set_low().ok();
            let res = self.bus.write(&[CMD_READ_STATUS]);
            if res.is_err() {
                self.cs.set_high().ok();
                return Err(W25qError::Spi(res.unwrap_err()));
            }
            let mut status = [0u8; 1];
            let res = self.bus.read(&mut status);
            self.cs.set_high().ok();
            res.map_err(W25qError::Spi)?;
            if status[0] & 0x01 == 0 {
                return Ok(());
            }
        }
    }

    fn write_enable(&mut self) -> Result<(), W25qError<BUS::Error>> {
        self.cs.set_low().ok();
        let res = self.bus.write(&[CMD_WRITE_ENABLE]).map_err(W25qError::Spi);
        self.cs.set_high().ok();
        res
    }

    fn read_data(&mut self, addr: u32, buf: &mut [u8]) -> Result<(), W25qError<BUS::Error>> {
        self.wait_wip()?;
        let cmd = [CMD_READ, (addr >> 16) as u8, (addr >> 8) as u8, addr as u8];
        self.cs.set_low().ok();
        let res = self.bus.write(&cmd);
        if res.is_err() {
            self.cs.set_high().ok();
            return Err(W25qError::Spi(res.unwrap_err()));
        }
        let res = self.bus.read(buf).map_err(W25qError::Spi);
        self.cs.set_high().ok();
        res
    }

    fn page_program(&mut self, addr: u32, data: &[u8]) -> Result<(), W25qError<BUS::Error>> {
        self.write_enable()?;
        let cmd = [CMD_PAGE_PROGRAM, (addr >> 16) as u8, (addr >> 8) as u8, addr as u8];
        self.cs.set_low().ok();
        let res = self.bus.write(&cmd);
        if res.is_err() {
            self.cs.set_high().ok();
            return Err(W25qError::Spi(res.unwrap_err()));
        }
        let res = self.bus.write(data).map_err(W25qError::Spi);
        self.cs.set_high().ok();
        res
    }

    fn sector_erase(&mut self, addr: u32) -> Result<(), W25qError<BUS::Error>> {
        self.write_enable()?;
        let cmd = [CMD_SECTOR_ERASE, (addr >> 16) as u8, (addr >> 8) as u8, addr as u8];
        self.cs.set_low().ok();
        let res = self.bus.write(&cmd).map_err(W25qError::Spi);
        self.cs.set_high().ok();
        res
    }

    fn block_erase_64k(&mut self, addr: u32) -> Result<(), W25qError<BUS::Error>> {
        self.write_enable()?;
        let cmd = [CMD_BLOCK_ERASE_64K, (addr >> 16) as u8, (addr >> 8) as u8, addr as u8];
        self.cs.set_low().ok();
        let res = self.bus.write(&cmd).map_err(W25qError::Spi);
        self.cs.set_high().ok();
        res
    }
}

#[derive(Debug)]
pub enum W25qError<SPI: embedded_hal::spi::Error> {
    Spi(SPI),
}

impl<SPI: embedded_hal::spi::Error + core::fmt::Debug> core::fmt::Display for W25qError<SPI> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            W25qError::Spi(e) => write!(f, "SPI error: {:?}", e),
        }
    }
}

impl<SPI: embedded_hal::spi::Error> NorFlashError for W25qError<SPI> {
    fn kind(&self) -> NorFlashErrorKind {
        NorFlashErrorKind::Other
    }
}

impl<BUS: SpiBus, CS: OutputPin> ErrorType for W25qNorFlash<BUS, CS> {
    type Error = W25qError<BUS::Error>;
}

impl<BUS: SpiBus, CS: OutputPin> ReadNorFlash for W25qNorFlash<BUS, CS> {
    const READ_SIZE: usize = 1;

    fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        self.read_data(offset, bytes)
    }

    fn capacity(&self) -> usize {
        self.flash_size as usize
    }
}

impl<BUS: SpiBus, CS: OutputPin> NorFlash for W25qNorFlash<BUS, CS> {
    const WRITE_SIZE: usize = 1;
    const ERASE_SIZE: usize = SECTOR_SIZE as usize;

    fn erase(&mut self, mut from: u32, to: u32) -> Result<(), Self::Error> {
        while from < to {
            self.wait_wip()?;
            let remaining = to - from;
            if remaining >= BLOCK64_SIZE && from % BLOCK64_SIZE == 0 {
                self.block_erase_64k(from)?;
                from += BLOCK64_SIZE;
            } else {
                self.sector_erase(from)?;
                from += SECTOR_SIZE;
            }
        }
        Ok(())
    }

    fn write(&mut self, mut offset: u32, mut bytes: &[u8]) -> Result<(), Self::Error> {
        while !bytes.is_empty() {
            self.wait_wip()?;
            let page_offset = offset & (PAGE_SIZE - 1);
            let chunk = bytes.len().min((PAGE_SIZE - page_offset) as usize);
            self.page_program(offset, &bytes[..chunk])?;
            offset += chunk as u32;
            bytes = &bytes[chunk..];
        }
        Ok(())
    }
}

impl<BUS: SpiBus, CS: OutputPin> MultiwriteNorFlash for W25qNorFlash<BUS, CS> {}
