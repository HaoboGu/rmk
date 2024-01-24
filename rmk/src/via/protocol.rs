use num_enum::{FromPrimitive, TryFromPrimitive};

pub(crate) const VIA_PROTOCOL_VERSION: u16 = 0x0009;
pub(crate) const VIA_FIRMWARE_VERSION: u32 = 0x0001;

/// Via communication commands. Check [qmk/quantum/via.h`](https://github.com/qmk/qmk_firmware/blob/2fad45132f0777002934e07d17bfe8ec7aa95740/quantum/via.h#L74)
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, FromPrimitive)]
#[repr(u8)]
pub(crate) enum ViaCommand {
    GetProtocolVersion = 0x01, // always 0x01
    GetKeyboardValue = 0x02,
    SetKeyboardValue = 0x03,
    DynamicKeymapGetKeyCode = 0x04,
    DynamicKeymapSetKeyCode = 0x05,
    DynamicKeymapReset = 0x06,
    CustomSetValue = 0x07,
    CustomGetValue = 0x08,
    CustomSave = 0x09,
    EepromReset = 0x0A,
    BootloaderJump = 0x0B,
    DynamicKeymapMacroGetCount = 0x0C,
    DynamicKeymapMacroGetBufferSize = 0x0D,
    DynamicKeymapMacroGetBuffer = 0x0E,
    DynamicKeymapMacroSetBuffer = 0x0F,
    DynamicKeymapMacroReset = 0x10,
    DynamicKeymapGetLayerCount = 0x11,
    DynamicKeymapGetBuffer = 0x12,
    DynamicKeymapSetBuffer = 0x13,
    DynamicKeymapGetEncoder = 0x14,
    DynamicKeymapSetEncoder = 0x15,
    Vial = 0xFE,
    #[num_enum(default)]
    Unhandled = 0xFF,
}

/// Information of a via keyboard.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, TryFromPrimitive)]
#[repr(u8)]
pub(crate) enum ViaKeyboardInfo {
    Uptime = 0x01,
    LayoutOptions = 0x02,
    SwitchMatrixState = 0x03,
    FirmwareVersion = 0x04,
    DeviceIndication = 0x05,
}
