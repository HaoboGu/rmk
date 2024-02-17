/// Create a layer in keymap
#[macro_export]
macro_rules! layer {
    ([$([$($x: expr), +]), +]) => {
        [$([$($x), +]),+]
    };
}

/// Create a normal keycode
#[macro_export]
macro_rules! k {
    ($k: ident) => {
        $crate::action::KeyAction::Single($crate::action::Action::Key($crate::keycode::KeyCode::$k))
    };
}

/// Create a normal action
#[macro_export]
macro_rules! a {
    ($a: ident) => {
        $crate::action::KeyAction::$a
    };
}

/// Create a mouse key action
#[macro_export]
macro_rules! mk {
    ($k: ident) => {
        $crate::action::KeyAction::Single($crate::action::Action::MouseKey(
            $crate::keycode::KeyCode::$k,
        ))
    };
}

/// Create a control action
#[macro_export]
macro_rules! sc {
    ($k: ident) => {
        $crate::action::KeyAction::Single($crate::action::Action::SystemControl(
            $crate::keycode::KeyCode::$k,
        ))
    };
}

/// Create a consumer control(media) action
#[macro_export]
macro_rules! cc {
    ($k: ident) => {
        $crate::action::KeyAction::Single($crate::action::Action::ConsumerControl(
            $crate::keycode::KeyCode::$k,
        ))
    };
}

/// Create a layer activate action
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
        $crate::action::KeyAction::TapHold(
            $crate::keycode::KeyCode::$k,
            $crate::action::Action::LayerOn($x),
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
        $crate::action::TapHold(
            $crate::action::Action::LayerToggle($x),
            $crate::action::Action::LayerOn($x),
        )
    };
}
