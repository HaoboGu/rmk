use crate::keycode::KeyCode;

// Default macro space size
pub(crate) type MacroSpaceSize = typenum::U256;

// Default number of keyboard macros
pub(crate) const NumMacro: usize = 8;

pub(crate) enum MacroOperation {
    Press(KeyCode),
    Release(KeyCode),
    Tap(KeyCode),
    Text(KeyCode, bool),
    Delay(u16),
    End,
}
