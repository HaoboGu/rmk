// impl<F: NorFlash, const EEPROM_SIZE: usize> Eeprom<F, EEPROM_SIZE> {
//     pub(crate) async fn set_keymap<const ROW: usize, const COL: usize, const NUM_LAYER: usize>(
//         &mut self,
//         keymap: &[[[KeyAction; COL]; ROW]; NUM_LAYER],
//     ) {
//         keymap
//             .iter()
//             .flatten()
//             .flatten()
//             .enumerate()
//             .for_each(|(i, action)| {
//                 // 2-byte value, relative addr should be i*2
//                 let addr = DYNAMIC_KEYMAP_ADDR + (i * 2) as u16;
//                 let mut buf: [u8; 2] = [0xFF; 2];
//                 BigEndian::write_u16(&mut buf, to_via_keycode(*action));
//                 block_on(self.write_byte(addr, &buf));
//             });
//     }

//     pub(crate) async fn set_keymap_action(
//         &mut self,
//         row: usize,
//         col: usize,
//         layer: usize,
//         action: KeyAction,
//     ) {
//         let addr = self.get_keymap_addr(row, col, layer);
//         let mut buf: [u8; 2] = [0xFF; 2];
//         BigEndian::write_u16(&mut buf, to_via_keycode(action));
//         self.write_byte(addr, &buf).await;
//     }

//     pub(crate) fn read_keymap<const ROW: usize, const COL: usize, const NUM_LAYER: usize>(
//         &self,
//         keymap: &mut [[[KeyAction; COL]; ROW]; NUM_LAYER],
//     ) {
//         for (layer, layer_data) in keymap.iter_mut().enumerate() {
//             for (row, row_data) in layer_data.iter_mut().enumerate() {
//                 for (col, value) in row_data.iter_mut().enumerate() {
//                     let addr = self.get_keymap_addr(row, col, layer);
//                     let data = self.read_byte(addr, 2);
//                     *value = from_via_keycode(BigEndian::read_u16(data));
//                 }
//             }
//         }
//     }

//     fn get_keymap_addr(&self, row: usize, col: usize, layer: usize) -> u16 {
//         DYNAMIC_KEYMAP_ADDR
//             + ((self.keymap_config.col * self.keymap_config.row * layer
//                 + self.keymap_config.col * row
//                 + col)
//                 * 2) as u16
//     }
// }
