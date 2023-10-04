use embedded_storage::Storage;
use log::{error, info, warn};

/// Eeprom based on any storage device which implements `embedded-storage::Storage` trait
/// Ref: https://www.st.com/resource/en/application_note/an4894-how-to-use-eeprom-emulation-on-stm32-mcus-stmicroelectronics.pdf
/// Zh Ref: https://www.st.com/resource/zh/application_note/an3969-eeprom-emulation-in-stm32f40xstm32f41x-microcontrollers-stmicroelectronics.pdf
/// Data in eeprom is saved in a 4-byte `record`, with 2-byte address in the first 16 bits and 2-byte data in the next 16 bits.
/// Eeprom struct maintains a cache in ram to speed up reads, whose size is same as the logical eeprom capacity.
/// User can specify the size of the logical size of eeprom(maximum 64KB), Eeprom struct maintains a cache in ram to speed up reads, whose size is same as the user defined logical eeprom capacity.
pub struct Eeprom<F, const STORAGE_PAGE_SIZE: u32, const EEPROM_SIZE: usize>
where
    F: Storage,
{
    /// Current position in the storage
    pos: u32,
    /// Backend storage, implements `embedded-storage::Storage` trait
    storage: F,
    /// A eeprom cache in ram to speed up reads, whose size is same as the logical eeprom capacity
    cache: [u8; EEPROM_SIZE],
}

/// A record in the eeprom, with 2-byte address and 2-byte data
/// A record is 4-byte long, so the tracking pos in the `Eeprom` implementation must be a multiple of 4
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

impl<F: Storage, const STORAGE_PAGE_SIZE: u32, const EEPROM_SIZE: usize>
    Eeprom<F, STORAGE_PAGE_SIZE, EEPROM_SIZE>
{
    pub fn write_byte(&mut self, mut address: u16, data: &mut [u8]) {
        if data.len() == 0 {
            warn!("No data to write to eeprom, skip");
            return;
        }
        // Check address
        if address as usize >= EEPROM_SIZE {
            error!("Invalid address");
            return;
        }

        let mut start_idx = 0;
        // If the address is even, we have to append the first byte.
        if address % 2 != 0 {
            let prev_byte = self.cache[address as usize - 1];
            self.write_record(EepromRecord {
                address,
                data: ((prev_byte as u16) << 8) | (data[0] as u16),
            });
            start_idx = 1;
        }

        for i in (start_idx..data.len()).step_by(2) {
            if i + 1 == data.len() {
                // Last byte, append 0xFF
                self.write_record(EepromRecord {
                    address,
                    data: (0xFF << 8) | (data[i] as u16)
                });
            } else {
                self.write_record(EepromRecord {
                    address,
                    data: ((data[i + 1] as u16) << 8) | (data[i] as u16),
                });
            }
            address += 2;
        }
    }

    /// Read bytes from eeprom, starting from the given address, and reading `read_size` bytes.
    /// Returns a slice of eeprom cache, which is immutable
    pub fn read_byte(&mut self, address: u16, read_size: usize) -> &[u8] {
        &self.cache[address as usize..(address as usize + read_size)]
    }

    fn write_record(&mut self, record: EepromRecord) {
        if self.pos + 4 > STORAGE_PAGE_SIZE {
            info!("Backend storage is full, consolidating records");
            self.consolidate_records();
            // Check position again
            if self.pos + 4 > STORAGE_PAGE_SIZE {
                error!("Backend storage is full, failed to write record");
                return;
            }
        }
        match self.storage.write(self.pos, &record.to_bytes()) {
            Ok(_) => self.pos += 4,
            Err(_) => error!("Failed to write record to storage"),
        }
    }

    /// Read a eeprom record at the given address from the storage
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

    fn consolidate_records(&mut self) {
        todo!()
    }
}
