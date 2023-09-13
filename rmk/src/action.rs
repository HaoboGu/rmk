use crate::keycode::{Modifier, KeyCode};

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
    LayerActivate(u8),
    /// Deactivate a layer
    LayerDeactivate(u8),
    /// Toggle a layer
    LayerToggle(u8),
}

// // TODO: Classify action types to the following types: normal(press/release), tap, hold, oneshot key, tap & key, tap & toggle, tap & hold.
// // Defines standard process of each type of action.

// /// Action represents all actions that can be executed by keyboard.
// /// Actions are stored in keymaps, some actions have different funtionalities which is triggered by different ways.
// /// In QMK, action is defined by a uint16_t, with a lot of bitwise operation.
// /// Action + TriggerType => What's actually executed by the keyboard.
// #[derive(Debug, Copy, Clone, PartialEq, Eq)]
// pub enum Action {
//     // Action for keys
//     No,
//     Transparent,
//     /// Send a keycode to the host.
//     Key(KeyCode),
//     /// Send a keycode with modifier to the host.
//     KeyWithModifier(KeyCode, Modifier),

//     /// Modifier tap, use Action::Key for normal modifier.
//     Modifier(Modifier),
//     /// OneShot key for modifier.
//     OneShotModifier(Modifier),
//     /// Hold this key to activate modifier temporarily, tap to toggle the modifier.
//     ModifiertOrTapToggle(Modifier),
//     /// Hold this key to activate modifier temporarily, tap to send a keycode.
//     ModifierOrTapKey(Modifier, KeyCode),

//     /// Activate the layer.
//     LayerActivate(u8),
//     /// Deactivate the layer.
//     LayerDeactivate(u8),
//     /// Toggle the layer.
//     LayerToggle(u8),
//     /// OneShot key for layer.
//     OneShotLayer(u8),
//     /// Activate the layer with a modifier.
//     LayerMods(u8, Modifier),
//     /// Hold this key to activate the layer temporarily, tap to send a keycode.
//     LayerOrTapKey(u8, KeyCode),
//     /// Hold this key to activate layer temporarily, tap to toggle the layer.
//     LayerOrTapToggle(u8),

//     // Action for other usages
//     /// Action for mouse key.
//     MouseKey(KeyCode),
//     /// Action for System Control(0x80), which is in General Desktop Page(0x01) defined by HID spec.
//     SystemControl(KeyCode),
//     /// Action for Consumer Control(0x01), which is in Consumer Page(0x0C) defined by HID spec.
//     ConsumerControl(KeyCode),
//     /// One-hand support
//     SwapHands(KeyCode),
// }
