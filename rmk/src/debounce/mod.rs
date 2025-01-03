use crate::matrix::KeyState;

pub mod default_bouncer;
pub mod fast_debouncer;

/// Default DEBOUNCE_THRESHOLD in ms.
static DEBOUNCE_THRESHOLD: u16 = 10;

pub trait DebouncerTrait {
    fn new() -> Self;

    /// The `in_idx` `out_idx` can be used as two normal dimensions.
    fn detect_change_with_debounce(
        &mut self,
        in_idx: usize,
        out_idx: usize,
        pin_state: bool,
        key_state: &KeyState,
    ) -> DebounceState;
}

/// Debounce state
pub enum DebounceState {
    Debounced,
    InProgress,
    Ignored,
}
