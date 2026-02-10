// Test that #[input_event] rejects generic types.
// Static channels cannot be generic, so this should fail to compile.

use rmk_macro::input_event;

#[input_event]
#[derive(Clone, Copy, Debug)]
pub struct GenericInputEvent<T> {
    pub value: T,
}

fn main() {}
