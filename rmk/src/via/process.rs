use super::{descriptor::*, protocol::*, *};
use crate::{
    eeprom::Eeprom,
    keymap::KeyMap,
    via::keycode_convert::{from_via_keycode, to_via_keycode},
};
use byteorder::{BigEndian, ByteOrder, LittleEndian};
use embedded_storage::nor_flash::NorFlash;
use log::{debug, info, warn};
use num_enum::{FromPrimitive, TryFromPrimitive};
use rtic_monotonics::{systick::Systick, Monotonic};

pub fn process_via_packet<
    F: NorFlash,
    const EEPROM_SIZE: usize,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    report: &mut ViaReport,
    keymap: &mut KeyMap<ROW, COL, NUM_LAYER>,
    eeprom: &mut Option<Eeprom<F, EEPROM_SIZE>>,
) {
    let command_id = report.output_data[0];

    // `report.input_data` is initialized using `report.output_data`
    report.input_data = report.output_data;
    let via_command = ViaCommand::from_primitive(command_id);
    // debug!("Received via command: {:?}, report: {:02X?}", via_command, report.output_data);
    match via_command {
        ViaCommand::GetProtocolVersion => {
            BigEndian::write_u16(&mut report.input_data[1..3], VIA_PROTOCOL_VERSION);
        }
        ViaCommand::GetKeyboardValue => {
            // Check the second u8
            match ViaKeyboardInfo::try_from_primitive(report.output_data[1]) {
                Ok(v) => match v {
                    ViaKeyboardInfo::Uptime => {
                        let value = Systick::now().ticks();
                        BigEndian::write_u32(&mut report.input_data[2..6], value);
                    }
                    ViaKeyboardInfo::LayoutOptions => {
                        let layout_option: u32 = 0;
                        BigEndian::write_u32(&mut report.input_data[2..6], layout_option);
                    }
                    ViaKeyboardInfo::SwitchMatrixState => {
                        warn!("GetKeyboardValue - SwitchMatrixState")
                    }
                    ViaKeyboardInfo::FirmwareVersion => {
                        BigEndian::write_u32(&mut report.input_data[2..6], VIA_FIRMWARE_VERSION);
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
                        match eeprom {
                            Some(e) => e.set_layout_option(layout_option),
                            None => (),
                        }
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
            let action = keymap.get_action_at(row, col, layer);
            let keycode = to_via_keycode(action);
            info!(
                "Getting keycode: {:02X?} at ({},{}), layer {}",
                keycode, row, col, layer
            );
            BigEndian::write_u16(&mut report.input_data[4..6], keycode);
        }
        ViaCommand::DynamicKeymapSetKeyCode => {
            let layer = report.output_data[1] as usize;
            let row = report.output_data[2] as usize;
            let col = report.output_data[3] as usize;
            let keycode = BigEndian::read_u16(&report.output_data[4..6]);
            let action = from_via_keycode(keycode);
            info!(
                "Setting keycode: 0x{:02X?} at ({},{}), layer {} as {:?}",
                keycode, row, col, layer, action
            );
            keymap.set_action_at(row, col, layer, action.clone());
            match eeprom {
                Some(e) => e.set_keymap_action(row, col, layer, action),
                None => (),
            }
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
            warn!("Eeprom reset -- not supported")
        }
        ViaCommand::BootloaderJump => {
            warn!("Bootloader jump -- not supported")
        }
        ViaCommand::DynamicKeymapMacroGetCount => {
            report.input_data[1] = 1;
            warn!("Macro get count -- to be implemented")
        }
        ViaCommand::DynamicKeymapMacroGetBufferSize => {
            // report.input_data[0] = 0xFF;
            report.input_data[1] = 0x00;
            report.input_data[2] = 0x10;
            warn!("Macro get buffer size -- to be implemented")
        }
        ViaCommand::DynamicKeymapMacroGetBuffer => {
            let _offset = BigEndian::read_u16(&report.output_data[1..3]);
            let size = report.output_data[3];
            if size <= 28 {
                debug!("Current returned data: {:02X?}", report.input_data);
            } else {
                report.input_data[0] = 0xFF;
            }
            warn!("Macro get buffer -- to be implemented")
        }
        ViaCommand::DynamicKeymapMacroSetBuffer => {
            warn!("Macro set buffer -- to be implemented")
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
            let (row_num, col_num, _layer_num) = keymap.get_keymap_config();
            keymap
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
                    match eeprom {
                        Some(e) => e.set_keymap_action(row, col, layer, action),
                        None => (),
                    }
                });
        }
        ViaCommand::DynamicKeymapGetEncoder => {
            warn!("Keymap get encoder -- not supported");
        }
        ViaCommand::DynamicKeymapSetEncoder => {
            warn!("Keymap get encoder -- not supported");
        }
        ViaCommand::Vial => vial::process_vial(report),
        ViaCommand::Unhandled => report.input_data[0] = ViaCommand::Unhandled as u8,
    }
}

pub fn get_position_from_offset(
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
