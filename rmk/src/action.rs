use crate::keycode::{KeyCode, Modifier};

/// A KeyAction is the action of a keyboard position, stored in keymap.
/// It can be a single action like triggering a key, or a composite keyboard action like TapHold
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum KeyAction {
    /// No action
    No,
    /// Transparent action, next layer will be checked
    Transparent,
    /// A single action, such as triggering a key, or activating a layer
    /// Action is triggered when pressed and cancelled when released
    Single(Action),
    /// Action with a modifier triggered
    WithModifier(Action, Modifier),
    /// Don't wait the release of the key, auto-release after a time threshold
    Tap(Action),
    /// Tap/hold will trigger different actions: TapHold(tap_action, hold_action)
    TapHold(Action, Action),
    /// Keep current key pressed until the next key is triggered
    OneShot(Action),
}

/// A single basic action that a keyboard can execute.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Action {
    /// A normal key
    Key(KeyCode),
    /// Action for mouse key.
    MouseKey(KeyCode),
    /// Action for System Control(0x80), which is in General Desktop Page(0x01) defined by HID spec.
    SystemControl(KeyCode),
    /// Action for Consumer Control(0x01), which is in Consumer Page(0x0C) defined by HID spec.
    ConsumerControl(KeyCode),
    /// One-hand support
    SwapHands(KeyCode),
    /// Activate a layer
    LayerOn(u8),
    /// Deactivate a layer
    LayerOff(u8),
    /// Toggle a layer
    LayerToggle(u8),
}
