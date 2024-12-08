use num_enum::FromPrimitive;

/// Vial communication commands. Check [vial-qmk/quantum/vial.h`](https://github.com/vial-kb/vial-qmk/blob/20d61fcb373354dc17d6ecad8f8176be469743da/quantum/vial.h#L36)
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, FromPrimitive)]
#[repr(u8)]
pub(crate) enum VialCommand {
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

/// Vial dynamic commands. Check [vial-qmk/quantum/vial.h`](https://github.com/vial-kb/vial-qmk/blob/20d61fcb373354dc17d6ecad8f8176be469743da/quantum/vial.h#L53)
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, FromPrimitive)]
#[repr(u8)]
pub(crate) enum VialDynamic {
    DynamicVialGetNumberOfEntries = 0x00,
    DynamicVialTapDanceGet = 0x01,
    DynamicVialTapDanceSet = 0x02,
    DynamicVialComboGet = 0x03,
    DynamicVialComboSet = 0x04,
    DynamicVialKeyOverrideGet = 0x05,
    DynamicVialKeyOverrideSet = 0x06,
    #[num_enum(default)]
    Unhandled = 0xFF,
}

pub(crate) const VIAL_PROTOCOL_VERSION: u32 = 6;
pub(crate) const VIAL_EP_SIZE: usize = 32;

pub(crate) const VIAL_COMBO_MAX_LENGTH: usize = 4;
