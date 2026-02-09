use rmk_macro::event;

#[event]
#[derive(Clone, Copy, Debug)]
pub struct GenericEvent<T> {
    pub value: T,
}

fn main() {}
