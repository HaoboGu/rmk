use rmk::types::action::{Action, KeyAction, KeyboardAction};
use rmk::types::modifier::ModifierCombination;
use rmk::types::morse::{MorseMode, MorseProfile};
use rmk::{a, k, layer, lt, ltp, mtp, special, user, wm};

pub(crate) const ROW: usize = 4;
pub(crate) const COL: usize = 12;
pub(crate) const NUM_LAYER: usize = 8;

const TAP_PREFERRED: MorseProfile = MorseProfile::new(None, Some(MorseMode::Normal), Some(200), Some(180));
const HOLD_PREFERRED: MorseProfile = MorseProfile::new(None, Some(MorseMode::HoldOnOtherPress), Some(260), Some(240));
const LONG_HOLD: MorseProfile = MorseProfile::new(None, Some(MorseMode::Normal), Some(3000), Some(0));

#[rustfmt::skip]
pub const fn get_default_keymap() -> [[[KeyAction; COL]; ROW]; NUM_LAYER] {
    [
        layer!([
            [special!(GraveEscape), k!(Q), k!(W), k!(E), k!(R), k!(T), k!(Y), k!(U), k!(I), k!(O), mtp!(Backslash, ModifierCombination::LGUI, TAP_PREFERRED), k!(Backspace)],
            [ltp!(2, Tab, HOLD_PREFERRED), k!(A), k!(S), k!(D), k!(F), k!(G), k!(H), k!(J), k!(K), k!(L), k!(Semicolon), k!(Enter)],
            [k!(LShift), k!(Z), k!(X), k!(C), k!(V), k!(B), k!(N), k!(M), k!(Comma), k!(Dot), mtp!(Up, ModifierCombination::RSHIFT, HOLD_PREFERRED), ltp!(3, Right, HOLD_PREFERRED)],
            [a!(No), k!(LCtrl), k!(LAlt), a!(No), lt!(1, Space), a!(No), k!(Space), k!(P), a!(No), k!(Left), k!(Down), a!(No)]
        ]),
        layer!([
            [k!(Grave), k!(Kc1), k!(Kc2), k!(Kc3), k!(Kc4), k!(Kc5), k!(Kc6), k!(Kc7), k!(Kc8), k!(Kc9), k!(Minus), k!(Equal)],
            [a!(Transparent), a!(Transparent), a!(Transparent), k!(MouseBtn1), k!(MouseUp), k!(MouseBtn2), k!(LeftBracket), k!(RightBracket), a!(Transparent), a!(Transparent), k!(Quote), a!(Transparent)],
            [a!(Transparent), a!(Transparent), a!(Transparent), k!(MouseLeft), k!(MouseDown), k!(MouseRight), a!(Transparent), a!(Transparent), a!(Transparent), k!(Slash), a!(Transparent), a!(Transparent)],
            [a!(No), a!(Transparent), a!(Transparent), a!(No), a!(Transparent), a!(No), a!(Transparent), k!(Kc0), a!(No), a!(Transparent), a!(Transparent), a!(No)]
        ]),
        layer!([
            [k!(CapsLock), k!(PrintScreen), k!(PageUp), k!(Home), k!(F2), a!(Transparent), a!(Transparent), a!(Transparent), k!(Insert), a!(Transparent), a!(Transparent), a!(Transparent)],
            [a!(Transparent), k!(Delete), k!(PageDown), k!(End), k!(F2), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent)],
            [a!(Transparent), k!(Pause), k!(ScrollLock), k!(F4), wm!(V, ModifierCombination::LGUI), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), k!(RGui), a!(Transparent), a!(Transparent)],
            [a!(No), a!(Transparent), a!(Transparent), a!(No), a!(Transparent), a!(No), a!(Transparent), a!(Transparent), a!(No), k!(RAlt), k!(RCtrl), a!(No)]
        ]),
        layer!([
            [a!(Transparent), k!(F1), k!(F2), k!(F3), k!(F4), k!(F5), k!(F6), k!(F7), k!(F8), k!(F9), k!(F11), k!(F12)],
            [a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), user!(6), user!(0), user!(1), user!(2), a!(Transparent), k!(MediaPlayPause)],
            [a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), KeyAction::TapHold(Action::KeyboardControl(KeyboardAction::Reboot), Action::KeyboardControl(KeyboardAction::Bootloader), LONG_HOLD), a!(Transparent), a!(Transparent), k!(MediaPrevTrack), k!(MediaNextTrack), k!(AudioVolUp), a!(Transparent)],
            [a!(No), a!(Transparent), a!(Transparent), a!(No), a!(Transparent), a!(No), a!(Transparent), k!(F10), a!(No), k!(AudioMute), k!(AudioVolDown), a!(No)]
        ]),
        layer!([
            [a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent)],
            [a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent)],
            [a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent)],
            [a!(No), a!(Transparent), a!(Transparent), a!(No), a!(Transparent), a!(No), a!(Transparent), a!(Transparent), a!(No), a!(Transparent), a!(Transparent), a!(No)]
        ]),
        layer!([
            [a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent)],
            [a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent)],
            [a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent)],
            [a!(No), a!(Transparent), a!(Transparent), a!(No), a!(Transparent), a!(No), a!(Transparent), a!(Transparent), a!(No), a!(Transparent), a!(Transparent), a!(No)]
        ]),
        layer!([
            [a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent)],
            [a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent)],
            [a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent)],
            [a!(No), a!(Transparent), a!(Transparent), a!(No), a!(Transparent), a!(No), a!(Transparent), a!(Transparent), a!(No), a!(Transparent), a!(Transparent), a!(No)]
        ]),
        layer!([
            [a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent)],
            [a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent)],
            [a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent), a!(Transparent)],
            [a!(No), a!(Transparent), a!(Transparent), a!(No), a!(Transparent), a!(No), a!(Transparent), a!(Transparent), a!(No), a!(Transparent), a!(Transparent), a!(No)]
        ]),
    ]
}
