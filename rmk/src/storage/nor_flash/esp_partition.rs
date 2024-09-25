use core::{
    ffi::{c_void, CStr},
    num::NonZeroI32,
    ops::Deref,
};

use defmt::{info, warn};
use embassy_time::Timer;
use embedded_storage_async::nor_flash::{
    ErrorType, MultiwriteNorFlash, NorFlash, NorFlashError, NorFlashErrorKind, ReadNorFlash,
};
use esp_idf_svc::sys::*;

#[derive(Copy, Clone)]
#[repr(u32)]
pub enum PartitionType {
    App = 0,
    Data = esp_partition_type_t_ESP_PARTITION_TYPE_DATA << 8,
    Custom = 0x4000,
    Any = esp_partition_type_t_ESP_PARTITION_TYPE_ANY << 8,
}
pub struct Partition(*const esp_partition_t);

impl Deref for Partition {
    type Target = esp_partition_t;
    fn deref<'a>(&'a self) -> &'a Self::Target {
        unsafe { self.0.as_ref().unwrap() }
    }
}

impl Partition {
    pub const WORD_SIZE: usize = 4;
    pub const SECTOR_SIZE: usize = 4096;

    pub fn new(type_: PartitionType, label: Option<&CStr>) -> Self {
        let partition = Self(unsafe {
            esp_partition_find_first(
                type_ as u32 >> 8,
                type_ as u32 & 0xff,
                match label {
                    Some(ref v) => v.as_ptr(),
                    None => core::ptr::null(),
                },
            )
        });
        info!(
            "Partition found at: {:#x}, {:#x}",
            partition.address, partition.size
        );
        partition
    }
    fn result(r: esp_err_t) -> Result<(), Error> {
        match NonZeroI32::new(r) {
            None => Ok(()),
            Some(err) => Err(Error(EspError::from_non_zero(err))),
        }
    }
}

#[derive(Debug)]
pub struct Error(EspError);

impl NorFlashError for Error {
    fn kind(&self) -> NorFlashErrorKind {
        match self.0.code() {
            //  => NorFlashErrorKind::NotAligned,
            ESP_ERR_INVALID_SIZE => NorFlashErrorKind::OutOfBounds,
            _ => NorFlashErrorKind::Other,
        }
    }
}

impl ErrorType for Partition {
    type Error = Error;
}

impl ReadNorFlash for Partition {
    const READ_SIZE: usize = Self::WORD_SIZE as _;

    async fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        Self::result(unsafe {
            esp_partition_read_raw(
                self.0,
                offset as usize,
                bytes.as_mut_ptr() as *mut c_void,
                bytes.len(),
            )
        })
    }

    fn capacity(&self) -> usize {
        self.size as usize
    }
}

impl NorFlash for Partition {
    const WRITE_SIZE: usize = Self::WORD_SIZE as _;
    const ERASE_SIZE: usize = Self::SECTOR_SIZE as _;

    async fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
        warn!(
            "esp partition write at {:#x} size {:#x}",
            offset,
            bytes.len()
        );
        Timer::after_millis(7).await;
        Self::result(unsafe {
            esp_partition_write_raw(
                self.0,
                offset as usize,
                bytes.as_ptr() as *mut c_void,
                bytes.len(),
            )
        })
    }

    async fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> {
        warn!("esp partition erase from {:#x} size {:#x}", from, to - from);
        Timer::after_millis(7).await;
        Self::result(unsafe {
            esp_partition_erase_range(self.0, from as usize, (to - from) as usize)
        })
    }
}

impl MultiwriteNorFlash for Partition {}
