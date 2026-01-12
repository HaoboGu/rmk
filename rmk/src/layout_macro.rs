/// Create a layer in keymap.
///
/// This macro simplifies the syntax for defining keyboard layers by allowing
/// a more natural 2D array notation.
///
/// # Example
/// ```ignore
/// let layer = layer!([
///     [k!(Esc), k!(Kc1), k!(Kc2)],
///     [k!(Tab), k!(Q), k!(W)]
/// ]);
/// ```
#[macro_export]
macro_rules! layer {
    ([$([$($x: expr), +]), +]) => {
        [$([$($x), +]),+]
    };
}

/// Create a normal key action.
///
/// This macro creates a simple key press action for any HID keyboard key.
/// When the key is pressed, it sends the corresponding HID keycode.
///
/// # Parameters
/// - `$k`: The HID keycode identifier (e.g., `A`, `Space`, `Enter`, `F1`)
///
/// # Example
/// ```ignore
/// k!(A)        // Creates action for key 'A'
/// k!(Space)    // Creates action for Space key
/// k!(Enter)    // Creates action for Enter key
/// k!(F1)       // Creates action for F1 key
/// ```
///
/// # Expands to
/// `KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::A)))`
#[macro_export]
macro_rules! k {
    ($k: ident) => {
        $crate::types::action::KeyAction::Single($crate::types::action::Action::Key(
            $crate::types::keycode::KeyCode::Hid($crate::types::keycode::HidKeyCode::$k),
        ))
    };
}

/// Create a key action with modifier combination.
///
/// This macro creates a key action that sends a key along with modifier keys
/// (Ctrl, Shift, Alt, GUI) pressed simultaneously.
///
/// # Parameters
/// - `$x`: The HID keycode identifier
/// - `$m`: A `ModifierCombination` expression specifying which modifiers to apply
///
/// # Example
/// ```ignore
/// // Ctrl+C
/// wm!(C, ModifierCombination::new_from(false, false, false, false, true))
///
/// // Shift+A (can also use the `shifted!` macro for this)
/// wm!(A, ModifierCombination::LSHIFT)
///
/// // Ctrl+Shift+Esc
/// wm!(Escape, ModifierCombination::LCTRL.with_left_shift(true))
/// ```
#[macro_export]
macro_rules! wm {
    ($x: ident, $m: expr) => {
        $crate::types::action::KeyAction::Single($crate::types::action::Action::KeyWithModifier(
            $crate::types::keycode::KeyCode::Hid($crate::types::keycode::HidKeyCode::$x),
            $m,
        ))
    };
}

/// Create a KeyAction variant directly.
///
/// This macro provides shorthand access to KeyAction enum variants.
///
/// # Parameters
/// - `$a`: The KeyAction variant name (e.g., `No`, `Transparent`)
///
/// # Example
/// ```ignore
/// a!(No)           // KeyAction::No - empty action
/// a!(Transparent)  // KeyAction::Transparent - pass through to next layer
/// ```
#[macro_export]
macro_rules! a {
    ($a: ident) => {
        $crate::types::action::KeyAction::$a
    };
}

/// Create a momentary layer activation action.
///
/// This macro creates an action that activates a layer while the key is held down.
/// The layer is deactivated when the key is released. Similar to QMK's `MO()`.
///
/// # Parameters
/// - `$x`: Layer number (0-255)
///
/// # Example
/// ```ignore
/// mo!(1)  // Activates layer 1 while held
/// mo!(2)  // Activates layer 2 while held
/// ```
#[macro_export]
macro_rules! mo {
    ($x: literal) => {
        $crate::types::action::KeyAction::Single($crate::types::action::Action::LayerOn($x))
    };
}

/// Create a layer activation with modifier action.
///
/// This macro activates a layer while also applying modifier keys.
/// Both the layer and modifiers are active while the key is held.
///
/// # Parameters
/// - `$x`: Layer number (0-15)
/// - `$m`: A `ModifierCombination` expression
///
/// # Example
/// ```ignore
/// lm!(1, ModifierCombination::LSHIFT)  // Activates layer 1 with Left Shift
/// lm!(2, ModifierCombination::LCTRL)   // Activates layer 2 with Left Ctrl
/// ```
#[macro_export]
macro_rules! lm {
    ($x: literal, $m: expr) => {
        $crate::types::action::KeyAction::Single($crate::types::action::Action::LayerOnWithModifier($x, $m))
    };
}

/// Create a layer-tap action (tap/hold behavior).
///
/// This macro creates a dual-function key:
/// - **Tap**: Sends the specified key or triggers an action
/// - **Hold**: Activates the specified layer
///
/// Uses default timing configuration for tap/hold detection.
///
/// # Parameters
/// - `$x`: Layer number to activate when held
/// - Second parameter (tap action) can be:
///   - HID keycode identifier (e.g., `Space`, `Enter`)
///   - Action variant with arguments (e.g., `TriggerMacro(0)`, `LayerToggle(1)`)
///   - Action variant without arguments (e.g., `Action::TriLayerLower`)
///   - Fully qualified Action (e.g., `Action::TriggerMacro(0)`)
///
/// # Examples
/// ```ignore
/// // Traditional usage - HID keycode
/// lt!(1, Space)  // Tap for Space, hold for layer 1
/// lt!(2, Enter)  // Tap for Enter, hold for layer 2
///
/// // New usage - Action variants
/// lt!(1, TriggerMacro(0))  // Tap triggers macro 0, hold for layer 1
/// lt!(2, Action::TriLayerLower)  // Tap tri-layer lower, hold for layer 2
///
/// // Fully qualified Action
/// lt!(1, Action::TriggerMacro(0))  // Also works
/// ```
#[macro_export]
macro_rules! lt {
    // Rule 1: Fully qualified Action with arguments
    ($x: literal, Action::$variant:ident($($args:expr),+)) => {
        $crate::types::action::KeyAction::TapHold(
            $crate::types::action::Action::$variant($($args),*),
            $crate::types::action::Action::LayerOn($x),
            $crate::types::action::MorseProfile::const_default(),
        )
    };

    // Rule 2: Fully qualified Action without arguments
    ($x: literal, Action::$variant:ident) => {
        $crate::types::action::KeyAction::TapHold(
            $crate::types::action::Action::$variant,
            $crate::types::action::Action::LayerOn($x),
            $crate::types::action::MorseProfile::const_default(),
        )
    };

    // Rule 3: Bare Action variant with arguments
    ($x: literal, $variant:ident($($args:expr),+)) => {
        $crate::types::action::KeyAction::TapHold(
            $crate::types::action::Action::$variant($($args),*),
            $crate::types::action::Action::LayerOn($x),
            $crate::types::action::MorseProfile::const_default(),
        )
    };

    // Rule 4: HID keycode (backward compatible)
    ($x: literal, $k: ident) => {
        $crate::types::action::KeyAction::TapHold(
            $crate::types::action::Action::Key($crate::types::keycode::KeyCode::Hid(
                $crate::types::keycode::HidKeyCode::$k,
            )),
            $crate::types::action::Action::LayerOn($x),
            $crate::types::action::MorseProfile::const_default(),
        )
    };
}

/// Create a layer-tap action with custom timing profile.
///
/// Same as `lt!` but allows specifying custom tap/hold timing configuration
/// through a `MorseProfile`.
///
/// # Parameters
/// - `$x`: Layer number to activate when held
/// - Second parameter (tap action) can be:
///   - HID keycode identifier (e.g., `Space`, `Enter`)
///   - Action variant with arguments (e.g., `TriggerMacro(0)`, `LayerToggle(1)`)
///   - Action variant without arguments (e.g., `Action::TriLayerLower`)
///   - Fully qualified Action (e.g., `Action::TriggerMacro(0)`)
/// - `$p`: Custom `MorseProfile` for timing configuration
///
/// # Examples
/// ```ignore
/// let profile = MorseProfile::new(Some(true), None, Some(200), Some(300));
///
/// // Traditional usage - HID keycode
/// ltp!(1, Space, profile)  // Layer-tap with custom timing
///
/// // New usage - Action variants
/// ltp!(1, TriggerMacro(0), profile)  // Tap macro 0, hold layer 1
/// ltp!(2, Action::TriLayerLower, profile)  // Tap tri-layer, hold layer 2
/// ```
#[macro_export]
macro_rules! ltp {
    // Rule 1: Fully qualified Action with arguments
    ($x: literal, Action::$variant:ident($($args:expr),+), $p: expr) => {
        $crate::types::action::KeyAction::TapHold(
            $crate::types::action::Action::$variant($($args),*),
            $crate::types::action::Action::LayerOn($x),
            $p,
        )
    };

    // Rule 2: Fully qualified Action without arguments
    ($x: literal, Action::$variant:ident, $p: expr) => {
        $crate::types::action::KeyAction::TapHold(
            $crate::types::action::Action::$variant,
            $crate::types::action::Action::LayerOn($x),
            $p,
        )
    };

    // Rule 3: Bare Action variant with arguments
    ($x: literal, $variant:ident($($args:expr),+), $p: expr) => {
        $crate::types::action::KeyAction::TapHold(
            $crate::types::action::Action::$variant($($args),*),
            $crate::types::action::Action::LayerOn($x),
            $p,
        )
    };

    // Rule 4: HID keycode (backward compatible)
    ($x: literal, $k: ident, $p: expr) => {
        $crate::types::action::KeyAction::TapHold(
            $crate::types::action::Action::Key($crate::types::keycode::KeyCode::Hid(
                $crate::types::keycode::HidKeyCode::$k,
            )),
            $crate::types::action::Action::LayerOn($x),
            $p,
        )
    };
}

/// Create a modifier-tap action (tap/hold behavior).
///
/// This macro creates a dual-function key:
/// - **Tap**: Sends the specified key or triggers an action
/// - **Hold**: Applies the specified modifier(s)
///
/// Commonly used for home row mods. Uses default timing configuration.
///
/// # Parameters
/// - First parameter can be:
///   - HID keycode identifier (e.g., `A`, `Space`)
///   - Action variant with arguments (e.g., `TriggerMacro(0)`, `LayerToggle(1)`)
///   - Action variant without arguments (e.g., `Action::TriLayerLower`)
///   - Fully qualified Action (e.g., `Action::TriggerMacro(0)`)
/// - `$m`: `ModifierCombination` to apply when held
///
/// # Examples
/// ```ignore
/// // HID keycode (traditional usage)
/// mt!(A, ModifierCombination::LCTRL)   // Tap for A, hold for Ctrl
/// mt!(S, ModifierCombination::LSHIFT)  // Tap for S, hold for Shift
///
/// // Action variant with arguments (new usage)
/// mt!(TriggerMacro(0), ModifierCombination::LSHIFT)  // Tap triggers macro 0, hold for Shift
/// mt!(LayerToggle(1), ModifierCombination::LCTRL)    // Tap toggles layer 1, hold for Ctrl
///
/// // Action variant without arguments (must use fully qualified syntax)
/// mt!(Action::TriLayerLower, ModifierCombination::LSHIFT)  // Tap tri-layer lower, hold for Shift
/// mt!(Action::TriLayerUpper, ModifierCombination::LCTRL)   // Tap tri-layer upper, hold for Ctrl
///
/// // Fully qualified Action with arguments
/// mt!(Action::TriggerMacro(0), ModifierCombination::LSHIFT)  // Also works
/// ```
#[macro_export]
macro_rules! mt {
    // Rule 1: Fully qualified Action variant with arguments (e.g., Action::TriggerMacro(0))
    (Action::$variant:ident($($args:expr),+), $m:expr) => {
        $crate::types::action::KeyAction::TapHold(
            $crate::types::action::Action::$variant($($args),*),
            $crate::types::action::Action::Modifier($m),
            $crate::types::action::MorseProfile::const_default(),
        )
    };

    // Rule 2: Fully qualified Action variant without arguments (e.g., Action::TriLayerLower)
    (Action::$variant:ident, $m:expr) => {
        $crate::types::action::KeyAction::TapHold(
            $crate::types::action::Action::$variant,
            $crate::types::action::Action::Modifier($m),
            $crate::types::action::MorseProfile::const_default(),
        )
    };

    // Rule 3: Bare Action variant with arguments (e.g., TriggerMacro(0))
    ($variant:ident($($args:expr),+), $m:expr) => {
        $crate::types::action::KeyAction::TapHold(
            $crate::types::action::Action::$variant($($args),*),
            $crate::types::action::Action::Modifier($m),
            $crate::types::action::MorseProfile::const_default(),
        )
    };

    // Rule 4: HID keycode identifier (backward compatible)
    ($k:ident, $m:expr) => {
        $crate::types::action::KeyAction::TapHold(
            $crate::types::action::Action::Key($crate::types::keycode::KeyCode::Hid(
                $crate::types::keycode::HidKeyCode::$k,
            )),
            $crate::types::action::Action::Modifier($m),
            $crate::types::action::MorseProfile::const_default(),
        )
    };
}

/// Create a modifier-tap action with custom timing profile.
///
/// Same as `mt!` but allows specifying custom tap/hold timing configuration
/// through a `MorseProfile`.
///
/// # Parameters
/// - First parameter can be:
///   - HID keycode identifier (e.g., `A`, `Space`)
///   - Action variant with arguments (e.g., `TriggerMacro(0)`, `LayerToggle(1)`)
///   - Action variant without arguments (e.g., `Action::TriLayerLower`)
///   - Fully qualified Action (e.g., `Action::TriggerMacro(0)`)
/// - `$m`: `ModifierCombination` to apply when held
/// - `$p`: Custom `MorseProfile` for timing configuration
///
/// # Examples
/// ```ignore
/// let profile = MorseProfile::new(Some(false), None, Some(180), None);
///
/// // HID keycode (traditional usage)
/// mtp!(A, ModifierCombination::LCTRL, profile)
///
/// // Action variant with arguments (new usage)
/// mtp!(TriggerMacro(0), ModifierCombination::LSHIFT, profile)
///
/// // Action variant without arguments
/// mtp!(Action::TriLayerLower, ModifierCombination::LSHIFT, profile)
///
/// // Fully qualified Action with arguments
/// mtp!(Action::TriggerMacro(0), ModifierCombination::LSHIFT, profile)
/// ```
#[macro_export]
macro_rules! mtp {
    // Rule 1: Fully qualified Action variant with arguments
    (Action::$variant:ident($($args:expr),+), $m:expr, $p:expr) => {
        $crate::types::action::KeyAction::TapHold(
            $crate::types::action::Action::$variant($($args),*),
            $crate::types::action::Action::Modifier($m),
            $p,
        )
    };

    // Rule 2: Fully qualified Action variant without arguments
    (Action::$variant:ident, $m:expr, $p:expr) => {
        $crate::types::action::KeyAction::TapHold(
            $crate::types::action::Action::$variant,
            $crate::types::action::Action::Modifier($m),
            $p,
        )
    };

    // Rule 3: Bare Action variant with arguments
    ($variant:ident($($args:expr),+), $m:expr, $p:expr) => {
        $crate::types::action::KeyAction::TapHold(
            $crate::types::action::Action::$variant($($args),*),
            $crate::types::action::Action::Modifier($m),
            $p,
        )
    };

    // Rule 4: HID keycode identifier (backward compatible)
    ($k:ident, $m:expr, $p:expr) => {
        $crate::types::action::KeyAction::TapHold(
            $crate::types::action::Action::Key($crate::types::keycode::KeyCode::Hid(
                $crate::types::keycode::HidKeyCode::$k,
            )),
            $crate::types::action::Action::Modifier($m),
            $p,
        )
    };
}

/// Create a dual-key tap-hold action.
///
/// This macro creates a key with two different key behaviors:
/// - **Tap**: Sends the first key
/// - **Hold**: Sends the second key
///
/// Uses default timing configuration.
///
/// # Parameters
/// - `$t`: HID keycode to send when tapped
/// - `$h`: HID keycode to send when held
///
/// # Example
/// ```ignore
/// th!(Space, Backspace)  // Tap for Space, hold for Backspace
/// th!(Escape, Grave)     // Tap for Escape, hold for Grave
/// ```
#[macro_export]
macro_rules! th {
    ($t: ident, $h: ident) => {
        $crate::types::action::KeyAction::TapHold(
            $crate::types::action::Action::Key($crate::types::keycode::KeyCode::Hid(
                $crate::types::keycode::HidKeyCode::$t,
            )),
            $crate::types::action::Action::Key($crate::types::keycode::KeyCode::Hid(
                $crate::types::keycode::HidKeyCode::$h,
            )),
            $crate::types::action::MorseProfile::const_default(),
        )
    };
}

/// Create a dual-key tap-hold action with custom timing profile.
///
/// Same as `th!` but allows specifying custom tap/hold timing configuration.
///
/// # Parameters
/// - `$t`: HID keycode to send when tapped
/// - `$h`: HID keycode to send when held
/// - `$p`: Custom `MorseProfile` for timing configuration
///
/// # Example
/// ```ignore
/// let profile = MorseProfile::new(None, Some(MorseMode::PermissiveHold), Some(200), None);
/// thp!(Space, Backspace, profile)
/// ```
#[macro_export]
macro_rules! thp {
    ($t: ident, $h: ident, $p: expr) => {
        $crate::types::action::KeyAction::TapHold(
            $crate::types::action::Action::Key($crate::types::keycode::KeyCode::Hid(
                $crate::types::keycode::HidKeyCode::$t,
            )),
            $crate::types::action::Action::Key($crate::types::keycode::KeyCode::Hid(
                $crate::types::keycode::HidKeyCode::$h,
            )),
            $p,
        )
    };
}

/// Create a one-shot layer action.
///
/// This macro creates a key that activates a layer for the next keypress only.
/// After the next key is pressed, the layer automatically deactivates.
///
/// # Parameters
/// - `$x`: Layer number (0-255)
///
/// # Example
/// ```ignore
/// osl!(1)  // Next key will be from layer 1, then return to current layer
/// osl!(2)  // Next key will be from layer 2, then return to current layer
/// ```
#[macro_export]
macro_rules! osl {
    ($x: literal) => {
        $crate::types::action::KeyAction::Single($crate::types::action::Action::OneShotLayer($x))
    };
}

/// Create a one-shot modifier action.
///
/// This macro creates a key that applies modifiers for the next keypress only.
/// After the next key is pressed, the modifiers automatically deactivate.
///
/// # Parameters
/// - `$m`: `ModifierCombination` to apply for the next keypress
///
/// # Example
/// ```ignore
/// osm!(ModifierCombination::LSHIFT)  // Next key will be shifted
/// osm!(ModifierCombination::LCTRL)   // Next key will have Ctrl applied
/// ```
#[macro_export]
macro_rules! osm {
    ($m: expr) => {
        $crate::types::action::KeyAction::Single($crate::types::action::Action::OneShotModifier($m))
    };
}

/// Create a layer toggle action.
///
/// This macro creates a key that toggles a layer on/off with each press.
/// First press activates the layer, second press deactivates it.
///
/// # Parameters
/// - `$x`: Layer number (0-255)
///
/// # Example
/// ```ignore
/// tg!(1)  // Toggle layer 1 on/off
/// tg!(2)  // Toggle layer 2 on/off
/// ```
#[macro_export]
macro_rules! tg {
    ($x: literal) => {
        $crate::types::action::KeyAction::Single($crate::types::action::Action::LayerToggle($x))
    };
}

/// Create a layer tap-toggle action.
///
/// This macro creates a dual-function key:
/// - **Tap**: Toggles the layer on/off
/// - **Hold**: Momentarily activates the layer (like `mo!`)
///
/// # Parameters
/// - `$x`: Layer number (0-255)
///
/// # Example
/// ```ignore
/// tt!(1)  // Tap to toggle layer 1, hold to activate momentarily
/// tt!(2)  // Tap to toggle layer 2, hold to activate momentarily
/// ```
#[macro_export]
macro_rules! tt {
    ($x: literal) => {
        $crate::types::action::KeyAction::TapHold(
            $crate::types::action::Action::LayerToggle($x),
            $crate::types::action::Action::LayerOn($x),
            $crate::types::action::MorseProfile::const_default(),
        )
    };
}

/// Create a layer tap-toggle action with custom timing profile.
///
/// Same as `tt!` but allows specifying custom tap/hold timing configuration.
///
/// # Parameters
/// - `$x`: Layer number (0-255)
/// - `$p`: Custom `MorseProfile` for timing configuration
#[macro_export]
macro_rules! ttp {
    ($x: literal, $p: expr) => {
        $crate::types::action::KeyAction::TapHold(
            $crate::types::action::Action::LayerToggle($x),
            $crate::types::action::Action::LayerOn($x),
            $p,
        )
    };
}

/// Create a "to layer" action (exclusive layer activation).
///
/// This macro activates the specified layer and deactivates all other layers
/// (except the default layer). This creates an exclusive layer switch.
///
/// # Parameters
/// - `$x`: Layer number (0-255)
///
/// # Example
/// ```ignore
/// to!(1)  // Switch exclusively to layer 1
/// to!(0)  // Return to base layer
/// ```
#[macro_export]
macro_rules! to {
    ($x: literal) => {
        $crate::types::action::KeyAction::Single($crate::types::action::Action::LayerToggleOnly($x))
    };
}

/// Create a default layer switch action.
///
/// This macro sets the default/base layer. The default layer is always active
/// and serves as the fallback when no other layers are active.
///
/// # Parameters
/// - `$x`: Layer number (0-255)
///
/// # Example
/// ```ignore
/// df!(0)  // Set layer 0 as the default layer
/// df!(1)  // Set layer 1 as the default layer (e.g., for Dvorak layout)
/// ```
#[macro_export]
macro_rules! df {
    ($x: literal) => {
        $crate::types::action::KeyAction::Single($crate::types::action::Action::DefaultLayer($x))
    };
}

/// Create a shifted key action.
///
/// This is a convenience macro that creates a key with left shift applied.
/// Equivalent to `wm!($x, ModifierCombination::LSHIFT)`.
///
/// # Parameters
/// - `$x`: HID keycode identifier
///
/// # Example
/// ```ignore
/// shifted!(A)      // Sends Shift+A (uppercase 'A')
/// shifted!(Kc1)    // Sends Shift+1 (exclamation mark '!')
/// shifted!(Slash)  // Sends Shift+/ (question mark '?')
/// ```
#[macro_export]
macro_rules! shifted {
    ($x: ident) => {
        $crate::wm!(
            $x,
            $crate::types::modifier::ModifierCombination::new_from(false, false, false, true, false)
        )
    };
}

/// Create a rotary encoder action.
///
/// This macro defines the behavior of a rotary encoder, specifying different
/// actions for clockwise and counter-clockwise rotation.
///
/// # Parameters
/// - `$clockwise`: `KeyAction` to execute on clockwise rotation
/// - `$counter_clockwise`: `KeyAction` to execute on counter-clockwise rotation
///
/// # Example
/// ```ignore
/// // Volume control encoder
/// encoder!(
///     k!(KbVolumeUp),     // Clockwise increases volume
///     k!(KbVolumeDown)    // Counter-clockwise decreases volume
/// )
///
/// // Scroll encoder
/// encoder!(
///     k!(MouseWheelUp),
///     k!(MouseWheelDown)
/// )
/// ```
#[macro_export]
macro_rules! encoder {
    ($clockwise: expr, $counter_clockwise: expr) => {
        $crate::types::action::EncoderAction::new($clockwise, $counter_clockwise)
    };
}

/// Create a tap dance action (Morse action).
///
/// This macro creates a reference to a tap dance configuration by index.
/// Tap dance allows multiple actions based on the number of taps.
/// In Vial, this appears as "Tap Dance".
///
/// # Parameters
/// - `$index`: Index of the tap dance configuration (0-255)
///
/// # Example
/// ```ignore
/// td!(0)  // References tap dance configuration at index 0
/// td!(1)  // References tap dance configuration at index 1
/// ```
///
/// # Note
/// The actual tap dance behavior must be configured separately in the
/// keyboard's tap dance configuration array.
#[macro_export]
macro_rules! td {
    ($index: literal) => {
        $crate::types::action::KeyAction::Morse($index)
    };
}

/// Create a Morse action (alias for tap dance).
///
/// This is an alias for `td!` macro. "Morse" is the internal name for
/// the superset of tap dance and tap-hold functionality.
///
/// # Parameters
/// - `$index`: Index of the Morse configuration (0-255)
///
/// # Example
/// ```ignore
/// morse!(0)  // Same as td!(0)
/// ```
#[macro_export]
macro_rules! morse {
    ($index: literal) => {
        $crate::types::action::KeyAction::Morse($index)
    };
}

/// Create a macro trigger action.
///
/// This macro creates a key that triggers a predefined macro sequence by index.
/// Macros can send multiple keypresses or perform complex sequences.
///
/// # Parameters
/// - `$index`: Index of the macro (0-255)
///
/// # Example
/// ```ignore
/// macros!(0)  // Triggers macro at index 0
/// macros!(1)  // Triggers macro at index 1
/// ```
#[macro_export]
macro_rules! macros {
    ($index: literal) => {
        $crate::types::action::KeyAction::Single($crate::types::action::Action::TriggerMacro($index))
    };
}

/// Create a user key action with given index.
///
/// # Parameters
/// - `$index`: Index of the user key (0-31)
#[macro_export]
macro_rules! user {
    ($index: literal) => {
        $crate::types::action::KeyAction::Single($crate::types::action::Action::User($index))
    };
}

/// Create a keyboard control action.
///
/// This macro creates actions for system-level keyboard operations.
///
/// # Available Actions
/// - Bootloader: Enter bootloader mode for firmware updates
/// - Reboot: Reboot the keyboard
/// - DebugToggle: Toggle debug mode
/// - ClearEeprom: Clear EEPROM storage
/// - OutputAuto: Auto-select output (USB/Bluetooth)
/// - OutputUsb: Force USB output
/// - OutputBluetooth: Force Bluetooth output
/// - ComboOn: Enable combos
/// - ComboOff: Disable combos
/// - ComboToggle: Toggle combos
/// - CapsWordToggle: Toggle caps word mode
///
/// # Example (internal use only)
/// ```ignore
/// kbctrl!(Bootloader)
/// kbctrl!(OutputUsb)
/// ```
#[macro_export]
macro_rules! kbctrl {
    ($action: ident) => {
        $crate::types::action::KeyAction::Single($crate::types::action::Action::KeyboardControl(
            $crate::types::action::KeyboardAction::$action,
        ))
    };
}

/// Create a light control action.
///
/// This macro creates light control actions for backlight and RGB lighting.
///
/// # Available Actions
/// Backlight: BacklightOn, BacklightOff, BacklightToggle, BacklightDown,
/// BacklightUp, BacklightStep, BacklightToggleBreathing
///
/// RGB: RgbTog, RgbModeForward, RgbModeReverse, RgbHui, RgbHud, RgbSai,
/// RgbSad, RgbVai, RgbVad, RgbSpi, RgbSpd, RgbModePlain, RgbModeBreathe,
/// RgbModeRainbow, RgbModeSwirl, RgbModeSnake, RgbModeKnight, RgbModeXmas,
/// RgbModeGradient, RgbModeRgbtest, RgbModeTwinkle
///
/// # Example (internal use only)
/// ```ignore
/// light!(BacklightOn)
/// light!(RgbTog)
/// ```
#[macro_export]
macro_rules! light {
    ($action: ident) => {
        $crate::types::action::KeyAction::Single($crate::types::action::Action::Light(
            $crate::types::action::LightAction::$action,
        ))
    };
}

/// Create a special key action.
///
/// This macro creates special key actions that are not in the HID spec
/// but are commonly used.
///
/// # Available Actions
/// - GraveEscape: Acts as Grave when pressed alone, Escape when pressed with Shift/Ctrl/Alt/GUI
/// - Repeat: Repeats the last key pressed
///
/// # Example (internal use only)
/// ```ignore
/// special!(GraveEscape)
/// special!(Repeat)
/// ```
#[macro_export]
macro_rules! special {
    ($key: ident) => {
        $crate::types::action::KeyAction::Single($crate::types::action::Action::Special(
            $crate::types::keycode::SpecialKey::$key,
        ))
    };
}
