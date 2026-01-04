use serde::{Deserialize, Serialize};
use strum::FromRepr;

use crate::modifier::ModifierCombination;

// All key codes defined in HID spec
#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord, FromRepr)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum HidKeyCode {
    /// Reserved, no-key.
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
}

impl HidKeyCode {
    /// Returns `true` if the keycode is a simple keycode defined in HID spec
    pub fn is_simple_key(self) -> bool {
        HidKeyCode::No <= self && self <= HidKeyCode::MouseAccel2
    }

    /// Returns `true` if the keycode is a modifier keycode
    pub fn is_modifier(self) -> bool {
        HidKeyCode::LCtrl <= self && self <= HidKeyCode::RGui
    }

    /// Returns `true` if the keycode is a mouse keycode
    pub fn is_mouse_key(self) -> bool {
        HidKeyCode::MouseUp <= self && self <= HidKeyCode::MouseAccel2
    }

    /// Returns the byte with the bit corresponding to the USB HID
    /// modifier bitfield set.
    pub fn to_hid_modifiers(self) -> ModifierCombination {
        match self {
            HidKeyCode::LCtrl => ModifierCombination::LCTRL,
            HidKeyCode::LShift => ModifierCombination::LSHIFT,
            HidKeyCode::LAlt => ModifierCombination::LALT,
            HidKeyCode::LGui => ModifierCombination::LGUI,
            HidKeyCode::RCtrl => ModifierCombination::RCTRL,
            HidKeyCode::RShift => ModifierCombination::RSHIFT,
            HidKeyCode::RAlt => ModifierCombination::RALT,
            HidKeyCode::RGui => ModifierCombination::RGUI,
            _ => ModifierCombination::new(),
        }
    }

    /// Does current keycode continues Caps Word?
    pub fn is_caps_word_continue_key(self) -> bool {
        if self >= HidKeyCode::A && self <= HidKeyCode::Z {
            return true;
        }
        if self >= HidKeyCode::Kc1 && self <= HidKeyCode::Kc0 {
            return true;
        }
        if self == HidKeyCode::Minus || self == HidKeyCode::Backspace || self == HidKeyCode::Delete {
            return true;
        }
        false
    }

    /// Does current keycode is to be shifted by Caps Word?
    pub fn is_caps_word_shifted_key(self) -> bool {
        if self >= HidKeyCode::A && self <= HidKeyCode::Z {
            return true;
        }
        if self == HidKeyCode::Minus {
            return true;
        }
        false
    }

    /// Some hid keycodes are processed as consumer keys, for compatibility
    pub fn process_as_consumer(&self) -> Option<ConsumerKey> {
        match self {
            HidKeyCode::AudioMute => Some(ConsumerKey::Mute),
            HidKeyCode::AudioVolUp => Some(ConsumerKey::VolumeIncrement),
            HidKeyCode::AudioVolDown => Some(ConsumerKey::VolumeDecrement),
            HidKeyCode::MediaNextTrack => Some(ConsumerKey::NextTrack),
            HidKeyCode::MediaPrevTrack => Some(ConsumerKey::PrevTrack),
            HidKeyCode::MediaStop => Some(ConsumerKey::StopPlay),
            HidKeyCode::MediaPlayPause => Some(ConsumerKey::PlayPause),
            HidKeyCode::MediaSelect => Some(ConsumerKey::Record),
            HidKeyCode::MediaEject => Some(ConsumerKey::Eject),
            HidKeyCode::Mail => Some(ConsumerKey::Email),
            HidKeyCode::Calculator => Some(ConsumerKey::Calculator),
            HidKeyCode::MyComputer => Some(ConsumerKey::LocalBrowser),
            HidKeyCode::WwwSearch => Some(ConsumerKey::Search),
            HidKeyCode::WwwHome => Some(ConsumerKey::Home),
            HidKeyCode::WwwBack => Some(ConsumerKey::Back),
            HidKeyCode::WwwForward => Some(ConsumerKey::Forward),
            HidKeyCode::WwwStop => Some(ConsumerKey::Stop),
            HidKeyCode::WwwRefresh => Some(ConsumerKey::Refresh),
            HidKeyCode::WwwFavorites => Some(ConsumerKey::Bookmarks),
            HidKeyCode::MediaFastForward => Some(ConsumerKey::FastForward),
            HidKeyCode::MediaRewind => Some(ConsumerKey::Rewind),
            HidKeyCode::BrightnessUp => Some(ConsumerKey::BrightnessUp),
            HidKeyCode::BrightnessDown => Some(ConsumerKey::BrightnessDown),
            HidKeyCode::ControlPanel => Some(ConsumerKey::ControlPanel),
            HidKeyCode::Assistant => Some(ConsumerKey::Assistant),
            HidKeyCode::MissionControl => Some(ConsumerKey::DesktopShowAllWindows),
            HidKeyCode::Launchpad => Some(ConsumerKey::AcSoftKeyLeft),
            _ => None,
        }
    }

    /// Some hid keycodes are processed as system control keys, for compatibility
    pub fn process_as_system_control(&self) -> Option<SystemControlKey> {
        match self {
            HidKeyCode::SystemPower => Some(SystemControlKey::PowerDown),
            HidKeyCode::SystemSleep => Some(SystemControlKey::Sleep),
            HidKeyCode::SystemWake => Some(SystemControlKey::WakeUp),
            _ => None,
        }
    }
}

impl ::postcard::experimental::max_size::MaxSize for HidKeyCode {
    const POSTCARD_MAX_SIZE: usize = 1usize;
}

impl From<u8> for HidKeyCode {
    fn from(value: u8) -> Self {
        Self::from_repr(value).unwrap_or(HidKeyCode::No)
    }
}

/// Key codes which are not in the HID spec, but still commonly used
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(postcard::experimental::max_size::MaxSize)]
#[cfg_attr(feature = "_codegen", derive(strum::VariantNames))]
pub enum SpecialKey {
    // GraveEscape
    GraveEscape,
    // Repeat
    Repeat,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(postcard::experimental::max_size::MaxSize)]
pub enum KeyCode {
    Hid(HidKeyCode),
    Consumer(ConsumerKey),
    SystemControl(SystemControlKey),
}

/// Keys in consumer page
/// Ref: <https://www.usb.org/sites/default/files/documents/hut1_12v2.pdf#page=75>
#[non_exhaustive]
#[repr(u16)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ConsumerKey {
    No = 0x00,
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

impl ::postcard::experimental::max_size::MaxSize for ConsumerKey {
    const POSTCARD_MAX_SIZE: usize = 3usize;
}

impl ConsumerKey {
    /// Convert ConsumerKey to the corresponding HidKeyCode
    pub fn to_hid_keycode(&self) -> Option<HidKeyCode> {
        match self {
            ConsumerKey::Mute => Some(HidKeyCode::AudioMute),
            ConsumerKey::VolumeIncrement => Some(HidKeyCode::AudioVolUp),
            ConsumerKey::VolumeDecrement => Some(HidKeyCode::AudioVolDown),
            ConsumerKey::NextTrack => Some(HidKeyCode::MediaNextTrack),
            ConsumerKey::PrevTrack => Some(HidKeyCode::MediaPrevTrack),
            ConsumerKey::StopPlay => Some(HidKeyCode::MediaStop),
            ConsumerKey::PlayPause => Some(HidKeyCode::MediaPlayPause),
            ConsumerKey::Record => Some(HidKeyCode::MediaSelect),
            ConsumerKey::Eject => Some(HidKeyCode::MediaEject),
            ConsumerKey::Email => Some(HidKeyCode::Mail),
            ConsumerKey::Calculator => Some(HidKeyCode::Calculator),
            ConsumerKey::LocalBrowser => Some(HidKeyCode::MyComputer),
            ConsumerKey::Search => Some(HidKeyCode::WwwSearch),
            ConsumerKey::Home => Some(HidKeyCode::WwwHome),
            ConsumerKey::Back => Some(HidKeyCode::WwwBack),
            ConsumerKey::Forward => Some(HidKeyCode::WwwForward),
            ConsumerKey::Stop => Some(HidKeyCode::WwwStop),
            ConsumerKey::Refresh => Some(HidKeyCode::WwwRefresh),
            ConsumerKey::Bookmarks => Some(HidKeyCode::WwwFavorites),
            ConsumerKey::FastForward => Some(HidKeyCode::MediaFastForward),
            ConsumerKey::Rewind => Some(HidKeyCode::MediaRewind),
            ConsumerKey::BrightnessUp => Some(HidKeyCode::BrightnessUp),
            ConsumerKey::BrightnessDown => Some(HidKeyCode::BrightnessDown),
            ConsumerKey::ControlPanel => Some(HidKeyCode::ControlPanel),
            ConsumerKey::Assistant => Some(HidKeyCode::Assistant),
            ConsumerKey::DesktopShowAllWindows => Some(HidKeyCode::MissionControl),
            ConsumerKey::AcSoftKeyLeft => Some(HidKeyCode::Launchpad),
            _ => None,
        }
    }
}

/// Keys in `Generic Desktop Page`, generally used for system control
/// Ref: <https://www.usb.org/sites/default/files/documents/hut1_12v2.pdf#page=26>
#[non_exhaustive]
#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum SystemControlKey {
    No = 0x00,
    PowerDown = 0x81,
    Sleep = 0x82,
    WakeUp = 0x83,
    Restart = 0x8F,
}

impl ::postcard::experimental::max_size::MaxSize for SystemControlKey {
    const POSTCARD_MAX_SIZE: usize = 1usize;
}

impl SystemControlKey {
    /// Convert SystemControlKey to the corresponding HidKeyCode
    pub fn to_hid_keycode(&self) -> Option<HidKeyCode> {
        match self {
            SystemControlKey::PowerDown => Some(HidKeyCode::SystemPower),
            SystemControlKey::Sleep => Some(HidKeyCode::SystemSleep),
            SystemControlKey::WakeUp => Some(HidKeyCode::SystemWake),
            _ => None,
        }
    }
}

/// Convert a ascii chat to keycode
/// bool, if the keycode should be shifted
/// assumes en-us keyboard mapping
pub fn from_ascii(ascii: u8) -> (HidKeyCode, bool) {
    match ascii {
        b'0' => (HidKeyCode::Kc0, false),
        b'1' => (HidKeyCode::Kc1, false),
        b'2' => (HidKeyCode::Kc2, false),
        b'3' => (HidKeyCode::Kc3, false),
        b'4' => (HidKeyCode::Kc4, false),
        b'5' => (HidKeyCode::Kc5, false),
        b'6' => (HidKeyCode::Kc6, false),
        b'7' => (HidKeyCode::Kc7, false),
        b'8' => (HidKeyCode::Kc8, false),
        b'9' => (HidKeyCode::Kc9, false),
        b'a' => (HidKeyCode::A, false),
        b'b' => (HidKeyCode::B, false),
        b'c' => (HidKeyCode::C, false),
        b'd' => (HidKeyCode::D, false),
        b'e' => (HidKeyCode::E, false),
        b'f' => (HidKeyCode::F, false),
        b'g' => (HidKeyCode::G, false),
        b'h' => (HidKeyCode::H, false),
        b'i' => (HidKeyCode::I, false),
        b'j' => (HidKeyCode::J, false),
        b'k' => (HidKeyCode::K, false),
        b'l' => (HidKeyCode::L, false),
        b'm' => (HidKeyCode::M, false),
        b'n' => (HidKeyCode::N, false),
        b'o' => (HidKeyCode::O, false),
        b'p' => (HidKeyCode::P, false),
        b'q' => (HidKeyCode::Q, false),
        b'r' => (HidKeyCode::R, false),
        b's' => (HidKeyCode::S, false),
        b't' => (HidKeyCode::T, false),
        b'u' => (HidKeyCode::U, false),
        b'v' => (HidKeyCode::V, false),
        b'w' => (HidKeyCode::W, false),
        b'x' => (HidKeyCode::X, false),
        b'y' => (HidKeyCode::Y, false),
        b'z' => (HidKeyCode::Z, false),
        b'A' => (HidKeyCode::A, true),
        b'B' => (HidKeyCode::B, true),
        b'C' => (HidKeyCode::C, true),
        b'D' => (HidKeyCode::D, true),
        b'E' => (HidKeyCode::E, true),
        b'F' => (HidKeyCode::F, true),
        b'G' => (HidKeyCode::G, true),
        b'H' => (HidKeyCode::H, true),
        b'I' => (HidKeyCode::I, true),
        b'J' => (HidKeyCode::J, true),
        b'K' => (HidKeyCode::K, true),
        b'L' => (HidKeyCode::L, true),
        b'M' => (HidKeyCode::M, true),
        b'N' => (HidKeyCode::N, true),
        b'O' => (HidKeyCode::O, true),
        b'P' => (HidKeyCode::P, true),
        b'Q' => (HidKeyCode::Q, true),
        b'R' => (HidKeyCode::R, true),
        b'S' => (HidKeyCode::S, true),
        b'T' => (HidKeyCode::T, true),
        b'U' => (HidKeyCode::U, true),
        b'V' => (HidKeyCode::V, true),
        b'W' => (HidKeyCode::W, true),
        b'X' => (HidKeyCode::X, true),
        b'Y' => (HidKeyCode::Y, true),
        b'Z' => (HidKeyCode::Z, true),
        b'!' => (HidKeyCode::Kc1, true),
        b'@' => (HidKeyCode::Kc2, true),
        b'#' => (HidKeyCode::Kc3, true),
        b'$' => (HidKeyCode::Kc4, true),
        b'%' => (HidKeyCode::Kc5, true),
        b'^' => (HidKeyCode::Kc6, true),
        b'&' => (HidKeyCode::Kc7, true),
        b'*' => (HidKeyCode::Kc8, true),
        b'(' => (HidKeyCode::Kc9, true),
        b')' => (HidKeyCode::Kc0, true),
        b'-' => (HidKeyCode::Minus, false),
        b'_' => (HidKeyCode::Minus, true),
        b'=' => (HidKeyCode::Equal, false),
        b'+' => (HidKeyCode::Equal, true),
        b'[' => (HidKeyCode::LeftBracket, false),
        b']' => (HidKeyCode::RightBracket, false),
        b'{' => (HidKeyCode::LeftBracket, true),
        b'}' => (HidKeyCode::RightBracket, true),
        b';' => (HidKeyCode::Semicolon, false),
        b':' => (HidKeyCode::Semicolon, true),
        b'\'' => (HidKeyCode::Quote, false),
        b'"' => (HidKeyCode::Quote, true),
        b'`' => (HidKeyCode::Grave, false),
        b'~' => (HidKeyCode::Grave, true),
        b'\\' => (HidKeyCode::Backslash, false),
        b'|' => (HidKeyCode::Backslash, true),
        b',' => (HidKeyCode::Comma, false),
        b'<' => (HidKeyCode::Comma, true),
        b'.' => (HidKeyCode::Dot, false),
        b'>' => (HidKeyCode::Dot, true),
        b'/' => (HidKeyCode::Slash, false),
        b'?' => (HidKeyCode::Slash, true),
        b' ' => (HidKeyCode::Space, false),
        b'\n' => (HidKeyCode::Enter, false),
        b'\t' => (HidKeyCode::Tab, false),
        b'\x08' => (HidKeyCode::Backspace, false),
        b'\x1B' => (HidKeyCode::Escape, false),
        b'\x7F' => (HidKeyCode::Delete, false),
        _ => (HidKeyCode::No, false),
    }
}

/// Convert a ascii chat to keycode
/// assumes en-us keyboard mapping
pub fn to_ascii(keycode: HidKeyCode, shifted: bool) -> u8 {
    match (keycode, shifted) {
        (HidKeyCode::Kc0, false) => b'0',
        (HidKeyCode::Kc1, false) => b'1',
        (HidKeyCode::Kc2, false) => b'2',
        (HidKeyCode::Kc3, false) => b'3',
        (HidKeyCode::Kc4, false) => b'4',
        (HidKeyCode::Kc5, false) => b'5',
        (HidKeyCode::Kc6, false) => b'6',
        (HidKeyCode::Kc7, false) => b'7',
        (HidKeyCode::Kc8, false) => b'8',
        (HidKeyCode::Kc9, false) => b'9',
        (HidKeyCode::A, false) => b'a',
        (HidKeyCode::B, false) => b'b',
        (HidKeyCode::C, false) => b'c',
        (HidKeyCode::D, false) => b'd',
        (HidKeyCode::E, false) => b'e',
        (HidKeyCode::F, false) => b'f',
        (HidKeyCode::G, false) => b'g',
        (HidKeyCode::H, false) => b'h',
        (HidKeyCode::I, false) => b'i',
        (HidKeyCode::J, false) => b'j',
        (HidKeyCode::K, false) => b'k',
        (HidKeyCode::L, false) => b'l',
        (HidKeyCode::M, false) => b'm',
        (HidKeyCode::N, false) => b'n',
        (HidKeyCode::O, false) => b'o',
        (HidKeyCode::P, false) => b'p',
        (HidKeyCode::Q, false) => b'q',
        (HidKeyCode::R, false) => b'r',
        (HidKeyCode::S, false) => b's',
        (HidKeyCode::T, false) => b't',
        (HidKeyCode::U, false) => b'u',
        (HidKeyCode::V, false) => b'v',
        (HidKeyCode::W, false) => b'w',
        (HidKeyCode::X, false) => b'x',
        (HidKeyCode::Y, false) => b'y',
        (HidKeyCode::Z, false) => b'z',
        (HidKeyCode::A, true) => b'A',
        (HidKeyCode::B, true) => b'B',
        (HidKeyCode::C, true) => b'C',
        (HidKeyCode::D, true) => b'D',
        (HidKeyCode::E, true) => b'E',
        (HidKeyCode::F, true) => b'F',
        (HidKeyCode::G, true) => b'G',
        (HidKeyCode::H, true) => b'H',
        (HidKeyCode::I, true) => b'I',
        (HidKeyCode::J, true) => b'J',
        (HidKeyCode::K, true) => b'K',
        (HidKeyCode::L, true) => b'L',
        (HidKeyCode::M, true) => b'M',
        (HidKeyCode::N, true) => b'N',
        (HidKeyCode::O, true) => b'O',
        (HidKeyCode::P, true) => b'P',
        (HidKeyCode::Q, true) => b'Q',
        (HidKeyCode::R, true) => b'R',
        (HidKeyCode::S, true) => b'S',
        (HidKeyCode::T, true) => b'T',
        (HidKeyCode::U, true) => b'U',
        (HidKeyCode::V, true) => b'V',
        (HidKeyCode::W, true) => b'W',
        (HidKeyCode::X, true) => b'X',
        (HidKeyCode::Y, true) => b'Y',
        (HidKeyCode::Z, true) => b'Z',
        (HidKeyCode::Kc1, true) => b'!',
        (HidKeyCode::Kc2, true) => b'@',
        (HidKeyCode::Kc3, true) => b'#',
        (HidKeyCode::Kc4, true) => b'$',
        (HidKeyCode::Kc5, true) => b'%',
        (HidKeyCode::Kc6, true) => b'^',
        (HidKeyCode::Kc7, true) => b'&',
        (HidKeyCode::Kc8, true) => b'*',
        (HidKeyCode::Kc9, true) => b'(',
        (HidKeyCode::Kc0, true) => b')',
        (HidKeyCode::Minus, false) => b'-',
        (HidKeyCode::Minus, true) => b'_',
        (HidKeyCode::Equal, false) => b'=',
        (HidKeyCode::Equal, true) => b'+',
        (HidKeyCode::LeftBracket, false) => b'[',
        (HidKeyCode::RightBracket, false) => b']',
        (HidKeyCode::LeftBracket, true) => b'{',
        (HidKeyCode::RightBracket, true) => b'}',
        (HidKeyCode::Semicolon, false) => b';',
        (HidKeyCode::Semicolon, true) => b':',
        (HidKeyCode::Quote, false) => b'\'',
        (HidKeyCode::Quote, true) => b'"',
        (HidKeyCode::Grave, false) => b'`',
        (HidKeyCode::Grave, true) => b'~',
        (HidKeyCode::Backslash, true) => b'\\',
        (HidKeyCode::Backslash, false) => b'|',
        (HidKeyCode::Comma, false) => b',',
        (HidKeyCode::Comma, true) => b'<',
        (HidKeyCode::Dot, false) => b'.',
        (HidKeyCode::Dot, true) => b'>',
        (HidKeyCode::Slash, false) => b'/',
        (HidKeyCode::Slash, true) => b'?',
        (HidKeyCode::Space, false) => b' ',
        (HidKeyCode::Enter, false) => b'\n',
        (HidKeyCode::Tab, false) => b'\t',
        (HidKeyCode::Backspace, false) => b'\x08',
        (HidKeyCode::Escape, false) => b'\x1B',
        (HidKeyCode::Delete, false) => b'\x7F',
        // not supported keycodes
        (_, _) => b'X',
    }
}
