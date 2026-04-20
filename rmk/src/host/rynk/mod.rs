use crate::keymap::KeyMap;

// TODO: Basic rynk service
pub(crate) struct RynkService<'a> {
    keymap: &'a KeyMap<'a>,
}
