use crate::keycode::KeyCode;

// Default macro space size
pub(crate) const MACRO_SPACE_SIZE: usize = 256;

// Default number of keyboard macros
pub(crate) const DEFAULT_NUM_MACRO: usize = 8;

pub(crate) enum MacroOperation {
    Press(KeyCode),
    Release(KeyCode),
    Tap(KeyCode),
    Text(KeyCode, bool),
    Delay(u16),
    End,
}
