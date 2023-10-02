use embedded_storage::Storage;

/// Eeprom based on any storage device which implements `embedded-storage::Storage` trait
/// Ref: https://www.st.com/resource/en/application_note/an4894-how-to-use-eeprom-emulation-on-stm32-mcus-stmicroelectronics.pdf
/// Zh Ref: https://www.st.com/resource/zh/application_note/an3969-eeprom-emulation-in-stm32f40xstm32f41x-microcontrollers-stmicroelectronics.pdf
/// 
pub struct Eeprom<F, const STORAGE_PAGE_SIZE: usize>
where
    F: Storage,
{
    /// Current position in the storage
    pos: usize,
    storage: F,
}

impl<F: Storage, const STORAGE_PAGE_SIZE: usize> Eeprom<F, STORAGE_PAGE_SIZE> {
    pub fn write_byte(&mut self, data: &mut [u8]) {
        match self.storage.write(0, data) {
            Ok(_) => todo!(),
            Err(_) => todo!(),
        }
    }

    pub fn read_byte(&mut self, data: &mut [u8]) {
        match self.storage.read(0, data) {
            Ok(_) => todo!(),
            Err(_) => todo!(),
        }
    }
}
