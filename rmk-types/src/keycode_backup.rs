//! Complete keycode definitions.
//!
//! This module provides keycode definitions following the USB HID
//! specification, extended with additional codes
use serde::{Deserialize, Serialize};
use strum::FromRepr;

use crate::modifier::ModifierCombination;

/// KeyCode is the internal representation of all keycodes, keyboard operations, etc.
/// Use flat representation of keycodes.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum KeyCode {
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
    GraveEscape = 0x716,
    OutputAuto = 0x720,
    OutputUsb = 0x721,
    OutputBluetooth = 0x722,
    ComboOn = 0x750,
    ComboOff = 0x751,
    ComboToggle = 0x752,
    CapsWordToggle = 0x773,
    TriLayerLower = 0x777,
    TriLayerUpper = 0x778,
    RepeatKey = 0x779,
    AltRepeatKey = 0x77A,
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

impl ::postcard::experimental::max_size::MaxSize for KeyCode {
    const POSTCARD_MAX_SIZE: usize = 3usize;
}

impl KeyCode {
    /// Returns `true` if the keycode is a simple keycode defined in HID spec
    pub fn is_simple_key(self) -> bool {
        KeyCode::No <= self && self <= KeyCode::MouseAccel2
    }

    /// Returns `true` if the keycode is a modifier keycode
    pub fn is_modifier(self) -> bool {
        KeyCode::LCtrl <= self && self <= KeyCode::RGui
    }

    /// Returns `true` if the keycode is basic keycode
    /// The basic keycode = simple key + modifier
    pub fn is_basic(self) -> bool {
        KeyCode::No <= self && self <= KeyCode::RGui
    }

    /// Returns `true` if the keycode is a letter
    pub fn is_letter(self) -> bool {
        KeyCode::A <= self && self <= KeyCode::Z
    }

    /// Returns the byte with the bit corresponding to the USB HID
    /// modifier bitfield set.
    pub fn to_hid_modifiers(self) -> ModifierCombination {
        match self {
            KeyCode::LCtrl => ModifierCombination::new().with_left_ctrl(true),
            KeyCode::LShift => ModifierCombination::new().with_left_shift(true),
            KeyCode::LAlt => ModifierCombination::new().with_left_alt(true),
            KeyCode::LGui => ModifierCombination::new().with_left_gui(true),
            KeyCode::RCtrl => ModifierCombination::new().with_right_ctrl(true),
            KeyCode::RShift => ModifierCombination::new().with_right_shift(true),
            KeyCode::RAlt => ModifierCombination::new().with_right_alt(true),
            KeyCode::RGui => ModifierCombination::new().with_right_gui(true),
            _ => ModifierCombination::new(),
        }
    }

    /// Returns `true` if the keycode is a system keycode
    pub fn is_system(self) -> bool {
        KeyCode::SystemPower <= self && self <= KeyCode::SystemWake
    }

    /// Returns `true` if the keycode is a keycode in consumer page
    pub fn is_consumer(self) -> bool {
        KeyCode::AudioMute <= self && self <= KeyCode::Launchpad
    }

    /// Returns `true` if the keycode is a mouse keycode
    pub fn is_mouse_key(self) -> bool {
        KeyCode::MouseUp <= self && self <= KeyCode::MouseAccel2
    }

    /// Returns `true` if the keycode is a combo keycode
    pub fn is_combo(self) -> bool {
        KeyCode::ComboOn <= self && self <= KeyCode::ComboToggle
    }

    /// Returns `true` if the keycode is a macro keycode
    pub fn is_macro(self) -> bool {
        KeyCode::Macro0 <= self && self <= KeyCode::Macro31
    }

    /// Returns `true` if the keycode is a backlight keycode
    pub fn is_backlight(self) -> bool {
        KeyCode::BacklightOn <= self && self <= KeyCode::BacklightToggleBreathing
    }

    /// Returns `true` if the keycode is a rgb keycode
    pub fn is_rgb(self) -> bool {
        KeyCode::RgbTog <= self && self <= KeyCode::RgbModeTwinkle
    }

    /// Returns `true` if the keycode is defined by rmk to achieve special functionalities, such as reboot keyboard, goto bootloader, etc.
    pub fn is_rmk(self) -> bool {
        KeyCode::Bootloader <= self && self <= KeyCode::AltRepeatKey
    }

    /// Returns `true` if the keycode is a boot keycode
    pub fn is_boot(self) -> bool {
        KeyCode::Bootloader <= self && self <= KeyCode::Reboot
    }
    /// Returns `true` if the keycode is a user keycode
    pub fn is_user(self) -> bool {
        KeyCode::User0 <= self && self <= KeyCode::User31
    }

    /// Convert a keycode to macro number
    pub fn as_macro_index(self) -> Option<u8> {
        if self.is_macro() {
            Some((self as u16 & 0x1F) as u8)
        } else {
            None
        }
    }

    /// Does current keycode continues Caps Word?
    pub fn is_caps_word_continue_key(self) -> bool {
        if self >= KeyCode::A && self <= KeyCode::Z {
            return true;
        }
        if self >= KeyCode::Kc1 && self <= KeyCode::Kc0 {
            return true;
        }
        if self == KeyCode::Minus || self == KeyCode::Backspace || self == KeyCode::Delete {
            return true;
        }
        false
    }

    /// Does current keycode is to be shifted by Caps Word?
    pub fn is_caps_word_shifted_key(self) -> bool {
        if self >= KeyCode::A && self <= KeyCode::Z {
            return true;
        }
        if self == KeyCode::Minus {
            return true;
        }
        false
    }

    /// Convert a keycode to usb hid media key
    pub fn as_consumer_control_usage_id(self) -> ConsumerKey {
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
            _ => ConsumerKey::No,
        }
    }

    /// Convert a keycode to usb hid media key
    pub fn as_system_control_usage_id(self) -> Option<SystemControlKey> {
        match self {
            KeyCode::SystemPower => Some(SystemControlKey::PowerDown),
            KeyCode::SystemSleep => Some(SystemControlKey::Sleep),
            KeyCode::SystemWake => Some(SystemControlKey::WakeUp),
            _ => None,
        }
    }
}

/// Convert a ascii chat to keycode
/// bool, if the keycode should be shifted
/// assumes en-us keyboard mapping
pub fn from_ascii(ascii: u8) -> (KeyCode, bool) {
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
        b'\\' => (KeyCode::Backslash, false),
        b'|' => (KeyCode::Backslash, true),
        b',' => (KeyCode::Comma, false),
        b'<' => (KeyCode::Comma, true),
        b'.' => (KeyCode::Dot, false),
        b'>' => (KeyCode::Dot, true),
        b'/' => (KeyCode::Slash, false),
        b'?' => (KeyCode::Slash, true),
        b' ' => (KeyCode::Space, false),
        b'\n' => (KeyCode::Enter, false),
        b'\t' => (KeyCode::Tab, false),
        b'\x08' => (KeyCode::Backspace, false),
        b'\x1B' => (KeyCode::Escape, false),
        b'\x7F' => (KeyCode::Delete, false),
        _ => (KeyCode::No, false),
    }
}

/// Convert a ascii chat to keycode
/// assumes en-us keyboard mapping
pub fn to_ascii(keycode: KeyCode, shifted: bool) -> u8 {
    match (keycode, shifted) {
        (KeyCode::Kc0, false) => b'0',
        (KeyCode::Kc1, false) => b'1',
        (KeyCode::Kc2, false) => b'2',
        (KeyCode::Kc3, false) => b'3',
        (KeyCode::Kc4, false) => b'4',
        (KeyCode::Kc5, false) => b'5',
        (KeyCode::Kc6, false) => b'6',
        (KeyCode::Kc7, false) => b'7',
        (KeyCode::Kc8, false) => b'8',
        (KeyCode::Kc9, false) => b'9',
        (KeyCode::A, false) => b'a',
        (KeyCode::B, false) => b'b',
        (KeyCode::C, false) => b'c',
        (KeyCode::D, false) => b'd',
        (KeyCode::E, false) => b'e',
        (KeyCode::F, false) => b'f',
        (KeyCode::G, false) => b'g',
        (KeyCode::H, false) => b'h',
        (KeyCode::I, false) => b'i',
        (KeyCode::J, false) => b'j',
        (KeyCode::K, false) => b'k',
        (KeyCode::L, false) => b'l',
        (KeyCode::M, false) => b'm',
        (KeyCode::N, false) => b'n',
        (KeyCode::O, false) => b'o',
        (KeyCode::P, false) => b'p',
        (KeyCode::Q, false) => b'q',
        (KeyCode::R, false) => b'r',
        (KeyCode::S, false) => b's',
        (KeyCode::T, false) => b't',
        (KeyCode::U, false) => b'u',
        (KeyCode::V, false) => b'v',
        (KeyCode::W, false) => b'w',
        (KeyCode::X, false) => b'x',
        (KeyCode::Y, false) => b'y',
        (KeyCode::Z, false) => b'z',
        (KeyCode::A, true) => b'A',
        (KeyCode::B, true) => b'B',
        (KeyCode::C, true) => b'C',
        (KeyCode::D, true) => b'D',
        (KeyCode::E, true) => b'E',
        (KeyCode::F, true) => b'F',
        (KeyCode::G, true) => b'G',
        (KeyCode::H, true) => b'H',
        (KeyCode::I, true) => b'I',
        (KeyCode::J, true) => b'J',
        (KeyCode::K, true) => b'K',
        (KeyCode::L, true) => b'L',
        (KeyCode::M, true) => b'M',
        (KeyCode::N, true) => b'N',
        (KeyCode::O, true) => b'O',
        (KeyCode::P, true) => b'P',
        (KeyCode::Q, true) => b'Q',
        (KeyCode::R, true) => b'R',
        (KeyCode::S, true) => b'S',
        (KeyCode::T, true) => b'T',
        (KeyCode::U, true) => b'U',
        (KeyCode::V, true) => b'V',
        (KeyCode::W, true) => b'W',
        (KeyCode::X, true) => b'X',
        (KeyCode::Y, true) => b'Y',
        (KeyCode::Z, true) => b'Z',
        (KeyCode::Kc1, true) => b'!',
        (KeyCode::Kc2, true) => b'@',
        (KeyCode::Kc3, true) => b'#',
        (KeyCode::Kc4, true) => b'$',
        (KeyCode::Kc5, true) => b'%',
        (KeyCode::Kc6, true) => b'^',
        (KeyCode::Kc7, true) => b'&',
        (KeyCode::Kc8, true) => b'*',
        (KeyCode::Kc9, true) => b'(',
        (KeyCode::Kc0, true) => b')',
        (KeyCode::Minus, false) => b'-',
        (KeyCode::Minus, true) => b'_',
        (KeyCode::Equal, false) => b'=',
        (KeyCode::Equal, true) => b'+',
        (KeyCode::LeftBracket, false) => b'[',
        (KeyCode::RightBracket, false) => b']',
        (KeyCode::LeftBracket, true) => b'{',
        (KeyCode::RightBracket, true) => b'}',
        (KeyCode::Semicolon, false) => b';',
        (KeyCode::Semicolon, true) => b':',
        (KeyCode::Quote, false) => b'\'',
        (KeyCode::Quote, true) => b'"',
        (KeyCode::Grave, false) => b'`',
        (KeyCode::Grave, true) => b'~',
        (KeyCode::Backslash, true) => b'\\',
        (KeyCode::Backslash, false) => b'|',
        (KeyCode::Comma, false) => b',',
        (KeyCode::Comma, true) => b'<',
        (KeyCode::Dot, false) => b'.',
        (KeyCode::Dot, true) => b'>',
        (KeyCode::Slash, false) => b'/',
        (KeyCode::Slash, true) => b'?',
        (KeyCode::Space, false) => b' ',
        (KeyCode::Enter, false) => b'\n',
        (KeyCode::Tab, false) => b'\t',
        (KeyCode::Backspace, false) => b'\x08',
        (KeyCode::Escape, false) => b'\x1B',
        (KeyCode::Delete, false) => b'\x7F',
        // not supported keycodes
        (_, _) => b'X',
    }
}

impl From<u16> for KeyCode {
    fn from(value: u16) -> Self {
        match value {
            0x0000 => Self::No,
            0x0001 => Self::ErrorRollover,
            0x0002 => Self::PostFail,
            0x0003 => Self::ErrorUndefined,
            0x0004 => Self::A,
            0x0005 => Self::B,
            0x0006 => Self::C,
            0x0007 => Self::D,
            0x0008 => Self::E,
            0x0009 => Self::F,
            0x000A => Self::G,
            0x000B => Self::H,
            0x000C => Self::I,
            0x000D => Self::J,
            0x000E => Self::K,
            0x000F => Self::L,
            0x0010 => Self::M,
            0x0011 => Self::N,
            0x0012 => Self::O,
            0x0013 => Self::P,
            0x0014 => Self::Q,
            0x0015 => Self::R,
            0x0016 => Self::S,
            0x0017 => Self::T,
            0x0018 => Self::U,
            0x0019 => Self::V,
            0x001A => Self::W,
            0x001B => Self::X,
            0x001C => Self::Y,
            0x001D => Self::Z,
            0x001E => Self::Kc1,
            0x001F => Self::Kc2,
            0x0020 => Self::Kc3,
            0x0021 => Self::Kc4,
            0x0022 => Self::Kc5,
            0x0023 => Self::Kc6,
            0x0024 => Self::Kc7,
            0x0025 => Self::Kc8,
            0x0026 => Self::Kc9,
            0x0027 => Self::Kc0,
            0x0028 => Self::Enter,
            0x0029 => Self::Escape,
            0x002A => Self::Backspace,
            0x002B => Self::Tab,
            0x002C => Self::Space,
            0x002D => Self::Minus,
            0x002E => Self::Equal,
            0x002F => Self::LeftBracket,
            0x0030 => Self::RightBracket,
            0x0031 => Self::Backslash,
            0x0032 => Self::NonusHash,
            0x0033 => Self::Semicolon,
            0x0034 => Self::Quote,
            0x0035 => Self::Grave,
            0x0036 => Self::Comma,
            0x0037 => Self::Dot,
            0x0038 => Self::Slash,
            0x0039 => Self::CapsLock,
            0x003A => Self::F1,
            0x003B => Self::F2,
            0x003C => Self::F3,
            0x003D => Self::F4,
            0x003E => Self::F5,
            0x003F => Self::F6,
            0x0040 => Self::F7,
            0x0041 => Self::F8,
            0x0042 => Self::F9,
            0x0043 => Self::F10,
            0x0044 => Self::F11,
            0x0045 => Self::F12,
            0x0046 => Self::PrintScreen,
            0x0047 => Self::ScrollLock,
            0x0048 => Self::Pause,
            0x0049 => Self::Insert,
            0x004A => Self::Home,
            0x004B => Self::PageUp,
            0x004C => Self::Delete,
            0x004D => Self::End,
            0x004E => Self::PageDown,
            0x004F => Self::Right,
            0x0050 => Self::Left,
            0x0051 => Self::Down,
            0x0052 => Self::Up,
            0x0053 => Self::NumLock,
            0x0054 => Self::KpSlash,
            0x0055 => Self::KpAsterisk,
            0x0056 => Self::KpMinus,
            0x0057 => Self::KpPlus,
            0x0058 => Self::KpEnter,
            0x0059 => Self::Kp1,
            0x005A => Self::Kp2,
            0x005B => Self::Kp3,
            0x005C => Self::Kp4,
            0x005D => Self::Kp5,
            0x005E => Self::Kp6,
            0x005F => Self::Kp7,
            0x0060 => Self::Kp8,
            0x0061 => Self::Kp9,
            0x0062 => Self::Kp0,
            0x0063 => Self::KpDot,
            0x0064 => Self::NonusBackslash,
            0x0065 => Self::Application,
            0x0066 => Self::KbPower,
            0x0067 => Self::KpEqual,
            0x0068 => Self::F13,
            0x0069 => Self::F14,
            0x006A => Self::F15,
            0x006B => Self::F16,
            0x006C => Self::F17,
            0x006D => Self::F18,
            0x006E => Self::F19,
            0x006F => Self::F20,
            0x0070 => Self::F21,
            0x0071 => Self::F22,
            0x0072 => Self::F23,
            0x0073 => Self::F24,
            0x0074 => Self::Execute,
            0x0075 => Self::Help,
            0x0076 => Self::Menu,
            0x0077 => Self::Select,
            0x0078 => Self::Stop,
            0x0079 => Self::Again,
            0x007A => Self::Undo,
            0x007B => Self::Cut,
            0x007C => Self::Copy,
            0x007D => Self::Paste,
            0x007E => Self::Find,
            0x007F => Self::KbMute,
            0x0080 => Self::KbVolumeUp,
            0x0081 => Self::KbVolumeDown,
            0x0082 => Self::LockingCapsLock,
            0x0083 => Self::LockingNumLock,
            0x0084 => Self::LockingScrollLock,
            0x0085 => Self::KpComma,
            0x0086 => Self::KpEqualAs400,
            0x0087 => Self::International1,
            0x0088 => Self::International2,
            0x0089 => Self::International3,
            0x008A => Self::International4,
            0x008B => Self::International5,
            0x008C => Self::International6,
            0x008D => Self::International7,
            0x008E => Self::International8,
            0x008F => Self::International9,
            0x0090 => Self::Language1,
            0x0091 => Self::Language2,
            0x0092 => Self::Language3,
            0x0093 => Self::Language4,
            0x0094 => Self::Language5,
            0x0095 => Self::Language6,
            0x0096 => Self::Language7,
            0x0097 => Self::Language8,
            0x0098 => Self::Language9,
            0x0099 => Self::AlternateErase,
            0x009A => Self::SystemRequest,
            0x009B => Self::Cancel,
            0x009C => Self::Clear,
            0x009D => Self::Prior,
            0x009E => Self::Return,
            0x009F => Self::Separator,
            0x00A0 => Self::Out,
            0x00A1 => Self::Oper,
            0x00A2 => Self::ClearAgain,
            0x00A3 => Self::Crsel,
            0x00A4 => Self::Exsel,
            0x00A5 => Self::SystemPower,
            0x00A6 => Self::SystemSleep,
            0x00A7 => Self::SystemWake,
            0x00A8 => Self::AudioMute,
            0x00A9 => Self::AudioVolUp,
            0x00AA => Self::AudioVolDown,
            0x00AB => Self::MediaNextTrack,
            0x00AC => Self::MediaPrevTrack,
            0x00AD => Self::MediaStop,
            0x00AE => Self::MediaPlayPause,
            0x00AF => Self::MediaSelect,
            0x00B0 => Self::MediaEject,
            0x00B1 => Self::Mail,
            0x00B2 => Self::Calculator,
            0x00B3 => Self::MyComputer,
            0x00B4 => Self::WwwSearch,
            0x00B5 => Self::WwwHome,
            0x00B6 => Self::WwwBack,
            0x00B7 => Self::WwwForward,
            0x00B8 => Self::WwwStop,
            0x00B9 => Self::WwwRefresh,
            0x00BA => Self::WwwFavorites,
            0x00BB => Self::MediaFastForward,
            0x00BC => Self::MediaRewind,
            0x00BD => Self::BrightnessUp,
            0x00BE => Self::BrightnessDown,
            0x00BF => Self::ControlPanel,
            0x00C0 => Self::Assistant,
            0x00C1 => Self::MissionControl,
            0x00C2 => Self::Launchpad,
            0x00CD => Self::MouseUp,
            0x00CE => Self::MouseDown,
            0x00CF => Self::MouseLeft,
            0x00D0 => Self::MouseRight,
            0x00D1 => Self::MouseBtn1,
            0x00D2 => Self::MouseBtn2,
            0x00D3 => Self::MouseBtn3,
            0x00D4 => Self::MouseBtn4,
            0x00D5 => Self::MouseBtn5,
            0x00D6 => Self::MouseBtn6,
            0x00D7 => Self::MouseBtn7,
            0x00D8 => Self::MouseBtn8,
            0x00D9 => Self::MouseWheelUp,
            0x00DA => Self::MouseWheelDown,
            0x00DB => Self::MouseWheelLeft,
            0x00DC => Self::MouseWheelRight,
            0x00DD => Self::MouseAccel0,
            0x00DE => Self::MouseAccel1,
            0x00DF => Self::MouseAccel2,
            0x00E0 => Self::LCtrl,
            0x00E1 => Self::LShift,
            0x00E2 => Self::LAlt,
            0x00E3 => Self::LGui,
            0x00E4 => Self::RCtrl,
            0x00E5 => Self::RShift,
            0x00E6 => Self::RAlt,
            0x00E7 => Self::RGui,
            0x500 => Self::Macro0,
            0x501 => Self::Macro1,
            0x502 => Self::Macro2,
            0x503 => Self::Macro3,
            0x504 => Self::Macro4,
            0x505 => Self::Macro5,
            0x506 => Self::Macro6,
            0x507 => Self::Macro7,
            0x508 => Self::Macro8,
            0x509 => Self::Macro9,
            0x50A => Self::Macro10,
            0x50B => Self::Macro11,
            0x50C => Self::Macro12,
            0x50D => Self::Macro13,
            0x50E => Self::Macro14,
            0x50F => Self::Macro15,
            0x510 => Self::Macro16,
            0x511 => Self::Macro17,
            0x512 => Self::Macro18,
            0x513 => Self::Macro19,
            0x514 => Self::Macro20,
            0x515 => Self::Macro21,
            0x516 => Self::Macro22,
            0x517 => Self::Macro23,
            0x518 => Self::Macro24,
            0x519 => Self::Macro25,
            0x51A => Self::Macro26,
            0x51B => Self::Macro27,
            0x51C => Self::Macro28,
            0x51D => Self::Macro29,
            0x51E => Self::Macro30,
            0x51F => Self::Macro31,
            0x600 => Self::BacklightOn,
            0x601 => Self::BacklightOff,
            0x602 => Self::BacklightToggle,
            0x603 => Self::BacklightDown,
            0x604 => Self::BacklightUp,
            0x605 => Self::BacklightStep,
            0x606 => Self::BacklightToggleBreathing,
            0x620 => Self::RgbTog,
            0x621 => Self::RgbModeForward,
            0x622 => Self::RgbModeReverse,
            0x623 => Self::RgbHui,
            0x624 => Self::RgbHud,
            0x625 => Self::RgbSai,
            0x626 => Self::RgbSad,
            0x627 => Self::RgbVai,
            0x628 => Self::RgbVad,
            0x629 => Self::RgbSpi,
            0x62A => Self::RgbSpd,
            0x62B => Self::RgbModePlain,
            0x62C => Self::RgbModeBreathe,
            0x62D => Self::RgbModeRainbow,
            0x62E => Self::RgbModeSwirl,
            0x62F => Self::RgbModeSnake,
            0x630 => Self::RgbModeKnight,
            0x631 => Self::RgbModeXmas,
            0x632 => Self::RgbModeGradient,
            0x633 => Self::RgbModeRgbtest,
            0x634 => Self::RgbModeTwinkle,
            0x700 => Self::Bootloader,
            0x701 => Self::Reboot,
            0x702 => Self::DebugToggle,
            0x703 => Self::ClearEeprom,
            0x716 => Self::GraveEscape,
            0x720 => Self::OutputAuto,
            0x721 => Self::OutputUsb,
            0x722 => Self::OutputBluetooth,
            0x750 => Self::ComboOn,
            0x751 => Self::ComboOff,
            0x752 => Self::ComboToggle,
            0x773 => Self::CapsWordToggle,
            0x777 => Self::TriLayerLower,
            0x778 => Self::TriLayerUpper,
            0x779 => Self::RepeatKey,
            0x77A => Self::AltRepeatKey,
            0x840 => Self::User0,
            0x841 => Self::User1,
            0x842 => Self::User2,
            0x843 => Self::User3,
            0x844 => Self::User4,
            0x845 => Self::User5,
            0x846 => Self::User6,
            0x847 => Self::User7,
            0x848 => Self::User8,
            0x849 => Self::User9,
            0x84A => Self::User10,
            0x84B => Self::User11,
            0x84C => Self::User12,
            0x84D => Self::User13,
            0x84E => Self::User14,
            0x84F => Self::User15,
            0x850 => Self::User16,
            0x851 => Self::User17,
            0x852 => Self::User18,
            0x853 => Self::User19,
            0x854 => Self::User20,
            0x855 => Self::User21,
            0x856 => Self::User22,
            0x857 => Self::User23,
            0x858 => Self::User24,
            0x859 => Self::User25,
            0x85A => Self::User26,
            0x85B => Self::User27,
            0x85C => Self::User28,
            0x85D => Self::User29,
            0x85E => Self::User30,
            0x85F => Self::User31,
            _ => Self::No,
        }
    }
}

/// Keys in consumer page
/// Ref: <https://www.usb.org/sites/default/files/documents/hut1_12v2.pdf#page=75>
#[non_exhaustive]
#[repr(u16)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord, FromRepr)]
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

impl From<u16> for ConsumerKey {
    fn from(value: u16) -> Self {
        match value {
            0x00 => Self::No,
            0x65 => Self::SnapShot,
            0x6F => Self::BrightnessUp,
            0x70 => Self::BrightnessDown,
            0xB0 => Self::Play,
            0xB1 => Self::Pause,
            0xB2 => Self::Record,
            0xB3 => Self::FastForward,
            0xB4 => Self::Rewind,
            0xB5 => Self::NextTrack,
            0xB6 => Self::PrevTrack,
            0xB7 => Self::StopPlay,
            0xB8 => Self::Eject,
            0xB9 => Self::RandomPlay,
            0xBC => Self::Repeat,
            0xCC => Self::StopEject,
            0xCD => Self::PlayPause,
            0xE2 => Self::Mute,
            0xE9 => Self::VolumeIncrement,
            0xEA => Self::VolumeDecrement,
            0xEB => Self::Reserved,
            0x18A => Self::Email,
            0x192 => Self::Calculator,
            0x194 => Self::LocalBrowser,
            0x19E => Self::Lock,
            0x19F => Self::ControlPanel,
            0x1CB => Self::Assistant,
            0x201 => Self::New,
            0x202 => Self::Open,
            0x203 => Self::Close,
            0x204 => Self::Exit,
            0x205 => Self::Maximize,
            0x206 => Self::Minimize,
            0x207 => Self::Save,
            0x208 => Self::Print,
            0x209 => Self::Properties,
            0x21A => Self::Undo,
            0x21B => Self::Copy,
            0x21C => Self::Cut,
            0x21D => Self::Paste,
            0x21E => Self::SelectAll,
            0x21F => Self::Find,
            0x221 => Self::Search,
            0x223 => Self::Home,
            0x224 => Self::Back,
            0x225 => Self::Forward,
            0x226 => Self::Stop,
            0x227 => Self::Refresh,
            0x22A => Self::Bookmarks,
            0x29D => Self::NextKeyboardLayoutSelect,
            0x29F => Self::DesktopShowAllWindows,
            0x2A0 => Self::AcSoftKeyLeft,
            _ => Self::No,
        }
    }
}

/// Keys in `Generic Desktop Page`, generally used for system control
/// Ref: <https://www.usb.org/sites/default/files/documents/hut1_12v2.pdf#page=26>
#[non_exhaustive]
#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord, FromRepr)]
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

impl From<u16> for SystemControlKey {
    fn from(value: u16) -> Self {
        match value {
            0x00 => Self::No,
            0x81 => Self::PowerDown,
            0x82 => Self::Sleep,
            0x83 => Self::WakeUp,
            0x8F => Self::Restart,
            _ => Self::No,
        }
    }
}
