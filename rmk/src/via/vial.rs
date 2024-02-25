use byteorder::{ByteOrder, LittleEndian};
use defmt::debug;
use num_enum::FromPrimitive;

use crate::usb::descriptor::ViaReport;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, FromPrimitive)]
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
///
/// Note: vial uses litte endian, while via uses big endian
pub(crate) fn process_vial(
    report: &mut ViaReport,
    vial_keyboard_Id: &[u8],
    vial_keyboard_def: &[u8],
) {
    // report.output_data[0] == 0xFE -> vial commands
    let vial_command = VialCommand::from_primitive(report.output_data[1]);
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
        _ => (),
    }
}
