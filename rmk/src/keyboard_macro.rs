use crate::keycode::KeyCode;

// Default macro space size
pub(crate) const MACRO_SPACE_SIZE: usize = 512;

pub enum MacroOperation {
    /// This bit is the start of a macro, with macro length
    Start(u8),
    Press(KeyCode),
    Release(KeyCode),
    Tap(KeyCode),
    Text(KeyCode),
    End,
}
