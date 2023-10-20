use crate::keymap::KeyMap;
use byteorder::{BigEndian, ByteOrder};
use embedded_storage::nor_flash::NorFlash;

use super::{eeconfig::DYNAMIC_KEYMAP_ADDR, Eeprom};

impl<
        F: NorFlash,
        const STORAGE_START_ADDR: u32,
        const STORAGE_SIZE: u32,
        const EEPROM_SIZE: usize,
    > Eeprom<F, STORAGE_START_ADDR, STORAGE_SIZE, EEPROM_SIZE>
{
    pub fn set_keymap<const ROW: usize, const COL: usize, const NUM_LAYER: usize>(
        &mut self,
        keymap: &KeyMap<ROW, COL, NUM_LAYER>,
    ) {
        keymap
            .layers
            .iter()
            .flatten()
            .flatten()
            .enumerate()
            .for_each(|(i, action)| {
                // 2-byte value, relative addr should be i*2
                let addr = DYNAMIC_KEYMAP_ADDR + (i * 2) as u16;
                let mut buf: [u8; 2] = [0xFF; 2];
                BigEndian::write_u16(&mut buf, action.to_u16());
                self.write_byte(addr, &buf);
            });
    }
}
