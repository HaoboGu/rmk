use core::fmt::Error;

use embedded_storage::Storage;
use log::error;

/// Eeprom based on any storage device which implements `embedded-storage::Storage` trait
/// Ref: https://www.st.com/resource/en/application_note/an4894-how-to-use-eeprom-emulation-on-stm32-mcus-stmicroelectronics.pdf
/// Zh Ref: https://www.st.com/resource/zh/application_note/an3969-eeprom-emulation-in-stm32f40xstm32f41x-microcontrollers-stmicroelectronics.pdf
/// Data in eeprom is saved in a 4-byte `record`, with 2-byte address in the first 16 bits and 2-byte data in the next 16 bits.
/// Hence, the logical capacity of the eeprom is 64KB.
pub struct Eeprom<F, const STORAGE_PAGE_SIZE: usize>
where
    F: Storage,
{
    /// Current position in the storage
    pos: u32,
    storage: F,
}

pub struct EepromRecord {
    address: u16,
    data: u16,
}

impl EepromRecord {
    fn to_bytes(&self) -> [u8; 4] {
        let mut bytes = [0u8; 4];
        bytes[0..2].copy_from_slice(&self.address.to_be_bytes());
        bytes[2..4].copy_from_slice(&self.data.to_be_bytes());
        bytes
    }

    fn from_bytes(bytes: [u8; 4]) -> Self {
        let address = u16::from_be_bytes([bytes[0], bytes[1]]);
        let data = u16::from_be_bytes([bytes[2], bytes[3]]);
        Self { address, data }
    }
}

impl<F: Storage, const STORAGE_PAGE_SIZE: usize> Eeprom<F, STORAGE_PAGE_SIZE> {
    fn write_record(&mut self, record: EepromRecord) {
        match self.storage.write(self.pos, &record.to_bytes()) {
            Ok(_) => self.pos += 4,
            Err(_) => error!("Failed to write record to storage"),
        }
    }

    /// Read a eeprom record at the given address from the storage
    /// TODO: use ram cache to speed up reads?
    fn read_record(&mut self, address: u16) -> Option<EepromRecord> {
        let mut bytes = [0u8; 4];
        // Scan the storage, find the record with the given address
        for p in (0..self.pos).step_by(4) {
            match self.storage.read(p, &mut bytes) {
                Ok(_) => {
                    // Check address
                    let record = EepromRecord::from_bytes(bytes);
                    if record.address == address {
                        return Some(record);
                    }
                }
                Err(_) => todo!(),
            }
        }

        None
    }

    fn build_record(&mut self, address: u16, idx: usize, data: &mut [u8]) -> EepromRecord {
        if idx == data.len() - 1 {
            return EepromRecord {
                address,
                data: data[idx] as u16,
            };
        } else {
            return EepromRecord {
                address,
                data: (data[idx] as u16) << 8 | data[idx - 1] as u16,
            };
        }
    }

    pub fn write_byte(&mut self, address: u16, data: &mut [u8]) {
        match self.storage.write(0, data) {
            Ok(_) => todo!(),
            Err(_) => todo!(),
        }
    }

    pub fn read_byte(&mut self, address: u16, data: &mut [u8]) {
        match self.storage.read(0, data) {
            Ok(_) => todo!(),
            Err(_) => todo!(),
        }
    }
}
