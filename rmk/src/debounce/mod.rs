use crate::matrix::KeyState;

pub mod default_debouncer;
pub mod fast_debouncer;

pub trait DebouncerTrait<const ROW: usize, const COL: usize> {
    fn detect_change_with_debounce(
        &mut self,
        row_idx: usize,
        col_idx: usize,
        key_active: bool,     // Hardware key active signal
        key_state: &KeyState, // Current key state
    ) -> DebounceState;
}

/// Debounce state
pub enum DebounceState {
    Debounced,
    InProgress,
    Ignored,
}
