use rmk_macro::input_processor;

#[derive(Clone, Copy, Debug)]
pub struct KeyEvent {
    pub row: u8,
    pub col: u8,
    pub pressed: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct EncoderEvent {
    pub index: u8,
    pub direction: i8,
}

#[input_processor(subscribe = [KeyEvent, EncoderEvent])]
pub struct KeyProcessor<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize> {
    pub keymap: &'a core::cell::RefCell<()>,
}
