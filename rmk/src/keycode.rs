use core::ops::BitOr;

use bitfield_struct::bitfield;
use num_enum::FromPrimitive;

use crate::hid_state::HidModifiers;

/// To represent all combinations of modifiers, at least 5 bits are needed.
/// 1 bit for Left/Right, 4 bits for modifier type. Represented in LSB format.
///
/// | bit4 | bit3 | bit2 | bit1 | bit0 |
/// | --- | --- | --- | --- | --- |
/// | L/R | GUI | ALT |SHIFT| CTRL|
#[bitfield(u8, order = Lsb)]
#[derive(Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ModifierCombination {
    #[bits(1)]
    pub(crate) ctrl: bool,
    #[bits(1)]
    pub(crate) shift: bool,
    #[bits(1)]
    pub(crate) alt: bool,
    #[bits(1)]
    pub(crate) gui: bool,
    #[bits(1)]
    pub(crate) right: bool,
    #[bits(3)]
    _reserved: u8,
}

impl BitOr for ModifierCombination {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self::from_bits(self.into_bits() | rhs.into_bits())
    }
}

impl ModifierCombination {
    pub const fn new_from(right: bool, gui: bool, alt: bool, shift: bool, ctrl: bool) -> Self {
        ModifierCombination::new()
            .with_right(right)
            .with_gui(gui)
            .with_alt(alt)
            .with_shift(shift)
            .with_ctrl(ctrl)
    }

    /// Get modifier hid report bits from modifier combination
    pub(crate) fn to_hid_modifiers(self) -> HidModifiers {
        if !self.right() {
            HidModifiers::new()
                .with_left_ctrl(self.ctrl())
                .with_left_shift(self.shift())
                .with_left_alt(self.alt())
                .with_left_gui(self.gui())
        } else {
            HidModifiers::new()
                .with_right_ctrl(self.ctrl())
                .with_right_shift(self.shift())
                .with_right_alt(self.alt())
                .with_right_gui(self.gui())
        }
    }
}

/// Keys in consumer page
/// Ref: <https://www.usb.org/sites/default/files/documents/hut1_12v2.pdf#page=75>
#[non_exhaustive]
#[repr(u16)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, FromPrimitive)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ConsumerKey {
    #[num_enum(default)]
    Zero = 0x00,
    // 15.5 Display Controls
    SnapShot = 0x65,
    /// <https://www.usb.org/sites/default/files/hutrr41_0.pdf>
    BrightnessUp = 0x6F,
    BrightnessDown = 0x70,
    // 15.7 Transport Controls
    Play = 0xB0,
    Pause = 0xB1,
    Record = 0xB2,
    FastForward = 0xB3,
    Rewind = 0xB4,
    NextTrack = 0xB5,
    PrevTrack = 0xB6,
    StopPlay = 0xB7,
    Eject = 0xB8,
    RandomPlay = 0xB9,
    Repeat = 0xBC,
    StopEject = 0xCC,
    PlayPause = 0xCD,
    // 15.9.1 Audio Controls - Volume
    Mute = 0xE2,
    VolumeIncrement = 0xE9,
    VolumeDecrement = 0xEA,
    Reserved = 0xEB,
    // 15.15 Application Launch Buttons
    Email = 0x18A,
    Calculator = 0x192,
    LocalBrowser = 0x194,
    Lock = 0x19E,
    ControlPanel = 0x19F,
    Assistant = 0x1CB,
    // 15.16 Generic GUI Application Controls
    New = 0x201,
    Open = 0x202,
    Close = 0x203,
    Exit = 0x204,
    Maximize = 0x205,
    Minimize = 0x206,
    Save = 0x207,
    Print = 0x208,
    Properties = 0x209,
    Undo = 0x21A,
    Copy = 0x21B,
    Cut = 0x21C,
    Paste = 0x21D,
    SelectAll = 0x21E,
    Find = 0x21F,
    Search = 0x221,
    Home = 0x223,
    Back = 0x224,
    Forward = 0x225,
    Stop = 0x226,
    Refresh = 0x227,
    Bookmarks = 0x22A,
    NextKeyboardLayoutSelect = 0x29D,
    DesktopShowAllWindows = 0x29F,
    AcSoftKeyLeft = 0x2A0,
}

/// Keys in `Generic Desktop Page`, generally used for system control
/// Ref: <https://www.usb.org/sites/default/files/documents/hut1_12v2.pdf#page=26>
#[non_exhaustive]
#[repr(u16)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, FromPrimitive)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum SystemControlKey {
    #[num_enum(default)]
    Zero = 0x00,
    PowerDown = 0x81,
    Sleep = 0x82,
    WakeUp = 0x83,
    Restart = 0x8F,
}

/// KeyCode is the internal representation of all keycodes, keyboard operations, etc.
/// Use flat representation of keycodes.
#[repr(u16)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, FromPrimitive, Hash)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
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
    /// `a` and `A`
    A = 0x0004,
    /// `b` and `B`
    B = 0x0005,
    /// `c` and `C`
    C = 0x0006,
    /// `d` and `D`
    D = 0x0007,
    /// `e` and `E`
    E = 0x0008,
    /// `f` and `F`
    F = 0x0009,
    /// `g` and `G`
    G = 0x000A,
    /// `h` and `H`
    H = 0x000B,
    /// `i` and `I`
    I = 0x000C,
    /// `j` and `J`
    J = 0x000D,
    /// `k` and `K`
    K = 0x000E,
    /// `l` and `L`
    L = 0x000F,
    /// `m` and `M`
    M = 0x0010,
    /// `n` and `N`
    N = 0x0011,
    /// `o` and `O`
    O = 0x0012,
    /// `p` and `P`
    P = 0x0013,
    /// `q` and `Q`
    Q = 0x0014,
    /// `r` and `R`
    R = 0x0015,
    /// `s` and `S`
    S = 0x0016,
    /// `t` and `T`
    T = 0x0017,
    /// `u` and `U`
    U = 0x0018,
    /// `v` and `V`
    V = 0x0019,
    /// `w` and `W`
    W = 0x001A,
    /// `x` and `X`
    X = 0x001B,
    /// `y` and `Y`
    Y = 0x001C,
    /// `z` and `Z`
    Z = 0x001D,
    /// `1` and `!`
    Kc1 = 0x001E,
    /// `2` and `@`
    Kc2 = 0x001F,
    /// `3` and `#`
    Kc3 = 0x0020,
    /// `4` and `$`
    Kc4 = 0x0021,
    /// `5` and `%`
    Kc5 = 0x0022,
    /// `6` and `^`
    Kc6 = 0x0023,
    /// `7` and `&`
    Kc7 = 0x0024,
    /// `8` and `*`
    Kc8 = 0x0025,
    /// `9` and `(`
    Kc9 = 0x0026,
    /// `0` and `)`
    Kc0 = 0x0027,
    /// `Enter`
    Enter = 0x0028,
    /// `Esc`
    Escape = 0x0029,
    /// `Backspace`
    Backspace = 0x002A,
    /// `Tab`
    Tab = 0x002B,
    /// `Space`
    Space = 0x002C,
    /// `-` and `_`
    Minus = 0x002D,
    /// `=` and `+`
    Equal = 0x002E,
    /// `[` and `{`
    LeftBracket = 0x002F,
    /// `]` and `}`
    RightBracket = 0x0030,
    /// `\` and `|`
    Backslash = 0x0031,
    /// Non-US `#` and `~`
    NonusHash = 0x0032,
    /// `;` and `:`
    Semicolon = 0x0033,
    /// `'` and `"`
    Quote = 0x0034,
    /// `~` and `\``
    Grave = 0x0035,
    /// `,` and `<`
    Comma = 0x0036,
    /// `.` and `>`
    Dot = 0x0037,
    /// `/` and `?`
    Slash = 0x0038,
    /// `CapsLock`
    CapsLock = 0x0039,
    /// `F1`
    F1 = 0x003A,
    /// `F2`
    F2 = 0x003B,
    /// `F3`
    F3 = 0x003C,
    /// `F4`
    F4 = 0x003D,
    /// `F5`
    F5 = 0x003E,
    /// `F6`
    F6 = 0x003F,
    /// `F7`
    F7 = 0x0040,
    /// `F8`
    F8 = 0x0041,
    /// `F9`
    F9 = 0x0042,
    /// `F10`
    F10 = 0x0043,
    /// `F11`
    F11 = 0x0044,
    /// `F12`
    F12 = 0x0045,
    /// Print Screen
    PrintScreen = 0x0046,
    /// Scroll Lock
    ScrollLock = 0x0047,
    /// Pause
    Pause = 0x0048,
    /// Insert
    Insert = 0x0049,
    /// Home
    Home = 0x004A,
    /// Page Up
    PageUp = 0x004B,
    /// Delete
    Delete = 0x004C,
    /// End
    End = 0x004D,
    /// Page Down
    PageDown = 0x004E,
    /// Right arrow
    Right = 0x004F,
    /// Left arrow
    Left = 0x0050,
    /// Down arrow
    Down = 0x0051,
    /// Up arrow
    Up = 0x0052,
    /// Nums Lock
    NumLock = 0x0053,
    /// `/` on keypad
    KpSlash = 0x0054,
    /// `*` on keypad
    KpAsterisk = 0x0055,
    /// `-` on keypad
    KpMinus = 0x0056,
    /// `+` on keypad
    KpPlus = 0x0057,
    /// `Enter` on keypad
    KpEnter = 0x0058,
    /// `1` on keypad
    Kp1 = 0x0059,
    /// `2` on keypad
    Kp2 = 0x005A,
    /// `3` on keypad
    Kp3 = 0x005B,
    /// `4` on keypad
    Kp4 = 0x005C,
    /// `5` on keypad
    Kp5 = 0x005D,
    /// `6` on keypad
    Kp6 = 0x005E,
    /// `7` on keypad
    Kp7 = 0x005F,
    /// `8` on keypad
    Kp8 = 0x0060,
    /// `9` on keypad
    Kp9 = 0x0061,
    /// `0` on keypad
    Kp0 = 0x0062,
    /// `.` on keypad
    KpDot = 0x0063,
    /// Non-US `\` or `|`
    NonusBackslash = 0x0064,
    /// `Application`
    Application = 0x0065,
    /// `Power`
    KbPower = 0x0066,
    /// `=` on keypad
    KpEqual = 0x0067,
    /// `F13`
    F13 = 0x0068,
    /// `F14`
    F14 = 0x0069,
    /// `F15`
    F15 = 0x006A,
    /// `F16`
    F16 = 0x006B,
    /// `F17`
    F17 = 0x006C,
    /// `F18`
    F18 = 0x006D,
    /// `F19`
    F19 = 0x006E,
    /// `F20`
    F20 = 0x006F,
    /// `F21`
    F21 = 0x0070,
    /// `F22`
    F22 = 0x0071,
    /// `F23`
    F23 = 0x0072,
    /// `F24`
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
    /// Mute
    KbMute = 0x007F,
    /// Volume Up
    KbVolumeUp = 0x0080,
    /// Volume Down
    KbVolumeDown = 0x0081,
    /// Locking Caps Lock
    LockingCapsLock = 0x0082,
    /// Locking Num Lock
    LockingNumLock = 0x0083,
    /// Locking scroll lock
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
    /// Brightness Up
    BrightnessUp = 0x00BD,
    /// Brightness Down
    BrightnessDown = 0x00BE,
    ControlPanel = 0x00BF,
    Assistant = 0x00C0,
    MissionControl = 0x00C1,
    Launchpad = 0x00C2,
    /// Mouse Up
    MouseUp = 0x00CD,
    /// Mouse Down
    MouseDown = 0x00CE,
    /// Mouse Left
    MouseLeft = 0x00CF,
    /// Mouse Right
    MouseRight = 0x00D0,
    /// Mouse Button 1(Left)
    MouseBtn1 = 0x00D1,
    /// Mouse Button 2(Right)
    MouseBtn2 = 0x00D2,
    /// Mouse Button 3(Middle)
    MouseBtn3 = 0x00D3,
    /// Mouse Button 4(Back)
    MouseBtn4 = 0x00D4,
    /// Mouse Button 5(Forward)
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
    /// Left Control
    LCtrl = 0x00E0,
    /// Left Shift
    LShift = 0x00E1,
    /// Left Alt
    LAlt = 0x00E2,
    /// Left GUI
    LGui = 0x00E3,
    /// Right Control
    RCtrl = 0x00E4,
    /// Right Shift
    RShift = 0x00E5,
    /// Right Alt
    RAlt = 0x00E6,
    /// Right GUI
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
    pub(crate) fn to_hid_modifiers(self) -> HidModifiers {
        match self {
            KeyCode::LCtrl => HidModifiers::new().with_left_ctrl(true),
            KeyCode::LShift => HidModifiers::new().with_left_shift(true),
            KeyCode::LAlt => HidModifiers::new().with_left_alt(true),
            KeyCode::LGui => HidModifiers::new().with_left_gui(true),
            KeyCode::RCtrl => HidModifiers::new().with_right_ctrl(true),
            KeyCode::RShift => HidModifiers::new().with_right_shift(true),
            KeyCode::RAlt => HidModifiers::new().with_right_alt(true),
            KeyCode::RGui => HidModifiers::new().with_right_gui(true),
            _ => HidModifiers::new(),
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

    /// Returns `true` if the keycode is a combo keycode
    pub(crate) fn is_combo(self) -> bool {
        KeyCode::ComboOn <= self && self <= KeyCode::ComboToggle
    }

    /// Returns `true` if the keycode is a boot keycode
    pub(crate) fn is_boot(self) -> bool {
        KeyCode::Bootloader <= self && self <= KeyCode::Reboot
    }

    /// Returns `true` if the keycode is a kb keycode
    pub(crate) fn is_kb(self) -> bool {
        KeyCode::Kb0 <= self && self <= KeyCode::Kb31
    }

    /// Returns `true` if the keycode is a user keycode
    pub(crate) fn is_user(self) -> bool {
        KeyCode::User0 <= self && self <= KeyCode::User31
    }

    /// Convert a keycode to macro number
    pub(crate) fn as_macro_index(self) -> Option<u8> {
        if self.is_macro() {
            Some((self as u16 & 0x1F) as u8)
        } else {
            None
        }
    }

    /// Convert a keycode to usb hid media key
    pub(crate) fn as_consumer_control_usage_id(self) -> ConsumerKey {
        match self {
            KeyCode::AudioMute => ConsumerKey::Mute,
            KeyCode::AudioVolUp => ConsumerKey::VolumeIncrement,
            KeyCode::AudioVolDown => ConsumerKey::VolumeDecrement,
            KeyCode::MediaNextTrack => ConsumerKey::NextTrack,
            KeyCode::MediaPrevTrack => ConsumerKey::PrevTrack,
            KeyCode::MediaStop => ConsumerKey::StopPlay,
            KeyCode::MediaPlayPause => ConsumerKey::PlayPause,
            KeyCode::MediaSelect => ConsumerKey::Record,
            KeyCode::MediaEject => ConsumerKey::Eject,
            KeyCode::Mail => ConsumerKey::Email,
            KeyCode::Calculator => ConsumerKey::Calculator,
            KeyCode::MyComputer => ConsumerKey::LocalBrowser,
            KeyCode::WwwSearch => ConsumerKey::Search,
            KeyCode::WwwHome => ConsumerKey::Home,
            KeyCode::WwwBack => ConsumerKey::Back,
            KeyCode::WwwForward => ConsumerKey::Forward,
            KeyCode::WwwStop => ConsumerKey::Stop,
            KeyCode::WwwRefresh => ConsumerKey::Refresh,
            KeyCode::WwwFavorites => ConsumerKey::Bookmarks,
            KeyCode::MediaFastForward => ConsumerKey::FastForward,
            KeyCode::MediaRewind => ConsumerKey::Rewind,
            KeyCode::BrightnessUp => ConsumerKey::BrightnessUp,
            KeyCode::BrightnessDown => ConsumerKey::BrightnessDown,
            KeyCode::ControlPanel => ConsumerKey::ControlPanel,
            KeyCode::Assistant => ConsumerKey::Assistant,
            KeyCode::MissionControl => ConsumerKey::DesktopShowAllWindows,
            KeyCode::Launchpad => ConsumerKey::AcSoftKeyLeft,
            _ => ConsumerKey::Zero,
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

    /// Convert a ascii chat to keycode
    pub(crate) fn from_ascii(ascii: u8) -> (Self, bool) {
        match ascii {
            b'0' => (KeyCode::Kc0, false),
            b'1' => (KeyCode::Kc1, false),
            b'2' => (KeyCode::Kc2, false),
            b'3' => (KeyCode::Kc3, false),
            b'4' => (KeyCode::Kc4, false),
            b'5' => (KeyCode::Kc5, false),
            b'6' => (KeyCode::Kc6, false),
            b'7' => (KeyCode::Kc7, false),
            b'8' => (KeyCode::Kc8, false),
            b'9' => (KeyCode::Kc9, false),
            b'a' => (KeyCode::A, false),
            b'b' => (KeyCode::B, false),
            b'c' => (KeyCode::C, false),
            b'd' => (KeyCode::D, false),
            b'e' => (KeyCode::E, false),
            b'f' => (KeyCode::F, false),
            b'g' => (KeyCode::G, false),
            b'h' => (KeyCode::H, false),
            b'i' => (KeyCode::I, false),
            b'j' => (KeyCode::J, false),
            b'k' => (KeyCode::K, false),
            b'l' => (KeyCode::L, false),
            b'm' => (KeyCode::M, false),
            b'n' => (KeyCode::N, false),
            b'o' => (KeyCode::O, false),
            b'p' => (KeyCode::P, false),
            b'q' => (KeyCode::Q, false),
            b'r' => (KeyCode::R, false),
            b's' => (KeyCode::S, false),
            b't' => (KeyCode::T, false),
            b'u' => (KeyCode::U, false),
            b'v' => (KeyCode::V, false),
            b'w' => (KeyCode::W, false),
            b'x' => (KeyCode::X, false),
            b'y' => (KeyCode::Y, false),
            b'z' => (KeyCode::Z, false),
            b'A' => (KeyCode::A, true),
            b'B' => (KeyCode::B, true),
            b'C' => (KeyCode::C, true),
            b'D' => (KeyCode::D, true),
            b'E' => (KeyCode::E, true),
            b'F' => (KeyCode::F, true),
            b'G' => (KeyCode::G, true),
            b'H' => (KeyCode::H, true),
            b'I' => (KeyCode::I, true),
            b'J' => (KeyCode::J, true),
            b'K' => (KeyCode::K, true),
            b'L' => (KeyCode::L, true),
            b'M' => (KeyCode::M, true),
            b'N' => (KeyCode::N, true),
            b'O' => (KeyCode::O, true),
            b'P' => (KeyCode::P, true),
            b'Q' => (KeyCode::Q, true),
            b'R' => (KeyCode::R, true),
            b'S' => (KeyCode::S, true),
            b'T' => (KeyCode::T, true),
            b'U' => (KeyCode::U, true),
            b'V' => (KeyCode::V, true),
            b'W' => (KeyCode::W, true),
            b'X' => (KeyCode::X, true),
            b'Y' => (KeyCode::Y, true),
            b'Z' => (KeyCode::Z, true),
            b'!' => (KeyCode::Kc1, true),
            b'@' => (KeyCode::Kc2, true),
            b'#' => (KeyCode::Kc3, true),
            b'$' => (KeyCode::Kc4, true),
            b'%' => (KeyCode::Kc5, true),
            b'^' => (KeyCode::Kc6, true),
            b'&' => (KeyCode::Kc7, true),
            b'*' => (KeyCode::Kc8, true),
            b'(' => (KeyCode::Kc9, true),
            b')' => (KeyCode::Kc0, true),
            b'-' => (KeyCode::Minus, false),
            b'_' => (KeyCode::Minus, true),
            b'=' => (KeyCode::Equal, false),
            b'+' => (KeyCode::Equal, true),
            b'[' => (KeyCode::LeftBracket, false),
            b']' => (KeyCode::RightBracket, false),
            b'{' => (KeyCode::LeftBracket, true),
            b'}' => (KeyCode::RightBracket, true),
            b';' => (KeyCode::Semicolon, false),
            b':' => (KeyCode::Semicolon, true),
            b'\'' => (KeyCode::Quote, false),
            b'"' => (KeyCode::Quote, true),
            b'`' => (KeyCode::Grave, false),
            b'~' => (KeyCode::Grave, true),
            b'\\' => (KeyCode::Backslash, true),
            b'|' => (KeyCode::Backslash, false),
            b',' => (KeyCode::Comma, false),
            b'<' => (KeyCode::Comma, true),
            b'.' => (KeyCode::Dot, false),
            b'>' => (KeyCode::Dot, true),
            b'/' => (KeyCode::Slash, false),
            b'?' => (KeyCode::Slash, false),
            b' ' => (KeyCode::Space, false),
            b'\n' => (KeyCode::Enter, false),
            b'\t' => (KeyCode::Tab, false),
            b'\x08' => (KeyCode::Backspace, false),
            b'\x1B' => (KeyCode::Escape, false),
            b'\x7F' => (KeyCode::Delete, false),
            _ => (KeyCode::No, false),
        }
    }
}
