use embedded_storage::Storage;

/// Eeprom based on any storage device which implements `embedded-storage::Storage` trait
pub struct Eeprom<F>
where
    F: Storage,
{
    storage: F,
}

impl<F: Storage> Eeprom<F> {
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
