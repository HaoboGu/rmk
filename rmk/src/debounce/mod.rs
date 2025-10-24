use crate::matrix::KeyState;

pub mod default_debouncer;
pub mod fast_debouncer;

pub trait DebouncerTrait<const ROW: usize, const COL: usize> {
    /// The `in_idx` `out_idx` can be used as two normal dimensions.
    fn detect_change_with_debounce(
        &mut self,
        row_idx: usize,
        col_idx: usize,
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
