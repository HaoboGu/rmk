//! Consumer page keycodes.

use postcard::experimental::max_size::MaxSize;
#[cfg(feature = "rmk_protocol")]
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use super::hid::HidKeyCode;

/// Keys in consumer page
/// Ref: <https://www.usb.org/sites/default/files/documents/hut1_12v2.pdf#page=75>
#[non_exhaustive]
#[repr(u16)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord, MaxSize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[cfg_attr(feature = "rmk_protocol", derive(Schema))]
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
