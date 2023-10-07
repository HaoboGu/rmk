use super::{descriptor::*, protocol::*, *};
use byteorder::{BigEndian, ByteOrder};
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
                        let _layout_option: u32 = todo!("get layout option");
                        // BigEndian::write_u32(&mut report.input_data[2..6], _layout_option);
                    }
                    ViaKeyboardInfo::SwitchMatrixState => {
                        todo!("GetKeyboardValue - SwitchMatrixState")
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
                        let _layout_option = BigEndian::read_u32(&report.output_data[2..6]);
                        todo!("SetKeyboardValue - LayoutOptions: need eeprom");
                    }
                    ViaKeyboardInfo::DeviceIndication => {
                        let _device_indication = report.output_data[2];
                        todo!("SetKeyboardValue - DeviceIndication")
                    }
                    _ => (),
                },
                Err(e) => error!("Invalid subcommand: {} of GetKeyboardValue", e.number),
            }
        }
        ViaCommand::DynamicKeymapGetKeycode => {
            let _layer = report.output_data[1];
            let _row = report.output_data[2];
            let _col = report.output_data[3];
            let _keycode: u16 = todo!("get keycode");
            // BigEndian::write_u16(&mut report.input_data[4..6], keycode);
        }
        ViaCommand::DynamicKeymapSetKeycode => {
            let _layer = report.output_data[1];
            let _row = report.output_data[2];
            let _col = report.output_data[3];
            let _keycode = BigEndian::read_u16(&report.output_data[4..6]);
            todo!("DynamicKeymap - Set Keycode")
        }
        ViaCommand::DynamicKeymapReset => todo!("DynamicKeymap - Reset"),
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
        ViaCommand::DynamicKeymapGetBuffer => {
            let _offset = BigEndian::read_u16(&report.output_data[1..3]);
            // size <= 28
            let _size = report.output_data[3]; 
            todo!("DynamicKeymap - Get Buffer");
        },
        ViaCommand::DynamicKeymapSetBuffer => {
            let _offset = BigEndian::read_u16(&report.output_data[1..3]);
            // size <= 28
            let _size = report.output_data[3]; 
            todo!("DynamicKeymap - Set Buffer");
        },
        ViaCommand::DynamicKeymapGetEncoder => todo!(),
        ViaCommand::DynamicKeymapSetEncoder => todo!(),
        ViaCommand::Vial => info!("Received vial command!"),
        ViaCommand::Unhandled => todo!(),
    }
}
