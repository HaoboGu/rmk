use embedded_storage::Storage;

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
    pos: usize,
    storage: F,
}

pub struct EepromRecord {
    address: u16,
    data: u16,
}

impl<F: Storage, const STORAGE_PAGE_SIZE: usize> Eeprom<F, STORAGE_PAGE_SIZE> {
    fn write_record(&mut self, address: u16, data: u16) {
        todo!()
    }

    fn read_record(&mut self, address: u16, data: u16) {
        todo!()
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
