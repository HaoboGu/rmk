use crate::matrix::KeyState;

pub mod fast_debouncer;
pub mod default_bouncer;

/// Default DEBOUNCE_THRESHOLD in ms.
static DEBOUNCE_THRESHOLD: u16 = 10;

pub(crate) trait DebouncerTrait {
    fn new() -> Self;
    fn detect_change_with_debounce(
        &mut self,
        in_idx: usize,
        out_idx: usize,
        pin_state: bool,
        key_state: &KeyState,
    ) -> DebounceState;
}


/// Debounce state
pub(crate) enum DebounceState {
    Debounced,
    InProgress,
    Ignored,
}