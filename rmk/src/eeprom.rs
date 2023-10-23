pub mod eeconfig;
pub mod eekeymap;

use self::eeconfig::EEPROM_MAGIC;
use crate::{action::KeyAction, keymap::KeyMapConfig};
use core::sync::atomic::{AtomicBool, Ordering::SeqCst};
use embedded_storage::nor_flash::NorFlash;
use log::{error, info, warn};

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

/// Configuration of eeprom's backend storage.
pub struct EepromStorageConfig {
    /// The start address in the backend storage.
    pub start_addr: u32,
    /// Total used size in backend storage for eeprom.
    pub storage_size: u32,
    /// Minimal write size of backend storage.
    /// For example, stm32h7's internal flash allows 256-bit(32 bytes) or 128-bit(16 bytes) write, so page_size should be 32/16 for stm32h7.
    pub page_size: u32,
}

/// Eeprom based on any storage device which implements `embedded-storage::NorFlash` trait
/// Data in eeprom is saved in a 4-byte `record`, with 2-byte address in the first 16 bits and 2-byte data in the next 16 bits.
/// Eeprom struct maintains a cache in ram to speed up reads, whose size is same as the logical eeprom capacity.
/// User can specify the size of the logical size of eeprom(maximum 64KB), Eeprom struct maintains a cache in ram to speed up reads, whose size is same as the user defined logical eeprom capacity.
pub struct Eeprom<F: NorFlash, const EEPROM_SIZE: usize> {
    /// Current position in the storage
    pos: u32,
    /// Backend storage, implements `embedded-storage::NorFlash` trait
    storage: F,
    /// A eeprom cache in ram to speed up reads, whose size is same as the logical eeprom capacity
    cache: [u8; EEPROM_SIZE],

    /// Configuration of the backend storage
    storage_config: EepromStorageConfig,

    /// Lock
    lock: AtomicBool,

    /// Layout info of dynamic keymap.
    /// Each key in keymap used 2 bytes, so the size should be at least 2 * NUM_LAYER * ROW * COL.
    ///
    ///  For a 104-key keyboard, with 4 layers, 6 rows and 21 columns, the size is 1008 bytes,
    ///  EEPROM_SIZE should be at least 1008(keymap) + 15(eeconfig) + 100(macro)
    keymap_config: KeyMapConfig,
}
impl<F: NorFlash, const EEPROM_SIZE: usize> Eeprom<F, EEPROM_SIZE> {
    pub fn new<const ROW: usize, const COL: usize, const NUM_LAYER: usize>(
        storage: F,
        storage_config: EepromStorageConfig,
        keymap: &[[[KeyAction; COL]; ROW]; NUM_LAYER],
    ) -> Option<Self> {
        // Check backend storage config
        if (!is_power_of_two(storage_config.page_size))
            || storage_config.start_addr == 0
            || storage_config.storage_size == 0
        {
            return None;
        }

        let mut eeprom = Eeprom {
            pos: 0,
            storage,
            storage_config,
            lock: AtomicBool::new(false),
            cache: [0xFF; EEPROM_SIZE],
            keymap_config: KeyMapConfig {
                row: ROW,
                col: COL,
                layer: NUM_LAYER,
            },
        };

        // Initialize eeprom using default config
        let current_magic = eeprom.get_magic();
        // if current_magic != 0 {
        if current_magic != EEPROM_MAGIC {
            // Need initialize the eeprom, erase the storage first
            eeprom
                .storage
                .erase(
                    eeprom.storage_config.start_addr,
                    eeprom.storage_config.start_addr + eeprom.storage_config.storage_size,
                )
                .unwrap();
            // TODO: support user custom config
            eeprom.init_with_default_config();
            eeprom.set_keymap(keymap);
        }

        // Restore eeprom from storage
        let mut buf: [u8; 4] = [0xFF; 4];
        while eeprom.pos < eeprom.storage_config.storage_size {
            match eeprom
                .storage
                .read(eeprom.storage_config.start_addr + eeprom.pos, &mut buf)
            {
                Ok(_) => {
                    let record = EepromRecord::from_bytes(buf);
                    if record.address >= EEPROM_SIZE as u16 {
                        break;
                    }
                    eeprom.cache[record.address as usize] = (record.data >> 8) as u8;
                    eeprom.cache[record.address as usize + 1] = record.data as u8;
                    eeprom.pos += eeprom.storage_config.page_size;
                }
                Err(e) => {
                    error!(
                        "Restore eeprom value at pos {:x} error: {:?}",
                        eeprom.pos, e
                    );
                    break;
                }
            }
        }

        Some(eeprom)
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
                data = ((self.cache[data_idx] as u16) << 8) | (0xFF << 8);
            } else {
                data = ((self.cache[data_idx] as u16) << 8) | (self.cache[data_idx + 1] as u16);
            }
            let record = EepromRecord { address, data };

            // If the storage is full, do consolidation
            if self.check_consolidation() {
                self.write_record(record);
            } else {
                warn!("Write eeprom error, the backend storage is full")
            }

            address += 2;
        }
    }

    /// Read bytes from eeprom, starting from the given address, and reading `read_size` bytes.
    /// Returns a slice of eeprom cache, which is immutable
    pub fn read_byte(&self, address: u16, read_size: usize) -> &[u8] {
        &self.cache[address as usize..(address as usize + read_size)]
    }

    // Each write should be aligned to 16 bytes / 32 bytes for
    fn write_record(&mut self, record: EepromRecord) {
        let mut buf = [0xFF; 1];

        // Find a free page to write
        while self.pos <= self.storage_config.storage_size {
            match self
                .storage
                .read(self.storage_config.start_addr + self.pos, &mut buf)
            {
                Ok(_) => {
                    // Check buf
                    if buf[0] == 0xFF {
                        break;
                    } else {
                        warn!(
                            "Writing addr {:X} is not 0xFF",
                            self.storage_config.start_addr + self.pos
                        );
                        self.pos += self.storage_config.page_size;
                    }
                }
                Err(e) => {
                    warn!(
                        "Check addr {:X} error before writing: {:?}",
                        self.storage_config.start_addr + self.pos,
                        e
                    );
                    // Go to next possible addr
                    self.pos += self.storage_config.page_size;
                }
            }
        }

        let bytes = record.to_bytes();
        buf[..bytes.len()].copy_from_slice(&bytes);
        info!(
            "Write buf: {:02X?} at {:X}",
            buf,
            self.storage_config.start_addr + self.pos
        );

        match self
            .storage
            .write(self.storage_config.start_addr + self.pos, &buf)
        {
            // MUST BE ALIGNED HERE!
            Ok(_) => self.pos += self.storage_config.page_size,
            Err(e) => {
                error!(
                    "Failed to write record to storage at {:X}: {:?}",
                    self.storage_config.start_addr + self.pos,
                    e
                )
            }
        }
    }

    /// Read a eeprom record at the given address from the storage
    fn read_record(&mut self, address: u16) -> Option<EepromRecord> {
        let mut bytes = [0u8; 4];
        let mut end = self.pos;
        // Before the eeprom initialized, check all the storage to read a record
        if self.pos == 0 {
            end = self.storage_config.storage_size;
        }
        // Scan the storage, find the record with the given address
        for p in (0..end).step_by(16) {
            match self
                .storage
                .read(self.storage_config.start_addr + p, &mut bytes)
            {
                Ok(_) => {
                    // Check address
                    let record = EepromRecord::from_bytes(bytes);
                    if record.address == address {
                        return Some(record);
                    } else if record.address == 0xFFFF {
                        // Reach the end of current records
                        break;
                    }
                }
                Err(_) => error!("Failed to read record from storage"),
            }
        }

        None
    }

    fn check_consolidation(&mut self) -> bool {
        if self.pos + self.storage_config.page_size > self.storage_config.storage_size {
            info!("Backend storage is full, consolidating records");
            self.consolidate_records();
            // Check position again
            if self.pos + self.storage_config.page_size > self.storage_config.storage_size {
                error!("Backend storage is full, failed to write record");
                return false;
            }
        }

        true
    }

    fn consolidate_records(&mut self) {
        // Lock the eeprom when reconstructing
        match self.lock.compare_exchange(false, true, SeqCst, SeqCst) {
            Ok(_) => (),
            Err(_) => return,
        };

        // Erase the flash page first
        match self.storage.erase(
            self.storage_config.start_addr,
            self.storage_config.start_addr + self.pos,
        ) {
            Ok(_) => {
                // Consolidate records
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

        // Unlock
        self.lock.store(false, SeqCst);
    }
}

fn is_power_of_two(n: u32) -> bool {
    n > 0 && (n & (n - 1)) == 0
}
