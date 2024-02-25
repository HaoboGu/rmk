use defmt::Format;
use num_enum::FromPrimitive;
use packed_struct::prelude::*;
use usbd_hid::descriptor::{MediaKey, SystemControlKey};

/// To represent all combinations of modifiers, at least 5 bits are needed.
/// 1 bit for Left/Right, 4 bits for modifier type. Represented in LSB format.
///
/// | bit0 | bit1 | bit2 | bit3 | bit4 |
/// | --- | --- | --- | --- | --- |  
/// | L/R | GUI | ALT |SHIFT| CTRL|
#[derive(PackedStruct, Clone, Copy, Debug, Format, Default, Eq, PartialEq)]
#[packed_struct(bit_numbering = "lsb0", size_bytes = "1")]
pub struct ModifierCombination {
    #[packed_field(bits = "0")]
    ctrl: bool,
    #[packed_field(bits = "1")]
    shift: bool,
    #[packed_field(bits = "2")]
    alt: bool,
    #[packed_field(bits = "3")]
    gui: bool,
    #[packed_field(bits = "4")]
    right: bool,
}

impl ModifierCombination {
    pub(crate) fn new(right: bool, gui: bool, alt: bool, shift: bool, ctrl: bool) -> Self {
        ModifierCombination {
            ctrl,
            shift,
            alt,
            gui,
            right,
        }
    }

    /// Convert modifier combination to a list of modifier keycodes.
    /// Returns a list of modifiers keycodes, and the length of the list.
    pub(crate) fn to_modifier_keycodes(self) -> ([KeyCode; 8], usize) {
        let mut keycodes = [KeyCode::No; 8];
        let mut i = 0;
        if self.right {
            if self.ctrl {
                keycodes[i] = KeyCode::LCtrl;
                i += 1;
            }
            if self.shift {
                keycodes[i] = KeyCode::LShift;
                i += 1;
            }
            if self.alt {
                keycodes[i] = KeyCode::LAlt;
                i += 1;
            }
            if self.gui {
                keycodes[i] = KeyCode::LGui;
                i += 1;
            }
        } else {
            if self.ctrl {
                keycodes[i] = KeyCode::RCtrl;
                i += 1;
            }
            if self.shift {
                keycodes[i] = KeyCode::RShift;
                i += 1;
            }
            if self.alt {
                keycodes[i] = KeyCode::RAlt;
                i += 1;
            }
            if self.gui {
                keycodes[i] = KeyCode::RGui;
                i += 1;
            }
        }

        (keycodes, i)
    }

    /// Get modifier hid report bits from modifier combination
    pub(crate) fn to_hid_modifier_bits(self) -> u8 {
        let (keycodes, n) = self.to_modifier_keycodes();
        let mut hid_modifier_bits = 0;
        for item in keycodes.iter().take(n) {
            hid_modifier_bits |= item.as_modifier_bit();
        }
        // for i in 0..n {
            // hid_modifier_bits |= keycodes[i].as_modifier_bit();
        // }

        hid_modifier_bits
    }

    /// Convert modifier combination to bits
    pub(crate) fn to_bits(self) -> u8 {
        ModifierCombination::pack(&self).unwrap_or_default()[0]
    }

    /// Convert from bits
    pub(crate) fn from_bits(bits: u8) -> Self {
        ModifierCombination::unpack_from_slice(&[bits]).unwrap_or_default()
    }
}

/// KeyCode is the internal representation of all keycodes, keyboard operations, etc.
/// Use flat representation of keycodes.
#[derive(Debug, Format, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, FromPrimitive)]
#[repr(u16)]
pub enum KeyCode {
    /// Reserved, no-key.
    #[num_enum(default)]
    No = 0x0000,
    /// Keyboard roll over error, too many keys are pressed simultaneously, not a physical key.
    /// NKRO: n-key rollover.
    ErrorRollover = 0x0001,
    /// Keyboard post fail error, not a physical key.
    PostFail = 0x0002,
    /// An undefined error, not a physical key.
    ErrorUndefined = 0x0003,
    A = 0x0004,
    B = 0x0005,
    C = 0x0006,
    D = 0x0007,
    E = 0x0008,
    F = 0x0009,
    G = 0x000A,
    H = 0x000B,
    I = 0x000C,
    J = 0x000D,
    K = 0x000E,
    L = 0x000F,
    M = 0x0010,
    N = 0x0011,
    O = 0x0012,
    P = 0x0013,
    Q = 0x0014,
    R = 0x0015,
    S = 0x0016,
    T = 0x0017,
    U = 0x0018,
    V = 0x0019,
    W = 0x001A,
    X = 0x001B,
    Y = 0x001C,
    Z = 0x001D,
    Kc1 = 0x001E,
    Kc2 = 0x001F,
    Kc3 = 0x0020,
    Kc4 = 0x0021,
    Kc5 = 0x0022,
    Kc6 = 0x0023,
    Kc7 = 0x0024,
    Kc8 = 0x0025,
    Kc9 = 0x0026,
    Kc0 = 0x0027,
    Enter = 0x0028,
    Escape = 0x0029,
    Backspace = 0x002A,
    Tab = 0x002B,
    Space = 0x002C,
    Minus = 0x002D,
    Equal = 0x002E,
    LeftBracket = 0x002F,
    RightBracket = 0x0030,
    Backslash = 0x0031,
    NonusHash = 0x0032,
    Semicolon = 0x0033,
    Quote = 0x0034,
    Grave = 0x0035,
    Comma = 0x0036,
    Dot = 0x0037,
    Slash = 0x0038,
    CapsLock = 0x0039,
    F1 = 0x003A,
    F2 = 0x003B,
    F3 = 0x003C,
    F4 = 0x003D,
    F5 = 0x003E,
    F6 = 0x003F,
    F7 = 0x0040,
    F8 = 0x0041,
    F9 = 0x0042,
    F10 = 0x0043,
    F11 = 0x0044,
    F12 = 0x0045,
    PrintScreen = 0x0046,
    ScrollLock = 0x0047,
    Pause = 0x0048,
    Insert = 0x0049,
    Home = 0x004A,
    PageUp = 0x004B,
    Delete = 0x004C,
    End = 0x004D,
    PageDown = 0x004E,
    Right = 0x004F,
    Left = 0x0050,
    Down = 0x0051,
    UP = 0x0052,
    NumLock = 0x0053,
    KpSlash = 0x0054,
    KpAsterisk = 0x0055,
    KpMinus = 0x0056,
    KpPlus = 0x0057,
    KpEnter = 0x0058,
    Kp1 = 0x0059,
    Kp2 = 0x005A,
    Kp3 = 0x005B,
    Kp4 = 0x005C,
    Kp5 = 0x005D,
    Kp6 = 0x005E,
    Kp7 = 0x005F,
    Kp8 = 0x0060,
    Kp9 = 0x0061,
    Kp0 = 0x0062,
    KpDot = 0x0063,
    NonusBackslash = 0x0064,
    Application = 0x0065,
    KbPower = 0x0066,
    KpEqual = 0x0067,
    F13 = 0x0068,
    F14 = 0x0069,
    F15 = 0x006A,
    F16 = 0x006B,
    F17 = 0x006C,
    F18 = 0x006D,
    F19 = 0x006E,
    F20 = 0x006F,
    F21 = 0x0070,
    F22 = 0x0071,
    F23 = 0x0072,
    F24 = 0x0073,
    Execute = 0x0074,
    Help = 0x0075,
    Menu = 0x0076,
    Select = 0x0077,
    Stop = 0x0078,
    Again = 0x0079,
    Undo = 0x007A,
    Cut = 0x007B,
    Copy = 0x007C,
    Paste = 0x007D,
    Find = 0x007E,
    KbMute = 0x007F,
    KbVolumeUp = 0x0080,
    KbVolumeDown = 0x0081,
    LockingCapsLock = 0x0082,
    LockingNumLock = 0x0083,
    LockingScrollLock = 0x0084,
    KpComma = 0x0085,
    KpEqualAs400 = 0x0086,
    International1 = 0x0087,
    International2 = 0x0088,
    International3 = 0x0089,
    International4 = 0x008A,
    International5 = 0x008B,
    International6 = 0x008C,
    International7 = 0x008D,
    International8 = 0x008E,
    International9 = 0x008F,
    Language1 = 0x0090,
    Language2 = 0x0091,
    Language3 = 0x0092,
    Language4 = 0x0093,
    Language5 = 0x0094,
    Language6 = 0x0095,
    Language7 = 0x0096,
    Language8 = 0x0097,
    Language9 = 0x0098,
    AlternateErase = 0x0099,
    SystemRequest = 0x009A,
    Cancel = 0x009B,
    Clear = 0x009C,
    Prior = 0x009D,
    Return = 0x009E,
    Separator = 0x009F,
    Out = 0x00A0,
    Oper = 0x00A1,
    ClearAgain = 0x00A2,
    Crsel = 0x00A3,
    Exsel = 0x00A4,
    SystemPower = 0x00A5,
    SystemSleep = 0x00A6,
    SystemWake = 0x00A7,
    AudioMute = 0x00A8,
    AudioVolUp = 0x00A9,
    AudioVolDown = 0x00AA,
    MediaNextTrack = 0x00AB,
    MediaPrevTrack = 0x00AC,
    MediaStop = 0x00AD,
    MediaPlayPause = 0x00AE,
    MediaSelect = 0x00AF,
    MediaEject = 0x00B0,
    Mail = 0x00B1,
    Calculator = 0x00B2,
    MyComputer = 0x00B3,
    WwwSearch = 0x00B4,
    WwwHome = 0x00B5,
    WwwBack = 0x00B6,
    WwwForward = 0x00B7,
    WwwStop = 0x00B8,
    WwwRefresh = 0x00B9,
    WwwFavorites = 0x00BA,
    MediaFastForward = 0x00BB,
    MediaRewind = 0x00BC,
    BrightnessUp = 0x00BD,
    BrightnessDown = 0x00BE,
    ControlPanel = 0x00BF,
    Assistant = 0x00C0,
    MissionControl = 0x00C1,
    Launchpad = 0x00C2,
    MouseUp = 0x00CD,
    MouseDown = 0x00CE,
    MouseLeft = 0x00CF,
    MouseRight = 0x00D0,
    MouseBtn1 = 0x00D1,
    MouseBtn2 = 0x00D2,
    MouseBtn3 = 0x00D3,
    MouseBtn4 = 0x00D4,
    MouseBtn5 = 0x00D5,
    MouseBtn6 = 0x00D6,
    MouseBtn7 = 0x00D7,
    MouseBtn8 = 0x00D8,
    MouseWheelUp = 0x00D9,
    MouseWheelDown = 0x00DA,
    MouseWheelLeft = 0x00DB,
    MouseWheelRight = 0x00DC,
    MouseAccel0 = 0x00DD,
    MouseAccel1 = 0x00DE,
    MouseAccel2 = 0x00DF,
    LCtrl = 0x00E0,
    LShift = 0x00E1,
    LAlt = 0x00E2,
    LGui = 0x00E3,
    RCtrl = 0x00E4,
    RShift = 0x00E5,
    RAlt = 0x00E6,
    RGui = 0x00E7,
    // Magic keycodes, use 0x100 ~ 0x1FF
    MagicSwapControlCapsLock = 0x100,
    MagicUnswapControlCapsLock = 0x101,
    MagicToggleControlCapsLock = 0x102,
    MagicCapsLockAsControlOff = 0x103,
    MagicCapsLockAsControlOn = 0x104,
    MagicSwapLaltLGui = 0x105,
    MagicUnswapLaltLGui = 0x106,
    MagicSwapRaltRGui = 0x107,
    MagicUnswapRaltRGui = 0x108,
    MagicGuiOn = 0x109,
    MagicGuiOff = 0x10A,
    MagicToggleGui = 0x10B,
    MagicSwapGraveEsc = 0x10C,
    MagicUnswapGraveEsc = 0x10D,
    MagicSwapBackslashBackspace = 0x10E,
    MagicUnswapBackslashBackspace = 0x10F,
    MagicToggleBackslashBackspace = 0x110,
    MagicNkroOn = 0x111,
    MagicNkroOff = 0x112,
    MagicToggleNkro = 0x113,
    MagicSwapAltGui = 0x114,
    MagicUnswapAltGui = 0x115,
    MagicToggleAltGui = 0x116,
    MagicSwapLctlLGui = 0x117,
    MagicUnswapLctlLGui = 0x118,
    MagicSwapRctlRGui = 0x119,
    MagicUnswapRctlRGui = 0x11A,
    MagicSwapCtlGui = 0x11B,
    MagicUnswapCtlGui = 0x11C,
    MagicToggleCtlGui = 0x11D,
    MagicEeHandsLeft = 0x11E,
    MagicEeHandsRight = 0x11F,
    MagicSwapEscapeCapsLock = 0x120,
    MagicUnswapEscapeCapsLock = 0x121,
    MagicToggleEscapeCapsLock = 0x122,
    // Midi keycodes, use 0x200 ~ 0x2FF
    MidiOn = 0x200,
    MidiOff = 0x201,
    MidiToggle = 0x202,
    MidiNoteC0 = 0x203,
    MidiNoteCSharp0 = 0x204,
    MidiNoteD0 = 0x205,
    MidiNoteDSharp0 = 0x206,
    MidiNoteE0 = 0x207,
    MidiNoteF0 = 0x208,
    MidiNoteFSharp0 = 0x209,
    MidiNoteG0 = 0x20A,
    MidiNoteGSharp0 = 0x20B,
    MidiNoteA0 = 0x20C,
    MidiNoteASharp0 = 0x20D,
    MidiNoteB0 = 0x20E,
    MidiNoteC1 = 0x20F,
    MidiNoteCSharp1 = 0x210,
    MidiNoteD1 = 0x211,
    MidiNoteDSharp1 = 0x212,
    MidiNoteE1 = 0x213,
    MidiNoteF1 = 0x214,
    MidiNoteFSharp1 = 0x215,
    MidiNoteG1 = 0x216,
    MidiNoteGSharp1 = 0x217,
    MidiNoteA1 = 0x218,
    MidiNoteASharp1 = 0x219,
    MidiNoteB1 = 0x21A,
    MidiNoteC2 = 0x21B,
    MidiNoteCSharp2 = 0x21C,
    MidiNoteD2 = 0x21D,
    MidiNoteDSharp2 = 0x21E,
    MidiNoteE2 = 0x21F,
    MidiNoteF2 = 0x220,
    MidiNoteFSharp2 = 0x221,
    MidiNoteG2 = 0x222,
    MidiNoteGSharp2 = 0x223,
    MidiNoteA2 = 0x224,
    MidiNoteASharp2 = 0x225,
    MidiNoteB2 = 0x226,
    MidiNoteC3 = 0x227,
    MidiNoteCSharp3 = 0x228,
    MidiNoteD3 = 0x229,
    MidiNoteDSharp3 = 0x22A,
    MidiNoteE3 = 0x22B,
    MidiNoteF3 = 0x22C,
    MidiNoteFSharp3 = 0x22D,
    MidiNoteG3 = 0x22E,
    MidiNoteGSharp3 = 0x22F,
    MidiNoteA3 = 0x230,
    MidiNoteASharp3 = 0x231,
    MidiNoteB3 = 0x232,
    MidiNoteC4 = 0x233,
    MidiNoteCSharp4 = 0x234,
    MidiNoteD4 = 0x235,
    MidiNoteDSharp4 = 0x236,
    MidiNoteE4 = 0x237,
    MidiNoteF4 = 0x238,
    MidiNoteFSharp4 = 0x239,
    MidiNoteG4 = 0x23A,
    MidiNoteGSharp4 = 0x23B,
    MidiNoteA4 = 0x23C,
    MidiNoteASharp4 = 0x23D,
    MidiNoteB4 = 0x23E,
    MidiNoteC5 = 0x23F,
    MidiNoteCSharp5 = 0x240,
    MidiNoteD5 = 0x241,
    MidiNoteDSharp5 = 0x242,
    MidiNoteE5 = 0x243,
    MidiNoteF5 = 0x244,
    MidiNoteFSharp5 = 0x245,
    MidiNoteG5 = 0x246,
    MidiNoteGSharp5 = 0x247,
    MidiNoteA5 = 0x248,
    MidiNoteASharp5 = 0x249,
    MidiNoteB5 = 0x24A,
    MidiOctaveN2 = 0x24B,
    MidiOctaveN1 = 0x24C,
    MidiOctave0 = 0x24D,
    MidiOctave1 = 0x24E,
    MidiOctave2 = 0x24F,
    MidiOctave3 = 0x250,
    MidiOctave4 = 0x251,
    MidiOctave5 = 0x252,
    MidiOctave6 = 0x253,
    MidiOctave7 = 0x254,
    MidiOctaveDOWN = 0x255,
    MidiOctaveUP = 0x256,
    MidiTransposeN6 = 0x257,
    MidiTransposeN5 = 0x258,
    MidiTransposeN4 = 0x259,
    MidiTransposeN3 = 0x25A,
    MidiTransposeN2 = 0x25B,
    MidiTransposeN1 = 0x25C,
    MidiTranspose0 = 0x25D,
    MidiTranspose1 = 0x25E,
    MidiTranspose2 = 0x25F,
    MidiTranspose3 = 0x260,
    MidiTranspose4 = 0x261,
    MidiTranspose5 = 0x262,
    MidiTranspose6 = 0x263,
    MidiTransposeDown = 0x264,
    MidiTransposeUp = 0x265,
    MidiVelocity0 = 0x266,
    MidiVelocity1 = 0x267,
    MidiVelocity2 = 0x268,
    MidiVelocity3 = 0x269,
    MidiVelocity4 = 0x26A,
    MidiVelocity5 = 0x26B,
    MidiVelocity6 = 0x26C,
    MidiVelocity7 = 0x26D,
    MidiVelocity8 = 0x26E,
    MidiVelocity9 = 0x26F,
    MidiVelocity10 = 0x270,
    MidiVelocityDOWN = 0x271,
    MidiVelocityUP = 0x272,
    MidiChannel1 = 0x273,
    MidiChannel2 = 0x274,
    MidiChannel3 = 0x275,
    MidiChannel4 = 0x276,
    MidiChannel5 = 0x277,
    MidiChannel6 = 0x278,
    MidiChannel7 = 0x279,
    MidiChannel8 = 0x27A,
    MidiChannel9 = 0x27B,
    MidiChannel10 = 0x27C,
    MidiChannel11 = 0x27D,
    MidiChannel12 = 0x27E,
    MidiChannel13 = 0x27F,
    MidiChannel14 = 0x280,
    MidiChannel15 = 0x281,
    MidiChannel16 = 0x282,
    MidiChannelDOWN = 0x283,
    MidiChannelUP = 0x284,
    MidiAllNotesOff = 0x285,
    MidiSustain = 0x286,
    MidiPortamento = 0x287,
    MidiSostenuto = 0x288,
    MidiSoft = 0x289,
    MidiLegato = 0x28A,
    MidiModulation = 0x28B,
    MidiModulationSpeedDown = 0x28C,
    MidiModulationSpeedUp = 0x28D,
    MidiPitchBendDown = 0x28E,
    MidiPitchBendUp = 0x28F,
    // Sequencer keycodes, use 0x300 ~ 0x30F
    SequencerOn = 0x300,
    SequencerOff = 0x301,
    SequencerToggle = 0x302,
    SequencerTempoDown = 0x303,
    SequencerTempoUp = 0x304,
    SequencerResolutionDown = 0x305,
    SequencerResolutionUp = 0x306,
    SequencerStepsAll = 0x307,
    SequencerStepsClear = 0x308,
    // Joystick button keycodes, use 0x400 ~ 0x41F
    JoystickButton0 = 0x400,
    JoystickButton1 = 0x401,
    JoystickButton2 = 0x402,
    JoystickButton3 = 0x403,
    JoystickButton4 = 0x404,
    JoystickButton5 = 0x405,
    JoystickButton6 = 0x406,
    JoystickButton7 = 0x407,
    JoystickButton8 = 0x408,
    JoystickButton9 = 0x409,
    JoystickButton10 = 0x40A,
    JoystickButton11 = 0x40B,
    JoystickButton12 = 0x40C,
    JoystickButton13 = 0x40D,
    JoystickButton14 = 0x40E,
    JoystickButton15 = 0x40F,
    JoystickButton16 = 0x410,
    JoystickButton17 = 0x411,
    JoystickButton18 = 0x412,
    JoystickButton19 = 0x413,
    JoystickButton20 = 0x414,
    JoystickButton21 = 0x415,
    JoystickButton22 = 0x416,
    JoystickButton23 = 0x417,
    JoystickButton24 = 0x418,
    JoystickButton25 = 0x419,
    JoystickButton26 = 0x41A,
    JoystickButton27 = 0x41B,
    JoystickButton28 = 0x41C,
    JoystickButton29 = 0x41D,
    JoystickButton30 = 0x41E,
    JoystickButton31 = 0x41F,
    // Programmable button keycodes, use 0x420 ~ 0x43F
    ProgrammableButton1 = 0x420,
    ProgrammableButton2 = 0x421,
    ProgrammableButton3 = 0x422,
    ProgrammableButton4 = 0x423,
    ProgrammableButton5 = 0x424,
    ProgrammableButton6 = 0x425,
    ProgrammableButton7 = 0x426,
    ProgrammableButton8 = 0x427,
    ProgrammableButton9 = 0x428,
    ProgrammableButton10 = 0x429,
    ProgrammableButton11 = 0x42A,
    ProgrammableButton12 = 0x42B,
    ProgrammableButton13 = 0x42C,
    ProgrammableButton14 = 0x42D,
    ProgrammableButton15 = 0x42E,
    ProgrammableButton16 = 0x42F,
    ProgrammableButton17 = 0x430,
    ProgrammableButton18 = 0x431,
    ProgrammableButton19 = 0x432,
    ProgrammableButton20 = 0x433,
    ProgrammableButton21 = 0x434,
    ProgrammableButton22 = 0x435,
    ProgrammableButton23 = 0x436,
    ProgrammableButton24 = 0x437,
    ProgrammableButton25 = 0x438,
    ProgrammableButton26 = 0x439,
    ProgrammableButton27 = 0x43A,
    ProgrammableButton28 = 0x43B,
    ProgrammableButton29 = 0x43C,
    ProgrammableButton30 = 0x43D,
    ProgrammableButton31 = 0x43E,
    ProgrammableButton32 = 0x43F,
    // Audio keycodes, use 0x460 ~ 0x47F
    AudioOn = 0x460,
    AudioOff = 0x461,
    AudioToggle = 0x462,
    AudioClickyToggle = 0x46A,
    AudioClickyOn = 0x46B,
    AudioClickyOff = 0x46C,
    AudioClickyUp = 0x46D,
    AudioClickyDown = 0x46E,
    AudioClickyReset = 0x46F,
    MusicOn = 0x470,
    MusicOff = 0x471,
    MusicToggle = 0x472,
    MusicModeNext = 0x473,
    AudioVoiceNext = 0x474,
    AudioVoicePrevious = 0x475,
    // Steno keycodes, use 0x4F0 ~ 0x4FF
    StenoBolt = 0x4F0,
    StenoGemini = 0x4F1,
    StenoComb = 0x4F2,
    StenoCombMax = 0x4FC,
    // Macro keycodes, use 0x500 ~ 0x5FF
    Macro0 = 0x500,
    Macro1 = 0x501,
    Macro2 = 0x502,
    Macro3 = 0x503,
    Macro4 = 0x504,
    Macro5 = 0x505,
    Macro6 = 0x506,
    Macro7 = 0x507,
    Macro8 = 0x508,
    Macro9 = 0x509,
    Macro10 = 0x50A,
    Macro11 = 0x50B,
    Macro12 = 0x50C,
    Macro13 = 0x50D,
    Macro14 = 0x50E,
    Macro15 = 0x50F,
    Macro16 = 0x510,
    Macro17 = 0x511,
    Macro18 = 0x512,
    Macro19 = 0x513,
    Macro20 = 0x514,
    Macro21 = 0x515,
    Macro22 = 0x516,
    Macro23 = 0x517,
    Macro24 = 0x518,
    Macro25 = 0x519,
    Macro26 = 0x51A,
    Macro27 = 0x51B,
    Macro28 = 0x51C,
    Macro29 = 0x51D,
    Macro30 = 0x51E,
    Macro31 = 0x51F,
    // Backlight and RGB keycodes, uses 0x600 ~ 0x6FF
    BacklightOn = 0x600,
    BacklightOff = 0x601,
    BacklightToggle = 0x602,
    BacklightDown = 0x603,
    BacklightUp = 0x604,
    BacklightStep = 0x605,
    BacklightToggleBreathing = 0x606,
    RgbTog = 0x620,
    RgbModeForward = 0x621,
    RgbModeReverse = 0x622,
    RgbHui = 0x623,
    RgbHud = 0x624,
    RgbSai = 0x625,
    RgbSad = 0x626,
    RgbVai = 0x627,
    RgbVad = 0x628,
    RgbSpi = 0x629,
    RgbSpd = 0x62A,
    RgbModePlain = 0x62B,
    RgbModeBreathe = 0x62C,
    RgbModeRainbow = 0x62D,
    RgbModeSwirl = 0x62E,
    RgbModeSnake = 0x62F,
    RgbModeKnight = 0x630,
    RgbModeXmas = 0x631,
    RgbModeGradient = 0x632,
    // Not in vial
    RgbModeRgbtest = 0x633,
    RgbModeTwinkle = 0x634,
    // Internal functional keycodes, use 0x700 ~ 0x7FF
    Bootloader = 0x700,
    Reboot = 0x701,
    DebugToggle = 0x702,
    ClearEeprom = 0x703,
    Make = 0x704,
    AutoShiftDown = 0x710,
    AutoShiftUp = 0x711,
    AutoShiftReport = 0x712,
    AutoShiftOn = 0x713,
    AutoShiftOff = 0x714,
    AutoShiftToggle = 0x715,
    GraveEscape = 0x716,
    VelocikeyToggle = 0x717,
    SpaceCadetLCtrlParenthesisOpen = 0x718,
    SpaceCadetRCtrlParenthesisClose = 0x719,
    SpaceCadetLShiftParenthesisOpen = 0x71A,
    SpaceCadetRShiftParenthesisClose = 0x71B,
    SpaceCadetLAltParenthesisOpen = 0x71C,
    SpaceCadetRAltParenthesisClose = 0x71D,
    SpaceCadetRShiftEnter = 0x71E,
    OutputAuto = 0x720,
    OutputUsb = 0x721,
    OutputBluetooth = 0x722,
    UnicodeModeNext = 0x730,
    UnicodeModePrevious = 0x731,
    UnicodeModeMacos = 0x732,
    UnicodeModeLinux = 0x733,
    UnicodeModeWindows = 0x734,
    UnicodeModeBsd = 0x735,
    UnicodeModeWincompose = 0x736,
    UnicodeModeEmacs = 0x737,
    HapticOn = 0x740,
    HapticOff = 0x741,
    HapticToggle = 0x742,
    HapticReset = 0x743,
    HapticFeedbackToggle = 0x744,
    HapticBuzzToggle = 0x745,
    HapticModeNext = 0x746,
    HapticModePrevious = 0x747,
    HapticContinuousToggle = 0x748,
    HapticContinuousUp = 0x749,
    HapticContinuousDown = 0x74A,
    HapticDwellUp = 0x74B,
    HapticDwellDown = 0x74C,
    ComboOn = 0x750,
    ComboOff = 0x751,
    ComboToggle = 0x752,
    DynamicMacroRecordStart1 = 0x753,
    DynamicMacroRecordStart2 = 0x754,
    DynamicMacroRecordStop = 0x755,
    DynamicMacroPlay1 = 0x756,
    DynamicMacroPlay2 = 0x757,
    Leader = 0x758,
    Lock = 0x759,
    OneShotOn = 0x75A,
    OneShotOff = 0x75B,
    OneShotToggle = 0x75C,
    KeyOverrideToggle = 0x75D,
    KeyOverrideOn = 0x75E,
    KeyOverrideOff = 0x75F,
    SecureLock = 0x760,
    SecureUnlock = 0x761,
    SecureToggle = 0x762,
    SecureRequest = 0x763,
    DynamicTappingTermPrint = 0x770,
    DynamicTappingTermUp = 0x771,
    DynamicTappingTermDown = 0x772,
    CapsWordToggle = 0x773,
    AutocorrectOn = 0x774,
    AutocorrectOff = 0x775,
    AutocorrectToggle = 0x776,
    TriLayerLower = 0x777,
    TriLayerUpper = 0x778,
    RepeatKey = 0x779,
    AltRepeatKey = 0x77A,
    // Kb keycodes, use 0x800 ~ 0x81F
    Kb0 = 0x800,
    Kb1 = 0x801,
    Kb2 = 0x802,
    Kb3 = 0x803,
    Kb4 = 0x804,
    Kb5 = 0x805,
    Kb6 = 0x806,
    Kb7 = 0x807,
    Kb8 = 0x808,
    Kb9 = 0x809,
    Kb10 = 0x80A,
    Kb11 = 0x80B,
    Kb12 = 0x80C,
    Kb13 = 0x80D,
    Kb14 = 0x80E,
    Kb15 = 0x80F,
    Kb16 = 0x810,
    Kb17 = 0x811,
    Kb18 = 0x812,
    Kb19 = 0x813,
    Kb20 = 0x814,
    Kb21 = 0x815,
    Kb22 = 0x816,
    Kb23 = 0x817,
    Kb24 = 0x818,
    Kb25 = 0x819,
    Kb26 = 0x81A,
    Kb27 = 0x81B,
    Kb28 = 0x81C,
    Kb29 = 0x81D,
    Kb30 = 0x81E,
    Kb31 = 0x81F,
    // User keycodes, use 0x840 ~ 0x85F
    User0 = 0x840,
    User1 = 0x841,
    User2 = 0x842,
    User3 = 0x843,
    User4 = 0x844,
    User5 = 0x845,
    User6 = 0x846,
    User7 = 0x847,
    User8 = 0x848,
    User9 = 0x849,
    User10 = 0x84A,
    User11 = 0x84B,
    User12 = 0x84C,
    User13 = 0x84D,
    User14 = 0x84E,
    User15 = 0x84F,
    User16 = 0x850,
    User17 = 0x851,
    User18 = 0x852,
    User19 = 0x853,
    User20 = 0x854,
    User21 = 0x855,
    User22 = 0x856,
    User23 = 0x857,
    User24 = 0x858,
    User25 = 0x859,
    User26 = 0x85A,
    User27 = 0x85B,
    User28 = 0x85C,
    User29 = 0x85D,
    User30 = 0x85E,
    User31 = 0x85F,
}

impl KeyCode {
    /// Returns `true` if the keycode is basic keycode
    pub(crate) fn is_basic(self) -> bool {
        KeyCode::No <= self && self <= KeyCode::RGui
    }

    /// Returns `true` if the keycode is a modifier keycode
    pub(crate) fn is_modifier(self) -> bool {
        KeyCode::LCtrl <= self && self <= KeyCode::RGui
    }

    /// Returns the byte with the bit corresponding to the USB HID
    /// modifier bitfield set.
    pub(crate) fn as_modifier_bit(self) -> u8 {
        if self.is_modifier() {
            1 << (self as u16 as u8 - KeyCode::LCtrl as u16 as u8)
        } else {
            0
        }
    }

    /// Returns `true` if the keycode is a system keycode
    pub(crate) fn is_system(self) -> bool {
        KeyCode::SystemPower <= self && self <= KeyCode::SystemWake
    }

    /// Returns `true` if the keycode is a keycode in consumer page
    pub(crate) fn is_consumer(self) -> bool {
        KeyCode::AudioMute <= self && self <= KeyCode::Launchpad
    }

    /// Returns `true` if the keycode is a mouse keycode
    pub(crate) fn is_mouse_key(self) -> bool {
        KeyCode::MouseUp <= self && self <= KeyCode::MouseAccel2
    }

    /// Returns `true` if the keycode is a magic keycode
    pub(crate) fn is_magic(self) -> bool {
        KeyCode::MagicSwapControlCapsLock <= self && self <= KeyCode::MagicToggleEscapeCapsLock
    }

    /// Returns `true` if the keycode is a midi keycode
    pub(crate) fn is_midi(self) -> bool {
        KeyCode::MidiOn <= self && self <= KeyCode::MidiPitchBendUp
    }

    /// Returns `true` if the keycode is a sequencer keycode
    pub(crate) fn is_sequencer(self) -> bool {
        KeyCode::SequencerOn <= self && self <= KeyCode::SequencerStepsClear
    }

    /// Returns `true` if the keycode is a joystick keycode
    pub(crate) fn is_joystick(self) -> bool {
        KeyCode::JoystickButton0 <= self && self <= KeyCode::JoystickButton31
    }

    /// Returns `true` if the keycode is a programmable button keycode
    pub(crate) fn is_programmable_button(self) -> bool {
        KeyCode::ProgrammableButton1 <= self && self <= KeyCode::ProgrammableButton32
    }

    /// Returns `true` if the keycode is a audio keycode
    /// Note: Basic audio keycodes are not included
    pub(crate) fn is_audio(self) -> bool {
        KeyCode::AudioOn <= self && self <= KeyCode::AudioVoicePrevious
    }

    /// Returns `true` if the keycode is a steno keycode
    pub(crate) fn is_steno(self) -> bool {
        KeyCode::StenoBolt <= self && self <= KeyCode::StenoCombMax
    }

    /// Returns `true` if the keycode is a macro keycode
    pub(crate) fn is_macro(self) -> bool {
        KeyCode::Macro0 <= self && self <= KeyCode::Macro31
    }

    /// Returns `true` if the keycode is a backlight keycode
    pub(crate) fn is_backlight(self) -> bool {
        KeyCode::BacklightOn <= self && self <= KeyCode::BacklightToggleBreathing
    }

    /// Returns `true` if the keycode is a rgb keycode
    pub(crate) fn is_rgb(self) -> bool {
        KeyCode::RgbTog <= self && self <= KeyCode::RgbModeTwinkle
    }

    /// Returns `true` if the keycode is defined by rmk to achieve special functionalities, such as reboot keyboard, goto bootloader, etc.
    pub(crate) fn is_rmk(self) -> bool {
        KeyCode::Bootloader <= self && self <= KeyCode::AltRepeatKey
    }

    /// Returns `true` if the keycode is a kb keycode
    pub(crate) fn is_kb(self) -> bool {
        KeyCode::Kb0 <= self && self <= KeyCode::Kb31
    }

    /// Returns `true` if the keycode is a user keycode
    pub(crate) fn is_user(self) -> bool {
        KeyCode::User0 <= self && self <= KeyCode::User31
    }

    /// Convert a keycode to usb hid media key
    pub(crate) fn as_consumer_control_usage_id(self) -> MediaKey {
        match self {
            KeyCode::AudioMute => MediaKey::Mute,
            KeyCode::AudioVolUp => MediaKey::VolumeIncrement,
            KeyCode::AudioVolDown => MediaKey::VolumeDecrement,
            KeyCode::MediaNextTrack => MediaKey::NextTrack,
            KeyCode::MediaPrevTrack => MediaKey::PrevTrack,
            KeyCode::MediaStop => MediaKey::Stop,
            KeyCode::MediaPlayPause => MediaKey::PlayPause,
            KeyCode::MediaSelect => MediaKey::Record,
            // KeyCode::MediaEject => None,
            // KeyCode::MediaFastForward => None,
            // KeyCode::MediaRewind => None,
            // KeyCode::BrightnessUp => MediaKey::BrightnessUp,
            // KeyCode::BrightnessDown => MediaKey::BrightnessDown,
            // KeyCode::ControlPanel => None,
            // KeyCode::Assistant => None,
            // KeyCode::MissionControl => None,
            // KeyCode::Launchpad => None,
            _ => MediaKey::Zero,
        }
    }

    /// Convert a keycode to usb hid media key
    pub(crate) fn as_system_control_usage_id(self) -> Option<SystemControlKey> {
        match self {
            KeyCode::SystemPower => Some(SystemControlKey::PowerDown),
            KeyCode::SystemSleep => Some(SystemControlKey::Sleep),
            KeyCode::SystemWake => Some(SystemControlKey::WakeUp),
            _ => None,
        }
    }
}
