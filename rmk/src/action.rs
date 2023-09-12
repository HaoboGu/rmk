use crate::keycode::KeyCode;

pub enum TriggerType {
    NoOp,
    KeyCode,
    Tap,
    Hold,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Modifier {
    /// Left control.
    LCtrl,
    /// Left shift.
    LShift,
    /// Left alt(option).
    LAlt,
    /// Left gui(widnows/command/meta key).
    LGui,
    /// Right control.
    RCtrl,
    /// Right shift.
    RShift,
    /// Right alt(option/AltGr).
    RAlt,
    /// Right gui(windows/command/meta key).
    RGui,
}

impl Modifier {
    pub fn to_keycode(self) -> KeyCode {
        match self {
            Modifier::LCtrl => KeyCode::LCtrl,
            Modifier::LShift => KeyCode::LShift,
            Modifier::LAlt => KeyCode::LAlt,
            Modifier::LGui => KeyCode::LGui,
            Modifier::RCtrl => KeyCode::RCtrl,
            Modifier::RShift => KeyCode::RShift,
            Modifier::RAlt => KeyCode::RAlt,
            Modifier::RGui => KeyCode::RGui,
        }
    }
}

/// Action represents all actions that can be executed by keyboard.
/// Actions are stored in keymaps, some actions have different funtionalities which is triggered by different ways.
/// In QMK, action is defined by a uint16_t, with a lot of bitwise operation.
/// Action + TriggerType => What's actually executed by the keyboard.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Action {
    // Action for keys
    No,
    Transparent,
    /// Send a keycode to the host.
    Key(KeyCode),
    /// Send a keycode with modifier to the host.
    KeyWithModifier(KeyCode, Modifier),

    /// Modifier tap, use Action::Key for normal modifier.
    Modifier(Modifier),
    /// OneShot key for modifier.
    OneShotModifier(Modifier),
    /// Hold this key to activate modifier temporarily, tap to toggle the modifier.
    ModifiertOrTapToggle(Modifier),
    /// Hold this key to activate modifier temporarily, tap to send a keycode.
    ModifierOrTapKey(Modifier, KeyCode),

    /// Activate the layer.
    LayerActivate(u8),
    /// Deactivate the layer.
    LayerDeactivate(u8),
    /// Toggle the layer.
    LayerToggle(u8),
    /// OneShot key for layer.
    OneShotLayer(u8),
    /// Activate the layer with a modifier.
    LayerMods(u8, Modifier),
    /// Hold this key to activate the layer temporarily, tap to send a keycode.
    LayerOrTapKey(u8, KeyCode),
    /// Hold this key to activate layer temporarily, tap to toggle the layer.
    LayerOrTapToggle(u8),

    // Action for other usages
    /// Action for mouse key.
    MouseKey(KeyCode),
    /// Action for System Control(0x80), which is in General Desktop Page(0x01) defined by HID spec.
    SystemControl(KeyCode),
    /// Action for Consumer Control(0x01), which is in Consumer Page(0x0C) defined by HID spec.
    ConsumerControl(KeyCode),
    /// One-hand support
    SwapHands(KeyCode),
}

pub trait HoldOrTap {}
