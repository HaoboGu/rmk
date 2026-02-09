use rmk_macro::input_processor;

#[derive(Clone, Copy, Debug)]
pub struct KeyEvent {
    pub row: u8,
    pub col: u8,
    pub pressed: bool,
}

#[input_processor(subscribe = [KeyEvent])]
pub struct SingleEventInputProcessor;
