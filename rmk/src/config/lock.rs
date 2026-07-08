/// Rynk lock-gate configuration, emitted by the macro from `[host]` in
/// keyboard.toml. `unlock_keys` empty ⇒ dangerous ops are permanently locked.
#[derive(Clone, Copy, Debug, Default)]
pub struct LockConfig {
    /// Physical `(row, col)` keys held simultaneously to unlock (max
    /// `UNLOCK_KEYS_SIZE`). Empty ⇒ no unlock possible.
    pub unlock_keys: &'static [(u8, u8)],
    /// Start (and stay) unlocked — development escape hatch.
    pub insecure: bool,
    /// Move the config-write tier (`SetKeyAction`, `SetMacro`, …) into the
    /// locked set.
    pub write_requires_unlock: bool,
}
