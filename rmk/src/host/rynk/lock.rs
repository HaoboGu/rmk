//! Physical-key unlock state machine for rynk.
//!
//! Mirrors `rmk/src/host/via/vial_lock.rs` but driven by the rynk protocol's
//! `UnlockRequest` / `LockRequest` endpoints. Extracting a shared
//! `HostLock` between Vial and rynk is an open follow-up.

use crate::keymap::KeyMap;

pub(crate) struct RynkLock<'a> {
    #[allow(dead_code)]
    keymap: &'a KeyMap<'a>,
}

impl<'a> RynkLock<'a> {
    pub(crate) fn new(keymap: &'a KeyMap<'a>) -> Self {
        Self { keymap }
    }
}
