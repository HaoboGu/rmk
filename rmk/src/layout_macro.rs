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
        Action::Key(KeyCode::$k)
    };
}

#[macro_export]
macro_rules! a {
    ($a: ident) => {
        Action::$a
    };
}

/// Macro for mouse key action
#[macro_export]
macro_rules! mk {
    ($k: ident) => {
        Action::MouseKey(KeyCode::$k)
    };
}

/// Macro for system control action
#[macro_export]
macro_rules! sc {
    ($k: ident) => {
        Action::SystemControl(KeyCode::$k)
    };
}

/// Macro for consumer control action
#[macro_export]
macro_rules! cc {
    ($k: ident) => {
        Action::ConsumerControl(KeyCode::$k)
    };
}

/// Macro for layer activate
#[macro_export]
macro_rules! mo {
    ($x: literal) => {
        Action::LayerActivate($x)
    };
}

/// Macro for layer activate with modifier
#[macro_export]
macro_rules! lm {
    ($x: literal, $m: expr) => {
        Action::LayerMods($x, $m)
    };
}

/// Macro for layer or tap key
#[macro_export]
macro_rules! lt {
    ($x: literal, $k: ident) => {
        Action::LayerOrTapKey($x, KeyCode::$k)
    };
}

/// Macro for oneshot layer
#[macro_export]
macro_rules! osl {
    ($x: literal) => {
        Action::OneShotLayer($x)
    };
}

/// Macro for layer toggle
#[macro_export]
macro_rules! tg {
    ($x: literal) => {
        Action::LayerToggle($x)
    };
}

/// Macro for layer or tap toggle
#[macro_export]
macro_rules! tt {
    ($x: literal) => {
        Action::LayerOrTapToggle($x)
    };
}
