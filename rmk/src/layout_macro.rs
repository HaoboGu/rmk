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
        $crate::action::KeyAction::Single($crate::action::Action::Key($crate::keycode::KeyCode::$k))
    };
}

/// Create a normal key with modifier action
#[macro_export]
macro_rules! wm {
    ($x: ident, $m: expr) => {
        $crate::action::KeyAction::WithModifier(
            $crate::action::Action::Key($crate::keycode::KeyCode::$x),
            $m,
        )
    };
}

/// Create a normal action: `KeyAction`
#[macro_export]
macro_rules! a {
    ($a: ident) => {
        $crate::action::KeyAction::$a
    };
}

/// Create a layer activate action. For example, `mo!(1)` activates layer 1.
#[macro_export]
macro_rules! mo {
    ($x: literal) => {
        $crate::action::KeyAction::Single($crate::action::Action::LayerOn($x))
    };
}

/// Create a layer activate with modifier action
#[macro_export]
macro_rules! lm {
    ($x: literal, $m: expr) => {
        $crate::action::KeyAction::WithModifier($crate::action::Action::LayerOn($x), $m)
    };
}

/// Create a layer activate action or tap key(tap/hold)
#[macro_export]
macro_rules! lt {
    ($x: literal, $k: ident) => {
        $crate::action::KeyAction::LayerTapHold(
            $crate::action::Action::Key($crate::keycode::KeyCode::$k),
            $x,
        )
    };
}

/// Create an oneshot layer key in keymap
#[macro_export]
macro_rules! osl {
    ($x: literal) => {
        $crate::action::KeyAction::OneShot($crate::action::Action::LayerOn($x))
    };
}

/// Create a layer toggle action
#[macro_export]
macro_rules! tg {
    ($x: literal) => {
        $crate::action::KeyAction::Single($crate::action::Action::LayerToggle($x))
    };
}

/// Create a layer activate or tap toggle action
#[macro_export]
macro_rules! tt {
    ($x: literal) => {
        $crate::action::KeyAction::LayerTapHold($crate::action::Action::LayerToggle($x), $x)
    };
}

/// Create a layer toggle only action (activate layer `n` and deactivate all other layers), `n` is the layer number
#[macro_export]
macro_rules! to {
    ($x: literal) => {
        $crate::action::KeyAction::Single($crate::action::Action::LayerToggleOnly($x))
    };
}

/// create a switch default layer action, `n` is the layer number
#[macro_export]
macro_rules! df {
    ($x: literal) => {
        $crate::action::KeyAction::Single($crate::action::Action::DefaultLayer($x))
    };
}