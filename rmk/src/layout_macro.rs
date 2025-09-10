/// Create a layer in keymap
#[macro_export]
macro_rules! layer {
    ([$([$($x: expr), +]), +]) => {
        [$([$($x), +]),+]
    };
}

/// Create a normal key. For example, `k!(A)` represents `KeyAction::Single(Action::Key(KeyCode::A))`
#[macro_export]
macro_rules! k {
    ($k: ident) => {
        $crate::types::action::KeyAction::Single($crate::types::action::Action::Key(
            $crate::types::keycode::KeyCode::$k,
        ))
    };
}

/// Create a normal key with modifier action
#[macro_export]
macro_rules! wm {
    ($x: ident, $m: expr) => {
        $crate::types::action::KeyAction::Single($crate::types::action::Action::KeyWithModifier(
            $crate::types::keycode::KeyCode::$x,
            $m,
        ))
    };
}

/// Create a normal action: `KeyAction`
#[macro_export]
macro_rules! a {
    ($a: ident) => {
        $crate::types::action::KeyAction::$a
    };
}

/// Create a layer activate action. For example, `mo!(1)` activates layer 1.
#[macro_export]
macro_rules! mo {
    ($x: literal) => {
        $crate::types::action::KeyAction::Single($crate::types::action::Action::LayerOn($x))
    };
}

/// Create a layer activate with modifier action
#[macro_export]
macro_rules! lm {
    ($x: literal, $m: expr) => {
        $crate::types::action::KeyAction::Single($crate::types::action::Action::LayerOnWithModifier($x, $m))
    };
}

/// Create a layer activate action or tap key(tap/hold)
#[macro_export]
macro_rules! lt {
    ($x: literal, $k: ident) => {
        $crate::types::action::KeyAction::TapHold(
            $crate::types::action::Action::Key($crate::types::keycode::KeyCode::$k),
            $crate::types::action::Action::LayerOn($x),
            $crate::types::action::MorseProfile::const_default(),
        )
    };
}
/// Create a layer activate action or tap key(tap/hold) with profile
#[macro_export]
macro_rules! ltp {
    ($x: literal, $k: ident, $p: expr) => {
        $crate::types::action::KeyAction::TapHold(
            $crate::types::action::Action::Key($crate::types::keycode::KeyCode::$k),
            $crate::types::action::Action::LayerOn($x),
            $p,
        )
    };
}

/// Create a modifier-tap-hold action
#[macro_export]
macro_rules! mt {
    ($k: ident, $m: expr) => {
        $crate::types::action::KeyAction::TapHold(
            $crate::types::action::Action::Key($crate::types::keycode::KeyCode::$k),
            $crate::types::action::Action::Modifier($m),
            $crate::types::action::MorseProfile::const_default(),
        )
    };
}
/// Create a modifier-tap-hold action with profile
#[macro_export]
macro_rules! mtp {
    ($k: ident, $m: expr, $p: expr) => {
        $crate::types::action::KeyAction::TapHold(
            $crate::types::action::Action::Key($crate::types::keycode::KeyCode::$k),
            $crate::types::action::Action::Modifier($m),
            $p,
        )
    };
}

/// Create a tap-hold action
#[macro_export]
macro_rules! th {
    ($t: ident, $h: ident) => {
        $crate::types::action::KeyAction::TapHold(
            $crate::types::action::Action::Key($crate::types::keycode::KeyCode::$t),
            $crate::types::action::Action::Key($crate::types::keycode::KeyCode::$h),
            $crate::types::action::MorseProfile::const_default(),
        )
    };
}
/// Create a tap-hold action with profile
#[macro_export]
macro_rules! thp {
    ($t: ident, $h: ident, $p: expr) => {
        $crate::types::action::KeyAction::TapHold(
            $crate::types::action::Action::Key($crate::types::keycode::KeyCode::$t),
            $crate::types::action::Action::Key($crate::types::keycode::KeyCode::$h),
            $p,
        )
    };
}

/// Create an oneshot layer key in keymap
#[macro_export]
macro_rules! osl {
    ($x: literal) => {
        $crate::types::action::KeyAction::Single($crate::types::action::Action::OneShotLayer($x))
    };
}

/// Create an oneshot modifier key in keymap
#[macro_export]
macro_rules! osm {
    ($m: expr) => {
        $crate::types::action::KeyAction::Single($crate::types::action::Action::OneShotModifier($m))
    };
}

/// Create a layer toggle action
#[macro_export]
macro_rules! tg {
    ($x: literal) => {
        $crate::types::action::KeyAction::Single($crate::types::action::Action::LayerToggle($x))
    };
}

/// Create a layer activate or tap toggle action
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
/// Create a layer activate or tap toggle action with profile
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

/// Create a layer toggle only action (activate layer `n` and deactivate all other layers), `n` is the layer number
#[macro_export]
macro_rules! to {
    ($x: literal) => {
        $crate::types::action::KeyAction::Single($crate::types::action::Action::LayerToggleOnly($x))
    };
}

/// create a switch default layer action, `n` is the layer number
#[macro_export]
macro_rules! df {
    ($x: literal) => {
        $crate::types::action::KeyAction::Single($crate::types::action::Action::DefaultLayer($x))
    };
}

/// Create a shifted key
#[macro_export]
macro_rules! shifted {
    ($x: ident) => {
        $crate::wm!(
            $x,
            $crate::types::modifier::ModifierCombination::new_from(false, false, false, true, false)
        )
    };
}

/// Create an encoder action, the first argument is the clockwise action, the second is the counter-clockwise action
#[macro_export]
macro_rules! encoder {
    ($clockwise: expr, $counter_clockwise: expr) => {
        $crate::types::action::EncoderAction::new($clockwise, $counter_clockwise)
    };
}

/// Create a Morse(index) action (in Vial its simplest form is known as "Tap Dance", so `td` name is used)
#[macro_export]
macro_rules! td {
    ($index: literal) => {
        $crate::types::action::KeyAction::Morse($index)
    };
}

/// Create a Morse(index) action (in Vial it will appear as "Tap Dance")
#[macro_export]
macro_rules! morse {
    ($index: literal) => {
        $crate::types::action::KeyAction::Morse($index)
    };
}

// Create a macro trigger action
// Use `macros` because `macro` is a key word in Rust
#[macro_export]
macro_rules! macros {
    ($index: literal) => {
        $crate::action::KeyAction::Single($crate::action::Action::TriggerMacro($index))
    };
}
