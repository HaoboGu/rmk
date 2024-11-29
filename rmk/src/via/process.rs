use super::{protocol::*, vial::process_vial};
use crate::{
    hid::{HidError, HidReaderWriterWrapper},
    keyboard_macro::{MACRO_SPACE_SIZE, NUM_MACRO},
    keymap::KeyMap,
    storage::{FlashOperationMessage, FLASH_CHANNEL},
    usb::descriptor::ViaReport,
    via::keycode_convert::{from_via_keycode, to_via_keycode},
};
use byteorder::{BigEndian, ByteOrder, LittleEndian};
use core::cell::RefCell;
use defmt::{debug, error, info, warn};
use embassy_time::Instant;
use num_enum::{FromPrimitive, TryFromPrimitive};
use crate::config::VialConfig;

pub(crate) struct VialService<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize> {
    // VialService holds a reference of keymap, for updating
    keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER>>,

    // Vial config
    vial_config: VialConfig<'a>,
}

impl<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize>
    VialService<'a, ROW, COL, NUM_LAYER>
{
    pub(crate) fn new(
        keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER>>,
        vial_config: VialConfig<'a>,
    ) -> Self {
        Self {
            keymap,
            vial_config,
        }
    }

    pub(crate) async fn process_via_report<Hid: HidReaderWriterWrapper>(
        &mut self,
        hid_interface: &mut Hid,
    ) -> Result<(), ()> {
        let mut via_report = ViaReport {
            input_data: [0; 32],
            output_data: [0; 32],
        };
        match hid_interface.read(&mut via_report.output_data).await {
            Ok(_) => {
                self.process_via_packet(&mut via_report, self.keymap).await;

                // Send via report back after processing
                match hid_interface.write_serialize(&via_report).await {
                    Ok(_) => Ok(()),
                    Err(e) => {
                        error!("Send via report error: {}", e);
                        // Printed error message, ignore the error type
                        Err(())
                    }
                }
            }
            Err(e) => {
                if e != HidError::UsbDisabled && e != HidError::BleDisconnected {
                    // Don't print message if the USB endpoint is disabled(aka not connected)
                    error!("Read via report error: {}", e);
                }
                // Printed error message, ignore the error type
                Err(())
            }
        }
    }

    async fn process_via_packet(
        &self,
        report: &mut ViaReport,
        keymap: &RefCell<KeyMap<'a, ROW, COL, NUM_LAYER>>,
    ) {
        let command_id = report.output_data[0];

        // `report.input_data` is initialized using `report.output_data`
        report.input_data = report.output_data;
        let via_command = ViaCommand::from_primitive(command_id);
        // debug!("Received via command: {}, report: {:02X?}", via_command, report.output_data);
        match via_command {
            ViaCommand::GetProtocolVersion => {
                BigEndian::write_u16(&mut report.input_data[1..3], VIA_PROTOCOL_VERSION);
            }
            ViaCommand::GetKeyboardValue => {
                // Check the second u8
                match ViaKeyboardInfo::try_from_primitive(report.output_data[1]) {
                    Ok(v) => match v {
                        ViaKeyboardInfo::Uptime => {
                            let value = Instant::now().as_millis() as u32;
                            BigEndian::write_u32(&mut report.input_data[2..6], value);
                        }
                        ViaKeyboardInfo::LayoutOptions => {
                            // TODO: retrieve layout option from storage
                            let layout_option: u32 = 0;
                            BigEndian::write_u32(&mut report.input_data[2..6], layout_option);
                        }
                        ViaKeyboardInfo::SwitchMatrixState => {
                            warn!("GetKeyboardValue - SwitchMatrixState")
                        }
                        ViaKeyboardInfo::FirmwareVersion => {
                            BigEndian::write_u32(
                                &mut report.input_data[2..6],
                                VIA_FIRMWARE_VERSION,
                            );
                        }
                        _ => (),
                    },
                    Err(e) => error!("Invalid subcommand: {} of GetKeyboardValue", e.number),
                }
            }
            ViaCommand::SetKeyboardValue => {
                // Check the second u8
                match ViaKeyboardInfo::try_from_primitive(report.output_data[1]) {
                    Ok(v) => match v {
                        ViaKeyboardInfo::LayoutOptions => {
                            let layout_option = BigEndian::read_u32(&report.output_data[2..6]);
                            FLASH_CHANNEL
                                .send(FlashOperationMessage::LayoutOptions(layout_option))
                                .await;
                        }
                        ViaKeyboardInfo::DeviceIndication => {
                            let _device_indication = report.output_data[2];
                            warn!("SetKeyboardValue - DeviceIndication")
                        }
                        _ => (),
                    },
                    Err(e) => error!("Invalid subcommand: {} of GetKeyboardValue", e.number),
                }
            }
            ViaCommand::DynamicKeymapGetKeyCode => {
                let layer = report.output_data[1] as usize;
                let row = report.output_data[2] as usize;
                let col = report.output_data[3] as usize;
                let action = keymap.borrow_mut().get_action_at(row, col, layer);
                let keycode = to_via_keycode(action);
                info!(
                    "Getting keycode: {:02X} at ({},{}), layer {}",
                    keycode, row, col, layer
                );
                BigEndian::write_u16(&mut report.input_data[4..6], keycode);
            }
            ViaCommand::DynamicKeymapSetKeyCode => {
                let layer = report.output_data[1];
                let row = report.output_data[2];
                let col = report.output_data[3];
                let keycode = BigEndian::read_u16(&report.output_data[4..6]);
                let action = from_via_keycode(keycode);
                info!(
                    "Setting keycode: 0x{:02X} at ({},{}), layer {} as {}",
                    keycode, row, col, layer, action
                );
                keymap.borrow_mut().set_action_at(
                    row as usize,
                    col as usize,
                    layer as usize,
                    action,
                );
                FLASH_CHANNEL
                    .send(FlashOperationMessage::KeymapKey {
                        layer,
                        col,
                        row,
                        action,
                    })
                    .await;
            }
            ViaCommand::DynamicKeymapReset => {
                warn!("Dynamic keymap reset -- not supported")
            }
            ViaCommand::CustomSetValue => {
                // backlight/rgblight/rgb matrix/led matrix/audio settings here
                warn!("Custom set value -- not supported")
            }
            ViaCommand::CustomGetValue => {
                // backlight/rgblight/rgb matrix/led matrix/audio settings here
                warn!("Custom get value -- not supported")
            }
            ViaCommand::CustomSave => {
                // backlight/rgblight/rgb matrix/led matrix/audio settings here
                warn!("Custom get value -- not supported")
            }
            ViaCommand::EepromReset => {
                warn!("Reseting storage..");
                FLASH_CHANNEL.send(FlashOperationMessage::Reset).await
                // TODO: Reboot after a eeprom reset?
            }
            ViaCommand::BootloaderJump => {
                warn!("Bootloader jump -- not supported")
            }
            ViaCommand::DynamicKeymapMacroGetCount => {
                report.input_data[1] = 8;
                warn!("Macro get count -- to be implemented")
            }
            ViaCommand::DynamicKeymapMacroGetBufferSize => {
                report.input_data[1] = (MACRO_SPACE_SIZE as u16 >> 8) as u8;
                report.input_data[2] = (MACRO_SPACE_SIZE & 0xFF) as u8;
                warn!("Macro get buffer size -- to be implemented")
            }
            ViaCommand::DynamicKeymapMacroGetBuffer => {
                let offset = BigEndian::read_u16(&report.output_data[1..3]) as usize;
                let size = report.output_data[3] as usize;
                if size <= 28 {
                    report.input_data[4..4 + size]
                        .copy_from_slice(&self.keymap.borrow().macro_cache[offset..offset + size]);
                    debug!(
                        "Get macro buffer: offset: {}, data: {:02X}",
                        offset, report.input_data
                    );
                } else {
                    report.input_data[0] = 0xFF;
                }
            }
            ViaCommand::DynamicKeymapMacroSetBuffer => {
                // Every write writes all buffer space of the macro(if it's not empty)
                // The sequence must have NUM_MACRO 0s, where each 0 indicates the end of a macro
                let offset = BigEndian::read_u16(&report.output_data[1..3]);
                // Current sequence size, <= 28
                let size = report.output_data[3];
                // End of current sequence in the macro cache
                let end = offset + size as u16;

                // The first sequence, reset the macro cache
                if offset == 0 {
                    self.keymap.borrow_mut().macro_cache = [0; MACRO_SPACE_SIZE];
                }

                // Update macro cache
                info!("Setting macro buffer, offset: {}, size: {}", offset, size);
                info!("Data: {=[u8]:x}", report.output_data[4..]);
                self.keymap.borrow_mut().macro_cache[offset as usize..end as usize]
                    .copy_from_slice(&report.output_data[4..4 + size as usize]);

                // Count zeros, if there're NUM_MACRO 0s in total, current sequnce is the last.
                // Then flush macros to storage
                let num_zero = count_zeros(&self.keymap.borrow_mut().macro_cache[0..end as usize]);
                if size < 28 || num_zero >= NUM_MACRO {
                    let buf = self.keymap.borrow_mut().macro_cache;
                    FLASH_CHANNEL
                        .send(FlashOperationMessage::WriteMacro(buf))
                        .await;
                    info!("Flush macros to storage")
                }
            }
            ViaCommand::DynamicKeymapMacroReset => {
                warn!("Macro reset -- to be implemented")
            }
            ViaCommand::DynamicKeymapGetLayerCount => {
                report.input_data[1] = NUM_LAYER as u8;
            }
            ViaCommand::DynamicKeymapGetBuffer => {
                let offset = BigEndian::read_u16(&report.output_data[1..3]);
                // size <= 28
                let size = report.output_data[3];
                info!("Getting keymap buffer, offset: {}, size: {}", offset, size);
                let mut idx = 4;
                keymap
                    .borrow()
                    .layers
                    .iter()
                    .flatten()
                    .flatten()
                    .skip((offset / 2) as usize)
                    .take((size / 2) as usize)
                    .for_each(|a| {
                        let kc = to_via_keycode(*a);
                        BigEndian::write_u16(&mut report.input_data[idx..idx + 2], kc);
                        idx += 2;
                    });
            }
            ViaCommand::DynamicKeymapSetBuffer => {
                debug!("Dynamic keymap set buffer");
                let offset = BigEndian::read_u16(&report.output_data[1..3]);
                // size <= 28
                let size = report.output_data[3];
                let mut idx = 4;
                let (row_num, col_num, _layer_num) = keymap.borrow().get_keymap_config();
                keymap
                    .borrow_mut()
                    .layers
                    .iter_mut()
                    .flatten()
                    .flatten()
                    .skip(offset as usize)
                    .take(size as usize)
                    .enumerate()
                    .for_each(|(i, a)| {
                        let via_keycode = LittleEndian::read_u16(&report.output_data[idx..idx + 2]);
                        let action: crate::action::KeyAction = from_via_keycode(via_keycode);
                        *a = action;
                        idx += 2;
                        let current_offset = offset as usize + i;
                        let (row, col, layer) =
                            get_position_from_offset(current_offset, row_num, col_num);
                        info!(
                            "Setting keymap buffer of offset: {}, row,col,layer: {},{},{}",
                            offset, row, col, layer
                        );
                        if let Err(_e) = FLASH_CHANNEL.try_send(FlashOperationMessage::KeymapKey {
                            layer: layer as u8,
                            col: col as u8,
                            row: row as u8,
                            action,
                        }) {
                            error!("Send keymap setting command error")
                        }
                    });
            }
            ViaCommand::DynamicKeymapGetEncoder => {
                warn!("Keymap get encoder -- not supported");
            }
            ViaCommand::DynamicKeymapSetEncoder => {
                warn!("Keymap get encoder -- not supported");
            }
            ViaCommand::Vial => process_vial(
                report,
                self.vial_config.vial_keyboard_id,
                self.vial_config.vial_keyboard_def,
            ),
            ViaCommand::Unhandled => report.input_data[0] = ViaCommand::Unhandled as u8,
        }
    }
}

fn get_position_from_offset(
    offset: usize,
    max_row: usize,
    max_col: usize,
) -> (usize, usize, usize) {
    let layer = offset / (max_col * max_row);
    let current_layer_offset = offset % (max_col * max_row);
    let row = current_layer_offset / max_col;
    let col = current_layer_offset % max_col;
    (row, col, layer)
}

fn count_zeros(data: &[u8]) -> usize {
    data.iter().filter(|&&x| x == 0).count()
}
