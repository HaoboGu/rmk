use log::error;
use num_enum::{FromPrimitive, TryFromPrimitive};
use usbd_hid::descriptor::generator_prelude::*;

const VIA_PROTOCOL_VERSION: u16 = 0x000C;

#[gen_hid_descriptor(
    (collection = APPLICATION, usage_page = 0xFF60, usage = 0x61) = {
        (usage = 0x62, logical_min = 0x0) = {
            #[item_settings data,variable,absolute] input_data=input;
        };
        (usage = 0x63, logical_min = 0x0) = {
            #[item_settings data,variable,absolute] output_data=output;
        };
    }
)]
pub struct ViaReport {
    pub input_data: [u8; 32],
    pub output_data: [u8; 32],
}

/// Via communication commands. Check [qmk/quantum/via.h`](https://github.com/qmk/qmk_firmware/blob/2fad45132f0777002934e07d17bfe8ec7aa95740/quantum/via.h#L74)
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, FromPrimitive)]
#[repr(u8)]
pub enum ViaCommand {
    GetProtocolVersion                 = 0x01, // always 0x01
    GetKeyboardValue                = 0x02,
    SetKeyboardValue                   = 0x03,
    DynamicKeymapGetKeycode           = 0x04,
    DynamicKeymapSetKeycode           = 0x05,
    DynamicKeymapReset                 = 0x06,
    CustomSetValue                     = 0x07,
    CustomGetValue                     = 0x08,
    CustomSave                          = 0x09,
    EepromReset                         = 0x0A,
    BootloaderJump                      = 0x0B,
    DynamicKeymapMacroGetCount       = 0x0C,
    DynamicKeymapMacroGetBufferSize = 0x0D,
    DynamicKeymapMacroGetBuffer      = 0x0E,
    DynamicKeymapMacroSetBuffer      = 0x0F,
    DynamicKeymapMacroReset           = 0x10,
    DynamicKeymapGetLayerCount       = 0x11,
    DynamicKeymapGetBuffer            = 0x12,
    DynamicKeymapSetBuffer            = 0x13,
    DynamicKeymapGetEncoder           = 0x14,
    DynamicKeymapSetEncoder           = 0x15,
    #[num_enum(default)]
    Unhandled                            = 0xFF,
}

/// Information of a via keyboard.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, TryFromPrimitive)]
#[repr(u8)]
pub enum ViaKeyboardInfo {
    Uptime              = 0x01,
    LayoutOptions      = 0x02,
    SwitchMatrixState = 0x03,
    FirmwareVersion    = 0x04,
    DeviceIndication   = 0x05,
}

pub fn process_via_packet(report: &mut ViaReport) {
    let command_id = report.output_data[0];
    
    // `report.input_data` is initialized using `report.output_data`
    report.input_data = report.output_data;
    match ViaCommand::from_primitive(command_id) {
        ViaCommand::GetProtocolVersion => {
            report.input_data[0] = (VIA_PROTOCOL_VERSION >> 8) as u8;
            report.input_data[0] = (VIA_PROTOCOL_VERSION & 0xFF) as u8;
        }
        ViaCommand::GetKeyboardValue => {
            // Check the second u8
            match ViaKeyboardInfo::try_from_primitive(report.output_data[1]) {
                Ok(v) => {
                    match v {
                        ViaKeyboardInfo::Uptime => todo!("Return timestamp"),
                        ViaKeyboardInfo::LayoutOptions => todo!(),
                        ViaKeyboardInfo::SwitchMatrixState => todo!(),
                        ViaKeyboardInfo::FirmwareVersion => todo!(),
                        ViaKeyboardInfo::DeviceIndication => todo!(),
                    }
                }
                Err(e) => error!("Invalid keyboard value number: {}", e.number),
            }
        },
        ViaCommand::SetKeyboardValue => todo!(),
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
        ViaCommand::Unhandled => todo!(),
    }
}
