use super::{protocol::*, descriptor::*, *};
use log::info;
use num_enum::{FromPrimitive, TryFromPrimitive};
use rtic_monotonics::{systick::Systick, Monotonic};

pub fn process_via_packet(report: &mut ViaReport) {
    let command_id = report.output_data[0];
    info!(
        "Received via report: {:02X?}, command_id: {}",
        report.output_data, command_id
    );

    // `report.input_data` is initialized using `report.output_data`
    report.input_data = report.output_data;
    match ViaCommand::from_primitive(command_id) {
        ViaCommand::GetProtocolVersion => {
            set_data_u16(&mut report.input_data[1..], VIA_PROTOCOL_VERSION);
        }
        ViaCommand::GetKeyboardValue => {
            // Check the second u8
            match ViaKeyboardInfo::try_from_primitive(report.output_data[1]) {
                Ok(v) => match v {
                    ViaKeyboardInfo::Uptime => {
                        let value = Systick::now().ticks();
                        set_data_u32(&mut report.input_data[1..], value);
                    }
                    ViaKeyboardInfo::LayoutOptions => todo!("GetKeyboardValue - LayoutOptions: need eeprom"),
                    ViaKeyboardInfo::SwitchMatrixState => todo!("GetKeyboardValue - SwitchMatrixState"),
                    ViaKeyboardInfo::FirmwareVersion => {
                        let value = VIA_FIRMWARE_VERSION;
                        set_data_u32(&mut report.input_data[1..], value);
                    }
                    _ => ()
                },
                Err(e) => error!("Invalid subcommand: {} of GetKeyboardValue", e.number),
            }
        }
        ViaCommand::SetKeyboardValue => {
            // Check the second u8
            match ViaKeyboardInfo::try_from_primitive(report.output_data[1]) {
                Ok(v) => match v {
                    ViaKeyboardInfo::LayoutOptions => todo!("SetKeyboardValue - LayoutOptions: need eeprom"),
                    ViaKeyboardInfo::DeviceIndication => todo!("SetKeyboardValue - DeviceIndication"),
                    _ => ()
                },
                Err(e) => error!("Invalid subcommand: {} of GetKeyboardValue", e.number),
            }
        }
        ViaCommand::DynamicKeymapGetKeycode => todo!(),
        ViaCommand::DynamicKeymapSetKeycode => todo!(),
        ViaCommand::DynamicKeymapReset => todo!(),
        ViaCommand::CustomSetValue => todo!(),
        ViaCommand::CustomGetValue => todo!(),
        ViaCommand::CustomSave => todo!(),
        ViaCommand::EepromReset => todo!(),
        ViaCommand::BootloaderJump => todo!(),
        ViaCommand::DynamicKeymapMacroGetCount => todo!(),
        ViaCommand::DynamicKeymapMacroGetBufferSize => todo!(),
        ViaCommand::DynamicKeymapMacroGetBuffer => todo!(),
        ViaCommand::DynamicKeymapMacroSetBuffer => todo!(),
        ViaCommand::DynamicKeymapMacroReset => todo!(),
        ViaCommand::DynamicKeymapGetLayerCount => todo!(),
        ViaCommand::DynamicKeymapGetBuffer => todo!(),
        ViaCommand::DynamicKeymapSetBuffer => todo!(),
        ViaCommand::DynamicKeymapGetEncoder => todo!(),
        ViaCommand::DynamicKeymapSetEncoder => todo!(),
        ViaCommand::Vial => info!("Received vial command!"),
        ViaCommand::Unhandled => todo!(),
    }
}

fn set_data_u32(data: &mut [u8], value: u32) {
    data[0] = ((value >> 24) as u8) & 0xFF;
    data[1] = ((value >> 16) as u8) & 0xFF;
    data[2] = ((value >> 8) as u8) & 0xFF;
    data[3] = (value as u8) & 0xFF;
}

fn set_data_u16(data: &mut [u8], value: u16) {
    data[0] = ((value >> 8) as u8) & 0xFF;
    data[1] = (value as u8) & 0xFF;
}
