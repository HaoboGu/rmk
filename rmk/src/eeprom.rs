use embedded_storage::Storage;

/// Eeprom based on any storage device which implements `embedded-storage::Storage` trait
/// Ref: https://www.st.com/content/ccc/resource/technical/document/application_note/group0/b2/94/a6/62/18/c0/4f/e6/DM00311483/files/DM00311483.pdf/jcr:content/translations/en.DM00311483.pdf
/// 
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
