use crate::keycode::KeyCode;
pub(crate) enum MacroOperation {
    Press(KeyCode),
    Release(KeyCode),
    Tap(KeyCode),
    Text(KeyCode, bool),
    Delay(u16),
    End,
}
