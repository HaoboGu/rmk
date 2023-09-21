/// Macro for creating layout of actions
#[macro_export]
macro_rules! layer {
    ([$([$($x: expr), +]), +]) => {
        [$([$($x), +]),+]
    };
}

#[macro_export]
macro_rules! k {
    ($k: ident) => {
        $crate::action::KeyAction::Single($crate::action::Action::Key($crate::keycode::KeyCode::$k))
    };
}

#[macro_export]
macro_rules! a {
    ($a: ident) => {
        $crate::action::KeyAction::$a
    };
}

/// Macro for mouse key action
#[macro_export]
macro_rules! mk {
    ($k: ident) => {
        $crate::action::KeyAction::Single($crate::action::Action::MouseKey(
            $crate::keycode::KeyCode::$k,
        ))
    };
}

/// Macro for system control action
#[macro_export]
macro_rules! sc {
    ($k: ident) => {
        $crate::action::KeyAction::Single($crate::action::Action::SystemControl(
            $crate::keycode::KeyCode::$k,
        ))
    };
}

/// Macro for consumer control action
#[macro_export]
macro_rules! cc {
    ($k: ident) => {
        $crate::action::KeyAction::Single($crate::action::Action::ConsumerControl(
            $crate::keycode::KeyCode::$k,
        ))
    };
}

/// Macro for layer activate
#[macro_export]
macro_rules! mo {
    ($x: literal) => {
        $crate::action::KeyAction::Single($crate::action::Action::LayerOn($x))
    };
}

/// Macro for layer activate with modifier
#[macro_export]
macro_rules! lm {
    ($x: literal, $m: expr) => {
        $crate::action::KeyAction::WithModifier($crate::action::Action::LayerOn($x), $m)
    };
}

/// Macro for layer or tap key
#[macro_export]
macro_rules! lt {
    ($x: literal, $k: ident) => {
        $crate::action::KeyAction::TapHold(
            $crate::keycode::KeyCode::$k,
            $crate::action::Action::LayerOn($x),
        )
    };
}

/// Macro for oneshot layer
#[macro_export]
macro_rules! osl {
    ($x: literal) => {
        $crate::action::KeyAction::OneShot($crate::action::Action::LayerOn($x))
    };
}

/// Macro for layer toggle
#[macro_export]
macro_rules! tg {
    ($x: literal) => {
        $crate::action::KeyAction::Single($crate::action::Action::LayerToggle($x))
    };
}

/// Macro for layer or tap toggle
#[macro_export]
macro_rules! tt {
    ($x: literal) => {
        $crate::action::TapHold(
            $crate::action::Action::LayerToggle($x),
            $crate::action::Action::LayerOn($x),
        )
    };
}
