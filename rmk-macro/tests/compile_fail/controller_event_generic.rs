// Test that #[controller_event] rejects generic types.
// Static channels cannot be generic, so this should fail to compile.

use rmk_macro::controller_event;

#[controller_event]
#[derive(Clone, Copy, Debug)]
pub struct GenericControllerEvent<T> {
    pub value: T,
}

fn main() {}
