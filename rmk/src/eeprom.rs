pub mod eeconfig;
pub mod eekeymap;

use embedded_storage::nor_flash::NorFlash;
use log::{error, info, warn};

use crate::keymap::KeyMap;

use self::eeconfig::EEPROM_MAGIC;

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

/// Eeprom based on any storage device which implements `embedded-storage::NorFlash` trait
/// Data in eeprom is saved in a 4-byte `record`, with 2-byte address in the first 16 bits and 2-byte data in the next 16 bits.
/// Eeprom struct maintains a cache in ram to speed up reads, whose size is same as the logical eeprom capacity.
/// User can specify the size of the logical size of eeprom(maximum 64KB), Eeprom struct maintains a cache in ram to speed up reads, whose size is same as the user defined logical eeprom capacity.
pub struct Eeprom<
    F: NorFlash,
    const STORAGE_START_ADDR: u32,
    const STORAGE_SIZE: u32,
    const EEPROM_SIZE: usize,
> {
    /// Current position in the storage
    pos: u32,
    /// Backend storage, implements `embedded-storage::NorFlash` trait
    storage: F,
    /// A eeprom cache in ram to speed up reads, whose size is same as the logical eeprom capacity
    cache: [u8; EEPROM_SIZE],
    /// Size for dynamic keymap.
    /// Each key in keymap used 2 bytes, so the size should be at least 2 * NUM_LAYER * ROW * COL.
    ///
    ///  For a 104-key keyboard, with 4 layers, 6 rows and 21 columns, the size is 1008 bytes,
    ///  EEPROM_SIZE should be at least 1008(keymap) + 15(eeconfig) + 100(macro)
    dynamic_keymap_size: usize,
}
impl<
        F: NorFlash,
        const STORAGE_START_ADDR: u32,
        const STORAGE_SIZE: u32,
        const EEPROM_SIZE: usize,
    > Eeprom<F, STORAGE_START_ADDR, STORAGE_SIZE, EEPROM_SIZE>
{
    pub fn new<const ROW: usize, const COL: usize, const NUM_LAYER: usize>(
        storage: F,
        keymap: &KeyMap<ROW, COL, NUM_LAYER>,
    ) -> Self {
        let mut eeprom = Eeprom {
            pos: 0,
            storage,
            cache: [0xFF; EEPROM_SIZE],
            dynamic_keymap_size: ROW * COL * NUM_LAYER * 2,
        };

        // Initialize eeprom using default config
        if eeprom.get_magic() != EEPROM_MAGIC {
            // TODO: support user custom config
            eeprom.init_with_default_config();
            eeprom.set_keymap(keymap);
        }

        // Restore eeprom from storage
        let mut buf: [u8; 4] = [0xFF; 4];
        while eeprom.pos < STORAGE_SIZE {
            match eeprom.storage.read(eeprom.pos, &mut buf) {
                Ok(_) => {
                    let record = EepromRecord::from_bytes(buf);
                    if record.address >= EEPROM_SIZE as u16 {
                        break;
                    }
                    eeprom.cache[record.address as usize] = record.data as u8;
                    eeprom.cache[record.address as usize + 1] = (record.data >> 8) as u8;
                    eeprom.pos += 4;
                }
                Err(_) => break,
            }
        }

        eeprom
    }

    pub fn write_byte(&mut self, mut address: u16, data: &[u8]) {
        if data.len() == 0 {
            warn!("No data to write to eeprom, skip");
            return;
        }

        // Check address
        if address as usize + data.len() >= EEPROM_SIZE {
            error!("Invalid address");
            return;
        }

        // Update cache first
        self.cache[address as usize..(address as usize + data.len())].copy_from_slice(data);

        // If the address is odd, add the previous byte to data.
        let mut data_len = data.len();
        if address % 2 != 0 {
            address -= 1;
            data_len += 1;
        }

        for i in (0..data_len).step_by(2) {
            let data_idx = address as usize + i;
            let data;
            if i + 1 == data_len {
                // Last byte, append 0xFF
                data = (0xFF << 8) | (self.cache[data_idx] as u16);
            } else {
                data = ((self.cache[data_idx + 1] as u16) << 8) | (self.cache[data_idx] as u16);
            }
            let record = EepromRecord { address, data };

            // If the storage is full, do consolidation
            if self.check_consolidation() {
                self.write_record(record);
            }

            address += 2;
        }
    }

    /// Read bytes from eeprom, starting from the given address, and reading `read_size` bytes.
    /// Returns a slice of eeprom cache, which is immutable
    pub fn read_byte(&self, address: u16, read_size: usize) -> &[u8] {
        &self.cache[address as usize..(address as usize + read_size)]
    }

    fn write_record(&mut self, record: EepromRecord) {
        match self
            .storage
            .write(STORAGE_START_ADDR + self.pos, &record.to_bytes())
        {
            Ok(_) => self.pos += 4,
            Err(_) => error!("Failed to write record to storage"),
        }
    }

    /// Read a eeprom record at the given address from the storage
    fn read_record(&mut self, address: u16) -> Option<EepromRecord> {
        let mut bytes = [0u8; 4];
        // Scan the storage, find the record with the given address
        for p in (0..self.pos).step_by(4) {
            match self.storage.read(STORAGE_START_ADDR + p, &mut bytes) {
                Ok(_) => {
                    // Check address
                    let record = EepromRecord::from_bytes(bytes);
                    if record.address == address {
                        return Some(record);
                    }
                }
                Err(_) => error!("Failed to read record from storage"),
            }
        }

        None
    }

    fn check_consolidation(&mut self) -> bool {
        if self.pos + 4 > STORAGE_SIZE {
            info!("Backend storage is full, consolidating records");
            self.consolidate_records();
            // Check position again
            if self.pos + 4 > STORAGE_SIZE {
                error!("Backend storage is full, failed to write record");
                return false;
            }
        }

        true
    }

    fn consolidate_records(&mut self) {
        // Erase the flash page first
        match self
            .storage
            .erase(STORAGE_START_ADDR, STORAGE_START_ADDR + self.pos)
        {
            Ok(_) => {
                // Consolidate records
                // TODO: lock the eeprom while reconstructing
                self.pos = 0;
                for idx in (0..self.cache.len()).step_by(2) {
                    // Skip default value
                    if self.cache[idx] == 0xFF && self.cache[idx + 1] == 0xFF {
                        continue;
                    }
                    // Build Eeprom record and write to flash
                    let record = EepromRecord {
                        address: idx as u16,
                        data: ((self.cache[idx + 1] as u16) << 8) | (self.cache[idx] as u16),
                    };
                    self.write_record(record);
                }
            }
            Err(_) => error!("Failed to erase storage"),
        }
    }
}
