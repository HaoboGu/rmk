use core::cell::RefCell;

use byteorder::{ByteOrder, LittleEndian};
use defmt::{debug, info};
use num_enum::FromPrimitive;

use crate::{keymap::KeyMap, usb::descriptor::ViaReport};

#[derive(Debug, Copy, Clone, defmt::Format, PartialEq, Eq, PartialOrd, Ord, FromPrimitive)]
#[repr(u8)]
enum VialCommand {
    GetKeyboardId = 0x00,
    GetSize = 0x01,
    GetKeyboardDef = 0x02,
    GetEncoder = 0x03,
    SetEncoder = 0x04,
    GetUnlockStatus = 0x05,
    UnlockStart = 0x06,
    UnlockPoll = 0x07,
    Lock = 0x08,
    QmkSettingsQuery = 0x09,
    QmkSettingsGet = 0x0A,
    QmkSettingsSet = 0x0B,
    QmkSettingsReset = 0x0C,
    DynamicEntryOp = 0x0D, /* operate on tapdance, combos, etc */
    #[num_enum(default)]
    Unhandled = 0xFF,
}

const VIAL_PROTOCOL_VERSION: u32 = 6;
const VIAL_EP_SIZE: usize = 32;

/// Note: vial uses litte endian, while via uses big endian
pub(crate) fn process_vial<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize>(
    report: &mut ViaReport,
    vial_keyboard_Id: &[u8],
    vial_keyboard_def: &[u8],
    _keymap: &RefCell<KeyMap<'a, ROW, COL, NUM_LAYER>>,
) {
    // report.output_data[0] == 0xFE -> vial commands
    let vial_command = VialCommand::from_primitive(report.output_data[1]);
    info!("Received vial command: {}", vial_command);
    match vial_command {
        VialCommand::GetKeyboardId => {
            debug!("Received Vial - GetKeyboardId");
            // Returns vial protocol version + vial keyboard id
            LittleEndian::write_u32(&mut report.input_data[0..4], VIAL_PROTOCOL_VERSION);
            report.input_data[4..12].clone_from_slice(vial_keyboard_Id);
        }
        VialCommand::GetSize => {
            debug!("Received Vial - GetSize");
            LittleEndian::write_u32(&mut report.input_data[0..4], vial_keyboard_def.len() as u32);
        }
        VialCommand::GetKeyboardDef => {
            debug!("Received Vial - GetKeyboardDefinition");
            let page = LittleEndian::read_u16(&report.output_data[2..4]) as usize;
            let start = page * VIAL_EP_SIZE;
            let mut end = start + VIAL_EP_SIZE;
            if end < start || start >= vial_keyboard_def.len() {
                return;
            }
            if end > vial_keyboard_def.len() {
                end = vial_keyboard_def.len();
            }
            vial_keyboard_def[start..end]
                .iter()
                .enumerate()
                .for_each(|(i, v)| {
                    report.input_data[i] = *v;
                });
            debug!(
                "Vial return: page:{} start:{} end: {}, data: {:?}",
                page, start, end, report.input_data
            );
        }
        VialCommand::GetUnlockStatus => {
            debug!("Received Vial - GetUnlockStatus");
            // Reset all data to 0xFF(it's required!)
            report.input_data.fill(0xFF);
            // Unlocked
            report.input_data[0] = 1;
            // Unlock in progress
            report.input_data[1] = 0;
        }
        VialCommand::QmkSettingsQuery => {
            report.input_data.fill(0xFF);
        }
        VialCommand::DynamicEntryOp => {
            report.input_data.fill(0x00);
        }
        VialCommand::GetEncoder => {
            let layer = report.output_data[2];
            let index = report.output_data[3];
            debug!(
                "Received Vial - GetEncoder, encoder idx: {} at layer: {}",
                index, layer
            );
            // Get encoder value
            // if let Some(encoders) = &keymap.borrow().encoders {
            //     if let Some(encoder_layer) = encoders.get(layer as usize) {
            //         if let Some(encoder) = encoder_layer.get(index as usize) {
            //             let clockwise = to_via_keycode(encoder.0);
            //             BigEndian::write_u16(&mut report.input_data[0..2], clockwise);
            //             let counter_clockwise = to_via_keycode(encoder.1);
            //             BigEndian::write_u16(&mut report.input_data[2..4], counter_clockwise);
            //             return;
            //         }
            //     }
            // }

            // Clear returned value, aka `KeyAction::No`
            report.input_data.fill(0x0);
        }
        VialCommand::SetEncoder => {
            let layer = report.output_data[2];
            let index = report.output_data[3];
            let clockwise = report.output_data[4];
            debug!(
                "Received Vial - SetEncoder, encoder idx: {} clockwise: {} at layer: {}",
                index, clockwise, layer
            );
            // if let Some(&mut mut encoders) = keymap.borrow_mut().encoders {
            //     if let Some(&mut mut encoder_layer) = encoders.get_mut(layer as usize) {
            //         if let Some(&mut mut encoder) = encoder_layer.get_mut(index as usize) {
            //             if clockwise == 1 {
            //                 let keycode = BigEndian::read_u16(&report.output_data[5..7]);
            //                 let action = from_via_keycode(keycode);
            //                 info!("Setting clockwise action: {}", action);
            //                 encoder.0 = action
            //             } else {
            //                 let keycode = BigEndian::read_u16(&report.output_data[5..7]);
            //                 let action = from_via_keycode(keycode);
            //                 info!("Setting counter-clockwise action: {}", action);
            //                 encoder.1 = action
            //             }
            //         }
            //     }
            // }
            debug!("Received Vial - SetEncoder, data: {}", report.output_data);
        }
        _ => (),
    }
}
