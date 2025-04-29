use crate::matrix::KeyState;

pub mod default_debouncer;
pub mod fast_debouncer;

pub trait DebouncerTrait {
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
